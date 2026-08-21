"""PC 通用直播/弹幕事件通道（live_event.py）单元测试。"""

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

from open_llm_vtuber.live_event import (  # noqa: E402
    LiveEventGate,
    init_live_event_routes,
    sanitize_live_event,
)


class FakeWsHandler:
    def __init__(self):
        self.calls = []

    async def broadcast_live_event(self, event: dict) -> int:
        self.calls.append(event)
        return 2


class LiveEventPureTest(unittest.TestCase):
    def test_sanitize_keeps_allowed_fields_and_drops_unknown(self):
        event = sanitize_live_event(
            {
                "type": "danmaku",
                "text": "你好呀",
                "user": "小明",
                "motion": "happy",
                "emotion": "joy",
                "strength": 9,
                "parameterOverrides": {"ParamAngleX": 0.6},
                "danger": "DROP",
            }
        )
        self.assertEqual(event["type"], "danmaku")
        self.assertEqual(event["text"], "你好呀")
        self.assertEqual(event["strength"], 3)
        self.assertNotIn("danger", event)

    def test_sanitize_empty_payload(self):
        self.assertEqual(sanitize_live_event({}), {})

    def test_gate_loopback_authorized_by_default(self):
        gate = LiveEventGate()
        self.assertIsNone(gate.authorize("127.0.0.1", None))
        self.assertIsNone(gate.authorize("testclient", None))

    def test_gate_remote_rejected_by_default(self):
        gate = LiveEventGate()
        reason = gate.authorize("192.168.1.10", None)
        self.assertIsNotNone(reason)
        self.assertIn("本机", reason)


class LiveEventRouteTest(unittest.TestCase):
    def test_event_broadcasts_and_returns_client_count(self):
        fake = FakeWsHandler()
        gate = LiveEventGate()
        app = FastAPI()
        app.include_router(init_live_event_routes(gate=gate, ws_handler=fake))
        client = TestClient(app)
        resp = client.post(
            "/api/live/event",
            json={
                "type": "danmaku",
                "text": "你好",
                "user": "小明",
                "motion": "wave",
                "emotion": "joy",
            },
        )
        self.assertEqual(resp.status_code, 200)
        body = resp.json()
        self.assertTrue(body["ok"])
        self.assertEqual(body["clients"], 2)
        self.assertEqual(fake.calls[0]["motion"], "wave")

    def test_empty_event_rejected(self):
        fake = FakeWsHandler()
        gate = LiveEventGate()
        app = FastAPI()
        app.include_router(init_live_event_routes(gate=gate, ws_handler=fake))
        client = TestClient(app)
        resp = client.post("/api/live/event", json={})
        self.assertEqual(resp.status_code, 400)


if __name__ == "__main__":
    unittest.main()
