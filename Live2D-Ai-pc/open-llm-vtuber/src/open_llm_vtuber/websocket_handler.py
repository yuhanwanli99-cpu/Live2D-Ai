from typing import Dict, List, Optional, Callable, TypedDict
from fastapi import WebSocket, WebSocketDisconnect
import asyncio
import json
import numpy as np
from loguru import logger

from .service_context import ServiceContext
from .chat_group import (
    ChatGroupManager,
    handle_group_operation,
    handle_client_disconnect,
    broadcast_to_group,
)
from .message_handler import message_handler
from .utils.stream_audio import prepare_audio_payload
from .chat_history_manager import (
    create_new_history,
    get_history,
    delete_history,
    get_history_list,
)
from .config_manager.utils import scan_config_alts_directory, scan_bg_directory
from .conversations.conversation_handler import (
    handle_conversation_trigger,
    handle_group_interrupt,
    handle_individual_interrupt,
)


class WSMessage(TypedDict, total=False):
    """Type definition for WebSocket messages"""

    type: str
    action: Optional[str]
    text: Optional[str]
    audio: Optional[List[float]]
    images: Optional[List[str]]
    history_uid: Optional[str]
    file: Optional[str]
    display_text: Optional[dict]


class WebSocketHandler:
    """Handles WebSocket connections and message routing"""

    def __init__(self, default_context_cache: ServiceContext):
        """Initialize the WebSocket handler with default context"""
        self.client_connections: Dict[str, WebSocket] = {}
        self.client_contexts: Dict[str, ServiceContext] = {}
        self.chat_group_manager = ChatGroupManager()
        self.current_conversation_tasks: Dict[str, Optional[asyncio.Task]] = {}
        self.default_context_cache = default_context_cache
        # 音频累积缓冲：**按 chunk 存 np 数组**，取用时一次性 concatenate。
        # 原实现每收一个 chunk 就 np.append（每次重新分配 + 全量拷贝）→ 长语音 O(n²)；
        # 改为 list 累积后总拷贝量降为 O(n)（审计 E-4）。
        self.received_data_buffers: Dict[str, List[np.ndarray]] = {}

        # Message handlers mapping
        self._message_handlers = self._init_message_handlers()

    def _init_message_handlers(self) -> Dict[str, Callable]:
        """Initialize message type to handler mapping"""
        return {
            "add-client-to-group": self._handle_group_operation,
            "remove-client-from-group": self._handle_group_operation,
            "request-group-info": self._handle_group_info,
            "fetch-history-list": self._handle_history_list_request,
            "fetch-and-set-history": self._handle_fetch_history,
            "create-new-history": self._handle_create_history,
            "delete-history": self._handle_delete_history,
            "interrupt-signal": self._handle_interrupt,
            "mic-audio-data": self._handle_audio_data,
            "mic-audio-end": self._handle_conversation_trigger,
            "raw-audio-data": self._handle_raw_audio_data,
            "text-input": self._handle_conversation_trigger,
            "ai-speak-signal": self._handle_conversation_trigger,
            "fetch-configs": self._handle_fetch_configs,
            "switch-config": self._handle_config_switch,
            "fetch-backgrounds": self._handle_fetch_backgrounds,
            "audio-play-start": self._handle_audio_play_start,
            "request-init-config": self._handle_init_config_request,
            "heartbeat": self._handle_heartbeat,
        }

    async def handle_new_connection(
        self, websocket: WebSocket, client_uid: str
    ) -> None:
        """
        Handle new WebSocket connection setup

        Args:
            websocket: The WebSocket connection
            client_uid: Unique identifier for the client

        Raises:
            Exception: If initialization fails
        """
        try:
            session_service_context = await self._init_service_context(
                websocket.send_text, client_uid
            )

            await self._store_client_data(
                websocket, client_uid, session_service_context
            )

            await self._send_initial_messages(
                websocket, client_uid, session_service_context
            )

            logger.info(f"Connection established for client {client_uid}")

        except Exception as e:
            logger.error(
                f"Failed to initialize connection for client {client_uid}: {e}"
            )
            await self._cleanup_failed_connection(client_uid)
            raise

    async def _store_client_data(
        self,
        websocket: WebSocket,
        client_uid: str,
        session_service_context: ServiceContext,
    ):
        """Store client data and initialize group status"""
        self.client_connections[client_uid] = websocket
        self.client_contexts[client_uid] = session_service_context
        self.received_data_buffers[client_uid] = []

        self.chat_group_manager.client_group_map[client_uid] = ""
        await self.send_group_update(websocket, client_uid)

    async def _send_initial_messages(
        self,
        websocket: WebSocket,
        client_uid: str,
        session_service_context: ServiceContext,
    ):
        """Send initial connection messages to the client"""
        await websocket.send_text(
            json.dumps({"type": "full-text", "text": "Connection established"})
        )

        await websocket.send_text(
            json.dumps(
                {
                    "type": "set-model-and-conf",
                    "model_info": session_service_context.live2d_model.model_info,
                    "conf_name": session_service_context.character_config.conf_name,
                    "conf_uid": session_service_context.character_config.conf_uid,
                    "client_uid": client_uid,
                    "live2d_fps": session_service_context.system_config.live2d_fps,
                }
            )
        )

        # Send initial group status
        await self.send_group_update(websocket, client_uid)

        # 绑定聊天历史（P0-3）：必须在 start-mic 之前，保证第一句话就落盘
        await self._attach_session_history(websocket, session_service_context)

        # Start microphone
        await websocket.send_text(json.dumps({"type": "control", "text": "start-mic"}))

    async def _attach_session_history(
        self, websocket: WebSocket, context: ServiceContext
    ) -> Optional[str]:
        """给新连接绑定聊天历史，并把已有消息回灌给前端（P0-3 聊天历史持久化）。

        为什么必须有这一步：`ServiceContext.history_uid` 初值是空串，而
        `process_single_conversation` 只在 `context.history_uid` 非空时 `store_message`。
        此前只有前端显式发 `create-new-history` 才会绑定 —— 新 renderer 从不发，
        于是**一条消息都没落过盘**（`chat_history/` 目录甚至不存在）。

        策略：**续接最近一次会话**（刷新页面 = 接着聊，符合直觉），没有则新建。
        用户要开新会话时前端发 `create-new-history`（已有处理器）。

        Returns:
            绑定的 history_uid；conf_uid 缺失或创建失败时返回 None。
        """
        conf_uid = getattr(context.character_config, "conf_uid", "") or ""
        if not conf_uid:
            logger.warning("conf_uid 为空，跳过聊天历史绑定（本次对话不会落盘）")
            return None

        try:
            histories = get_history_list(conf_uid) or []
            history_uid = histories[0].get("uid") if histories else ""
            if not history_uid:
                history_uid = create_new_history(conf_uid)
            if not history_uid:
                logger.warning("创建聊天历史失败，本次对话不会落盘")
                return None

            context.history_uid = history_uid
            # 让 LLM 记忆也接上这段历史（否则"界面上有上文、模型却不记得"）
            try:
                context.agent_engine.set_memory_from_history(
                    conf_uid=conf_uid, history_uid=history_uid
                )
            except Exception as e:  # noqa: BLE001 - 记忆恢复失败不该阻断连接
                logger.warning(f"从历史恢复 LLM 记忆失败（忽略）: {e}")

            messages = [
                msg
                for msg in get_history(conf_uid, history_uid)
                if msg.get("role") != "system"
            ]
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "history-data",
                        "history_uid": history_uid,
                        "messages": messages,
                    }
                )
            )
            logger.info(
                f"聊天历史已绑定: {history_uid}（恢复 {len(messages)} 条消息）"
            )
            return history_uid
        except Exception as e:  # noqa: BLE001 - 历史绑定失败不该让连接建立失败
            logger.warning(f"聊天历史绑定失败（本次对话不会落盘）: {e}")
            return None

    async def _init_service_context(
        self, send_text: Callable, client_uid: str
    ) -> ServiceContext:
        """Initialize service context for a new session by cloning the default context"""
        session_service_context = ServiceContext()
        await session_service_context.load_cache(
            config=self.default_context_cache.config.model_copy(deep=True),
            system_config=self.default_context_cache.system_config.model_copy(
                deep=True
            ),
            character_config=self.default_context_cache.character_config.model_copy(
                deep=True
            ),
            live2d_model=self.default_context_cache.live2d_model,
            asr_engine=self.default_context_cache.asr_engine,
            tts_engine=self.default_context_cache.tts_engine,
            vad_engine=self.default_context_cache.vad_engine,
            agent_engine=self.default_context_cache.agent_engine,
            # translate_engine removed - using direct LLM call
            mcp_server_registery=self.default_context_cache.mcp_server_registery,
            tool_adapter=self.default_context_cache.tool_adapter,
            send_text=send_text,
            client_uid=client_uid,
        )
        return session_service_context

    async def handle_websocket_communication(
        self, websocket: WebSocket, client_uid: str
    ) -> None:
        """
        Handle ongoing WebSocket communication

        Args:
            websocket: The WebSocket connection
            client_uid: Unique identifier for the client
        """
        try:
            while True:
                try:
                    data = await websocket.receive_json()
                    message_handler.handle_message(client_uid, data)
                    await self._route_message(websocket, client_uid, data)
                except WebSocketDisconnect:
                    raise
                except json.JSONDecodeError:
                    logger.error("Invalid JSON received")
                    continue
                except Exception as e:
                    logger.error(f"Error processing message: {e}")
                    await websocket.send_text(
                        json.dumps({"type": "error", "message": str(e)})
                    )
                    continue

        except WebSocketDisconnect:
            logger.info(f"Client {client_uid} disconnected")
            raise
        except Exception as e:
            logger.error(f"Fatal error in WebSocket communication: {e}")
            raise

    async def _route_message(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """
        Route incoming message to appropriate handler

        Args:
            websocket: The WebSocket connection
            client_uid: Client identifier
            data: Message data
        """
        msg_type = data.get("type")
        if not msg_type:
            logger.warning("Message received without type")
            return

        handler = self._message_handlers.get(msg_type)
        if handler:
            await handler(websocket, client_uid, data)
        else:
            if msg_type != "frontend-playback-complete":
                logger.warning(f"Unknown message type: {msg_type}")

    async def _handle_group_operation(
        self, websocket: WebSocket, client_uid: str, data: dict
    ) -> None:
        """Handle group-related operations"""
        operation = data.get("type")
        target_uid = data.get(
            "invitee_uid" if operation == "add-client-to-group" else "target_uid"
        )

        await handle_group_operation(
            operation=operation,
            client_uid=client_uid,
            target_uid=target_uid,
            chat_group_manager=self.chat_group_manager,
            client_connections=self.client_connections,
            send_group_update=self.send_group_update,
        )

    async def handle_disconnect(self, client_uid: str) -> None:
        """Handle client disconnection"""
        group = self.chat_group_manager.get_client_group(client_uid)
        if group:
            await handle_group_interrupt(
                group_id=group.group_id,
                heard_response="",
                current_conversation_tasks=self.current_conversation_tasks,
                chat_group_manager=self.chat_group_manager,
                client_contexts=self.client_contexts,
                broadcast_to_group=self.broadcast_to_group,
            )

        await handle_client_disconnect(
            client_uid=client_uid,
            chat_group_manager=self.chat_group_manager,
            client_connections=self.client_connections,
            send_group_update=self.send_group_update,
        )

        # 先取出 context 再清理，否则 get() 恒为 None，close() 成死代码（B7）
        context = self.client_contexts.get(client_uid)

        # Clean up other client data
        self.client_connections.pop(client_uid, None)
        self.client_contexts.pop(client_uid, None)
        self.received_data_buffers.pop(client_uid, None)
        if client_uid in self.current_conversation_tasks:
            task = self.current_conversation_tasks[client_uid]
            if task and not task.done():
                task.cancel()
            self.current_conversation_tasks.pop(client_uid, None)

        # Call context close to clean up resources (e.g., MCPClient)
        if context:
            await context.close()

        logger.info(f"Client {client_uid} disconnected")
        message_handler.cleanup_client(client_uid)

    async def _cleanup_failed_connection(self, client_uid: str) -> None:
        """Clean up failed connection data"""
        context = self.client_contexts.get(client_uid)
        self.client_connections.pop(client_uid, None)
        self.client_contexts.pop(client_uid, None)
        self.received_data_buffers.pop(client_uid, None)
        self.chat_group_manager.client_group_map.pop(client_uid, None)

        if client_uid in self.current_conversation_tasks:
            task = self.current_conversation_tasks[client_uid]
            if task and not task.done():
                task.cancel()
            self.current_conversation_tasks.pop(client_uid, None)

        if context:
            try:
                await context.close()
            except Exception as e:
                logger.warning(f"Error closing failed connection context: {e}")

        message_handler.cleanup_client(client_uid)

    async def broadcast_to_group(
        self, group_members: list[str], message: dict, exclude_uid: str = None
    ) -> None:
        """Broadcasts a message to group members"""
        await broadcast_to_group(
            group_members=group_members,
            message=message,
            client_connections=self.client_connections,
            exclude_uid=exclude_uid,
        )

    async def send_group_update(self, websocket: WebSocket, client_uid: str):
        """Sends group information to a client"""
        group = self.chat_group_manager.get_client_group(client_uid)
        if group:
            current_members = self.chat_group_manager.get_group_members(client_uid)
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "group-update",
                        "members": current_members,
                        "is_owner": group.owner_uid == client_uid,
                    }
                )
            )
        else:
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "group-update",
                        "members": [],
                        "is_owner": False,
                    }
                )
            )

    async def _handle_interrupt(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle conversation interruption"""
        heard_response = data.get("text", "")
        context = self.client_contexts[client_uid]
        group = self.chat_group_manager.get_client_group(client_uid)

        if group and len(group.members) > 1:
            await handle_group_interrupt(
                group_id=group.group_id,
                heard_response=heard_response,
                current_conversation_tasks=self.current_conversation_tasks,
                chat_group_manager=self.chat_group_manager,
                client_contexts=self.client_contexts,
                broadcast_to_group=self.broadcast_to_group,
            )
        else:
            await handle_individual_interrupt(
                client_uid=client_uid,
                current_conversation_tasks=self.current_conversation_tasks,
                context=context,
                heard_response=heard_response,
            )

    async def _handle_history_list_request(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle request for chat history list"""
        context = self.client_contexts[client_uid]
        histories = get_history_list(context.character_config.conf_uid)
        await websocket.send_text(
            json.dumps({"type": "history-list", "histories": histories})
        )

    async def _handle_fetch_history(
        self, websocket: WebSocket, client_uid: str, data: dict
    ):
        """Handle fetching and setting specific chat history"""
        history_uid = data.get("history_uid")
        if not history_uid:
            return

        context = self.client_contexts[client_uid]
        # Update history_uid in service context
        context.history_uid = history_uid
        context.agent_engine.set_memory_from_history(
            conf_uid=context.character_config.conf_uid,
            history_uid=history_uid,
        )

        messages = [
            msg
            for msg in get_history(
                context.character_config.conf_uid,
                history_uid,
            )
            if msg["role"] != "system"
        ]
        await websocket.send_text(
            json.dumps(
                {
                    "type": "history-data",
                    "history_uid": history_uid,  # 前端需要知道"当前是哪一段"
                    "messages": messages,
                }
            )
        )

    async def _handle_create_history(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle creation of new chat history"""
        context = self.client_contexts[client_uid]
        history_uid = create_new_history(context.character_config.conf_uid)
        if history_uid:
            context.history_uid = history_uid
            context.agent_engine.set_memory_from_history(
                conf_uid=context.character_config.conf_uid,
                history_uid=history_uid,
            )
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "new-history-created",
                        "history_uid": history_uid,
                    }
                )
            )

    async def _handle_delete_history(
        self, websocket: WebSocket, client_uid: str, data: dict
    ):
        """Handle deletion of chat history"""
        history_uid = data.get("history_uid")
        if not history_uid:
            return

        context = self.client_contexts[client_uid]
        success = delete_history(
            context.character_config.conf_uid,
            history_uid,
        )
        await websocket.send_text(
            json.dumps(
                {
                    "type": "history-deleted",
                    "success": success,
                    "history_uid": history_uid,
                }
            )
        )
        if history_uid == context.history_uid:
            context.history_uid = None

    def _append_audio(self, client_uid: str, values) -> None:
        """把一段音频样本追加到该会话的缓冲（审计 E-4：list 累积，取用时一次 concatenate）。"""
        chunk = np.asarray(values, dtype=np.float32)
        if chunk.size == 0:
            return
        self.received_data_buffers.setdefault(client_uid, []).append(chunk)

    async def _handle_audio_data(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle incoming audio data"""
        audio_data = data.get("audio", [])
        if audio_data:
            self._append_audio(client_uid, audio_data)

    async def _handle_raw_audio_data(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle incoming raw audio data for VAD processing"""
        context = self.client_contexts[client_uid]
        chunk = data.get("audio", [])
        if chunk:
            for audio_bytes in context.vad_engine.detect_speech(chunk):
                if audio_bytes == b"<|PAUSE|>":
                    await websocket.send_text(
                        json.dumps({"type": "control", "text": "interrupt"})
                    )
                elif audio_bytes == b"<|RESUME|>":
                    pass
                elif len(audio_bytes) > 1024:
                    # Detected audio activity (voice)
                    self._append_audio(
                        client_uid,
                        np.frombuffer(audio_bytes, dtype=np.int16).astype(np.float32),
                    )
                    await websocket.send_text(
                        json.dumps({"type": "control", "text": "mic-audio-end"})
                    )

    async def _handle_conversation_trigger(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle triggers that start a conversation"""
        await handle_conversation_trigger(
            msg_type=data.get("type", ""),
            data=data,
            client_uid=client_uid,
            context=self.client_contexts[client_uid],
            websocket=websocket,
            client_contexts=self.client_contexts,
            client_connections=self.client_connections,
            chat_group_manager=self.chat_group_manager,
            received_data_buffers=self.received_data_buffers,
            current_conversation_tasks=self.current_conversation_tasks,
            broadcast_to_group=self.broadcast_to_group,
        )

    async def inject_text_input(
        self,
        text: str,
        client_uid: Optional[str] = None,
        interrupt: bool = False,
        source: str = "external",
    ) -> tuple[str, Optional[asyncio.Task]]:
        """把一段外部文本注入内部对话链路（等价于前端发 `{"type":"text-input"}`）。

        供 `external_input.py` 的 HTTP 接口调用——**不新开链路**：直接复用
        `_handle_conversation_trigger`，因此 persona / 记忆 / mood / TTS / 口型
        与用户在聊天框里打字完全一致。

        Args:
            text: 要注入的文本。
            client_uid: 目标会话；None 表示最近连接的那个会话。
            interrupt: 目标会话正在对话中时是否打断后重新开始（False → 抛 RuntimeError）。
            source: 来源标记，仅用于日志与前端提示气泡。

        Returns:
            `(client_uid, task)`：实际接收的会话 id 与本轮对话任务（可 await 取回复文本）。

        Raises:
            LookupError: 没有已连接的前端会话，或指定的 client_uid 不存在。
            RuntimeError: 目标会话正在对话中且 `interrupt=False`。
        """
        if not self.client_connections:
            raise LookupError("没有已连接的前端会话：请先打开 Live2D-Ai 页面再发送外部文字")

        target = client_uid or next(reversed(self.client_connections))
        if target not in self.client_connections:
            raise LookupError(f"会话 {target} 不存在或已断开")

        websocket = self.client_connections[target]
        running = self.current_conversation_tasks.get(target)
        if running is not None and not running.done():
            if not interrupt:
                raise RuntimeError(
                    "角色正在说话：请稍后重试，或用 interrupt=true 打断当前发言"
                )
            await handle_individual_interrupt(
                client_uid=target,
                current_conversation_tasks=self.current_conversation_tasks,
                context=self.client_contexts[target],
                heard_response="",
            )

        # 让前端把这句话显示成用户气泡（否则外部注入对用户是"角色自己突然开口"）
        try:
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "external-input",
                        "text": text,
                        "source": source,
                    }
                )
            )
        except Exception as e:  # noqa: BLE001 - 回显失败不影响对话本身
            logger.warning(f"外部注入回显失败（忽略）: {e}")

        logger.info(f"外部文字注入 [{source}] → 会话 {target}: {text[:80]}")
        await self._handle_conversation_trigger(
            websocket,
            target,
            {"type": "text-input", "text": text},
        )
        return target, self.current_conversation_tasks.get(target)

    async def _handle_fetch_configs(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle fetching available configurations"""
        context = self.client_contexts[client_uid]
        config_files = scan_config_alts_directory(context.system_config.config_alts_dir)
        await websocket.send_text(
            json.dumps({"type": "config-files", "configs": config_files})
        )

    async def _handle_config_switch(
        self, websocket: WebSocket, client_uid: str, data: dict
    ):
        """Handle switching to a different configuration"""
        config_file_name = data.get("file")
        if config_file_name:
            context = self.client_contexts[client_uid]
            await context.handle_config_switch(websocket, config_file_name)

    async def _handle_fetch_backgrounds(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle fetching available background images"""
        bg_files = scan_bg_directory()
        await websocket.send_text(
            json.dumps({"type": "background-files", "files": bg_files})
        )

    async def _handle_audio_play_start(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """
        Handle audio playback start notification
        """
        group_members = self.chat_group_manager.get_group_members(client_uid)
        if len(group_members) > 1:
            display_text = data.get("display_text")
            if display_text:
                silent_payload = prepare_audio_payload(
                    audio_path=None,
                    display_text=display_text,
                    actions=None,
                    forwarded=True,
                )
                await self.broadcast_to_group(
                    group_members, silent_payload, exclude_uid=client_uid
                )

    async def _handle_group_info(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle group info request"""
        await self.send_group_update(websocket, client_uid)

    async def _handle_init_config_request(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle request for initialization configuration"""
        context = self.client_contexts.get(client_uid)
        if not context:
            context = self.default_context_cache

        await websocket.send_text(
            json.dumps(
                {
                    "type": "set-model-and-conf",
                    "model_info": context.live2d_model.model_info,
                    "conf_name": context.character_config.conf_name,
                    "conf_uid": context.character_config.conf_uid,
                    "client_uid": client_uid,
                    "live2d_fps": context.system_config.live2d_fps,
                }
            )
        )

    async def broadcast_model_change(self, live2d_model_name: str) -> int:
        """把 Live2D 形象切换**即时**下发给所有已连接会话（P1-4）。

        此前 `POST /api/models/select` 只写 conf.yaml 并回 `requires_restart: true`，
        用户必须重启后端才能看到新形象；而前端 `main.ts` 早就支持
        `set-model-and-conf` 热切换（同 url 只重算相机，不同 url 重新加载）。
        这里补上"服务端主动推送"这一段，把重启需求去掉。

        Args:
            live2d_model_name: 新形象名（对应 `live2d-models/<name>/`）。

        Returns:
            成功热切换的会话数（0 表示没有在线会话，配置仍已落盘）。
        """
        # 先更新共享 cache，使随后新建/重连的会话直接拿到新形象
        self.default_context_cache.init_live2d(live2d_model_name)

        applied = 0
        for client_uid, websocket in list(self.client_connections.items()):
            context = self.client_contexts.get(client_uid)
            if context is None:
                continue
            try:
                context.init_live2d(live2d_model_name)
                await websocket.send_text(
                    json.dumps(
                        {
                            "type": "set-model-and-conf",
                            "model_info": context.live2d_model.model_info,
                            "conf_name": context.character_config.conf_name,
                            "conf_uid": context.character_config.conf_uid,
                            "client_uid": client_uid,
                            "live2d_fps": context.system_config.live2d_fps,
                        }
                    )
                )
                applied += 1
            except Exception as e:  # noqa: BLE001 - 单个会话失败不影响其余
                logger.warning(f"会话 {client_uid} 形象热切换失败（忽略）: {e}")
        logger.info(f"形象切换为 {live2d_model_name}，已热更 {applied} 个会话")
        return applied

    async def _handle_heartbeat(
        self, websocket: WebSocket, client_uid: str, data: WSMessage
    ) -> None:
        """Handle heartbeat messages from clients"""
        try:
            await websocket.send_json({"type": "heartbeat-ack"})
        except Exception as e:
            logger.error(f"Error sending heartbeat acknowledgment: {e}")
