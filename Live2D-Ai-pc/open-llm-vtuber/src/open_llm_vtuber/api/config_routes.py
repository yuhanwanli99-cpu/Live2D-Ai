"""Settings dashboard and client WebSocket route factory."""

import os
import time

from fastapi import APIRouter, Request
from fastapi.responses import FileResponse
from loguru import logger
from starlette.responses import JSONResponse

from .. import versions as _versions

from .config_update import update_common_config
from .settings_catalogs import (
    _LLM_DEFAULTS,
    _MODEL_CATALOG,
    _TTS_DEFAULTS,
    _TTS_MODEL_CATALOG,
    _TTS_VOICE_CATALOG,
)
from ..service_context import ServiceContext
from ..services.config_endpoint_service import ConfigEndpointService
from ..services.config_routes_helpers import ConfigRoutesHelper
from ..services.config_service import ConfigService
from ..services.llm_routes_helpers import normalize_provider_id
from ..services.llm_service import LLMService
from ..services.local_tts_service import LocalTTSService
from ..services.model_service import ModelService
from ..services.tts_routes_helpers import normalize_tts_id
from ..services.tts_service import TTSService
from ..websocket_handler import WebSocketHandler

# 与 pyproject.toml 的 version 保持一致，供 /health 上报。

_SETTINGS_VERSION = "v1.2.1"

_ALL_TIMEOUT = 5.0



# tts_config 里不是 provider 的键（全局 TTS 选项）
_TTS_NON_PROVIDER_KEYS = ("tts_model", "concurrency_limit", "viseme_mode")

# 「全部检查可用性」的并发上限（审计 E-1：原先 ~16 个 provider 串行探测，
# 每个 timeout=8 → 最坏近 128s 阻塞事件循环，全站 WS 一起卡死）
_CHECK_CONCURRENCY = 4

# 本地 Melo TTS REST 接口约束（PLAN-V2-PC-LOCAL-TTS）
# 限制单次合成长度，避免 Melo 把长文本拆成过多小句。
_LOCAL_TTS_MAX_TEXT_LEN = int(os.environ.get("MELO_MAX_TEXT_LEN", "200"))
_LOCAL_TTS_FILE_PREFIX = "tts_local_"
_LOCAL_TTS_PREVIEW_TEXT = "你好，我是白～这是一段本地试听。"





def init_config_routes(
    config_path: str = "conf.yaml",
    default_context_cache: ServiceContext = None,
    ws_handler: WebSocketHandler = None,
) -> APIRouter:
    """轻量设置 Dashboard API：读取/写入 conf.yaml、persona.yaml 与 .env。

    支持常用设置、人设/系统提示词、原始 YAML 设定树、LLM 测试与 TTS 试听。

    Args:
        config_path: conf.yaml 路径。
        default_context_cache: 共享 ServiceContext（用于 apply 热重载）。
        ws_handler: 可选的 WebSocketHandler；非 None 时 apply=true 会遍历其
            client_contexts，对已连接会话重跑 load（修复 B3 已连会话热更）。
    """
    router = APIRouter()
    config_abs = os.path.abspath(config_path)
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(config_abs)))
    env_path = os.path.join(repo_root, ".env")
    persona_path = os.path.join(repo_root, "shared", "persona.yaml")
    config_service = ConfigService(config_abs, persona_path)
    _start_time = time.time()

    config_helper = ConfigRoutesHelper(
        config_abs,
        env_path,
        config_service,
        _LLM_DEFAULTS,
        _MODEL_CATALOG,
        _TTS_DEFAULTS,
        _TTS_VOICE_CATALOG,
        _TTS_MODEL_CATALOG,
        _TTS_NON_PROVIDER_KEYS,
    )

    llm_service = LLMService(
        config_service,
        config_helper,
        _LLM_DEFAULTS,
        _MODEL_CATALOG,
        _CHECK_CONCURRENCY,
    )

    model_service = ModelService(repo_root, config_service, ws_handler)

    tts_service = TTSService(
        config_service,
        config_helper,
        _TTS_DEFAULTS,
        _TTS_VOICE_CATALOG,
        _TTS_MODEL_CATALOG,
        _TTS_NON_PROVIDER_KEYS,
    )

    local_tts_service = LocalTTSService(
        os.path.join(os.path.dirname(config_abs), "cache"),
        _LOCAL_TTS_MAX_TEXT_LEN,
        _LOCAL_TTS_FILE_PREFIX,
    )

    config_endpoint_service = ConfigEndpointService(
        config_service,
        config_helper,
        env_path,
    )






















    @router.get("/health")
    async def health():
        # WSL2 迁移验收项：GET /health -> 200 {"status":"ok","version":..,"uptime_s":..}
        # version 字段为单一产品版本；另附 backend/frontend/commit 便于可观测性。
        info = _versions.common_version_info()
        return JSONResponse(
            {
                "status": "ok",
                "version": _versions.PRODUCT_VERSION,
                "product_version": _versions.PRODUCT_VERSION,
                "backend_version": info["backend_version"],
                "frontend_version": info["frontend_version"],
                "commit": info["commit"],
                "uptime_s": int(time.time() - _start_time),
            }
        )

    @router.get("/api/config")
    async def get_config():
        try:
            data = config_service.load_yaml()
            return JSONResponse(config_helper.get_common(data))
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to read config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/config")
    async def update_config(request: Request):
        return await update_common_config(
            request,
            config_service=config_service,
            config_helper=config_helper,
            env_path=env_path,
            llm_defaults=_LLM_DEFAULTS,
            model_catalog=_MODEL_CATALOG,
            tts_voice_catalog=_TTS_VOICE_CATALOG,
            tts_model_catalog=_TTS_MODEL_CATALOG,
            default_context_cache=default_context_cache,
            ws_handler=ws_handler,
        )

    @router.get("/api/persona")
    async def get_persona():
        try:
            persona = config_service.load_persona()
            return JSONResponse(
                {
                    "profile": persona.get("profile", {}),
                    "system_prompt": persona.get("system_prompt", ""),
                }
            )
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to read persona: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/persona")
    async def update_persona(request: Request):
        try:
            payload = await request.json()
            # System Prompt 唯一且不可为空（同 /api/config 守卫语义）
            if "system_prompt" in payload:
                _sp = payload.get("system_prompt")
                if _sp is None or (isinstance(_sp, str) and not _sp.strip()):
                    return JSONResponse(
                        {
                            "ok": False,
                            "error": "System Prompt 不能为空——它承载角色核心人设",
                        },
                        status_code=400,
                    )
            persona = config_service.load_persona()
            for field in (
                "system_prompt",
            ):
                if field not in payload:
                    continue
                if payload[field] is None:
                    # 显式清空：删除字段
                    persona.pop(field, None)
                elif payload[field] != "":
                    persona[field] = payload[field]
            if "profile" in payload and isinstance(payload["profile"], dict):
                persona["profile"] = payload["profile"]
            config_service.save_persona(persona)
            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to update persona: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.get("/api/config/tree")
    async def get_config_tree():
        try:
            with open(config_abs, "r", encoding="utf-8") as f:
                raw = f.read()
            return JSONResponse({"yaml": raw})
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to read config tree: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/config/tree")
    async def update_config_tree(request: Request):
        try:
            payload = await request.json()
            result, status_code = config_endpoint_service.update_config_tree(payload.get("yaml", ""))
            return JSONResponse(result, status_code=status_code)
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to update config tree: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.get("/api/models")
    async def get_models():
        try:
            return JSONResponse(model_service.get_models())
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to read model registry: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/models/scan")
    async def scan_models():
        try:
            return JSONResponse(model_service.scan_models())
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to scan models: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/models/select")
    async def select_model(request: Request):
        try:
            payload = await request.json()
            return JSONResponse(await model_service.select_model(payload.get("model_id", "")))
        except ValueError as e:
            return JSONResponse({"error": str(e)}, status_code=400)
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to select model: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/llm/discover-models")
    async def discover_llm_models(request: Request):
        try:
            return JSONResponse(await llm_service.discover_models(await request.json()))
        except Exception as e:  # noqa: BLE001
            logger.error(f"Discover models failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/providers/check")
    async def check_providers(request: Request):
        try:
            payload = {}
            try:
                payload = await request.json()
            except Exception:  # noqa: BLE001
                pass
            data = config_service.load_yaml()
            cc = data.get("character_config", {}) or {}
            agent_cfg = cc.get("agent_config", {}) or {}
            llm_configs = agent_cfg.get("llm_configs", {}) or {}
            tts_cfg = cc.get("tts_config", {}) or {}
            llm_overrides = payload.get("llm_configs") or {}
            tts_overrides = payload.get("tts_configs") or {}
            api_keys = payload.get("api_keys") or {}
            llm_results = await llm_service.check_llm_providers(payload)
            tts_results = tts_service.check_tts_providers(payload)
            return JSONResponse({"llm": llm_results, "tts": tts_results})
        except Exception as e:  # noqa: BLE001
            logger.error(f"Provider check failed: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/test-llm")
    async def test_llm(request: Request):
        try:
            return JSONResponse(await llm_service.test_llm(await request.json()))
        except Exception as e:  # noqa: BLE001
            logger.error(f"LLM test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/test-tts")
    async def test_tts(request: Request):
        try:
            payload = {}
            try:
                payload = await request.json()
            except Exception:  # noqa: BLE001
                payload = {}
            return JSONResponse(await tts_service.test_tts(payload, default_context_cache))
        except Exception as e:  # noqa: BLE001
            logger.error(f"TTS test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/tts/local/synthesize")
    async def local_tts_synthesize(request: Request):
        try:
            return JSONResponse(await local_tts_service.synthesize(await request.json()))
        except ValueError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=400)
        except RuntimeError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=400)
        except Exception as exc:  # noqa: BLE001
            logger.exception(f"Local Melo TTS synthesize failed: {exc}")
            return JSONResponse(
                {"ok": False, "error": f"本地 Melo TTS 合成失败: {exc}"},
                status_code=500,
            )

    @router.get("/api/tts/local/audio")
    async def local_tts_audio(name: str):
        try:
            return FileResponse(local_tts_service.audio_path(name), media_type="audio/wav")
        except FileNotFoundError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=404)
        except ValueError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=403)

    return router
