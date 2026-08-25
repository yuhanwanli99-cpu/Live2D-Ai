"""PC 端 pymouth 元音嘴型单元测试（pymouth_viseme + stream_audio 接线）。

覆盖（真实 pymouth 路径 + mock 确定性路径，pymouth 缺失时静默降级）:
- VOWEL_MAP 映射契约: A/I/U/E/O/SIL -> (mouth_open, mouth_form)
- VisemeFrame 序列化（JSON 可序列化）
- PymouthVisemeDriver.process_audio:
    * 空/非法音频、pymouth 缺失 -> 静默降级 []
    * 静音音频 -> 全 SIL 帧、时间戳递增
    * 真实 TTS 音频（fixture）-> 存在非 SIL 元音标签
    * 多声道取第一声道、analyser 懒加载复用
- stream_audio.prepare_audio_payload:
    * 正常音频 payload 携带 visemes 列表
    * 静音 payload visemes 为空
    * pymouth 失败时 visemes 为空（静默降级）
"""
import io
import json
import os
import sys
import tempfile
import unittest
import wave
from unittest.mock import patch

import numpy as np

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.tts.pymouth_viseme import (  # noqa: E402
    PYMOUTH_AVAILABLE,
    DEFAULT_FRAME_MS,
    VOWEL_MAP,
    PymouthVisemeDriver,
    VisemeFrame,
)
from open_llm_vtuber.utils.stream_audio import (  # noqa: E402
    prepare_audio_payload,
)

_FIXTURE_DIR = os.path.join(os.path.dirname(__file__), "fixtures")
_TTS_FIXTURE = os.path.join(_FIXTURE_DIR, "tts_vowels.wav")


def make_wav_bytes(
    duration_ms: int = 600,
    sample_rate: int = 16000,
    channels: int = 1,
    samples: np.ndarray = None,
    tone_hz: float = None,
) -> bytes:
    """用标准库 wave 生成 PCM16 WAV 字节流（不依赖 ffmpeg）。

    tone_hz 非空时生成正弦波（避免 RMS 全零导致 volumes 计算失败）。
    """
    n = int(sample_rate * duration_ms / 1000)
    if samples is None:
        if tone_hz:
            t = np.arange(n) / sample_rate
            samples = (0.4 * np.sin(2 * np.pi * tone_hz * t) * 32767).astype(
                np.int16
            )
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


def _fake_analyser(label_probs):
    """返回 process 返回固定概率字典的伪 analyser。"""

    class _Fake:
        def process(self, data, samplerate):
            return dict(label_probs)

    return _Fake()


class VowelMapTest(unittest.TestCase):
    def test_vowel_map_contract(self):
        """VOWEL_MAP 必须覆盖 A/I/U/E/O/SIL 且参数在 [0,1] 内。"""
        self.assertEqual(
            set(VOWEL_MAP.keys()), {"A", "I", "U", "E", "O", "SIL"}
        )
        for label, (mouth_open, mouth_form) in VOWEL_MAP.items():
            self.assertGreaterEqual(mouth_open, 0.0)
            self.assertLessEqual(mouth_open, 1.0)
            self.assertGreaterEqual(mouth_form, 0.0)
            self.assertLessEqual(mouth_form, 1.0)

    def test_vowel_map_expected_values(self):
        """映射值与设计文档一致（A 大张口 / I 抿嘴 / SIL 闭合）。"""
        self.assertEqual(VOWEL_MAP["A"], (0.8, 0.2))
        self.assertEqual(VOWEL_MAP["I"], (0.3, 0.8))
        self.assertEqual(VOWEL_MAP["U"], (0.2, 0.0))
        self.assertEqual(VOWEL_MAP["E"], (0.5, 0.5))
        self.assertEqual(VOWEL_MAP["O"], (0.6, 0.7))
        self.assertEqual(VOWEL_MAP["SIL"], (0.0, 0.0))


class VisemeFrameTest(unittest.TestCase):
    def test_to_dict(self):
        frame = VisemeFrame(
            timestamp_ms=150, vowel_label="A", mouth_open=0.8, mouth_form=0.2
        )
        self.assertEqual(
            frame.to_dict(),
            {
                "timestamp_ms": 150,
                "vowel_label": "A",
                "mouth_open": 0.8,
                "mouth_form": 0.2,
            },
        )

    def test_to_dict_json_serializable(self):
        frame = VisemeFrame(
            timestamp_ms=300, vowel_label="O", mouth_open=0.6, mouth_form=0.7
        )
        # WS 消息会走 json.dumps，必须可直接序列化
        json.dumps(frame.to_dict())


class DriverConfigTest(unittest.TestCase):
    def test_default_frame_ms(self):
        self.assertEqual(DEFAULT_FRAME_MS, 150)

    def test_frame_ms_must_be_positive(self):
        with self.assertRaises(ValueError):
            PymouthVisemeDriver(frame_ms=0)
        with self.assertRaises(ValueError):
            PymouthVisemeDriver(frame_ms=-50)

    def test_available_property(self):
        driver = PymouthVisemeDriver()
        # 与模块级探测结果一致（不强制为 True，pymouth 缺失环境允许降级）
        self.assertEqual(driver.available, PYMOUTH_AVAILABLE)

    def test_analyser_lazy_and_reused(self):
        """analyser 懒加载且跨调用复用同一实例。"""
        driver = PymouthVisemeDriver()
        self.assertIsNone(driver._analyser)
        first = driver._get_analyser()
        if first is None:  # pymouth 缺失时跳过
            self.skipTest("pymouth unavailable")
        second = driver._get_analyser()
        self.assertIs(first, second)


class ProcessAudioDegradeTest(unittest.TestCase):
    def test_empty_bytes_returns_empty(self):
        driver = PymouthVisemeDriver()
        self.assertEqual(driver.process_audio(b""), [])

    def test_invalid_bytes_returns_empty(self):
        driver = PymouthVisemeDriver()
        self.assertEqual(driver.process_audio(b"not a wav file"), [])

    def test_pymouth_missing_returns_empty(self):
        """pymouth 缺失（PYMOUTH_AVAILABLE=False）时静默降级为 []。"""
        wav = make_wav_bytes(duration_ms=600)
        with patch(
            "open_llm_vtuber.tts.pymouth_viseme.PYMOUTH_AVAILABLE", False
        ):
            driver = PymouthVisemeDriver()
            self.assertFalse(driver.available)
            self.assertEqual(driver.process_audio(wav), [])


class ProcessAudioRealTest(unittest.TestCase):
    @unittest.skipUnless(PYMOUTH_AVAILABLE, "pymouth 未安装")
    def test_silent_wav_all_silence_frames(self):
        """静音 WAV：全为 SIL 帧，参数为 0，时间戳按帧长递增。"""
        driver = PymouthVisemeDriver(frame_ms=150)
        frames = driver.process_audio(make_wav_bytes(duration_ms=600))
        self.assertEqual(len(frames), 4)
        for i, frame in enumerate(frames):
            self.assertEqual(frame.vowel_label, "SIL")
            self.assertEqual((frame.mouth_open, frame.mouth_form), (0.0, 0.0))
            self.assertEqual(frame.timestamp_ms, i * 150)

    @unittest.skipUnless(PYMOUTH_AVAILABLE, "pymouth 未安装")
    def test_real_tts_fixture_yields_non_silence_labels(self):
        """真实 TTS 语音（fixture）能识别出非 SIL 元音标签。"""
        with open(_TTS_FIXTURE, "rb") as f:
            wav = f.read()
        driver = PymouthVisemeDriver(frame_ms=150)
        frames = driver.process_audio(wav)
        self.assertGreater(len(frames), 0)
        non_silence = [f.vowel_label for f in frames if f.vowel_label != "SIL"]
        self.assertGreater(len(non_silence), 0, "真实语音应识别出非静音元音")

    @unittest.skipUnless(PYMOUTH_AVAILABLE, "pymouth 未安装")
    def test_stereo_wav_uses_first_channel(self):
        """多声道 WAV 取第一声道处理，帧数与单声道一致。"""
        mono = make_wav_bytes(duration_ms=300)
        stereo = make_wav_bytes(duration_ms=300, channels=2)
        driver = PymouthVisemeDriver(frame_ms=150)
        frames_mono = driver.process_audio(mono)
        frames_stereo = driver.process_audio(stereo)
        self.assertEqual(len(frames_mono), 2)
        self.assertEqual(len(frames_stereo), 2)
        for f in frames_stereo:
            self.assertEqual(f.vowel_label, "SIL")


class ProcessAudioMockTest(unittest.TestCase):
    """用伪 analyser 做确定性验证（不依赖真实 pymouth 识别结果）。"""

    def test_label_mapping_applies_vowel_map(self):
        """识别出的元音标签应映射到 VOWEL_MAP 参数。"""
        probs = {
            "VoiceSilence": 0.1,
            "VoiceA": 0.7,
            "VoiceI": 0.05,
            "VoiceU": 0.05,
            "VoiceE": 0.05,
            "VoiceO": 0.05,
        }
        wav = make_wav_bytes(duration_ms=300)
        driver = PymouthVisemeDriver(frame_ms=150)
        with patch.object(
            driver, "_get_analyser", return_value=_fake_analyser(probs)
        ):
            frames = driver.process_audio(wav)
        self.assertEqual(len(frames), 2)
        for frame in frames:
            self.assertEqual(frame.vowel_label, "A")
            self.assertEqual(frame.mouth_open, VOWEL_MAP["A"][0])
            self.assertEqual(frame.mouth_form, VOWEL_MAP["A"][1])

    def test_timestamps_increment_by_frame_ms(self):
        """时间戳按 frame_ms 递增，末帧按实际起点截断。"""
        probs = {
            "VoiceSilence": 0.9,
            "VoiceA": 0.02,
            "VoiceI": 0.02,
            "VoiceU": 0.02,
            "VoiceE": 0.02,
            "VoiceO": 0.02,
        }
        # 1000ms @ 16kHz / 150ms 帧 = 7 帧
        wav = make_wav_bytes(duration_ms=1000)
        driver = PymouthVisemeDriver(frame_ms=150)
        with patch.object(
            driver, "_get_analyser", return_value=_fake_analyser(probs)
        ):
            frames = driver.process_audio(wav)
        self.assertEqual(len(frames), 7)
        self.assertEqual([f.timestamp_ms for f in frames], [0, 150, 300, 450, 600, 750, 900])

    def test_custom_frame_ms(self):
        """自定义 frame_ms=200 时帧数按 200ms 切分。"""
        probs = {
            "VoiceSilence": 1.0,
        }
        wav = make_wav_bytes(duration_ms=1000)
        driver = PymouthVisemeDriver(frame_ms=200)
        with patch.object(
            driver, "_get_analyser", return_value=_fake_analyser(probs)
        ):
            frames = driver.process_audio(wav)
        self.assertEqual(len(frames), 5)
        self.assertEqual([f.timestamp_ms for f in frames], [0, 200, 400, 600, 800])

    def test_mono_conversion_passes_channel0(self):
        """stereo 输入在进入 analyser 前应被降为第一声道（1-D）。"""
        seen = []

        class _Recording:
            def process(self, data, samplerate):
                seen.append(np.asarray(data).shape)
                return {"VoiceSilence": 1.0}

        stereo = make_wav_bytes(duration_ms=300, channels=2)
        driver = PymouthVisemeDriver(frame_ms=150)
        with patch.object(driver, "_get_analyser", return_value=_Recording()):
            driver.process_audio(stereo)
        self.assertEqual(seen, [(2400,), (2400,)])


class DominantLabelTest(unittest.TestCase):
    def test_argmax_label(self):
        probs = {
            "VoiceSilence": 0.1,
            "VoiceA": 0.2,
            "VoiceI": 0.5,
            "VoiceU": 0.05,
            "VoiceE": 0.1,
            "VoiceO": 0.05,
        }
        self.assertEqual(PymouthVisemeDriver._dominant_label(probs), "I")

    def test_silence_dominant(self):
        probs = {
            "VoiceSilence": 0.8,
            "VoiceA": 0.04,
            "VoiceI": 0.04,
            "VoiceU": 0.04,
            "VoiceE": 0.04,
            "VoiceO": 0.04,
        }
        self.assertEqual(PymouthVisemeDriver._dominant_label(probs), "SIL")

    def test_unknown_keys_fallback_to_sil(self):
        self.assertEqual(PymouthVisemeDriver._dominant_label({"VoiceX": 1.0}), "SIL")
        self.assertEqual(PymouthVisemeDriver._dominant_label({}), "SIL")


class PrepareAudioPayloadTest(unittest.TestCase):
    def _write_tmp_wav(self, duration_ms=300):
        """写一个临时 WAV 文件（含音调，避免 RMS 全零）供 prepare_audio_payload 读取。"""
        fd, path = tempfile.mkstemp(suffix=".wav")
        os.close(fd)
        with open(path, "wb") as f:
            f.write(
                make_wav_bytes(duration_ms=duration_ms, tone_hz=440.0)
            )
        self.addCleanup(os.unlink, path)
        return path

    def test_payload_contains_visemes_list(self):
        """pymouth 模式：正常音频 payload 应携带 visemes 列表（元素为 dict）。"""
        path = self._write_tmp_wav(duration_ms=600)
        payload = prepare_audio_payload(
            audio_path=path, chunk_length_ms=100, viseme_mode="pymouth"
        )
        self.assertIn("visemes", payload)
        self.assertIsInstance(payload["visemes"], list)
        self.assertGreater(len(payload["visemes"]), 0)
        frame = payload["visemes"][0]
        self.assertEqual(
            set(frame.keys()),
            {"timestamp_ms", "vowel_label", "mouth_open", "mouth_form"},
        )
        self.assertIn(frame["vowel_label"], VOWEL_MAP)

    def test_payload_json_serializable(self):
        """携带 visemes 的 payload 可直接 json.dumps。"""
        path = self._write_tmp_wav(duration_ms=300)
        payload = prepare_audio_payload(audio_path=path)
        json.dumps(payload)

    def test_rms_mode_returns_volume_visemes_without_pymouth(self):
        """默认 rms 模式：不跑 pymouth，直接用 volumes 生成 mouth_open_y。"""
        path = self._write_tmp_wav(duration_ms=300)
        payload = prepare_audio_payload(audio_path=path, viseme_mode="rms")
        self.assertEqual(len(payload["volumes"]), len(payload["visemes"]))
        self.assertIn("mouth_open_y", payload["visemes"][0])
        self.assertIn("mouth_form", payload["visemes"][0])

    def test_off_mode_returns_empty_visemes(self):
        """off 模式：完全不生成 viseme，前端仍可用 volumes 驱动嘴部开合。"""
        path = self._write_tmp_wav(duration_ms=300)
        payload = prepare_audio_payload(audio_path=path, viseme_mode="off")
        self.assertEqual(payload["visemes"], [])
        self.assertGreater(len(payload["volumes"]), 0)

    def test_silent_payload_visemes_empty(self):
        """audio_path=None 的静音 payload visemes 为空列表。"""
        payload = prepare_audio_payload(audio_path=None)
        self.assertEqual(payload["audio"], None)
        self.assertEqual(payload["visemes"], [])

    def test_visemes_degrade_when_driver_fails(self):
        """pymouth 处理失败时 visemes 静默降级为空列表。"""
        path = self._write_tmp_wav(duration_ms=300)
        with patch(
            "open_llm_vtuber.utils.stream_audio._viseme_driver.process_audio",
            return_value=[],
        ):
            payload = prepare_audio_payload(audio_path=path, viseme_mode="pymouth")
        self.assertEqual(payload["visemes"], [])
        # 其余字段不受影响
        self.assertIsNotNone(payload["audio"])


if __name__ == "__main__":
    unittest.main()
