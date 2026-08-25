"""插件门控测试：input-port / live-ingress 未启用时对应接口必须明确拒绝。"""
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
from open_llm_vtuber.external_input import ExternalInputGate, init_external_input_routes  # noqa: E402
from open_llm_vtuber.live_event import LiveEventGate, init_live_event_routes  # noqa: E402


class FakeWS:
    def __init__(self):
        self.client_connections = {}
        self.broadcast_live_event_calls = []

    async def broadcast_live_event(self, event):
        self.broadcast_live_event_calls.append(event)
        return 1


class TestPluginGating(unittest.TestCase):
    def test_input_port_disabled_returns_403(self):
        manager = PluginManager()
        manager.load_native()
        manager.disable("input-port")
        gate = ExternalInputGate()
        gate.state.enabled = True

        app = FastAPI()
        app.include_router(init_external_input_routes(gate, FakeWS(), manager))
        client = TestClient(app)
        resp = client.post("/api/external/chat", json={"text": "hi"})
        self.assertEqual(resp.status_code, 403)
        self.assertIn("input-port", resp.json()["error"])

    def test_live_ingress_disabled_returns_403(self):
        manager = PluginManager()
        manager.load_native()
        manager.disable("live-ingress")
        gate = LiveEventGate()
        gate.state.enabled = True
        ws = FakeWS()

        app = FastAPI()
        app.include_router(init_live_event_routes(gate, ws, manager))
        client = TestClient(app)
        resp = client.post("/api/live/event", json={"type": "danmaku", "text": "hi"})
        self.assertEqual(resp.status_code, 403)
        self.assertIn("live-ingress", resp.json()["error"])
        self.assertEqual(ws.broadcast_live_event_calls, [])
