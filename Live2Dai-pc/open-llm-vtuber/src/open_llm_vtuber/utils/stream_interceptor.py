r"""
流式 [emo:xxx] 拦截器（P1/P4）

在 LLM token 流中实时检测句首 `[emo:xxx]` / `[emo:xxx_N]` 前置 Token：
  - 命中 → 产出 EmoToken（调用方立即发送 emotion WS 帧 + mood 事件）
  - 剥离 `[emo:xxx]` 后继续产出干净文本 chunk
  - 非法格式 / 未知 emotion / 不闭合 Token → 忽略不崩溃，原样透传

句首探测状态机（契约 contract-pc-v0.md §4 PI-01~03）：
  - 探测期（句首且未命中）：chunk 只累积不 yield（避免跨 chunk 拆 token 时漏命中）
    - 命中关键词 → yield (token, 剩余文本)，转透传模式
    - 累积 ≥ MIN_BUFFER_FOR_MATCH(20) 仍无命中 → 一次性透传累积文本，转透传模式
    - <20 → 继续累积
  - 透传模式：每 chunk 直接 yield (None, chunk)；流结束 flush 探测期残留缓冲
  - 快路径：`_try_extract_emo` 以 `startswith("[emo:")` 短路，正则仅前缀候选执行

约束：
  - `[emo:xxx]` 只可能在句首被检测（LLM prompt 句首约束）
  - 跨 chunk 累积（"[em" + "o:joy]" 分两次到来也能检测）
  - 句首前 20 字符无匹配 → 标记句首已过，不再尝试（契约 P1-06）

契约：contract-pc-v0.md §4（PI-01~06）+ contract-pc-pipeline.md §P1（P1-01 ~ P1-08）
架构：architecture-pc-pipeline.md §3.3 P1 / §4.1.1

> 偏差说明（deviationFromSpec）：
> 契约原文正则 `^\[emo:(\w+)(?:_(\d))?\]` 存在贪婪吞并问题——`[emo:joy_3]` 会被
> `\w+` 整体吞成 `'joy_3'`，导致强度后缀永远捕获不到（P1-02 必然失败）。
> 实现改为 `^\[emo:([a-zA-Z]+)(?:_(\d))?\]`（排除 `_` 于基础名之外），
> 与任务书正则 `[a-z_]+` 语义一致，另兼容大写输入（经 `.lower()` 归一）。
"""

import re
from dataclasses import dataclass
from typing import AsyncIterator, Optional, Tuple

from loguru import logger

# [emo:xxx] 或 [emo:xxx_N] 正则（跨 chunk 累积匹配）
# 注意：基础名类排除 "_"，否则贪婪匹配会把强度后缀吞进 emotion 名（见文件头偏差说明）
_EMO_TOKEN_RE = re.compile(r"^\[emo:([a-zA-Z]+)(?:_(\d))?\]")

# 合法 emotion key 枚举（契约 §P1）
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}

# 合法 strength 范围：1..3，默认 2
VALID_STRENGTHS = (1, 2, 3)
DEFAULT_STRENGTH = 2

# 句首需要累积的最小字符数才放弃匹配（覆盖最长可能格式 [emo:surprise_3] = 17 字符）
MIN_BUFFER_FOR_MATCH = 20


@dataclass
class EmoToken:
    """流式拦截到的情绪 Token（句首约束，position 恒为 0）"""

    emotion: str       # 8 key 枚举，如 joy / anger / surprise ...
    strength: int      # 1-3，默认 2
    raw: str           # 原始 token 文本，如 "[emo:joy_3]"
    position: int      # 在累积文本中的起始位置（始终为 0，句首约束）


class StreamInterceptor:
    """
    流式 LLM 输出拦截器：在 token 流中实时检测 [emo:xxx] 前缀。

    职责：
      1. 接收 LLM str chunk 流
      2. 在累积文本开头检测 [emo:xxx] Token
      3. 命中 → yield EmoToken（调用方立即发送 emotion WS 帧 + mood 事件）
      4. 剥离 [emo:xxx] 后继续 yield 干净文本 chunk

    状态机（P4 重构，契约 PI-01~03）：
      - `_sentence_start=True` 且 `_emo_detected=False` → 句首探测期：只累积不 yield
      - 命中 → 产出 token + 剩余文本，进入透传模式
      - 累积 ≥ MIN_BUFFER_FOR_MATCH 未命中 → 一次性透传，进入透传模式
      - 透传模式：每 chunk 原样 yield，流结束 flush 残留
    """

    # 需累积的最小字符数才尝试匹配（覆盖最长可能格式 [emo:surprise_3] = 17 字符）
    _MIN_BUFFER_FOR_MATCH = MIN_BUFFER_FOR_MATCH

    def __init__(self) -> None:
        self._buffer = ""              # 累积缓冲区（句首探测期）
        self._emo_detected = False     # 当前句子是否已检测到 emo token
        self._sentence_start = True    # 是否在句首探测期（True=探测，False=透传模式）

    def reset_sentence(self) -> None:
        """句子结束时调用，重置 per-sentence 状态（下个句首可再次检测）。

        注意：必须同时清空累积缓冲——若上一句无 [emo:xxx] 且未清空，
        残留文本会成为下一句 [emo:xxx] 锚定匹配的前缀，导致检测失效（P1-07）。
        """
        self._buffer = ""
        self._emo_detected = False
        self._sentence_start = True

    def reset(self) -> None:
        """完全重置（新对话开始时调用）。"""
        self._buffer = ""
        self._emo_detected = False
        self._sentence_start = True

    async def intercept(
        self,
        token_stream: AsyncIterator[str],
    ) -> AsyncIterator[Tuple[Optional[EmoToken], str]]:
        """
        拦截 token 流。

        Yields:
            Tuple[Optional[EmoToken], str]:
                - EmoToken: 检测到的情绪 Token（仅首次命中时非 None）
                - str: 剥离 [emo:xxx] 后的干净文本 chunk

        流结束（或调用方取消）时：若 buffer 中残留未输出文本
        （如未闭合的 "[emo:joy" 前缀 / 探测期 <20 的累积文本），flush 为 (None, text) 透传，不崩溃。
        """
        async for chunk in token_stream:
            if not chunk:
                continue

            # 透传模式：句首已过（命中剥离或 ≥20 放弃探测）→ 每 chunk 直接透传（PI-02）
            if not self._sentence_start:
                yield (None, chunk)
                continue

            # 句首探测期：只累积不 yield（PI-01），避免跨 chunk 拆 token 时漏命中
            self._buffer += chunk

            emo_token, clean_start = self._try_extract_emo(self._buffer)
            if emo_token is not None:
                # 命中 → 产出 token + 剥离后剩余文本，转透传模式
                self._emo_detected = True
                self._sentence_start = False
                clean_text = self._buffer[clean_start:]
                self._buffer = ""
                yield (emo_token, clean_text)
                continue

            # 累积 ≥20 仍未命中 → 放弃探测，一次性透传累积文本并转透传模式（PI-01）
            if len(self._buffer) >= self._MIN_BUFFER_FOR_MATCH:
                self._sentence_start = False
                text = self._buffer
                self._buffer = ""
                yield (None, text)
                continue

            # <20 继续累积，不 yield

        # 流结束：flush 探测期残留缓冲（未闭合 Token / <20 累积文本兜底，PI-02）
        if self._buffer:
            tail = self._buffer
            self._buffer = ""
            yield (None, tail)

    def _try_extract_emo(self, text: str) -> Tuple[Optional[EmoToken], int]:
        """
        尝试从文本开头提取 [emo:xxx] Token。

        快路径（PI-03）：非 "[emo:" 前缀候选直接短路，正则仅前缀候选执行。

        Returns:
            (EmoToken | None, clean_start_index):
                - EmoToken 命中时为对象，clean_start 为剥离后干净文本起始索引
                - 未命中/未知 emotion/非法格式 → (None, 0)
        """
        if not text.startswith("[emo:"):
            return None, 0

        match = _EMO_TOKEN_RE.match(text)
        if not match:
            return None, 0

        emotion = match.group(1).lower()
        strength_str = match.group(2)
        strength = int(strength_str) if strength_str else DEFAULT_STRENGTH

        # 校验 emotion 是否在合法枚举中（P1-03：未知 emotion 原样透传）
        if emotion not in VALID_EMOTIONS:
            logger.warning(f"Unknown emotion in stream: '{emotion}'")
            return None, 0

        # 校验强度范围（非法强度回退默认值 2）
        if strength not in VALID_STRENGTHS:
            strength = DEFAULT_STRENGTH

        raw = match.group(0)
        return (
            EmoToken(emotion=emotion, strength=strength, raw=raw, position=0),
            len(raw),
        )
