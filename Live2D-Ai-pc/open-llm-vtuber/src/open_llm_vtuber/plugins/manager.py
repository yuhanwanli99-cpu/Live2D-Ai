# -*- coding: utf-8 -*-
"""Plugin registry manager: manifest storage, enable/disable, status."""

from __future__ import annotations

import importlib
import os
from typing import Any

from .host import PluginHost
from .manifest import PluginManifest


class _NativeStub:
    """No-op instance registered for native plugins without an entrypoint."""

    def __init__(self, plugin_id: str):
        self.id = plugin_id


class _FailingPlugin:
    """Stands in for an external entrypoint that failed to load or run.

    Its handle_event always raises so the failure is surfaced through the host
    status ("error") instead of being dropped silently.
    """

    def __init__(self, plugin_id: str, message: str):
        self.id = plugin_id
        self.message = message

    def handle_event(self, name: str, payload: dict[str, Any]) -> None:
        raise RuntimeError(self.message)


class PluginManager:
    """Registry of PluginManifest objects with runtime enable/disable state."""

    def __init__(self, plugins_dir: str | None = None) -> None:
        self.plugins_dir = plugins_dir or os.path.dirname(os.path.abspath(__file__))
        self.host = PluginHost()
        self._registry: dict[str, PluginManifest] = {}
        self._enabled: dict[str, bool] = {}

    def load_native(self) -> None:
        """Register all built-in native plugin manifests."""
        from . import native

        native.register_all(self)

    def register(self, manifest: PluginManifest) -> PluginManifest:
        """Add a manifest to the registry and enable it by default when asked."""
        if manifest.origin not in ("native", "external"):
            raise ValueError(
                "plugin '{}' has invalid origin '{}' (must be 'native' or 'external')".format(
                    manifest.id, manifest.origin
                )
            )
        self._registry[manifest.id] = manifest
        self._enabled[manifest.id] = bool(manifest.default_enabled)
        if manifest.default_enabled:
            self.enable(manifest.id)
        return manifest

    def list_plugins(self) -> list[PluginManifest]:
        """Return all registered manifests in registration order."""
        return list(self._registry.values())

    def get_plugin(self, plugin_id: str) -> PluginManifest | None:
        """Return a manifest by id, or None when unknown."""
        return self._registry.get(plugin_id)

    def implementation(self, plugin_id: str) -> str:
        """Return the effective implementation classification for a plugin.

        Native manifests report the implementation declared at registration.
        For external manifests, a non-empty entrypoint is reported as
        "runtime" regardless of the declared value; otherwise the manifest
        own implementation (default "stub") is returned.
        """
        manifest = self._require(plugin_id)
        if manifest.origin == "native":
            return manifest.implementation or "stub"
        if manifest.entrypoint:
            return "runtime"
        return manifest.implementation or "stub"

    def is_enabled(self, plugin_id: str) -> bool:
        return bool(self._enabled.get(plugin_id, False))

    def enable(self, plugin_id: str) -> PluginManifest:
        """Enable a plugin and load its instance into the host."""
        manifest = self._require(plugin_id)
        self._enabled[plugin_id] = True
        instance = self._instantiate(manifest)
        self.host.register(instance)
        if isinstance(instance, _FailingPlugin):
            self.host.record_error(plugin_id, instance.message)
        return manifest

    def disable(self, plugin_id: str) -> PluginManifest:
        """Disable a plugin and remove its instance from the host."""
        manifest = self._require(plugin_id)
        self._enabled[plugin_id] = False
        self.host.unregister(plugin_id)
        return manifest

    def status(self, plugin_id: str) -> str:
        """Return a truthful runtime-status string for a plugin id.

        Values:
          - "disabled" when the plugin is not enabled;
          - "error" when the host recorded a load/runtime error;
          - "ok" only when enabled, has a real entrypoint, and is loaded;
          - otherwise the effective implementation classification
            ("runtime"/"gated"/"core"/"stub"), reflecting that an enabled
            plugin without a runnable instance is not genuinely "ok".
        """
        self._require(plugin_id)
        if not self._enabled.get(plugin_id, False):
            return "disabled"
        host_status = self.host.get_status().get(plugin_id)
        if host_status and host_status.get("error"):
            return "error"
        manifest = self._registry[plugin_id]
        if manifest.entrypoint and host_status and host_status.get("loaded"):
            return "ok"
        return self.implementation(plugin_id)

    def _require(self, plugin_id: str) -> PluginManifest:
        manifest = self._registry.get(plugin_id)
        if manifest is None:
            raise ValueError("Unknown plugin id: {}".format(plugin_id))
        return manifest

    def _instantiate(self, manifest: PluginManifest) -> Any:
        """Build the runtime instance for a manifest.

        Native stubs (no entrypoint) map to a no-op instance. External
        entrypoints are resolved via importlib; a failed import becomes a
        failing instance so enable() never crashes and the error shows up in
        status().
        """
        if not manifest.entrypoint:
            return _NativeStub(manifest.id)
        try:
            obj = self._resolve_entrypoint(manifest.entrypoint)
            return self._materialize(obj)
        except Exception as exc:  # noqa: BLE001 - record, do not crash the manager
            return _FailingPlugin(manifest.id, str(exc) or exc.__class__.__name__)

    @staticmethod
    def _materialize(obj: Any) -> Any:
        """Turn an entrypoint attribute into a runnable plugin instance.

        An entrypoint may name a class (instantiate it) or a factory callable
        (call it and use its return value). Already-built instances and
        no-op stubs pass through unchanged.
        """
        if isinstance(obj, type):
            return obj()
        # A callable with no id is treated as a factory function; anything that
        # already looks like a plugin instance (has an id) passes through so we
        # never re-call a built plugin.
        if callable(obj) and getattr(obj, "id", None) is None:
            result = obj()
            return result if result is not None else obj
        return obj

    @staticmethod
    def _resolve_entrypoint(entrypoint: str) -> Any:
        """Import 'module.path:attribute' and return the attribute."""
        module_name, _, attr_name = entrypoint.partition(":")
        if not module_name or not attr_name:
            raise ValueError(
                "entrypoint must look like 'module.path:attribute', got {!r}".format(
                    entrypoint
                )
            )
        module = importlib.import_module(module_name)
        obj: Any = module
        for part in attr_name.split("."):
            obj = getattr(obj, part)
        return obj
