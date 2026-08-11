"""P5 · 段级 TTS 并发管线 —— RED 测试（初始失败，驱动实现）。

契约来源：`.pi/spoq/contract-pc-pipeline.md` §P5（P5-01 ~ P5-04）
架构来源：`.pi/spoq/architecture-pc-pipeline.md` §3.7 / §4.5

目标实现（改造文件 `src/open_llm_vtuber/conversations/tts_manager.py`）：

    ```python
    class TTSTaskManager:
        async def speak_async(self, tts_text, display_text, actions,
                              emotion, mood_tier, sequence_number,
                              websocket_send) -> None:
            '''异步启动 TTS 合成（不阻塞后续句子），立即返回。
            - 后台 asyncio.create_task 合成
            - 合成完成后入队发送 audio 帧'''
        async def flush(self) -> None:
            '''等待所有排队的 TTS task 完成。'''
    ```

契约验收断言：
  P5-01  连续 2 次 speak_async，第二次不等待第一次完成（返回 wall-time < 10ms）
  P5-02  并行合成 2 句 → 2 个音频均生成且内容不同（sha256 不同）
  P5-03  flush() 后所有排队的 audio 帧已通过 WS 发送（mock websocket 调用数 = 合成数）
  P5-04  并发上限 2：第 3 句排队等待，不创建第 3 个并行 task（同时 active ≤ 2）

任务扩展断言（本测试文件）：
  · instruction 透传：每段按各自 (emotion, mood_tier) 透传对应 instruction 到 TTS 引擎
  · 失败段重试/跳过：合成抛异常 → 该段不阻塞整体，flush 仍返回（跳过）或按实现重试
  · 整体超时控制：flush 可带超时，超时后返回（不永久挂起）

⚠️ 文件锁遵守：本测试只读契约/架构，未读 `src/` 实现代码。mocker 只 patch
`get_instruction` 到模块命名空间，不触碰实现体。

测试运行说明：
  venv 未安装 pytest-asyncio，故 async 逻辑统一在同步 pytest 用例内用
  `asyncio.run(...)` 驱动，保证机械可执行（无需插件）。
"""

import asyncio
import hashlib
import time
from unittest import mock

import pytest

from open_llm_vtuber.conversations import tts_manager as tts_manager_module

# 契约枚举基准（与 P1 VALID_EMOTIONS 一致）
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}
VALID_MOOD_TIERS = {"furious", "upset", "neutral", "happy", "ecstatic"}


# ---------------------------------------------------------------------------
# 测试支柱：mock TTS 合成器（不真实调用 CosyVoice）
# ---------------------------------------------------------------------------

class FakeTTSEngine:
    """可观测并发/instruction/失败的 mock 合成器。

    模拟 `TTSInterface.async_generate_audio(text, file_name_no_ext=None, instruction="")`
    契约签名（architecture §4.4.3）。每次合成：
      - 记录 (instruction, text, sequence) 调用
      - 生成确定性的纯文本"音频内容"（hash 可测不同 -> 对应不同音频文件）
      - 用可注入延迟/失败来控制并发观察点
    """

    def __init__(self, delay: float = 0.0, fail_on_text: str = "", fail_count: int = 1):
        """
        Args:
            delay: 每次合成 sleep 秒（放大并发窗口用于并发观测）。
            fail_on_text: 合成该文本匹配的段时注入异常（按文本匹配，确定性、与并发
                顺序无关）；空串 = 从不失败。
            fail_count: 该文本被注入失败的次数；≤0 = 每次该文本都失败（模拟持续失败→跳过）；
                ≥1 时第 (fail_count+1) 次起该文本改为成功（模拟重试成功）。
        """
        self.delay = delay
        self.fail_on_text = fail_on_text
        self.fail_count = fail_count
        self.calls: list[dict] = []        # 成功合成的记录 [{text, instruction,...}]
        self.failures: list[dict] = []     # 失败合成记录（用于失败计数断言）
        self._active = 0                   # 当前 in-flight 数（并发观察点）
        self.max_active = 0                # 观测到的峰值并发

    async def async_generate_audio(self, text, file_name_no_ext=None, instruction=""):
        started = time.monotonic()
        self._active += 1
        self.max_active = max(self.max_active, self._active)
        if self.delay:
            await asyncio.sleep(self.delay)

        # 失败注入：命中目标文本且未耗尽失败次数 → 抛异常（计入 failures，不计入成功）
        failed_times = len([f for f in self.failures if f["text"] == text])
        if text == self.fail_on_text and (self.fail_count <= 0 or failed_times < self.fail_count):
            self.failures.append({"text": text, "instruction": instruction})
            self._active -= 1
            raise RuntimeError(f"injected failure for text {text!r}")

        self.calls.append({
            "text": text,
            "instruction": instruction,
            "in_flight_minus_self": self._active - 1,
            "started_ms": started,
        })
        content = f"audio[{text}|{instruction}]"
        self._active -= 1
        return content

    @property
    def instructions(self) -> list[str]:
        return [c["instruction"] for c in self.calls]


def _make_websocket_log():
    """返回 (send_async, messages[]) —— 记录所有 WS 发送的 mock。"""
    messages: list = []

    async def send(payload):
        messages.append(payload)

    return send, messages


def _make_display_text(text: str):
    """构造最小 DisplayText 占位对象（契约类型字段：text/name/avatar）。"""
    return {"text": text, "name": "凛", "avatar": None}


def _make_actions(sequencers=None):
    """构造最小 Actions 占位对象（契约字段：expressions/parameterOverrides 等）。"""
    return {
        "expressions": [],
        "pictures": None,
        "sounds": None,
        "motion": None,
        "parameterOverrides": sequencers or {},
    }


# ---------------------------------------------------------------------------
# fixture：可注入的 TTSTaskManager 构造（RED 目标：speak_async/flush 待实现）
# ---------------------------------------------------------------------------

@pytest.fixture
def manager_factory():
    """返回一个工厂：构造 TTSTaskManager 并注入 mock 引擎与 get_instruction。

    由于契约/架构未（也不能在本测试内）读取实现细节，这里采用"模块命名空间 patch"
    方式注入指令解析：测试会把 `tts_manager_module.get_instruction` 替换为可控 stub，
    以便断言每个段是否按各自 emotion 透传正确 instruction。
    引擎通过构造参数注入（契约 P5 语义：管理器持有一个引擎供 `_generate_audio` 用）。

    返回的工厂签名：
        factory(engine, instruction_map) -> (manager, ws_send, messages)
    """
    def build(engine, instruction_map=None, concurrency_limit=2):
        # 默认 instruction_map：emotion -> "<emotion>:<mood_tier>"，用于断言透传
        if instruction_map is None:
            instruction_map = {
                em: f"{em}-ins" for em in VALID_EMOTIONS
            }

        # patch 到模块命名空间，使管理器内部调用的 get_instruction 解析到 stub
        def fake_get_instruction(emotion, strength=2, mood_tier="neutral"):
            # dict.get 的 default 是急切求值：instruction_map 可能无 "neutral" 键 → 用安全兜底
            return instruction_map.get(emotion, instruction_map.get("neutral", f"{emotion}-ins"))

        ws_send, messages = _make_websocket_log()
        manager = tts_manager_module.TTSTaskManager(
            tts_engine=engine,
            concurrency_limit=concurrency_limit,
        )

        # 使用 mock.patch 上下文管理器持久作用于 manager 所在的模块，直到测试结束
        patcher = mock.patch.object(
            tts_manager_module, "get_instruction", fake_get_instruction
        )
        patcher.start()
        # 注册 tear-down，测试结束时恢复
        import atexit
        atexit.register(patcher.stop)
        return manager, ws_send, messages, patcher

    return build


# ---------------------------------------------------------------------------
# P5-01：第二次 speak_async 立即返回（不等待第一次）
# ---------------------------------------------------------------------------

class TestSpeakAsyncReturnsImmediately:
    def test_second_call_returns_without_waiting_first(self, manager_factory):
        # P5-01：第一个合成很慢（delay 0.1s），第二个 speak_async 应 <10ms 返回
        engine = FakeTTSEngine(delay=0.1)

        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.speak_async(
                tts_text="句一", display_text=_make_display_text("句一"),
                actions=_make_actions(), emotion="joy", mood_tier="happy",
                sequence_number=1, websocket_send=ws_send,
            )
            t0 = time.monotonic()
            await manager.speak_async(
                tts_text="句二", display_text=_make_display_text("句二"),
                actions=_make_actions(), emotion="sadness", mood_tier="neutral",
                sequence_number=2, websocket_send=ws_send,
            )
            elapsed_ms = (time.monotonic() - t0) * 1000
            await manager.flush()
            return elapsed_ms

        elapsed_ms = asyncio.run(scenario())
        # 第二次 call 不等待第一个慢合成完成 → 墙钟 <10ms
        assert elapsed_ms < 10, f"第二次 speak_async 阻塞了 {elapsed_ms:.1f}ms，未并行"
        patcher.stop()


# ---------------------------------------------------------------------------
# P5-02：并行合成 2 句 → 2 个音频内容不同 + 并发发生
# ---------------------------------------------------------------------------

class TestParallelSynthesis:
    def test_two_segments_concurrently_produce_distinct_content(self, manager_factory):
        # P5-02：2 句并行，2 个不同音频内容（hash 不同），且确实发生并发重叠
        engine = FakeTTSEngine(delay=0.05)

        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.speak_async(
                tts_text="甲", display_text=_make_display_text("甲"),
                actions=_make_actions(), emotion="joy", mood_tier="happy",
                sequence_number=1, websocket_send=ws_send,
            )
            await manager.speak_async(
                tts_text="乙", display_text=_make_display_text("乙"),
                actions=_make_actions(), emotion="surprise", mood_tier="ecstatic",
                sequence_number=2, websocket_send=ws_send,
            )
            await manager.flush()

        asyncio.run(scenario())

        # 两个段都被合成了
        assert len(engine.calls) == 2, f"预期合成 2 段，实际 {len(engine.calls)}"
        contents = [
            f"audio[{c['text']}|{c['instruction']}]" for c in engine.calls
        ]
        digests = {hashlib.sha256(ct.encode()).hexdigest() for ct in contents}
        # 内容不同 → sha256 不同
        assert len(digests) == 2, "两段音频内容应不同（sha256 不同）"
        # 真实并发：至少有一次调用时另一个在 in-flight（重叠）
        assert any(c["in_flight_minus_self"] >= 1 for c in engine.calls), (
            "两段未并发：未观察到任一合成时另一段在飞"
        )
        patcher.stop()


# ---------------------------------------------------------------------------
# P5-03：flush() 后所有排队 audio 帧已通过 WS 发送
# ---------------------------------------------------------------------------

class TestFlushSendsAllAudio:
    def test_flush_waits_and_sends_all_audio(self, manager_factory):
        # P5-03：3 段合成后 flush，WS 发送数 = 合成数
        engine = FakeTTSEngine(delay=0.01)

        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            for i in range(1, 4):
                await manager.speak_async(
                    tts_text=f"段{i}", display_text=_make_display_text(f"段{i}"),
                    actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                    sequence_number=i, websocket_send=ws_send,
                )
            await manager.flush()
            # flush 返回后：3 段全部合成
            assert len(engine.calls) == 3

        asyncio.run(scenario())

        # flush 之后 WS 发送数 = 合成数（每段一条 audio 帧）
        assert len(messages) == len(engine.calls) == 3, (
            f"flush 后 WS 发送 {len(messages)} 帧，合成 {len(engine.calls)} 段，应相等"
        )
        patcher.stop()

    def test_flush_no_segments_is_noop(self, manager_factory):
        # 无段时 flush 不抛异常、也不发任何帧
        engine = FakeTTSEngine()
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.flush()
            return len(messages)

        sent = asyncio.run(scenario())
        assert sent == 0
        patcher.stop()


# ---------------------------------------------------------------------------
# P5-04：并发上限 2（第 3 句排队，不创建第 3 个并行 task）
# ---------------------------------------------------------------------------

class TestConcurrencyCap:
    def test_max_two_concurrent_synthesis(self, manager_factory):
        # P5-04：并发上限 2 → 同时 active 合成 ≤ 2，即使压入 4 段
        engine = FakeTTSEngine(delay=0.03)

        manager, ws_send, messages, patcher = manager_factory(
            engine, concurrency_limit=2
        )

        async def scenario():
            for i in range(1, 5):
                await manager.speak_async(
                    tts_text=f"段{i}", display_text=_make_display_text(f"段{i}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=i, websocket_send=ws_send,
                )
            await manager.flush()

        asyncio.run(scenario())

        # 4 段全部合成了
        assert len(engine.calls) == 4
        # 且任何时刻并发合成峰值 ≤ 2（第 3/4 段排队等待）
        assert engine.max_active <= 2, (
            f"并发上限 2 被突破：观测到 {engine.max_active} 个并行合成"
        )
        patcher.stop()


# ---------------------------------------------------------------------------
# 任务扩展：instruction 透传（每段带各自 instruction）
# ---------------------------------------------------------------------------

class TestInstructionPassThrough:
    def test_each_segment_forwards_its_own_instruction(self, manager_factory):
        # 每段 (emotion, mood_tier) 不同 → 引擎收到各自对应的 instruction
        # instruction stub：emotion -> "<emotion>-ins"，可据此唯一识别
        instruction_map = {
            "joy": "joy-开心活泼指令",
            "sadness": "sadness-委屈失落指令",
            "surprise": "surprise-惊讶慌张指令",
        }
        engine = FakeTTSEngine(delay=0.01)
        manager, ws_send, messages, patcher = manager_factory(engine, instruction_map)

        segment_plan = [
            (1, "joy", "happy"),
            (2, "sadness", "neutral"),
            (3, "surprise", "ecstatic"),
        ]

        async def scenario():
            for seq, emotion, mood in segment_plan:
                await manager.speak_async(
                    tts_text=f"段{seq}", display_text=_make_display_text(f"段{seq}"),
                    actions=_make_actions(), emotion=emotion, mood_tier=mood,
                    sequence_number=seq, websocket_send=ws_send,
                )
            await manager.flush()

        asyncio.run(scenario())

        # 每个段的 emotion 都映射到了它自己的 instruction
        expected = {em: instruction_map[em] for _, em, _ in segment_plan}
        actual_instructions = engine.instructions
        for seq, emotion, _ in segment_plan:
            assert actual_instructions[seq - 1] == expected[emotion], (
                f"段{seq} (emotion={emotion!r}) 应透传 instruction="
                f"{expected[emotion]!r}，实际 {actual_instructions[seq-1]!r}"
            )
        patcher.stop()

    def test_segments_with_same_emotion_share_instruction(self, manager_factory):
        # 同名 emotion 段透传同一 instruction（去重语义）
        instruction_map = {"joy": "joy-standard"}
        engine = FakeTTSEngine()
        manager, ws_send, messages, patcher = manager_factory(engine, instruction_map)

        async def scenario():
            for seq in (1, 2):
                await manager.speak_async(
                    tts_text=f"段{seq}", display_text=_make_display_text(f"段{seq}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=seq, websocket_send=ws_send,
                )
            await manager.flush()

        asyncio.run(scenario())
        assert engine.instructions == ["joy-standard", "joy-standard"]
        patcher.stop()


# ---------------------------------------------------------------------------
# 任务扩展：失败段重试/跳过（不阻塞整体）
# ---------------------------------------------------------------------------

class TestFailureHandling:
    def test_failed_segment_skipped_others_still_deliver(self, manager_factory):
        # 段2 持续合成失败（fail_count=0 = 每次该段都失败）→ 段2 被跳过不入成功集合；
        # 段1/段3 不受影响；flush 不因单段失败抛异常（R7 脱敏跳过，不阻塞整体管线）。
        engine = FakeTTSEngine(delay=0.01, fail_on_text="段2", fail_count=0)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            for seq in (1, 2, 3):
                await manager.speak_async(
                    tts_text=f"段{seq}", display_text=_make_display_text(f"段{seq}"),
                    actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                    sequence_number=seq, websocket_send=ws_send,
                )
            try:
                await manager.flush()
                flush_raised = False
            except Exception:
                flush_raised = True
            return flush_raised

        flush_raised = asyncio.run(scenario())
        assert flush_raised is False, "失败段不应让 flush 抛异常（应跳过/隔离）"

        delivered_texts = {c["text"] for c in engine.calls}
        assert "段1" in delivered_texts and "段3" in delivered_texts, (
            f"好段 1/3 应正常送达，实际成功段 {delivered_texts}"
        )
        assert "段2" not in delivered_texts, "持续失败的段2 不应进入成功集合（被跳过）"
        # 失败确实被尝试过（引擎收到段2 的合成请求并记录失败）
        assert any(f["text"] == "段2" for f in engine.failures), (
            "引擎应收到段2 的合成请求并记录其失败"
        )
        patcher.stop()

    def test_failed_segment_retried_then_recovers(self, manager_factory):
        # 段1 第一次合成失败（fail_count=1），实现应重试并最终成功；
        # 引擎记录 1 次失败 + 1 次成功，段1 进入成功集合（P5 失败重试语义）。
        engine = FakeTTSEngine(delay=0.01, fail_on_text="段1", fail_count=1)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            for seq in (1, 2):
                await manager.speak_async(
                    tts_text=f"段{seq}", display_text=_make_display_text(f"段{seq}"),
                    actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                    sequence_number=seq, websocket_send=ws_send,
                )
            try:
                await manager.flush()
                flush_raised = False
            except Exception:
                flush_raised = True
            return flush_raised

        flush_raised = asyncio.run(scenario())
        assert flush_raised is False

        delivered_texts = {c["text"] for c in engine.calls}
        assert "段1" in delivered_texts, "重试后段1 应最终成功"
        assert "段2" in delivered_texts, "段2 正常送达"
        # 段1 恰好失败 1 次（其余尝试成功）
        fail1 = [f for f in engine.failures if f["text"] == "段1"]
        assert len(fail1) == 1, f"段1 应仅失败 1 次即重试成功，实际失败 {len(fail1)} 次"
        patcher.stop()


# ---------------------------------------------------------------------------
# 任务扩展：整体超时控制（flush 可超时返回，不永久挂起）
# ---------------------------------------------------------------------------

class TestOverallTimeout:
    def test_flush_with_timeout_returns_after_timeout(self, manager_factory):
        # flush(timeout) 在超时后返回，不无限等待慢合成
        engine = FakeTTSEngine(delay=10.0)   # 每个合成极慢（10s）
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.speak_async(
                tts_text="慢段", display_text=_make_display_text("慢段"),
                actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                sequence_number=1, websocket_send=ws_send,
            )
            t0 = time.monotonic()
            await manager.flush(timeout=0.05)   # 0.05s 超时
            return (time.monotonic() - t0) * 1000

        elapsed_ms = asyncio.run(scenario())
        # 超时后应已返回，且远小于慢合成的 10s
        assert elapsed_ms < 1000, f"flush 超时未生效，阻塞了 {elapsed_ms:.0f}ms"
        patcher.stop()


# ---------------------------------------------------------------------------
# spoq-fix P2-3（M-A4/M-A5）：concurrency_limit 配置解析 + 串行行为
# ---------------------------------------------------------------------------

class TestConcurrencyLimitConfig:
    """M-A4：conf.yaml tts_config.concurrency_limit 默认 2 / 可覆盖 / ge=1 非法拒绝"""

    def test_default_is_two(self):
        from open_llm_vtuber.config_manager.tts import TTSConfig
        cfg = TTSConfig(tts_model="edge_tts")
        assert cfg.concurrency_limit == 2

    def test_custom_value_accepted(self):
        from open_llm_vtuber.config_manager.tts import TTSConfig
        cfg = TTSConfig(tts_model="edge_tts", concurrency_limit=5)
        assert cfg.concurrency_limit == 5

    def test_ge_one_rejects_zero(self):
        import pydantic
        from open_llm_vtuber.config_manager.tts import TTSConfig
        with pytest.raises(pydantic.ValidationError):
            TTSConfig(tts_model="edge_tts", concurrency_limit=0)


class TestConcurrencyLimitOneSerial:
    """M-A5：concurrency_limit=1 → 两段并发合成串行（峰值 ≤ 1）"""

    def test_limit_one_serializes_synthesis(self, manager_factory):
        engine = FakeTTSEngine(delay=0.03)
        manager, ws_send, messages, patcher = manager_factory(
            engine, concurrency_limit=1
        )

        async def scenario():
            for i in range(1, 3):
                await manager.speak_async(
                    tts_text=f"段{i}", display_text=_make_display_text(f"段{i}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=i, websocket_send=ws_send,
                )
            await manager.flush()

        asyncio.run(scenario())

        assert len(engine.calls) == 2
        assert engine.max_active <= 1, (
            f"concurrency_limit=1 被突破：观测到 {engine.max_active} 个并行合成"
        )
        patcher.stop()
