"""ContextCompactor 纯逻辑测试。"""
import os
import sys
import unittest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.context_compactor import ContextCompactor  # noqa: E402


class TestContextCompactor(unittest.TestCase):
    def test_no_compaction_under_budget(self):
        compactor = ContextCompactor(max_tokens=100, keep_recent_tokens=50)
        plan = compactor.plan([{"content": "你好"}, {"content": "世界"}])
        self.assertFalse(plan.needs_compaction)

    def test_compaction_plans_older_chunk(self):
        compactor = ContextCompactor(max_tokens=8, keep_recent_tokens=2)
        messages = [{"content": "很长的历史内容"}] * 20
        plan = compactor.plan(messages)
        self.assertTrue(plan.needs_compaction)
        self.assertGreater(plan.keep_recent_count, 0)
        self.assertLess(plan.keep_recent_count, len(messages))

    def test_apply_summaries_replaces_old_block(self):
        compactor = ContextCompactor(max_tokens=8, keep_recent_tokens=1)
        messages = [
            {"role": "user", "content": "这是一条非常长的旧消息内容"},
            {"role": "assistant", "content": "这是另一条非常长的旧消息内容"},
            {"role": "user", "content": "新"},
        ]
        plan = compactor.plan(messages)
        self.assertTrue(plan.needs_compaction)
        result = ContextCompactor.apply_summaries(messages, plan, ["摘要"])
        self.assertEqual(result[0]["role"], "system")
        self.assertEqual(result[0]["content"], "摘要")
        self.assertEqual(result[-1]["content"], "新")
