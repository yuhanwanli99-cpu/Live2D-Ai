# -*- coding: utf-8 -*-
"""Runtime plugin host: loads plugin instances and emits hook events safely."""

from __future__ import annotations

from typing import Any


class LoadedPlugin:
    """Wrap a plugin instance so the host can invoke it uniformly.

    A plugin instance may expose a handle_event(name, payload) method, or may
    itself be callable as instance(name, payload). Native stubs have neither
    and are treated as no-op plugins.
    """

    def __init__(self, plugin_id: str, instance: Any):
        self.id = plugin_id
        self.instance = instance
        self.error: str | None = None

    def handle_event(self, name: str, payload: dict[str, Any]) -> Any:
        handler = getattr(self.instance, "handle_event", None)
        if callable(handler):
            return handler(name, payload)
        if callable(self.instance):
            return self.instance(name, payload)
        return None


class PluginHost:
    """Holds loaded plugin instances and dispatches hook events to them.

    A raising plugin never propagates its exception to the caller; the error is
    recorded on the plugin and returned as an error event, so callers can always
    inspect what failed instead of receiving a silent drop.
    """

    def __init__(self) -> None:
        self._plugins: dict[str, LoadedPlugin] = {}

    def register(self, plugin: Any) -> None:
        """Register a plugin instance (anything exposing an id attribute)."""
        if isinstance(plugin, LoadedPlugin):
            loaded = plugin
        else:
            plugin_id = getattr(plugin, "id", None)
            if not plugin_id:
                raise ValueError("plugin must expose a non-empty 'id'")
            loaded = LoadedPlugin(str(plugin_id), plugin)
        self._plugins[loaded.id] = loaded

    def unregister(self, plugin_id: str) -> None:
        """Remove a loaded plugin by id. Missing ids are ignored."""
        self._plugins.pop(plugin_id, None)

    def record_error(self, plugin_id: str, message: str) -> None:
        """Attach an error to a loaded plugin (used for failed entrypoints)."""
        loaded = self._plugins.get(plugin_id)
        if loaded is not None:
            loaded.error = message

    def get_instance(self, plugin_id: str):
        """Return the raw runtime instance of a loaded plugin (or None).

        管理类路由（如 /api/memory/*）用它触达插件公开方法；只读访问，
        不绕过 handle_event 的异常边界——调用方自行 try/except。
        """
        loaded = self._plugins.get(plugin_id)
        return loaded.instance if loaded is not None else None

    def emit_event(self, name: str, payload: dict[str, Any]) -> list[dict[str, Any]]:
        """Deliver an event to every loaded plugin.

        Returns one entry per plugin: {"plugin", "ok", "result"} on success or
        {"plugin", "ok", "error"} on failure. Plugin exceptions are caught and
        recorded, never raised out of this method.
        """
        events: list[dict[str, Any]] = []
        for plugin_id, loaded in list(self._plugins.items()):
            try:
                result = loaded.handle_event(name, payload)
                events.append({"plugin": plugin_id, "ok": True, "result": result})
            except Exception as exc:  # noqa: BLE001 - boundary of the plugin system
                loaded.error = str(exc) or exc.__class__.__name__
                events.append({"plugin": plugin_id, "ok": False, "error": loaded.error})
        return events

    def get_status(self) -> dict[str, dict[str, Any]]:
        """Return per-plugin runtime status (loaded + optional error)."""
        return {
            plugin_id: {"loaded": True, "error": loaded.error}
            for plugin_id, loaded in self._plugins.items()
        }
