"""
Open-LLM-VTuber Server
========================
This module contains the WebSocket server for Open-LLM-VTuber, which handles
the WebSocket connections, serves static files, and manages the web tool.
It uses FastAPI for the server and Starlette for static file serving.
"""

import os
import shutil

from fastapi import FastAPI
from starlette.middleware.cors import CORSMiddleware
from starlette.responses import Response
from starlette.staticfiles import StaticFiles as StarletteStaticFiles

from .backgrounds import BackgroundStore, init_background_routes
from .config_manager.utils import Config
from .config_transfer import init_config_transfer_routes
from .external_input import ExternalInputGate, init_external_input_routes
from .frontend_version import init_frontend_version_routes
from .live_event import LiveEventGate, init_live_event_routes
from .model_upload import init_model_upload_routes
from .plugin_routes import init_plugin_routes
from .plugins.manager import PluginManager
from .routes import (
    init_client_ws_route,
    init_config_routes,
    init_dev_routes,
    init_proxy_route,
    init_webtool_routes,
)
from .service_context import ServiceContext
from .voice_library import init_voice_library_routes
from .websocket_handler import WebSocketHandler


# Create a custom StaticFiles class that adds CORS headers
class CORSStaticFiles(StarletteStaticFiles):
    """
    Static files handler that adds CORS headers to all responses.
    Needed because Starlette StaticFiles might bypass standard middleware.
    """

    async def get_response(self, path: str, scope):
        response = await super().get_response(path, scope)

        # Add CORS headers to all responses
        response.headers["Access-Control-Allow-Origin"] = "*"
        response.headers["Access-Control-Allow-Methods"] = "GET, OPTIONS"
        response.headers["Access-Control-Allow-Headers"] = "*"

        if path.endswith(".js"):
            response.headers["Content-Type"] = "application/javascript"

        # spoq-fix：禁用缓存——前端频繁迭代（bundle hash 变化），缓存的 index.html 会引用旧 bundle
        # 导致用户看到旧版页面（如缺失聊天面板）。etag 协商仍允许 304，但不再长期缓存。
        response.headers["Cache-Control"] = "no-cache, no-store, must-revalidate"
        response.headers["Pragma"] = "no-cache"

        return response


class AvatarStaticFiles(CORSStaticFiles):
    """
    Avatar files handler with security restrictions and CORS headers
    """

    async def get_response(self, path: str, scope):
        allowed_extensions = (".jpg", ".jpeg", ".png", ".gif", ".svg")
        if not any(path.lower().endswith(ext) for ext in allowed_extensions):
            return Response("Forbidden file type", status_code=403)
        response = await super().get_response(path, scope)
        return response


class WebSocketServer:
    """
    API server for Open-LLM-VTuber. This contains the websocket endpoint for the client, hosts the web tool, and serves static files.

    Creates and configures a FastAPI app, registers all routes
    (WebSocket, web tools, proxy) and mounts static assets with CORS.

    Args:
        config (Config): Application configuration containing system settings.
        default_context_cache (ServiceContext, optional):
            Pre‑initialized service context for sessions' service context to reference to.
            **If omitted, `initialize()` method needs to be called to load service context.**

    Notes:
        - If default_context_cache is omitted, call `await initialize()` to load service context cache.
        - Use `clean_cache()` to clear and recreate the local cache directory.
    """

    def __init__(self, config: Config, default_context_cache: ServiceContext = None):
        self.app = FastAPI(title="Open-LLM-VTuber Server")  # Added title for clarity
        self.config = config
        self.default_context_cache = (
            default_context_cache or ServiceContext()
        )  # Use provided context or initialize a new empty one waiting to be loaded
        # It will be populated during the initialize method call

        # Add global CORS middleware
        self.app.add_middleware(
            CORSMiddleware,
            allow_origins=["*"],
            allow_credentials=True,
            allow_methods=["*"],
            allow_headers=["*"],
        )

        # Include routes, passing the context instance
        # The context will be populated during the initialize step
        # 共享一个 WebSocketHandler 实例：既服务 /client-ws，也让配置 API 的
        # apply=true 能遍历 client_contexts 对已连接会话热更（B3）。
        self.ws_handler = WebSocketHandler(self.default_context_cache)
        self.app.include_router(
            init_client_ws_route(
                default_context_cache=self.default_context_cache,
                ws_handler=self.ws_handler,
            ),
        )
        self.app.include_router(
            init_webtool_routes(default_context_cache=self.default_context_cache),
        )
        # 轻量设置 Dashboard API（需在 static mount 前注册）
        self.app.include_router(
            init_config_routes(
                config_path="conf.yaml",
                default_context_cache=self.default_context_cache,
                ws_handler=self.ws_handler,
            )
        )
        # 本地开发者调试工具（默认关闭，localhost-only）
        self.app.include_router(
            init_dev_routes(
                default_context_cache=self.default_context_cache,
                ws_handler=self.ws_handler,
            )
        )

        # 插件注册中心：注册原生插件并提供 /api/plugins 管理接口。
        # 必须在所有需要门控的路由之前创建，供这些路由做启用/禁用门控。
        self.plugin_manager = PluginManager()
        self.plugin_manager.load_native()
        self.app.include_router(init_plugin_routes(self.plugin_manager))
        # 启动可观测（任务定版）：逐条打印插件与原生工具是否真的生效，
        # 让「插件是否工作」在启动日志里可直接回答。
        from .logsetup import get_logger
        from .plugins.health import startup_report

        _plugin_log = get_logger("plugins")
        for line in startup_report(self.plugin_manager):
            if "未生效" in line or "异常" in line:
                _plugin_log.warning(line)
            else:
                _plugin_log.info(line)
        # 让每个会话上下文都能访问到同一个插件宿主（WebSocketHandler 会透传）。
        self.default_context_cache.plugin_manager = self.plugin_manager

        # JSON 配置导入/导出（需在 static mount 前注册）
        self.app.include_router(
            init_config_transfer_routes(
                config_path="conf.yaml",
                plugin_manager=self.plugin_manager,
            )
        )

        # 外部文字接入（默认关闭，由前端「设置 → 外部接入」开关控制；见 external_input.py）
        self.external_input_gate = ExternalInputGate().load()
        self.app.include_router(
            init_external_input_routes(
                gate=self.external_input_gate,
                ws_handler=self.ws_handler,
                plugin_manager=self.plugin_manager,
            )
        )

        # 角色背景图上传 / 选择（静态目录 backgrounds/ 由下方 /bg 挂载对外提供）
        self.background_store = BackgroundStore().load()
        self.app.include_router(init_background_routes(store=self.background_store))

        # 通用直播/弹幕事件通道：POST /api/live/event → WS 广播到所有在线前端
        self.live_event_gate = LiveEventGate().load()
        self.app.include_router(
            init_live_event_routes(
                gate=self.live_event_gate,
                ws_handler=self.ws_handler,
                plugin_manager=self.plugin_manager,
            )
        )

        # Live2D 模型 zip 上传导入：POST /api/models/upload → 安全解压 + registry 更新 + 热切换
        self.app.include_router(
            init_model_upload_routes(
                ws_handler=self.ws_handler,
                plugin_manager=self.plugin_manager,
            )
        )

        # 声音克隆资产管理：POST /api/voice/* → 上传/选用/应用/删除参考音频
        self.app.include_router(
            init_voice_library_routes(plugin_manager=self.plugin_manager)
        )

        # Initialize and include proxy routes if proxy is enabled
        system_config = config.system_config
        if hasattr(system_config, "enable_proxy") and system_config.enable_proxy:
            # Construct the server URL for the proxy
            host = system_config.host
            port = system_config.port
            server_url = f"ws://{host}:{port}/client-ws"
            self.app.include_router(
                init_proxy_route(server_url=server_url),
            )

        # Mount cache directory first (to ensure audio file access)
        if not os.path.exists("cache"):
            os.makedirs("cache")
        self.app.mount(
            "/cache",
            CORSStaticFiles(directory="cache"),
            name="cache",
        )

        # Public PC Preview intentionally ships without model assets. Create the
        # import target before mounting so a fresh archive can start and let the
        # user upload a lawfully obtained model from the settings UI.
        if not os.path.exists("live2d-models"):
            os.makedirs("live2d-models")
        self.app.mount(
            "/live2d-models",
            CORSStaticFiles(directory="live2d-models"),
            name="live2d-models",
        )
        # backgrounds/ 可能被用户清空/删除（内容 gitignore）→ 挂载前确保存在，
        # 否则 StaticFiles 会在启动时直接抛错。
        if not os.path.exists("backgrounds"):
            os.makedirs("backgrounds")
        self.app.mount(
            "/bg",
            CORSStaticFiles(directory="backgrounds"),
            name="backgrounds",
        )
        # "avatars" is gitignored (user-uploaded content), so it does not exist
        # on a fresh clone; create it before mounting to avoid a startup crash.
        if not os.path.exists("avatars"):
            os.makedirs("avatars")
        self.app.mount(
            "/avatars",
            AvatarStaticFiles(directory="avatars"),
            name="avatars",
        )

        # Mount web tool directory separately from frontend
        self.app.mount(
            "/web-tool",
            CORSStaticFiles(directory="web_tool", html=True),
            name="web_tool",
        )

        # P3: 前端版本上报（需在 static mount 前注册，避免被 catch-all 吞掉）
        self.app.include_router(init_frontend_version_routes())

        # Mount main frontend last (as catch-all)
        self.app.mount(
            "/",
            CORSStaticFiles(directory="frontend", html=True),
            name="frontend",
        )

    async def initialize(self):
        """Asynchronously load the service context from config.
        Calling this function is needed if default_context_cache was not provided to the constructor."""
        await self.default_context_cache.load_from_config(self.config)

    @staticmethod
    def clean_cache():
        """Clean the cache directory by removing and recreating it."""
        cache_dir = "cache"
        if os.path.exists(cache_dir):
            shutil.rmtree(cache_dir)
            os.makedirs(cache_dir)