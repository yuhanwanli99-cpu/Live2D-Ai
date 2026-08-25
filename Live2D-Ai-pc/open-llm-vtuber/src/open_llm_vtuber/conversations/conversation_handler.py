import asyncio
import json
from typing import Any, Dict, Optional, Callable

import numpy as np
from fastapi import WebSocket
from loguru import logger

from ..chat_group import ChatGroupManager
from ..chat_history_manager import store_message
from ..service_context import ServiceContext
from .single_conversation import process_single_conversation
from .conversation_utils import EMOJI_LIST
from .types import GroupConversationState
from prompts import prompt_loader


async def handle_conversation_trigger(
    msg_type: str,
    data: dict,
    client_uid: str,
    context: ServiceContext,
    websocket: WebSocket,
    client_contexts: Dict[str, ServiceContext],
    client_connections: Dict[str, WebSocket],
    chat_group_manager: ChatGroupManager,
    received_data_buffers: Dict[str, list],
    current_conversation_tasks: Dict[str, Optional[asyncio.Task]],
    broadcast_to_group: Callable,
) -> None:
    """Handle triggers that start a conversation"""
    metadata = None

    if msg_type == "ai-speak-signal":
        try:
            prompt_name = "proactive_speak_prompt"
            prompt_file = context.system_config.tool_prompts.get(prompt_name)
            if prompt_file:
                user_input = prompt_loader.load_util(prompt_file)
            else:
                logger.warning("Proactive speak prompt not configured, using default")
                user_input = "Please say something."
        except Exception as e:  # noqa: BLE001
            logger.error(f"Error loading proactive speak prompt: {e}")
            user_input = "Please say something."

        metadata = {
            "proactive_speak": True,
            "skip_memory": True,
            "skip_history": True,
        }

        await websocket.send_text(
            json.dumps(
                {
                    "type": "full-text",
                    "text": "AI wants to speak something...",
                }
            )
        )
    elif msg_type == "text-input":
        user_input = data.get("text", "")
        # P7-03：用户消息到达 → mood_engine.on_user_message_sentiment()（mood 触发点）。
        # 仅文本输入在此触发；mic 输入文本不可得，由 process_single_conversation 在 ASR 后补触发。
        mood_engine = getattr(context, "mood_engine", None)
        if mood_engine is not None:
            try:
                await mood_engine.on_user_message_sentiment(user_input)
            except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                logger.warning(f"mood sentiment trigger failed (ignored): {e}")
    else:  # mic-audio-end
        # 缓冲按 chunk 累积（审计 E-4），这里一次性拼成 ASR 需要的 float32 波形
        chunks = received_data_buffers.get(client_uid) or []
        user_input = (
            np.concatenate(chunks) if chunks else np.array([], dtype=np.float32)
        )
        received_data_buffers[client_uid] = []

    images = data.get("images")
    session_emoji = np.random.choice(EMOJI_LIST)

    group = chat_group_manager.get_client_group(client_uid)
    if group and len(group.members) > 1:
        logger.warning("Group conversations are disabled in streamlined mode.")
        # Use single conversation even for groups
        current_conversation_tasks[client_uid] = asyncio.create_task(
            process_single_conversation(
                user_input=user_input,
                context=context,
                websocket_send=websocket.send_text,
                client_uid=client_uid,
                images=images,
                session_emoji=session_emoji,
                metadata=metadata,
            )
        )
    else:
        # Use client_uid as task key for individual conversations
        current_conversation_tasks[client_uid] = asyncio.create_task(
            process_single_conversation(
                user_input=user_input,
                context=context,
                websocket_send=websocket.send_text,
                client_uid=client_uid,
                images=images,
                session_emoji=session_emoji,
                metadata=metadata,
            )
        )


async def handle_group_interrupt(
    group_id: str,
    heard_response: str,
    current_conversation_tasks: Dict[str, Optional[asyncio.Task]],
    chat_group_manager: ChatGroupManager,
    client_contexts: Dict[str, ServiceContext],
    broadcast_to_group: Callable,
) -> None:
    """Handle interrupt signal for a group conversation.

    PC-L1：签名对齐 websocket_handler 调用点（旧参 caller_uid/client_connections
    已由 chat_group_manager + client_contexts + broadcast_to_group 取代）。
    """
    logger.debug(
        f"Handling group interrupt for {group_id} (heard: {heard_response!r})"
    )
    if group_id in current_conversation_tasks:
        task = current_conversation_tasks[group_id]
        if task and not task.done():
            task.cancel()
            logger.info(f"Cancelled group conversation task for {group_id}")

            # 同步打断 TTS 管线：停止播放、取消在途合成、丢弃旧句 payload
            context = client_contexts.get(group_id)
            tts_manager = getattr(context, "tts_manager", None) if context else None
            if tts_manager is not None:
                try:
                    await tts_manager.interrupt()
                except Exception as e:  # noqa: BLE001 - TTS 打断失败不阻断主取消
                    logger.warning(f"Group TTS interrupt failed (ignored): {e}")

            group = chat_group_manager.groups.get(group_id)
            members = list(group.members) if group else list(client_contexts.keys())
            if members:
                try:
                    await broadcast_to_group(
                        group_members=members,
                        message={
                            "type": "full-text",
                            "text": "\n\n[interrupted]",
                        },
                    )
                except Exception as e:  # noqa: BLE001 - 打断通知失败不阻断任务取消
                    logger.warning(f"Group interrupt broadcast failed (ignored): {e}")
            current_conversation_tasks[group_id] = None


async def _stop_tts_for_interrupt(
    tts_manager: Optional[Any],
    log_ctx: str,
) -> None:
    """Stop TTS pipeline for interruption.  Should be awaited after cancelling
    the conversation task so that no late audio frames are emitted.
    """
    if tts_manager is None:
        return
    try:
        await tts_manager.interrupt()
        logger.debug(f"TTS interrupt completed for {log_ctx}")
    except Exception:  # noqa: BLE001 - TTS interrupt failure must not block cancel
        pass


async def handle_individual_interrupt(
    client_uid: str,
    current_conversation_tasks: Dict[str, Optional[asyncio.Task]],
    context: ServiceContext,
    heard_response: str,
) -> None:
    """Handle interrupt signal for an individual conversation.

    PC-L1：签名对齐 websocket_handler 调用点（旧参 websocket 已由
    context.send_text 取代——ServiceContext 持有会话级 send_text）。
    """
    logger.debug(
        f"Handling individual interrupt for {client_uid} (heard: {heard_response!r})"
    )
    if client_uid in current_conversation_tasks:
        task = current_conversation_tasks[client_uid]
        tts_manager = getattr(context, "tts_manager", None)
        if task and not task.done():
            task.cancel()
            logger.info(f"Cancelled conversation task for {client_uid}")
            await _stop_tts_for_interrupt(tts_manager, client_uid)

            send_text = getattr(context, "send_text", None)
            if send_text is not None:
                try:
                    await send_text(
                        json.dumps(
                            {
                                "type": "full-text",
                                "text": "\n\n[interrupted]",
                            }
                        )
                    )
                except Exception:  # noqa: BLE001
                    pass
            current_conversation_tasks[client_uid] = None
        elif tts_manager is not None:
            await _stop_tts_for_interrupt(tts_manager, client_uid)
