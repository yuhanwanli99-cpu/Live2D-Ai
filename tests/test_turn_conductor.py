"""TurnConductor 骨架测试：核心脑失败与专家错误都必须可见，不允许静默。"""
import asyncio
import os
import sys
import unittest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.turn.conductor import TurnConductor, ExpertSpec  # noqa: E402


class TestTurnConductor(unittest.IsolatedAsyncioTestCase):
    async def test_expert_error_is_visible(self):
        async def core():
            return "hello"

        async def bad_expert():
            raise RuntimeError("boom")

        bundle = await TurnConductor(core).run(
            [ExpertSpec(expert_id="mood-tts-director", run=bad_expert)]
        )
        self.assertEqual(bundle.text, "hello")
        statuses = {s.expert_id: s for s in bundle.expert_status}
        self.assertEqual(statuses["mood-tts-director"].status, "error")
        self.assertIn("boom", statuses["mood-tts-director"].detail)

    async def test_expert_timeout_is_visible(self):
        async def core():
            return "hello"

        async def slow_expert():
            await asyncio.sleep(0.2)
            return {"mood": {"tier": "happy"}}

        bundle = await TurnConductor(core).run(
            [ExpertSpec(expert_id="mood-tts-director", run=slow_expert, timeout=0.02)]
        )
        statuses = {s.expert_id: s for s in bundle.expert_status}
        self.assertEqual(statuses["mood-tts-director"].status, "timeout")

    async def test_body_result_merges(self):
        async def core():
            return "hi"

        async def body_expert():
            return {"body_frames": [{"sentence_id": 1, "motion": "joy"}]}

        bundle = await TurnConductor(core).run(
            [ExpertSpec(expert_id="body-director", run=body_expert)]
        )
        self.assertEqual(bundle.body_frames, [{"sentence_id": 1, "motion": "joy"}])
