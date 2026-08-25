"""主 LLM 原生工具动作 → 句子 Actions payload 绑定测试。

契约：
- live2d_action 事件入队后，下一个切出的 SentenceOutput 被绑定该动作
  （motion/motionStrength/source=llm_tool/priority=90/suggested_duration_ms）；
- 主 LLM 显式工具优先级高于 ExpressionDirector：命中工具动作时跳过导演分析，
  导演仅作 fallback（无工具动作时照常兜底）；
- 显式用户命令仍最高（priority=100），覆盖同句的工具动作；
- 动作 payload 过白名单：非法 id 不产 payload；
- 记忆/正文零污染：绑定只改 Actions payload，不动 display/tts 文本。
"""

import asyncio

from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput
from open_llm_vtuber.conversations.single_conversation import (
    _apply_director,
    _bind_pending_llm_action,
)
from open_llm_vtuber.director import ExpressionDirector


def _run(coro):
    return asyncio.run(coro)


def _sentence(text: str = "好呀，我们出发吧。") -> SentenceOutput:
    return SentenceOutput(
        display_text=DisplayText(text=text, name="AI"),
        tts_text=text,
        actions=Actions(),
    )


class _Ctx:
    """最小 ServiceContext 替身：可注入导演。"""

    def __init__(self, director=None):
        self.director = director
        self.live2d_model = None


def _llm_tool_event(action="nod", strength=2, reason="开心"):
    return {
        "type": "live2d_action",
        "action": action,
        "strength": strength,
        "reason": reason,
        "source": "llm_tool",
        "tool": "live2d.perform_action",
    }


class TestBinding:
    def test_pending_action_binds_to_next_sentence(self):
        output = _sentence()
        pending = [_llm_tool_event()]
        _bind_pending_llm_action(output, pending)
        assert pending == [], "绑定应消费一条事件"

        ctx = _Ctx(director=None)
        _run(_apply_director(ctx, output))
        actions = output.actions
        assert actions.motion == "nod"
        assert actions.motionStrength == 2
        assert actions.source == "llm_tool"
        assert actions.priority == 90
        assert actions.suggested_duration_ms == 900  # MOTION_METADATA[nod]
        # 正文零污染：display/tts 文本不含任何动作 JSON
        assert "nod" not in output.display_text.text
        assert "{" not in output.display_text.text

    def test_action_consumed_by_first_sentence_only(self):
        first = _sentence("第一句")
        second = _sentence("第二句")
        pending = [_llm_tool_event(action="tilt", strength=3)]
        for out in (first, second):
            _bind_pending_llm_action(out, pending)
            _run(_apply_director(_Ctx(), out))
        assert first.actions.motion == "tilt"
        assert first.actions.priority == 90
        # 第二句没有工具动作 → 无导演时保持空动作（fallback 链不产动作）
        assert second.actions.motion is None

    def test_illegal_action_never_binds(self):
        output = _sentence()
        _bind_pending_llm_action(output, [_llm_tool_event(action="backflip")])
        _run(_apply_director(_Ctx(), output))
        assert output.actions.motion is None


class TestDirectorFallback:
    def test_director_still_runs_without_native_action(self):
        """无工具动作时 ExpressionDirector 兜底照常工作。"""
        output = _sentence("哇！真的吗！")
        director = ExpressionDirector(llm=None)  # None → rule_fallback
        ctx = _Ctx(director=director)
        _run(_apply_director(ctx, output, user_text=""))
        # 规则兜底至少产出情绪；动作按克制映射（surprise→surprise）
        assert getattr(output, "_director_result", None) is not None
        assert output._director_result.emotion in {"surprise", "neutral"}

    def test_native_tool_skips_director_llm(self):
        """命中工具动作时不应调用导演 LLM（导演仅 fallback）。"""

        class _BoomDirector:
            async def analyze(self, text):
                raise AssertionError("director must not run when llm tool action bound")

        output = _sentence()
        _bind_pending_llm_action(output, [_llm_tool_event()])
        ctx = _Ctx(director=_BoomDirector())
        _run(_apply_director(ctx, output))  # 不抛即通过
        assert output.actions.source == "llm_tool"


class TestUserCommandPrecedence:
    def test_user_command_beats_llm_tool_on_same_sentence(self):
        from open_llm_vtuber.conversations.single_conversation import (
            _LLM_TOOL_PRIORITY,
        )

        output = _sentence()
        _bind_pending_llm_action(output, [_llm_tool_event(action="nod")])
        # 用户显式命令「摇头」→ priority 100 应胜出
        ctx = _Ctx(director=ExpressionDirector(llm=None))
        _run(_apply_director(ctx, output, user_text="帮我摇摇头"))
        assert output.actions.motion == "shake_no"
        assert output.actions.source == "user_command"
        assert output.actions.priority == 100
        assert output.actions.priority > _LLM_TOOL_PRIORITY


class TestActionPayloadReachesTTSAudio:
    """端到端：绑定后的动作随句子进入 TTS 分发（Actions payload 与音频同路）。"""

    def test_handle_sentence_output_carries_llm_tool_payload(self):
        from open_llm_vtuber.conversations.conversation_utils import (
            handle_sentence_output,
        )

        class _RecordingTTSManager:
            def __init__(self):
                self.calls = []

            async def speak(self, **kwargs):
                self.calls.append(kwargs)

        output = _sentence("好呀，我们出发吧。一起走吧。")
        _bind_pending_llm_action(output, [_llm_tool_event(action="nod", strength=3)])
        _run(_apply_director(_Ctx(), output))

        tts_manager = _RecordingTTSManager()
        sent = []

        async def ws_send(msg):
            sent.append(msg)

        async def scenario():
            return await handle_sentence_output(
                output=output,
                live2d_model=None,
                tts_engine=None,
                websocket_send=ws_send,
                tts_manager=tts_manager,
                translate_engine=None,
                mood_tier="neutral",
                plugin_manager=None,
            )

        spoken_text = _run(scenario())

        # 正文零污染：朗读/返回文本只有说话内容，无动作 JSON
        assert "nod" not in spoken_text
        assert "{" not in spoken_text

        # 动作随首个 TTS 分块的 actions payload 下发
        assert tts_manager.calls, "至少一次 TTS 分发"
        first = tts_manager.calls[0]
        actions = first["actions"]
        assert actions.motion == "nod"
        assert actions.motionStrength == 3
        assert actions.source == "llm_tool"
        assert actions.priority == 90

        # 一次性 motion 只挂句首块（既有行为），后续块不重复触发
        for index, call in enumerate(tts_manager.calls[1:], start=1):
            later_actions = call["actions"]
            if later_actions is not None:
                assert getattr(later_actions, "motion", None) is None, (
                    f"chunk {index} 不应重复携带一次性 motion"
                )
        # 表情帧通道（emotion/strength）与工具强度一致
        assert all(call["strength"] == 3 for call in tts_manager.calls)
