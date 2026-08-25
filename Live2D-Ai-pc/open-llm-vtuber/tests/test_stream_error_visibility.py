# -*- coding: utf-8 -*-
"""上游流失败必须"可见"的回归测试。

背景（本轮真机 E2E 抓到的真实缺陷）：
`agent/transformers.py` 的 `emo_interceptor` 把上游异常按 P7-05 降级为一条 warning
日志就吞掉了。于是「未配置 API Key / 401 / 网络不可达」时：
  - 后端日志有 warning，
  - **前端一个消息都收不到**，用户只看到"角色不说话"。

修复约定（本测试锁定）：
  1. pump/consume 捕获异常后仍**不崩溃**（保持降级语义、照常收尾）；
  2. 但必须向下游发一个 `{"type": "stream_error", "message": ...}` 事件；
  3. `conversations/single_conversation.py` 收到它后转成 `{"type": "error"}` 发前端
     （前端 ws-bridge → 错误气泡 + Toast）。
"""

import asyncio
import inspect

import pytest

from open_llm_vtuber.agent.transformers import emo_interceptor


def collect(stream_factory):
    """把被装饰后的异步流跑完，返回产出列表。"""

    async def run():
        items = []
        async for item in stream_factory():
            items.append(item)
        return items

    return asyncio.run(run())


class TestPumpSurfacesUpstreamFailure:
    def test_upstream_exception_emits_stream_error_event(self):
        @emo_interceptor()
        async def broken_stream():
            yield "开头一点文字"
            raise ValueError("未配置 API Key：请在 .env 设置 DEEPSEEK_API_KEY 或在 conf.yaml 填写")

        items = collect(broken_stream)
        errors = [
            i for i in items if isinstance(i, dict) and i.get("type") == "stream_error"
        ]
        assert len(errors) == 1, f"应恰好产出一个 stream_error，实际 items={items}"
        assert ".env" in errors[0]["message"]
        assert "conf.yaml" in errors[0]["message"]

    def test_does_not_crash_and_keeps_text_before_failure(self):
        @emo_interceptor()
        async def broken_stream():
            yield "失败前的文字"
            raise RuntimeError("boom")

        items = collect(broken_stream)  # 不抛错即为"降级不崩溃"
        text = "".join(i for i in items if isinstance(i, str))
        assert "失败前的文字" in text

    def test_healthy_stream_has_no_stream_error(self):
        @emo_interceptor()
        async def ok_stream():
            yield "一切正常"

        items = collect(ok_stream)
        assert not any(
            isinstance(i, dict) and i.get("type") == "stream_error" for i in items
        )
        assert "一切正常" in "".join(i for i in items if isinstance(i, str))

    def test_immediate_failure_still_reports(self):
        """第一个 token 就失败（最常见：构造好的占位 LLM 一开口就抛）。"""

        @emo_interceptor()
        async def dead_stream():
            raise ValueError("Provider「deepseek_llm」 的 API Key 仍是未解析的占位符")
            yield ""  # pragma: no cover

        items = collect(dead_stream)
        errors = [
            i for i in items if isinstance(i, dict) and i.get("type") == "stream_error"
        ]
        assert len(errors) == 1
        assert "占位符" in errors[0]["message"]


class TestConversationForwardsStreamError:
    """`single_conversation` 必须把 stream_error 转成前端可见的 error 消息。"""

    def test_source_has_stream_error_branch_sending_error_type(self):
        from open_llm_vtuber.conversations import single_conversation

        source = inspect.getsource(single_conversation.process_single_conversation)
        assert '"stream_error"' in source, "缺少 stream_error 分支"
        # 该分支必须真的把 type=error 发给前端（而不是只记日志）
        assert '"type": "error"' in source

    @pytest.mark.parametrize("payload", [{}, {"message": ""}, {"message": None}])
    def test_missing_message_falls_back_to_generic_text(self, payload):
        """message 缺失时不能发出空错误（前端会显示空气泡）。"""
        message = str(payload.get("message") or "对话失败")
        assert message.strip()
