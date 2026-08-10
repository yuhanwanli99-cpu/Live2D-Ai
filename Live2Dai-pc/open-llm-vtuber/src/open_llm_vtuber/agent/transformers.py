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
                try:
                    async for item in raw_stream:
                        if isinstance(item, str):
                            await text_q.put(item)
                        else:
                            await out_q.put(item)
                except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                    logger.warning(f"emo_interceptor pump error: {e}")
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
                            await out_q.put(clean)
                except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                    logger.warning(f"emo_interceptor intercept error: {e}")
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
