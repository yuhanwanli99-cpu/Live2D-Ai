"""Launcher script for Open-LLM-VTuber server (Live2Dai slimmed, no-gateway).
- Resolves shared/ paths to F:\Live2Dai\shared\ (cross-platform config)
- Pre-loads persona from shared/persona.yaml into persona_prompt
- All API calls go directly to DeepSeek / GLM (no gateway)
"""

import sys
import os
import asyncio
import uvicorn
import yaml

_project_root = os.path.dirname(os.path.abspath(__file__))
os.chdir(_project_root)

sys.path.insert(0, os.path.join(_project_root, "src"))
sys.path.insert(0, _project_root)

from open_llm_vtuber.server import WebSocketServer
from open_llm_vtuber.config_manager.main import Config


def _resolve_shared_path(config_path: str) -> str:
    """Resolve 'shared/xxx' relative to the actual F:/Live2Dai/shared/ directory.
    Two levels up from open-llm-vtuber/ -> F:/Live2Dai"""
    if config_path.startswith("shared/"):
        base = os.path.dirname(os.path.dirname(_project_root))  # F:\Live2Dai
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
            print(f"[Live2Dai] Loaded persona from {resolved}")
            if overrides:
                print(f"[Live2Dai] Persona model overrides: {overrides}")
        return prompt, overrides
    except Exception as e:
        print(f"[WARN] Failed to load persona from {resolved}: {e}")
        return cc.get("persona_prompt", ""), empty_overrides


async def main():
    print("[Live2Dai] Starting Open-LLM-VTuber server (direct-API mode, no gateway)...")
    print(f"[Live2Dai] Working directory: {_project_root}")

    with open("conf.yaml", "r", encoding="utf-8") as f:
        config_dict = yaml.safe_load(f)

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

    # Apply model overrides from persona.yaml to deepseek_llm provider
    if model_overrides:
        llm_configs = (
            config_dict.setdefault("character_config", {})
            .setdefault("agent_config", {})
            .setdefault("llm_configs", {})
        )
        deepseek_cfg = llm_configs.get("deepseek_llm", {})
        if model_overrides.get("model"):
            deepseek_cfg["model"] = model_overrides["model"]
            print(f"[Live2Dai] LLM model set to: {model_overrides['model']}")
        if model_overrides.get("temperature") is not None:
            deepseek_cfg["temperature"] = model_overrides["temperature"]
            print(
                f"[Live2Dai] LLM temperature set to: {model_overrides['temperature']}"
            )

    # Resolve MCP server config path
    if "mcp_servers" in config_dict:
        config_dict["mcp_servers"] = _resolve_shared_path(config_dict["mcp_servers"])

    config = Config(**config_dict)
    server = WebSocketServer(config)
    await server.initialize()

    uvicorn_config = uvicorn.Config(
        server.app,
        host="0.0.0.0",
        port=12393,
        log_level="info",
    )
    uvicorn_server = uvicorn.Server(uvicorn_config)
    print("[Live2Dai] Server listening on http://0.0.0.0:12393")
    print("[Live2Dai] Connect Electron frontend to ws://<PC_IP>:12393")
    await uvicorn_server.serve()


if __name__ == "__main__":
    asyncio.run(main())
