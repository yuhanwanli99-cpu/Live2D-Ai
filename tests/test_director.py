"""ExpressionDirector 纯逻辑测试。"""
import os
import sys
import unittest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.director import rule_fallback, _extract_json  # noqa: E402


class TestDirectorFallback(unittest.TestCase):
    def test_rule_joy(self):
        result = rule_fallback("今天好开心呀")
        self.assertEqual(result.emotion, "joy")
        self.assertEqual(result.motion_label, "happy")

    def test_rule_anger(self):
        result = rule_fallback("我讨厌这样")
        self.assertEqual(result.emotion, "anger")
        self.assertIsNone(result.motion_label)

    def test_extract_json(self):
        data = _extract_json('{"emotion":"joy","strength":2}')
        self.assertEqual(data["emotion"], "joy")
