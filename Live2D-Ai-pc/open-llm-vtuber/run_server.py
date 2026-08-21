"""Launcher script for Open-LLM-VTuber server (Live2D-Ai slimmed, no-gateway).
- Resolves shared/ paths to the project root shared/ (cross-platform config)
- Pre-loads persona from shared/persona.yaml into persona_prompt
- All API calls go directly to DeepSeek / GLM (no gateway)
"""

import sys
import os
import asyncio
import uvicorn
import yaml
from dotenv import load_dotenv

_project_root = os.path.dirname(os.path.abspath(__file__))
os.chdir(_project_root)

# 本地密钥：仓库根 .env（gitignored；不存在则静默跳过）——必须在读取配置前加载。
# load_dotenv 默认找 cwd（open-llm-vtuber/），而 .env 在项目根 → 显式指定绝对路径。
# 使用 override=True：即使父 shell 带有空的环境变量，也以 .env 文件为准。
_PROJECT_BASE = os.path.dirname(os.path.dirname(_project_root))  # 项目根 Live2D-Ai/（上两级）
load_dotenv(os.path.join(_PROJECT_BASE, ".env"), override=True)

sys.path.insert(0, os.path.join(_project_root, "src"))
sys.path.insert(0, _project_root)

# 统一日志：控制台紧凑彩色（LIVE2DAI_LOG_LEVEL，默认 INFO）+ logs/server.log 滚动文件；
# 同时压制 uvicorn 访问日志刷屏。必须在导入业务模块前安装。
from open_llm_vtuber.logsetup import setup_logging

_effective_level = setup_logging()
print(f"[Live2D-Ai] 日志级别: {_effective_level}（控制台）+ logs/server.log（滚动文件）")

from open_llm_vtuber.server import WebSocketServer
from open_llm_vtuber.config_manager.main import Config
from open_llm_vtuber.config_manager.utils import read_yaml  # 带 ${VAR} 环境替换的配置读取


def _resolve_shared_path(config_path: str) -> str:
    """Resolve 'shared/xxx' relative to the project root shared/ directory.
    Two levels up from open-llm-vtuber/ -> project root (Live2D-Ai/)"""
    if config_path.startswith("shared/"):
        base = os.path.dirname(os.path.dirname(_project_root))  # 项目根 Live2D-Ai/
        return os.path.join(base, config_path)
    return config_path


def _load_persona_from_config(config_dict: dict) -> tuple[str, dict]:
    """Load persona system_prompt + model config from shared/persona.yaml if specified.
    Returns (persona_prompt, model_overrides) where model_overrides may contain:
      - model: str (e.g. 'deepseek-v4-flash')
      - temperature: float
      - max_tokens: int
      - vision_model: str
    """
    cc = config_dict.get("character_config", {})
    persona_file = cc.get("load_persona_from_file", "")
    empty_overrides = {}
    if not persona_file:
        return cc.get("persona_prompt", ""), empty_overrides
    resolved = _resolve_shared_path(persona_file)
    if not os.path.exists(resolved):
        print(f"[WARN] Persona file not found: {resolved}")
        return cc.get("persona_prompt", ""), empty_overrides
    try:
        with open(resolved, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        prompt = data.get("system_prompt", "")
        overrides = {
            k: data[k]
            for k in ("model", "temperature", "max_tokens", "vision_model")
            if k in data
        }
        if prompt:
            print(f"[Live2D-Ai] Loaded persona from {resolved}")
            # 核心链路不含视觉：vision_model 不进启动横幅（避免误导）
            core_overrides = {k: v for k, v in overrides.items() if k != "vision_model"}
            if core_overrides:
                print(f"[Live2D-Ai] Persona model overrides: {core_overrides}")
        return prompt, overrides
    except Exception as e:
        print(f"[WARN] Failed to load persona from {resolved}: {e}")
        return cc.get("persona_prompt", ""), empty_overrides


async def main():
    print("[Live2D-Ai] Starting Open-LLM-VTuber server (direct-API mode, no gateway)...")
    print(f"[Live2D-Ai] Working directory: {_project_root}")

    config_dict = read_yaml("conf.yaml")  # read_yaml 完成 ${DEEPSEEK_API_KEY} 等环境变量替换（yaml.safe_load 不替换）

    # Pre-load persona from shared/persona.yaml (no longer injected by gateway)
    persona, model_overrides = _load_persona_from_config(config_dict)
    if persona:
        config_dict.setdefault("character_config", {})["persona_prompt"] = persona
    else:
        print("[FATAL] Persona prompt is empty! Cannot start without a valid persona.")
        print("[FATAL] Expected file: shared/persona.yaml")
        print(
            "[FATAL] Please ensure the file exists and contains a 'system_prompt' field."
        )
        sys.exit(1)

    # Apply model overrides from persona.yaml to the currently selected LLM provider
    if model_overrides:
        agent_cfg = config_dict.setdefault("character_config", {}).setdefault(
            "agent_config", {}
        )
        llm_configs = agent_cfg.setdefault("llm_configs", {})
        basic = agent_cfg.setdefault("agent_settings", {}).setdefault(
            "basic_memory_agent", {}
        )
        provider = basic.get("llm_provider") or "deepseek_llm"
        if provider == "openai_compatible":
            provider = "openai_compatible_llm"
            basic["llm_provider"] = provider
        provider_cfg = llm_configs.setdefault(provider, {})
        if model_overrides.get("model"):
            provider_cfg["model"] = model_overrides["model"]
            print(f"[Live2D-Ai] LLM model set to: {model_overrides['model']}")
        if model_overrides.get("temperature") is not None:
            provider_cfg["temperature"] = model_overrides["temperature"]
            print(
                f"[Live2D-Ai] LLM temperature set to: {model_overrides['temperature']}"
            )
        if model_overrides.get("max_tokens") is not None:
            provider_cfg["max_tokens"] = model_overrides["max_tokens"]
            print(
                f"[Live2D-Ai] LLM max_tokens set to: {model_overrides['max_tokens']}"
            )
        if model_overrides.get("vision_model"):
            # 仅当用户显式开启视觉管线时才应用/提示 vision 模型（核心链路不需要视觉）
            _vision_enabled = bool(
                (config_dict.get("vision_config") or {}).get("vision_enabled")
            )
            if _vision_enabled:
                vision_cfg = config_dict.setdefault("vision_config", {})
                if not isinstance(vision_cfg, dict):
                    vision_cfg = config_dict["vision_config"] = {}
                vision_cfg["model"] = model_overrides["vision_model"]
                print(
                    f"[Live2D-Ai] Vision model set to: {model_overrides['vision_model']}"
                )

    # Resolve MCP server config path
    if "mcp_servers" in config_dict:
        config_dict["mcp_servers"] = _resolve_shared_path(config_dict["mcp_servers"])

    config = Config(**config_dict)
    server = WebSocketServer(config)
    await server.initialize()

    # 端口取 config.system_config.port（不再写死 12393），使设置里改 port 后按新端口监听（B5）。
    # uvicorn 自身日志级别跟随 LIVE2DAI_LOG_LEVEL（访问日志已由 logsetup 压制刷屏）。
    listen_port = int(config.system_config.port or 12393)
    uvicorn_config = uvicorn.Config(
        server.app,
        host="0.0.0.0",
        port=listen_port,
        log_level=_effective_level.lower(),
    )
    uvicorn_server = uvicorn.Server(uvicorn_config)
    print(f"[Live2D-Ai] Server listening on http://0.0.0.0:{listen_port}")
    print(f"[Live2D-Ai] Connect Electron frontend to ws://<PC_IP>:{listen_port}")
    await uvicorn_server.serve()


if __name__ == "__main__":
    asyncio.run(main())
