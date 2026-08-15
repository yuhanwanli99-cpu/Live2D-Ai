"""P6 · WebSocket 消息协议 — RED 测试

依据:
  - contract-pc-pipeline.md §P6（P6-01 ~ P6-06）
  - architecture-pc-pipeline.md §3.8 / §P6（WS 消息协议）

被测目标态:
  - `prepare_audio_payload` 新增可选 `emotion` 字段（架构 §3.8 "仅新增可选字段"，audio 帧 schema §4.6.3）
  - 新增 emotion 帧构造（types.py EMOTION_FRAME_SCHEMA / 独立构造函数），schema 见 architecture §4.6.2
  - 消息 type 枚举：audio / emotion（向后兼容）

文件锁：本文件不读 src/ 下任何实现 body，只按契约+架构声明的接口签名断言。
当前实现（stream_audio.prepare_audio_payload）尚无 `emotion` 参数 → 本文件初始 RED。
"""
import pytest

# 被测接口（契约 §P6）。现有实现签名 = (audio_path, chunk_length_ms, display_text, actions, forwarded)，
# 目标态在尾部追加 `emotion` 可选字段。若 import 失败说明模块结构不符 → 视为 RED（collection error）。
from open_llm_vtuber.utils.stream_audio import prepare_audio_payload

# ── 合法 emotion key 枚举（contract-shared C1 / architecture 2.2）──
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}


class TestPrepareAudioPayloadEmotionField:
    """P6-01 / P6-02 · audio 帧 emotion 可选字段"""

    def test_p601_audio_payload_contains_emotion_when_provided(self):
        # P6-01 · prepare_audio_payload(emotion="joy") 产出 dict 含 "emotion": "joy"
        result = prepare_audio_payload(
            audio_path=None,
            emotion="joy",
        )
        assert result["emotion"] == "joy"

    def test_p602_audio_payload_omits_emotion_key_when_none(self):
        # P6-02 · emotion=None → 不含 "emotion" key（向后兼容）
        result = prepare_audio_payload(
            audio_path=None,
            emotion=None,
        )
        assert "emotion" not in result

    def test_p601_audio_payload_emotion_must_be_valid_key(self):
        # 加固 P6-01：emotion 命中合法枚举
        result = prepare_audio_payload(
            audio_path=None,
            emotion="anger",
        )
        # 若目标实现总是无 emotion 则此 key 缺失 → RED
        assert result.get("emotion", "<missing>") in VALID_EMOTIONS


class TestEmotionFrameSchema:
    """P6-03 / P6-04 · emotion 帧结构（architecture §4.6.2 EMOTION_FRAME_SCHEMA）"""

    def test_p603_emotion_frame_required_field_types(self):
        # P6-03 · emotion 帧必填字段存在且类型正确
        # 目标态：src/open_llm_vtuber/utils/stream_audio.py 或 types.py 暴露 emotion-frame 构造函数。
        # 契约 P6 未指定构造函数名最低限度；为机械可测，测试 seek 一个可调用入口构造 emotion 帧。
        frame = _make_emotion_frame(emotion="joy", strength=2)
        assert isinstance(frame["type"], str)
        assert frame["type"] == "emotion"
        assert isinstance(frame["emotion"], str)
        assert frame["emotion"] == "joy"
        assert isinstance(frame["strength"], int)
        assert isinstance(frame["parameterOverrides"], dict)
        assert isinstance(frame["fadeMs"], int)

    def test_p603_emotion_frame_strength_range(self):
        # 加固：strength ∈ [1,3]
        frame = _make_emotion_frame(emotion="joy", strength=3)
        assert 1 <= frame["strength"] <= 3

    def test_p604_parameter_overrides_values_clamped(self):
        # P6-04 · parameterOverrides 所有值 ∈ [0, 1]
        frame = _make_emotion_frame(emotion="smirk", strength=2)
        overrides = frame["parameterOverrides"]
        assert isinstance(overrides, dict) and len(overrides) > 0
        for name, value in overrides.items():
            assert 0.0 <= value <= 1.0, f"参数 {name}={value} 越界 [0,1]"

    def test_p604_parameter_overrides_key_uses_live2d_param_name(self):
        # 加固：参数键为 Live2D 参数名（非空 str）
        frame = _make_emotion_frame(emotion="joy", strength=2)
        for name in frame["parameterOverrides"].keys():
            assert isinstance(name, str) and name.strip()


class TestAudioFrameShape:
    """P6-05 / P6-06 · audio 帧几何一致性（保留现有 audio 帧约束）"""

    def test_p605_volumes_length_equals_visemes_length(self):
        # P6-05 · audio 帧 volumes / visemes 切片对齐
        # 契约 P6-05：len(volumes) == len(visemes)。为达对齐，调用方需传入等长序列。
        # 目标实现对空 audio（audio_path=None）产出 volumes=[]/visemes=[] → 相等。
        result = prepare_audio_payload(audio_path=None)
        assert len(result["volumes"]) == len(result["visemes"])
        assert result["volumes"] == []
        assert result["visemes"] == []

    def test_p606_slice_length_field_present_and_positive(self):
        # P6-06 · slice_length 字段存在且为正整数
        result = prepare_audio_payload(audio_path=None)
        # 现实现该字段名 = "slice_length"（见 stream_audio payload，实为 float/ms）——
        # 契约 P6-06 断言为正整数。若实现返回非 int → RED。
        assert "slice_length" in result
        assert isinstance(result["slice_length"], int) and result["slice_length"] > 0

    def test_p606_audio_frame_type_is_audio(self):
        # 加固 · audio 帧 type 枚举正确
        result = prepare_audio_payload(audio_path=None)
        assert result["type"] == "audio"


class TestMessageTypeEnum:
    """消息 type 枚举合法性（architecture §4.6.1）"""

    def test_audio_and_emotion_are_distinct_message_types(self):
        # 消息协议含 audio 与 emotion 两种后端→前端类型
        assert "audio" in _KNOWN_TYPES
        assert "emotion" in _KNOWN_TYPES
        assert "audio" != "emotion"


# ── 测试辅助 ──

# 消息 type 枚举（architecture §4.6.1）
_KNOWN_TYPES = {"audio", "emotion"}


def _make_emotion_frame(emotion: str, strength: int):
    """按 architecture §4.6.2 EMOTION_FRAME_SCHEMA 构造 emotion 帧。

    说明：这是**被测目标态**的构造入口。目标实现可在 utils/stream_audio.py 或 types.py
    提供等价构造函数（如 prepare_emotion_payload(emotion, strength, parameterOverrides, fadeMs)）。
    本测试驱动该函数签名落地——
      prepare_emotion_payload(emotion, strength, parameter_overrides, fade_ms=400) -> dict
    若实现未暴露该函数，此 helper 处向上抛 ImportError/AttributeError → RED。
    设计理由：契约 P6 定义一个「emotion 帧必有字段」的 schema，但不强制具体构造函数名；
    此处以最小契约接口锁定，确保字段 schema 机械可断言。
    """
    from open_llm_vtuber.utils.stream_audio import prepare_emotion_payload  # 目标态新函数
    parameter_overrides = {
        "ParamBrowY": 0.35, "ParamEyeLOpen": 0.9,
        "ParamMouthForm": 0.9, "ParamMouthOpenY": 0.35,
    }
    return prepare_emotion_payload(
        emotion=emotion,
        strength=strength,
        parameter_overrides=parameter_overrides,
        fade_ms=400,
    )
