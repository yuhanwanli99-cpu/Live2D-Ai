```python
"""
Edge TTS 引擎统一封装模块。

支持流式（异步生成器）和非流式（直接写入文件）两种调用方式。
语速和音调参数可配置，格式遵循 edge_tts 库规范（如 "+0%", "-20%", "+0Hz", "-50Hz"）。

依赖:
    pip install edge-tts
"""

import asyncio
from typing import AsyncGenerator, Optional

import edge_tts

__all__ = [
    "EdgeTTS",
    "generate_speech",
    "generate_speech_stream",
]


class EdgeTTS:
    """
    Edge TTS 引擎封装类。
    提供统一的 TTS 调用接口，支持语速和音调调节。
    """

    def __init__(self, voice: str = "zh-CN-XiaoxiaoNeural", rate: str = "+0%", pitch: str = "+0Hz"):
        """
        初始化 TTS 引擎。

        Args:
            voice: 语音名称，默认为中文女声 Xiaoxiao。
            rate: 语速，格式如 "+0%", "-20%", "+30%"。
            pitch: 音调，格式如 "+0Hz", "-50Hz", "+100Hz"。
        """
        self.voice = voice
        self.rate = rate
        self.pitch = pitch

    async def generate_speech(self, text: str, voice: Optional[str] = None,
                              rate: Optional[str] = None, pitch: Optional[str] = None,
                              output_file: str = "output.mp3") -> None:
        """
        非流式合成语音，直接保存到文件。

        Args:
            text: 待合成的文本。
            voice: 覆盖构造时设置的语音。
            rate: 覆盖构造时设置的语速。
            pitch: 覆盖构造时设置的音调。
            output_file: 输出音频文件路径，支持 .mp3 或 .wav。
        """
        _voice = voice or self.voice
        _rate = rate or self.rate
        _pitch = pitch or self.pitch

        communicate = edge_tts.Communicate(text, _voice, rate=_rate, pitch=_pitch)
        await communicate.save(output_file)

    async def generate_speech_stream(self, text: str, voice: Optional[str] = None,
                                     rate: Optional[str] = None, pitch: Optional[str] = None) -> AsyncGenerator[bytes, None]:
        """
        流式合成语音，返回音频数据块的异步生成器。

        Args:
            text: 待合成的文本。
            voice: 覆盖构造时设置的语音。
            rate: 覆盖构造时设置的语速。
            pitch: 覆盖构造时设置的音调。

        Yields:
            音频数据块（bytes），可直接传给播放器或流式响应。
        """
        _voice = voice or self.voice
        _rate = rate or self.rate
        _pitch = pitch or self.pitch

        communicate = edge_tts.Communicate(text, _voice, rate=_rate, pitch=_pitch)
        async for chunk in communicate.stream():
            if chunk["type"] == "audio":
                yield chunk["data"]


# 为了兼容简单调用场景，提供模块级快捷函数

_default_engine = EdgeTTS()


async def generate_speech(text: str, voice: str = "zh-CN-XiaoxiaoNeural",
                          output_file: str = "output.mp3",
                          rate: str = "+0%", pitch: str = "+0Hz") -> None:
    """
    模块级快捷函数：非流式合成语音并保存文件。

    Args:
        text: 文本。
        voice: 语音名称。
        output_file: 输出文件路径。
        rate: 语速，如 "+0%", "-20%"。
        pitch: 音调，如 "+0Hz", "-50Hz"。
    """
    await _default_engine.generate_speech(text, voice=voice, rate=rate, pitch=pitch, output_file=output_file)


async def generate_speech_stream(text: str, voice: str = "zh-CN-XiaoxiaoNeural",
                                 rate: str = "+0%", pitch: str = "+0Hz") -> AsyncGenerator[bytes, None]:
    """
    模块级快捷函数：流式合成语音，返回异步生成器。

    Args:
        text: 文本。
        voice: 语音名称。
        rate: 语速。
        pitch: 音调。

    Yields:
        音频数据块。
    """
    async for chunk in _default_engine.generate_speech_stream(text, voice=voice, rate=rate, pitch=pitch):
        yield chunk

```