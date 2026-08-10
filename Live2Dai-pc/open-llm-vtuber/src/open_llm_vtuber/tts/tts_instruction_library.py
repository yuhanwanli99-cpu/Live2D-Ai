"""TTS instruction 动态拼接库（P4）。

按 emotion + strength + mood_tier 动态拼接 CosyVoice instruction 文案：

- ``INSTRUCTION_LIBRARY``: 8 种情绪的基础文案（素材来自
  search-report-polish-materials.md §1.3，每条 ≤80 字符）
- ``MOOD_MODIFIERS``: 5 档心情分层（mood_tier）的前缀修饰词（≤20 字符）
- ``STRENGTH_MODIFIERS``: 1-3 级强度的语气修饰词（strength=2 默认无修饰）

所有拼接结果保证 ≤100 字符（CosyVoice 硬限制；字符计数规则：中文字符
含简体/繁体/日文汉字/韩文汉字每个计 2，标点/字母/数字计 1，见材料报告 §1.1）。
"""

#: 8 emotion 基础文案（每条已按"中文计 2 字符"规则换算校验，≤80 字符）
INSTRUCTION_LIBRARY: dict[str, str] = {
    "neutral": "语气自然平静带点小高冷，像天才少女日常淡淡的傲气，语速平稳不拖沓。",
    "joy": "用非常开心活泼的语气说话，透着雀跃和撒娇，句尾带点上扬的俏皮感。",
    "anger": "语气带着傲娇的小脾气，嘴硬生闷气，语速略快，强调句尾，但不能太凶。",
    "sadness": "用委屈失落的语气说话，像被冷落的小猫，声音轻轻的、带点鼻音和哽咽。",
    "surprise": "用惊讶慌张的语气说话，像突然被吓一跳，声调瞬间拔高，语速变快带点结巴。",
    "fear": "声音透出紧张和不安，语速略快，声调微微发颤，像受惊的小动物。",
    "smirk": "用得意又有点腹黑的语气说话，带着小得意的轻笑感，语速慢条斯理、句尾玩味。",
    "disgust": "语气透着嫌弃和不耐烦，像看到讨厌的东西，声调略低，语速偏快。",
}

#: 5 档 mood tier 前缀修饰词（拼接在 instruction 开头，总长 ≤100 字符）
MOOD_MODIFIERS: dict[str, str] = {
    "furious": "情绪很差，",
    "upset": "情绪有点低落，",
    "neutral": "",
    "happy": "心情不错，",
    "ecstatic": "心情超好，",
}

#: 1-3 级强度修饰词（strength=2 默认无修饰，保证 P4-04 降级时精确命中基础文案）
STRENGTH_MODIFIERS: dict[int, str] = {
    1: "语气轻柔些，",
    2: "",
    3: "语气强烈些，",
}

#: 情绪别名归一化（兼容任务侧 emotion 命名与契约/架构 key 的差异）
_EMOTION_ALIASES: dict[str, str] = {
    "sad": "sadness",
    "angry": "anger",
    "love": "joy",
}

#: CosyVoice instruction 最大字符数（中文字符按 2 计）
MAX_INSTRUCTION_CHARS = 100


def _is_cjk_char(ch: str) -> bool:
    """判断字符是否为中日韩统一表意文字（中文计 2 字符的判定依据）。"""
    cp = ord(ch)
    return (
        0x4E00 <= cp <= 0x9FFF  # CJK 统一表意文字
        or 0x3400 <= cp <= 0x4DBF  # CJK 扩展 A
        or 0xF900 <= cp <= 0xFAFF  # CJK 兼容表意文字
    )


def _char_len(text: str) -> int:
    """按 CosyVoice 规则计算字符数：中文字符计 2，标点/字母/数字计 1。

    例：``"中文。"`` = 2+2+1 = 5 字符（见 search-report-polish-materials.md §1.1）。
    """
    return sum(2 if _is_cjk_char(ch) else 1 for ch in text)


def get_instruction(emotion: str, strength: int = 2, mood_tier: str = "neutral") -> str:
    """按 emotion + strength + mood_tier 拼接 TTS instruction 文案。

    Args:
        emotion: 情绪名（joy/sadness/anger/surprise/fear/smirk/disgust/neutral；
            兼容 sad/angry/love 别名；未知情绪降级为 neutral，不抛异常）
        strength: 强度 1-3（默认 2，无强度修饰）
        mood_tier: 心情分层（furious/upset/neutral/happy/ecstatic）

    Returns:
        str: ≤100 字符的 instruction 文案（中文字符按 2 计）。
    """
    key = str(emotion).lower()
    key = _EMOTION_ALIASES.get(key, key)
    base = INSTRUCTION_LIBRARY.get(key, INSTRUCTION_LIBRARY["neutral"])
    modifier = MOOD_MODIFIERS.get(mood_tier, "")
    strength_mod = STRENGTH_MODIFIERS.get(strength, "")

    instruction = f"{modifier}{strength_mod}{base}"

    # 硬截断兜底：保证 ≤100 字符（保留修饰词，从 base 尾部收缩）
    if _char_len(instruction) > MAX_INSTRUCTION_CHARS:
        budget = MAX_INSTRUCTION_CHARS - _char_len(modifier) - _char_len(strength_mod)
        if budget < 0:
            # 修饰词本身已超限（理论上不会发生），退化返回修饰词
            return f"{modifier}{strength_mod}"
        trimmed = ""
        for ch in base:
            if _char_len(trimmed + ch) > budget:
                break
            trimmed += ch
        instruction = f"{modifier}{strength_mod}{trimmed}"

    return instruction
