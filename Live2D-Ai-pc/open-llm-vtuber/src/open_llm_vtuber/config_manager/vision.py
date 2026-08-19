"""
Vision configuration model for GLM-4.6V vision pipeline.

This config defines how screenshots are captured and sent to a vision model
(e.g. Zhipu GLM-4.6V) for context-aware conversation.
"""

from pydantic import BaseModel, Field
from typing import Dict, ClassVar, Optional
from .i18n import I18nMixin, Description


class VisionConfig(I18nMixin, BaseModel):
    """
    Configuration for the vision/screenshot pipeline.

    When vision_enabled is True, the system will periodically capture
    screenshots and send them to the configured vision model (default: glm-4v)
    to provide visual context to the conversation agent.
    """

    vision_enabled: bool = Field(False, alias="vision_enabled")
    provider: str = Field("zhipu", alias="provider")
    api_key: Optional[str] = Field(None, alias="api_key")
    model: str = Field("glm-4v", alias="model")
    capture_interval: float = Field(5.0, alias="capture_interval")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "vision_enabled": Description(
            en="Enable vision/screenshot pipeline (default: False)",
            zh="启用视觉/截图管道（默认：False）",
        ),
        "provider": Description(
            en="Vision provider (e.g. 'zhipu' for GLM-4V)",
            zh="视觉提供者（如 'zhipu' 对应 GLM-4V）",
        ),
        "api_key": Description(
            en="API key for the vision provider (falls back to zhipu_llm key if empty)",
            zh="视觉提供者的 API 密钥（留空则回退到 zhipu_llm 的密钥）",
        ),
        "model": Description(
            en="Vision model name (default: glm-4v)",
            zh="视觉模型名称（默认：glm-4v）",
        ),
        "capture_interval": Description(
            en="Interval in seconds between screenshot captures (default: 5.0)",
            zh="截图捕获间隔（秒）（默认：5.0）",
        ),
    }
