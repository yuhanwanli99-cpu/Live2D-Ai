"""PC 端 Actions.motion 序列化 + transformers 接线测试（M8）。

覆盖（契约 M6-M8 §5.1）:
- T1-T4, P22: Actions.to_dict() 序列化（motion 省略/过滤/全字段）
- T5-T9/P16: actions_extractor 接线（结构化 JSON → expressions/motion 派生）
- T10-T12: prepare_audio_payload 含/不含 motion

构造: _PC_SRC 注入 output_types.py / transformers.py / stream_audio.py。
make_wav_bytes 助理由 test_pymouth_viseme.py 复制（生成合法 WAV 用于 payload 测试）。
"""

import io
import os
import sys
import tempfile
import pytest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.agent.output_types import Actions  # noqa: E402
from open_llm_vtuber.utils.stream_audio import prepare_audio_payload  # noqa: E402

# make_wav_bytes + temp file helper（prepare_audio_payload 需要文件路径，非 bytes）
import wave  # noqa: E402
import numpy as np  # noqa: E402


def make_wav_bytes(
    duration_ms: int = 600,
    sample_rate: int = 16000,
    channels: int = 1,
    tone_hz: float = None,
) -> bytes:
    n = int(sample_rate * duration_ms / 1000)
    if tone_hz:
        t = np.arange(n) / sample_rate
        samples = (0.4 * np.sin(2 * np.pi * tone_hz * t) * 32767).astype(np.int16)
    else:
        samples = np.zeros(n, dtype=np.int16)
    if samples.ndim == 1 and channels > 1:
        samples = np.repeat(samples[:, None], channels, axis=1)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        w.writeframes(samples.astype(np.int16).tobytes())
    return buf.getvalue()


def _write_temp_wav(wav_bytes: bytes) -> str:
    """将 WAV 字节流写入临时文件，返回路径（由调用方负责清理）。"""
    tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
    tmp.write(wav_bytes)
    tmp.close()
    return tmp.name


# ---------------------------------------------------------------------------
# T1-T4, P22: Actions.to_dict() 序列化
# ---------------------------------------------------------------------------


class TestActionsToDict:
    def test_t1_motion_serialization(self):
        """T1: Actions(motion="wave").to_dict() == {"motion":"wave"}"""
        result = Actions(motion="wave").to_dict()
        assert result == {"motion": "wave"}

    def test_t2_no_motion(self):
        """T2: Actions().to_dict() == {}（无 motion 键，省略语义）"""
        result = Actions().to_dict()
        assert result == {}
        assert "motion" not in result

    def test_t3_mixed_with_expressions(self):
        """T3: Actions(expressions=[0], motion="wave").to_dict() 含全字段"""
        result = Actions(expressions=[0], motion="wave").to_dict()
        assert result == {"expressions": [0], "motion": "wave"}

    def test_t4_explicit_none(self):
        """T4: Actions(motion=None).to_dict() == {}（None 被过滤）"""
        result = Actions(motion=None).to_dict()
        assert result == {}
        assert "motion" not in result

    def test_p22_full_fields(self):
        """P22: Actions(expressions=[5,7], motion="wave",
        parameterOverrides={"ParamEyeLOpen":1.2}).to_dict() 全字段命中。"""
        result = Actions(
            expressions=[5, 7],
            motion="wave",
            parameterOverrides={"ParamEyeLOpen": 1.2},
        ).to_dict()
        assert "expressions" in result
        assert result["expressions"] == [5, 7]
        assert result["motion"] == "wave"
        assert result["parameterOverrides"] == {"ParamEyeLOpen": 1.2}

    def test_p22_none_omitted(self):
        """P22: None 字段不进 payload。"""
        result = Actions(
            expressions=[5],
            motion=None,
            parameterOverrides=None,
        ).to_dict()
        assert "motion" not in result
        assert "parameterOverrides" not in result
        assert "expressions" in result


# ---------------------------------------------------------------------------
# T10-T12: prepare_audio_payload 含 motion
# ---------------------------------------------------------------------------


class TestAudioPayloadMotion:
    def test_t10_payload_with_motion(self):
        """T10: payload["actions"]["motion"]=="wave"；visemes 仍为 list。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=Actions(motion="wave"))
            assert payload["actions"] is not None
            assert payload["actions"]["motion"] == "wave"
            assert isinstance(payload["visemes"], list)
        finally:
            os.unlink(wav_path)

    def test_t11_payload_no_motion(self):
        """T11: actions=Actions() → payload["actions"]=={}。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=Actions())
            assert payload["actions"] == {}
        finally:
            os.unlink(wav_path)

    def test_t12_payload_actions_none(self):
        """T12: actions=None → payload["actions"] is None。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=None)
            assert payload["actions"] is None
        finally:
            os.unlink(wav_path)
