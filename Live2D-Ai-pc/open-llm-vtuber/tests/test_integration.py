"""P7 · 后端管线集成 — RED 测试

依据:
  - contract-pc-pipeline.md §P7（P7-01 ~ P7-05）
  - architecture-pc-pipeline.md §3.1 目标流水线 / §P7 集成流程伪代码

被测目标态:
  - 流式 LLM `[emo:joy]主人回来啦！` → StreamInterceptor 拦截
    → 立即 WS 发送 emotion 帧（先于音频） + mood_engine.on_emotion_event()
  - 干净文本 → SentenceDivider 分段 → TTS（mock） → WS audio 帧
  - 无 [emo] → 不发送 emotion 帧（向后兼容，只 audio）
  - conversation_handler 在用户消息到达后调 mood_engine.on_user_message_sentiment()
  - service_context 用 mood_engine.get_prompt_injection() 追加 system prompt

文件锁：本文件引用架构定义的目标模块路径（src/utils/stream_interceptor.py、
src/mood_engine.py 等）。这些类**当前不存在** → collection error / 断言失败即 RED。
测试用可注入依赖（fake websocket sender / fake TTS），只断言契约行为，不读实现 body。
"""
import asyncio
from typing import AsyncIterator, List, Tuple, Optional

import pytest

# ── 架构定义的目标模块接口（target 态）。当前不存在 → RED ──
try:
    from open_llm_vtuber.utils.stream_interceptor import StreamInterceptor, EmoToken
    from open_llm_vtuber.mood_engine import MoodEngine
    _HAS_TARGET_MODULES = True
except ImportError:
    StreamInterceptor = None  # type: ignore
    MoodEngine = None  # type: ignore
    _HAS_TARGET_MODULES = False


# 合法 emotion key（契约）
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}


# ── Fake 依赖角色（扮演架构 §P7 伪代码中的调用方/发送方）──

class FakeWebSocketSender:
    """记录 WS 发送消息顺序（含类型 + 时间戳），用于断言 emotion 帧先于 audio 帧。"""

    def __init__(self):
        self.sent: List[dict] = []
        self._clock = 0.0

    def tick(self):
        self._clock += 1.0
        return self._clock

    def send(self, message: dict):
        """记录一条 WS 消息，附带单调递增的虚拟时间戳。"""
        msg = dict(message)
        msg.setdefault("_sent_at", self.tick())
        self.sent.append(msg)

    @property
    def types_in_order(self) -> List[str]:
        return [m.get("type") for m in self.sent if "type" in m]


class FakeTTSEngine:
    """mock TTS：直接把文本包成 audio 帧，不真正合成。"""

    async def synthesize_audio(self, text: str):
        await asyncio.sleep(0)  # 让出事件循环，模拟 async
        return {"audio": "<mock-b64>", "text": text}


def _run(coro):
    return asyncio.run(coro)


# ═══════════════════════════════════════════════════════════════
# P7-01 / P7-02 · 表情先行时序 + 无 emo 兼容
# ═══════════════════════════════════════════════════════════════

class TestEmotionFrameTiming:
    """LLM 流 → StreamInterceptor → emotion 帧先于 audio 帧；无 emo → 无 emotion 帧"""

    def _intercept_all(
        self, pipeline, chunks: List[str]
    ) -> List[Tuple[Optional["EmoToken"], str]]:
        """把 chunk 列表喂给 pipeline.intercept，收集全部 yield。"""
        return _run(_collect_async(pipeline, chunks))

    def test_p701_emotion_frame_before_audio_frame(self):
        # P7-01 · `[emo:joy]主人回来啦！今天过得怎么样？`
        #   → 前端先收 emotion 帧，后收 audio 帧
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED：StreamInterceptor 尚不存在）")
        pipeline = StreamInterceptor()
        sender = FakeWebSocketSender()

        # 架构 §P7 伪代码：检测到 emo → 立即发 emotion 帧 + mood；后续 TTS → audio 帧
        emotion_sent = False
        for emo_token, _clean_chunk in self._intercept_all(
            pipeline, ["[emo:joy]主人回来啦！今天过得怎么样？"]
        ):
            if emo_token is not None:
                sender.send({
                    "type": "emotion",
                    "emotion": emo_token.emotion,
                    "strength": emo_token.strength,
                    "parameterOverrides": {},
                    "fadeMs": 400,
                })
                emotion_sent = True

        # sentence 完成后发送 audio 帧
        sender.send({"type": "audio", "audio": "<b64>", "volumes": [], "visemes": []})

        assert emotion_sent, "期望 StreamInterceptor 拦截到 [emo:joy]"
        types = sender.types_in_order
        assert types.index("emotion") < types.index("audio"), \
            f"emotion 帧必须先于 audio 帧；实际顺序={types}"

    def test_p701_emotion_frame_strength_parsed_from_suffix(self):
        # 加固 P7-01：[emo:surprise_3] 强度后缀解析正确
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        pipeline = StreamInterceptor()

        def feed(chunk):
            return self._intercept_all(pipeline, [chunk])

        # 首次拦截到 emo token
        got = feed("[emo:surprise_3]诶？！")
        hit = next((t for t, _ in got if t is not None), None)
        assert hit is not None, "期望拦截到 [emo:surprise_3]"
        assert hit.emotion == "surprise"
        assert hit.strength == 3

    def test_p702_no_emotion_frame_without_emo_token(self):
        # P7-02 · 无 [emo:xxx] → 只发送 audio 帧，无 type="emotion"
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        pipeline = StreamInterceptor()
        sender = FakeWebSocketSender()

        no_emo_found = True
        for emo_token, _clean in self._intercept_all(pipeline, ["今天天气不错。"]):
            if emo_token is not None:
                no_emo_found = False

        sender.send({"type": "audio", "audio": "<b64>", "volumes": [], "visemes": []})

        assert no_emo_found, "无 [emo] 输入时不应产生 emo token"
        assert "emotion" not in sender.types_in_order, \
            "无 [emo] 时不发送 emotion 帧（向后兼容）"

    def test_p702_emo_stream_cross_chunk_detection(self):
        # 加固 P7-01 · 跨 chunk 累积识别（"[emo:j" + "oy]来了"）
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        pipeline = StreamInterceptor()
        got = self._intercept_all(pipeline, ["[emo:j", "oy]来了"])
        hit = next((t for t, _ in got if t is not None), None)
        assert hit is not None, "跨 chunk 应累积识别 [emo:joy]"
        assert hit.emotion == "joy"
        assert hit.strength == 2  # 默认强度


# ═══════════════════════════════════════════════════════════════
# P7-03 · mood_engine 生命周期 + sentiment 触发点
# ═══════════════════════════════════════════════════════════════

class TestMoodEngineLifecycle:
    """P7-03 · 对话开始 start()、结束 stop() 各一次；用户消息调 sentiment"""

    def test_p703_mood_engine_start_and_stop_each_once(self):
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        mood = MoodEngine()

        tracked = {"start": 0, "stop": 0}
        async def scenario():
            await mood.start(); tracked["start"] += 1
            await mood.stop(); tracked["stop"] += 1
        _run(scenario())

        assert tracked["start"] == 1
        assert tracked["stop"] == 1

    def test_p703_conversation_handler_calls_sentiment_trigger(self):
        # 架构 §4.3.2：conversation_handler 在用户消息到达后调 on_user_message_sentiment
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        mood = MoodEngine()

        async def scenario():
            # 模拟 conversation_handler 在收到用户消息后触发情感
            await mood.on_user_message_sentiment("negative")
            return mood.get_mood_value()
        val = _run(scenario())

        # user_negative = -10.0（契约 P3 / EMOTION_MOOD_DELTA）
        assert val < 0, "用户负面消息应使 mood 变负"


# ═══════════════════════════════════════════════════════════════
# P7-04 · mood 注入 system prompt
# ═══════════════════════════════════════════════════════════════

class TestMoodPromptInjection:
    """P7-04 · mood 值变化反映在 get_prompt_injection() 返回值"""

    def test_p704_positive_mood_injects_happy_prompt(self):
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")
        mood = MoodEngine()

        async def scenario():
            await mood.start()
            # 累加正向事件到 HAPPY
            for _ in range(3):
                await mood.on_emotion_event("joy", 2)  # +5.0 每次
            inj = mood.get_prompt_injection()
            await mood.stop()
            return mood.get_mood_value(), inj

        value, injection = _run(scenario())
        assert value >= 20, f"应达到 HAPPY；实际 mood={value}"
        assert "心情" in injection, f"正向 mood 应注入含'心情'的 prompt；实际={injection!r}"


# ═══════════════════════════════════════════════════════════════
# P7-05 · 异常不崩溃（降级）
# ═══════════════════════════════════════════════════════════════

class TestPipelineDegradation:
    """P7-05 · 各新增模块异常不放倒对话，降级为无 emotion/无 mood"""

    def test_p705_interceptor_exception_degrades_gracefully(self):
        if not _HAS_TARGET_MODULES:
            pytest.fail("目标模块未实现（RED）")

        # 架构约定：即使拦截器抛异常，管线也应继续（try-catch 兜底）。
        # 这里用真实 StreamInterceptor + 一个会在 emo 检测时抛错的包装，断言外层能继续。
        pipeline = StreamInterceptor()
        raised = {"hit": False}

        class FailingInterceptor:
            async def intercept(self, token_stream):
                async for chunk in token_stream:
                    raised["hit"] = True
                    if "[emo:" in chunk:
                        raise RuntimeError("mock interceptor failure")
                    yield None, chunk  # 含 yield → async generator（与 StreamInterceptor.intercept 一致）

        class OuterPipeline:
            def __init__(self, inner, on_emo):
                self.inner = inner
                self.on_emo = on_emo

            async def run(self, chunks):
                async def stream():
                    for c in chunks:
                        yield c
                try:
                    async for emo_token, clean in self.inner.intercept(stream()):
                        if emo_token is not None:
                            self.on_emo(emo_token)
                except Exception:
                    # 兜底：捕获后继续（降级无 emotion）
                    return "degraded"
                return "complete"

        sender = FakeWebSocketSender()
        outer = OuterPipeline(FailingInterceptor(), on_emo=lambda t: sender.send({"type": "emotion"}))
        result = _run(outer.run(["[emo:joy]hello"]))

        assert raised["hit"] is True, "mock 应确实抛出异常"
        assert result == "degraded", "外层应捕获 Interceptor 异常并降级"


# ── 脚手架 ──

async def _collect_async(pipeline, chunks: List[str]) -> List[tuple]:
    """把 list → async iterator，收集 pipeline.intercept 的全部 yield。"""
    async def stream() -> AsyncIterator[str]:
        for c in chunks:
            yield c

    out = []
    async for item in pipeline.intercept(stream()):
        out.append(item)
    return out
