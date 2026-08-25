"""PC 端 JSON 配置导入/导出接口（config_transfer.py）单元测试。

全部使用 tmp_path，不触碰仓库真实 conf.yaml / shared/persona.yaml。
"""

import copy
import os
import sys

import pytest
import yaml
from fastapi import FastAPI
from fastapi.testclient import TestClient

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.config_transfer import init_config_transfer_routes  # noqa: E402


MINIMAL_CONF = {
    "system_config": {
        "conf_version": "v1.2.1",
        "host": "127.0.0.1",
        "port": 12393,
        "config_alts_dir": "characters",
        "tool_prompts": {},
    },
    "character_config": {
        "conf_name": "test",
        "conf_uid": "test_001",
        "live2d_model_name": "test",
        "character_name": "测试",
        "persona_prompt": "你是测试助手",
        "agent_config": {
            "conversation_agent_choice": "basic_memory_agent",
            "agent_settings": {
                "basic_memory_agent": {
                    "llm_provider": "deepseek_llm",
                    "segment_method": "priority",
                }
            },
            "llm_configs": {
                "deepseek_llm": {
                    "base_url": "https://api.deepseek.com/v1",
                    "llm_api_key": "${DEEPSEEK_API_KEY}",
                    "model": "deepseek-v4-flash",
                    "temperature": 0.8,
                }
            },
        },
        "asr_config": {"asr_model": "disabled"},
        "tts_config": {
            "tts_model": "edge_tts",
            "edge_tts": {"voice": "zh-CN-XiaoyiNeural"},
        },
        "vad_config": {},
        "tts_preprocessor_config": {
            "remove_special_char": True,
            "translator_config": {
                "translate_audio": False,
                "translate_provider": "deeplx",
            },
        },
    },
}

PERSONA = {
    "name": "白 (Bai)",
    "role": "gentle cute anime girl assistant companion",
    "system_prompt": "你是白，一位温柔可爱的测试助手。",
    "model": "deepseek-v4-flash",
    "temperature": 1,
    "max_tokens": 300,
}


def _dump(data: dict) -> str:
    return yaml.safe_dump(data, allow_unicode=True, sort_keys=False)


def _make_client(tmp_path, conf=None, persona=PERSONA):
    """在 tmp_path 下构造 repo_root/{conf.yaml, shared/persona.yaml} 并返回 TestClient。"""
    root = tmp_path / "repo"
    shared = root / "shared"
    shared.mkdir(parents=True)
    conf_path = root / "conf.yaml"
    conf_path.write_text(_dump(conf if conf is not None else MINIMAL_CONF), encoding="utf-8")
    persona_path = shared / "persona.yaml"
    persona_path.write_text(_dump(persona), encoding="utf-8")
    app = FastAPI()
    app.include_router(
        init_config_transfer_routes(config_path=str(conf_path), repo_root=str(root))
    )
    return TestClient(app), conf_path, persona_path


def test_export_includes_conf_and_persona(tmp_path):
    client, conf_path, persona_path = _make_client(tmp_path)

    resp = client.get("/api/config/export")
    assert resp.status_code == 200
    body = resp.json()
    assert body["ok"] is True
    assert body["format"] == "live2dai-config"
    assert body["version"] == 1
    assert "persona_prompt" in body["conf_yaml"]
    assert body["persona"]["system_prompt"] == PERSONA["system_prompt"]
    assert body["persona"]["model"] == PERSONA["model"]


def test_import_valid_conf_persists(tmp_path):
    client, conf_path, persona_path = _make_client(tmp_path)

    new_conf = copy.deepcopy(MINIMAL_CONF)
    new_conf["character_config"]["character_name"] = "导入角色"
    before_persona = persona_path.read_text(encoding="utf-8")

    resp = client.post("/api/config/import", json={"conf_yaml": _dump(new_conf)})
    assert resp.status_code == 200
    body = resp.json()
    assert body["ok"] is True
    assert body["persisted"] is True
    assert body["message"] == "已导入"

    written = yaml.safe_load(conf_path.read_text(encoding="utf-8"))
    assert written["character_config"]["character_name"] == "导入角色"
    # 未提供 persona 时不得改写 persona.yaml
    assert persona_path.read_text(encoding="utf-8") == before_persona


def test_import_invalid_top_level_list_returns_400_and_keeps_file(tmp_path):
    client, conf_path, _ = _make_client(tmp_path)
    before = conf_path.read_text(encoding="utf-8")

    resp = client.post("/api/config/import", json={"conf_yaml": "- a\n- b\n"})
    assert resp.status_code == 400
    assert resp.json()["ok"] is False
    assert conf_path.read_text(encoding="utf-8") == before


def test_import_invalid_schema_returns_400_and_keeps_file(tmp_path):
    client, conf_path, _ = _make_client(tmp_path)
    before = conf_path.read_text(encoding="utf-8")

    resp = client.post("/api/config/import", json={"conf_yaml": "foo: bar\n"})
    assert resp.status_code == 400
    assert resp.json()["ok"] is False
    assert conf_path.read_text(encoding="utf-8") == before


def test_import_with_persona_writes_persona(tmp_path):
    client, conf_path, persona_path = _make_client(tmp_path)

    # 去掉 conf 里的 persona_prompt，验证 persona.system_prompt 注入后才通过校验
    conf_no_prompt = copy.deepcopy(MINIMAL_CONF)
    del conf_no_prompt["character_config"]["persona_prompt"]

    new_persona = {
        "system_prompt": "新导入的系统提示词",
        "model": "deepseek-v4-pro",
        "temperature": 0.3,
        "max_tokens": 150,
    }

    resp = client.post(
        "/api/config/import",
        json={"conf_yaml": _dump(conf_no_prompt), "persona": new_persona},
    )
    assert resp.status_code == 200
    assert resp.json()["ok"] is True

    written_persona = yaml.safe_load(persona_path.read_text(encoding="utf-8"))
    assert written_persona["system_prompt"] == "新导入的系统提示词"
    assert written_persona["model"] == "deepseek-v4-pro"
    assert written_persona["temperature"] == 0.3
    assert written_persona["max_tokens"] == 150
