# -*- coding: utf-8 -*-
"""插件有效性/诚实分类测试：implementation 判定与 status 不再虚报 ok。"""

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

# 原生插件 implementation 分类（与 native.py _NATIVE_IMPLEMENTATION 一致）
_RUNTIME = {
    "memory",
    "context-compactor",
    "body-director",
    "mood-tts-director",
}
_GATED = {
    "input-port",
    "live-ingress",
    "voice-assets",
    "model-assets",
    "config-pack",
}
_CORE = {"persona", "asr"}
_STUB = {"knowledge", "web-search"}

_ALL_NATIVE_IDS = _RUNTIME | _GATED | _CORE | _STUB


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


class PluginImplementationClassificationTest(unittest.TestCase):
    """原生 13 个插件 id 全存在，且实现分类与四类完全一致。"""

    def test_all_native_ids_exist_with_exact_classification(self):
        manager = PluginManager()
        manager.load_native()
        ids = {p.id for p in manager.list_plugins()}
        self.assertEqual(ids, _ALL_NATIVE_IDS)

        impl_by_id = {p.id: manager.implementation(p.id) for p in manager.list_plugins()}

        expected = {}
        for pid in _RUNTIME:
            expected[pid] = "runtime"
        for pid in _GATED:
            expected[pid] = "gated"
        for pid in _CORE:
            expected[pid] = "core"
        for pid in _STUB:
            expected[pid] = "stub"

        self.assertEqual(impl_by_id, expected)


class RuntimePluginTest(unittest.TestCase):
    """runtime 插件 load_native 后 status=="ok"，且实例可被 host 调用。"""

    def test_runtime_plugins_are_ok_and_callable(self):
        manager = PluginManager()
        manager.load_native()
        for pid in _RUNTIME:
            self.assertEqual(manager.status(pid), "ok", pid)
            loaded = manager.host.get_status().get(pid)
            self.assertIsNotNone(loaded, pid)
            self.assertTrue(loaded.get("loaded"), pid)
            # host 中实例有可调用的 handle_event（或实例本身 callable）
            instance = manager.host._plugins[pid].instance
            handler = getattr(instance, "handle_event", None)
            self.assertTrue(
                callable(handler) or callable(instance), "{} not callable".format(pid)
            )
            # 实际调用一次不应抛错
            result = manager.host.emit_event("sentence.after", {"motion": "wave"})
            self.assertTrue(any(ev["plugin"] == pid for ev in result), pid)

    def test_runtime_plugins_have_entrypoints(self):
        manager = PluginManager()
        manager.load_native()
        for pid in _RUNTIME:
            self.assertTrue(manager.get_plugin(pid).entrypoint, pid)


class GatedCorePluginTest(unittest.TestCase):
    """gated/core 插件 status 分别为 gated/core，且无 entrypoint。"""

    def test_gated_status_is_gated(self):
        manager = PluginManager()
        manager.load_native()
        for pid in _GATED:
            self.assertEqual(manager.status(pid), "gated", pid)
            self.assertIsNone(manager.get_plugin(pid).entrypoint, pid)

    def test_core_status_is_core(self):
        manager = PluginManager()
        manager.load_native()
        for pid in _CORE:
            self.assertEqual(manager.status(pid), "core", pid)
            self.assertIsNone(manager.get_plugin(pid).entrypoint, pid)


class StubPluginTest(unittest.TestCase):
    """stub 插件（knowledge、web-search）无 entrypoint，status != "ok"。"""

    def test_stub_status_is_not_ok_and_no_entrypoint(self):
        manager = PluginManager()
        manager.load_native()
        for pid in _STUB:
            self.assertIsNone(manager.get_plugin(pid).entrypoint, pid)
            self.assertEqual(manager.status(pid), "stub", pid)
            self.assertNotEqual(manager.status(pid), "ok", pid)


class PluginApiEffectivenessTest(unittest.TestCase):
    """GET /api/plugins 每个插件都有 implementation 字段，stub 的 status != "ok"。"""

    def test_api_plugins_have_implementation_and_stub_not_ok(self):
        manager = PluginManager()
        manager.load_native()
        client = _make_client(manager)

        resp = client.get("/api/plugins")
        self.assertEqual(resp.status_code, 200)
        plugins = resp.json()["plugins"]

        self.assertEqual(len(plugins), 13)
        by_id = {p["id"]: p for p in plugins}

        for pid in _ALL_NATIVE_IDS:
            plugin = by_id[pid]
            self.assertIn("implementation", plugin, pid)
            self.assertEqual(plugin["implementation"], manager.implementation(pid), pid)

        for pid in _STUB:
            self.assertIn(pid, by_id, pid)
            self.assertNotEqual(by_id[pid]["status"], "ok", pid)
            self.assertEqual(by_id[pid]["implementation"], "stub", pid)


class FailedEntrypointTest(unittest.TestCase):
    """有 entrypoint 但加载失败的插件 status=="error"。"""

    def test_failing_entrypoint_reports_error(self):
        manager = PluginManager()
        manager.register(
            _external_manifest(
                "boom",
                default_enabled=True,
                entrypoint="open_llm_vtuber.nonexistent_module:does_not_exist",
            )
        )
        self.assertEqual(manager.status("boom"), "error")

        # 通过 API 也能看到 error 状态
        client = _make_client(manager)
        resp = client.get("/api/plugins/boom")
        self.assertEqual(resp.status_code, 200)
        plugin = resp.json()["plugin"]
        self.assertEqual(plugin["status"], "error")
        self.assertEqual(plugin["implementation"], "runtime")


class ExternalImplementationTest(unittest.TestCase):
    """external 无 entrypoint 时返回 manifest.implementation（默认 stub）。"""

    def test_external_without_entrypoint_is_stub(self):
        manager = PluginManager()
        manifest = _external_manifest("ext-off", default_enabled=False)
        manager.register(manifest)
        self.assertEqual(manager.implementation("ext-off"), "stub")
        self.assertEqual(manager.status("ext-off"), "disabled")

    def test_external_with_entrypoint_is_runtime(self):
        manager = PluginManager()
        # 使用与运行时实例 id 一致的 plugin_id（MemoryPlugin 实例 id 为 "memory"），
        # 使外部 entrypoint 能正常加载进 host 并报告 "ok"。
        manifest = _external_manifest(
            "memory",
            entrypoint="open_llm_vtuber.plugins.native_runtime:MemoryPlugin",
        )
        manager.register(manifest)
        self.assertEqual(manager.implementation("memory"), "runtime")
        self.assertEqual(manager.status("memory"), "ok")

    def test_external_explicit_implementation_returned(self):
        manager = PluginManager()
        manifest = _external_manifest("ext-custom", default_enabled=False)
        manifest.implementation = "gated"
        manager.register(manifest)
        self.assertEqual(manager.implementation("ext-custom"), "gated")


if __name__ == "__main__":
    unittest.main()
