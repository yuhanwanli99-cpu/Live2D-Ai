"""插件系统 health/effectiveness 可观测测试。

契约：
- PluginHost 统计每个插件的调用次数/最近事件/错误（emit_event 与 record_error）；
- PluginManager.health() 聚合 loaded/enabled/status + 观测字段；
- live2d-action 原生插件：默认加载启用，工具注册可见；disable 后工具立即不可用；
- /api/plugins 列表带 health 字段；/api/plugins/health 返回聚合与原生工具状态；
- 启动报告 startup_report 明确回答「插件是否真的工作」。
"""

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from open_llm_vtuber.live2d_action import (
    LIVE2D_ACTION_TOOL_NAME,
    get_default_tool,
    reset_default_tool,
)
from open_llm_vtuber.plugin_routes import init_plugin_routes
from open_llm_vtuber.plugins.health import (
    native_tool_reports,
    startup_report,
)
from open_llm_vtuber.plugins.manager import PluginManager


@pytest.fixture(autouse=True)
def _fresh_tool():
    reset_default_tool()
    yield
    reset_default_tool()


@pytest.fixture
def manager():
    pm = PluginManager()
    pm.load_native()
    return pm


@pytest.fixture
def client(manager):
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    return TestClient(app)


class _OkPlugin:
    id = "ok-plugin"

    def handle_event(self, name, payload):
        return {"echo": name}


class _BoomPlugin:
    id = "boom-plugin"

    def handle_event(self, name, payload):
        raise RuntimeError("exploded")


class TestPluginHostObservability:
    def test_emit_event_counts_calls_and_last_event(self):
        from open_llm_vtuber.plugins.host import PluginHost

        host = PluginHost()
        host.register(_OkPlugin())
        host.emit_event("llm.before", {})
        host.emit_event("sentence.after", {})

        status = host.get_status()["ok-plugin"]
        assert status["loaded"] is True
        assert status["calls"] == 2
        assert status["last_event"] == "sentence.after"
        assert status["errors"] == 0

    def test_raising_plugin_records_error_not_silence(self):
        from open_llm_vtuber.plugins.host import PluginHost

        host = PluginHost()
        host.register(_BoomPlugin())
        events = host.emit_event("llm.before", {})

        assert events[0]["ok"] is False
        assert "exploded" in events[0]["error"]
        status = host.get_status()["boom-plugin"]
        assert status["errors"] == 1
        assert "exploded" in status["last_error"]
        assert status["error"]

    def test_record_error_increments_error_counter(self):
        from open_llm_vtuber.plugins.host import PluginHost

        host = PluginHost()
        host.register(_BoomPlugin())
        host.record_error("boom-plugin", "load failed")
        status = host.get_status()["boom-plugin"]
        assert status["errors"] == 1
        assert status["last_error"] == "load failed"


class TestManagerHealth:
    def test_live2d_action_plugin_loaded_and_enabled(self, manager):
        health = manager.health()
        entry = health["live2d-action"]
        assert entry["loaded"] is True
        assert entry["enabled"] is True
        assert entry["status"] == "ok"
        assert entry["implementation"] == "runtime"

    def test_disable_disables_native_tool_immediately(self, manager):
        tool = get_default_tool()
        assert tool.available() is True

        manager.disable("live2d-action")
        assert tool.available() is False
        assert manager.health()["live2d-action"]["enabled"] is False

        manager.enable("live2d-action")
        assert tool.available() is True

    def test_disable_hook_failure_does_not_block(self, manager):
        instance = manager.host.get_instance("live2d-action")

        def _boom():
            raise RuntimeError("hook failed")

        instance.on_disable = _boom
        manifest = manager.disable("live2d-action")  # 不应抛出
        assert manifest.id == "live2d-action"
        assert manager.is_enabled("live2d-action") is False


class TestNativeToolRegistration:
    def test_native_tools_report_registered(self, manager):
        reports = native_tool_reports(manager)
        assert len(reports) >= 1
        report = reports[0]
        assert report["tool"] == LIVE2D_ACTION_TOOL_NAME
        assert report["registered"] is True
        assert report["available"] is True
        assert "nod" in report["whitelist"]
        assert report["calls"] == 0

    def test_missing_plugin_reports_unregistered(self, manager):
        manager.host.unregister("live2d-action")
        reports = native_tool_reports(manager)
        assert reports[0]["registered"] is False
        assert reports[0]["available"] is False


class TestHealthAPI:
    def test_list_plugins_contains_health_fields(self, client):
        r = client.get("/api/plugins")
        assert r.status_code == 200
        plugins = {p["id"]: p for p in r.json()["plugins"]}
        entry = plugins["live2d-action"]
        assert entry["enabled"] is True
        assert entry["health"]["loaded"] is True
        assert entry["health"]["calls"] == 0

    def test_health_endpoint_aggregates(self, client, manager):
        # 制造一次调用统计
        import asyncio

        asyncio.run(get_default_tool().execute({"action": "nod"}))

        r = client.get("/api/plugins/health")
        assert r.status_code == 200
        body = r.json()
        assert body["ok"] is True
        summary = body["summary"]
        assert summary["total"] == len(manager.list_plugins())
        assert summary["native_tools_registered"] == 1
        plugin_entry = body["plugins"]["live2d-action"]
        assert plugin_entry["loaded"] is True
        tool = body["native_tools"][0]
        assert tool["accepted"] == 1
        assert tool["last_action"] == "nod"

    def test_health_reflects_disabled_plugin(self, client, manager):
        manager.disable("live2d-action")
        body = client.get("/api/plugins/health").json()
        assert body["plugins"]["live2d-action"]["enabled"] is False
        tool = body["native_tools"][0]
        assert tool["available"] is False

    def test_health_route_not_shadowed_by_dynamic_route(self, client):
        """/api/plugins/health 必须先于 {plugin_id} 注册，不能 404。"""
        r = client.get("/api/plugins/health")
        assert r.status_code == 200
        assert r.json()["ok"] is True


class TestStartupReport:
    def test_report_states_plugins_and_native_tool(self, manager):
        lines = startup_report(manager)
        joined = "\n".join(lines)
        # 总量行
        assert "[Plugins]" in joined
        assert f"共 {len(manager.list_plugins())} 个" in joined
        # 每个插件一行，含 enabled/status
        for manifest in manager.list_plugins():
            assert manifest.id in joined
        # 原生工具有效性明确声明（解决「插件是否真的工作」）
        assert LIVE2D_ACTION_TOOL_NAME in joined
        assert "已注册并可用" in joined

    def test_report_flags_missing_native_tool(self, manager):
        manager.host.unregister("live2d-action")
        joined = "\n".join(startup_report(manager))
        assert "未生效" in joined
