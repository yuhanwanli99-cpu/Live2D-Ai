# -*- coding: utf-8 -*-
"""Minimal plugin registry for the PC backend.

Exposes the manifest model, the runtime plugin host, and the registry manager
so callers can import them from a single place::

    from open_llm_vtuber.plugins import PluginManifest, PluginHost, PluginManager
"""

from .manifest import PluginManifest
from .host import PluginHost, LoadedPlugin
from .manager import PluginManager
from . import native

__all__ = [
    "PluginManifest",
    "LoadedPlugin",
    "PluginHost",
    "PluginManager",
    "native",
]
