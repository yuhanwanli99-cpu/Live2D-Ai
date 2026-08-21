import os
import json
import copy
import tempfile
import time
import asyncio
from uuid import uuid4
import numpy as np
from datetime import datetime
from fastapi import APIRouter, WebSocket, UploadFile, File, Response, Request
from fastapi.responses import FileResponse
from starlette.responses import JSONResponse
from starlette.websockets import WebSocketDisconnect
from loguru import logger
from ruamel.yaml import YAML
from .service_context import ServiceContext
from .websocket_handler import WebSocketHandler
from .proxy_handler import ProxyHandler


# 与 pyproject.toml 的 version 保持一致，供 /health 上报。
_SETTINGS_VERSION = "v1.2.1"
_ALL_TIMEOUT = 5.0

# TTS provider 旧 id → 现 id（审计 R-2：原先这张表在 4 处各写一遍，最易漂移）
_TTS_ID_ALIASES = {
    "gpt_sovits": "gpt_sovits_tts",
    "piper": "piper_tts",
    "sherpa_onnx": "sherpa_onnx_tts",
    "pyttsx3": "pyttsx3_tts",
}

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


def normalize_tts_id(pid: str) -> str:
    """把 TTS provider 的旧 id 归一为现 id（未知 id 原样返回）。"""
    return _TTS_ID_ALIASES.get(pid, pid)



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
    _start_time = time.time()

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
        "openai_compatible_llm": {
            "name": "OpenAI 兼容",
            "base_url": "",
            "key_env": "OPENAI_API_KEY",
        },
    }
    MODEL_CATALOG = {
        "deepseek_llm": ["deepseek-v4-flash", "deepseek-v4-pro"],
        "zhipu_llm": ["glm-4.7-flash", "glm-4.6v"],
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
        "openai_compatible_llm": [],
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
        "gpt_sovits_tts": [],
        "piper_tts": [],
        "sherpa_onnx_tts": [],
        "pyttsx3_tts": [],
        "melo_tts": ["ZH", "EN", "ES", "FR", "JP", "KR"],
    }
    TTS_MODEL_CATALOG = {
        "gpt_sovits_tts": ["GPT-SoVITS"],
        "piper_tts": ["piper"],
        "sherpa_onnx_tts": ["sherpa-onnx"],
        "pyttsx3_tts": ["pyttsx3"],
        "melo_tts": ["cpu", "cuda", "auto"],
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
        "gpt_sovits_tts": {"name": "GPT-SoVITS", "key_env": None},
        "piper_tts": {"name": "Piper", "key_env": None},
        "sherpa_onnx_tts": {"name": "Sherpa-ONNX", "key_env": None},
        "pyttsx3_tts": {"name": "pyttsx3", "key_env": None},
        "melo_tts": {"name": "Melo TTS（本地）", "key_env": None},
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
        # 先写同目录临时文件再原子替换，避免写盘中途失败留下半份 conf.yaml。
        fd, tmp_path = tempfile.mkstemp(
            prefix="conf.", suffix=".yaml.tmp", dir=os.path.dirname(config_abs)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                yaml.dump(data, f)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, config_abs)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

    def _write_raw_atomic(content: str):
        """把用户原始 YAML 文本原子写入 config_abs（与 _save_yaml 同模式），
        避免中途失败留下半份 conf.yaml 导致下次启动失败。"""
        fd, tmp_path = tempfile.mkstemp(
            prefix="conf.", suffix=".yaml.tmp", dir=os.path.dirname(config_abs)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(content)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, config_abs)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

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

    def _normalize_provider_id(pid: str) -> str:
        """把 UI 常用别名归一为配置里的 provider id。"""
        return "openai_compatible_llm" if pid == "openai_compatible" else pid

    def _llm_protocol(provider: str) -> str:
        """按 provider 返回协议族：openai / anthropic / ollama。

        修复 B9：test/discover/check 不能再对所有 provider 一律 OpenAI 形态。
        - anthropic / claude_llm -> Anthropic /v1/messages
        - ollama / ollama_llm    -> Ollama /api/chat (+ /api/tags 列模型)
        - 其余 OpenLLM 各厂家均为 OpenAI 兼容形态（/models + /chat/completions）
        """
        provider = _normalize_provider_id(provider or "")
        if provider in ("anthropic", "claude_llm"):
            return "anthropic"
        if provider in ("ollama", "ollama_llm"):
            return "ollama"
        return "openai"

    def _extract_upstream_message(resp) -> str:
        """从上游错误响应提取人类可读 message，不吞成裸 HTTP 数字。"""
        try:
            body = resp.json()
        except Exception:
            body = None
        if isinstance(body, dict):
            err = body.get("error")
            if isinstance(err, dict) and err.get("message"):
                return str(err["message"])
            if isinstance(err, str):
                return err
            if body.get("message"):
                return str(body["message"])
        text = resp.text or ""
        return text[:300]

    def _save_persona(data):
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        with open(persona_path, "w", encoding="utf-8") as f:
            yaml.dump(data, f)

    def _apply_persona_to_data(data, persona):
        """把 shared/persona.yaml 的提示词/模型/温度/视觉模型应用到待解析配置。

        与 run_server.main() 的启动逻辑保持一致，但作用于当前选中的 LLM Provider，
        不再硬编码 deepseek_llm。
        """
        cc = data.setdefault("character_config", {})
        if persona.get("system_prompt"):
            cc["persona_prompt"] = persona["system_prompt"]

        model_overrides = {
            k: persona[k]
            for k in ("model", "temperature", "max_tokens")
            if persona.get(k) is not None
        }
        if model_overrides:
            agent_cfg = cc.setdefault("agent_config", {})
            llm_configs = agent_cfg.setdefault("llm_configs", {})
            basic = (
                agent_cfg.setdefault("agent_settings", {})
                .setdefault("basic_memory_agent", {})
            )
            provider = _normalize_provider_id(
                basic.get("llm_provider") or "deepseek_llm"
            )
            if provider != basic.get("llm_provider"):
                basic["llm_provider"] = provider
            provider_cfg = llm_configs.setdefault(provider, {})
            if "model" in model_overrides:
                provider_cfg["model"] = model_overrides["model"]
            if "temperature" in model_overrides:
                provider_cfg["temperature"] = model_overrides["temperature"]
            if "max_tokens" in model_overrides:
                provider_cfg["max_tokens"] = model_overrides["max_tokens"]
        if persona.get("vision_model"):
            vision_cfg = data.setdefault("vision_config", {})
            if not isinstance(vision_cfg, dict):
                vision_cfg = data["vision_config"] = {}
            vision_cfg["model"] = persona["vision_model"]
        return data

    def _config_dict_for_reload():
        """构造用于热重载的完整 Config dict。

        与 run_server.main() 的启动逻辑保持一致：
          - conf.yaml 本身不含 persona_prompt（它来自 shared/persona.yaml）
          - 必须注入 persona_prompt，否则 Pydantic 热重载会报 Field required
          - 应用 persona.yaml 中的 model/temperature/max_tokens/vision_model 覆盖
        """
        from .config_manager.utils import read_yaml

        data = read_yaml(config_abs)
        return _apply_persona_to_data(data, _load_persona() or {})

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

    def _load_env_map() -> dict:
        """一次性把 .env 读成 dict（审计 E-2）。

        原先 `_get_common` 会对 ~16 个 LLM + ~13 个 TTS provider 各调一次
        `_has_key` → 单个 `GET /api/config` 重复全量读 .env 近 30 次。
        """
        env_map = {}
        if os.path.exists(env_path):
            with open(env_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("#") or "=" not in line:
                        continue
                    k, _, v = line.partition("=")
                    env_map[k.strip()] = v.strip()
        return env_map

    def _requests_get(url: str, headers: dict, timeout: float):
        """同步 requests.get 移出事件循环（审计 E-1）。

        FastAPI 单线程事件循环：直接同步请求会把所有已连接 WS 会话
        （对话/口型/字幕/心跳）一起卡住。这里统一走线程池。
        """
        import requests as _requests

        return asyncio.to_thread(_requests.get, url, headers=headers, timeout=timeout)

    def _requests_post(url: str, headers: dict, json_body: dict, timeout: float):
        """同步 requests.post 移出事件循环（审计 E-1）。"""
        import requests as _requests

        return asyncio.to_thread(
            lambda: _requests.post(
                url, headers=headers, json=json_body, timeout=timeout
            )
        )

    def _models_endpoint(protocol: str, base_url: str, api_key: str) -> tuple[str, dict]:
        """按协议族给出「列模型」的 URL 与请求头（审计 R-4：三处曾各写一遍）。"""
        base = base_url.rstrip("/")
        if protocol == "ollama":
            return base + "/api/tags", {}  # Ollama 列模型无需鉴权
        if protocol == "anthropic":
            headers = (
                {"x-api-key": api_key, "anthropic-version": "2023-06-01"}
                if api_key
                else {}
            )
            return base + "/models", headers
        headers = {"Authorization": f"Bearer {api_key}"} if api_key else {}
        return base + "/models", headers

    def _models_from_response(protocol: str, raw) -> list:
        """把「列模型」响应解析成模型名列表（Ollama 结构与 OpenAI 系不同）。"""
        if protocol == "ollama":
            return [
                str(m.get("name", ""))
                for m in (raw.get("models") or [])
                if isinstance(m, dict) and m.get("name")
            ]
        return _parse_models_response(raw)

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
        # .env 一次读入，provider 循环里只查内存（审计 E-2）
        env_map = _load_env_map()

        def env_has(name) -> bool:
            if not name:
                return False
            return bool(env_map.get(name) or os.getenv(name, ""))

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
                    "has_key": env_has(key_env) if key_env else False,
                    "configured": pid in llm_configs,
                }
            )

        tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(TTS_DEFAULTS.keys())))
        tts_provider_list = []
        for pid in tts_ids:
            if pid in _TTS_NON_PROVIDER_KEYS:
                continue
            cfg = tts_cfg.get(pid, {}) or {}
            if not isinstance(cfg, dict):
                cfg = {}
            default = TTS_DEFAULTS.get(pid, {})
            key_env = default.get("key_env")
            known_voices = cfg.get("voices") or TTS_VOICE_CATALOG.get(pid, [])
            known_tts_models = cfg.get("models") or TTS_MODEL_CATALOG.get(pid, [])
            current_voice = cfg.get("voice", "")
            current_tts_model = cfg.get("model", "")
            if pid == "melo_tts":
                # Melo 的 conf 字段是 language/device；旧版可能残留 voice/model。
                current_voice = current_voice or cfg.get("language", "")
                current_tts_model = current_tts_model or cfg.get("device", "")
            if not current_voice and known_voices:
                current_voice = known_voices[0]
            if not current_tts_model and known_tts_models:
                current_tts_model = known_tts_models[0]
            entry = {
                "id": pid,
                "name": default.get("name", pid),
                "voice": current_voice,
                "model": current_tts_model,
                "ws_url": cfg.get("ws_url", ""),
                "voices": known_voices,
                "models": known_tts_models,
                "key_env": key_env,
                "has_key": env_has(key_env) if key_env else True,
                "configured": pid in tts_cfg,
            }
            if pid == "melo_tts":
                entry.update(
                    {
                        "language": current_voice,
                        "device": current_tts_model,
                        "speaker": cfg.get("speaker", "") or "",
                        "speed": cfg.get("speed", 1.0),
                        "sdp_ratio": cfg.get("sdp_ratio", 0.2),
                        "noise_scale": cfg.get("noise_scale", 0.6),
                        "noise_scale_w": cfg.get("noise_scale_w", 0.8),
                        "format": cfg.get("format", "wav"),
                    }
                )
            tts_provider_list.append(entry)

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
                        if k not in _TTS_NON_PROVIDER_KEYS
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
            "log_level": _env_value("LIVE2DAI_LOG_LEVEL") or "INFO",
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

    @router.get("/health")
    async def health():
        # WSL2 迁移验收项：GET /health -> 200 {"status":"ok","version":..,"uptime_s":..}
        return JSONResponse(
            {
                "status": "ok",
                "version": _SETTINGS_VERSION,
                "uptime_s": int(time.time() - _start_time),
            }
        )

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
                requested_tts = normalize_tts_id(payload["tts_model"])
                cc.setdefault("tts_config", {})["tts_model"] = requested_tts
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
            for raw_pid, override in list(llm_overrides.items()):
                pid = _normalize_provider_id(raw_pid)
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

            # 自动补齐当前选中 LLM 的最小配置（未配置但被选中的 Provider 也能保存/热重载）
            selected_llm = _normalize_provider_id(basic.get("llm_provider", ""))
            if selected_llm != basic.get("llm_provider", ""):
                basic["llm_provider"] = selected_llm
            if selected_llm and selected_llm not in existing_llm:
                llm_default = LLM_DEFAULTS.get(selected_llm, {})
                target = existing_llm[selected_llm] = {}
                key_env = llm_default.get("key_env") or "OPENAI_API_KEY"
                target["llm_api_key"] = "${" + key_env + "}"
                if llm_default.get("base_url"):
                    target["base_url"] = llm_default["base_url"]
                catalog_models = MODEL_CATALOG.get(selected_llm, [])
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

            if "tts_model" in payload and payload["tts_model"]:
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
                        tts_voices = TTS_VOICE_CATALOG.get(requested_tts, [])
                        tts_models = TTS_MODEL_CATALOG.get(requested_tts, [])
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
                            voices = TTS_VOICE_CATALOG.get(selected_tts, [])
                            if voices:
                                sel_entry["voice"] = voices[0]
                                sel_entry.setdefault("voices", voices)
                        if not sel_entry.get("model"):
                            models = TTS_MODEL_CATALOG.get(selected_tts, [])
                            if models:
                                sel_entry["model"] = models[0]
                                sel_entry.setdefault("models", models)

            # 设置页的“视觉模型”写入 vision_config.model（如果用户填写）
            persona_payload = payload.get("persona")
            if persona_payload and persona_payload.get("vision_model"):
                vision_cfg = data.setdefault("vision_config", {})
                if not isinstance(vision_cfg, dict):
                    vision_cfg = data["vision_config"] = {}
                vision_cfg["model"] = persona_payload["vision_model"]

            # 写盘前用 Config 做一次完整校验，避免非法 Provider/配置落盘。
            # （normalize + 校验都在 _save_yaml/_save_persona 之前完成 → 校验失败
            #  保证三个落盘目标都不会被改写，修复 P0-2 热重载失败后 YAML 已落盘。）
            try:
                from .config_manager.main import Config as AppConfig
                from pydantic import ValidationError as _PydanticValidationError

                persona_for_validation = _load_persona() or {}
                persona_payload = payload.get("persona")
                if persona_payload:
                    for field in (
                        "name",
                        "role",
                        "model",
                        "temperature",
                        "max_tokens",
                        "vision_model",
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
                _apply_persona_to_data(candidate, persona_for_validation)
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
            except Exception as validation_error:
                logger.error(f"Config validation failed: {validation_error}")
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"配置校验失败: {validation_error}",
                        "field_errors": {},
                    },
                    status_code=400,
                )

            _save_yaml(data)

            # 写入仓库根 .env（Key 不落 conf.yaml，避免误提交）
            # 显式清空语义（spec 1.5）：api_keys 里值为 null 的 key 会被删除；
            # 未提供的 key 保持原样；非空值则写入/更新。
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

            # 人设/提示词保存（含显式清空：null 删除字段）
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
                    if field not in persona_payload:
                        continue
                    if persona_payload[field] is None:
                        persona.pop(field, None)
                    elif persona_payload[field] != "":
                        persona[field] = persona_payload[field]
                _save_persona(persona)

            # normalize 后完整视图：保存成功回传，供前端 re-populate（spec 1.3 响应）
            normalized_config = _get_common(_load_yaml())

            if payload.get("apply") and default_context_cache is not None:
                try:
                    # 让新写入 .env 的 LIVE2DAI_* Key 对热重载可见（B4）
                    try:
                        from dotenv import load_dotenv
                        load_dotenv(env_path, override=True)
                    except Exception as env_error:
                        logger.warning(f"Failed to reload .env for apply: {env_error}")

                    from .config_manager.main import Config as AppConfig
                    new_config = AppConfig(**_config_dict_for_reload())
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
                        except Exception as ctx_err:
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
                except Exception as reload_error:
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
                if field not in payload:
                    continue
                if payload[field] is None:
                    # 显式清空：删除字段
                    persona.pop(field, None)
                elif payload[field] != "":
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
            tree = _yaml.safe_load(raw)  # 语法校验，失败会抛异常
            if not isinstance(tree, dict):
                return JSONResponse(
                    {"error": "配置树顶层必须是键值映射（YAML object）"},
                    status_code=400,
                )
            # 写盘前做与 update_config 一致的 schema 校验（含 persona 注入），
            # 避免“合法 YAML 但结构坏掉”写坏 conf.yaml 导致下次启动失败。
            candidate = copy.deepcopy(tree)
            _apply_persona_to_data(candidate, _load_persona() or {})
            try:
                from .config_manager.main import Config as AppConfig
                from pydantic import ValidationError as _PydanticValidationError

                AppConfig(**candidate)
            except _PydanticValidationError as validation_error:
                logger.error(f"Config tree validation failed: {validation_error}")
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
            except Exception as validation_error:
                logger.error(f"Config tree validation failed: {validation_error}")
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"配置校验失败: {validation_error}",
                        "field_errors": {},
                    },
                    status_code=400,
                )
            # 同目录临时文件 + os.replace 原子替换（与 _save_yaml 一致性），
            # 避免写盘中途失败留下半份 conf.yaml 导致下次启动失败。
            _write_raw_atomic(raw)
            return JSONResponse(
                {
                    "ok": True,
                    "config": _get_common(tree),
                    "applied": False,
                    "requires_restart": True,
                    "message": "已保存",
                }
            )
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

            # P1-4：不再要求重启——直接热切换并推给所有在线会话
            applied = 0
            hot_error = ""
            if ws_handler is not None:
                try:
                    applied = await ws_handler.broadcast_model_change(model_id)
                except Exception as e:  # noqa: BLE001 - 热切换失败要诚实回报（配置已落盘）
                    hot_error = str(e)
                    logger.error(f"形象热切换失败（配置已保存）: {e}")

            if hot_error:
                return JSONResponse(
                    {
                        "ok": True,
                        "applied": False,
                        "requires_restart": True,
                        "message": f"已保存，重启后生效（热切换失败: {hot_error}）",
                    }
                )
            return JSONResponse(
                {
                    "ok": True,
                    "applied": True,
                    "requires_restart": False,
                    "clients_updated": applied,
                    "message": (
                        f"已切换并即时生效（{applied} 个窗口已更新）"
                        if applied
                        else "已切换；当前没有已连接窗口，打开页面即生效"
                    ),
                }
            )
        except Exception as e:
            logger.error(f"Failed to select model: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/llm/discover-models")
    async def discover_llm_models(request: Request):
        try:
            payload = await request.json()
            provider = _normalize_provider_id(payload.get("provider", ""))
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
            protocol = _llm_protocol(provider)
            url, headers = _models_endpoint(protocol, base_url, api_key)
            resp = await _requests_get(url, headers=headers, timeout=15)
            if resp.status_code >= 400:
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"HTTP {resp.status_code}: {_extract_upstream_message(resp)}",
                    },
                    status_code=400,
                )
            models = _models_from_response(protocol, resp.json())
            return JSONResponse({"ok": True, "models": models})
        except Exception as e:
            logger.error(f"Discover models failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/providers/check")
    async def check_providers(request: Request):
        try:
            payload = {}
            try:
                payload = await request.json()
            except Exception:
                pass
            data = _load_yaml()
            cc = data.get("character_config", {}) or {}
            agent_cfg = cc.get("agent_config", {}) or {}
            llm_configs = agent_cfg.get("llm_configs", {}) or {}
            tts_cfg = cc.get("tts_config", {}) or {}
            llm_overrides = payload.get("llm_configs") or {}
            tts_overrides = payload.get("tts_configs") or {}
            api_keys = payload.get("api_keys") or {}
            provider_ids = list(
                dict.fromkeys(list(llm_configs.keys()) + list(LLM_DEFAULTS.keys()))
            )

            # 审计 E-1：原实现在 async 端点里**串行**探测 ~16 个 provider（各 timeout=8），
            # 最坏近 128s 阻塞事件循环 → 全站 WS（对话/口型/字幕/心跳）一起卡死。
            # 现改为「线程池 + 限并发 gather」：既不阻塞事件循环，总耗时也从 Σ 变成 max。
            semaphore = asyncio.Semaphore(_CHECK_CONCURRENCY)

            async def probe_llm(pid: str) -> dict:
                cfg = dict(llm_configs.get(pid, {}) or {})
                override = llm_overrides.get(pid)
                if isinstance(override, dict):
                    for k in ("base_url", "model"):
                        if override.get(k) not in (None, ""):
                            cfg[k] = override[k]
                default = LLM_DEFAULTS.get(pid, {})
                base_url = (
                    cfg.get("base_url", "").strip()
                    or default.get("base_url", "")
                )
                key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
                override_key = (
                    override.get("api_key") if isinstance(override, dict) else None
                )
                key = override_key or api_keys.get(key_env) or _env_value(key_env)
                if not key:
                    return {"provider": pid, "kind": "llm", "ok": False, "status": "未配置 Key", "models": []}
                if not base_url:
                    return {"provider": pid, "kind": "llm", "ok": False, "status": "未配置 Base URL", "models": []}
                try:
                    protocol = _llm_protocol(pid)
                    url, headers = _models_endpoint(protocol, base_url, key)
                    async with semaphore:
                        resp = await _requests_get(url, headers=headers, timeout=8)
                    if resp.status_code < 400:
                        return {
                            "provider": pid,
                            "kind": "llm",
                            "ok": True,
                            "status": "ok",
                            "models": _models_from_response(protocol, resp.json()),
                        }
                    return {
                        "provider": pid,
                        "kind": "llm",
                        "ok": False,
                        "status": f"HTTP {resp.status_code}: {_extract_upstream_message(resp)}",
                        "models": [],
                    }
                except Exception as e:  # noqa: BLE001 - 单个 provider 失败不影响其余
                    return {"provider": pid, "kind": "llm", "ok": False, "status": str(e), "models": []}

            # 顺序与 provider_ids 一致（gather 保序），前端按序展示不受并发影响
            llm_results = list(await asyncio.gather(*(probe_llm(pid) for pid in provider_ids)))

            tts_results = []
            tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(TTS_DEFAULTS.keys())))
            for pid in tts_ids:
                if pid in _TTS_NON_PROVIDER_KEYS:
                    continue
                cfg = dict(tts_cfg.get(pid, {}) or {})
                override = tts_overrides.get(pid)
                if isinstance(override, dict):
                    for k in ("voice", "model", "ws_url"):
                        if override.get(k) not in (None, ""):
                            cfg[k] = override[k]
                key_env = TTS_DEFAULTS.get(pid, {}).get("key_env")
                override_key = (
                    override.get("api_key") if isinstance(override, dict) else None
                )
                has_ui_key = bool(override_key) or bool(
                    key_env and api_keys.get(key_env)
                )
                configured = has_ui_key if key_env else True
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
            payload = await request.json()
            provider = _normalize_provider_id(payload.get("provider", ""))
            key = (payload.get("api_key") or "").strip()
            base_url = (payload.get("base_url") or "").strip()
            model = (payload.get("model") or "").strip()
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
            if not model:
                model = cfg.get("model", "")
            key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
            if not key:
                key = _env_value(key_env) if key_env else ""
            if not key and _llm_protocol(provider) != "ollama":
                return JSONResponse(
                    {"ok": False, "error": f"{provider} 未配置 API Key"}, status_code=400
                )
            if not base_url:
                return JSONResponse(
                    {"ok": False, "error": f"{provider} 未配置 base_url"}, status_code=400
                )
            if not model:
                return JSONResponse(
                    {"ok": False, "error": f"{provider} 未配置 model"}, status_code=400
                )
            # 一次 ping 探活；按协议族给出 URL / header / body（审计 E-1：走线程池不阻塞事件循环）
            protocol = _llm_protocol(provider)
            base = base_url.rstrip("/")
            max_tokens = payload.get("max_tokens") or 5
            if protocol == "ollama":
                url = base + "/api/chat"
                headers = {"Content-Type": "application/json"}
                body = {
                    "model": model,
                    "messages": [{"role": "user", "content": "ping"}],
                    "stream": False,
                }
            elif protocol == "anthropic":
                # base_url 形如 https://api.anthropic.com/v1 或 .../v1/：
                # 追加 /v1/messages 前先避免重复 /v1。
                url = base + ("/messages" if base.endswith("/v1") else "/v1/messages")
                headers = {
                    "x-api-key": key,
                    "anthropic-version": "2023-06-01",
                    "Content-Type": "application/json",
                }
                body = {
                    "model": model,
                    "max_tokens": max_tokens,
                    "messages": [{"role": "user", "content": "ping"}],
                }
            else:
                url = base + "/chat/completions"
                headers = {
                    "Authorization": f"Bearer {key}",
                    "Content-Type": "application/json",
                }
                body = {
                    "model": model,
                    "messages": [{"role": "user", "content": "ping"}],
                    "max_tokens": max_tokens,
                }
            resp = await _requests_post(url, headers=headers, json_body=body, timeout=10)
            if resp.status_code < 400:
                return JSONResponse({"ok": True, "status": resp.status_code})
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"HTTP {resp.status_code}: {_extract_upstream_message(resp)}",
                },
                status_code=400,
            )
        except Exception as e:
            logger.error(f"LLM test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    @router.post("/api/test-tts")
    async def test_tts(request: Request):
        try:
            payload = {}
            try:
                payload = await request.json()
            except Exception:
                payload = {}
            provider = normalize_tts_id((payload.get("provider") or "").strip())
            data = _load_yaml()
            tts_cfg = (data.get("character_config", {}) or {}).get("tts_config", {}) or {}
            preview_text = (
                _LOCAL_TTS_PREVIEW_TEXT
                if provider == "melo_tts"
                else "你好，我是白～这是一段试听。"
            )

            if not provider:
                # 旧行为：没有指定 provider 时测试当前已加载引擎
                if default_context_cache is None or default_context_cache.tts_engine is None:
                    return JSONResponse(
                        {"ok": False, "error": "当前 TTS 引擎未初始化"}, status_code=400
                    )
                path = await default_context_cache.tts_engine.async_generate_audio(
                    text=preview_text,
                    file_name_no_ext="settings_preview",
                )
                if not path:
                    return JSONResponse(
                        {"ok": False, "error": "TTS 合成失败：引擎未返回音频文件"}, status_code=400
                    )
                return JSONResponse({"ok": True, "audio_path": path})

            # 用已保存的 typed config 作为基底，再叠加当前 UI 未保存参数
            params = {}
            try:
                from .config_manager.main import Config as AppConfig

                candidate = copy.deepcopy(data)
                _apply_persona_to_data(candidate, _load_persona() or {})
                parsed = AppConfig(**candidate)
                typed_tts = parsed.character_config.tts_config
                typed_provider = getattr(typed_tts, provider, None)
                if typed_provider is not None:
                    params = typed_provider.model_dump()
            except Exception as parse_error:
                logger.warning(f"TTS test config parse failed, use raw fallback: {parse_error}")

            if not params:
                saved = tts_cfg.get(provider, {}) or {}
                if isinstance(saved, dict):
                    for k in (
                        "voice", "model", "ws_url", "api_key",
                        "base_url", "api_url", "region", "model_path",
                        "language", "device", "speaker", "speed",
                        "sdp_ratio", "noise_scale", "noise_scale_w", "format",
                    ):
                        if saved.get(k) not in (None, ""):
                            params[k] = saved[k]
                if provider == "melo_tts" and not params:
                    from .config_manager.tts import MeloTTSConfig
                    params = MeloTTSConfig().model_dump()
                elif provider == "edge_tts" and not params.get("voice"):
                    params["voice"] = (TTS_VOICE_CATALOG.get("edge_tts") or ["zh-CN-XiaoxiaoNeural"])[0]
                elif provider == "piper_tts" and not params:
                    from .config_manager.tts import PiperTTSConfig
                    params = PiperTTSConfig().model_dump()
                elif provider == "pyttsx3_tts":
                    params = {}

            # 用 UI 当前值覆盖；已保存 typed config 存在时只覆盖其已有的字段，
            # 避免把 UI 的 model/ws_url 误传给 Edge/pyttsx3 等严格构造器。
            has_typed_params = bool(params)
            for field in (
                "voice", "model", "ws_url", "base_url", "api_url", "region",
                "model_path", "language", "device", "speaker", "speed",
                "sdp_ratio", "noise_scale", "noise_scale_w", "format",
            ):
                if payload.get(field) not in (None, ""):
                    if not has_typed_params or field in params:
                        params[field] = payload[field]
            if provider == "melo_tts":
                # 设置 UI 复用 voice/model 下拉承载 language/device；这里显式映射。
                if payload.get("voice") not in (None, ""):
                    params["language"] = payload["voice"]
                if payload.get("model") not in (None, ""):
                    params["device"] = payload["model"]
            if payload.get("api_key") not in (None, ""):
                if not has_typed_params or "api_key" in params:
                    params["api_key"] = payload["api_key"]
            elif not params.get("api_key"):
                key_env = TTS_DEFAULTS.get(provider, {}).get("key_env")
                if key_env:
                    params["api_key"] = _env_value(key_env)

            # pyttsx3 是无参本地引擎，绝不能透传任何字段
            if provider == "pyttsx3_tts":
                params = {}

            from .tts.tts_factory import TTSFactory
            engine = TTSFactory.get_tts_engine(provider, **params)
            path = await engine.async_generate_audio(
                text=preview_text,
                file_name_no_ext="settings_preview",
            )
            if not path:
                return JSONResponse(
                    {"ok": False, "error": "TTS 合成失败：引擎未返回音频文件"}, status_code=400
                )
            return JSONResponse({"ok": True, "audio_path": path})
        except Exception as e:
            logger.error(f"TTS test failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)

    # ------------------------------------------------------------------
    # 本地 Melo TTS REST 接口（PLAN-V2-PC-LOCAL-TTS：阶段 3）
    # ------------------------------------------------------------------
    _local_tts_cache_dir = os.path.join(os.path.dirname(config_abs), "cache")
    _local_tts_engine = None
    _local_tts_engine_key = None
    _local_tts_lock = asyncio.Lock()

    def _local_tts_clamp(value, lo: float, hi: float, default: float) -> float:
        try:
            v = float(value)
        except (TypeError, ValueError):
            return default
        return max(lo, min(hi, v))

    def _local_tts_make_params(payload: dict) -> dict:
        language = str(payload.get("language") or payload.get("voice") or "ZH").upper()
        if language not in ("ZH", "EN", "ES", "FR", "JP", "KR"):
            raise ValueError(
                f"不支持的 Melo 语言: {language}，可选 ZH/EN/ES/FR/JP/KR"
            )
        device = str(payload.get("device") or payload.get("model") or "cpu").lower()
        if device not in ("cpu", "auto") and not device.startswith("cuda"):
            device = "cpu"
        speaker = payload.get("speaker") or None
        fmt = str(payload.get("format") or "wav").lower() or "wav"
        return {
            "language": language,
            "device": device,
            "speaker": speaker,
            "speed": _local_tts_clamp(payload.get("speed", 1.0), 0.5, 2.0, 1.0),
            "sdp_ratio": _local_tts_clamp(payload.get("sdp_ratio", 0.2), 0.0, 1.0, 0.2),
            "noise_scale": _local_tts_clamp(payload.get("noise_scale", 0.6), 0.0, 1.0, 0.6),
            "noise_scale_w": _local_tts_clamp(payload.get("noise_scale_w", 0.8), 0.0, 1.0, 0.8),
            "format": fmt,
        }

    async def _local_tts_get_engine(params: dict):
        nonlocal _local_tts_engine, _local_tts_engine_key
        key = tuple(sorted(params.items()))
        async with _local_tts_lock:
            if _local_tts_engine is None or key != _local_tts_engine_key:
                from .tts.tts_factory import TTSFactory

                _local_tts_engine = await asyncio.to_thread(
                    TTSFactory.get_tts_engine, "melo_tts", **params
                )
                _local_tts_engine_key = key
            return _local_tts_engine

    def _local_tts_safe_name(name: str):
        if not name:
            return None
        if name != os.path.basename(name):
            return None
        if ".." in name or name.startswith("."):
            return None
        if not name.startswith(_LOCAL_TTS_FILE_PREFIX) or not name.endswith(".wav"):
            return None
        return name

    def _audio_duration_ms(path: str) -> int:
        try:
            import soundfile as sf

            info = sf.info(path)
            if info.samplerate:
                return int(info.frames * 1000 / info.samplerate)
        except Exception:  # noqa: BLE001, S110 - 时长解析失败按 0 处理
            pass
        try:
            import wave

            with wave.open(path, "rb") as wav_file:
                rate = wav_file.getframerate()
                if rate:
                    return int(wav_file.getnframes() * 1000 / rate)
        except Exception:  # noqa: BLE001 - 非 wav 时回退 0
            return 0
        return 0

    @router.post("/api/tts/local/synthesize")
    async def local_tts_synthesize(request: Request):
        try:
            try:
                payload = await request.json()
            except Exception:  # noqa: BLE001 - FastAPI 未提供专用 JSON 解析异常
                return JSONResponse(
                    {"ok": False, "error": "请求体必须是 JSON"}, status_code=400
                )
            if not isinstance(payload, dict):
                return JSONResponse(
                    {"ok": False, "error": "请求体必须是 JSON 对象"}, status_code=400
                )
            text = str(payload.get("text") or "").strip()
            if not text:
                return JSONResponse(
                    {"ok": False, "error": "text 不能为空"}, status_code=400
                )
            if len(text) > _LOCAL_TTS_MAX_TEXT_LEN:
                return JSONResponse(
                    {
                        "ok": False,
                        "error": f"文本长度超过上限 {_LOCAL_TTS_MAX_TEXT_LEN} 字符",
                    },
                    status_code=400,
                )
            params = _local_tts_make_params(payload)
            engine = await _local_tts_get_engine(params)
            file_stem = f"{_LOCAL_TTS_FILE_PREFIX}{uuid4().hex[:12]}"
            async with _local_tts_lock:
                path = await engine.async_generate_audio(
                    text=text, file_name_no_ext=file_stem
                )
            if not path:
                return JSONResponse(
                    {"ok": False, "error": "Melo 合成失败：引擎未返回音频文件"},
                    status_code=500,
                )
            abs_path = path if os.path.isabs(path) else os.path.abspath(path)
            safe_name = _local_tts_safe_name(os.path.basename(abs_path))
            if safe_name is None or not os.path.isfile(abs_path):
                return JSONResponse(
                    {"ok": False, "error": "Melo 合成失败：音频文件未生成"},
                    status_code=500,
                )
            return JSONResponse(
                {
                    "ok": True,
                    "audio_path": f"cache/{safe_name}",
                    "audio_url": f"/api/tts/local/audio?name={safe_name}",
                    "format": params["format"],
                    "duration_ms": _audio_duration_ms(abs_path),
                }
            )
        except ValueError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=400)
        except RuntimeError as exc:
            return JSONResponse({"ok": False, "error": str(exc)}, status_code=400)
        except Exception as exc:  # noqa: BLE001 - 接口需给出统一 JSON 错误
            logger.exception(f"Local Melo TTS synthesize failed: {exc}")
            return JSONResponse(
                {"ok": False, "error": f"本地 Melo TTS 合成失败: {exc}"},
                status_code=500,
            )

    @router.get("/api/tts/local/audio")
    async def local_tts_audio(name: str):
        safe_name = _local_tts_safe_name(name)
        if safe_name is None:
            return JSONResponse(
                {"ok": False, "error": "非法音频文件名"}, status_code=403
            )
        audio_path = os.path.abspath(os.path.join(_local_tts_cache_dir, safe_name))
        if os.path.dirname(audio_path) != os.path.abspath(_local_tts_cache_dir):
            return JSONResponse(
                {"ok": False, "error": "非法音频路径"}, status_code=403
            )
        if not os.path.isfile(audio_path):
            return JSONResponse(
                {"ok": False, "error": "音频不存在"}, status_code=404
            )
        return FileResponse(audio_path, media_type="audio/wav")

    return router


def init_client_ws_route(
    default_context_cache: ServiceContext,
    ws_handler: WebSocketHandler = None,
) -> APIRouter:
    """
    Create and return API routes for handling the `/client-ws` WebSocket connections.

    Args:
        default_context_cache: Default service context cache for new sessions.
        ws_handler: Optional shared WebSocketHandler. When provided it is reused
            (so the config API can traverse the same client_contexts on apply);
            otherwise a dedicated one is created.

    Returns:
        APIRouter: Configured router with WebSocket endpoint.
    """

    router = APIRouter()
    ws_handler = ws_handler or WebSocketHandler(default_context_cache)

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
