"""Backward-compatible route-factory exports.

Domain implementations live in open_llm_vtuber.api. New endpoints must be added to
their domain module instead of growing this assembly file.
"""

from .api.client_ws_routes import init_client_ws_route as init_client_ws_route
from .api.config_routes import init_config_routes as init_config_routes
from .api.dev_routes import init_dev_routes as init_dev_routes
from .api.proxy_routes import init_proxy_route as init_proxy_route
from .api.webtool_routes import init_webtool_routes as init_webtool_routes

__all__ = ["init_client_ws_route", "init_config_routes", "init_dev_routes", "init_proxy_route", "init_webtool_routes"]