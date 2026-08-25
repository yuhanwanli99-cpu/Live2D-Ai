"""无 Key / 无网络地基测试。

这些测试只检查本地文件、配置一致性、目录结构，不调用任何云端 API。
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
SHARED = ROOT / "shared"
ANDROID_ASSETS = ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets"
PC = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber"


def test_shared_core_files_exist() -> None:
    for rel in [
        "persona.yaml",
        "model_registry.json",
        "mcp_tools.json",
        "emotion-protocol.md",
        "model-adapter/bai.adapter.json",
    ]:
        assert (SHARED / rel).is_file(), f"missing shared/{rel}"


def test_persona_yaml_parse() -> None:
    yaml = pytest.importorskip("yaml")
    data = yaml.safe_load((SHARED / "persona.yaml").read_text(encoding="utf-8"))
    assert data["name"] == "白 (Bai)"
    assert data["system_prompt"]


def test_model_registry_valid_json() -> None:
    data = json.loads((SHARED / "model_registry.json").read_text(encoding="utf-8"))
    assert data["models"]
    assert sum(1 for m in data["models"] if m.get("default")) == 1


def test_android_assets_match_shared() -> None:
    for name in ["persona.yaml", "mcp_tools.json"]:
        src = SHARED / name
        dst = ANDROID_ASSETS / name
        assert src.read_bytes() == dst.read_bytes(), f"Android assets/{name} drifted from shared/{name}"


def test_pc_conf_loads_shared_persona() -> None:
    conf = PC / "conf.yaml"
    text = conf.read_text(encoding="utf-8")
    assert "load_persona_from_file" in text
    assert "shared/persona.yaml" in text


def test_plugin_sdk_skeleton_exists() -> None:
    sdk = ROOT / "Live2D-Ai-Android" / "plugin-sdk"
    for rel in [
        "build.gradle.kts",
        "src/main/java/com/live2d/ai/pluginsdk/Live2DAiPlugin.kt",
        "src/main/java/com/live2d/ai/pluginsdk/PluginHost.kt",
        "src/main/java/com/live2d/ai/pluginsdk/ChatHook.kt",
        "src/main/java/com/live2d/ai/pluginsdk/ToolDefinition.kt",
    ]:
        assert (sdk / rel).is_file(), f"missing plugin-sdk/{rel}"


def test_root_no_build_clutter() -> None:
    # build/ 和 cache/ 是打包/运行时/测试会生成的目录，交给 clean.sh 清理，不判失败。
    bad = [p for p in ROOT.iterdir() if p.name in {"s.log", "s2.log", "s_dl.log"}]
    assert not bad, f"root contains generated clutter: {[p.name for p in bad]}"
