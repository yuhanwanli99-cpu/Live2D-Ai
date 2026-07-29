import asyncio
import json
from typing import Dict, Optional, Callable

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
    received_data_buffers: Dict[str, np.ndarray],
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
        except Exception as e:
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
    else:  # mic-audio-end
        user_input = received_data_buffers[client_uid]
        received_data_buffers[client_uid] = np.array([])

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
                websocket=websocket,
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
                websocket=websocket,
                images=images,
                session_emoji=session_emoji,
                metadata=metadata,
            )
        )


async def handle_group_interrupt(
    group_id: str,
    caller_uid: str,
    client_contexts: Dict[str, ServiceContext],
    client_connections: Dict[str, WebSocket],
    current_conversation_tasks: Dict[str, Optional[asyncio.Task]],
) -> None:
    """Handle interrupt signal for a group conversation."""
    logger.debug(f"Handling group interrupt for {group_id} by {caller_uid}")
    if group_id in current_conversation_tasks:
        task = current_conversation_tasks[group_id]
        if task and not task.done():
            task.cancel()
            logger.info(f"Cancelled group conversation task for {group_id}")
            for uid in client_contexts:
                if uid in client_connections:
                    try:
                        await client_connections[uid].send_text(
                            json.dumps(
                                {
                                    "type": "full-text",
                                    "text": "\n\n[interrupted]",
                                }
                            )
                        )
                    except Exception:
                        pass
            current_conversation_tasks[group_id] = None


async def handle_individual_interrupt(
    client_uid: str,
    current_conversation_tasks: Dict[str, Optional[asyncio.Task]],
    websocket: WebSocket,
) -> None:
    """Handle interrupt signal for an individual conversation."""
    logger.debug(f"Handling individual interrupt for {client_uid}")
    if client_uid in current_conversation_tasks:
        task = current_conversation_tasks[client_uid]
        if task and not task.done():
            task.cancel()
            logger.info(f"Cancelled conversation task for {client_uid}")
            try:
                await websocket.send_text(
                    json.dumps(
                        {
                            "type": "full-text",
                            "text": "\n\n[interrupted]",
                        }
                    )
                )
            except Exception:
                pass
            current_conversation_tasks[client_uid] = None
