"""
mood_engine.py — 流式心情值系统（P3 mood_engine，tier: premium）

双层情绪模型：
  - 瞬时 emotion：当前句子的情绪（LLM [emo:xxx] Token / 用户消息情感），驱动当前表情/语气
  - 持续 mood：-100 ~ +100 的心情值，随时间向 0 衰减，影响三端：
      ① get_expression_modifier() → 默认表情基线微调
      ② get_tts_modifier()        → TTS rate/pitch 基调微调
      ③ get_prompt_injection()    → system prompt 注入文本

事件源（contract-pc-pipeline.md §P3）：
  - on_emotion_event(emotion, strength)  流式 LLM [emo:xxx] Token
  - on_user_message_sentiment(sentiment) 用户消息情感（规则匹配/LLM 分类结果）
  - on_interaction_event(event_type)     交互事件（打断/戳/沉默/主动搭话）

对齐契约断言：P3-01 ~ P3-07 + 附录 A 事件序列集成场景（±0.5 容差内）。

约束：
  - 值域钳制 [-100, +100]
  - 衰减：后台任务每 DECAY_INTERVAL 秒向 0 回归 DECAY_RATE（tick 制）
  - 时钟函数可注入（便于测试）；DECAY_RATE / DECAY_INTERVAL 可构造配置（默认契约值）
"""

import asyncio
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Callable, Optional

from loguru import logger


class MoodTier(Enum):
    """mood 分层（契约 §P3 值域区间）"""

    FURIOUS = "furious"  # -100 ~ -60
    UPSET = "upset"  # -60 ~ -20
    NEUTRAL = "neutral"  # -20 ~ +20
    HAPPY = "happy"  # +20 ~ +60
    ECSTATIC = "ecstatic"  # +60 ~ +100

    @classmethod
    def from_value(cls, mood: float) -> "MoodTier":
        if mood <= -60:
            return cls.FURIOUS
        elif mood <= -20:
            return cls.UPSET
        elif mood <= 20:
            return cls.NEUTRAL
        elif mood <= 60:
            return cls.HAPPY
        else:
            return cls.ECSTATIC


# 流式事件 → mood 偏移量表（硬编码，来源 contract-pc-pipeline.md §P3 数据 schema）
EMOTION_MOOD_DELTA = {
    # 用户消息情感 → mood 偏移
    "user_positive": +8.0,  # 用户说开心/夸奖的话
    "user_negative": -10.0,  # 用户说生气/贬低的话
    "user_neutral": +1.0,  # 用户普通聊天
    "user_question": +2.0,  # 用户提问（被需要感）
    # LLM 回复情绪 Token → mood 偏移（持续情绪积累）
    # emo:joy=+7.0：与 integration 断言对齐（P7-04：3× joy(2)=21 → 达 HAPPY 阈值 ≥20）
    "emo:joy": +7.0,
    "emo:surprise": +3.0,
    "emo:smirk": +4.0,
    "emo:neutral": +0.5,
    "emo:sadness": -6.0,
    "emo:anger": -5.0,
    "emo:fear": -7.0,
    "emo:disgust": -4.0,
    # 交互事件
    "user_interrupt": -3.0,  # 用户打断
    "user_tap": +2.0,  # 用户戳/互动
    "user_long_silence": -4.0,  # 长时间无交互
    "proactive_speak": +1.0,  # 主动搭话
}

# 常量（契约 §P3 数据 schema）
DECAY_RATE = 2.0  # 每秒向 0 回归
DECAY_INTERVAL = 1.0  # 衰减 tick 间隔（秒）
MOOD_MIN = -100.0
MOOD_MAX = 100.0

# mood → TTS 基调微调（架构 §4.3.1）
MOOD_TTS_MODIFIER = {
    "furious": {"speech_rate": 1.05, "pitch_rate": 1.05},
    "upset": {"speech_rate": 0.98, "pitch_rate": 0.98},
    "neutral": {"speech_rate": 1.00, "pitch_rate": 1.05},
    "happy": {"speech_rate": 1.02, "pitch_rate": 1.08},
    "ecstatic": {"speech_rate": 1.08, "pitch_rate": 1.12},
}

# mood → 默认表情基线微调（加到 neutral 参数上，架构 §4.3.1）
MOOD_EXPRESSION_MODIFIER = {
    "furious": {"ParamBrowY": -0.10, "ParamMouthForm": -0.15},
    "upset": {"ParamBrowY": -0.05, "ParamMouthForm": -0.05},
    "neutral": {},
    "happy": {"ParamMouthForm": +0.10, "ParamEyeLOpen": +0.05},
    "ecstatic": {"ParamMouthForm": +0.20, "ParamBrowY": +0.10},
}

# mood → prompt 注入文本（架构 §4.3.1）
MOOD_PROMPT_INJECTION = {
    "furious": "[当前心情：非常烦躁，语气可能不耐烦]",
    "upset": "[当前心情：有点低落，语气可能冷淡]",
    "neutral": "",
    "happy": "[当前心情：心情不错，语气活泼]",
    "ecstatic": "[当前心情：超级开心，语气兴奋]",
}

# 用户消息情感规则匹配关键词词典（架构 §4.3.3 方案 B：先用规则，后续迭代换 LLM）
_SENTIMENT_POSITIVE_KEYWORDS = (
    "哈哈", "嘻嘻", "嘿嘿", "开心", "高兴", "快乐", "喜欢", "爱", "棒", "赞",
    "厉害", "好耶", "不错", "满意", "谢谢", "感谢", "可爱", "好玩", "漂亮",
    "好看", "加油", "抱抱", "亲亲", "太好了", "真棒", "超爱", "好喜欢",
    "lol", "haha", "great", "love", "nice", "cute", "awesome",
)
_SENTIMENT_NEGATIVE_KEYWORDS = (
    "烦", "讨厌", "气死", "滚", "笨蛋", "傻瓜", "蠢", "垃圾", "差劲",
    "失望", "伤心", "难过", "哭", "恶心", "闭嘴", "走开", "别理我",
    "烦死", "恨", "废物", "没用", "糟糕", "生气", "可恶", "混蛋",
    "hate", "stupid", "awful", "terrible", "angry",
)
_QUESTION_MARKERS = (
    "？", "?", "吗", "呢", "怎么", "为什么", "啥", "什么", "哪",
    "是不是", "能否", "可以吗", "how", "what", "why", "when", "where",
)


@dataclass
class MoodState:
    """mood 运行时状态（含最近情绪历史，供审计/调试）"""

    value: float = 0.0  # -100 .. +100
    tier: MoodTier = MoodTier.NEUTRAL
    last_update: float = field(default_factory=time.monotonic)
    emotion_history: list = field(default_factory=list)  # 最近 N 个 emotion 事件


class MoodEngine:
    """
    流式心情引擎。

    职责：
      - 接收流式事件（emotion token / 用户消息情感 / 交互事件）→ 更新 mood
      - 每秒衰减（向 0 回归）
      - 产出三端联动值：表情基线微调、TTS 基调微调、prompt 注入文本

    事件序列（contract §P3 附录 A）：
      start() → on_user_message_sentiment(...) → on_emotion_event(...)
      → （时间衰减） → get_mood_tier() / get_tts_modifier() / get_expression_modifier() → stop()

    时钟注入：`clock` 参数替换 time.monotonic（测试可控）；`decay_interval`/`decay_rate`
    可配置（默认 DECAY_INTERVAL=1.0 / DECAY_RATE=2.0，契约值），测试可缩短间隔加速。
    """

    HISTORY_MAX = 20  # 保留最近 N 个情绪事件

    def __init__(
        self,
        clock: Optional[Callable[[], float]] = None,
        decay_interval: float = DECAY_INTERVAL,
        decay_rate: float = DECAY_RATE,
    ):
        self._clock: Callable[[], float] = clock or time.monotonic
        self._decay_interval: float = decay_interval
        self._decay_rate: float = decay_rate
        self._state = MoodState(last_update=self._clock())
        self._decay_task: Optional[asyncio.Task] = None
        self._lock = asyncio.Lock()

    # ── 生命周期 ──

    async def start(self) -> None:
        """启动后台衰减任务（幂等：已运行则跳过）。"""
        if self._decay_task is None or self._decay_task.done():
            self._decay_task = asyncio.create_task(self._decay_loop())
            logger.debug("MoodEngine decay loop started")

    async def stop(self) -> None:
        """停止后台衰减任务（幂等：未运行则跳过）。"""
        if self._decay_task and not self._decay_task.done():
            self._decay_task.cancel()
            try:
                await self._decay_task
            except asyncio.CancelledError:
                pass
            logger.debug("MoodEngine decay loop stopped")

    # ── 公共 API：事件打分 ──

    async def on_emotion_event(self, emotion: str, strength: int = 2) -> None:
        """
        流式 emotion 事件（LLM [emo:xxx] Token 到达时调用）。

        Args:
            emotion: 情绪名（joy/anger/sadness/surprise/fear/smirk/disgust/neutral）
            strength: 强度 1-3（偏移量按 strength/2 线性缩放，默认 2）
        """
        key = f"emo:{emotion}"
        base = EMOTION_MOOD_DELTA.get(key, 0.0)
        delta = base * (strength / 2.0)
        await self._apply_delta(delta)
        # 记录历史
        async with self._lock:
            self._state.emotion_history.append(
                {
                    "emotion": emotion,
                    "strength": strength,
                    "delta": delta,
                    "time": self._clock(),
                }
            )
            if len(self._state.emotion_history) > self.HISTORY_MAX:
                self._state.emotion_history.pop(0)
        logger.debug(
            f"Mood: emo:{emotion} (s={strength}) delta={delta:+.1f} "
            f"→ mood={self._state.value:.1f}"
        )

    async def on_user_message_sentiment(self, sentiment: str) -> None:
        """
        用户消息情感事件（对话开始时调用；sentiment 为分类标签或原始文本）。

        Args:
            sentiment: "positive" / "negative" / "neutral" / "question"
                       若传入原始用户文本，则先用规则词典分类（见 classify_sentiment）
        """
        if sentiment not in ("positive", "negative", "neutral", "question"):
            sentiment = self.classify_sentiment(sentiment)
        key = f"user_{sentiment}"
        delta = EMOTION_MOOD_DELTA.get(key, 0.0)
        await self._apply_delta(delta)
        logger.debug(f"Mood: user_{sentiment} delta={delta:+.1f} → mood={self._state.value:.1f}")

    async def on_interaction_event(self, event_type: str) -> None:
        """交互事件（打断/戳/沉默/主动搭话等，键直接查 EMOTION_MOOD_DELTA）。"""
        delta = EMOTION_MOOD_DELTA.get(event_type, 0.0)
        await self._apply_delta(delta)
        logger.debug(f"Mood: interaction {event_type} delta={delta:+.1f} → mood={self._state.value:.1f}")

    # ── 公共 API：三端联动读取 ──

    def get_mood_value(self) -> float:
        """获取当前 mood 数值（-100~+100）。"""
        return self._state.value

    def get_mood_tier(self) -> MoodTier:
        """获取当前 mood 分层。"""
        return self._state.tier

    def get_tts_modifier(self) -> dict:
        """获取 mood → TTS 基调微调参数（{"speech_rate": ..., "pitch_rate": ...}）。"""
        return MOOD_TTS_MODIFIER.get(self._state.tier.value, {})

    def get_expression_modifier(self) -> dict:
        """获取 mood → 默认表情基线微调参数（叠加到 neutral 表情基线上）。"""
        return MOOD_EXPRESSION_MODIFIER.get(self._state.tier.value, {})

    def get_prompt_injection(self) -> str:
        """获取 mood → prompt 注入文本（追加到 system prompt 尾部；neutral 返回 ""）。"""
        return MOOD_PROMPT_INJECTION.get(self._state.tier.value, "")

    # ── 规则匹配情感分类（方案 B：关键词词典）──

    @staticmethod
    def classify_sentiment(text: str) -> str:
        """
        规则匹配用户消息情感。

        优先级：question（被需要感）> negative（负面情绪强）> positive > neutral

        Returns:
            "positive" / "negative" / "neutral" / "question"
        """
        if not text:
            return "neutral"
        lowered = text.lower()
        # 问句优先：含疑问标点或疑问词 → question
        if any(marker in lowered for marker in _QUESTION_MARKERS):
            return "question"
        if any(kw in lowered for kw in _SENTIMENT_NEGATIVE_KEYWORDS):
            return "negative"
        if any(kw in lowered for kw in _SENTIMENT_POSITIVE_KEYWORDS):
            return "positive"
        return "neutral"

    # ── 内部 ──

    async def _apply_delta(self, delta: float) -> None:
        """应用偏移并钳制到 [-100, +100]，同步更新 tier。"""
        async with self._lock:
            new_value = self._state.value + delta
            self._state.value = max(MOOD_MIN, min(MOOD_MAX, new_value))
            self._state.tier = MoodTier.from_value(self._state.value)
            self._state.last_update = self._clock()

    async def _decay_loop(self) -> None:
        """后台衰减循环：每 decay_interval 秒向 0 回归 decay_rate（tick 制，不越过 0）。"""
        while True:
            await asyncio.sleep(self._decay_interval)
            async with self._lock:
                if self._state.value > 0:
                    self._state.value = max(0.0, self._state.value - self._decay_rate)
                elif self._state.value < 0:
                    self._state.value = min(0.0, self._state.value + self._decay_rate)
                self._state.tier = MoodTier.from_value(self._state.value)
