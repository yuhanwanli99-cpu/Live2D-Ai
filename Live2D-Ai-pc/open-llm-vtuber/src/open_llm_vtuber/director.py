"""ExpressionDirector：把情绪/表情/动作从主对话脑分流到独立导演模型。

主对话脑只输出自然语言；本模块用一个独立 LLM 实例对每句话做轻量分析，
输出确定性协议字段，供 TTS/Live2D 使用。失败或超时一律返回可观测的 fallback，
绝不静默吞掉。
"""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from typing import Any

from loguru import logger

from .halfbody import available_halfbody_channels
from .motion_catalog import SAFE_MOTION_IDS, sanitize_motion

VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness", "surprise", "fear", "smirk", "disgust",
}

_DIRECTOR_SYSTEM = (
    "You are an expression director for a Live2D avatar. "
    "Given one spoken sentence, output ONLY one JSON object on one line, no prose, no markdown. "
    'Fields: {"emotion":"one of neutral/joy/anger/sadness/surprise/fear/smirk/disgust",'
    ' "strength":1|2|3,'
    ' "motionLabel":"one of nod/shake_no/tilt/look_around/listen/surprise" or null,'
    ' "parameterOverrides":{}}. '
    "Choose only the semantic motionLabel; the client owns deterministic body and gaze timelines. "
    "The avatar supports exactly these six motions via head/eye/brow/mouth parameter animation. "
    "It has no arm, hand, full-body, or translation parameters: any label outside the six "
    "(invented or legacy) is invalid - use null when nothing fits. "
    "Do not output halfBody, gaze, head/body parameter values, or mouth/lip-sync channels. "
    "Pick the motionLabel that best matches the sentence tone; most sentences have one. "
    "Use null only when truly nothing fits. parameterOverrides is legacy-only and should normally be empty."
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
    except Exception:  # noqa: BLE001
        return None
    return data if isinstance(data, dict) else None


@dataclass
class DirectorResult:
    emotion: str = "neutral"
    strength: int = 2
    motion_label: str | None = None
    parameter_overrides: dict[str, float] = field(default_factory=dict)
    half_body: dict[str, float] = field(default_factory=dict)
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
            "halfBody": self.half_body,
            "motionTimeline": self.motion_timeline,
            "source": self.source,
            "raw": self.raw,
        }


# 情绪 → 动作兜底（严格克制，定版）：只允许 surprise → surprise 一条映射；
# 其余情绪（joy/anger/sadness/fear/smirk/disgust）一律 None —— 不强行动作，
# 不用近似动作冒充（无法诚实映射的情绪不产动作）。
_EMOTION_MOTION = {
    "surprise": "surprise",
}

# neutral 兜底动作：不产出任何动作（None）；强度用最低档 1。
_NEUTRAL_MOTION: str | None = None
_NEUTRAL_STRENGTH = 1

# 显式用户动作命令（优先级最高）：用户在输入里明确要求角色做某个动作时，
# 回应句按此动作执行，不再只依赖导演模型猜测。词序长者优先，避免子串歧义。
# 定版只保留 Bai 可诚实表达的四个动作命令；鞠躬/靠近等已删除（无对应参数）。
_USER_MOTION_COMMANDS: dict[str, tuple[str, ...]] = {
    "nod": ("点点头", "点一下头", "点个头", "点头"),
    "shake_no": ("摇摇头", "摇一下头", "摇个头", "摇头"),
    "tilt": ("歪一下头", "歪歪头", "歪个头", "歪头"),
    "look_around": ("张望一下", "看看周围", "看看四周", "东张西望", "左顾右盼", "张望"),
}

# 疑问/否定语境保护：整句命中即视为“讨论动作”而非“命令动作”，
# 避免「为什么摇头/别摇头/怎么张望」误触发。
# 保守策略：宁可漏触发，也不误触发（用户侧要求明确）。
_USER_MOTION_GUARD_TERMS = (
    "为什么", "为何", "为啥", "怎么", "干嘛", "什么叫", "什么是",
    "别", "不要", "不许", "禁止", "不愿", "不会", "不能", "没法",
)


def _resolve_motion(emotion: str, motion: str | None) -> str | None:
    """动作边界收敛：候选 → 白名单；越界/缺失 → 情绪映射兜底；仍缺 → None。"""
    if motion and motion in _VALID_MOTIONS:
        return motion
    return _EMOTION_MOTION.get(emotion)


def detect_user_motion_command(text: str | None) -> str | None:
    """从【用户输入】识别显式动作命令 → 合法动作标签；无命令/命中语境保护 → None。

    保护规则：文本中出现任一 guard 词（为什么/别/怎么/…）即整句不触发——
    这些语境是“讨论/否定/质疑动作”，不是对角色下达动作命令。
    """
    if not text:
        return None
    lowered = text.strip().lower()
    if not lowered:
        return None
    for guard in _USER_MOTION_GUARD_TERMS:
        if guard in lowered:
            return None
    for motion, keywords in _USER_MOTION_COMMANDS.items():
        for kw in keywords:
            if kw in lowered:
                return motion
    return None


# 舞台说明/动作括注 → 半身动作（bai 无 motion3，走参数驱动）
# 定版只保留六个 Bai 可诚实表达的动作；挥手/鼓掌/蹦跳/鞠躬/凑近/摇摆等
# 关键词一律不匹配（返回 None，不产出任何近似冒充动作）。
_STAGE_GESTURE_KEYWORDS = {
    "nod": ("点头", "颔首", "轻轻点头"),
    "shake_no": ("摇头", "用力摇头"),
    "tilt": ("歪头", "侧头", "歪了歪头"),
    "look_around": ("张望", "左顾右盼", "东张西望"),
    "listen": ("倾听", "思考", "托腮", "歪着脑袋想"),
    "surprise": ("惊讶", "震惊", "睁大"),
}

# 动作边界白名单：与 motion_catalog.SAFE_MOTION_IDS 单一来源对齐。
# 导演 LLM 输出的 motionLabel 必须落在集合内，越界一律丢弃并走情绪映射兜底。
# 注意：`idle` 仅作为开发者面板 release 语义，不作为逐句可触发的 motion。
_VALID_MOTIONS = frozenset(SAFE_MOTION_IDS)

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


def normalize_chunk_timeline(
    cues: list[dict[str, Any]], min_gap: float = 0.2
) -> list[dict[str, Any]]:
    """同一 TTS 分块内的动作时间轴：去重 + 强制最小间隔，避免冲突动作打架。

    - 同一 motion 在该块内多次出现只保留首个；
    - 两个不同动作若 at 间隔 < min_gap，保留前者（紧贴的 cue 在渲染端容易
      互相覆盖，产生抖动/半截动作）；
    - 输入为空 / 异常 → []。

    保持 projector 的输出形态：[{"at": float, "motion": str}, ...]（at 保留 4 位小数）。
    """
    if not cues:
        return []
    kept: list[dict[str, Any]] = []
    seen_motions: set[str] = set()
    last_at = -1.0
    # 浮点宽容：刚好落在 min_gap 边界的 cue 不应因 0.3-0.1=0.19999… 被误杀；
    # 真正小于 min_gap 的仍被过滤。
    threshold = float(min_gap) - 1e-9
    for cue in sorted(cues, key=lambda c: float(c.get("at", 0))):
        motion = sanitize_motion(str(cue.get("motion") or "").strip().lower())
        if not motion or motion in seen_motions:
            continue
        at = float(cue.get("at", 0))
        if kept and at - last_at < threshold:
            continue
        kept.append({"at": round(at, 4), "motion": motion})
        seen_motions.add(motion)
        last_at = at
    return kept


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
    # neutral 不产出动作（None，避免无意义抖动）；其他情绪严格走情绪映射兜底
    motion = _resolve_motion(emotion, None)
    strength = _NEUTRAL_STRENGTH if emotion == "neutral" else 2
    return DirectorResult(
        emotion=emotion,
        strength=strength,
        motion_label=motion,
        source="rule",
    )


# 超时告警只提示一次（每句都刷会淹没日志）
_TIMEOUT_WARNED = False

class ExpressionDirector:
    """独立的情绪/表情导演模型。"""

    def __init__(
        self,
        llm=None,
        timeout: float | None = None,
        parameter_ids: set[str] | None = None,
    ):
        self._llm = llm
        self._parameter_ids = set(parameter_ids or ())
        self.halfbody_channels = available_halfbody_channels(self._parameter_ids)
        if timeout is None:
            # DeepSeek 等推理型模型首 token 延迟常在 1-2s+；默认 1.5s 会让导演
            # 几乎每句都超时、静默回落规则兜底（表现为表情/动作「不生效」）。
            # 放宽到 3.5s，并允许用环境变量 LIVE2DAI_DIRECTOR_TIMEOUT 调整。
            env = os.environ.get("LIVE2DAI_DIRECTOR_TIMEOUT")
            try:
                timeout = float(env) if env else 3.5
            except ValueError:
                timeout = 3.5
        self.timeout = timeout

    async def analyze(self, text: str) -> DirectorResult:
        import asyncio

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
                # neutral 动作克制：统一最低强度（与 rule_fallback 对齐）
                if emotion == "neutral":
                    strength = _NEUTRAL_STRENGTH
                motion = data.get("motionLabel") or data.get("motion")
                # 动作边界：LLM 只能在白名单内选择；越界/缺失 → 情绪映射
                # 兜底；仍缺（neutral）→ 克制动作（不静止、不静默）
                if isinstance(motion, str):
                    motion = motion.strip().lower() or None
                motion = _resolve_motion(emotion, motion)
                overrides = data.get("parameterOverrides") or {}
                if not isinstance(overrides, dict):
                    overrides = {}
                # Preview 控制策略：LLM 只选择语义动作；身体、头部与 gaze 的
                # 幅度/时序/回中全部由渲染端确定性动作库负责。忽略模型输出的
                # halfBody，避免一次性 gaze/pose 永久占用参数（眼睛偏左下根因）。
                half_body: dict[str, float] = {}
                return DirectorResult(
                    emotion=emotion,
                    strength=strength,
                    motion_label=motion if isinstance(motion, str) else None,
                    parameter_overrides=overrides,
                    half_body=half_body,
                    source="llm",
                    raw=raw[:300],
                )
        except Exception as exc:  # noqa: BLE001
            if isinstance(exc, asyncio.TimeoutError):
                # 超时回落此前完全静默，用户侧只看到「导演不工作」；至少提示一次
                if not _TIMEOUT_WARNED:
                    logger.warning(
                        f"ExpressionDirector LLM 超时({self.timeout:.1f}s)，本句回落规则兜底"
                        "（仅提示一次；可用环境变量 LIVE2DAI_DIRECTOR_TIMEOUT 调大超时）"
                    )
                    globals()["_TIMEOUT_WARNED"] = True
            else:
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


def sanitize_director_result(data: dict[str, Any]) -> dict[str, Any]:
    """Sanitize a DirectorResult dict for developer-mode display.

    Strips raw LLM output and any credential-like fields so the dev endpoint
    never leaks prompts, keys, or upstream internals. motionLabel 与
    motionTimeline 逐帧过动作目录白名单：已删/未知 id 一律清空（dev 协议
    与公开协议同边界，绝不回显已删动作 id）。
    """
    if not isinstance(data, dict):
        return {}
    allowed = {
        "emotion",
        "strength",
        "motionLabel",
        "parameterOverrides",
        "halfBody",
        "motionTimeline",
        "source",
    }
    out: dict[str, Any] = {}
    for key in allowed:
        if key in data:
            out[key] = data[key]
    # 动作目录白名单：motionLabel / timeline 帧里的已删 id → None / 丢弃
    if "motionLabel" in out:
        label = out.get("motionLabel")
        out["motionLabel"] = (
            sanitize_motion(label) if isinstance(label, str) else None
        )
    if isinstance(out.get("motionTimeline"), list):
        clean_timeline: list[dict[str, Any]] = []
        for frame in out["motionTimeline"]:
            if not isinstance(frame, dict):
                continue
            frame_motion = frame.get("motion")
            safe_frame_motion = (
                sanitize_motion(frame_motion)
                if isinstance(frame_motion, str)
                else None
            )
            if not safe_frame_motion:
                continue
            clean_frame = dict(frame)
            clean_frame["motion"] = safe_frame_motion
            clean_timeline.append(clean_frame)
        out["motionTimeline"] = clean_timeline
    # Clamp parameter overrides to safe numeric values / keys
    overrides = out.get("parameterOverrides")
    if isinstance(overrides, dict):
        clean: dict[str, float] = {}
        for k, v in overrides.items():
            if isinstance(v, (int, float)) and not isinstance(v, bool):
                clean[str(k)] = float(v)
        out["parameterOverrides"] = clean
    else:
        out["parameterOverrides"] = {}
    # Half-body must be a flat numeric dict
    half = out.get("halfBody")
    if isinstance(half, dict):
        clean_half: dict[str, float] = {}
        for k, v in half.items():
            if isinstance(v, (int, float)) and not isinstance(v, bool):
                clean_half[str(k)] = float(v)
        out["halfBody"] = clean_half
    else:
        out["halfBody"] = {}
    return out
