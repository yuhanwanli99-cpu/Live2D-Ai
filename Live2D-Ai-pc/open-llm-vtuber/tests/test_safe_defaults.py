"""Safe-default configuration tests for PC-only Preview.

Ensure the shipped default templates:
- bind to 127.0.0.1
- keep developer mode off
- disable ASR/VAD by default
- do not default to melo_tts (uses Edge TTS instead)
- contain no literal API keys
- can be parsed by the Pydantic Config model
"""

import os
import sys
from pathlib import Path

import pytest
import yaml

_PC_ROOT = Path(__file__).resolve().parents[1]
_PC_SRC = _PC_ROOT / "src"
if str(_PC_SRC) not in sys.path:
    sys.path.insert(0, str(_PC_SRC))

from open_llm_vtuber.config_manager.main import Config  # noqa: E402


def _load_yaml(name: str) -> dict:
    path = _PC_ROOT / name
    with open(path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f)


def _inject_persona(config_dict: dict) -> dict:
    """Mirror run_server.py persona loading so Config validates."""
    persona_path = (_PC_ROOT / ".." / ".." / "shared" / "persona.yaml").resolve()
    persona = ""
    if persona_path.exists():
        data = yaml.safe_load(persona_path.read_text(encoding="utf-8")) or {}
        persona = data.get("system_prompt", "")
    cc = config_dict.setdefault("character_config", {})
    cc["persona_prompt"] = persona or cc.get("persona_prompt", "Preview persona")
    return config_dict


@pytest.mark.parametrize(
    "template",
    [
        "config_templates/conf.default.yaml",
        "config_templates/conf.ZH.default.yaml",
        "conf.yaml.template",
    ],
)
def test_safe_default_values(template: str):
    cfg = _load_yaml(template)
    sys_cfg = cfg["system_config"]
    assert sys_cfg["host"] == "127.0.0.1"
    assert sys_cfg["developer_mode"] is False

    asr_cfg = cfg["character_config"]["asr_config"]
    assert asr_cfg["asr_model"] == "disabled"

    tts_cfg = cfg["character_config"]["tts_config"]
    assert tts_cfg["tts_model"] == "edge_tts"
    assert tts_cfg["tts_model"] != "melo_tts"
    assert "edge_tts" in tts_cfg

    vad_cfg = cfg["character_config"]["vad_config"]
    assert vad_cfg["vad_model"] is None


@pytest.mark.parametrize(
    "template",
    [
        "config_templates/conf.default.yaml",
        "config_templates/conf.ZH.default.yaml",
        "conf.yaml.template",
    ],
)
def test_no_literal_api_keys(template: str):
    text = (_PC_ROOT / template).read_text(encoding="utf-8")
    # Disallow hard-coded OpenAI-style keys (placeholders like ${...} are fine).
    import re

    matches = re.findall(r"(?<![A-Za-z0-9_-])sk-[A-Za-z0-9_-]{20,}", text)
    assert not matches, f"{template} contains literal API key-looking strings: {matches}"


@pytest.mark.parametrize(
    "template",
    [
        "config_templates/conf.default.yaml",
        "config_templates/conf.ZH.default.yaml",
        "conf.yaml.template",
    ],
)
def test_default_config_parses(template: str):
    cfg = _load_yaml(template)
    cfg = _inject_persona(cfg)
    Config(**cfg)
