"""ExpressionDirector：把情绪/表情/动作从主对话脑分流到独立导演模型。

主对话脑只输出自然语言；本模块用一个独立 LLM 实例对每句话做轻量分析，
输出确定性协议字段，供 TTS/Live2D 使用。失败或超时一律返回可观测的 fallback，
绝不静默吞掉。
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from typing import Any

from loguru import logger

VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness", "surprise", "fear", "smirk", "disgust",
}

_DIRECTOR_SYSTEM = (
    "You are an expression director for a Live2D avatar. "
    "Given one spoken sentence, output ONLY one JSON object on one line, no prose, no markdown. "
    'Fields: {"emotion":"one of neutral/joy/anger/sadness/surprise/fear/smirk/disgust",'
    ' "strength":1|2|3,'
    ' "motionLabel":"one of nod/tilt/sway/happy/surprise/listen/excited/'
    'wave_big/bow/shake_no/lean_in/look_around/bounce_happy" or null,'
    ' "parameterOverrides":{}}. '
    "Pick the motionLabel that best matches the sentence tone; most sentences have one. "
    "Use null only when truly nothing fits. Parameter overrides must only use Live2D parameter IDs like ParamEyeLOpen."
)


def _extract_json(text: str) -> dict[str, Any] | None:
    if not text:
        return None
    fence = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", text, re.DOTALL)
    raw = fence.group(1) if fence else None
    if raw is None:
        m = re.search(r"\{.*\}", text, re.DOTALL)
        raw = m.group(0) if m else None
    if not raw:
        return None
    try:
        data = json.loads(raw)
    except Exception:
        return None
    return data if isinstance(data, dict) else None


@dataclass
class DirectorResult:
    emotion: str = "neutral"
    strength: int = 2
    motion_label: str | None = None
    parameter_overrides: dict[str, float] = field(default_factory=dict)
    # 括注动作帧（my-neuro 式）：stage_gesture_timeline 产出，含 position /
    # clean_position / motion；conversation_utils 切片成 motionTimeline 随 audio 下发
    motion_timeline: list[dict[str, Any]] = field(default_factory=list)
    source: str = "rule"
    raw: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "emotion": self.emotion,
            "strength": self.strength,
            "motionLabel": self.motion_label,
            "parameterOverrides": self.parameter_overrides,
            "motionTimeline": self.motion_timeline,
            "source": self.source,
            "raw": self.raw,
        }


# 情绪 → 半身动作兜底映射（8 key 全覆盖；值为前端 BodyActionPlayer 可路由的动作名，
# 含编排库动作 shake_no 等，前端 resolveChoreography 自动路由）
_EMOTION_MOTION = {
    "joy": "happy",
    "surprise": "surprise",
    "sadness": "listen",
    "fear": "listen",
    "anger": "shake_no",
    "smirk": "tilt",
    "disgust": "tilt",
}


# 舞台说明/动作括注 → 半身动作（bai 无 motion3，走参数驱动）
# 值可以是单姿态手势（BodyGesture），也可以是编排动作（choreography 库：
# 时间轴关键帧、大幅度；前端 resolveChoreography 自动路由）。
_STAGE_GESTURE_KEYWORDS = {
    "nod": ("点头", "颔首", "轻轻点头"),
    "tilt": ("歪头", "侧头", "歪了歪头"),
    "sway": ("扇风", "挥手扇", "晃了晃", "摇摆", "摇手"),
    "happy": ("招手", "比心", "拍手", "欢呼"),
    "surprise": ("惊讶", "睁大", "捂嘴", "震惊"),
    "listen": ("倾听", "思考", "托腮", "歪着脑袋想"),
    "excited": ("鼓掌", "雀跃"),
    # —— 编排动作（大幅度时间轴）——
    "wave_big": ("挥手", "挥手打招呼", "用力挥手"),
    "bounce_happy": ("蹦跳", "跳起来", "蹦蹦跳跳"),
    "bow": ("鞠躬", "弯腰", "低头行礼"),
    "shake_no": ("摇头", "用力摇头"),
    "lean_in": ("凑近", "靠近", "贴近", "凑过来"),
    "look_around": ("张望", "左顾右盼", "东张西望"),
}

# 动作边界白名单：单姿态手势 + 编排库动作（与前端 resolveChoreography/BodyGesture 对齐）。
# 导演 LLM 输出的 motionLabel 必须落在集合内，越界一律丢弃并走情绪映射兜底 —— 这就是
# "参数使用说明定义的脚本动作边界"：LLM 只能选，不能发明。
_VALID_MOTIONS = frozenset(_STAGE_GESTURE_KEYWORDS) | {
    "idle", "excited", "surprise", "listen", "sway", "nod", "tilt", "happy",
}

# 动作括注统一正则（与 filter_parentheses 的全角/半角覆盖一致；不支持跨层嵌套，
# 与既有 stage_gesture 行为对齐）
_BRACKET_RE = re.compile(r"[（(]([^（）()]*)[）)]")


def _match_gesture(content: str) -> str | None:
    """括注内容 → BodyGesture；未命中关键词返回 None。"""
    if not content:
        return None
    lowered = content.lower()
    for gesture, keywords in _STAGE_GESTURE_KEYWORDS.items():
        if any(k in content or k in lowered for k in keywords):
            return gesture
    return None


def stage_gesture(text: str | None) -> str | None:
    """从全角/半角括号括注中识别动作意图，返回 BodyGesture 或 None。

    兼容入口：等价于 stage_gesture_timeline(text) 的第一个命中。
    """
    cues = stage_gesture_timeline(text)
    return cues[0]["motion"] if cues else None


def stage_gesture_timeline(text: str | None) -> list[dict[str, Any]]:
    """解析全部动作括注为带文本位置的 motion 帧列表（my-neuro 式锚点）。

    返回按出现顺序排列的
        [{"position": int, "clean_position": int, "motion": str}, ...]
    - position：括注起点在【原文】中的字符偏移；
    - clean_position：扣除该括注之前所有括注跨度后的偏移，即同一句话经
      filter_parentheses 清洗后，该动作在朗读文本中的近似字符锚点
      （filter_parentheses 还会折叠空白，此处忽略该误差——下游本就做线性近似）。
    - 无括注 / 括注内容未命中关键词 → []。
    """
    if not text:
        return []
    cues: list[dict[str, Any]] = []
    removed_before = 0  # position 之前已被清洗掉的括注总字符数（含未命中的）
    for m in _BRACKET_RE.finditer(text):
        gesture = _match_gesture(m.group(1).strip())
        if gesture:
            cues.append(
                {
                    "position": m.start(),
                    "clean_position": m.start() - removed_before,
                    "motion": gesture,
                }
            )
        removed_before += m.end() - m.start()
    return cues


def project_motion_cues_into_chunks(
    cues: list[dict[str, Any]],
    tts_text: str,
    tts_chunks: list[str],
    anchor_len: int,
) -> list[list[dict[str, Any]]]:
    """把句子级 motion 帧切片到各 TTS 分块（conversation_utils 下发用）。

    返回与 tts_chunks 等长的列表，每项形如 [{"at": float, "motion": str}, ...]：
    - at ∈ [0, 1)：该动作在所属音频块内的归一化播放进度，由文本位置线性近似
      （与 Android M7 关键词兜底 t(pos)=pos/len×totalMs 同法）；
    - 无动作的分块为 []；任何输入异常（anchor_len≤0 / chunk 无法在 tts_text 中
      顺序定位）一律返回全空列表 —— 时间轴整体不下发，前端退回旧的一次性触发。
    """
    sliced: list[list[dict[str, Any]]] = [[] for _ in tts_chunks]
    total = len(tts_text)
    if not cues or total <= 0 or not tts_chunks or anchor_len <= 0:
        return sliced

    # 顺序定位每个 chunk 在 tts_text 中的 [start, end)（split_tts_chunks 保序切出）
    spans: list[tuple[int, int]] = []
    cursor = 0
    for chunk in tts_chunks:
        idx = tts_text.find(chunk, cursor)
        if idx < 0:
            return sliced  # 定位失败 → 整体降级，不发时间轴
        spans.append((idx, idx + len(chunk)))
        cursor = idx + len(chunk)

    for cue in cues:
        frac = min(max(cue.get("clean_position", 0) / anchor_len, 0.0), 1.0)
        for i, (start, end) in enumerate(spans):
            s_frac, e_frac = start / total, end / total
            if frac < e_frac or i == len(spans) - 1:
                span = max(e_frac - s_frac, 1e-6)
                at = min(max((frac - s_frac) / span, 0.0), 0.999)
                sliced[i].append({"at": round(at, 4), "motion": cue["motion"]})
                break
    for item in sliced:
        item.sort(key=lambda c: c["at"])
    return sliced


def rule_fallback(text: str) -> DirectorResult:
    lowered = (text or "").lower()
    emotion = "neutral"
    if any(k in lowered for k in ("开心", "高兴", "喜欢", "哈哈", "太好了")):
        emotion = "joy"
    elif any(k in lowered for k in ("生气", "讨厌", "别", "不要")):
        emotion = "anger"
    elif any(k in lowered for k in ("难过", "伤心", "哭", "对不起")):
        emotion = "sadness"
    elif any(k in lowered for k in ("哇", "什么", "真的吗", "天哪")):
        emotion = "surprise"
    return DirectorResult(
        emotion=emotion,
        strength=2,
        motion_label=_EMOTION_MOTION.get(emotion),
        source="rule",
    )


class ExpressionDirector:
    """独立的情绪/表情导演模型。"""

    def __init__(self, llm=None, timeout: float = 1.5):
        self._llm = llm
        self.timeout = timeout

    async def analyze(self, text: str) -> DirectorResult:
        clean = (text or "").strip()
        if not clean:
            return rule_fallback(clean)
        if self._llm is None:
            return rule_fallback(clean)
        try:
            raw = await self._call_llm(clean)
            data = _extract_json(raw)
            if data:
                emotion = str(data.get("emotion") or "neutral").lower()
                if emotion not in VALID_EMOTIONS:
                    emotion = "neutral"
                strength = int(data.get("strength") or 2)
                strength = max(1, min(3, strength))
                motion = data.get("motionLabel") or data.get("motion")
                # 动作边界：LLM 只能在白名单内选择；越界/缺失 → 情绪映射兜底
                if isinstance(motion, str):
                    motion = motion.strip().lower() or None
                if not motion or motion not in _VALID_MOTIONS:
                    motion = _EMOTION_MOTION.get(emotion)
                overrides = data.get("parameterOverrides") or {}
                if not isinstance(overrides, dict):
                    overrides = {}
                return DirectorResult(
                    emotion=emotion,
                    strength=strength,
                    motion_label=motion if isinstance(motion, str) else None,
                    parameter_overrides=overrides,
                    source="llm",
                    raw=raw[:300],
                )
        except Exception as exc:
            logger.warning(f"ExpressionDirector LLM failed, fallback to rule: {exc}")
        return rule_fallback(clean)

    async def _call_llm(self, text: str) -> str:
        import asyncio

        prompt = f"{_DIRECTOR_SYSTEM}\n\nSentence: {text}\nJSON:"
        stream = self._llm.chat_completion(
            [{"role": "user", "content": prompt}],
            system=_DIRECTOR_SYSTEM,
        )
        chunks: list[str] = []

        async def collect():
            async for item in stream:
                if isinstance(item, str):
                    chunks.append(item)
                elif isinstance(item, list):
                    continue

        await asyncio.wait_for(collect(), timeout=self.timeout)
        return "".join(chunks).strip()