"""主 LLM 动作工具使用策略 + 本地显式命令（单/组合）测试。

任务定版契约：
- 启用工具时向主 LLM 注入短系统指导：普通对话可克制地用与语义匹配的动作
  增强表现（每句最多 1 个，避免每句必调）；多步骤用户命令按顺序多次调用；
  禁止只在文字里描述动作却不调用工具；指导只随工具注入、不污染人设；
- 本地显式命令识别扩展到 look_up/down、glance_left/right、smile/frown/
  pout/wry_mouth/deep_breath；组合句「向下看然后向上看」「摇头 然后点头」
  返回有序动作列表（不依赖 LLM 意愿）；
- 调度优先级：本地显式序列(100) > 主 LLM 工具(90) > 导演/括注(60/50)，
  且用户命令队列先消费，同句不再叠加工具事件；
- 多轮工具支持不因一次成功被关闭；只有 provider 明确不支持 tools 才永久降级；
- LLM 错误轮绝不触发额外多段续聊。
"""

import asyncio
import json
from collections.abc import AsyncIterator
from types import SimpleNamespace
from typing import Any

from open_llm_vtuber.agent.agents.basic_memory_agent import BasicMemoryAgent
from open_llm_vtuber.agent.input_types import BatchInput, TextData, TextSource
from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput
from open_llm_vtuber.agent.stateless_llm.openai_compatible_llm import (
    AsyncLLM as OpenAICompatibleAsyncLLM,
)
from open_llm_vtuber.conversations.single_conversation import (
    _USER_COMMAND_PRIORITY,
    _apply_director,
    process_single_conversation,
)
from open_llm_vtuber.director import (
    detect_user_motion_command,
    detect_user_motion_sequence,
)
from open_llm_vtuber.live2d_action import (
    LIVE2D_ACTION_TOOL_API_NAME,
    LIVE2D_ACTION_TOOL_GUIDANCE,
    LIVE2D_ACTION_TOOL_GUIDANCE_MARKER,
    Live2dActionTool,
    build_guided_system_prompt,
    build_live2d_action_tool_schema,
)
from open_llm_vtuber.mcpp.types import ToolCallFunctionObject, ToolCallObject
from open_llm_vtuber.motion_catalog import SAFE_MOTION_IDS
from openai import NOT_GIVEN


def _run(coro):
    return asyncio.run(coro)


# ------------------------------------------------------------------ 工具指导


class TestToolGuidance:
    def test_guidance_content_covers_restraint_and_sequences(self):
        # 克制约束 + 多步骤按序调用 + 禁止裸文本动作描写，三要素齐全
        assert "每句话最多调用 1 个动作" in LIVE2D_ACTION_TOOL_GUIDANCE
        assert "不要每一句话都调用" in LIVE2D_ACTION_TOOL_GUIDANCE
        assert "按顺序逐个调用" in LIVE2D_ACTION_TOOL_GUIDANCE
        assert "禁止只在回复正文里用文字描述动作" in LIVE2D_ACTION_TOOL_GUIDANCE

    def test_build_guided_system_prompt_keeps_persona_prefix(self):
        guided = build_guided_system_prompt("你是一只桌宠猫娘")
        assert guided.startswith("你是一只桌宠猫娘")
        assert LIVE2D_ACTION_TOOL_GUIDANCE_MARKER in guided

    def test_build_guided_system_prompt_without_base(self):
        guided = build_guided_system_prompt("")
        assert guided == LIVE2D_ACTION_TOOL_GUIDANCE
        assert build_guided_system_prompt(None) == LIVE2D_ACTION_TOOL_GUIDANCE

    def test_tool_schema_description_matches_policy(self):
        schema = build_live2d_action_tool_schema()
        desc = schema["function"]["description"]
        assert "每句话最多 1 个动作" in desc
        assert "按顺序逐个调用" in desc
        assert "绝不要在正文里用文字描述动作来代替调用本工具" in desc


# ------------------------------------------------------- 显式命令：单 + 组合


class TestExplicitCommandDetection:
    def test_capability_gated_commands_recognized(self):
        assert detect_user_motion_command("向上看一下") == "look_up"
        assert detect_user_motion_command("往下看") == "look_down"
        assert detect_user_motion_command("看看左边") == "glance_left"
        assert detect_user_motion_command("向右看") == "glance_right"
        assert detect_user_motion_command("笑一个") == "smile"
        assert detect_user_motion_command("皱一下眉") == "frown"
        assert detect_user_motion_command("嘟嘴") == "pout"
        assert detect_user_motion_command("歪一下嘴") == "wry_mouth"
        assert detect_user_motion_command("深呼吸") == "deep_breath"

    def test_all_detected_ids_are_in_safe_catalog(self):
        samples = [
            "点点头", "摇头", "歪头", "张望", "向上看", "向下看",
            "看看左边", "看看右边", "微笑一下", "皱眉", "嘟嘴",
            "歪嘴", "深呼吸",
        ]
        for text in samples:
            command = detect_user_motion_command(text)
            assert command in SAFE_MOTION_IDS, f"{text} -> {command}"

    def test_combo_sentence_returns_ordered_list(self):
        assert detect_user_motion_sequence("向下看然后向上看") == [
            "look_down",
            "look_up",
        ]
        assert detect_user_motion_sequence("摇头 然后点头") == ["shake_no", "nod"]
        assert detect_user_motion_sequence("先点头再摇摇头") == ["nod", "shake_no"]

    def test_combo_order_follows_user_wording_not_dict_order(self):
        # 回归：「摇头 然后点头」曾被字典序先扫 nod 误判成 nod
        assert detect_user_motion_sequence("摇头 然后点头")[0] == "shake_no"

    def test_single_command_degrades_to_one_element_list(self):
        assert detect_user_motion_sequence("点点头") == ["nod"]
        assert detect_user_motion_sequence("你好呀") == []
        assert detect_user_motion_sequence("") == []
        assert detect_user_motion_sequence(None) == []

    def test_guard_blocks_whole_sentence_for_sequence(self):
        assert detect_user_motion_sequence("为什么向下看然后向上看") == []
        assert detect_user_motion_sequence("别摇头然后点头") == []
        assert detect_user_motion_command("为什么看看左边") is None

    def test_consecutive_duplicates_collapsed(self):
        assert detect_user_motion_sequence("点头 点头") == ["nod"]


# ------------------------------------------- 序列绑定与优先级（100 > 90）


def _sentence(text: str = "好呀。") -> SentenceOutput:
    return SentenceOutput(
        display_text=DisplayText(text=text),
        tts_text=text,
        actions=Actions(),
    )


def _llm_tool_event(action="nod", strength=2, reason="开心") -> dict:
    return {
        "type": "live2d_action",
        "action": action,
        "strength": strength,
        "reason": reason,
        "source": "llm_tool",
        "tool": "live2d.perform_action",
    }


class _Ctx:
    def __init__(self, director=None):
        self.director = director
        self.live2d_model = None


class TestSequenceBindingPriority:
    def test_multi_command_sequence_binds_timeline_at_100(self):
        out = _sentence()
        queue = ["look_down", "look_up"]
        llm_events = [_llm_tool_event(action="nod")]
        _run(
            _apply_director(
                _Ctx(director=None),
                out,
                user_text="向下看然后向上看",
                pending_llm_actions=llm_events,
                user_cmd_queue=queue,
            )
        )
        # 用户命令整队绑定本句时间线，priority=100、source=user_command
        frames = out._native_action_timeline
        assert [f["motion"] for f in frames] == ["look_down", "look_up"]
        assert all(f["source"] == "user_command" for f in frames)
        assert all(f["priority"] == _USER_COMMAND_PRIORITY for f in frames)
        # 队列已清空；同句不叠加 llm 工具事件（留给后续句子）
        assert queue == []
        assert len(llm_events) == 1

    def test_single_command_binds_one_shot_at_100_skipping_director(self):
        class _BoomDirector:
            async def analyze(self, text):
                raise AssertionError("user command must skip director")

        out = _sentence()
        queue = ["glance_left"]
        _run(
            _apply_director(
                _Ctx(director=_BoomDirector()),
                out,
                user_text="看看左边",
                pending_llm_actions=[],
                user_cmd_queue=queue,
            )
        )
        assert out.actions.motion == "glance_left"
        assert out.actions.source == "user_command"
        assert out.actions.priority == _USER_COMMAND_PRIORITY
        assert out.actions.motionStrength >= 2
        assert queue == []

    def test_llm_tool_event_deferred_to_next_sentence_at_90(self):
        first = _sentence("第一句")
        second = _sentence("第二句")
        llm_events = [_llm_tool_event(action="tilt")]
        queue = ["deep_breath"]
        ctx = _Ctx(director=None)
        _run(
            _apply_director(
                ctx, first, user_text="深呼吸",
                pending_llm_actions=llm_events, user_cmd_queue=queue,
            )
        )
        _run(
            _apply_director(
                ctx, second, user_text="深呼吸",
                pending_llm_actions=llm_events, user_cmd_queue=queue,
            )
        )
        # 第一句：用户命令(100)；第二句：延迟到此的工具事件(90)，未被丢弃
        assert first.actions.motion == "deep_breath"
        assert first.actions.priority == _USER_COMMAND_PRIORITY
        assert second.actions.motion == "tilt"
        assert second.actions.source == "llm_tool"
        assert second.actions.priority == 90
        assert llm_events == []


# ------------------------------------------- 多轮工具支持不被一次成功关闭


class ScriptedOpenAILLM(OpenAICompatibleAsyncLLM):
    """脚本化 mock：按预设事件序列应答，记录每次调用的 messages/tools/system。"""

    def __init__(self, scripts: list[list[Any]]):
        self.scripts = list(scripts)
        self.calls: list[dict[str, Any]] = []
        self.model = "mock-model"
        self.base_url = "mock://local"
        self.support_tools = True

    async def chat_completion(
        self, messages, system=None, tools=NOT_GIVEN
    ) -> AsyncIterator[Any]:
        self.calls.append({"messages": messages, "system": system, "tools": tools})
        events = self.scripts.pop(0) if self.scripts else ["（脚本耗尽）"]
        for event in events:
            yield event


def _tool_call(name: str, arguments: str, call_id: str = "call_1") -> ToolCallObject:
    return ToolCallObject(
        id=call_id,
        type="function",
        index=0,
        function=ToolCallFunctionObject(name=name, arguments=arguments),
    )


def _make_agent(llm) -> BasicMemoryAgent:
    import os

    import yaml
    from open_llm_vtuber.config_manager import TTSPreprocessorConfig
    from open_llm_vtuber.live2d_model import Live2dModel

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    with open(os.path.join(root, "conf.yaml"), encoding="utf-8") as f:
        data = yaml.safe_load(f)
    return BasicMemoryAgent(
        llm=llm,
        system="你是测试助手",
        live2d_model=Live2dModel("bai"),
        tts_preprocessor_config=TTSPreprocessorConfig(
            **data["character_config"]["tts_preprocessor_config"]
        ),
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


class TestMultiTurnToolPersistence:
    def test_two_consecutive_turns_both_call_tools_effectively(self):
        llm = ScriptedOpenAILLM(
            [
                # 第 1 轮：普通语义寒暄中模型选择调用一次动作
                [
                    "好呀！",
                    [_tool_call(LIVE2D_ACTION_TOOL_API_NAME, '{"action": "nod"}')],
                ],
                ["我们出发吧。"],
                # 第 2 轮：上一轮刚成功过 —— 工具必须仍然可用并可再次生效
                [
                    [
                        _tool_call(
                            LIVE2D_ACTION_TOOL_API_NAME,
                            '{"action": "tilt", "reason": "好奇"}',
                        )
                    ],
                    ["歪头看你。"],
                ],
            ]
        )
        agent = _make_agent(llm)

        items1 = _collect(agent, _batch_input("在吗"))
        items2 = _collect(agent, _batch_input("再做一个"))

        events = [
            i
            for i in (items1 + items2)
            if isinstance(i, dict) and i.get("type") == "live2d_action"
        ]
        assert [e["action"] for e in events] == ["nod", "tilt"]
        # 两轮均提供工具；成功后绝不关闭
        assert llm.calls[0]["tools"] is not NOT_GIVEN
        assert llm.calls[2]["tools"] is not NOT_GIVEN
        assert agent._native_tools_degraded is False
        stats = agent._live2d_tool.stats()
        assert stats["accepted"] == 2 and stats["rejected"] == 0

    def test_normal_chat_system_prompt_carries_tool_guidance(self):
        """普通语义轮：启用工具时 system prompt 必须携带使用指导。"""
        llm = ScriptedOpenAILLM([["嗯嗯，我在听。"]])
        agent = _make_agent(llm)
        items = _collect(agent, _batch_input("今天天气不错"))
        assert "嗯嗯，我在听" in "".join(
            i.display_text.text for i in items if isinstance(i, SentenceOutput)
        )
        system = llm.calls[0]["system"]
        assert system.startswith("你是测试助手")
        assert LIVE2D_ACTION_TOOL_GUIDANCE_MARKER in system
        assert "每句话最多调用 1 个动作" in system
        assert "按顺序逐个调用" in system


# ------------------------------------------- 错误轮绝不触发额外多段续聊


class _FailingAgentEngine:
    """chat 即抛 LLMChatError 的桩：模拟 provider 硬失败。"""

    def __init__(self):
        self.calls = 0
        self._memory: list[dict] = []

    def chat(self, batch_input):
        self.calls += 1

        async def _gen():
            from open_llm_vtuber.agent.stateless_llm.openai_compatible_llm import (
                LLMChatError,
            )

            raise LLMChatError("无法连接到 LLM 服务")
            yield  # pragma: no cover —— 仅使该函数成为异步生成器

        return _gen()


class TestErrorTurnNoContinuation:
    def test_failed_turn_sends_error_and_never_continues_extra_segments(self):
        import os
        import sys

        repo_root = os.path.dirname(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        )
        if repo_root not in sys.path:
            sys.path.insert(0, repo_root)

        sent: list[str] = []

        async def ws_send(payload: str) -> None:
            sent.append(payload)

        engine = _FailingAgentEngine()
        context = SimpleNamespace(
            character_config=SimpleNamespace(
                conf_uid="",
                human_name="Tester",
                character_name="AI",
                avatar=None,
            ),
            history_uid="",
            agent_engine=engine,
            asr_engine=None,
            vision_handler=None,
            live2d_model=None,
            tts_engine=None,
            translate_engine=None,
            mood_engine=None,
            director=None,
            memory_curator=None,
        )

        # 续聊上限拉满：若失败轮误触发续聊，chat 调用次数会远大于 1
        os.environ["MELO_MULTI_TURN_EXTRA"] = "5"
        try:
            response = _run(
                process_single_conversation(
                    context=context,
                    websocket_send=ws_send,
                    client_uid="test-client",
                    user_input="你好",
                )
            )
        finally:
            os.environ.pop("MELO_MULTI_TURN_EXTRA", None)

        assert response == ""
        assert engine.calls == 1, "LLM 失败轮不得触发任何额外多段续聊"
        errors = [json.loads(m) for m in sent if '"error"' in m]
        assert errors and "无法连接" in errors[-1]["message"]
