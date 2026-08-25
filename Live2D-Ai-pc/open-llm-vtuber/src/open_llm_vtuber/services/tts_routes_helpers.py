import os
from typing import Any

# TTS provider 旧 id → 现 id（审计 R-2：原先这张表在 4 处各写一遍，最易漂移）
TTS_ID_ALIASES = {
    "gpt_sovits": "gpt_sovits_tts",
    "piper": "piper_tts",
    "sherpa_onnx": "sherpa_onnx_tts",
    "pyttsx3": "pyttsx3_tts",
}


def normalize_tts_id(pid: str) -> str:
    """把 TTS provider 的旧 id 归一为现 id（未知 id 原样返回）。"""
    return TTS_ID_ALIASES.get(pid, pid)


def clamp_tts_value(value, lo: float, hi: float, default: float) -> float:
    try:
        v = float(value)
    except (TypeError, ValueError):
        return default
    return max(lo, min(hi, v))


def make_local_tts_params(payload: dict[str, Any]) -> dict[str, Any]:
    """把 Melo TTS 试听请求参数归一化并校验。"""
    language = str(payload.get("language") or payload.get("voice") or "ZH").upper()
    if language not in ("ZH", "EN", "ES", "FR", "JP", "KR"):
        raise ValueError(
            f"不支持的 Melo 语言: {language}，可选 ZH/EN/ES/FR/JP/KR"
        )
    device = str(payload.get("device") or payload.get("model") or "cpu").lower()
    if device not in ("cpu", "auto") and not device.startswith("cuda"):
        device = "cpu"
    speaker = payload.get("speaker") or None
    fmt = str(payload.get("format") or "wav").lower() or "wav"
    return {
        "language": language,
        "device": device,
        "speaker": speaker,
        "speed": clamp_tts_value(payload.get("speed", 1.0), 0.5, 2.0, 1.0),
        "sdp_ratio": clamp_tts_value(payload.get("sdp_ratio", 0.2), 0.0, 1.0, 0.2),
        "noise_scale": clamp_tts_value(payload.get("noise_scale", 0.6), 0.0, 1.0, 0.6),
        "noise_scale_w": clamp_tts_value(payload.get("noise_scale_w", 0.8), 0.0, 1.0, 0.8),
        "format": fmt,
    }


def safe_local_tts_name(name: str, prefix: str) -> str | None:
    """校验本地 TTS 生成的音频文件名，防止目录遍历。"""
    if not name:
        return None
    if name != os.path.basename(name):
        return None
    if ".." in name or name.startswith("."):
        return None
    if not name.startswith(prefix) or not name.endswith(".wav"):
        return None
    return name


def audio_duration_ms(audio_path: str) -> int:
    """获取音频文件时长（毫秒）；失败时回退 0。"""
    try:
        import soundfile as sf

        info = sf.info(audio_path)
        if info.samplerate:
            return int(info.frames * 1000 / info.samplerate)
    except Exception:  # noqa: BLE001, S110
        pass
    try:
        import wave

        with wave.open(audio_path, "rb") as wav_file:
            rate = wav_file.getframerate()
            if rate:
                return int(wav_file.getnframes() * 1000 / rate)
    except Exception:  # noqa: BLE001
        return 0
    return 0
