# -*- coding: utf-8 -*-
"""PC 插件注册中心（plugins + plugin_routes）单元测试。"""

import os
import sys
import unittest

from fastapi import FastAPI
from fastapi.testclient import TestClient

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.plugins.manager import PluginManager  # noqa: E402
from open_llm_vtuber.plugins.manifest import PluginManifest  # noqa: E402
from open_llm_vtuber.plugin_routes import init_plugin_routes  # noqa: E402


_NATIVE_IDS = {
    "input-port",
    "live-ingress",
    "memory",
    "persona",
    "context-compactor",
    "body-director",
    "mood-tts-director",
    "asr",
    "knowledge",
    "web-search",
    "voice-assets",
    "model-assets",
    "config-pack",
}


def _make_client(manager):
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    return TestClient(app)


def _external_manifest(plugin_id, default_enabled=True, entrypoint=None):
    return PluginManifest(
        id=plugin_id,
        name="External {}".format(plugin_id),
        version="0.1.0",
        description="external test plugin",
        origin="external",
        hook_points=["llm.before"],
        capabilities=["test"],
        config_schema={},
        default_enabled=default_enabled,
        entrypoint=entrypoint,
    )


class PluginNativeTest(unittest.TestCase):
    def test_native_plugins_are_listed_with_native_origin(self):
        manager = PluginManager()
        manager.load_native()
        plugins = manager.list_plugins()
        ids = {p.id for p in plugins}
        self.assertEqual(ids, _NATIVE_IDS)
        for plugin in plugins:
            self.assertEqual(plugin.origin, "native")
            self.assertTrue(plugin.default_enabled)


class PluginExternalTest(unittest.TestCase):
    def test_external_manifest_is_listed(self):
        manager = PluginManager()
        manager.load_native()
        manager.register(_external_manifest("ext-1"))
        ids = [p.id for p in manager.list_plugins()]
        self.assertIn("ext-1", ids)
        self.assertEqual(manager.get_plugin("ext-1").origin, "external")


class PluginRouteTest(unittest.TestCase):
    def test_enable_disable_unknown_id_returns_404(self):
        manager = PluginManager()
        manager.load_native()
        client = _make_client(manager)

        for method in ("enable", "disable"):
            resp = client.post("/api/plugins/nope/{}".format(method))
            self.assertEqual(resp.status_code, 404)
            body = resp.json()
            self.assertFalse(body["ok"])
            self.assertIn("Unknown plugin id", body["error"])

    def test_disabled_plugin_has_disabled_status(self):
        manager = PluginManager()
        manager.load_native()
        # 使用与运行时实例 id 一致的 plugin_id（MemoryPlugin 实例 id 为 "memory"），
        # 使外部 entrypoint 能正常加载进 host 并覆盖 disabled/ok/disabled 生命周期。
        manager.register(
            _external_manifest(
                "memory",
                default_enabled=False,
                entrypoint="open_llm_vtuber.plugins.native_runtime:MemoryPlugin",
            )
        )
        self.assertEqual(manager.status("memory"), "disabled")

        manager.enable("memory")
        self.assertEqual(manager.status("memory"), "ok")
        # effective implementation of an enabled external with an entrypoint is "runtime"
        self.assertEqual(manager.implementation("memory"), "runtime")

        manager.disable("memory")
        self.assertEqual(manager.status("memory"), "disabled")

        client = _make_client(manager)
        resp = client.get("/api/plugins/memory")
        self.assertEqual(resp.status_code, 200)
        self.assertEqual(resp.json()["plugin"]["status"], "disabled")


class PluginErrorTest(unittest.TestCase):
    def test_raising_entrypoint_reports_error_and_does_not_crash(self):
        manager = PluginManager()
        manager.register(
            _external_manifest(
                "boom",
                default_enabled=True,
                entrypoint="open_llm_vtuber.nonexistent_module:does_not_exist",
            )
        )
        # register() enables by default; a failed entrypoint import must not raise.
        self.assertEqual(manager.status("boom"), "error")

        # emit_event also catches a raising plugin and returns an error event.
        events = manager.host.emit_event("llm.before", {"text": "hello"})
        self.assertEqual(len(events), 1)
        self.assertFalse(events[0]["ok"])
        self.assertIn("error", events[0])
        self.assertEqual(manager.status("boom"), "error")


if __name__ == "__main__":
    unittest.main()
