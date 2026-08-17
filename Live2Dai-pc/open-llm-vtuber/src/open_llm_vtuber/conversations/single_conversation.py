from typing import Union, List, Dict, Any, Optional
import asyncio
import json
import time
from loguru import logger
import numpy as np

from .conversation_utils import (
    create_batch_input,
    process_agent_output,
    send_conversation_start_signals,
    process_user_input,
    finalize_conversation_turn,
    cleanup_conversation,
    EMOJI_LIST,
)
from .types import WebSocketSend
from .tts_manager import TTSTaskManager
from ..chat_history_manager import store_message
from ..service_context import ServiceContext
from ..utils.stream_audio import prepare_emotion_payload

# Import necessary types from agent outputs
from ..agent.output_types import SentenceOutput, AudioOutput


async def _send_emotion_frame(
    websocket_send: WebSocketSend,
    emo_data: dict,
) -> None:
    """构造并发送 emotion WS 帧（P7-02：检测到 [emo:xxx] 立即发送，先于 audio）。

    parameterOverrides：优先取调用方解析的 Live2D 参数（未来 resolve_emotion_parameters），
    否则空 dict（前端仍可按 emotion+strength 应用）。所有异常降级为日志，不崩溃（P7-05）。
    """
    frame = prepare_emotion_payload(
        emotion=emo_data.get("emotion", "neutral"),
        strength=emo_data.get("strength", 2),
        parameter_overrides=emo_data.get("parameterOverrides") or {},
        fade_ms=emo_data.get("fadeMs", 400),
    )
    await websocket_send(json.dumps(frame))


async def _safe_mood(mood_engine, method: str, *args) -> None:
    """mood 事件调用兜底（P7-05：mood 模块异常不影响对话主链路）。"""
    if mood_engine is None:
        return
    try:
        await getattr(mood_engine, method)(*args)
    except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
        logger.warning(f"mood_engine.{method} failed (ignored): {e}")


_VISION_PROMPT = (
    "You are a screenshot analysis assistant. Describe the current screen content "
    "briefly but thoroughly. Focus on visible applications, windows, code, "
    "documents, images, or any content the user might be referring to. "
    "Your description will be used as context for a conversation agent."
)


async def _get_vision_context(
    context: ServiceContext,
    input_text: str,
) -> str:
    """
    If vision is enabled, capture a screenshot and prepend its description
    to the user's input text for richer conversational context.

    Returns the original or enhanced input text.
    """
    if context.vision_handler is None:
        return input_text

    logger.debug("Vision pipeline: capturing screenshot for context...")
    try:
        description = await context.vision_handler.capture_and_describe(
            prompt=_VISION_PROMPT
        )
        if description:
            # Prepend vision context to the user's input
            enhanced = f"[Screen Context: {description}]\n\n{input_text}"
            logger.info(f"Vision context injected ({len(description)} chars).")
            return enhanced
        else:
            logger.debug("Vision returned no description, using raw input.")
            return input_text
    except Exception as e:
        logger.warning(f"Vision pipeline error (non-fatal): {e}")
        return input_text


async def process_single_conversation(
    context: ServiceContext,
    websocket_send: WebSocketSend,
    client_uid: str,
    user_input: Union[str, np.ndarray],
    images: Optional[List[Dict[str, Any]]] = None,
    session_emoji: str = np.random.choice(EMOJI_LIST),
    metadata: Optional[Dict[str, Any]] = None,
) -> str:
    """Process a single-user conversation turn

    Args:
        context: Service context containing all configurations and engines
        websocket_send: WebSocket send function
        client_uid: Client unique identifier
        user_input: Text or audio input from user
        images: Optional list of image data
        session_emoji: Emoji identifier for the conversation
        metadata: Optional metadata for special processing flags

    Returns:
        str: Complete response text
    """
    # Create TTSTaskManager for this conversation
    # P2-3 (spoq-fix): 段级并发上限从 conf 读取（防御 None → 默认 2）
    _tts_conf = getattr(context, "character_config", None)
    _tts_conf = getattr(_tts_conf, "tts_config", None) if _tts_conf else None
    tts_manager = TTSTaskManager(
        concurrency_limit=getattr(_tts_conf, "concurrency_limit", 2) or 2,
        viseme_mode=getattr(_tts_conf, "viseme_mode", "rms") or "rms",
    )
    full_response = ""  # Initialize full_response here
    mood_engine = getattr(context, "mood_engine", None)
    _latency_start = time.monotonic()

    try:
        # Send initial signals
        await send_conversation_start_signals(websocket_send)
        logger.info(f"New Conversation Chain {session_emoji} started!")

        # P7-03：对话开始 → mood_engine.start()（衰减任务启动；异常降级不阻塞）
        await _safe_mood(mood_engine, "start")

        # Process user input
        input_text = await process_user_input(
            user_input, context.asr_engine, websocket_send
        )
        logger.info(
            f"[LATENCY] user_input_ready_ms={(time.monotonic() - _latency_start) * 1000:.0f}"
        )

        # P7-03：用户消息到达 → mood_engine.on_user_message_sentiment()。
        # 文本输入已在 conversation_handler 触发过（防双计）；此处仅补 mic（np 输入）路径。
        if not isinstance(user_input, str):
            await _safe_mood(mood_engine, "on_user_message_sentiment", input_text)

        # Capture screenshot and inject vision context (if enabled)
        input_text = await _get_vision_context(context, input_text)

        # Create batch input
        batch_input = create_batch_input(
            input_text=input_text,
            images=images,
            from_name=context.character_config.human_name,
            metadata=metadata,
        )

        # Store user message (check if we should skip storing to history)
        skip_history = metadata and metadata.get("skip_history", False)
        if context.history_uid and not skip_history:
            store_message(
                conf_uid=context.character_config.conf_uid,
                history_uid=context.history_uid,
                role="human",
                content=input_text,
                name=context.character_config.human_name,
            )

        if skip_history:
            logger.debug("Skipping storing user input to history (proactive speak)")

        logger.info(f"User input: {input_text}")
        if images:
            logger.info(f"With {len(images)} images")

        try:
            # agent.chat yields Union[SentenceOutput, Dict[str, Any]]
            agent_output_stream = context.agent_engine.chat(batch_input)
            _first_llm_item_logged = False

            async for output_item in agent_output_stream:
                if not _first_llm_item_logged:
                    logger.info(
                        f"[LATENCY] first_llm_item_ms={(time.monotonic() - _latency_start) * 1000:.0f}"
                    )
                    _first_llm_item_logged = True
                if (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "tool_call_status"
                ):
                    # Handle tool status event: send WebSocket message
                    output_item["name"] = context.character_config.character_name
                    logger.debug(f"Sending tool status update: {output_item}")

                    await websocket_send(json.dumps(output_item))

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "emo_token"
                ):
                    # P7-02：StreamInterceptor 命中 [emo:xxx] → 立即发 emotion 帧（先于 audio）
                    # + mood_engine.on_emotion_event()（流式心情事件）。异常降级为日志（P7-05）。
                    try:
                        await _send_emotion_frame(websocket_send, output_item)
                        await _safe_mood(
                            mood_engine,
                            "on_emotion_event",
                            output_item.get("emotion", "neutral"),
                            output_item.get("strength", 2),
                        )
                    except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                        logger.warning(f"emo_token handling failed (ignored): {e}")

                elif isinstance(output_item, (SentenceOutput, AudioOutput)):
                    # Handle SentenceOutput or AudioOutput
                    # PC-L4: 当前 mood 分层传入 TTS（P7 mood_engine 产出；
                    # 每句现取以反映流式 emotion 事件对 mood 的实时影响；异常降级 neutral 不阻断）
                    _mood_tier = "neutral"
                    if mood_engine is not None:
                        try:
                            _mood_tier = mood_engine.get_mood_tier().value
                        except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                            logger.warning(
                                f"mood_engine.get_mood_tier failed (ignored): {e}"
                            )
                    response_part = await process_agent_output(
                        output=output_item,
                        character_config=context.character_config,
                        live2d_model=context.live2d_model,
                        tts_engine=context.tts_engine,
                        websocket_send=websocket_send,  # Pass websocket_send for audio/tts messages
                        tts_manager=tts_manager,
                        translate_engine=context.translate_engine,
                        mood_tier=_mood_tier,
                    )
                    # Ensure response_part is treated as a string before concatenation
                    response_part_str = (
                        str(response_part) if response_part is not None else ""
                    )
                    full_response += response_part_str  # Accumulate text response
                else:
                    logger.warning(
                        f"Received unexpected item type from agent chat stream: {type(output_item)}"
                    )
                    logger.debug(f"Unexpected item content: {output_item}")

        except Exception as e:
            logger.exception(
                f"Error processing agent response stream: {e}"
            )  # Log with stack trace
            await websocket_send(
                json.dumps(
                    {
                        "type": "error",
                        "message": f"Error processing agent response: {str(e)}",
                    }
                )
            )
            # full_response will contain partial response before error
        # --- End processing agent response ---

        # Wait for any pending TTS tasks
        if tts_manager.task_list:
            await asyncio.gather(*tts_manager.task_list)
            await websocket_send(json.dumps({"type": "backend-synth-complete"}))

        await finalize_conversation_turn(
            tts_manager=tts_manager,
            websocket_send=websocket_send,
            client_uid=client_uid,
        )

        if context.history_uid and full_response:  # Check full_response before storing
            store_message(
                conf_uid=context.character_config.conf_uid,
                history_uid=context.history_uid,
                role="ai",
                content=full_response,
                name=context.character_config.character_name,
                avatar=context.character_config.avatar,
            )
            logger.info(f"AI response: {full_response}")

        return full_response  # Return accumulated full_response

    except asyncio.CancelledError:
        logger.info(f"🤡👍 Conversation {session_emoji} cancelled because interrupted.")
        raise
    except Exception as e:
        logger.error(f"Error in conversation chain: {e}")
        await websocket_send(
            json.dumps({"type": "error", "message": f"Conversation error: {str(e)}"})
        )
        raise
    finally:
        # P7-03：对话结束 → mood_engine.stop()（幂等；异常降级不阻塞清理）
        await _safe_mood(mood_engine, "stop")
        cleanup_conversation(tts_manager, session_emoji)
