"""Domain handler for the common settings update endpoint.

Kept separate from HTTP route registration so routes.py remains an assembly layer.
"""

import copy
import os
import tempfile

from dotenv import load_dotenv
from fastapi import Request
from fastapi.responses import JSONResponse
from loguru import logger
from pydantic import ValidationError as _PydanticValidationError

from ..config_manager.main import Config as AppConfig
from ..logsetup import get_logger
from ..services.llm_routes_helpers import normalize_provider_id
from ..services.tts_routes_helpers import normalize_tts_id
from ..validators.log_level import validate_log_level

_log = get_logger("config")


async def update_common_config(
    request: Request,
    *,
    config_service,
    config_helper,
    env_path: str,
    llm_defaults: dict,
    model_catalog: dict,
    tts_voice_catalog: dict,
    tts_model_catalog: dict,
    default_context_cache=None,
    ws_handler=None,
):
        try:
            payload = await request.json()
            data = config_service.load_yaml()

            cc = data.setdefault("character_config", {})
            agent_cfg = cc.setdefault("agent_config", {})
            settings = agent_cfg.setdefault("agent_settings", {})
            basic = settings.setdefault("basic_memory_agent", {})

            if payload.get("llm_provider"):
                basic["llm_provider"] = payload["llm_provider"]
            if payload.get("tts_model"):
                requested_tts = normalize_tts_id(payload["tts_model"])
                cc.setdefault("tts_config", {})["tts_model"] = requested_tts
            if payload.get("viseme_mode"):
                cc.setdefault("tts_config", {})["viseme_mode"] = payload["viseme_mode"]
            if payload.get("asr_model"):
                cc.setdefault("asr_config", {})["asr_model"] = payload["asr_model"]
            if payload.get("character_name"):
                cc["character_name"] = payload["character_name"]
            if payload.get("port"):
                data.setdefault("system_config", {})["port"] = int(payload["port"])
            if payload.get("live2d_fps"):
                data.setdefault("system_config", {})["live2d_fps"] = int(
                    payload["live2d_fps"]
                )
            # 开发者模式：仅接受布尔值；显式 false 也要落盘（用户关闭开关）。
            if isinstance(payload.get("developer_mode"), bool):
                prev_dev = bool(
                    (data.get("system_config") or {}).get("developer_mode", False)
                )
                data.setdefault("system_config", {})["developer_mode"] = payload[
                    "developer_mode"
                ]
                new_dev = bool(payload["developer_mode"])
                if new_dev != prev_dev:
                    _log.info(
                        "developer_mode toggled: {} → {}", prev_dev, new_dev
                    )

            # 更新 LLM Provider 参数（model/base_url/temperature/max_tokens）
            llm_overrides = payload.get("llm_configs") or {}
            existing_llm = agent_cfg.setdefault("llm_configs", {})
            for raw_pid, override in list(llm_overrides.items()):
                pid = normalize_provider_id(raw_pid)
                if not isinstance(override, dict):
                    continue
                if pid not in existing_llm:
                    if not override.get("model"):
                        continue
                    target = existing_llm[pid] = {}
                    new_key_env = llm_defaults.get(pid, {}).get("key_env") or "OPENAI_API_KEY"
                    target["llm_api_key"] = "${" + new_key_env + "}"
                else:
                    target = existing_llm[pid]
                # 思考模式 / top_p 一并持久化（此前白名单漏掉 → 设置页开了思考
                # 但 conf.yaml 不落盘，重载后"思考模式无法开启"）
                for field in (
                    "model",
                    "base_url",
                    "temperature",
                    "max_tokens",
                    "models",
                    "top_p",
                    "thinking_enabled",
                    "thinking_effort",
                ):
                    if field in override and override[field] not in (None, ""):
                        target[field] = override[field]

            # 自动补齐当前选中 LLM 的最小配置（未配置但被选中的 Provider 也能保存/热重载）
            selected_llm = normalize_provider_id(basic.get("llm_provider", ""))
            if selected_llm != basic.get("llm_provider", ""):
                basic["llm_provider"] = selected_llm
            if selected_llm and selected_llm not in existing_llm:
                llm_default = llm_defaults.get(selected_llm, {})
                target = existing_llm[selected_llm] = {}
                key_env = llm_default.get("key_env") or "OPENAI_API_KEY"
                target["llm_api_key"] = "${" + key_env + "}"
                if llm_default.get("base_url"):
                    target["base_url"] = llm_default["base_url"]
                catalog_models = model_catalog.get(selected_llm, [])
                if catalog_models:
                    target.setdefault("model", catalog_models[0])
                    target.setdefault("models", catalog_models)
                target.setdefault("temperature", 0.8)

            # 更新 TTS Provider 参数（voice/model/ws_url；Melo 额外保存 language/device/韵律参数）
            tts_overrides = payload.get("tts_configs") or {}
            tts_cfg = cc.setdefault("tts_config", {})
            for pid, override in tts_overrides.items():
                if not isinstance(override, dict):
                    continue
                has_any = any(
                    override.get(field) not in (None, "")
                    for field in (
                        "voice", "model", "ws_url", "voices", "models",
                        "language", "device", "speaker", "speed", "sdp_ratio",
                        "noise_scale", "noise_scale_w", "format",
                    )
                )
                if pid not in tts_cfg and not has_any:
                    continue
                target = tts_cfg.setdefault(pid, {})
                for field in ("voice", "model", "ws_url", "voices", "models"):
                    if field in override and override[field] not in (None, ""):
                        target[field] = override[field]
                if pid == "melo_tts":
                    # UI 用 voice/model 承载 language/device；落盘成 Melo 的正式字段。
                    if override.get("language") not in (None, ""):
                        target["language"] = override["language"]
                    elif override.get("voice") not in (None, ""):
                        target["language"] = override["voice"]
                    if override.get("device") not in (None, ""):
                        target["device"] = override["device"]
                    elif override.get("model") not in (None, ""):
                        target["device"] = override["model"]
                    for field in (
                        "speaker", "speed", "sdp_ratio", "noise_scale",
                        "noise_scale_w", "format",
                    ):
                        if field in override and override[field] not in (None, ""):
                            target[field] = override[field]
                    target.pop("voice", None)
                    target.pop("model", None)

            if payload.get("tts_model"):
                requested_tts = normalize_tts_id(payload["tts_model"])
                if requested_tts not in tts_cfg:
                    target = tts_cfg[requested_tts] = {}
                    if requested_tts == "melo_tts":
                        target.update(
                            {
                                "language": "ZH",
                                "device": "cpu",
                                "speaker": "",
                                "speed": 1.0,
                                "sdp_ratio": 0.2,
                                "noise_scale": 0.6,
                                "noise_scale_w": 0.8,
                                "format": "wav",
                            }
                        )
                    else:
                        tts_voices = tts_voice_catalog.get(requested_tts, [])
                        tts_models = tts_model_catalog.get(requested_tts, [])
                        if tts_voices:
                            target["voice"] = tts_voices[0]
                            target["voices"] = tts_voices
                        if tts_models:
                            target["model"] = tts_models[0]
                            target["models"] = tts_models

            # 自动补齐选中 TTS provider 的必填字段（voice/model）目录默认值。
            # 修复 P0-1：`gpt_sovits_tts`/`edge_tts` 等被选中但缺 voice/model 时，
            # 用目录默认值补齐；若目录也给不出且该字段必填，随后 AppConfig 校验
            # 会 400 指明字段，避免"落成空 {}"破坏校验。
            selected_tts = tts_cfg.get("tts_model", "")
            if selected_tts:
                selected_tts = normalize_tts_id(selected_tts)
                sel_entry = tts_cfg.get(selected_tts)
                if isinstance(sel_entry, dict):
                    if selected_tts == "melo_tts":
                        sel_entry.setdefault(
                            "language", sel_entry.get("voice") or "ZH"
                        )
                        sel_entry.setdefault(
                            "device", sel_entry.get("model") or "cpu"
                        )
                        for field, default in (
                            ("speaker", ""),
                            ("speed", 1.0),
                            ("sdp_ratio", 0.2),
                            ("noise_scale", 0.6),
                            ("noise_scale_w", 0.8),
                            ("format", "wav"),
                        ):
                            sel_entry.setdefault(field, default)
                        sel_entry.pop("voice", None)
                        sel_entry.pop("model", None)
                    else:
                        if not sel_entry.get("voice"):
                            voices = tts_voice_catalog.get(selected_tts, [])
                            if voices:
                                sel_entry["voice"] = voices[0]
                                sel_entry.setdefault("voices", voices)
                        if not sel_entry.get("model"):
                            models = tts_model_catalog.get(selected_tts, [])
                            if models:
                                sel_entry["model"] = models[0]
                                sel_entry.setdefault("models", models)

            # 人设收敛：视觉模型不再由设置页写入（主模型自带视觉能力）

            # System Prompt 是唯一人设字段且不可为空：显式清空直接拒绝
            # （空 persona 会让启动校验 FATAL、对话失去角色根基）
            persona_payload = payload.get("persona")
            if isinstance(persona_payload, dict) and "system_prompt" in persona_payload:
                _sp = persona_payload.get("system_prompt")
                _wants_clear_sp = bool(
                    (persona_payload.get("clear") or {}).get("system_prompt")
                )
                if _wants_clear_sp or _sp is None or (
                    isinstance(_sp, str) and not _sp.strip()
                ):
                    return JSONResponse(
                        {
                            "ok": False,
                            "error": "System Prompt 不能为空——它承载角色核心人设",
                        },
                        status_code=400,
                    )

            # 写盘前用 Config 做一次完整校验，避免非法 Provider/配置落盘。
            # （normalize + 校验都在 _save_yaml/_save_persona 之前完成 → 校验失败
            #  保证三个落盘目标都不会被改写，修复 P0-2 热重载失败后 YAML 已落盘。）
            try:

                persona_for_validation = config_service.load_persona() or {}
                persona_payload = payload.get("persona")
                if persona_payload:
                    for field in (
                        "system_prompt",
                    ):
                        if field in persona_payload:
                            value = persona_payload[field]
                            if value is None:
                                # 显式清空：从该字段中删除，方便 _apply_persona_to_data
                                # 不再把它写进候选配置。
                                persona_for_validation.pop(field, None)
                            elif value != "":
                                persona_for_validation[field] = value
                candidate = copy.deepcopy(data)
                config_helper.apply_persona_to_data(candidate, persona_for_validation)
                AppConfig(**candidate)
            except _PydanticValidationError as validation_error:
                logger.error(f"Config validation failed: {validation_error}")
                field_errors = {}
                for err in validation_error.errors():
                    loc = ".".join(str(x) for x in err.get("loc", []))
                    field_errors[loc] = err.get("msg", str(err))
                return JSONResponse(
                    {
                        "ok": False,
                        "error": "配置校验失败: 请检查标红字段",
                        "field_errors": field_errors,
                    },
                    status_code=400,
                )
            except Exception as validation_error:  # noqa: BLE001
                logger.error(f"Config validation failed: {validation_error}")
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"配置校验失败: {validation_error}",
                        "field_errors": {},
                    },
                    status_code=400,
                )

            config_service.save_yaml(data)
            _log.info("config saved (apply={})", bool(payload.get("apply")))

            # 写入仓库根 .env（Key 不落 conf.yaml，避免误提交）
            # 显式清空语义（spec 1.5）：api_keys 里值为 null 的 key 会被删除；
            # 未提供的 key 保持原样；非空值则写入/更新。
            api_keys = payload.get("api_keys") or {}
            if payload.get("log_level"):
                level_err = validate_log_level(payload["log_level"])
                if level_err:
                    return JSONResponse(
                        {"ok": False, "error": level_err}, status_code=400
                    )
                api_keys["LIVE2DAI_LOG_LEVEL"] = str(payload["log_level"]).strip().upper()
            if api_keys:
                lines = []
                if os.path.exists(env_path):
                    with open(env_path, "r", encoding="utf-8") as f:
                        lines = f.read().splitlines()
                else:
                    os.makedirs(os.path.dirname(env_path), exist_ok=True)
                existing = {}
                for line in lines:
                    if "=" in line and not line.strip().startswith("#"):
                        k, _, v = line.partition("=")
                        existing[k.strip()] = v.strip()
                # 记录原始文件中的 key 集合，用于识别“被显式清空的 key”（其原行应被丢弃）
                origin_keys = set(existing.keys())
                changed = False
                for k, v in api_keys.items():
                    key = k.strip()
                    if v is None:
                        # 显式清空：从 .env 中删除该 key
                        if key in existing:
                            del existing[key]
                            changed = True
                    elif str(v).strip():
                        if existing.get(key) != str(v).strip():
                            existing[key] = str(v).strip()
                            changed = True
                if changed:
                    new_lines = []
                    written = set()
                    for line in lines:
                        if "=" in line and not line.strip().startswith("#"):
                            k, _, _ = line.partition("=")
                            k = k.strip()
                            if k in existing:
                                new_lines.append(f"{k}={existing[k]}")
                                written.add(k)
                                continue
                            elif k in origin_keys:
                                # key 被显式清空（从 existing 删除）：丢弃原行，
                                # 否则会把被删 key 原样写回。
                                continue
                        new_lines.append(line)
                    for k, v in existing.items():
                        if k not in written:
                            new_lines.append(f"{k}={v}")
                    # 原子写入，避免 .env 半写
                    fd, tmp_env = tempfile.mkstemp(
                        prefix=".env.", suffix=".tmp", dir=os.path.dirname(env_path)
                    )
                    try:
                        with os.fdopen(fd, "w", encoding="utf-8") as f:
                            f.write("\n".join(new_lines) + "\n")
                            f.flush()
                            os.fsync(f.fileno())
                        os.replace(tmp_env, env_path)
                    finally:
                        if os.path.exists(tmp_env):
                            os.unlink(tmp_env)

            # 人设/提示词保存（含显式清空：null 删除字段）；字段已收敛为仅 system_prompt
            persona_payload = payload.get("persona")
            if persona_payload:
                persona = config_service.load_persona()
                for field in (
                    "system_prompt",
                ):
                    if field not in persona_payload:
                        continue
                    if persona_payload[field] is None:
                        persona.pop(field, None)
                    elif persona_payload[field] != "":
                        persona[field] = persona_payload[field]
                config_service.save_persona(persona)

            # normalize 后完整视图：保存成功回传，供前端 re-populate（spec 1.3 响应）
            normalized_config = config_helper.get_common(config_service.load_yaml())

            if payload.get("apply") and default_context_cache is not None:
                try:
                    # 让新写入 .env 的 LIVE2DAI_* Key 对热重载可见（B4）
                    try:
                        load_dotenv(env_path, override=True)
                    except Exception as env_error:  # noqa: BLE001
                        logger.warning(f"Failed to reload .env for apply: {env_error}")

                    # 开发者/系统级开关：先立即落到共享 context，保证「保存成功即在当前
                    # 进程生效」——即便下面重量级引擎热重载（init_tts/init_llm 等）失败回退到
                    # requires_restart，dev_routes 门禁读取的 default_context_cache.system_config
                    # 也已是最新值，/api/dev/* 随之开关。失败（400）的保存不会走到这里。
                    sc_data = data.get("system_config") or {}
                    if "developer_mode" in sc_data and getattr(
                        default_context_cache, "system_config", None
                    ) is not None:
                        try:
                            default_context_cache.system_config.developer_mode = bool(
                                sc_data["developer_mode"]
                            )
                        except Exception as dev_patch_err:  # noqa: BLE001
                            logger.warning(
                                f"Failed to patch developer_mode on shared cache: {dev_patch_err}"
                            )

                    new_config = AppConfig(**config_helper.config_dict_for_reload())
                    # 先应用到共享 cache，使随后连接/重连的会话拿到新配置。
                    await default_context_cache.load_from_config(new_config)

                    # 再对已连接 client_contexts 逐会话重跑（B3）。
                    # 若某个会话热更失败/抛错，不伪装 applied，改诚实返回 requires_restart。
                    connected = (
                        list(ws_handler.client_contexts.values())
                        if ws_handler is not None
                        else []
                    )
                    reapplied = []
                    failed = []
                    for ctx in connected:
                        try:
                            await ctx.load_from_config(new_config)
                            reapplied.append(id(ctx))
                        except Exception as ctx_err:  # noqa: BLE001
                            logger.warning(
                                f"Failed to hot-apply to session {id(ctx)}: {ctx_err}"
                            )
                            failed.append(id(ctx))

                    if failed:
                        return JSONResponse(
                            {
                                "ok": True,
                                "config": normalized_config,
                                "applied": False,
                                "requires_restart": True,
                                "message": "配置已保存并在新会话生效；部分已连接会话未能即时热更，重启/重连后生效",
                            }
                        )
                    return JSONResponse(
                        {
                            "ok": True,
                            "config": normalized_config,
                            "applied": True,
                            "requires_restart": False,
                            "message": "已保存并应用",
                        }
                    )
                except Exception as reload_error:  # noqa: BLE001
                    logger.error(f"Config saved but reload failed: {reload_error}")
                    return JSONResponse(
                        {
                            "ok": True,
                            "config": normalized_config,
                            "applied": False,
                            "requires_restart": True,
                            "message": f"已保存，重启/重连后生效（热重载失败: {reload_error}）",
                        }
                    )

            return JSONResponse(
                {
                    "ok": True,
                    "config": normalized_config,
                    "applied": False,
                    "requires_restart": True,
                    "message": "已保存",
                }
            )
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to update config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)
