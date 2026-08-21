"""原生插件运行时纯逻辑测试。"""
import os
import sys
import unittest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.plugins.native_runtime import (  # noqa: E402
    MemoryPlugin,
    ContextCompactorPlugin,
    BodyDirectorPlugin,
    MoodTtsDirectorPlugin,
)


class TestMemoryPlugin(unittest.TestCase):
    def test_extract_explicit_facts(self):
        facts = MemoryPlugin._extract_facts("我叫小明，我喜欢猫")
        self.assertIn("小明", facts)
        self.assertIn("猫", facts)


class TestContextCompactorPlugin(unittest.TestCase):
    def test_returns_plan_when_over_budget(self):
        plugin = ContextCompactorPlugin()
        history = [{"content": "很长的历史消息" * 50}] * 50
        result = plugin.handle_event("llm.before", {"history": history})
        self.assertIn("compaction_plan", result)


class TestBodyDirectorPlugin(unittest.TestCase):
    def test_sentence_after_passthrough(self):
        plugin = BodyDirectorPlugin()
        result = plugin.handle_event(
            "sentence.after", {"motion": "wave", "parameterOverrides": {"a": 1}}
        )
        self.assertEqual(result["motion"], "wave")
        self.assertEqual(result["parameterOverrides"], {"a": 1})


class TestMoodTtsDirectorPlugin(unittest.TestCase):
    def test_sentence_after_generates_instruction(self):
        plugin = MoodTtsDirectorPlugin()
        result = plugin.handle_event(
            "sentence.after",
            {"emotion": "joy", "strength": 2, "mood_tier": "happy"},
        )
        self.assertEqual(result["mood"]["tier"], "happy")
        self.assertIn("instruction", result["tts_style"])
