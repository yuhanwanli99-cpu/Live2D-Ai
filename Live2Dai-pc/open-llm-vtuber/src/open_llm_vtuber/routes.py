import os
import json
from uuid import uuid4
import numpy as np
from datetime import datetime
from fastapi import APIRouter, WebSocket, UploadFile, File, Response, Request
from starlette.responses import JSONResponse
from starlette.websockets import WebSocketDisconnect
from loguru import logger
from ruamel.yaml import YAML
from .service_context import ServiceContext
from .websocket_handler import WebSocketHandler
from .proxy_handler import ProxyHandler


def init_config_routes(
    config_path: str = "conf.yaml",
    default_context_cache: ServiceContext = None,
) -> APIRouter:
    """轻量设置 Dashboard API：读取/写入 conf.yaml、persona.yaml 与 .env。

    支持常用设置、人设/系统提示词、原始 YAML 设定树、LLM 测试与 TTS 试听。
    """
    router = APIRouter()
    config_abs = os.path.abspath(config_path)
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(config_abs)))
    env_path = os.path.join(repo_root, ".env")
    persona_path = os.path.join(repo_root, "shared", "persona.yaml")

    # Provider -> 默认 base_url / 环境变量名
    LLM_DEFAULTS = {
        "deepseek_llm": {
            "name": "DeepSeek",
            "base_url": "https://api.deepseek.com/v1",
            "key_env": "DEEPSEEK_API_KEY",
        },
        "zhipu_llm": {
            "name": "智谱 GLM",
            "base_url": "https://open.bigmodel.cn/api/paas/v4",
            "key_env": "ZHIPU_API_KEY",
        },
        "zhipu_free_llm": {
            "name": "免费 GLM",
            "base_url": "https://open.bigmodel.cn/api/paas/v4",
            "key_env": "ZHIPU_API_KEY",
        },
        "qwen_llm": {
            "name": "Qwen",
            "base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "key_env": "DASHSCOPE_API_KEY",
        },
        "openrouter": {
            "name": "OpenRouter",
            "base_url": "https://openrouter.ai/api/v1",
            "key_env": "OPENROUTER_API_KEY",
        },
        "openai": {
            "name": "OpenAI",
            "base_url": "https://api.openai.com/v1",
            "key_env": "OPENAI_API_KEY",
        },
        "anthropic": {
            "name": "Anthropic",
            "base_url": "https://api.anthropic.com/v1",
            "key_env": "ANTHROPIC_API_KEY",
        },
        "moonshot": {
            "name": "Moonshot",
            "base_url": "https://api.moonshot.cn/v1",
            "key_env": "MOONSHOT_API_KEY",
        },
        "groq": {
            "name": "Groq",
            "base_url": "https://api.groq.com/openai/v1",
            "key_env": "GROQ_API_KEY",
        },
        "mistral": {
            "name": "Mistral",
            "base_url": "https://api.mistral.ai/v1",
            "key_env": "MISTRAL_API_KEY",
        },
        "together": {
            "name": "Together AI",
            "base_url": "https://api.together.xyz/v1",
            "key_env": "TOGETHER_API_KEY",
        },
        "fireworks": {
            "name": "Fireworks AI",
            "base_url": "https://api.fireworks.ai/inference/v1",
            "key_env": "FIREWORKS_API_KEY",
        },
        "siliconflow": {
            "name": "SiliconFlow",
            "base_url": "https://api.siliconflow.cn/v1",
            "key_env": "SILICONFLOW_API_KEY",
        },
        "opencode": {
            "name": "OpenCode",
            "base_url": "",
            "key_env": "OPENCODE_API_KEY",
        },
        "opencode_go": {
            "name": "OpenCode Go",
            "base_url": "",
            "key_env": "OPENCODE_GO_API_KEY",
        },
        "openai_compatible": {
            "name": "OpenAI 兼容",
            "base_url": "",
            "key_env": "OPENAI_API_KEY",
        },
    }
    MODEL_CATALOG = {
        "deepseek_llm": ["deepseek-v4-flash", "deepseek-v4-pro"],
        "zhipu_llm": ["glm-4.7-flash", "glm-4.6v"],
        "zhipu_free_llm": ["glm-4.7-flash"],
        "qwen_llm": ["qwen3.7-flash", "qwen3.7-plus", "qwen-max"],
        "openrouter": [
            "openrouter/auto",
            "deepseek/deepseek-chat",
            "qwen/qwen3-7b",
        ],
        "openai": ["gpt-5", "gpt-4o", "gpt-4o-mini"],
        "anthropic": [
            "claude-sonnet-4-5",
            "claude-opus-4-1",
            "claude-haiku-4-5",
        ],
        "moonshot": ["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"],
        "groq": [
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
        ],
        "mistral": ["mistral-large-latest", "mistral-small-latest"],
        "together": [
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            "mistralai/Mixtral-8x7B-Instruct-v0.1",
        ],
        "fireworks": [
            "accounts/fireworks/models/llama-v3p3-70b-instruct",
            "accounts/fireworks/models/qwen2p5-coder-32b-instruct",
        ],
        "siliconflow": [
            "deepseek-ai/DeepSeek-V3",
            "Qwen/Qwen2.5-7B-Instruct",
        ],
        "opencode": [],
        "opencode_go": [],
        "openai_compatible": [],
    }
    TTS_VOICE_CATALOG = {
        "edge_tts": [
            "zh-CN-XiaoxiaoNeural", "zh-CN-XiaoyiNeural", "zh-CN-YunjianNeural",
            "zh-CN-YunxiNeural", "zh-CN-YunxiaNeural", "zh-CN-liaoning-XiaobeiNeural",
            "zh-CN-shaanxi-XiaoniNeural", "en-US-JennyNeural", "en-US-GuyNeural",
            "ja-JP-NanamiNeural", "ko-KR-SunHiNeural",
        ],
        "cosyvoice_cloud": [
            "longxiaochun", "longxiaoxia", "longshu", "longcheng",
            "longhua", "longyu", "longmei", "longyi",
        ],
        "qwen_tts": ["Cherry", "Ethan", "Serena", "Charlotte"],
        "openai_tts": ["alloy", "echo", "fable", "onyx", "nova", "shimmer"],
        "azure_tts": [
            "zh-CN-XiaoxiaoNeural", "zh-CN-XiaoyiNeural", "zh-CN-YunxiNeural",
            "zh-CN-YunjianNeural", "en-US-JennyNeural", "ja-JP-NanamiNeural",
        ],
        "minimax_tts": ["male-qn-qingse", "female-shaonv", "female-tianmei", "male-jingdian"],
        "siliconflow_tts": ["FunAudioLLM/CosyVoice2-0.5B", "iic/SpeechT5", "Qwen/Qwen2-Audio"],
        "fish_api_tts": ["fish-speech-1.5"],
        "spark_tts": ["xiaoyan", "aisjouxu", "aisjiushi", "aisjinger"],
        "gpt_sovits": [],
        "piper": [],
        "sherpa_onnx": [],
        "pyttsx3": [],
    }
    TTS_MODEL_CATALOG = {
        "gpt_sovits": ["GPT-SoVITS"],
        "piper": ["piper"],
        "sherpa_onnx": ["sherpa-onnx"],
        "pyttsx3": ["pyttsx3"],
    }

    TTS_DEFAULTS = {
        "cosyvoice_cloud": {"name": "CosyVoice", "key_env": "ALIYUN_API_KEY"},
        "qwen_tts": {"name": "Qwen TTS", "key_env": "DASHSCOPE_API_KEY"},
        "edge_tts": {"name": "Edge TTS", "key_env": None},
        "openai_tts": {"name": "OpenAI TTS", "key_env": "OPENAI_API_KEY"},
        "azure_tts": {"name": "Azure TTS", "key_env": "AZURE_SPEECH_KEY"},
        "minimax_tts": {"name": "MiniMax", "key_env": "MINIMAX_API_KEY"},
        "siliconflow_tts": {"name": "SiliconFlow TTS", "key_env": "SILICONFLOW_API_KEY"},
        "fish_api_tts": {"name": "Fish Audio", "key_env": "FISH_API_KEY"},
        "spark_tts": {"name": "Spark TTS", "key_env": "SPARK_API_KEY"},
        "gpt_sovits": {"name": "GPT-SoVITS", "key_env": None},
        "piper": {"name": "Piper", "key_env": None},
        "sherpa_onnx": {"name": "Sherpa-ONNX", "key_env": None},
        "pyttsx3": {"name": "pyttsx3", "key_env": None},
    }

    def _load_yaml():
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        with open(config_abs, "r", encoding="utf-8") as f:
            return yaml.load(f) or {}

    def _save_yaml(data):
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        with open(config_abs, "w", encoding="utf-8") as f:
            yaml.dump(data, f)

    def _load_persona():
        if not os.path.exists(persona_path):
            return {}
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        with open(persona_path, "r", encoding="utf-8") as f:
            return yaml.load(f) or {}

    def _parse_models_response(raw) -> list:
        """兼容各家 /models 返回结构：data 可能是 list、dict 或嵌套 models。"""
        models: list[str] = []

        def collect(items) -> None:
            if not isinstance(items, list):
                return
            for item in items:
                if isinstance(item, dict) and item.get("id"):
                    models.append(str(item["id"]))
                elif isinstance(item, str):
                    models.append(item)

        if isinstance(raw, dict):
            data = raw.get("data")
            if isinstance(data, dict):
                # 例如 { "data": { "models": [...] } }
                collect(data.get("models") or data.get("data") or [])
            elif isinstance(data, list):
                collect(data)
            collect(raw.get("models") or [])
            # 极少数结构：{ "models": { "data": [...] } }
            models_obj = raw.get("models")
            if isinstance(models_obj, dict):
                collect(models_obj.get("data") or models_obj.get("models") or [])
        elif isinstance(raw, list):
            collect(raw)
        return models

    def _save_persona(data):
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        with open(persona_path, "w", encoding="utf-8") as f:
            yaml.dump(data, f)

    def _config_dict_for_reload():
        """构造用于热重载的完整 Config dict。

        与 run_server.main() 的启动逻辑保持一致：
          - conf.yaml 本身不含 persona_prompt（它来自 shared/persona.yaml）
          - 必须注入 persona_prompt，否则 Pydantic 热重载会报 Field required
          - 应用 persona.yaml 中的 model/temperature/max_tokens 覆盖（当前仅 deepseek_llm）
        """
        from .config_manager.utils import read_yaml

        data = read_yaml(config_abs)
        cc = data.setdefault("character_config", {})
        persona = _load_persona() or {}
        system_prompt = persona.get("system_prompt", "")
        if system_prompt:
            cc["persona_prompt"] = system_prompt

        model_overrides = {
            k: persona[k]
            for k in ("model", "temperature", "max_tokens")
            if persona.get(k) is not None
        }
        if model_overrides:
            llm_configs = (
                cc.setdefault("agent_config", {})
                .setdefault("llm_configs", {})
            )
            deepseek_cfg = llm_configs.setdefault("deepseek_llm", {})
            if "model" in model_overrides:
                deepseek_cfg["model"] = model_overrides["model"]
            if "temperature" in model_overrides:
                deepseek_cfg["temperature"] = model_overrides["temperature"]
            if "max_tokens" in model_overrides:
                deepseek_cfg["max_tokens"] = model_overrides["max_tokens"]
        return data

    def _env_value(name):
        if not name:
            return ""
        if os.path.exists(env_path):
            with open(env_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("#") or "=" not in line:
                        continue
                    k, _, v = line.partition("=")
                    if k.strip() == name:
                        return v.strip()
        return os.getenv(name, "")

    def _has_key(name):
        return bool(_env_value(name))

    def _get_common(data: dict) -> dict:
        cc = data.get("character_config", {}) or {}
        agent_cfg = cc.get("agent_config", {}) or {}
        agent = agent_cfg.get("agent_settings", {}) or {}
        basic = agent.get("basic_memory_agent", {}) or {}
        llm_configs = agent_cfg.get("llm_configs", {}) or {}
        asr_cfg = cc.get("asr_config", {}) or {}
        tts_cfg = cc.get("tts_config", {}) or {}
        sys_cfg = data.get("system_config", {}) or {}
        persona = _load_persona()

        provider_ids = list(dict.fromkeys(list(llm_configs.keys()) + list(LLM_DEFAULTS.keys())))
        llm_provider_list = []
        for pid in provider_ids:
            cfg = llm_configs.get(pid, {}) or {}
            default = LLM_DEFAULTS.get(pid, {})
            key_env = default.get("key_env")
            known_models = cfg.get("models") or MODEL_CATALOG.get(pid, [])
            current_model = cfg.get("model", "")
            if not current_model and known_models:
                current_model = known_models[0]
            llm_provider_list.append(
                {
                    "id": pid,
                    "name": default.get("name", pid),
                    "model": current_model,
                    "models": known_models,
                    "base_url": cfg.get("base_url", default.get("base_url", "")),
                    "temperature": cfg.get("temperature", 0.8),
                    "max_tokens": cfg.get("max_tokens", None),
                    "key_env": key_env,
                    "has_key": _has_key(key_env) if key_env else False,
                    "configured": pid in llm_configs,
                }
            )

        tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(TTS_DEFAULTS.keys())))
        tts_provider_list = []
        for pid in tts_ids:
            if pid in ("tts_model", "concurrency_limit", "viseme_mode"):
                continue
            cfg = tts_cfg.get(pid, {}) or {}
            if not isinstance(cfg, dict):
                cfg = {}
            default = TTS_DEFAULTS.get(pid, {})
            key_env = default.get("key_env")
            known_voices = cfg.get("voices") or TTS_VOICE_CATALOG.get(pid, [])
            known_tts_models = cfg.get("models") or TTS_MODEL_CATALOG.get(pid, [])
            current_voice = cfg.get("voice", "")
            if not current_voice and known_voices:
                current_voice = known_voices[0]
            current_tts_model = cfg.get("model", "")
            if not current_tts_model and known_tts_models:
                current_tts_model = known_tts_models[0]
            tts_provider_list.append(
                {
                    "id": pid,
                    "name": default.get("name", pid),
                    "voice": current_voice,
                    "model": current_tts_model,
                    "ws_url": cfg.get("ws_url", ""),
                    "voices": known_voices,
                    "models": known_tts_models,
                    "key_env": key_env,
                    "has_key": _has_key(key_env) if key_env else True,
                    "configured": pid in tts_cfg,
                }
            )

        return {
            "llm_provider": basic.get("llm_provider", ""),
            "llm_providers": list(dict.fromkeys(list(llm_configs.keys()) + list(LLM_DEFAULTS.keys()))),
            "llm_configs": llm_provider_list,
            "tts_model": tts_cfg.get("tts_model", ""),
            "viseme_mode": tts_cfg.get("viseme_mode", "rms"),
            "tts_providers": list(
                dict.fromkeys(
                    [
                        k
                        for k, v in tts_cfg.items()
                        if k not in ("tts_model", "concurrency_limit", "viseme_mode")
                        and isinstance(v, dict)
                    ]
                    + list(TTS_DEFAULTS.keys())
                )
            ),
            "tts_configs": tts_provider_list,
            "asr_model": asr_cfg.get("asr_model", "disabled"),
            "asr_providers": [
                k
                for k, v in asr_cfg.items()
                if k != "asr_model" and isinstance(v, dict)
            ],
            "character_name": cc.get("character_name", ""),
            "port": sys_cfg.get("port", 12393),
            "live2d_fps": sys_cfg.get("live2d_fps", 120),
            "persona": {
                "name": persona.get("name", ""),
                "role": persona.get("role", ""),
                "model": persona.get("model", ""),
                "temperature": persona.get("temperature", 0.8),
                "max_tokens": persona.get("max_tokens", None),
                "vision_model": persona.get("vision_model", ""),
                "system_prompt": persona.get("system_prompt", ""),
            },
        }

    @router.get("/api/config")
    async def get_config():
        try:
            data = _load_yaml()
            return JSONResponse(_get_common(data))
        except Exception as e:
            logger.error(f"Failed to read config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/config")
    async def update_config(request: Request):
        try:
            payload = await request.json()
            data = _load_yaml()

            cc = data.setdefault("character_config", {})
            agent_cfg = cc.setdefault("agent_config", {})
            settings = agent_cfg.setdefault("agent_settings", {})
            basic = settings.setdefault("basic_memory_agent", {})

            if "llm_provider" in payload and payload["llm_provider"]:
                basic["llm_provider"] = payload["llm_provider"]
            if "tts_model" in payload and payload["tts_model"]:
                cc.setdefault("tts_config", {})["tts_model"] = payload["tts_model"]
            if "viseme_mode" in payload and payload["viseme_mode"]:
                cc.setdefault("tts_config", {})["viseme_mode"] = payload["viseme_mode"]
            if "asr_model" in payload and payload["asr_model"]:
                cc.setdefault("asr_config", {})["asr_model"] = payload["asr_model"]
            if "character_name" in payload and payload["character_name"]:
                cc["character_name"] = payload["character_name"]
            if "port" in payload and payload["port"]:
                data.setdefault("system_config", {})["port"] = int(payload["port"])
            if "live2d_fps" in payload and payload["live2d_fps"]:
                data.setdefault("system_config", {})["live2d_fps"] = int(
                    payload["live2d_fps"]
                )

            # 更新 LLM Provider 参数（model/base_url/temperature/max_tokens）
            llm_overrides = payload.get("llm_configs") or {}
            existing_llm = agent_cfg.setdefault("llm_configs", {})
            for pid, override in llm_overrides.items():
                if not isinstance(override, dict):
                    continue
                if pid not in existing_llm:
                    if not override.get("model"):
                        continue
                    target = existing_llm[pid] = {}
                    new_key_env = LLM_DEFAULTS.get(pid, {}).get("key_env") or "OPENAI_API_KEY"
                    target["llm_api_key"] = "${" + new_key_env + "}"
                else:
                    target = existing_llm[pid]
                for field in ("model", "base_url", "temperature", "max_tokens", "models"):
                    if field in override and override[field] not in (None, ""):
                        target[field] = override[field]

            # 更新 TTS Provider 参数（voice/model/ws_url）
            tts_overrides = payload.get("tts_configs") or {}
            tts_cfg = cc.setdefault("tts_config", {})
            for pid, override in tts_overrides.items():
                if not isinstance(override, dict):
                    continue
                has_any = any(
                    override.get(field) not in (None, "")
                    for field in ("voice", "model", "ws_url", "voices", "models")
                )
                if pid not in tts_cfg and not has_any:
                    continue
                target = tts_cfg.setdefault(pid, {})
                for field in ("voice", "model", "ws_url", "voices", "models"):
                    if field in override and override[field] not in (None, ""):
                        target[field] = override[field]

            if "tts_model" in payload and payload["tts_model"]:
                requested_tts = payload["tts_model"]
                if requested_tts not in tts_cfg:
                    return JSONResponse(
                        {
                            "error": f"TTS 提供商 {requested_tts} 尚未配置，请先在其卡片中填写参数",
                        },
                        status_code=400,
                    )

            _save_yaml(data)

            # 写入仓库根 .env（Key 不落 conf.yaml，避免误提交）
            api_keys = payload.get("api_keys") or {}
            if payload.get("log_level"):
                api_keys["LIVE2DAI_LOG_LEVEL"] = str(payload["log_level"])
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
                changed = False
                for k, v in api_keys.items():
                    if v and str(v).strip():
                        existing[k.strip()] = str(v).strip()
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
                        new_lines.append(line)
                    for k, v in existing.items():
                        if k not in written:
                            new_lines.append(f"{k}={v}")
                    with open(env_path, "w", encoding="utf-8") as f:
                        f.write("\n".join(new_lines) + "\n")

            # 人设/提示词保存
            persona_payload = payload.get("persona")
            if persona_payload:
                persona = _load_persona()
                for field in (
                    "name",
                    "role",
                    "model",
                    "temperature",
                    "max_tokens",
                    "vision_model",
                    "system_prompt",
                ):
                    if field in persona_payload and persona_payload[field] not in (
                        None,
                        "",
                    ):
                        persona[field] = persona_payload[field]
                _save_persona(persona)

            if payload.get("apply") and default_context_cache is not None:
                try:
                    from .config_manager.main import Config as AppConfig
                    new_config = AppConfig(**_config_dict_for_reload())
                    await default_context_cache.load_from_config(new_config)
                    return JSONResponse(
                        {"ok": True, "requires_restart": False, "applied": True}
                    )
                except Exception as reload_error:
                    logger.error(f"Config saved but reload failed: {reload_error}")
                    return JSONResponse(
                        {
                            "ok": False,
                            "error": f"配置已保存，但热重载失败: {reload_error}",
                        },
                        status_code=500,
                    )

            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:
            logger.error(f"Failed to update config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.get("/api/persona")
    async def get_persona():
        try:
            persona = _load_persona()
            return JSONResponse(
                {
                    "name": persona.get("name", ""),
                    "role": persona.get("role", ""),
                    "profile": persona.get("profile", {}),
                    "system_prompt": persona.get("system_prompt", ""),
                    "model": persona.get("model", ""),
                    "temperature": persona.get("temperature", 0.8),
                    "max_tokens": persona.get("max_tokens", None),
                    "vision_model": persona.get("vision_model", ""),
                }
            )
        except Exception as e:
            logger.error(f"Failed to read persona: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/persona")
    async def update_persona(request: Request):
        try:
            payload = await request.json()
            persona = _load_persona()
            for field in (
                "name",
                "role",
                "model",
                "temperature",
                "max_tokens",
                "vision_model",
                "system_prompt",
            ):
                if field in payload and payload[field] not in (None, ""):
                    persona[field] = payload[field]
            if "profile" in payload and isinstance(payload["profile"], dict):
                persona["profile"] = payload["profile"]
            _save_persona(persona)
            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:
            logger.error(f"Failed to update persona: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.get("/api/config/tree")
    async def get_config_tree():
        try:
            with open(config_abs, "r", encoding="utf-8") as f:
                raw = f.read()
            return JSONResponse({"yaml": raw})
        except Exception as e:
            logger.error(f"Failed to read config tree: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/config/tree")
    async def update_config_tree(request: Request):
        try:
            payload = await request.json()
            raw = payload.get("yaml", "")
            import yaml as _yaml
            _yaml.safe_load(raw)  # 语法校验，失败会抛异常
            with open(config_abs, "w", encoding="utf-8") as f:
                f.write(raw)
            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:
            logger.error(f"Failed to update config tree: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.get("/api/models")
    async def get_models():
        try:
            reg_path = os.path.join(repo_root, "shared", "model_registry.json")
            if os.path.exists(reg_path):
                with open(reg_path, "r", encoding="utf-8") as f:
                    reg = json.load(f)
                return JSONResponse({"models": reg.get("models", [])})
            return JSONResponse({"models": []})
        except Exception as e:
            logger.error(f"Failed to read model registry: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/models/scan")
    async def scan_models():
        try:
            live2d_dir = os.path.join(os.getcwd(), "live2d-models")
            reg_path = os.path.join(repo_root, "shared", "model_registry.json")
            registry = {"version": "1.0", "models": []}
            if os.path.exists(reg_path):
                with open(reg_path, "r", encoding="utf-8") as f:
                    registry = json.load(f)
            existing = {m.get("id") for m in registry.get("models", [])}
            added = []
            if os.path.isdir(live2d_dir):
                for entry in os.scandir(live2d_dir):
                    if not entry.is_dir():
                        continue
                    folder = entry.name
                    model3 = os.path.join(live2d_dir, folder, f"{folder}.model3.json")
                    if not os.path.isfile(model3):
                        continue
                    if folder in existing:
                        continue
                    registry.setdefault("models", []).append(
                        {
                            "id": folder,
                            "name": folder,
                            "description": "本地扫描导入的 Live2D 模型",
                            "model3JsonPath": f"{folder}/{folder}.model3.json",
                            "thumbnailPath": None,
                            "available": True,
                            "default": False,
                            "restrictPlatform": None,
                            "metadata": {"creator": "local", "license": "user", "version": "1.0"},
                        }
                    )
                    existing.add(folder)
                    added.append(folder)
            if added:
                with open(reg_path, "w", encoding="utf-8") as f:
                    json.dump(registry, f, ensure_ascii=False, indent=2)
            return JSONResponse({"ok": True, "added": added, "models": registry.get("models", [])})
        except Exception as e:
            logger.error(f"Failed to scan models: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/models/select")
    async def select_model(request: Request):
        try:
            payload = await request.json()
            model_id = payload.get("model_id", "")
            if not model_id:
                return JSONResponse({"error": "model_id 不能为空"}, status_code=400)
            data = _load_yaml()
            cc = data.setdefault("character_config", {})
            cc["live2d_model_name"] = model_id
            cc["conf_name"] = model_id
            _save_yaml(data)
            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:
            logger.error(f"Failed to select model: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/llm/discover-models")
    async def discover_llm_models(request: Request):
        try:
            import requests as _requests
            payload = await request.json()
            provider = payload.get("provider", "")
            base_url = (payload.get("base_url") or "").strip()
            api_key = (payload.get("api_key") or "").strip()
            data = _load_yaml()
            agent_cfg = (
                (data.get("character_config", {}) or {})
                .get("agent_config", {})
                or {}
            )
            cfg = (agent_cfg.get("llm_configs", {}) or {}).get(provider, {}) or {}
            default = LLM_DEFAULTS.get(provider, {})
            if not base_url:
                base_url = (
                    cfg.get("base_url", "").strip()
                    or default.get("base_url", "")
                )
            if not api_key:
                key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
                api_key = _env_value(key_env)
            if not base_url:
                return JSONResponse(
                    {"ok": False, "error": "Base URL 为空，无法发现模型"}, status_code=400
                )
            url = base_url.rstrip("/") + "/models"
            headers = {}
            if api_key:
                headers["Authorization"] = f"Bearer {api_key}"
            resp = _requests.get(url, headers=headers, timeout=15)
            if resp.status_code >= 400:
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"HTTP {resp.status_code}: {resp.text[:200]}",
                    },
                    status_code=400,
                )
            raw = resp.json()
            models = _parse_models_response(raw)
            return JSONResponse({"ok": True, "models": models})
        except Exception as e:
            logger.error(f"Discover models failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/providers/check")
    async def check_providers():
        try:
            import requests as _requests
            data = _load_yaml()
            cc = data.get("character_config", {}) or {}
            agent_cfg = cc.get("agent_config", {}) or {}
            llm_configs = agent_cfg.get("llm_configs", {}) or {}
            tts_cfg = cc.get("tts_config", {}) or {}
            llm_results = []
            provider_ids = list(
                dict.fromkeys(list(llm_configs.keys()) + list(LLM_DEFAULTS.keys()))
            )
            for pid in provider_ids:
                cfg = llm_configs.get(pid, {}) or {}
                default = LLM_DEFAULTS.get(pid, {})
                base_url = (
                    cfg.get("base_url", "").strip()
                    or default.get("base_url", "")
                )
                key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
                key = _env_value(key_env)
                if not key:
                    llm_results.append(
                        {"provider": pid, "kind": "llm", "ok": False, "status": "未配置 Key", "models": []}
                    )
                    continue
                if not base_url:
                    llm_results.append(
                        {"provider": pid, "kind": "llm", "ok": False, "status": "未配置 Base URL", "models": []}
                    )
                    continue
                try:
                    url = base_url.rstrip("/") + "/models"
                    resp = _requests.get(
                        url,
                        headers={"Authorization": f"Bearer {key}"},
                        timeout=8,
                    )
                    if resp.status_code < 400:
                        raw = resp.json()
                        models = _parse_models_response(raw)
                        llm_results.append(
                            {"provider": pid, "kind": "llm", "ok": True, "status": "ok", "models": models}
                        )
                    else:
                        llm_results.append(
                            {
                                "provider": pid,
                                "kind": "llm",
                                "ok": False,
                                "status": f"HTTP {resp.status_code}",
                                "models": [],
                            }
                        )
                except Exception as e:
                    llm_results.append(
                        {"provider": pid, "kind": "llm", "ok": False, "status": str(e), "models": []}
                    )

            tts_results = []
            tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(TTS_DEFAULTS.keys())))
            for pid in tts_ids:
                if pid in ("tts_model", "concurrency_limit", "viseme_mode"):
                    continue
                key_env = TTS_DEFAULTS.get(pid, {}).get("key_env")
                configured = _has_key(key_env) if key_env else True
                tts_results.append(
                    {
                        "provider": pid,
                        "kind": "tts",
                        "ok": configured,
                        "status": "已配置" if configured else "未配置 Key",
                        "models": [],
                    }
                )
            return JSONResponse({"llm": llm_results, "tts": tts_results})
        except Exception as e:
            logger.error(f"Provider check failed: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/test-llm")
    async def test_llm(request: Request):
        try:
            import requests as _requests
            payload = await request.json()
            provider = payload.get("provider", "")
            key = payload.get("api_key", "").strip()
            data = _load_yaml()
            agent_cfg = (
                (data.get("character_config", {}) or {})
                .get("agent_config", {})
                or {}
            )
            cfg = (agent_cfg.get("llm_configs", {}) or {}).get(provider, {}) or {}
            default = LLM_DEFAULTS.get(provider, {})
            base_url = (
                cfg.get("base_url", "").strip()
                or default.get("base_url", "")
            )
            key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
            if not key:
                key = _env_value(key_env) if key_env else ""
            if not key:
                return JSONResponse(
                    {"ok": False, "error": f"{provider} 未配置 API Key"}, status_code=400
                )
            if not base_url:
                return JSONResponse(
                    {"ok": False, "error": f"{provider} 未配置 base_url"}, status_code=400
                )
            url = base_url.rstrip("/") + "/chat/completions"
            resp = _requests.post(
                url,
                headers={
                    "Authorization": f"Bearer {key}",
                    "Content-Type": "application/json",
                },
                json={
                    "model": cfg.get("model", ""),
                    "messages": [{"role": "user", "content": "ping"}],
                    "max_tokens": 5,
                },
                timeout=10,
            )
            if resp.status_code < 400:
                return JSONResponse({"ok": True, "status": resp.status_code})
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"HTTP {resp.status_code}: {resp.text[:200]}",
                },
                status_code=400,
            )
        except Exception as e:
            logger.error(f"LLM test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/test-tts")
    async def test_tts():
        try:
            if default_context_cache is None or default_context_cache.tts_engine is None:
                return JSONResponse(
                    {"ok": False, "error": "当前 TTS 引擎未初始化"}, status_code=400
                )
            path = await default_context_cache.tts_engine.async_generate_audio(
                text="你好，我是白～这是一段试听。",
                file_name_no_ext="settings_preview",
            )
            return JSONResponse({"ok": True, "audio_path": path})
        except Exception as e:
            logger.error(f"TTS test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    return router


def init_client_ws_route(default_context_cache: ServiceContext) -> APIRouter:
    """
    Create and return API routes for handling the `/client-ws` WebSocket connections.

    Args:
        default_context_cache: Default service context cache for new sessions.

    Returns:
        APIRouter: Configured router with WebSocket endpoint.
    """

    router = APIRouter()
    ws_handler = WebSocketHandler(default_context_cache)

    @router.websocket("/client-ws")
    async def websocket_endpoint(websocket: WebSocket):
        """WebSocket endpoint for client connections"""
        await websocket.accept()
        client_uid = str(uuid4())

        try:
            await ws_handler.handle_new_connection(websocket, client_uid)
            await ws_handler.handle_websocket_communication(websocket, client_uid)
        except WebSocketDisconnect:
            await ws_handler.handle_disconnect(client_uid)
        except Exception as e:
            logger.error(f"Error in WebSocket connection: {e}")
            await ws_handler.handle_disconnect(client_uid)
            raise

    return router


def init_proxy_route(server_url: str) -> APIRouter:
    """
    Create and return API routes for handling proxy connections.

    Args:
        server_url: The WebSocket URL of the actual server

    Returns:
        APIRouter: Configured router with proxy WebSocket endpoint
    """
    router = APIRouter()
    proxy_handler = ProxyHandler(server_url)

    @router.websocket("/proxy-ws")
    async def proxy_endpoint(websocket: WebSocket):
        """WebSocket endpoint for proxy connections"""
        try:
            await proxy_handler.handle_client_connection(websocket)
        except Exception as e:
            logger.error(f"Error in proxy connection: {e}")
            raise

    return router


def init_webtool_routes(default_context_cache: ServiceContext) -> APIRouter:
    """
    Create and return API routes for handling web tool interactions.

    Args:
        default_context_cache: Default service context cache for new sessions.

    Returns:
        APIRouter: Configured router with WebSocket endpoint.
    """

    router = APIRouter()

    @router.get("/web-tool")
    async def web_tool_redirect():
        """Redirect /web-tool to /web_tool/index.html"""
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/web_tool")
    async def web_tool_redirect_alt():
        """Redirect /web_tool to /web_tool/index.html"""
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/live2d-models/info")
    async def get_live2d_folder_info():
        """Get information about available Live2D models"""
        live2d_dir = "live2d-models"
        if not os.path.exists(live2d_dir):
            return JSONResponse(
                {"error": "Live2D models directory not found"}, status_code=404
            )

        valid_characters = []
        supported_extensions = [".png", ".jpg", ".jpeg"]

        for entry in os.scandir(live2d_dir):
            if entry.is_dir():
                folder_name = entry.name.replace("\\", "/")
                model3_file = os.path.join(
                    live2d_dir, folder_name, f"{folder_name}.model3.json"
                ).replace("\\", "/")

                if os.path.isfile(model3_file):
                    # Find avatar file if it exists
                    avatar_file = None
                    for ext in supported_extensions:
                        avatar_path = os.path.join(
                            live2d_dir, folder_name, f"{folder_name}{ext}"
                        )
                        if os.path.isfile(avatar_path):
                            avatar_file = avatar_path.replace("\\", "/")
                            break

                    valid_characters.append(
                        {
                            "name": folder_name,
                            "avatar": avatar_file,
                            "model_path": model3_file,
                        }
                    )
        return JSONResponse(
            {
                "type": "live2d-models/info",
                "count": len(valid_characters),
                "characters": valid_characters,
            }
        )

    @router.post("/asr")
    async def transcribe_audio(file: UploadFile = File(...)):
        """
        Endpoint for transcribing audio using the ASR engine
        """
        logger.info(f"Received audio file for transcription: {file.filename}")

        try:
            contents = await file.read()

            # Validate minimum file size
            if len(contents) < 44:  # Minimum WAV header size
                raise ValueError("Invalid WAV file: File too small")

            # Decode the WAV header and get actual audio data
            wav_header_size = 44  # Standard WAV header size
            audio_data = contents[wav_header_size:]

            # Validate audio data size
            if len(audio_data) % 2 != 0:
                raise ValueError("Invalid audio data: Buffer size must be even")

            # Convert to 16-bit PCM samples to float32
            try:
                audio_array = (
                    np.frombuffer(audio_data, dtype=np.int16).astype(np.float32)
                    / 32768.0
                )
            except ValueError as e:
                raise ValueError(
                    f"Audio format error: {str(e)}. Please ensure the file is 16-bit PCM WAV format."
                )

            # Validate audio data
            if len(audio_array) == 0:
                raise ValueError("Empty audio data")

            text = await default_context_cache.asr_engine.async_transcribe_np(
                audio_array
            )
            logger.info(f"Transcription result: {text}")
            return {"text": text}

        except ValueError as e:
            logger.error(f"Audio format error: {e}")
            return Response(
                content=json.dumps({"error": str(e)}),
                status_code=400,
                media_type="application/json",
            )
        except Exception as e:
            logger.error(f"Error during transcription: {e}")
            return Response(
                content=json.dumps(
                    {"error": "Internal server error during transcription"}
                ),
                status_code=500,
                media_type="application/json",
            )

    @router.websocket("/tts-ws")
    async def tts_endpoint(websocket: WebSocket):
        """WebSocket endpoint for TTS generation"""
        await websocket.accept()
        logger.info("TTS WebSocket connection established")

        try:
            while True:
                data = await websocket.receive_json()
                text = data.get("text")
                if not text:
                    continue

                logger.info(f"Received text for TTS: {text}")

                # Split text into sentences
                sentences = [s.strip() for s in text.split(".") if s.strip()]

                try:
                    # Generate and send audio for each sentence
                    for sentence in sentences:
                        sentence = sentence + "."  # Add back the period
                        file_name = f"{datetime.now().strftime('%Y%m%d_%H%M%S')}_{str(uuid4())[:8]}"
                        audio_path = (
                            await default_context_cache.tts_engine.async_generate_audio(
                                text=sentence, file_name_no_ext=file_name
                            )
                        )
                        logger.info(
                            f"Generated audio for sentence: {sentence} at: {audio_path}"
                        )

                        await websocket.send_json(
                            {
                                "status": "partial",
                                "audioPath": audio_path,
                                "text": sentence,
                            }
                        )

                    # Send completion signal
                    await websocket.send_json({"status": "complete"})

                except Exception as e:
                    logger.error(f"Error generating TTS: {e}")
                    await websocket.send_json({"status": "error", "message": str(e)})

        except WebSocketDisconnect:
            logger.info("TTS WebSocket client disconnected")
        except Exception as e:
            logger.error(f"Error in TTS WebSocket connection: {e}")
            await websocket.close()

    return router
