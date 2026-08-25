"""live2d.perform_action 原生工具单元测试（严格白名单契约）。

契约：
- 唯一暴露 live2d.perform_action；action 只能取当前安全头/脸动作目录；
- 非法动作拒绝：返回 ok=False + 可见日志 + allowed_actions，绝不近似替换；
- strength 缺省 2、clamp 到 1~3；reason 截断；
- 统计（calls/accepted/rejected/last_action/last_error）随调用累积；
- 插件 enable/disable 实时切换工具可用性。
"""

import asyncio

import pytest

from open_llm_vtuber.live2d_action import (
    LIVE2D_ACTION_TOOL_NAME,
    Live2dActionTool,
    build_live2d_action_tool_schema,
    get_default_tool,
    native_tools_enabled,
    reset_default_tool,
)
from open_llm_vtuber.motion_catalog import SAFE_MOTION_IDS


@pytest.fixture(autouse=True)
def _fresh_default_tool():
    """每个用例独占默认单例，避免统计串扰。"""
    reset_default_tool()
    yield
    reset_default_tool()


def _run(coro):
    return asyncio.run(coro)


def test_schema_shape_and_whitelist_enum():
    schema = build_live2d_action_tool_schema()
    assert schema["type"] == "function"
    fn = schema["function"]
    assert fn["name"] == LIVE2D_ACTION_TOOL_NAME
    params = fn["parameters"]
    assert params["required"] == ["action"]
    action_prop = params["properties"]["action"]
    assert set(action_prop["enum"]) == set(SAFE_MOTION_IDS)
    # idle 仅 release，绝不能进入可触发白名单
    assert "idle" not in action_prop["enum"]
    strength = params["properties"]["strength"]
    assert strength["minimum"] == 1 and strength["maximum"] == 3


def test_execute_accepts_legal_action_and_records_stats():
    tool = Live2dActionTool()
    result = _run(tool.execute({"action": "Nod", "strength": 3, "reason": "同意"}))
    assert result["ok"] is True
    assert result["action"] == "nod"
    assert result["strength"] == 3
    assert result["bound"] == "next_sentence"

    stats = tool.stats()
    assert stats["calls"] == 1
    assert stats["accepted"] == 1
    assert stats["rejected"] == 0
    assert stats["last_action"] == "nod"
    assert stats["last_error"] is None


@pytest.mark.parametrize("bad", ["fly", "wave", "idle", "", None, {"a": 1}])
def test_execute_rejects_illegal_actions(bad):
    tool = Live2dActionTool()
    result = _run(tool.execute({"action": bad}))
    assert result["ok"] is False
    assert "invalid action" in result["error"]
    assert set(result["allowed_actions"]) == set(SAFE_MOTION_IDS)

    stats = tool.stats()
    assert stats["calls"] == 1
    assert stats["accepted"] == 0
    assert stats["rejected"] == 1
    assert stats["last_error"]
    # 非法调用绝不产出动作
    assert stats["last_action"] is None


def test_strength_defaults_and_clamps():
    tool = Live2dActionTool()
    assert _run(tool.execute({"action": "nod"}))["strength"] == 2
    assert _run(tool.execute({"action": "nod", "strength": 0}))["strength"] == 1
    assert _run(tool.execute({"action": "nod", "strength": 99}))["strength"] == 3
    assert _run(tool.execute({"action": "nod", "strength": "2"}))["strength"] == 2


def test_capability_gated_action_rejected_without_model_params():
    """模型缺门槛参数时，能力门控动作即使合法 id 也必须被拒绝。"""
    tool = Live2dActionTool()
    # fail-closed：无参数集 → 只有基础六动作
    tool.configure_allowed_actions(None)
    result = _run(tool.execute({"action": "pout"}))
    assert result["ok"] is False

    accepted = _run(tool.execute({"action": "nod"}))
    assert accepted["ok"] is True
    # enum 与执行白名单同源收窄，提示词与目录永不漂移
    schema_enum = tool.openai_schema["function"]["parameters"]["properties"]["action"][
        "enum"
    ]
    assert "pout" not in schema_enum
    assert "nod" in schema_enum


def test_disabled_tool_reports_unavailable_but_still_validates():
    """available=False 时 agent 循环不会提供工具；execute 本身保持确定性。"""
    tool = Live2dActionTool()
    assert tool.available() is True
    tool.set_enabled(False)
    assert tool.available() is False
    tool.set_enabled(True)
    assert tool.available() is True


def test_singleton_shares_instance():
    assert get_default_tool() is get_default_tool()
    assert get_default_tool().name == LIVE2D_ACTION_TOOL_NAME


def test_native_tools_env_switch(monkeypatch):
    monkeypatch.delenv("LIVE2DAI_NATIVE_TOOLS", raising=False)
    assert native_tools_enabled() is True
    monkeypatch.setenv("LIVE2DAI_NATIVE_TOOLS", "off")
    assert native_tools_enabled() is False


def test_reason_truncated():
    tool = Live2dActionTool()
    long_reason = "啊" * 500
    _run(tool.execute({"action": "tilt", "reason": long_reason}))
    assert len(tool.stats()["last_reason"]) == 200
