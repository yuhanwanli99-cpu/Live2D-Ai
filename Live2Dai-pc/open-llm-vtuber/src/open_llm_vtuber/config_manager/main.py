# config_manager/main.py
from pydantic import BaseModel, Field
from typing import Dict, ClassVar, Optional

from .system import SystemConfig
from .character import CharacterConfig
from .live import LiveConfig
from .vision import VisionConfig
from .i18n import I18nMixin, Description


class Config(I18nMixin, BaseModel):
    """
    Main configuration for the application.
    """

    system_config: SystemConfig = Field(default=None, alias="system_config")
    character_config: CharacterConfig = Field(..., alias="character_config")
    live_config: LiveConfig = Field(default=LiveConfig(), alias="live_config")
    vision_config: Optional[VisionConfig] = Field(default=None, alias="vision_config")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "system_config": Description(
            en="System configuration settings", zh="系统配置设置"
        ),
        "character_config": Description(
            en="Character configuration settings", zh="角色配置设置"
        ),
        "live_config": Description(
            en="Live streaming platform integration settings", zh="直播平台集成设置"
        ),
        "vision_config": Description(
            en="Vision/screenshot pipeline configuration", zh="视觉/截图管道配置"
        ),
    }
