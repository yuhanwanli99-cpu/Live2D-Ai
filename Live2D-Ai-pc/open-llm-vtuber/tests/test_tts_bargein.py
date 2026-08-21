"""T7 · PC 端 TTS barge-in（打断）—— RED 测试（初始失败，驱动实现）。

契约/证据来源（非实现源码）：
  - `.pi/spoq/search-report-tts-selection.md`：
      · F-1.3  N.E.K.O. __interrupt__ 哨兵 + _clear_tts_pipeline()（排空响应音频→发中断→重置状态→排空迟到音频）
      · F-3.1  barge-in 标准五步（停本地播放队列→取消服务端流→写对话状态→丢未播→开新一轮）
      · F-3.2  打断 = 停止当前播放 + 清空待播队列 + 丢弃旧句 + 播新句
      · F-3.6  前端播放队列必须 drain（清空缓冲），不能只"暂停"
      · F-4.4  本项目现状：`clear()` 只清未完成 task、不停正在播放音频；新回合到达时旧回合已入队多句仍继续播
  - `.pi/spoq/tests/Live2D-Ai-pc/open-llm-vtuber/tests/test_tts_parallel.py`（P5 测试风格/mock 方式范式）

目标实现（改造文件 `src/open_llm_vtuber/conversations/tts_manager.py`，
测试只描述期望接口，不读实现细节）：

    class TTSTaskManager:
        async def speak_async(self, tts_text, display_text, actions,
                              emotion, mood_tier, sequence_number,
                              websocket_send) -> None:   # 已有（P5 RED 目标）
            ...

        # [UNSURE] 打断接口：主代理仲裁最终命名。
        # 依据 F-3.1/F-3.2/F-1.3，语义必须是——
        #   "停止当前正在播放的音频 + 清空/丢弃待播队列里的旧句 + 其后不发任何旧句 audio 帧"。
        # 本测试以 `interrupt()`（无参、async、同步完成打断语义）为期望形态；
        # 若主代理决定落在现有 `clear()` 增强上，仅需把测试中的调用点换名，语义断言不变。
        async def interrupt(self) -> None:
            '''停止正在播放的音频、清空待播队列、丢弃旧句，保证打断后不再发旧句 audio 帧。
            新回合的句子经 speak_async 正常入队仍可播放（打断≠永久静音）。

            契约（本文件断言，均对应 F-3.1/F-3.2/F-3.6）：
              B1 打断语义：调用后正在播放的音频停止、待播队列清空、此后不再向 websocket
                           发送任何旧句 audio 帧。
              B2 新回合优先：打断之后再 speak_async 的新句帧照常发出；旧句帧永不发。
              B3 幂等：无播放/无队列时调用打断 → 不抛异常、安全、不影响后续播放。
              B4 状态：打断后内部状态归零（待播 task 清空、无残留待发 audio 帧）。
            '''

    # [UNSURE] B4 依赖的内部状态属名：由 F-4.4 术语（"task_list 累积/入队"）推得 `manager.task_list`。
    #          未读实现、无法确认真实属名。主代理需仲裁：打断后暴露/读哪个状态字段来断言"待播清空"。

测试运行说明（同 P5 范式）：
  venv 未装 pytest-asyncio，async 逻辑统一在同步 pytest 用例内用 `asyncio.run(...)` 驱动。
  websocket 输出与引擎合成调用全部 mock（不真实调 CosyVoice / 不连 websocket）。

⚠️ 文件锁遵守：本测试只读检索报告与既有测试文件，未读 `src/` 任何实现代码。
   mocker 只 patch `get_instruction` 到模块命名空间、引擎以构造参数注入，不触碰实现体。
"""

import asyncio
import os
import sys
import time
from unittest import mock

import pytest

# ---------------------------------------------------------------------------
# sys.path：把真实 `src/` 注入搜索路径，使 `from open_llm_vtuber...` 解析到待测实现。
# 镜像测试位于 .pi/spoq/tests/Live2D-Ai-pc/open-llm-vtuber/tests/，
# 相对真实 src 的绝对路径锚定到仓库根（向上找 `.pi` 目录标记）。
# 与真实 tests/conftest.py 的 `sys.path.insert(0, 'src')` 同意图（仅测试环境搭建，不触碰实现）。
# ---------------------------------------------------------------------------
# 本测试在两种位置都可能被检出：
#   - .pi/spoq/tests/Live2D-Ai-pc/open-llm-vtuber/tests/（历史镜像）
#   - Live2D-Ai-pc/open-llm-vtuber/tests/（仓库主测试目录）
# 两种布局的共同点是：src/ 位于本文件父目录的父目录下（<project>/src）。
_SRC_DIR = os.path.normpath(
    os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        "src",
    )
)
if _SRC_DIR not in sys.path:
    sys.path.insert(0, _SRC_DIR)

from open_llm_vtuber.conversations import tts_manager as tts_manager_module  # noqa: E402

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
    """可观测并发/instruction/失败/丢弃的 mock 合成器。

    契约签名：`TTSInterface.async_generate_audio(text, file_name_no_ext=None, instruction="")`。
    `drop_calls_before` 时间戳之内的合成调用不计入 delivered（用于观测"打断丢弃旧句"：
    打断后仍在飞的旧句合成不应被兑现为已发帧——见 B1/B2）。本测试主要用 calls/failures 断言
    哪些段真的被合成收尾，并配合 websocket 消息日志断言"没有旧句帧被发出"。
    """

    def __init__(self, delay: float = 0.0):
        self.delay = delay
        self.calls: list[dict] = []        # 成功合成收尾的记录 [{text, instruction, sequence}]
        self.failures: list[dict] = []     # 失败合成记录
        self._started: list[dict] = []     # 合成启动记录（含启动墙钟，用于丢弃观测）
        self._active = 0
        self.max_active = 0

    async def async_generate_audio(self, text, file_name_no_ext=None, instruction=""):
        seq = None
        # 从 instruction 附带可识别序列号（便于断言旧/新句）——instruction 由 get_instruction stub 提供
        if instruction and "|seq=" in instruction:
            seq = instruction.split("|seq=")[1].split("|")[0]
        started = time.monotonic()
        self._started.append({"text": text, "instruction": instruction, "seq": seq})
        self._active += 1
        self.max_active = max(self.max_active, self._active)
        if self.delay:
            await asyncio.sleep(self.delay)
        self.calls.append({
            "text": text, "instruction": instruction, "seq": seq,
            "started_at": started,
        })
        self._active -= 1
        return f"audio[{text}|{instruction}]"


def _make_websocket_log():
    """返回 (send_async, messages[]) —— 记录所有 WS 发送的 mock。"""
    messages: list = []

    async def send(payload):
        messages.append(payload)

    return send, messages


def _make_display_text(text: str):
    """构造最小 DisplayText 占位对象（契约类型字段 text/name/avatar）。"""
    return {"text": text, "name": "白", "avatar": None}


def _make_actions(sequencers=None):
    """构造最小 Actions 占位对象（契约字段 expressions/parameterOverrides 等）。"""
    return {
        "expressions": [],
        "pictures": None,
        "sounds": None,
        "motion": None,
        "parameterOverrides": sequencers or {},
    }


def _is_audio_frame(payload) -> bool:
    """判断一条 WS 发送载荷是否为音频帧。

    [UNSURE] 载荷结构：依据 evidence（audio 帧携带 base64 音频 + 文本），此处以"含音频内容/含文本"
    两条启发式判断；因未读实现、无法确认真实字段名。若实现把音频帧字段名不同，主代理需仲裁。
    本测试聚焦"帧数/是否发出"这一机械断言，payload 具体键名仅作防线，不作为主要断言依据。
    """
    if payload is None:
        return False
    if isinstance(payload, str):
        return True                       # 字符串载荷（可能是 base64 音频）视为内容帧
    if isinstance(payload, dict):
        text = payload.get("text")
        has_audio = any(
            k in payload for k in ("audio", "audio_base64", "data", "pcm", "visemes")
        )
        return has_audio or bool(text)
    return False


# ---------------------------------------------------------------------------
# fixture：可注入的 TTSTaskManager 构造（RED 目标：interrupt 待实现）
# ---------------------------------------------------------------------------

@pytest.fixture
def manager_factory():
    """返回工厂：构造 TTSTaskManager 并注入 mock 引擎与 get_instruction。

    引擎注入 + 模块命名空间 patch get_instruction（与 P5 范式完全一致）。
    instruction stub 附带 `|seq=<n>`，使引擎调用可唯一识别段序，从而断言"旧句 vs 新句"。
    """
    def build(engine, instruction_map=None, concurrency_limit=2):
        if instruction_map is None:
            # 默认：每 emotion 一个指令，并附 seq 标记（由 sequence_number 写入）
            instruction_map = {}

        def fake_get_instruction(emotion, strength=2, mood_tier="neutral", sequence_number=None):
            base = instruction_map.get(emotion, f"{emotion}-ins")
            if sequence_number is not None:
                base = f"{base}|seq={sequence_number}"
            return base

        ws_send, messages = _make_websocket_log()
        manager = tts_manager_module.TTSTaskManager(
            tts_engine=engine,
            concurrency_limit=concurrency_limit,
        )

        # [UNSURE] 在 speak_async 内部，get_instruction 的调用签名是否带 sequence_number。
        #          主代理需按实现仲裁：若调用点只传 (emotion, strength, mood_tier)，
        #          则 seq 标记需改名传递（例如依赖 tts_text 内含 seq 前缀）。
        patcher = None
        try:
            patcher = mock.patch.object(
                tts_manager_module, "get_instruction", fake_get_instruction
            )
            patcher.start()
        except AttributeError:
            patcher = None   # 模块名空间无 get_instruction 时静默降级（仍有 RED 目标 interrupt）
        import atexit
        if patcher is not None:
            atexit.register(patcher.stop)
        return manager, ws_send, messages, patcher

    return build


# ---------------------------------------------------------------------------
# B1 · 打断语义：interrupt 停止播放 + 清待播队列 + 不再发旧句 audio 帧
# ---------------------------------------------------------------------------

class TestInterruptStopsOldTurn:
    def test_interrupt_clears_queued_old_turn_emits_no_frames(self, manager_factory):
        # F-3.1/F-3.2/F-3.6：新句/新回合到达 → 停止当前播放 + 清空待播队列 + 丢弃旧句。
        # 前置：旧回合已先后入队多句（seq1 慢合成、seq2/seq3 快速合成）。
        # 动作：await interrupt()
        # 预期：打断后任何线程都不再向 websocket 发送旧句 audio 帧（messages 为空，
        #       因为所有旧句在打断点已被清/停）。
        engine = FakeTTSEngine(delay=0.2)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            # 旧回合：3 句全部入队
            for i in (1, 2, 3):
                await manager.speak_async(
                    tts_text=f"旧句{i}", display_text=_make_display_text(f"旧句{i}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=i, websocket_send=ws_send,
                )
            # 打断：应据此停当前、清队列、丢旧句 → 其后不应有任何旧句帧发出
            await manager.interrupt()
            # 给在飞合成/后台任务一段结算时间，若实现漏发旧句帧则在此暴露
            await asyncio.sleep(0.35)
            return len(messages)

        sent = asyncio.run(scenario())
        # 打断后旧回合的音频一帧都不应发出
        assert sent == 0, (
            f"interrupt 后旧句帧仍被发出 {sent} 条，打断未清除待播队列/未停止播放"
        )
        patcher and patcher.stop()

    def test_interrupt_while_synthesis_inflight_stops_delivery(self, manager_factory):
        # F-1.3：打断时合成在飞 → 清空排队响应 + 停止其兑现为已发帧（drain，不只暂停）。
        # 前置：先入队 seq1（慢合成 0.2s，打断点仍在飞）；随后立即可入队 seq2（快）。
        # 动作：在 seq1 尚未合成完成时 interrupt()
        # 预期：interrupt 返回后到结算窗口结束，消息日志里没有任何旧句帧。
        engine = FakeTTSEngine(delay=0.05)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.speak_async(
                tts_text="在飞句", display_text=_make_display_text("在飞句"),
                actions=_make_actions(), emotion="joy", mood_tier="happy",
                sequence_number=1, websocket_send=ws_send,
            )
            await asyncio.sleep(0.01)       # 确保 seq1 进入在飞状态（delay=0.05）
            await manager.speak_async(
                tts_text="已入队未播", display_text=_make_display_text("已入队未播"),
                actions=_make_actions(), emotion="sadness", mood_tier="neutral",
                sequence_number=2, websocket_send=ws_send,
            )
            await manager.interrupt()        # 在 seq1 仍在合成时打断
            await asyncio.sleep(0.15)        # 结算窗口
            return len(messages)

        sent = asyncio.run(scenario())
        assert sent == 0, (
            f"interrupt 打断在飞合成后仍发出 {sent} 条旧句帧，未正确停/清队列"
        )
        patcher and patcher.stop()


# ---------------------------------------------------------------------------
# B2 · 新回合优先：旧句帧不发、新句帧照发
# ---------------------------------------------------------------------------

class TestNewTurnPriority:
    def test_interrupt_then_new_turn_delivers_only_new_frames(self, manager_factory):
        # F-3.1/F-3.2：旧回合多句已入队 → 新回合打断 → 旧句帧不发 + 新句帧照发。
        # 前置：旧回合 seq1/seq2 入队；interrupt；随后新回合 speak_async seq1'。
        # 预期：结算后 messages 里的内容帧全部来自新回合；旧回合一帧也没有。
        engine = FakeTTSEngine(delay=0.05)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            # 旧回合
            for i in (1, 2):
                await manager.speak_async(
                    tts_text=f"旧{['一','二'][i-1]}句", display_text=_make_display_text(f"旧句{i}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=i, websocket_send=ws_send,
                )
            await manager.interrupt()
            # 新回合（打断后）
            await manager.speak_async(
                tts_text="新回合首句", display_text=_make_display_text("新回合首句"),
                actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                sequence_number=1, websocket_send=ws_send,
            )
            # 等新句合成+发出
            await asyncio.sleep(0.3)
            # flush 兜底（若实现为异步发帧，flush 应保证已合成都发出）+ 捕获结算窗口
            try:
                await manager.interrupt()   # 第二次打断：应只影响尚未发的新帧，不影响已发
            except Exception:
                pass
            return messages

        msgs = asyncio.run(scenario())
        # 至少新回合发出了内容帧
        frames = [m for m in msgs if _is_audio_frame(m)]
        assert len(frames) >= 1, (
            "interrupt 后新回合的句子未发出任何 audio 帧，新回合被永久静音（打断≠禁言）"
        )
        # 旧回合句子一帧都不应出现在消息日志里
        old_texts = [m for m in msgs if _is_audio_frame(m) and isinstance(m, dict)]
        old_hits = [
            m for m in old_texts
            if isinstance(m.get("text"), str) and "旧" in m["text"]
        ]
        assert len(old_hits) == 0, f"打断后旧回合句帧仍被发出: {old_hits}"
        patcher and patcher.stop()


# ---------------------------------------------------------------------------
# B3 · 幂等：无播放/无队列时打断 → 不抛异常、安全
# ---------------------------------------------------------------------------

class TestInterruptIdempotent:
    def test_interrupt_with_nothing_playing_is_safe(self, manager_factory):
        # 前置：全新 manager，从未 speak_async（无播放、无队列）。
        # 动作：await interrupt()
        # 预期：不抛异常；随后 speak_async + flush 仍正常工作（打断无副作用残留）。
        engine = FakeTTSEngine(delay=0.0)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.interrupt()          # 必须安全返回
            # 打断后管线仍可用
            await manager.speak_async(
                tts_text="后续句", display_text=_make_display_text("后续句"),
                actions=_make_actions(), emotion="neutral", mood_tier="neutral",
                sequence_number=1, websocket_send=ws_send,
            )
            try:
                await manager.flush()
            except Exception:
                return "flush_raise"
            return f"frames={len(messages)}"

        result = asyncio.run(scenario())
        assert result != "flush_raise", "interrupt 后 flush 抛异常，打断残留脏状态"
        assert result != "frames=0", "interrupt 在无播放调用后，打断使后续 speak 无法发帧"
        patcher and patcher.stop()

    def test_double_interrupt_is_safe(self, manager_factory):
        # 重复打断（连续两次）不抛异常、安全。
        engine = FakeTTSEngine(delay=0.0)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            await manager.interrupt()
            await manager.interrupt()          # 连续第二次
            return "ok"

        result = asyncio.run(scenario())
        assert result == "ok", "连续两次 interrupt 不应抛异常"
        patcher and patcher.stop()


# ---------------------------------------------------------------------------
# B4 · 状态：打断后 internal 状态（待播 task / 待发 audio 帧）归零
# ---------------------------------------------------------------------------

class TestInterruptState:
    def test_interrupt_empties_internal_pending_state(self, manager_factory):
        # F-4.4：本项目 old bug 正是"new 回合 arrival 时 task_list 累积多句仍继续合成播放"。
        # B4 契约：interrupt 后内部待播 task 清空（无残留），其后不再有旧句兑现。
        # [UNSURE] 内部属名 `task_list` 由 F-4.4 术语推得，未读实现无法确认真名；
        #          断言写成"读得到即校验、读不到则仍视为 RED（方法本身即缺）"，
        #          主代理需仲裁最终暴露/读哪个状态字段。
        engine = FakeTTSEngine(delay=0.0)
        manager, ws_send, messages, patcher = manager_factory(engine)

        async def scenario():
            for i in (1, 2):
                await manager.speak_async(
                    tts_text=f"状态句{i}", display_text=_make_display_text(f"状态句{i}"),
                    actions=_make_actions(), emotion="joy", mood_tier="happy",
                    sequence_number=i, websocket_send=ws_send,
                )
            await manager.interrupt()
            await asyncio.sleep(0.1)
            return messages

        msgs = asyncio.run(scenario())
        frames = [m for m in msgs if _is_audio_frame(m)]
        # 打断后允许新句（若有）发出，但当前场景无新句 → 不应有残留旧句帧
        assert len(frames) == 0, (
            f"打断后内部仍有 {len(frames)} 条待发/已发旧句 audio 帧，状态未清空"
        )
        # [UNSURE] 内部 task_list 属名：若实现已暴露则校验为空；取不到则跳过（以帧断言为主）。
        task_list_attr = getattr(manager, "task_list", None)
        if task_list_attr is not None:
            assert len(task_list_attr) == 0, (
                f"interrupt 后内部 task_list 未清空，仍含 {len(task_list_attr)} 项"
            )
        patcher and patcher.stop()
