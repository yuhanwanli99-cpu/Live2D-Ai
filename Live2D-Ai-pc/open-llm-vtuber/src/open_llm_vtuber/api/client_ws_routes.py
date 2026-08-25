"""Primary client WebSocket route factory."""

from uuid import uuid4

from fastapi import APIRouter, WebSocket
from loguru import logger
from starlette.websockets import WebSocketDisconnect

from ..service_context import ServiceContext
from ..websocket_handler import WebSocketHandler


def init_client_ws_route(
    default_context_cache: ServiceContext = None,
    ws_handler: WebSocketHandler = None,
) -> APIRouter:
    router = APIRouter()
    handler = ws_handler or WebSocketHandler(default_context_cache)

    @router.websocket("/client-ws")
    async def websocket_endpoint(websocket: WebSocket):
        await websocket.accept()
        client_uid = str(uuid4())
        try:
            await handler.handle_new_connection(websocket, client_uid)
            await handler.handle_websocket_communication(websocket, client_uid)
        except WebSocketDisconnect:
            await handler.handle_disconnect(client_uid)
        except Exception as exc:  # noqa: BLE001 - WebSocket boundary cleanup
            logger.error(f"Error in WebSocket connection: {exc}")
            await handler.handle_disconnect(client_uid)
            raise

    return router
