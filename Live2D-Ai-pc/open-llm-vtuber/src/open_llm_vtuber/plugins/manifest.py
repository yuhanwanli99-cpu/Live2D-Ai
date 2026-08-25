# -*- coding: utf-8 -*-
"""Plugin manifest model for the PC backend plugin registry."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class PluginManifest:
    """Declarative description of a single plugin.

    origin must be "native" (built into the application) or "external"
    (loaded from a user-supplied entrypoint). entrypoint is an import path of
    the form "module.path:attribute" for external plugins, or None for native
    stubs.
    """

    id: str
    name: str
    version: str
    description: str
    origin: str  # "native" | "external"
    name_zh: str = ""
    description_zh: str = ""
    hook_points: list[str] = field(default_factory=list)
    capabilities: list[str] = field(default_factory=list)
    config_schema: dict[str, Any] = field(default_factory=dict)
    default_enabled: bool = True
    entrypoint: str | None = None
    implementation: str = "stub"

    def to_public(self) -> dict[str, Any]:
        """Return a JSON-safe representation of the manifest fields."""
        return {
            "id": self.id,
            "name": self.name,
            "name_zh": self.name_zh,
            "version": self.version,
            "description": self.description,
            "description_zh": self.description_zh,
            "origin": self.origin,
            "hook_points": list(self.hook_points),
            "capabilities": list(self.capabilities),
            "config_schema": dict(self.config_schema),
            "default_enabled": self.default_enabled,
            "entrypoint": self.entrypoint,
            "implementation": self.implementation,
        }
