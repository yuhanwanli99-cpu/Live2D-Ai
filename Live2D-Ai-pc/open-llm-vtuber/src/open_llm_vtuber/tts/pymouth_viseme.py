"""PC 端 pymouth 元音嘴型驱动。

基于 pymouth（MIT 纯 Python DTW 元音识别，无 AI 模型依赖，移动端 CPU
可运行）对 WAV 音频做逐帧元音识别（A/I/U/E/O/SIL），并把每个元音映射为
Live2D 嘴型参数：
    - mouth_open  -> ParamMouthOpenY
    - mouth_form  -> ParamMouthForm

pymouth 未安装、音频非法或处理异常时静默降级：process_audio 返回空列表，
由调用方决定是否在 WS payload 中携带空 visemes 字段。
"""

from __future__ import annotations

import io
from dataclasses import dataclass
from typing import List, Optional

import numpy as np  # noqa: F401  (pymouth analyser 依赖 numpy，保持导入顺序稳定)

try:
    import soundfile as sf
except Exception:  # pragma: no cover - soundfile 缺失时无法解码 WAV
    sf = None

try:
    from pymouth import VowelAnalyser

    PYMOUTH_AVAILABLE = True
except Exception:  # pragma: no cover - pymouth/依赖缺失时静默降级
    VowelAnalyser = None
    PYMOUTH_AVAILABLE = False

# 元音标签 -> (mouth_open, mouth_form)，对应 Live2D ParamMouthOpenY / ParamMouthForm。
# 值域 0.0~1.0：open 越大嘴张得越开，form 越大嘴型越"圆/撮"。
VOWEL_MAP = {
    "A": (0.8, 0.2),
    "I": (0.3, 0.8),
    "U": (0.2, 0.0),
    "E": (0.5, 0.5),
    "O": (0.6, 0.7),
    "SIL": (0.0, 0.0),
}

# pymouth 概率字典键 -> 统一元音标签
_PYMOUTH_KEYS = {
    "VoiceSilence": "SIL",
    "VoiceA": "A",
    "VoiceI": "I",
    "VoiceU": "U",
    "VoiceE": "E",
    "VoiceO": "O",
}

# 默认帧长：pymouth 内部 MFCC 需要至少 ~5 帧（@16kHz 约 160ms）才有识别意义，
# 150ms 在识别率与嘴型动画粒度之间取得平衡。
DEFAULT_FRAME_MS = 150


@dataclass(frozen=True)
class VisemeFrame:
    """一帧元音嘴型数据。"""

    timestamp_ms: int
    vowel_label: str
    mouth_open: float
    mouth_form: float

    def to_dict(self) -> dict:
        """转为 JSON 可序列化字典。"""
        return {
            "timestamp_ms": self.timestamp_ms,
            "vowel_label": self.vowel_label,
            "mouth_open": self.mouth_open,
            "mouth_form": self.mouth_form,
        }


class PymouthVisemeDriver:
    """把 WAV 音频字节流切帧并识别元音，输出 VisemeFrame 序列。"""

    def __init__(self, frame_ms: int = DEFAULT_FRAME_MS, temperature: float = 10.0):
        if frame_ms <= 0:
            raise ValueError("frame_ms must be positive")
        self.frame_ms = int(frame_ms)
        self.temperature = float(temperature)
        # 懒加载并跨调用复用 analyser（process 为纯计算，可安全共享）
        self._analyser: Optional[VowelAnalyser] = None

    @property
    def available(self) -> bool:
        """pymouth 是否可用。"""
        return PYMOUTH_AVAILABLE

    def _get_analyser(self) -> Optional[VowelAnalyser]:
        if not PYMOUTH_AVAILABLE:
            return None
        if self._analyser is None:
            self._analyser = VowelAnalyser(temperature=self.temperature)
        return self._analyser

    def process_audio(self, wav_bytes: bytes) -> List[VisemeFrame]:
        """对 WAV 字节流逐帧识别元音。

        pymouth 缺失、wav_bytes 非法/为空或处理异常时静默降级返回 []。
        """
        if not wav_bytes:
            return []
        if sf is None:
            return []
        try:
            data, samplerate = sf.read(io.BytesIO(wav_bytes), dtype="float32")
        except Exception:
            return []
        if data.size == 0 or samplerate <= 0:
            return []
        if data.ndim > 1:
            # 多声道取第一声道（与 pymouth channel_conversion 行为一致）
            data = data[:, 0]

        analyser = self._get_analyser()
        if analyser is None:
            return self._fallback_frames(data, samplerate)

        frame_size = max(1, int(samplerate * self.frame_ms / 1000))
        frames: List[VisemeFrame] = []
        for start in range(0, data.size, frame_size):
            chunk = data[start : start + frame_size]
            probs = analyser.process(chunk, samplerate)
            label = self._dominant_label(probs)
            mouth_open, mouth_form = VOWEL_MAP[label]
            timestamp_ms = int(start / samplerate * 1000)
            frames.append(
                VisemeFrame(
                    timestamp_ms=timestamp_ms,
                    vowel_label=label,
                    mouth_open=mouth_open,
                    mouth_form=mouth_form,
                )
            )
        return frames

    def _fallback_frames(self, data: np.ndarray, samplerate: int) -> List[VisemeFrame]:
        """pymouth 不可用时对非静音音频生成按帧长递增的 SIL 兜底帧。

        保持 viseme 时间轴连续（timestamp_ms 递增），但嘴型参数为 0。
        纯静音（全零样本）仍返回空列表，避免无声音时产生虚假口型。
        """
        if data.size == 0 or float(np.max(np.abs(data))) <= 0.0:
            return []
        frame_size = max(1, int(samplerate * self.frame_ms / 1000))
        return [
            VisemeFrame(
                timestamp_ms=int(start / samplerate * 1000),
                vowel_label="SIL",
                mouth_open=0.0,
                mouth_form=0.0,
            )
            for start in range(0, data.size, frame_size)
        ]

    @staticmethod
    def _dominant_label(probs: dict) -> str:
        """取概率最大的元音标签；出现未知键时兜底为 SIL。"""
        if not probs:
            return "SIL"
        best_key = max(probs, key=lambda k: probs.get(k, 0.0))
        return _PYMOUTH_KEYS.get(best_key, "SIL")
