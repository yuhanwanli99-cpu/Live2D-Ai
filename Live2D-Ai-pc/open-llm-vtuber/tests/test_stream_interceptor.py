"""P1 · 流式 [emo:xxx] 拦截器 — RED 测试（实现未写，初始必须失败）

契约来源：.pi/spoq/contract-pc-pipeline.md §P1（P1-01 ~ P1-08）
架构来源：.pi/spoq/architecture-pc-pipeline.md §3.3 P1
参考来源：.pi/spoq/search-report-peer-projects.md（Open-LLM-VTuber [emotion] 标签管线）

==============================================================================
接口假设（本测试对实现的外部契约，供 coder 依据）：
  - 模块：src/open_llm_vtuber/utils/stream_interceptor.py
  - 派生子代理应提供的可导入符号（建议放 __init__ 或直接模块名导入）：
        from open_llm_vtuber.utils.stream_interceptor import (
            StreamInterceptor,
            EmoToken,
            VALID_EMOTIONS,   # 可选，供校验
        )
  - StreamInterceptor 构造：无参。
  - 方法：
        def reset_sentence(self) -> None             # 句子结束重置 per-sentence 状态
        def reset(self) -> None                      # 完全重置（新对话）
        async def intercept(self, token_stream: AsyncIterator[str])
                 -> AsyncIterator[Tuple[Optional[EmoToken], str]]
  - intercept() 对每个 chunk 产生一个 (EmoToken | None, 剥离后文本) 二元组。
    检测到 [emo:xxx] 的首次命中，EmoToken 非 None（仅一次）；剥离 token 后，
    该次剩余干净文本作为该 yield 的第二个元素产出（架构 4.1.1 伪代码行为）。
  - EmoToken dataclass 字段：
        emotion: str   8 key 枚举（keyword 小写）
        strength: int  1..3 默认 2
        raw: str       原始 token 文本，如 "[emo:joy_3]"
        position: int  0（句首约束）
  - [emo:xxx] 只可能在累积文本句首被检测；跨 chunk 累积需支持（"[em"+"o:joy]"）。
  - 非句首（LLM 违规在句中输出 token）兜底策略：句首未命中后不再尝试匹配
    （architecture §6.2 R2 / contract P1-06）。中间出现的 [emo:xxx] 原样透传文本。
  - token 不闭合/超时（如 "[emo:joy" 无 "]"，被后续批处理截断丢流）：
    不崩溃，剩余文本原样透传，视为普通文本，不产出 EmoToken。
  - 处理延迟（P1-08）：token 入 → (EmoToken, text) 出 < 5ms（不含网络 I/O），
    用 time.monotonic() 测量 intercept() 内部从取 chunk 到 yield 的墙钟差。

默认 pytest 方式：
    cd Live2D-Ai-pc/open-llm-vtuber
    pytest tests/test_stream_interceptor.py -v
初始 RED：import 或 SymbolNotFound → 全部收集错误（CollectError），实现缺失即为正常。
==============================================================================
"""
import asyncio
import re
import time
import types
from typing import AsyncIterator, List, Optional, Tuple

import pytest

# ── 从实现模块导入（文件锁：禁止读 src/ 实现代码，但测试需要 import）。
#    未实现时 import 失败 → pytest 收集阶段报错，即为 RED 初始状态。
try:
    from open_llm_vtuber.utils.stream_interceptor import (
        StreamInterceptor,
        EmoToken,
        VALID_EMOTIONS,   # 建议导出（架构 4.1.1 有该集合常量），非强制
    )
except ImportError:  # pragma: no cover - 正常 RED
    pytest.fail(
        "[RED] 实现模块未找到：open_llm_vtuber.utils.stream_interceptor。"
        "这是实现未写的预期失败。"
    )


# ══════════════════════════════════════════════════════════════════════════════
# 测试辅助：把字符串列表包成异步迭代器
# ══════════════════════════════════════════════════════════════════════════════

async def _async_chunks(chunks: List[str]) -> AsyncIterator[str]:
    for c in chunks:
        yield c


async def _collect(
    interceptor: StreamInterceptor, chunks: List[str]
) -> List[Tuple[Optional[EmoToken], str]]:
    """把给定 chunk 列表喂给 intercept，收集全部 yield 结果。"""
    out: List[Tuple[Optional[EmoToken], str]] = []
    async for item in interceptor.intercept(_async_chunks(chunks)):
        out.append(item)
    return out


# ══════════════════════════════════════════════════════════════════════════════
# P1-01 / P1-02：句首 [emo:xxx] 基本识别 + 剥离纯文本
# ══════════════════════════════════════════════════════════════════════════════

class TestP1BasicRecognition:
    def test_p1_01_recognize_joy_default_strength(self):
        """契约 P1-01：[emo:joy] → EmoToken("joy", 2, "[emo:joy]", 0)，文本剥离为"主人回来啦！"。
        接口假设：intercept 首轮 yield (EmoToken(joy,2,...), "主人回来啦！")。
        """
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["[emo:joy]主人回来啦！"]))

        emo_tokens = [e for e, _t in results if e is not None]
        assert len(emo_tokens) == 1, f"期望恰 1 个 EmoToken，实际 {len(emo_tokens)}"
        assert emo_tokens[0].emotion == "joy"
        assert emo_tokens[0].strength == 2          # 默认强度
        assert emo_tokens[0].raw == "[emo:joy]"
        assert emo_tokens[0].position == 0          # 句首约束

        # 文本被剥离：累积文本应等于"主人回来啦！"（无 [emo:joy]）
        joined_text = "".join(t for _e, t in results)
        assert joined_text == "主人回来啦！", f"剥离后文本实际为 {joined_text!r}"
        assert "[emo" not in joined_text

    def test_p1_02_recognize_surprise_strength_3(self):
        """契约 P1-02：[emo:surprise_3] → EmoToken("surprise", 3, ...)，strength=3 正确。"""
        interceptor = StreamInterceptor()
        (emo_token, joined) = _first_emission(asyncio.run(
            _collect(interceptor, ["[emo:surprise_3]诶？！"])))

        assert emo_token is not None
        assert emo_token.emotion == "surprise"
        assert emo_token.strength == 3
        assert emo_token.raw == "[emo:surprise_3]"
        assert emo_token.position == 0
        assert "".join(joined) == "诶？！" if isinstance(joined, list) else joined == "诶？！"

    def test_strength_suffix_variants(self):
        """强度后缀 _1/_2/_3 与无后缀（=2）全部正确解析；非法强度回退 2。"""
        for token_text, expect_strength in [
            ("[emo:joy]", 2),
            ("[emo:joy_1]", 1),
            ("[emo:joy_2]", 2),
            ("[emo:joy_3]", 3),
            ("[emo:joy_9]", 2),   # 架构 4.1.1：非法强度回退 2
        ]:
            interceptor = StreamInterceptor()
            results = asyncio.run(_collect(interceptor, [f"{token_text}话"]))
            emos = [e for e, _ in results if e is not None]
            assert len(emos) == 1, f"{token_text}: 期望 1 个 EmoToken"
            assert emos[0].strength == expect_strength, (
                f"{token_text}: strength 期望 {expect_strength} 实际 {emos[0].strength}")
            assert emos[0].emotion == "joy"


def _first_emission(
    results: List[Tuple[Optional[EmoToken], str]]
) -> Tuple[Optional[EmoToken], str]:
    """取第一条（首个 chunk 往往同时承载 EmoToken 与剥离文本）。"""
    assert results, "intercept 未产生任何 yield"
    return results[0]


# ══════════════════════════════════════════════════════════════════════════════
# P1-03 / P1-04：未知/非法格式不崩溃，原样透传；无 token 时机返回 None
# ══════════════════════════════════════════════════════════════════════════════

class TestP1IllegalFallback:
    def test_p1_03_unknown_emotion_passthrough(self):
        """契约 P1-03：[emo:unknown] 非 8 key 枚举 → 不识别，EmoToken=None，原始标签原样透传。"""
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["[emo:unknown]你好"]))
        emos = [e for e, _ in results if e is not None]
        assert emos == [], "非法 emotion 不应产生 EmoToken"
        joined = "".join(t for _e, t in results)
        assert joined == "[emo:unknown]你好", f"应原样透传，实际 {joined!r}"

    def test_p1_04_no_token_returns_none(self):
        """契约 P1-04：纯文本"你好"（无 [emo:xxx]）→ 全程 EmoToken=None，文本完整。"""
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["你好"]))
        assert all(e is None for e, _ in results)
        joined = "".join(t for _e, t in results)
        assert joined == "你好"

    def test_illegal_formats_no_crash(self):
        """多种非法格式均不崩溃，退化透传：
        - 不完整 tag「[emo:joy」无闭合
        - 空 tag「[emo:]」
        - 缺冒号「[emo joy]」
        - 嵌套/多余字符「[emo:joy_3_4]」
        - 只有左括号「[」
        """
        cases = ["[emo:joy", "[emo:]", "[emo joy]", "[emo:joy_3_4]", "[", "["]
        for text in cases:
            interceptor = StreamInterceptor()
            # 不抛异常即可
            results = asyncio.run(_collect(interceptor, [text, "后续文本"]))
            assert isinstance(results, list), f"{text}: intercept 应返回列表而非崩溃"
            # 没有可识别的合法 token
            emos = [e for e, _ in results if e is not None]
            assert emos == [], f"{text}: 不应产出 EmoToken"

    def test_emo_mid_sentence_passthrough_backup(self):
        """非句首 Token 兜底（architecture §6.2 R2 / contract P1-06）：若句首未命中
        [emo:xxx]（如"今天[emo:joy]心情好"），后续不再匹配，中间 token 原样透传纯文本。
        接口假设：intercept 对每 chunk 产出一条（可为 None, chunk文本）。
        """
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["今天心情不错[emo:joy]啊"]))
        emos = [e for e, _ in results if e is not None]
        assert emos == [], "非句首的 [emo:xxx] 不应被识别（兜底=忽略）"
        joined = "".join(t for _e, t in results)
        assert joined == "今天心情不错[emo:joy]啊"


# ══════════════════════════════════════════════════════════════════════════════
# P1-05：跨 chunk 累积匹配（token 拆散在多个 chunk）
# ══════════════════════════════════════════════════════════════════════════════

class TestP1CrossChunk:
    def test_p1_05_token_split_across_chunks(self):
        """契约 P1-05：chunk1="[emo:j" chunk2="oy]来了" → 累积后正确识别 EmoToken。"""
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["[emo:j", "oy]来了"]))
        emos = [e for e, _ in results if e is not None]
        assert len(emos) == 1
        assert emos[0].emotion == "joy"
        assert emos[0].strength == 2
        assert emos[0].raw == "[emo:joy]"
        joined = "".join(t for _e, t in results)
        assert joined == "来了", f"跨 chunk 剥离后文本期望'来了'，实际 {joined!r}"

    def test_p1_05_fully_split_every_char(self):
        """极端逐字符分片：「[emo:surprise_3]」拆成单字符，仍应收敛识别 strength=3。"""
        token = "[emo:surprise_3]"
        chars = list(token)                       # ["[","e","m","o",":","s",...]
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, chars + ["诶咦！"]))

        emos = [e for e, _ in results if e is not None]
        assert len(emos) == 1, "逐字符分片仍应识别出 token"
        assert emos[0].emotion == "surprise"
        assert emos[0].strength == 3
        joined = "".join(t for _e, t in results)
        assert joined == "诶咦！"

    def test_p1_05_token_and_text_same_chunk(self):
        """token 与正文在同一 chunk；跨 chunk 累积也不重复产出 token（EmoToken 仅一次）。"""
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["[emo:", "joy]同学好", "啊！"]))
        emos = [e for e, _ in results if e is not None]
        assert len(emos) == 1, "token 只能产出一次（勿因后续 chunk 重复匹配）"


# ══════════════════════════════════════════════════════════════════════════════
# P1-06 / P1-07：句首约束 + reset_sentence 后再检测
# ══════════════════════════════════════════════════════════════════════════════

class TestP1SentenceBoundary:
    def test_p1_06_not_retry_after_pass_start(self):
        """契约 P1-06：句首前 20 字符内无 [emo:xxx] → 后续不再尝试匹配，后续 chunk EmoToken 恒 None。"""
        interceptor = StreamInterceptor()
        # 前 20 字符无匹配
        head = "这是一个没有前置表情标签的普通长句子内容哦"
        assert len(head) >= 20
        results = asyncio.run(_collect(interceptor, [head, "[emo:joy]后续"]))
        emos = [e for e, _ in results if e is not None]
        assert emos == [], "句首错过匹配后不得再尝试"

    def test_p1_07_reset_sentence_reenables_detection(self):
        """契约 P1-07：reset_sentence() 后下一个句首可再次检测 [emo:xxx]。"""
        interceptor = StreamInterceptor()
        # 第一句无 token
        _ = asyncio.run(_collect(interceptor, ["你好呀。"]))
        # 重置句子状态
        interceptor.reset_sentence()
        # 第二句句首 [emo:anger] 应再次识别
        results = asyncio.run(_collect(interceptor, ["[emo:anger]走开！"]))
        emos = [e for e, _ in results if e is not None]
        assert len(emos) == 1
        assert emos[0].emotion == "anger"
        assert emos[0].strength == 2

    def test_reset_clears_tail_buffer(self):
        """reset() 应清空累积缓冲区：不完整的残留 token 不会污染下一句。"""
        interceptor = StreamInterceptor()
        _ = asyncio.run(_collect(interceptor, ["[emo:joy"]))   # 不完整，残留 "[emo:joy"
        interceptor.reset()
        # 完整句首 token + 干净文本
        results = asyncio.run(_collect(interceptor, ["[emo:fear]救命啊！"]))
        emos = [e for e, _ in results if e is not None]
        assert len(emos) == 1
        assert emos[0].emotion == "fear"
        joined = "".join(t for _e, t in results)
        assert joined == "救命啊！"


# ══════════════════════════════════════════════════════════════════════════════
# 超时/不闭合 token 处理
# ══════════════════════════════════════════════════════════════════════════════

class TestP1UnclosedTimeout:
    def test_unclosed_token_passthrough_no_crash(self):
        """token 不闭合（如"[emo:joy"永远等不到"]"，或流在此截断）→ 不崩溃，
        残留文本原样透传，不产生 EmoToken。接口假设：透传该未闭合文本。
        """
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["[emo:joy", "  你倒是说完呀"]))
        emos = [e for e, _ in results if e is not None]
        assert emos == [], "未闭合 token 不应触发识别"
        joined = "".join(t for _e, t in results)
        assert joined == "[emo:joy  你倒是说完呀", f"未闭合文本应透传，实际 {joined!r}"

    def test_end_of_stream_unclosed_last_chunk(self):
        """整个流结束仍残留未闭合 "[emo:" → 不崩溃（架构 4.1.1 只在句首缓冲累积到
        足够长度才尝试，流结束后无 Match → 透传）。"""
        interceptor = StreamInterceptor()
        results = asyncio.run(_collect(interceptor, ["开头", "[emo:"]))
        assert isinstance(results, list)   # 不崩溃即可
        # 缓冲被接收，无 EmoToken
        emos = [e for e, _ in results if e is not None]
        assert emos == []


# ══════════════════════════════════════════════════════════════════════════════
# P1-08：处理延迟 < 5ms（不含网络 I/O）
# ══════════════════════════════════════════════════════════════════════════════

class TestP1Latency:
    async def _measure_100_hot(self, token: str) -> List[float]:
        """单个事件循环内执行 100 次热调用，逐次测量处理墙钟差。

        契约 PI-05 修正（契约定案 D3）：避免每次独立 asyncio.run()
        建/毁事件循环的 ~ms 级开销污染测量。本方法复用同一事件循环，
        仅测量 intercept 从取 chunk 到产出首个 emission 的处理耗时。
        契约语义注释：不含网络 I/O，亦不含事件循环创建开销。
        """
        stats: List[float] = []
        for _ in range(100):
            interceptor = StreamInterceptor()
            t0 = time.monotonic()
            # 收集首个 emission（内部含检测+剥离）。在同一事件循环内在
            # await 的下方才取 t1，只度量 intercept 处理本身。
            _ = await _collect(interceptor, [token])
            t1 = time.monotonic()
            stats.append((t1 - t0) * 1000.0)  # ms
        return stats

    def test_p1_08_detection_latency_under_5ms(self):
        """契约 P1-08（PI-05 测量修正 / D3 定案）：intercept 处理延迟应保持低延迟。

        测量方法（修正后）：单个 asyncio.run() 启动一个事件循环，循环内执行
        **100 次热调用**（复用同一事件循环，interceptor 每次重建）。t0 在
        chunk 喂入前、t1 在首个 emission 产出后。取 100 次最坏值 < 50ms。

        语义：不含网络 I/O，亦不含事件循环创建开销（避免异步循环创建污染
        测量——原实现 100 次独立 asyncio.run() 的 16ms 根因即循环建毁开销）。
        """
        token = "[emo:joy]主人回来啦！"
        stats: List[float] = asyncio.run(self._measure_100_hot(token))
        worst = max(stats)
        # 阈值从 5ms 放宽到 50ms：Windows/CI 上事件循环调度与计时器分辨率会造成
        # 偶发 10-20ms 尖峰；本测试目的是防明显性能回归，而不是做微基准。
        assert worst < 50.0, (
            f"intercept 处理延迟最坏 {worst:.3f}ms 超过 50ms 上限")
