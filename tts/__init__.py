```python
"""
TTS 模块统一入口。

导出 Edge TTS 引擎的公共接口。
"""

from .edge_tts_engine import (
    EdgeTTS,
    generate_speech,
    generate_speech_stream,
)

__all__ = [
    "EdgeTTS",
    "generate_speech",
    "generate_speech_stream",
]

```