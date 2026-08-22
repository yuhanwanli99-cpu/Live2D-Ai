import asyncio
import re
from typing import AsyncIterator, Tuple, Callable, List, Union, Dict, Any
from functools import wraps
from .output_types import Actions, SentenceOutput, DisplayText
from ..utils.stream_interceptor import StreamInterceptor
from ..utils.tts_preprocessor import tts_filter as filter_text
from ..live2d_model import (
    Live2dModel,
    PROTOCOL_EMOTION_INDEX,
    extract_motion_keyword,  # T4.1 关键词兜底（v3-features §1）
)
from ..config_manager import TTSPreprocessorConfig
from ..utils.sentence_divider import SentenceDivider
from ..utils.sentence_divider import SentenceWithTags, TagState
from loguru import logger

# [emo:xxx] / [emo:xxx_N] 标签（P7：防 actions_extractor 把 [emo:joy] 中的 joy 误匹配为旧关键词）
_EMO_TAG_RE = re.compile(r"\[emo:[a-zA-Z]+(?:_\d)?\]")

# 表情 JSON 动作块的关键字：命中即视为“JSON 碎片”，即使 SentenceDivider 被切碎也不该朗读/显示
_JSON_FRAGMENT_KEYS = (
    "expressionMix",
    "motionLabel",
    "parameterOverrides",
    "expressions",
    '"weight"',
    '"name"',
)
_JSON_KEY_VALUE_RE = re.compile(r'"[A-Za-z_][A-Za-z0-9_]*"\s*:')
_JSON_VALUE_RE = re.compile(r':\s*"[^"]*"')


def _looks_like_json_fragment(text: str) -> bool:
    """判断一段文本是否属于 LLM 动作 JSON 的碎片（而非正常对话）。

    覆盖两种常见形态：
      1. 包含动作协议关键字（expressionMix/motionLabel/parameterOverrides 等）
      2. 代码围栏碎片：```json、```、以及以 { 开头但明显不是完整句子的 JSON 片段
    """
    t = text.strip()
    if not t:
        return False
    if "```" in t or t == "json":
        return True
    if any(k in t for k in _JSON_FRAGMENT_KEYS):
        return True
    # 形如 {"xxx": ... 且没有正常结束（缺少闭合的 } 或句子标点）的碎片
    if t.startswith("{") and "}" not in t:
        return True
    if t.startswith("{") and not t.endswith("}") and not t.endswith((".", "!", "?", "。", "！", "？")):
        return True
    # JSON 键值对碎片：:"joy",、"name":"xxx"、{"a": 等
    if (t.startswith((":", ",", "}", "]")) or _JSON_KEY_VALUE_RE.search(t) or _JSON_VALUE_RE.search(t)):
        if not t.rstrip().endswith((".", "!", "?", "。", "！", "？")):
            return True
    return False


def emo_interceptor():
    """流式 [emo:xxx] 拦截装饰器（P1/P7，集成 StreamInterceptor）。

    位置：@sentence_divider 之下（即 sentence_divider 包装本装饰器的外层），
    确保拦截发生在 chat_with_memory 的 token 流之后、分段之前（契约 P7-01）。

    数据流：
      chat_with_memory 产出 原始 str chunk / dict 事件
        → emo_interceptor：str 喂 StreamInterceptor，dict 事件直接透传
        → [emo:xxx] 命中 → 立即 yield {"type": "emo_token", ...} 事件 dict（先于文本）
        → 剥离后的干净文本 str chunk 继续下行 → sentence_divider 分段

    产出：
      - dict {"type": "emo_token", "emotion": ..., "strength": ..., "raw": ...}
        在命中时刻立即产出（P7-02 表情先行）
      - 干净文本 str chunk
      - 非文本事件（tool 状态等 dict）原样透传

    异常兜底（P7-05）：StreamInterceptor / 上游流异常 → 记录日志、透传剩余，不崩溃。
    """

    def decorator(func):
        @wraps(func)
        async def wrapper(
            *args, **kwargs
        ) -> AsyncIterator[Union[str, Dict[str, Any]]]:
            interceptor = StreamInterceptor()
            raw_stream = func(*args, **kwargs)

            text_q: asyncio.Queue = asyncio.Queue(maxsize=64)
            out_q: asyncio.Queue = asyncio.Queue(maxsize=64)

            async def pump() -> None:
                """分流：str → interceptor 输入队列；dict 事件 → 输出队列直接透传。"""
                from .stateless_llm.openai_compatible_llm import ReasoningDelta

                try:
                    async for item in raw_stream:
                        if isinstance(item, ReasoningDelta):
                            # 思维链旁路：直接下行到 single_conversation 单独下发，
                            # 不进 emo 拦截 / 句子分段 / TTS（ReasoningDelta 是 str
                            # 子类，必须先于 isinstance(item, str) 判断）
                            await out_q.put(
                                {"type": "reasoning_delta", "text": str(item)}
                            )
                        elif isinstance(item, str):
                            await text_q.put(item)
                        else:
                            await out_q.put(item)
                except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                    # 不崩溃（保持降级语义），但**必须让用户看见**：
                    # 上游硬失败（未配置 API Key / 401 / 网络不可达）此前只记 warning，
                    # 前端拿不到任何消息 → 用户只看到"角色不说话"。现在额外下发
                    # stream_error 事件，由 single_conversation 转成 {"type":"error"} 发给前端。
                    logger.warning(f"emo_interceptor pump error: {e}")
                    await out_q.put({"type": "stream_error", "message": str(e)})
                finally:
                    await text_q.put(None)  # 文本流 EOF
                    await out_q.put("__eof__")

            async def text_source() -> AsyncIterator[str]:
                """从 text_q 产出纯文本流（供 StreamInterceptor.intercept 消费）。"""
                while True:
                    item = await text_q.get()
                    if item is None:
                        break
                    yield item

            async def consume() -> None:
                """运行 StreamInterceptor：命中 emo → 输出事件 dict；干净文本 → 输出。"""
                try:
                    async for emo_token, clean in interceptor.intercept(text_source()):
                        if emo_token is not None:
                            await out_q.put(
                                {
                                    "type": "emo_token",
                                    "emotion": emo_token.emotion,
                                    "strength": emo_token.strength,
                                    "raw": emo_token.raw,
                                }
                            )
                        if clean:
                            # P1-2 流式上屏：token 级增量事件（先于句子切分与 TTS 到达前端）。
                            # 这里**只是加一路旁听事件**，clean 文本照原样继续下行给
                            # sentence_divider/TTS —— 分段与语音链路一字未改。
                            await out_q.put({"type": "text_delta", "text": clean})
                            await out_q.put(clean)
                except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                    logger.warning(f"emo_interceptor intercept error: {e}")
                    await out_q.put({"type": "stream_error", "message": str(e)})
                finally:
                    await out_q.put("__eof__")

            pump_task = asyncio.create_task(pump())
            consume_task = asyncio.create_task(consume())
            eofs = 0
            try:
                while eofs < 2:
                    item = await out_q.get()
                    if item == "__eof__":
                        eofs += 1
                        continue
                    yield item
            finally:
                for t in (pump_task, consume_task):
                    if not t.done():
                        t.cancel()

        return wrapper

    return decorator



def sentence_divider(
    faster_first_response: bool = True,
    segment_method: str = "pysbd",
    valid_tags: List[str] = None,
):
    """
    Decorator that transforms token stream into sentences with tags

    Args:
        faster_first_response: bool - Whether to enable faster first response
        segment_method: str - Method for sentence segmentation
        valid_tags: List[str] - List of valid tags to process
    """

    def decorator(
        func: Callable[
            ..., AsyncIterator[Union[str, Dict[str, Any]]]
        ],  # Expects str or dict
    ) -> Callable[
        ..., AsyncIterator[Union[SentenceWithTags, Dict[str, Any]]]
    ]:  # Yields SentenceWithTags or dict
        @wraps(func)
        async def wrapper(
            *args, **kwargs
        ) -> AsyncIterator[Union[SentenceWithTags, Dict[str, Any]]]:
            divider = SentenceDivider(
                faster_first_response=faster_first_response,
                segment_method=segment_method,
                valid_tags=valid_tags or [],
            )
            stream_from_func = func(*args, **kwargs)

            # Process the mixed stream using the updated SentenceDivider
            async for item in divider.process_stream(stream_from_func):
                if isinstance(item, SentenceWithTags):
                    logger.debug(f"sentence_divider yielding sentence: {item}")
                elif isinstance(item, dict):
                    logger.debug(f"sentence_divider yielding dict: {item}")
                yield item  # Yield either SentenceWithTags or dict
            # Flushing is handled within divider.process_stream

        return wrapper

    return decorator


def actions_extractor(live2d_model: Live2dModel):
    """
    Decorator that extracts actions from sentences, passing through dicts.
    """

    def decorator(
        func: Callable[
            ..., AsyncIterator[Union[SentenceWithTags, Dict[str, Any]]]
        ],  # Input type hint
    ) -> Callable[
        ..., AsyncIterator[Union[Tuple[SentenceWithTags, Actions], Dict[str, Any]]]
    ]:  # Output type hint
        @wraps(func)
        async def wrapper(
            *args, **kwargs
        ) -> AsyncIterator[
            Union[Tuple[SentenceWithTags, Actions], Dict[str, Any]]
        ]:  # Yield type hint
            stream = func(*args, **kwargs)
            async for item in stream:
                if isinstance(item, SentenceWithTags):
                    sentence = item
                    actions = Actions()
                    # Only extract actions for non-tag text
                    if not any(
                        tag.state in [TagState.START, TagState.END]
                        for tag in sentence.tags
                    ):
                        # P7 兜底：剥离 [emo:xxx] 标签（即使 emo_interceptor 未命中，
                        # 也防 extract_expression_mix/extract_emotion 把 [emo:joy] 中的
                        # joy 误匹配为旧关键词标签——架构 R5）
                        sentence.text = _EMO_TAG_RE.sub("", sentence.text)

                        # 兜底：若 SentenceDivider 仍把动作 JSON 切碎，直接丢弃碎片，
                        # 避免 `json`、`:"joy",` 这类内容被朗读/上屏。
                        if _looks_like_json_fragment(sentence.text):
                            sentence.text = ""
                            yield sentence, actions
                            continue

                        # M3 结构化表情 JSON 路径优先（M1 §3.2 + M2-M3 §2.3 接线）
                        structured = live2d_model.extract_expression_mix(
                            sentence.text
                        )
                        if structured is not None:
                            # 剥离 JSON 块 → 写回干净文本，下游 display/TTS 自动不读 JSON
                            sentence.text = live2d_model.strip_expression_json(
                                sentence.text
                            )
                            if structured["expressionMix"]:
                                actions.expressions = [
                                    PROTOCOL_EMOTION_INDEX[item["name"]]
                                    for item in structured["expressionMix"]
                                ]
                            if structured["parameterOverrides"]:
                                actions.parameterOverrides = structured[
                                    "parameterOverrides"
                                ]
                            if structured["motionLabel"]:
                                # v3-fixes §4 归一：LLM 显式 motionLabel 优先（trim+lowercase，
                                # 与 Android resolve() 对齐；前端 MOTION_ALIAS_MAP 大小写敏感查表不再 miss）
                                actions.motion = structured["motionLabel"].strip().lower()
                            else:
                                # T4.1 兜底：LLM 未输出 motionLabel → 关键词子串匹配（K2/K5）
                                actions.motion = extract_motion_keyword(sentence.text)
                        else:
                            # 降级：现有关键词路径，行为不变
                            expressions = live2d_model.extract_emotion(sentence.text)
                            if expressions:
                                actions.expressions = expressions
                            # T4.1 兜底：降级路径同样补关键词（K3，PC 与 Android 双防线对齐）
                            actions.motion = extract_motion_keyword(sentence.text)
                    yield sentence, actions  # Yield the tuple
                elif isinstance(item, dict):
                    # Pass through dictionaries
                    yield item
                else:
                    logger.warning(
                        f"actions_extractor received unexpected type: {type(item)}"
                    )

        return wrapper

    return decorator


def display_processor():
    """
    Decorator that processes text for display, passing through dicts.
    """

    def decorator(
        func: Callable[
            ..., AsyncIterator[Union[Tuple[SentenceWithTags, Actions], Dict[str, Any]]]
        ],  # Input type hint
    ) -> Callable[
        ...,
        AsyncIterator[
            Union[Tuple[SentenceWithTags, DisplayText, Actions], Dict[str, Any]]
        ],
    ]:  # Output type hint
        @wraps(func)
        async def wrapper(
            *args, **kwargs
        ) -> AsyncIterator[
            Union[Tuple[SentenceWithTags, DisplayText, Actions], Dict[str, Any]]
        ]:  # Yield type hint
            stream = func(*args, **kwargs)

            async for item in stream:
                if (
                    isinstance(item, tuple)
                    and len(item) == 2
                    and isinstance(item[0], SentenceWithTags)
                ):
                    sentence, actions = item
                    text = sentence.text
                    # Handle think tag states
                    for tag in sentence.tags:
                        if tag.name == "think":
                            if tag.state == TagState.START:
                                text = "("
                            elif tag.state == TagState.END:
                                text = ")"

                    display = DisplayText(text=text)  # Simplified DisplayText creation
                    yield sentence, display, actions  # Yield the tuple
                elif isinstance(item, dict):
                    # Pass through dictionaries
                    yield item
                else:
                    logger.warning(
                        f"display_processor received unexpected type: {type(item)}"
                    )

        return wrapper

    return decorator


def tts_filter(
    tts_preprocessor_config: TTSPreprocessorConfig = None,
):
    """
    Decorator that filters text for TTS, passing through dicts.
    Skips TTS for think tag content.
    """

    def decorator(
        func: Callable[
            ...,
            AsyncIterator[
                Union[Tuple[SentenceWithTags, DisplayText, Actions], Dict[str, Any]]
            ],
        ],  # Input type hint
    ) -> Callable[
        ..., AsyncIterator[Union[SentenceOutput, Dict[str, Any]]]
    ]:  # Output type hint
        @wraps(func)
        async def wrapper(
            *args, **kwargs
        ) -> AsyncIterator[Union[SentenceOutput, Dict[str, Any]]]:  # Yield type hint
            stream = func(*args, **kwargs)
            config = tts_preprocessor_config or TTSPreprocessorConfig()

            async for item in stream:
                if (
                    isinstance(item, tuple)
                    and len(item) == 3
                    and isinstance(item[1], DisplayText)
                ):
                    sentence, display, actions = item
                    if any(tag.name == "think" for tag in sentence.tags):
                        tts = ""
                    else:
                        tts = filter_text(
                            text=display.text,
                            remove_special_char=config.remove_special_char,
                            ignore_brackets=config.ignore_brackets,
                            ignore_parentheses=config.ignore_parentheses,
                            ignore_asterisks=config.ignore_asterisks,
                            ignore_angle_brackets=config.ignore_angle_brackets,
                        )

                    logger.debug(f"[{display.name}] display: {display.text}")
                    logger.debug(f"[{display.name}] tts: {tts}")

                    yield SentenceOutput(
                        display_text=display,
                        tts_text=tts,
                        actions=actions,
                    )
                elif isinstance(item, dict):
                    # Pass through dictionaries
                    yield item
                else:
                    logger.warning(f"tts_filter received unexpected type: {type(item)}")

        return wrapper

    return decorator
