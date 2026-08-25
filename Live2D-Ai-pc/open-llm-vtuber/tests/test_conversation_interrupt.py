"""Conversation-level interrupt path focused tests.

Verify that handle_individual_interrupt / handle_group_interrupt actually
await tts_manager.interrupt() so in-flight TTS tasks and queued payloads are
cleared before the next turn starts.
"""

import asyncio
import sys
from unittest import mock

import pytest

sys.path.insert(
    0,
    str(__import__("pathlib").Path(__file__).resolve().parents[1] / "src"),
)

from open_llm_vtuber.conversations import conversation_handler as ch  # noqa: E402


class FakeTTSTaskManager:
    """Lightweight spy for tts_manager.interrupt()."""

    def __init__(self):
        self.interrupt_called = 0
        self.flush_called = 0

    async def interrupt(self):
        self.interrupt_called += 1

    async def flush(self):
        self.flush_called += 1


class FakeServiceContext:
    def __init__(self, tts_manager=None, send_text=None):
        self.tts_manager = tts_manager
        self.send_text = send_text


async def _noop_send(*args, **kwargs):
    pass


class TestIndividualInterrupt:
    def test_interrupt_awaits_tts_manager_interrupt(self):
        tts = FakeTTSTaskManager()
        context = FakeServiceContext(tts_manager=tts, send_text=_noop_send)
        tasks = {}

        async def fake_conversation():
            await asyncio.sleep(0.05)
            return "done"

        async def run():
            tasks["u1"] = asyncio.create_task(fake_conversation())
            await ch.handle_individual_interrupt(
                client_uid="u1",
                current_conversation_tasks=tasks,
                context=context,
                heard_response="",
            )
            return tts.interrupt_called

        called = asyncio.run(run())
        assert called == 1, "handle_individual_interrupt must await tts_manager.interrupt() once"

    def test_interrupt_without_tts_manager_is_safe(self):
        context = FakeServiceContext(tts_manager=None, send_text=_noop_send)
        tasks = {}

        async def fake_conversation():
            await asyncio.sleep(0.01)

        async def run():
            tasks["u2"] = asyncio.create_task(fake_conversation())
            await ch.handle_individual_interrupt(
                client_uid="u2",
                current_conversation_tasks=tasks,
                context=context,
                heard_response="",
            )

        asyncio.run(run())  # no exception == pass

    def test_interrupt_sends_interrupted_marker(self):
        tts = FakeTTSTaskManager()
        sent = []

        async def capture_send(payload):
            sent.append(payload)

        context = FakeServiceContext(tts_manager=tts, send_text=capture_send)
        tasks = {}

        async def fake_conversation():
            await asyncio.sleep(0.01)

        async def run():
            tasks["u3"] = asyncio.create_task(fake_conversation())
            await ch.handle_individual_interrupt(
                client_uid="u3",
                current_conversation_tasks=tasks,
                context=context,
                heard_response="",
            )

        asyncio.run(run())
        assert any('"text": "\\n\\n[interrupted]"' in p for p in sent)


class TestGroupInterrupt:
    def test_group_interrupt_awaits_tts_manager_interrupt(self):
        tts = FakeTTSTaskManager()
        context = FakeServiceContext(tts_manager=tts, send_text=_noop_send)
        client_contexts = {"g1": context}
        tasks = {}

        async def fake_conversation():
            await asyncio.sleep(0.05)

        class FakeGroupManager:
            groups = {}

        async def run():
            tasks["g1"] = asyncio.create_task(fake_conversation())
            await ch.handle_group_interrupt(
                group_id="g1",
                heard_response="",
                current_conversation_tasks=tasks,
                chat_group_manager=FakeGroupManager(),
                client_contexts=client_contexts,
                broadcast_to_group=None,
            )
            return tts.interrupt_called

        called = asyncio.run(run())
        assert called == 1, "handle_group_interrupt must await tts_manager.interrupt() once"


class TestCleanupConversation:
    def test_normal_cleanup_flushes_tts_manager(self):
        from open_llm_vtuber.conversations.conversation_utils import (
            cleanup_conversation,
        )

        tts = FakeTTSTaskManager()

        async def run():
            await cleanup_conversation(tts, "🐱")
            return tts.flush_called

        called = asyncio.run(run())
        assert called == 1, "normal cleanup must await tts_manager.flush(), not interrupt"
