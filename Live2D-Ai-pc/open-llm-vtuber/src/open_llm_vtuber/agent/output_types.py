from abc import ABC, abstractmethod
from dataclasses import asdict, dataclass


@dataclass
class Actions:
    """Represents actions that can be performed alongside text output"""

    expressions: list[str] | list[int] | None = None
    pictures: list[str] | None = None
    sounds: list[str] | None = None
    motion: str | None = None  # M1 §3.4 预锁：motionLabel 别名；M3 从 motionLabel 填充，M8 前端消费
    motionStrength: int | None = None  # 导演强度 1/2/3；渲染端映射为确定性动作幅度
    parameterOverrides: dict | None = None  # 兼容：模型参数绝对值直通
    # 无预设半身语义意图：headX/gazeX/bodyX 等归一化通道；渲染端按模型真实范围映射。
    # 口型不进入此字段，继续由 LipSync 单一来源持有。
    halfBody: dict | None = None
    # 括注动作帧（my-neuro 式"播放位置触发"）：[{"at": float, "motion": str}, ...]
    # at ∈ [0,1) 为所在 TTS 分块的归一化播放进度；由 director.stage_gesture_timeline
    # 解析、conversation_utils 按 split_tts_chunks 切片。存在时前端按进度调度触发，
    # 忽略一次性 motion 字段（避免双重触发）；to_dict() 对 None 自动省略，向后兼容。
    motionTimeline: list[dict] | None = None
    # —— Preview 协议可选元数据（旧客户端忽略未知字段；None 时 to_dict 省略，向后兼容）——
    # 动作来源：user_command / stage_direction / director / emotion / plugin 等，供前端
    # 调度优先级与开发者面板展示。
    source: str | None = None
    # 调度优先级：数值越大越优先（显式用户命令=100、括注时间轴=60、导演/情绪=50），
    # 前端与后端据此在冲突时做覆盖。
    priority: int | None = None
    # 建议执行时长（毫秒），来自动作目录；前端可覆盖，用于动作播放窗口调度。
    suggested_duration_ms: int | None = None

    def to_dict(self) -> dict:
        """Convert Actions object to a dictionary for JSON serialization"""
        return {k: v for k, v in asdict(self).items() if v is not None}


class BaseOutput(ABC):
    """Base class for agent outputs that can be iterated"""

    @abstractmethod
    def __aiter__(self):
        """Make the output iterable"""


@dataclass
class DisplayText:
    """Text to be displayed with optional metadata"""

    text: str
    name: str | None = "AI"  # Keep the name field for frontend display
    avatar: str | None = None

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization"""
        return {"text": self.text, "name": self.name, "avatar": self.avatar}

    def __str__(self) -> str:
        """String representation for logging"""
        return f"{self.name}: {self.text}"


@dataclass
class SentenceOutput(BaseOutput):
    """
    Output type for text-based responses.
    Contains a single sentence pair (display and TTS) with associated actions.

    Attributes:
        display_text: Text to be displayed in UI
        tts_text: Text to be sent to TTS engine
        actions: Associated actions (expressions, pictures, sounds)
    """

    display_text: DisplayText  # Changed from str to DisplayText
    tts_text: str  # Text for TTS
    actions: Actions

    async def __aiter__(self):
        """Yield the sentence pair and actions"""
        yield self.display_text, self.tts_text, self.actions


@dataclass
class AudioOutput(BaseOutput):
    """Output type for audio-based responses"""

    audio_path: str
    display_text: DisplayText  # Changed from str to DisplayText
    transcript: str  # Original transcript
    actions: Actions

    async def __aiter__(self):
        """Iterate through audio segments and their actions"""
        yield self.audio_path, self.display_text, self.transcript, self.actions
