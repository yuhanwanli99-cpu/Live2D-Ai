"""原生工具 400 降级与错误隔离回归测试（真机 DeepSeek 崩溃根因）。

根因：OpenAI/DeepSeek function-calling 的 function.name 必须匹配
``^[a-zA-Z0-9_-]+$``；schema 曾用点号名 ``live2d.perform_action`` →
provider 直接 HTTP 400，旧代码把错误字符串当 assistant 正文朗读。

锁定契约：
1. schema 函数名 = live2d_perform_action（合法字符集）；点号展示名绝不进 schema；
2. 携带 tools 的请求被 provider 400 拒绝 → 自动无 tools 重试一次 +
   本进程内永久停用 tools（后续轮次不再提供工具）；
3. 其他 LLM API 错误以 LLMChatError 异常传播 → 全管线只产出一条
   stream_error → 前端单一可读错误；错误文本绝不进入 TTS/字幕/记忆；
4. 错误轮次绝不触发额外多段续聊；
5. 工具调用计数照常进入 health 统计。
"""

import asyncio
import json
from types import SimpleNamespace
from typing import Any

import httpx
import pytest
from openai import NOT_GIVEN, APIConnectionError, BadRequestError, InternalServerError

from open_llm_vtuber.agent.agents.basic_memory_agent import BasicMemoryAgent
from open_llm_vtuber.agent.input_types import BatchInput, TextData, TextSource
from open_llm_vtuber.agent.output_types import SentenceOutput
from open_llm_vtuber.agent.stateless_llm.openai_compatible_llm import (
    AsyncLLM as OpenAICompatibleAsyncLLM,
)
from open_llm_vtuber.agent.stateless_llm.openai_compatible_llm import LLMChatError
from open_llm_vtuber.live2d_action import (
    LIVE2D_ACTION_TOOL_API_NAME,
    OPENAI_TOOL_NAME_PATTERN,
    Live2dActionTool,
    build_live2d_action_tool_schema,
    reset_default_tool,
)
from open_llm_vtuber.live2d_model import Live2dModel

# ---------------------------------------------------------------- #
# OpenAI SDK 异常/流的最小替身（不发生任何网络请求）
# ---------------------------------------------------------------- #


def _bad_request(detail: str = "'live2d.perform_action' is not valid") -> BadRequestError:
    """构造带 400 状态码的 BadRequestError（模拟 DeepSeek 拒绝非法函数名）。"""
    request = httpx.Request("POST", "https://api.deepseek.com/v1/chat/completions")
    response = httpx.Response(
        400,
        request=request,
        json={"error": {"message": detail, "type": "invalid_request_error"}},
    )
    return BadRequestError(detail, response=response, body=None)


def _server_error() -> InternalServerError:
    request = httpx.Request("POST", "https://api.deepseek.com/v1/chat/completions")
    response = httpx.Response(500, request=request, json={"error": {"message": "boom"}})
    return InternalServerError("boom", response=response, body=None)


def _connection_error() -> APIConnectionError:
    request = httpx.Request("POST", "https://api.deepseek.com/v1/chat/completions")
    return APIConnectionError(request=request)


class _Delta:
    def __init__(self, content=None):
        self.content = content
        self.tool_calls = None
        self.reasoning_content = None


class _Choice:
    def __init__(self, delta):
        self.delta = delta


class _Chunk:
    def __init__(self, content):
        self.choices = [_Choice(_Delta(content=content))]


class _FakeStream:
    def __init__(self, chunks):
        self._chunks = [_Chunk(content=c) if isinstance(c, str) else c for c in chunks]

    def __aiter__(self):
        return self._aiter()

    async def _aiter(self):
        for chunk in self._chunks:
            yield chunk

    async def close(self):
        pass


class _FakeCompletions:
    """脚本化 create()：按序列抛异常或返回假流；记录每次 kwargs。"""

    def __init__(self, script):
        self.script = list(script)
        self.calls: list[dict[str, Any]] = []

    async def create(self, **kwargs):
        self.calls.append(kwargs)
        if not self.script:
            raise AssertionError("脚本耗尽：不应再发起 LLM 请求")
        item = self.script.pop(0)
        if isinstance(item, Exception):
            raise item
        return _FakeStream(item)


def _make_llm(script) -> tuple[OpenAICompatibleAsyncLLM, _FakeCompletions]:
    llm = OpenAICompatibleAsyncLLM(
        model="deepseek-v4-flash",
        base_url="https://api.deepseek.com/v1",
        llm_api_key="test-key",
    )
    completions = _FakeCompletions(script)
    llm.client = SimpleNamespace(chat=SimpleNamespace(completions=completions))
    return llm, completions


def _visible_text(items) -> str:
    """提取管线可见文本（SentenceOutput 的 display/tts 文本 + 裸字符串）。"""
    parts: list[str] = []
    for item in items:
        if isinstance(item, SentenceOutput):
            parts.append(item.display_text.text or "")
            parts.append(item.tts_text or "")
        elif isinstance(item, str):
            parts.append(item)
    return "".join(parts)


def _run_stream(llm, messages, tools=NOT_GIVEN) -> list[Any]:
    async def consume():
        items = []
        async for event in llm.chat_completion(messages, system="sys", tools=tools):
            items.append(event)
        return items

    return asyncio.run(consume())


# ---------------------------------------------------------------- #
# 1. 命名正则（端到端：agent 发出的 schema 必须满足 provider 硬约束）
# ---------------------------------------------------------------- #


class TestToolNameCharset:
    def test_agent_schema_function_name_matches_deepseek_pattern(self):
        schema = build_live2d_action_tool_schema()
        name = schema["function"]["name"]
        assert OPENAI_TOOL_NAME_PATTERN.fullmatch(name), name
        assert name == LIVE2D_ACTION_TOOL_API_NAME
        assert "." not in name


# ---------------------------------------------------------------- #
# 2. tools 参数被 400 拒绝 → 无 tools 自动重试一次 + 永久降级
# ---------------------------------------------------------------- #


class TestToolsRejected400Fallback:
    def test_400_with_tools_retries_once_without_tools(self):
        llm, completions = _make_llm(
            [
                _bad_request(),  # 第 1 次：带 tools → 400
                ["你好呀", "，我在呢！"],  # 第 2 次：无 tools 重试成功
            ]
        )
        tools = [build_live2d_action_tool_schema()]
        items = _run_stream(llm, [{"role": "user", "content": "在吗"}], tools=tools)

        # 正文正常产出，绝无错误字符串混入
        assert "".join(i for i in items if isinstance(i, str)) == "你好呀，我在呢！"

        # 恰好两次请求：第一次带 tools、第二次不带
        assert len(completions.calls) == 2
        first_tools = completions.calls[0]["tools"]
        assert isinstance(first_tools, list)
        assert first_tools[0]["function"]["name"] == LIVE2D_ACTION_TOOL_API_NAME
        assert completions.calls[1]["tools"] is NOT_GIVEN

        # 本进程内永久降级：后续请求不再携带 tools
        assert llm.support_tools is False

    def test_400_without_tools_is_not_masked_as_tools_issue(self):
        """未携带 tools 时的 400 是真实请求错误：必须报错而不是静默重试。"""
        llm, completions = _make_llm([_bad_request()])
        with pytest.raises(LLMChatError):
            _run_stream(llm, [{"role": "user", "content": "在吗"}])
        assert len(completions.calls) == 1, "无 tools 时不得重试"

    def test_retry_failure_surfaces_single_readable_error(self):
        """重试仍失败 → 抛 LLMChatError（单一可读错误），绝不 yield 错误正文。"""
        llm, completions = _make_llm([_bad_request(), _bad_request()])
        tools = [build_live2d_action_tool_schema()]
        with pytest.raises(LLMChatError) as exc_info:
            _run_stream(llm, [{"role": "user", "content": "在吗"}], tools=tools)
        message = str(exc_info.value)
        assert "Error calling" not in message  # 不再把旧英文错误串当正文
        assert len(completions.calls) == 2

    @pytest.mark.parametrize(
        "exc",
        [
            _server_error(),
            _connection_error(),
        ],
    )
    def test_other_api_errors_raise_llm_chat_error(self, exc):
        llm, completions = _make_llm([exc])
        with pytest.raises(LLMChatError):
            _run_stream(llm, [{"role": "user", "content": "在吗"}], tools=None)
        assert len(completions.calls) == 1

    def test_degraded_provider_skips_native_loop_next_turn(self):
        """400 降级后的下一轮：不再提供工具，直接走纯文本简单路径。"""
        llm, completions = _make_llm(
            [
                _bad_request(),
                ["第一轮正文"],
                ["第二轮正文"],
            ]
        )
        agent = BasicMemoryAgent(
            llm=llm,
            system="你是测试助手",
            live2d_model=Live2dModel("bai"),
            tts_preprocessor_config=_tts_config(),
            use_mcpp=False,
            live2d_tool=Live2dActionTool(),
        )

        def chat_once(text):
            async def consume():
                items = []
                batch = BatchInput(
                    texts=[
                        TextData(
                            source=TextSource.INPUT, content=text, from_name="tester"
                        )
                    ]
                )
                async for item in agent.chat(batch):
                    items.append(item)
                return items

            return asyncio.run(consume())

        items1 = chat_once("你好")
        assert "第一轮正文" in _visible_text(items1)
        assert "__API_NOT_SUPPORT_TOOLS__" not in _visible_text(items1)
        assert len(completions.calls) == 2  # 400 + 无 tools 重试
        # LLM 层已永久降级（agent 在下一轮解析工具时感知并置位自身标记）
        assert llm.support_tools is False
        assert agent._native_tools_degraded is False

        items2 = chat_once("再来一句")
        assert "第二轮正文" in _visible_text(items2)
        assert agent._native_tools_degraded is True
        # 第二轮只发一次请求，且不带 tools（简单路径，非工具循环）
        assert len(completions.calls) == 3
        assert completions.calls[2]["tools"] is NOT_GIVEN


# ---------------------------------------------------------------- #
# 3. LLM API 错误不进 TTS/字幕/记忆；前端恰好收到一条可读错误
# ---------------------------------------------------------------- #


def _tts_config():
    import os

    import yaml

    from open_llm_vtuber.config_manager import TTSPreprocessorConfig

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    with open(os.path.join(root, "conf.yaml"), encoding="utf-8") as f:
        data = yaml.safe_load(f)
    return TTSPreprocessorConfig(**data["character_config"]["tts_preprocessor_config"])


class TestErrorIsolationInAgentPipeline:
    def _agent_with_failing_llm(self):
        llm, completions = _make_llm([_server_error()])
        agent = BasicMemoryAgent(
            llm=llm,
            system="你是测试助手",
            live2d_model=Live2dModel("bai"),
            tts_preprocessor_config=_tts_config(),
            use_mcpp=False,
            live2d_tool=Live2dActionTool(),
        )
        return agent, completions

    def test_pipeline_emits_single_stream_error_no_tts_no_memory(self):
        agent, _completions = self._agent_with_failing_llm()

        async def consume():
            items = []
            batch = BatchInput(
                texts=[
                    TextData(source=TextSource.INPUT, content="你好", from_name="t")
                ]
            )
            async for item in agent.chat(batch):
                items.append(item)
            return items

        items = asyncio.run(consume())

        # 恰好一条 stream_error（前端单一可读错误的唯一来源）
        errors = [
            i for i in items if isinstance(i, dict) and i.get("type") == "stream_error"
        ]
        assert len(errors) == 1
        assert errors[0]["message"].strip()

        # 错误绝不变成 TTS/字幕正文：无任何句子输出、无含错误文案的可见文本
        assert not [i for i in items if isinstance(i, SentenceOutput)]
        joined = _visible_text(items)
        assert "LLM 接口调用失败" not in joined
        assert "Error calling" not in joined

        # 记忆零污染：错误文本绝不写入记忆
        memory_text = "".join(m["content"] for m in agent._memory)
        assert "LLM 接口调用失败" not in memory_text
        assert not any(m["role"] == "assistant" for m in agent._memory)


# ---------------------------------------------------------------- #
# 4. 错误轮次不触发额外多段续聊（single_conversation 级功能测试）
# ---------------------------------------------------------------- #


class _FailingAgentEngine:
    """一开口就抛 LLMChatError 的最小 agent 替身；统计被调次数。"""

    def __init__(self):
        self.chat_calls = 0

    def chat(self, batch_input):
        self.chat_calls += 1
        return self._stream()

    async def _stream(self):
        raise LLMChatError("LLM 接口调用失败（InternalServerError），请查看后端日志。")
        yield  # pragma: no cover


class TestNoExtraContinuationAfterError:
    def test_failed_turn_does_not_trigger_extra_dialogue_segments(
        self, monkeypatch
    ):
        monkeypatch.setenv("MELO_MULTI_TURN_EXTRA", "2")

        from open_llm_vtuber.conversations.single_conversation import (
            process_single_conversation,
        )

        sent: list[str] = []

        async def ws_send(msg):
            sent.append(msg)

        class _Cfg(SimpleNamespace):
            pass

        context = SimpleNamespace(
            character_config=_Cfg(
                human_name="tester", character_name="AI", conf_uid="", avatar=""
            ),
            asr_engine=None,
            vision_handler=None,
            history_uid="",
            agent_engine=_FailingAgentEngine(),
        )

        result = asyncio.run(
            process_single_conversation(
                context=context,
                websocket_send=ws_send,
                client_uid="test-uid",
                user_input="你好",
            )
        )

        # 主轮失败后绝不发起额外续聊段（默认会额外续 2 段）
        assert context.agent_engine.chat_calls == 1
        assert result == ""

        # 前端恰好收到一条可读错误（不含旧英文错误串）
        error_frames = [
            json.loads(m)
            for m in sent
            if m.startswith("{") and json.loads(m).get("type") == "error"
        ]
        assert len(error_frames) == 1
        assert "LLM 接口调用失败" in error_frames[0]["message"]
        assert all("Error calling" not in (f.get("message") or "") for f in error_frames)

    def test_stream_error_event_does_not_trigger_extra_segments(
        self, monkeypatch
    ):
        """stream_error 事件形式（transformers 降级路径）同样终止续聊。"""
        monkeypatch.setenv("MELO_MULTI_TURN_EXTRA", "2")

        from open_llm_vtuber.conversations.single_conversation import (
            process_single_conversation,
        )

        class _StreamErrorEngine:
            def __init__(self):
                self.chat_calls = 0

            def chat(self, batch_input):
                self.chat_calls += 1

                async def stream():
                    yield {"type": "stream_error", "message": "上游流硬失败"}

                return stream()

        sent: list[str] = []

        async def ws_send(msg):
            sent.append(msg)

        context = SimpleNamespace(
            character_config=SimpleNamespace(
                human_name="tester", character_name="AI", conf_uid="", avatar=""
            ),
            asr_engine=None,
            vision_handler=None,
            history_uid="",
            agent_engine=_StreamErrorEngine(),
        )

        asyncio.run(
            process_single_conversation(
                context=context,
                websocket_send=ws_send,
                client_uid="test-uid",
                user_input="你好",
            )
        )
        assert context.agent_engine.chat_calls == 1


# ---------------------------------------------------------------- #
# 5. 正常 tool call 绑定动作 + health 调用计数
# ---------------------------------------------------------------- #


class TestHealthCallCounting:
    def test_accepted_tool_call_counts_in_health_stats(self):
        """合法工具调用经默认单例累计，health 报告可见（calls/accepted）。"""
        from open_llm_vtuber.live2d_action import get_default_tool

        reset_default_tool()
        try:
            tool = get_default_tool()
            assert tool.stats()["calls"] == 0
            result = asyncio.run(tool.execute({"action": "nod", "strength": 2}))
            assert result["ok"] is True

            stats = tool.stats()
            assert stats["calls"] == 1
            assert stats["accepted"] == 1
            assert stats["rejected"] == 0
            assert stats["last_action"] == "nod"
            assert stats["api_name"] == LIVE2D_ACTION_TOOL_API_NAME
        finally:
            reset_default_tool()
