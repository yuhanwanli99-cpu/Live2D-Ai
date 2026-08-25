# config_manager/system.py
from pydantic import Field, model_validator
from typing import Dict, ClassVar
from .i18n import I18nMixin, Description


class SystemConfig(I18nMixin):
    """System configuration settings."""

    conf_version: str = Field(..., alias="conf_version")
    host: str = Field(..., alias="host")
    port: int = Field(..., alias="port")
    config_alts_dir: str = Field(..., alias="config_alts_dir")
    tool_prompts: Dict[str, str] = Field(..., alias="tool_prompts")
    enable_proxy: bool = Field(False, alias="enable_proxy")
    live2d_fps: int = Field(60, alias="live2d_fps")
    # 独立导演模型配置（PC v4）：留空时复用主对话脑 provider/model。
    director_llm_provider: str | None = Field(None, alias="director_llm_provider")
    director_model: str | None = Field(None, alias="director_model")
    # 开发者模式：仅 localhost 可访问的调试用 API 与 UI（默认关闭）
    developer_mode: bool = Field(False, alias="developer_mode")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "conf_version": Description(en="Configuration version", zh="配置文件版本"),
        "host": Description(en="Server host address", zh="服务器主机地址"),
        "port": Description(en="Server port number", zh="服务器端口号"),
        "config_alts_dir": Description(
            en="Directory for alternative configurations", zh="备用配置目录"
        ),
        "tool_prompts": Description(
            en="Tool prompts to be inserted into persona prompt",
            zh="要插入到角色提示词中的工具提示词",
        ),
        "enable_proxy": Description(
            en="Enable proxy mode for multiple clients",
            zh="启用代理模式以支持多个客户端使用一个 ws 连接",
        ),
        "live2d_fps": Description(
            en="Live2D rendering target frame rate (default 60)",
            zh="Live2D 渲染目标帧率（默认 60）",
        ),
        "developer_mode": Description(
            en="Enable local-only developer tools (default false)",
            zh="启用仅本地可访问的开发者调试工具（默认关闭）",
        ),
    }

    @model_validator(mode="after")
    def check_port(cls, values):
        port = values.port
        if port < 0 or port > 65535:
            raise ValueError("Port must be between 0 and 65535")
        return values