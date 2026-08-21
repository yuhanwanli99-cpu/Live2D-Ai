import base64
import threading
import time

from typing import Optional
from pydub import AudioSegment
from pydub.utils import make_chunks
from ..agent.output_types import Actions
from ..logsetup import get_logger

# 本模块全部记录带 "audio" 标签
logger = get_logger("audio")
from ..agent.output_types import DisplayText
from ..tts.pymouth_viseme import PymouthVisemeDriver

# 进程级复用的元音嘴型驱动器：pymouth 缺失/失败时 process_audio 返回 []，静默降级
# prepare_audio_payload 已移入线程池执行，pymouth analyser 非线程安全，需加锁串行化。
_viseme_driver = PymouthVisemeDriver()
_viseme_lock = threading.Lock()

# 合法 emotion key 枚举（contract-pc-pipeline.md §P6 / emotion-protocol C1 8 key）
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}

# emotion 帧 schema 常量（architecture §4.6.2 EMOTION_FRAME_SCHEMA）
EMOTION_FRAME_SCHEMA = {
    "type": str,              # 固定 "emotion"
    "emotion": str,           # 8 key 枚举
    "strength": int,          # 1-3
    "parameterOverrides": dict,  # {"ParamName": float 0..1, ...}
    "fadeMs": int,            # 默认 400
}


def prepare_emotion_payload(
    emotion: str,
    strength: int = 2,
    parameter_overrides: Optional[dict] = None,
    fade_ms: int = 400,
) -> dict:
    """构造 emotion WS 帧（architecture §4.6.2 EMOTION_FRAME_SCHEMA）。

    P6-03/P6-04：
      - type 固定 "emotion"
      - emotion 必须是 8 合法 key 之一（非法 → 忽略并回落 neutral，不抛异常）
      - strength 钳制到 [1, 3]（非法值回落默认 2）
      - parameterOverrides 值钳制到 [0, 1]（key 为 Live2D 参数名）
      - fadeMs 默认 400
    """
    if emotion not in VALID_EMOTIONS:
        logger.warning(
            f"prepare_emotion_payload: 未知 emotion '{emotion}'，回落 neutral"
        )
        emotion = "neutral"

    try:
        strength = int(strength)
    except (TypeError, ValueError):
        strength = 2
    strength = max(1, min(3, strength))

    overrides = {}
    for name, value in (parameter_overrides or {}).items():
        try:
            value = float(value)
        except (TypeError, ValueError):
            logger.warning(f"prepare_emotion_payload: 忽略非法参数值 {name}={value!r}")
            continue
        overrides[str(name)] = max(0.0, min(1.0, value))

    return {
        "type": "emotion",
        "emotion": emotion,
        "strength": strength,
        "parameterOverrides": overrides,
        "fadeMs": int(fade_ms),
    }


def _get_volume_by_chunks(audio: AudioSegment, chunk_length_ms: int) -> list:
    """
    Calculate the normalized volume (RMS) for each chunk of the audio.

    Parameters:
        audio (AudioSegment): The audio segment to process.
        chunk_length_ms (int): The length of each audio chunk in milliseconds.

    Returns:
        list: Normalized volumes for each chunk.
    """
    chunks = make_chunks(audio, chunk_length_ms)
    volumes = [chunk.rms for chunk in chunks]
    max_volume = max(volumes)
    if max_volume == 0:
        raise ValueError("Audio is empty or all zero.")
    return [volume / max_volume for volume in volumes]


def prepare_audio_payload(
    audio_path: str | None,
    chunk_length_ms: int = 20,
    display_text: DisplayText = None,
    actions: Actions = None,
    forwarded: bool = False,
    emotion: Optional[str] = None,
    viseme_mode: str = "rms",
) -> dict[str, any]:
    """
    Prepares the audio payload for sending to a broadcast endpoint.
    If audio_path is None, returns a payload with audio=None for silent display.

    Parameters:
        audio_path (str | None): The path to the audio file to be processed, or None for silent display
        chunk_length_ms (int): The length of each audio chunk in milliseconds
        display_text (DisplayText, optional): Text to be displayed with the audio
        actions (Actions, optional): Actions associated with the audio
        emotion (str, optional): P6 新增可选字段——当前句子 emotion（8 key 枚举）。
            提供且合法 → payload 含 "emotion" key；None/非法 → 不含（向后兼容）。

    Returns:
        dict: The audio payload to be sent
    """
    if isinstance(display_text, DisplayText):
        display_text = display_text.to_dict()

    # P6-01/P6-02：emotion 可选字段。非法 key → 忽略（不输出），保持向后兼容。
    if emotion is not None and emotion not in VALID_EMOTIONS:
        logger.warning(
            f"prepare_audio_payload: 未知 emotion '{emotion}'，忽略该字段"
        )
        emotion = None

    if not audio_path:
        # Return payload for silent display
        payload = {
            "type": "audio",
            "audio": None,
            "volumes": [],
            "slice_length": int(chunk_length_ms),
            "visemes": [],
            "display_text": display_text,
            "actions": actions.to_dict() if actions else None,
            "forwarded": forwarded,
        }
        if emotion is not None:
            payload["emotion"] = emotion
        return payload

    _t0 = time.monotonic()
    try:
        audio = AudioSegment.from_file(audio_path)
        _t_pydub = time.monotonic()
        audio_bytes = audio.export(format="wav").read()
    except Exception as e:
        raise ValueError(
            f"Error loading or converting generated audio file to wav file '{audio_path}': {e}"
        )
    _t_export = time.monotonic()
    audio_base64 = base64.b64encode(audio_bytes).decode("utf-8")
    _t_base64 = time.monotonic()
    volumes = _get_volume_by_chunks(audio, chunk_length_ms)
    _t_volume = time.monotonic()

    # 口型模式（默认 rms，NEKO 同款快速音量驱动）：
    #   rms     → 直接用 volumes 生成 mouth_open_y，不跑 pymouth（毫秒级）
    #   off     → 完全关闭 viseme，前端仍可用 volumes 驱动嘴部开合
    #   pymouth → 精细元音识别（慢，可选）
    if viseme_mode == "pymouth":
        # pymouth 元音嘴型：缺失/失败时静默降级为空列表（线程池并发下加锁串行）
        with _viseme_lock:
            visemes = [
                frame.to_dict()
                for frame in _viseme_driver.process_audio(audio_bytes)
            ]
    elif viseme_mode == "rms":
        visemes = [
            {"mouth_open_y": volume, "mouth_form": 0.0}
            for volume in volumes
        ]
    else:  # off
        visemes = []
    _t_viseme = time.monotonic()

    # 逐 chunk 的耗时明细属于排障信息：降为 DEBUG、合并为一行可读格式
    logger.debug(
        "音频封装完成 总耗时={:.0f}ms（解码 {:.0f} / 编码 {:.0f} / base64 {:.0f} / "
        "音量 {:.0f} / 口型 {:.0f}）mode={}",
        (_t_viseme - _t0) * 1000,
        (_t_pydub - _t0) * 1000,
        (_t_export - _t_pydub) * 1000,
        (_t_base64 - _t_export) * 1000,
        (_t_volume - _t_base64) * 1000,
        (_t_viseme - _t_volume) * 1000,
        viseme_mode,
    )

    payload = {
        "type": "audio",
        "audio": audio_base64,
        "volumes": volumes,
        "slice_length": int(chunk_length_ms),
        "visemes": visemes,
        "display_text": display_text,
        "actions": actions.to_dict() if actions else None,
        "forwarded": forwarded,
    }
    if emotion is not None:
        payload["emotion"] = emotion

    return payload


# Example usage:
# payload, duration = prepare_audio_payload("path/to/audio.mp3", display_text="Hello", expression_list=[0,1,2])
