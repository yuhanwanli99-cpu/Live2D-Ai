"""括注动作时间轴（motionTimeline）：解析 → 分块投影 → 链路下发 测试。

契约（my-neuro 式"播放位置触发"，对齐 Android M7 关键词兜底的线性近似）：
- director.stage_gesture_timeline：全部括注 → 带文本位置的动作帧（升序）；
- director.project_motion_cues_into_chunks：句子级帧按 TTS 分块切片，
  at = 块内归一化播放进度 ∈ [0,1)，任何异常输入整体降级为全空；
- Actions.motionTimeline 随 to_dict() 透传；None 时省略（向后兼容）；
- handle_sentence_output：时间轴存在时清掉一次性 motion、逐块挂时间轴。
"""

from dataclasses import replace

from open_llm_vtuber.agent.output_types import (
    Actions,
    DisplayText,
    SentenceOutput,
)
from open_llm_vtuber.conversations.conversation_utils import (
    handle_sentence_output,
    split_tts_chunks,
)
from open_llm_vtuber.director import (
    DirectorResult,
    project_motion_cues_into_chunks,
    stage_gesture,
    stage_gesture_timeline,
)
from open_llm_vtuber.utils.tts_preprocessor import filter_parentheses


# ---------------------------------------------------------------- stage_gesture_timeline


def test_timeline_multi_bracket_ordered_with_positions():
    text = "（轻轻点头）你好呀，今天（开心地挥手）一起玩吧"
    cues = stage_gesture_timeline(text)
    # "（轻轻点头）"=6 字符，其后正文 6 字符 → 第二个括注 position=12、
    # 扣除第一个括注跨度后 clean_position=6；
    # 但"开心地挥手"已删除（bai 无手臂参数）→ 不再产出 happy 帧，仅保留 nod。
    assert cues == [
        {"position": 0, "clean_position": 0, "motion": "nod"},
    ]


def test_timeline_skips_unmatched_bracket_but_still_counts_span():
    # "(smile)" 未命中关键词 → 不产帧，但其跨度仍从后续坐标中扣除
    text = "(smile)（点头）好的"
    cues = stage_gesture_timeline(text)
    assert cues == [{"position": 7, "clean_position": 0, "motion": "nod"}]


def test_timeline_empty_cases():
    assert stage_gesture_timeline("哎呀，天气好热啊") == []
    assert stage_gesture_timeline("(只是普通括号内容)") == []
    assert stage_gesture_timeline("（点头）") != []  # 整句只有括注也能命中
    assert stage_gesture_timeline("") == []
    assert stage_gesture_timeline(None) == []


def test_timeline_mixed_full_half_width():
    cues = stage_gesture_timeline("a(点头)b（鼓掌）c")
    # 原文偏移：a=0、(点头)=1..4、b=5 → 第二个括注 position=6；
    # 扣除 "(点头)" 跨度 4 字符后 clean_position=2；
    # 但"鼓掌"已删除（手臂动作）→ 不产帧，仅保留 nod。
    assert cues == [
        {"position": 1, "clean_position": 1, "motion": "nod"},
    ]


def test_stage_gesture_first_hit_backward_compat():
    assert stage_gesture("（点头）（挥手扇风）") == "nod"
    assert stage_gesture("（看到热字，轻轻用小手扇风）哎呀") is None


# ---------------------------------------------------- project_motion_cues_into_chunks


def test_projection_splits_across_chunks_with_normalized_at():
    tts_text = "你好呀，今天我们一起出去玩吧好不好"  # len=18
    chunks = ["你好呀，", "今天我们一起出去玩吧好不好"]
    # 锚文本：清洗掉两个括注后的朗读文本长度 = 18 + 10 - 4 = 18？直接用实际值
    anchor_len = len(
        filter_parentheses("（点头）你好呀，（挥手扇）今天我们一起出去玩吧好不好")
    )
    cues = [
        {"clean_position": 2, "motion": "nod"},        # 落在块 1 中部
        {"clean_position": 10, "motion": "look_around"},  # 落在块 2 中部
    ]
    sliced = project_motion_cues_into_chunks(cues, tts_text, chunks, anchor_len)
    assert len(sliced) == 2
    total = len(tts_text)
    s1, e1 = 0, len(chunks[0]) / total
    s2, e2 = len(chunks[0]) / total, 1.0
    frac1 = 2 / anchor_len
    frac2 = 10 / anchor_len
    at1 = round(min(max((frac1 - s1) / max(e1 - s1, 1e-6), 0.0), 0.999), 4)
    at2 = round(min(max((frac2 - s2) / max(e2 - s2, 1e-6), 0.0), 0.999), 4)
    assert sliced[0] == [{"at": at1, "motion": "nod"}]
    assert sliced[1] == [{"at": at2, "motion": "look_around"}]


def test_projection_boundary_goes_to_later_chunk():
    tts_text = "abcd"
    chunks = ["ab", "cd"]
    cues = [{"clean_position": 2, "motion": "nod"}]  # 恰在分界
    sliced = project_motion_cues_into_chunks(cues, tts_text, chunks, anchor_len=4)
    assert sliced[0] == []
    assert sliced[1] == [{"at": 0.0, "motion": "nod"}]


def test_projection_failure_falls_back_to_all_empty():
    # chunk 无法在 tts_text 中顺序定位 → 时间轴整体不下发（退回旧行为）
    sliced = project_motion_cues_into_chunks(
        [{"clean_position": 1, "motion": "nod"}], "abcdef", ["xyz"], anchor_len=6
    )
    assert sliced == [[]]


def test_projection_degenerate_inputs_are_safe():
    assert project_motion_cues_into_chunks([], "abc", ["abc"], anchor_len=3) == [[]]
    assert project_motion_cues_into_chunks(
        [{"clean_position": 0, "motion": "nod"}], "abc", ["abc"], anchor_len=0
    ) == [[]]
    # 空 chunks → 零个分块列表
    assert project_motion_cues_into_chunks(
        [{"clean_position": 0, "motion": "nod"}], "", [], anchor_len=3
    ) == []


def test_projection_multiple_cues_same_chunk_sorted_by_at():
    tts_text = "abcdefgh"
    chunks = ["abcdefgh"]
    cues = [
        {"clean_position": 6, "motion": "listen"},
        {"clean_position": 2, "motion": "nod"},
    ]
    sliced = project_motion_cues_into_chunks(cues, tts_text, chunks, anchor_len=8)
    assert [c["motion"] for c in sliced[0]] == ["nod", "listen"]


# ------------------------------------------------------------------ Actions 透传


def test_actions_to_dict_includes_timeline_only_when_set():
    plain = Actions(motion="nod").to_dict()
    assert "motionTimeline" not in plain

    with_tl = Actions(motionTimeline=[{"at": 0.5, "motion": "nod"}]).to_dict()
    assert with_tl["motionTimeline"] == [{"at": 0.5, "motion": "nod"}]


def test_director_result_to_dict_carries_timeline():
    result = DirectorResult(motion_label="nod", motion_timeline=[{"position": 0}])
    assert result.to_dict()["motionTimeline"] == [{"position": 0}]
    assert DirectorResult().to_dict()["motionTimeline"] == []


def test_replace_preserves_other_fields_when_attaching_timeline():
    base = Actions(expressions=[1], parameterOverrides={"ParamEyeLOpen": 0.2})
    chunk = replace(base, motionTimeline=[{"at": 0.1, "motion": "tilt"}])
    assert chunk.parameterOverrides == {"ParamEyeLOpen": 0.2}
    assert chunk.expressions == [1]
    # 不污染原对象
    assert base.motionTimeline is None


# ------------------------------------------------------------ handle_sentence_output


class _RecorderTtsManager:
    """记录 speak() 关键字参数的假 TTS 队列（不合成音频）。"""

    def __init__(self):
        self.calls = []

    async def speak(self, **kwargs):
        self.calls.append(kwargs)


async def _noop_ws_send(_payload):
    return None


def _sentence_output(display_text: str, tts_text: str, actions: Actions) -> SentenceOutput:
    return SentenceOutput(
        display_text=DisplayText(text=display_text),
        tts_text=tts_text,
        actions=actions,
    )


def _run(coro):
    import asyncio

    return asyncio.run(coro)


def test_handle_sentence_output_slices_timeline_and_drops_one_shot_motion():
    # 清洗后朗读文本 50 字符 → split_tts_chunks 切成 [25, 25] 两块
    raw = "（轻轻点头）" + "啊" * 24 + "。" + "（开心地摇头）" + "吧" * 24 + "。"
    tts_text = filter_parentheses(raw)
    assert len(tts_text) == 50
    chunks = split_tts_chunks(tts_text)
    assert chunks == ["啊" * 24 + "。", "吧" * 24 + "。"]

    output = _sentence_output(raw, tts_text, Actions(motion="nod"))
    setattr(
        output,
        "_director_result",
        DirectorResult(
            motion_label="nod",
            motion_timeline=stage_gesture_timeline(raw),
        ),
    )

    manager = _RecorderTtsManager()
    _run(handle_sentence_output(output, None, None, _noop_ws_send, manager))

    assert [c["tts_text"] for c in manager.calls] == chunks
    first, second = manager.calls
    # 字幕-声音同步：仅句首块携带 sentence_start 标记
    assert first["sentence_start"] is True
    assert second["sentence_start"] is False
    # 时间轴接管：一次性 motion 被清掉，逐块挂上归一化动作帧
    assert first["actions"].motion is None
    assert first["actions"].motionTimeline == [{"at": 0.0, "motion": "nod"}]
    # 第二个括注恰好在两块交界（clean_position=25 → frac=0.5）→ 归入块 2、at=0
    # （括注只认六动作关键词：摇头 → shake_no；挥手等已删语义不产帧）
    assert second["actions"].motion is None
    assert second["actions"].motionTimeline == [{"at": 0.0, "motion": "shake_no"}]


def test_handle_sentence_output_without_timeline_keeps_legacy_behavior():
    raw = "普通一句话（点头）"
    tts_text = filter_parentheses(raw)

    # 无导演结果：整句一次性 motion 原样保留，无 motionTimeline
    output = _sentence_output(raw, tts_text, Actions(motion="nod"))
    manager = _RecorderTtsManager()
    _run(handle_sentence_output(output, None, None, _noop_ws_send, manager))
    assert len(manager.calls) == 1
    call = manager.calls[0]
    assert call["actions"].motion == "nod"
    assert call["actions"].motionTimeline is None

    # 最终防线：已删 id（sway）即使从上游漏进来，也在下发前被白名单清空
    output2 = _sentence_output(raw, tts_text, Actions(motion="sway"))
    setattr(output2, "_director_result", DirectorResult(motion_label=None))
    manager2 = _RecorderTtsManager()
    _run(handle_sentence_output(output2, None, None, _noop_ws_send, manager2))
    assert manager2.calls[0]["actions"].motion is None
    assert manager2.calls[0]["actions"].motionTimeline is None