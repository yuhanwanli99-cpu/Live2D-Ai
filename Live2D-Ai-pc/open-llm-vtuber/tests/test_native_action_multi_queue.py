"""主 LLM 同轮多个 live2d_action 动作：保序绑定 + motionTimeline 转换测试。

背景（修复「同轮多动作只保留最后一个」）：用户日志显示两个 tool calls
shake_no→nod 均 accepted/queue=2，但最终 payload 仅剩 motion=nod。

契约：
- 多个 live2d_action 事件按接受顺序保序；同句 2 个动作 → 整队转本句
  motionTimeline（[0.1, 0.8] 分布），每帧携带 motion/strength/source/priority
  （reason 可选），不再只剩最后一个、也不丢首事件；
- 三动作同理：帧序 = 接受顺序，at 单调递增，无重复无丢失；
- 多句场景：每句消费自己的动作；单动作句仍走一次性 motion 兼容路径；
- 显式用户命令（priority=100）覆盖工具时间轴；
- 中断 / 本轮结束清空未触发工具动作，绝不跨轮泄漏；后续轮工具仍可调用；
- TTS 链路逐块携带时间线帧（播放位置触发），渲染端帧级调度可用。
"""

import asyncio
from types import SimpleNamespace

import pytest
from loguru import logger
from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput
from open_llm_vtuber.conversations.conversation_utils import handle_sentence_output
from open_llm_vtuber.conversations.single_conversation import (
    _LLM_TOOL_PRIORITY,
    _apply_director,
    _bind_pending_llm_action,
    _drop_pending_llm_actions,
)
from open_llm_vtuber.director import project_tool_actions_into_chunks


def _run(coro):
    return asyncio.run(coro)


def _sentence(text: str = "好呀，我们出发吧。") -> SentenceOutput:
    return SentenceOutput(
        display_text=DisplayText(text=text, name="AI"),
        tts_text=text,
        actions=Actions(),
    )


def _event(action="nod", strength=2, reason=""):
    event = {
        "type": "live2d_action",
        "action": action,
        "strength": strength,
        "source": "llm_tool",
        "tool": "live2d.perform_action",
    }
    if reason:
        event["reason"] = reason
    return event


class _Ctx:
    """最小 ServiceContext 替身：可注入导演（默认一个会炸的导演）。"""

    def __init__(self, director=None):
        self.director = director
        self.live2d_model = None


class _BoomDirector:
    async def analyze(self, text):
        raise AssertionError("director must not run when llm tool actions bound")


# ---------------------------------------------------------------- 绑定层：同句多动作保序转时间线


class TestSameSentenceMultiActionBinding:
    def test_two_actions_keep_order_and_convert_to_timeline(self):
        output = _sentence()
        pending = [_event("shake_no", 2, "否认"), _event("nod", 3)]
        _bind_pending_llm_action(output, pending)

        # 队列清空：不丢不剩
        assert pending == []
        # 多动作不再走一次性路径（避免前端「有时间轴忽略 motion」吞掉首动作）
        assert getattr(output, "_native_action", None) is None
        frames = output._native_action_timeline
        assert [f["motion"] for f in frames] == ["shake_no", "nod"], "必须保持接受顺序"
        # 每帧携带调度元数据；首事件不丢、末事件不覆盖首事件
        assert frames[0] == {
            "motion": "shake_no",
            "strength": 2,
            "source": "llm_tool",
            "priority": _LLM_TOOL_PRIORITY,
            "reason": "否认",
        }
        assert frames[1]["strength"] == 3
        assert frames[1]["priority"] == _LLM_TOOL_PRIORITY
        assert "reason" not in frames[1]

    def test_three_actions_all_preserved_in_order(self):
        output = _sentence("真的假的？")
        pending = [
            _event("surprise", 3),
            _event("look_around"),
            _event("tilt", 1),
        ]
        _bind_pending_llm_action(output, pending)
        frames = output._native_action_timeline
        assert [f["motion"] for f in frames] == ["surprise", "look_around", "tilt"]
        assert [f["strength"] for f in frames] == [3, 2, 1]
        assert all(f["source"] == "llm_tool" for f in frames)
        assert all(f["priority"] == _LLM_TOOL_PRIORITY for f in frames)

    def test_single_action_keeps_one_shot_motion_compat(self):
        output = _sentence()
        pending = [_event("nod", 2)]
        _bind_pending_llm_action(output, pending)
        # 单动作：一次性 motion 兼容路径不变，无时间线
        assert output._native_action == {
            "action": "nod",
            "strength": 2,
            "reason": "",
            "source": "llm_tool",
        }
        assert not getattr(output, "_native_action_timeline", None)

    def test_director_skipped_for_timeline_only_sentence(self):
        output = _sentence()
        _bind_pending_llm_action(output, [_event("shake_no"), _event("nod")])
        ctx = _Ctx(director=_BoomDirector())
        _run(_apply_director(ctx, output))  # 不抛即通过：导演被跳过
        assert len(output._native_action_timeline) == 2

    def test_user_command_clears_tool_timeline(self):
        from open_llm_vtuber.director import ExpressionDirector

        output = _sentence()
        _bind_pending_llm_action(output, [_event("shake_no"), _event("nod")])
        ctx = _Ctx(director=ExpressionDirector(llm=None))
        _run(_apply_director(ctx, output, user_text="帮我摇摇头"))
        # 用户命令最高：工具时间线清空，命令动作生效
        assert output._native_action_timeline == []
        assert output.actions.motion == "shake_no"
        assert output.actions.source == "user_command"
        assert output.actions.priority == 100


# ---------------------------------------------------------------- 投影层：[0.1, 0.8] 分布 + 分块切片


class TestToolTimelineProjection:
    def test_two_frames_spread_across_span_in_order(self):
        chunks = ["好呀，我们出发吧"]
        sliced = project_tool_actions_into_chunks(
            [
                {"motion": "shake_no", "strength": 2, "source": "llm_tool", "priority": 90},
                {"motion": "nod", "strength": 3, "source": "llm_tool", "priority": 90},
            ],
            chunks,
        )
        flat = [cue for chunk in sliced for cue in chunk]
        assert [c["motion"] for c in flat] == ["shake_no", "nod"]
        # 任务定版示例区间：按顺序分布在 [0.1, 0.8]
        assert flat[0]["at"] == pytest.approx(0.1, abs=1e-3)
        assert flat[1]["at"] == pytest.approx(0.8, abs=1e-3)

    def test_three_frames_monotonic_no_loss(self):
        frames = [
            {"motion": m, "strength": s}
            for m, s in (("surprise", 3), ("look_around", 2), ("tilt", 1))
        ]
        sliced = project_tool_actions_into_chunks(frames, ["一句话"])
        flat = [c for chunk in sliced for c in chunk]
        ats = [c["at"] for c in flat]
        assert len(flat) == 3, "三帧一个都不能丢"
        assert ats == sorted(ats), "at 必须随帧序单调递增"
        assert ats[0] >= 0.1 - 1e-9 and ats[-1] <= 0.8 + 1e-9

    def test_multi_chunk_sentence_distributes_across_chunks(self):
        long_text = "啊" * 30 + "。" + "吧" * 30 + "。"  # split 后两块各 ~31 字
        from open_llm_vtuber.conversations.conversation_utils import split_tts_chunks

        chunks = split_tts_chunks(long_text)
        assert len(chunks) >= 2
        sliced = project_tool_actions_into_chunks(
            [{"motion": "nod"}, {"motion": "shake_no"}, {"motion": "tilt"}],
            chunks,
        )
        flat = [(i, c) for i, chunk in enumerate(sliced) for c in chunk]
        assert len(flat) == 3, "分块后仍然无丢失"
        # 帧序跨块单调：后一帧的 (块序号, at) 不可能整体早于前一帧
        positions = [(i, c["at"]) for i, c in flat]
        assert positions == sorted(positions)
        # 帧级字段逐帧保留
        assert all(c["source"] == "llm_tool" and c["priority"] == 90 for _, c in flat)

    def test_illegal_frame_id_dropped_by_whitelist(self):
        sliced = project_tool_actions_into_chunks(
            [{"motion": "backflip"}, {"motion": "nod"}],
            ["句子"],
        )
        flat = [c for chunk in sliced for c in chunk]
        assert [c["motion"] for c in flat] == ["nod"]

    def test_empty_inputs_safe(self):
        assert project_tool_actions_into_chunks([], ["abc"]) == [[]]
        assert project_tool_actions_into_chunks([{"motion": "nod"}], []) == []


# ---------------------------------------------------------------- TTS 链路：时间线随 audio 帧下发


class _RecorderTtsManager:
    def __init__(self):
        self.calls = []

    async def speak(self, **kwargs):
        self.calls.append(kwargs)


async def _noop_ws_send(_payload):
    return None


class TestHandleSentenceOutputToolTimeline:
    def test_single_tts_chunk_carries_ordered_frames_without_one_shot(self):
        output = _sentence("嗯嗯，好的。")
        _bind_pending_llm_action(
            output, [_event("shake_no", 2, "否认"), _event("nod", 3)]
        )
        _run(_apply_director(_Ctx(), output))

        manager = _RecorderTtsManager()
        _run(handle_sentence_output(output, None, None, _noop_ws_send, manager))

        assert len(manager.calls) == 1
        actions = manager.calls[0]["actions"]
        # 时间轴接管：一次性 motion 必须为空（防双通道重复触发）
        assert actions.motion is None
        timeline = actions.motionTimeline
        assert [c["motion"] for c in timeline] == ["shake_no", "nod"]
        # 帧级元数据完整透传给渲染端
        assert [c["strength"] for c in timeline] == [2, 3]
        assert all(c["source"] == "llm_tool" for c in timeline)
        assert all(c["priority"] == _LLM_TOOL_PRIORITY for c in timeline)
        assert timeline[0]["reason"] == "否认"

    def test_single_action_still_one_shot_on_first_chunk_only(self):
        output = _sentence("嗯。")
        _bind_pending_llm_action(output, [_event("tilt", 1)])
        _run(_apply_director(_Ctx(), output))
        manager = _RecorderTtsManager()
        _run(handle_sentence_output(output, None, None, _noop_ws_send, manager))
        first = manager.calls[0]["actions"]
        assert first.motion == "tilt"
        assert first.motionStrength == 1
        assert first.motionTimeline is None

    def test_user_command_payload_not_overridden_by_tool_timeline(self):
        output = _sentence("好。")
        setattr(
            output,
            "_native_action_timeline",
            [
                {"motion": "nod", "strength": 2, "source": "llm_tool", "priority": 90},
            ],
        )
        output.actions.motion = "shake_no"
        output.actions.source = "user_command"
        output.actions.priority = 100
        manager = _RecorderTtsManager()
        _run(handle_sentence_output(output, None, None, _noop_ws_send, manager))
        call = manager.calls[0]
        assert call["actions"].motion == "shake_no"
        assert call["actions"].motionTimeline is None, "用户命令时工具时间线不下发"


# ---------------------------------------------------------------- 对话级：多句 / 后续轮 / 中断清空


class _ScriptedEngine:
    """按脚本依次返回 agent 流的最小引擎替身。"""

    def __init__(self, scripts):
        self.scripts = [list(s) for s in scripts]
        self.chat_calls = 0

    def chat(self, batch_input):
        self.chat_calls += 1
        events = self.scripts.pop(0) if self.scripts else []

        async def stream():
            for item in events:
                yield item

        return stream()


def _make_context(engine):
    return SimpleNamespace(
        character_config=SimpleNamespace(
            human_name="tester",
            character_name="AI",
            conf_uid="",
            avatar="",
        ),
        asr_engine=None,
        vision_handler=None,
        history_uid="",
        live2d_model=None,
        tts_engine=None,
        translate_engine=None,
        plugin_manager=None,
        agent_engine=engine,
    )


@pytest.fixture()
def tts_recorder(monkeypatch):
    """把 TTSTaskManager.speak_async 换成记录器（不走真实音频合成）。"""
    from open_llm_vtuber.conversations.tts_manager import TTSTaskManager

    calls: list[dict] = []

    async def record(self, **kwargs):
        calls.append(kwargs)

    monkeypatch.setattr(TTSTaskManager, "speak_async", record)
    return calls


def _run_turn(monkeypatch, context, user_input="你好呀"):
    from open_llm_vtuber.conversations.single_conversation import (
        process_single_conversation,
    )

    monkeypatch.setenv("MELO_MULTI_TURN_EXTRA", "0")
    sent: list[str] = []

    async def ws_send(msg):
        sent.append(msg)

    return asyncio.run(
        process_single_conversation(
            context=context,
            websocket_send=ws_send,
            client_uid="test-uid",
            user_input=user_input,
        )
    )


class TestDropPendingHelper:
    def test_clears_queue_in_place(self):
        pending = [_event("nod"), _event("tilt")]
        _drop_pending_llm_actions(pending, "对话被打断")
        assert pending == []

    def test_empty_queue_is_noop(self):
        pending: list[dict] = []
        _drop_pending_llm_actions(pending, "本轮结束仍有未绑定动作")
        assert pending == []


class TestConversationLevel:
    def test_same_turn_two_actions_bind_to_one_sentence_in_order(
        self, monkeypatch, tts_recorder
    ):
        engine = _ScriptedEngine(
            [
                [
                    _event("shake_no", 2),
                    _event("nod", 3),
                    SentenceOutput(
                        display_text=DisplayText(text="才不是呢。"),
                        tts_text="才不是呢。",
                        actions=Actions(),
                    ),
                ]
            ]
        )
        context = _make_context(engine)
        _run_turn(monkeypatch, context)

        motions = [c["actions"].motion for c in tts_recorder]
        assert motions == [None], "多动作句不再下发一次性 motion"
        timeline = tts_recorder[0]["actions"].motionTimeline
        assert [c["motion"] for c in timeline] == ["shake_no", "nod"], (
            "两个动作都必须在 payload 里且保序（修复只留最后一个）"
        )

    def test_three_actions_same_sentence_all_present(self, monkeypatch, tts_recorder):
        engine = _ScriptedEngine(
            [
                [
                    _event("surprise", 3),
                    _event("look_around"),
                    _event("tilt", 1),
                    SentenceOutput(
                        display_text=DisplayText(text="真的吗！"),
                        tts_text="真的吗！",
                        actions=Actions(),
                    ),
                ]
            ]
        )
        context = _make_context(engine)
        _run_turn(monkeypatch, context)

        timeline = tts_recorder[0]["actions"].motionTimeline
        assert [c["motion"] for c in timeline] == ["surprise", "look_around", "tilt"]
        ats = [c["at"] for c in timeline]
        assert ats == sorted(ats) and len(set(ats)) == 3

    def test_multi_sentence_each_pops_one_legacy_one_shot(self, monkeypatch, tts_recorder):
        engine = _ScriptedEngine(
            [
                [
                    _event("listen", 2),
                    SentenceOutput(
                        display_text=DisplayText(text="第一句。"),
                        tts_text="第一句。",
                        actions=Actions(),
                    ),
                    _event("surprise", 3),
                    SentenceOutput(
                        display_text=DisplayText(text="第二句。"),
                        tts_text="第二句。",
                        actions=Actions(),
                    ),
                ]
            ]
        )
        context = _make_context(engine)
        _run_turn(monkeypatch, context)

        assert len(tts_recorder) == 2
        first, second = tts_recorder
        assert first["actions"].motion == "listen"
        assert first["actions"].motionTimeline is None
        assert second["actions"].motion == "surprise"
        assert second["actions"].motionTimeline is None

    def test_next_turn_tools_still_callable_no_cross_turn_leak(
        self, monkeypatch, tts_recorder
    ):
        engine = _ScriptedEngine(
            [
                # 第 1 轮：同句双动作
                [
                    _event("shake_no"),
                    _event("nod"),
                    SentenceOutput(
                        display_text=DisplayText(text="不是的。"),
                        tts_text="不是的。",
                        actions=Actions(),
                    ),
                ],
                # 第 2 轮：单动作 → 一次性兼容路径照常可用
                [
                    _event("tilt", 1),
                    SentenceOutput(
                        display_text=DisplayText(text="歪头看你。"),
                        tts_text="歪头看你。",
                        actions=Actions(),
                    ),
                ],
            ]
        )
        context = _make_context(engine)
        _run_turn(monkeypatch, context, user_input="第一问")
        _run_turn(monkeypatch, context, user_input="第二问")

        turn1 = tts_recorder[0]["actions"]
        assert [c["motion"] for c in turn1.motionTimeline] == ["shake_no", "nod"]
        # 第 2 轮不受上一轮队列影响：单动作一次性下发，无残留时间线
        turn2 = tts_recorder[-1]["actions"]
        assert turn2.motion == "tilt"
        assert turn2.motionTimeline is None
        assert engine.chat_calls == 2

    def test_leftover_actions_dropped_with_log_at_turn_end(
        self, monkeypatch, tts_recorder
    ):
        """模型只调工具不说正文：本轮结束显式丢弃并可观测，绝不跨轮泄漏。"""
        lines: list[str] = []
        handler_id = logger.add(lambda msg: lines.append(str(msg)), level="WARNING")
        try:
            engine = _ScriptedEngine(
                [
                    [_event("nod"), _event("tilt")],  # 无任何正文句
                    [SentenceOutput(
                        display_text=DisplayText(text="后来的一句话。"),
                        tts_text="后来的一句话。",
                        actions=Actions(),
                    )],  # 第 2 轮正常说话
                ]
            )
            context = _make_context(engine)
            _run_turn(monkeypatch, context, user_input="只做动作")
            _run_turn(monkeypatch, context, user_input="再说话")

            assert any("本轮结束仍有未绑定动作" in line for line in lines)
            # 泄漏的动作绝不出现在第 2 轮 payload 里
            final = tts_recorder[-1]["actions"]
            assert final.motion is None
            assert not final.motionTimeline
        finally:
            logger.remove(handler_id)

    def test_interrupt_clears_untriggered_tool_actions(self, monkeypatch, tts_recorder):
        """打断（任务取消）：未触发的工具动作立即作废并打可见日志。"""
        from open_llm_vtuber.agent.output_types import DisplayText as DT

        lines: list[str] = []
        handler_id = logger.add(lambda msg: lines.append(str(msg)), level="WARNING")
        try:
            release = asyncio.Event()

            class _HangingEngine:
                def chat(self, batch_input):
                    async def stream():
                        yield _event("nod")
                        yield _event("tilt")
                        await release.wait()  # 正文永远不来 → 模拟流挂起被打断
                        yield SentenceOutput(
                            display_text=DT(text="不该出现"),
                            tts_text="不该出现",
                            actions=Actions(),
                        )

                    return stream()

            context = _make_context(_HangingEngine())

            async def scenario():
                task = asyncio.create_task(
                    process_single_conversation_async(context)
                )
                await asyncio.sleep(0.05)
                task.cancel()
                with pytest.raises(asyncio.CancelledError):
                    await task

            monkeypatch.setenv("MELO_MULTI_TURN_EXTRA", "0")

            async def process_single_conversation_async(ctx):
                from open_llm_vtuber.conversations.single_conversation import (
                    process_single_conversation,
                )

                async def ws_send(msg):
                    return None

                return await process_single_conversation(
                    context=ctx,
                    websocket_send=ws_send,
                    client_uid="test-uid",
                    user_input="你好",
                )

            asyncio.run(scenario())
            assert any("对话被打断" in line and "nod" in line for line in lines)
        finally:
            logger.remove(handler_id)
            release.set()
