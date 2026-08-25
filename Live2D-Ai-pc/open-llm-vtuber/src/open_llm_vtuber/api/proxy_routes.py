"""Proxy WebSocket route factory."""

from fastapi import APIRouter, WebSocket
from loguru import logger

from ..proxy_handler import ProxyHandler


def init_proxy_route(server_url: str) -> APIRouter:
    """Create API routes for proxy WebSocket connections."""
    router = APIRouter()
    proxy_handler = ProxyHandler(server_url)

    @router.websocket("/proxy-ws")
    async def proxy_endpoint(websocket: WebSocket):
        try:
            await proxy_handler.handle_client_connection(websocket)
        except Exception as exc:
            logger.error(f"Error in proxy connection: {exc}")
            raise

    return router
