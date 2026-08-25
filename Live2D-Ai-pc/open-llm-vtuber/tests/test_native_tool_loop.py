"""主对话 LLM 原生工具循环测试（mock OpenAI 兼容 LLM，不发网络请求）。

契约：
- mock LLM 的 tool_call → live2d.perform_action 执行 → live2d_action 事件产出，
  动作绑定下一句；工具结果不朗读（不出现在任何 yield 的文本里）、
  不进入记忆正文；
- 非法动作：拒绝结果回灌模型，无 live2d_action 事件，统计可见；
- provider 报告 __API_NOT_SUPPORT_TOOLS__ → 立即无 tools 重试拿正文 +
  永久降级（后续轮次不再提供工具）；
- 非 OpenAI 兼容 provider 走原简单路径，不提供工具。
"""

import asyncio
import json
from collections.abc import AsyncIterator
from typing import Any

from openai import NOT_GIVEN

from open_llm_vtuber.agent.agents.basic_memory_agent import BasicMemoryAgent
from open_llm_vtuber.agent.input_types import BatchInput, TextData, TextSource
from open_llm_vtuber.agent.output_types import SentenceOutput
from open_llm_vtuber.agent.stateless_llm.openai_compatible_llm import (
    AsyncLLM as OpenAICompatibleAsyncLLM,
)
from open_llm_vtuber.live2d_action import LIVE2D_ACTION_TOOL_NAME, Live2dActionTool
from open_llm_vtuber.live2d_model import Live2dModel
from open_llm_vtuber.mcpp.types import ToolCallFunctionObject, ToolCallObject


def _run(coro):
    return asyncio.run(coro)


def _visible_text(items) -> str:
    """提取管线最终可见文本（SentenceOutput 的 display/tts 文本）。"""
    parts: list[str] = []
    for item in items:
        if isinstance(item, SentenceOutput):
            parts.append(item.display_text.text or "")
            parts.append(item.tts_text or "")
        elif isinstance(item, str):
            parts.append(item)
    return "".join(parts)


def _tts_config():
    """以仓库 conf.yaml 的合法 tts_preprocessor 配置构造（tts_filter 需要）。"""
    import os

    import yaml

    from open_llm_vtuber.config_manager import TTSPreprocessorConfig

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    with open(os.path.join(root, "conf.yaml"), encoding="utf-8") as f:
        data = yaml.safe_load(f)
    return TTSPreprocessorConfig(**data["character_config"]["tts_preprocessor_config"])


class ScriptedOpenAILLM(OpenAICompatibleAsyncLLM):
    """脚本化 mock：按预设事件序列应答，记录每次调用的 messages/tools。"""

    def __init__(self, scripts: list[list[Any]]):
        # 不调用父类 __init__：避免创建真实 AsyncOpenAI 客户端
        self.scripts = list(scripts)
        self.calls: list[dict[str, Any]] = []
        self.model = "mock-model"
        self.base_url = "mock://local"
        self.support_tools = True

    async def chat_completion(
        self,
        messages,
        system=None,
        tools=NOT_GIVEN,
    ) -> AsyncIterator[Any]:
        self.calls.append(
            {
                "messages": [dict(m) for m in messages],
                "system": system,
                "tools": tools,
            }
        )
        events = self.scripts.pop(0) if self.scripts else ["（脚本耗尽）"]
        for event in events:
            yield event


class PlainTextLLM:
    """非 OpenAI 兼容 provider：只产文本，不支持 tools 参数语义。"""

    def __init__(self, chunks: list[str]):
        self.chunks = chunks
        self.calls: list[dict[str, Any]] = []

    async def chat_completion(self, messages, system=None, tools=None):
        self.calls.append({"messages": messages, "system": system, "tools": tools})
        for chunk in self.chunks:
            yield chunk


def _tool_call(name: str, arguments: str, call_id: str = "call_1") -> ToolCallObject:
    return ToolCallObject(
        id=call_id,
        type="function",
        index=0,
        function=ToolCallFunctionObject(name=name, arguments=arguments),
    )


def _agent(llm, live2d_model=None) -> BasicMemoryAgent:
    from open_llm_vtuber.live2d_model import Live2dModel

    return BasicMemoryAgent(
        llm=llm,
        system="你是测试助手",
        live2d_model=live2d_model or Live2dModel("bai"),
        tts_preprocessor_config=_tts_config(),
        use_mcpp=False,
        live2d_tool=Live2dActionTool(),
    )


def _batch_input(text: str = "你好呀") -> BatchInput:
    return BatchInput(
        texts=[TextData(source=TextSource.INPUT, content=text, from_name="tester")]
    )


def _collect(agent: BasicMemoryAgent, batch: BatchInput):
    async def consume():
        items = []
        async for item in agent.chat(batch):
            items.append(item)
        return items

    return _run(consume())


class TestNativeToolLoopHappyPath:
    def test_tool_call_binds_action_and_text_reaches_memory(self):
        llm = ScriptedOpenAILLM(
            [
                # 第 1 轮：先说半句 + 发起合法动作调用
                [
                    "好呀！",
                    [
                        _tool_call(
                            LIVE2D_ACTION_TOOL_NAME,
                            '{"action": "nod", "strength": 3, "reason": "开心"}',
                        )
                    ],
                ],
                # 第 2 轮：拿到工具结果后继续说话收尾
                ["那我们出发吧。"],
            ]
        )
        agent = _agent(llm)
        items = _collect(agent, _batch_input())

        # 1) 动作事件恰好一次，字段完整
        action_events = [
            i for i in items if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        assert len(action_events) == 1
        evt = action_events[0]
        assert evt["action"] == "nod"
        assert evt["strength"] == 3
        assert evt["source"] == "llm_tool"

        # 2) 前端可见状态事件存在（done）
        status = [
            i
            for i in items
            if isinstance(i, dict) and i.get("type") == "tool_call_status"
        ]
        assert any(
            s.get("status") == "done" and s.get("action") == "nod" for s in status
        )

        # 3) 工具结果绝不朗读：所有 yield 出的纯文本不含工具结果 JSON
        texts = [i for i in items if isinstance(i, str)]
        joined = "".join(texts)
        assert '"ok"' not in joined
        assert "next_sentence" not in joined

        # 4) 记忆零污染：只有 user 与两段 assistant 可见正文；绝无 tool/工具 JSON
        roles = [m["role"] for m in agent._memory]
        assert roles.count("user") == 1
        assert all(role in ("user", "assistant") for role in roles)
        for message in agent._memory:
            content = message["content"]
            assert '"ok"' not in content
            assert "tool" not in json.dumps(
                message, ensure_ascii=False
            ).lower() or message["role"] in ("user", "assistant")
        memory_text = "".join(m["content"] for m in agent._memory)
        assert "好呀！" in memory_text
        assert "那我们出发吧。" in memory_text

        # 5) API 会话列表包含协议消息（assistant.tool_calls + tool 结果），
        #    但那只用于本轮对话，不等于记忆。
        second_round_messages = llm.calls[1]["messages"]
        assert any(
            m.get("role") == "assistant" and m.get("tool_calls")
            for m in second_round_messages
        )
        tool_msg = [m for m in second_round_messages if m.get("role") == "tool"]
        assert len(tool_msg) == 1
        assert json.loads(tool_msg[0]["content"])["ok"] is True
        assert tool_msg[0]["tool_call_id"] == "call_1"

        # 6) 工具统计
        tool = agent._live2d_tool
        stats = tool.stats()
        assert stats["accepted"] == 1 and stats["rejected"] == 0

    def test_first_call_offers_tool_schema(self):
        llm = ScriptedOpenAILLM([["嗯嗯。"]])
        agent = _agent(llm)
        _collect(agent, _batch_input())
        tools = llm.calls[0]["tools"]
        assert tools and tools[0]["function"]["name"] == LIVE2D_ACTION_TOOL_NAME
        enum = tools[0]["function"]["parameters"]["properties"]["action"]["enum"]
        assert "nod" in enum


class TestRejections:
    def test_illegal_action_rejected_no_binding(self):
        llm = ScriptedOpenAILLM(
            [
                [
                    "看我的！",
                    [_tool_call(LIVE2D_ACTION_TOOL_NAME, '{"action": "backflip"}')],
                ],
                ["好吧，那算了。"],
            ]
        )
        agent = _agent(llm)
        items = _collect(agent, _batch_input())

        assert not [
            i for i in items if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        rejected = [
            i
            for i in items
            if isinstance(i, dict)
            and i.get("type") == "tool_call_status"
            and i.get("status") == "rejected"
        ]
        assert rejected, "拒绝必须有可见状态事件"
        stats = agent._live2d_tool.stats()
        assert stats["rejected"] == 1 and stats["accepted"] == 0
        # 拒绝结果回灌给模型（协议层），但记忆里没有
        tool_msgs = [m for m in llm.calls[1]["messages"] if m.get("role") == "tool"]
        assert tool_msgs and json.loads(tool_msgs[0]["content"])["ok"] is False

    def test_unknown_tool_name_rejected(self):
        llm = ScriptedOpenAILLM(
            [
                ["", [_tool_call("system.shutdown", "{}")]],
                ["继续聊天。"],
            ]
        )
        agent = _agent(llm)
        items = _collect(agent, _batch_input())
        assert not [
            i for i in items if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        assert any(
            isinstance(i, dict)
            and i.get("type") == "tool_call_status"
            and i.get("status") == "rejected"
            and "unknown tool" in str(i.get("error"))
            for i in items
        )

    def test_broken_arguments_json_treated_as_invalid(self):
        llm = ScriptedOpenAILLM(
            [
                ["", [_tool_call(LIVE2D_ACTION_TOOL_NAME, "{broken json")]],
                ["嗯。"],
            ]
        )
        agent = _agent(llm)
        items = _collect(agent, _batch_input())
        assert not [
            i for i in items if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        assert agent._live2d_tool.stats()["rejected"] == 1


class TestDegradation:
    def test_unsupported_provider_degrades_and_recovers_text(self):
        llm = ScriptedOpenAILLM(
            [
                ["__API_NOT_SUPPORT_TOOLS__"],  # 带 tools 的第一次请求失败
                ["你好呀，我在呢！"],  # 无 tools 重试成功
            ]
        )
        agent = _agent(llm)
        items = _collect(agent, _batch_input())
        visible = _visible_text(items)
        assert "你好呀" in visible and "我在呢" in visible
        # 哨兵字符串绝不外泄为可见文本
        assert "__API_NOT_SUPPORT_TOOLS__" not in visible

        # 第一次带 tools、重试不带 tools
        assert llm.calls[0]["tools"] is not NOT_GIVEN
        assert llm.calls[1]["tools"] is NOT_GIVEN or llm.calls[1]["tools"] is None
        assert agent._native_tools_degraded is True

        # 后续轮次：不再提供工具（直接走简单路径，只调用一次且无 tools）
        _collect(agent, _batch_input("再来一句"))
        assert len(llm.calls) == 3  # 前两轮 + 本轮 1 次
        assert llm.calls[2]["tools"] is NOT_GIVEN

    def test_plain_provider_never_gets_tools(self):
        llm = PlainTextLLM(["普通回复"])
        agent = _agent(llm)
        items = _collect(agent, _batch_input())
        assert "普通回复" in _visible_text(items)
        assert llm.calls[0]["tools"] is None
        assert agent._native_tools_degraded is False

    def test_tool_round_limit_stops_runaway_loops(self):
        """模型连续发起工具调用时，轮次上限必须终止循环且正文照常产出。"""
        call = [_tool_call(LIVE2D_ACTION_TOOL_NAME, '{"action": "nod"}')]
        scripts = [["", call] for _ in range(4)] + [["好吧，不点了。", call]]
        llm = ScriptedOpenAILLM(scripts)
        agent = _agent(llm)
        items = _collect(agent, _batch_input())

        # 前 4 轮各接受一次动作；第 5 轮到达上限 → 调用被丢弃、正文照常收尾
        stats = agent._live2d_tool.stats()
        assert stats["accepted"] == 4
        assert any(
            isinstance(i, dict)
            and i.get("type") == "tool_call_status"
            and i.get("status") == "done"
            for i in items
        )
        sentences = [i for i in items if isinstance(i, SentenceOutput)]
        assert sentences, "循环结束后正文仍要正常产出"
        assert "好吧，不点了" in "".join(s.display_text.text for s in sentences)
        # 超限后的工具调用不再产出绑定事件
        action_events = [
            i for i in items if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        assert len(action_events) == 4


class TestDisabledTool:
    def test_disabled_plugin_falls_back_to_simple_path(self):
        llm = ScriptedOpenAILLM([["直接回答"]])
        tool = Live2dActionTool()
        tool.set_enabled(False)
        agent = BasicMemoryAgent(
            llm=llm,
            system="sys",
            live2d_model=Live2dModel("bai"),
            tts_preprocessor_config=_tts_config(),
            use_mcpp=False,
            live2d_tool=tool,
        )
        items = _collect(agent, _batch_input())
        assert "直接回答" in _visible_text(items)
        # 未提供工具
        assert llm.calls[0]["tools"] is NOT_GIVEN
