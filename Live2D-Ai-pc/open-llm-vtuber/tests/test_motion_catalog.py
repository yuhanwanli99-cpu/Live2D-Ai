"""PC Preview 半身安全动作目录 / 协议元数据 / 时间轴去重测试。

覆盖任务要求：
1) 目录不含全身动作（bounce_happy 移除）；idle 仅作为 release；
2) payload 可选 source/priority/duration 元数据，且旧客户端兼容（None 省略）；
3) 同一句时间轴去重 + 最小间隔；显式用户命令覆盖时间轴；
4) 导演/规则/括注链路不再产出 bounce_happy / idle 作为逐句 motion。
"""

import asyncio
from types import SimpleNamespace

from open_llm_vtuber.motion_catalog import (
    ALLOWED_ACTIONS,
    MOTION_METADATA,
    RELEASE_ONLY_MOTIONS,
    SAFE_MOTION_IDS,
)
from open_llm_vtuber.director import (
    _VALID_MOTIONS,
    ExpressionDirector,
    DirectorResult,
    detect_user_motion_command,
    normalize_chunk_timeline,
    stage_gesture,
)
from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput


# ─────────────────────────────────────────────
# 1) 安全半身动作目录
# ─────────────────────────────────────────────
def test_catalog_excludes_full_body_actions():
    assert "bounce_happy" not in ALLOWED_ACTIONS
    assert "bounce_happy" not in SAFE_MOTION_IDS
    assert "bounce_happy" not in _VALID_MOTIONS


def test_idle_is_release_only():
    assert "idle" in RELEASE_ONLY_MOTIONS
    assert "idle" not in SAFE_MOTION_IDS
    assert "idle" not in _VALID_MOTIONS
    assert MOTION_METADATA["idle"]["release_only"] is True


def test_all_safe_motions_have_full_metadata():
    for mid in SAFE_MOTION_IDS:
        meta = MOTION_METADATA.get(mid)
        assert meta is not None, mid
        assert meta["label"], mid
        assert meta["description"], mid
        assert meta["halfbody"] is True, mid
        assert isinstance(meta["suggested_duration_ms"], int) and meta["suggested_duration_ms"] > 0
        assert meta["release_only"] is False, mid


# ─────────────────────────────────────────────
# 2) 导演链路绝不产出全身动作 / idle 作为逐句 motion
# ─────────────────────────────────────────────
class _FakeLLM:
    def __init__(self, payload):
        self._payload = payload

    async def chat_completion(self, messages, system=None):
        yield self._payload


def _analyze(payload):
    director = ExpressionDirector(llm=_FakeLLM(payload))
    return asyncio.run(director.analyze("随便一句话"))


def test_llm_bounce_happy_falls_back_to_safe():
    result = _analyze('{"emotion":"joy","strength":2,"motionLabel":"bounce_happy"}')
    assert result.motion_label != "bounce_happy"
    assert result.motion_label in SAFE_MOTION_IDS  # joy → happy
    assert result.motion_label == "happy"


def test_llm_idle_falls_back_to_neutral_action():
    result = _analyze('{"emotion":"neutral","motionLabel":"idle"}')
    assert result.motion_label != "idle"
    assert result.motion_label in SAFE_MOTION_IDS  # neutral → nod


def test_stage_gesture_bounce_maps_to_safe_excited():
    assert stage_gesture("（蹦蹦跳跳）太棒啦") == "excited"
    assert stage_gesture("（跳起来）耶") == "excited"


# ─────────────────────────────────────────────
# 3) 时间轴去重 + 最小间隔
# ─────────────────────────────────────────────
def test_normalize_dedupes_same_motion_in_chunk():
    cues = [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.4, "motion": "nod"},
        {"at": 0.7, "motion": "nod"},
    ]
    assert normalize_chunk_timeline(cues) == [{"at": 0.1, "motion": "nod"}]


def test_normalize_enforces_min_gap_between_different_motions():
    # 0.05 间隔 < min_gap(0.2) → 保留前者
    cues = [{"at": 0.1, "motion": "nod"}, {"at": 0.15, "motion": "bow"}]
    assert normalize_chunk_timeline(cues) == [{"at": 0.1, "motion": "nod"}]
    # 0.5 间隔 ≥ min_gap → 两者保留
    spaced = [{"at": 0.1, "motion": "nod"}, {"at": 0.5, "motion": "bow"}]
    assert normalize_chunk_timeline(spaced) == [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.5, "motion": "bow"},
    ]


def test_normalize_empty_and_unsorted_input():
    assert normalize_chunk_timeline([]) == []
    unsorted = [{"at": 0.8, "motion": "tilt"}, {"at": 0.2, "motion": "nod"}]
    out = normalize_chunk_timeline(unsorted)
    assert out == [{"at": 0.2, "motion": "nod"}, {"at": 0.8, "motion": "tilt"}]
    # 无 motion / 重复间隔过近 → 过滤（tilt@0.3 距 nod@0.1 恰为 0.2，保留）
    assert normalize_chunk_timeline([{"at": 0.1, "motion": ""}]) == []
    assert normalize_chunk_timeline([{"at": 0.1, "motion": "nod"}, {"at": 0.2, "motion": "tilt"}, {"at": 0.3, "motion": "tilt"}]) == [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.3, "motion": "tilt"},
    ]


# ─────────────────────────────────────────────
# 4) payload 可选元数据 + 旧客户端兼容
# ─────────────────────────────────────────────
def test_actions_metadata_serialized_when_set():
    payload = Actions(
        motion="nod",
        motionStrength=2,
        source="user_command",
        priority=100,
        suggested_duration_ms=900,
    ).to_dict()
    assert payload["source"] == "user_command"
    assert payload["priority"] == 100
    assert payload["suggested_duration_ms"] == 900


def test_actions_metadata_omitted_when_none_compat():
    # 旧客户端兼容：未设元数据时 to_dict 与原协议完全相同（不出现新字段）。
    plain = Actions(motion="nod", motionStrength=2).to_dict()
    assert set(plain.keys()) == {"motion", "motionStrength"}
    assert "source" not in plain
    assert "priority" not in plain
    assert "suggested_duration_ms" not in plain


# ─────────────────────────────────────────────
# 5) 显式用户命令：设置优先级/来源并清空时间轴
# ─────────────────────────────────────────────
def test_user_command_sets_priority_and_clears_timeline():
    from open_llm_vtuber.conversations.single_conversation import _apply_director

    class _FakeDirector:
        halfbody_channels = []
        timeout = 0.1

        async def analyze(self, text):
            return DirectorResult(emotion="joy", strength=2, motion_label="wave_big")

    ctx = SimpleNamespace(
        director=_FakeDirector(),
        live2d_model=SimpleNamespace(emo_map={}),
    )
    out = SentenceOutput(
        display_text=DisplayText(text="好的呢（点头）"),
        tts_text="好的呢",
        actions=Actions(),
    )
    asyncio.run(_apply_director(ctx, out, user_text="你点点头好吗"))

    assert detect_user_motion_command("你点点头好吗") == "nod"
    assert out.actions.motion == "nod"
    assert out.actions.source == "user_command"
    assert out.actions.priority == 100
    assert out._director_result.motion_timeline == []
    assert out.actions.suggested_duration_ms == MOTION_METADATA["nod"]["suggested_duration_ms"]


def test_plugin_and_emotion_default_priority():
    from open_llm_vtuber.conversations.single_conversation import _apply_director

    class _FakeDirector:
        halfbody_channels = []
        timeout = 0.1

        async def analyze(self, text):
            return DirectorResult(emotion="sadness", strength=1, motion_label="listen")

    ctx = SimpleNamespace(
        director=_FakeDirector(),
        live2d_model=SimpleNamespace(emo_map={}),
    )
    out = SentenceOutput(
        display_text=DisplayText(text="有点难过呢"),
        tts_text="有点难过呢",
        actions=Actions(),
    )
    asyncio.run(_apply_director(ctx, out, user_text=""))
    assert out.actions.motion == "listen"
    assert out.actions.source == "director"
    assert out.actions.priority == 50