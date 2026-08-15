"""PC 端 Actions.motion 序列化 + transformers 接线测试（M8）。

覆盖（契约 M6-M8 §5.1）:
- T1-T4, P22: Actions.to_dict() 序列化（motion 省略/过滤/全字段）
- T5-T9/P16: actions_extractor 接线（结构化 JSON → expressions/motion 派生）
- T10-T12: prepare_audio_payload 含/不含 motion

构造: _PC_SRC 注入 output_types.py / transformers.py / stream_audio.py。
make_wav_bytes 助理由 test_pymouth_viseme.py 复制（生成合法 WAV 用于 payload 测试）。
"""

import io
import os
import sys
import tempfile
import pytest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.agent.output_types import Actions  # noqa: E402
from open_llm_vtuber.utils.stream_audio import prepare_audio_payload  # noqa: E402
from open_llm_vtuber.agent.transformers import actions_extractor  # noqa: E402
from open_llm_vtuber.utils.sentence_divider import (  # noqa: E402
    SentenceWithTags,
    TagState,
    TagInfo,
)

# make_wav_bytes + temp file helper（prepare_audio_payload 需要文件路径，非 bytes）
import wave  # noqa: E402
import numpy as np  # noqa: E402


def make_wav_bytes(
    duration_ms: int = 600,
    sample_rate: int = 16000,
    channels: int = 1,
    tone_hz: float = None,
) -> bytes:
    n = int(sample_rate * duration_ms / 1000)
    if tone_hz:
        t = np.arange(n) / sample_rate
        samples = (0.4 * np.sin(2 * np.pi * tone_hz * t) * 32767).astype(np.int16)
    else:
        samples = np.zeros(n, dtype=np.int16)
    if samples.ndim == 1 and channels > 1:
        samples = np.repeat(samples[:, None], channels, axis=1)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        w.writeframes(samples.astype(np.int16).tobytes())
    return buf.getvalue()


def _write_temp_wav(wav_bytes: bytes) -> str:
    """将 WAV 字节流写入临时文件，返回路径（由调用方负责清理）。"""
    tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
    tmp.write(wav_bytes)
    tmp.close()
    return tmp.name


# ---------------------------------------------------------------------------
# T1-T4, P22: Actions.to_dict() 序列化
# ---------------------------------------------------------------------------


class TestActionsToDict:
    def test_t1_motion_serialization(self):
        """T1: Actions(motion="wave").to_dict() == {"motion":"wave"}"""
        result = Actions(motion="wave").to_dict()
        assert result == {"motion": "wave"}

    def test_t2_no_motion(self):
        """T2: Actions().to_dict() == {}（无 motion 键，省略语义）"""
        result = Actions().to_dict()
        assert result == {}
        assert "motion" not in result

    def test_t3_mixed_with_expressions(self):
        """T3: Actions(expressions=[0], motion="wave").to_dict() 含全字段"""
        result = Actions(expressions=[0], motion="wave").to_dict()
        assert result == {"expressions": [0], "motion": "wave"}

    def test_t4_explicit_none(self):
        """T4: Actions(motion=None).to_dict() == {}（None 被过滤）"""
        result = Actions(motion=None).to_dict()
        assert result == {}
        assert "motion" not in result

    def test_p22_full_fields(self):
        """P22: Actions(expressions=[5,7], motion="wave",
        parameterOverrides={"ParamEyeLOpen":1.2}).to_dict() 全字段命中。"""
        result = Actions(
            expressions=[5, 7],
            motion="wave",
            parameterOverrides={"ParamEyeLOpen": 1.2},
        ).to_dict()
        assert "expressions" in result
        assert result["expressions"] == [5, 7]
        assert result["motion"] == "wave"
        assert result["parameterOverrides"] == {"ParamEyeLOpen": 1.2}

    def test_p22_none_omitted(self):
        """P22: None 字段不进 payload。"""
        result = Actions(
            expressions=[5],
            motion=None,
            parameterOverrides=None,
        ).to_dict()
        assert "motion" not in result
        assert "parameterOverrides" not in result
        assert "expressions" in result


# ---------------------------------------------------------------------------
# T10-T12: prepare_audio_payload 含 motion
# ---------------------------------------------------------------------------


class TestAudioPayloadMotion:
    def test_t10_payload_with_motion(self):
        """T10: payload["actions"]["motion"]=="wave"；visemes 仍为 list。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=Actions(motion="wave"))
            assert payload["actions"] is not None
            assert payload["actions"]["motion"] == "wave"
            assert isinstance(payload["visemes"], list)
        finally:
            os.unlink(wav_path)

    def test_t11_payload_no_motion(self):
        """T11: actions=Actions() → payload["actions"]=={}。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=Actions())
            assert payload["actions"] == {}
        finally:
            os.unlink(wav_path)

    def test_t12_payload_actions_none(self):
        """T12: actions=None → payload["actions"] is None。"""
        wav_path = _write_temp_wav(make_wav_bytes(duration_ms=600, tone_hz=440))
        try:
            payload = prepare_audio_payload(wav_path, actions=None)
            assert payload["actions"] is None
        finally:
            os.unlink(wav_path)


# ---------------------------------------------------------------------------
# T4.1: PC 关键词兜底（K1-K8，契约 v3-features §1.4）
#
# RED 目标：实现未做关键词兜底 → extract_motion_keyword 不存在 / MOTION_KEYWORDS
#           不存在 / actions_extractor 无兜底 wiring。以下断言预期 FAIL。
# 设计唯一依据：契约 v3-features §1.4（K1-K8 断言） + §1.1 词表 + §1.2 wiring。
# ❌ 未读 src/ 实现内部，仅按公开接口驱动（actions_extractor 装饰器 + 假 model）。
# ---------------------------------------------------------------------------


class _KeywordFakeModel:
    """可编程假 model：驱动 actions_extractor 的 structured/降级两条路径。

    公共接口与合同 §1.2 wiring 所需一致：extract_expression_mix / strip_expression_json
    / extract_emotion。关键词兜底由 transformers 模块内新函数 extract_motion_keyword
    提供（本测试不调用，仅验证 actions_extractor 的集成结果）。
    """

    def __init__(self, structured):
        self._structured = structured

    def extract_expression_mix(self, text):
        return self._structured

    def strip_expression_json(self, text):
        return text

    def extract_emotion(self, text):
        return []


async def _drive_extractor(model, text):
    """跑一次 actions_extractor：输入文本 → 返回 (sentence, Actions)。"""
    @actions_extractor(model)
    async def _gen(*_a, **_k):
        yield SentenceWithTags(text=text, tags=[])

    out = None
    async for item in _gen():
        out = item
        break
    sentence, actions = out
    return sentence, actions


def _import_motion_module():
    """导入 keywords 相关符号；未实现 → AttributeError（RED）。"""
    import open_llm_vtuber.live2d_model as m
    return m


class TestKeywordFallbackWiring:
    """K1-K4/K8 — actions_extractor 集成：motionLabel 主路径 > 关键词兜底。"""

    def test_k1_llm_explicit_motionlabel_priority(self):
        """K1: structured.motionLabel="wave" 时直接用（LLM 显式，不查关键词）。"""
        model = _KeywordFakeModel(
            structured={"expressionMix": [], "parameterOverrides": {}, "motionLabel": "wave"}
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你再见！"))
        assert actions.motion == "wave", (
            f"motionLabel('wave') 应直接生效，实际 {actions.motion!r}（无关键词兜底接线 → RED）"
        )

    def test_k2_keyword_fallback_structured_motionlabel_none(self):
        """K2: motionLabel=None + 文本"你再见！" → 关键词兜底 special_02。"""
        model = _KeywordFakeModel(
            structured={"expressionMix": [], "parameterOverrides": {}, "motionLabel": None}
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你再见！"))
        assert actions.motion == "special_02", (
            f"文本含再见应兜底 special_02，实际 {actions.motion!r}（无兜底接线 → RED）"
        )

    def test_k3_keyword_fallback_degraded_path(self):
        """K3: structured=None（降级路径）+ 文本"挥手告别" → 关键词兜底 wave。"""
        model = _KeywordFakeModel(structured=None)
        sentence, actions = asyncio_run(_drive_extractor(model, "挥手告别"))
        assert actions.motion == "wave", (
            f"降级路径文本含挥手应兜底 wave，实际 {actions.motion!r}（降级路径无兜底 → RED）"
        )

    def test_k4_no_keyword_no_motion_set(self):
        """K4: 无关键词文本"今天天气不错" → motion 不设置（None → 前端随机 Talk）。"""
        model = _KeywordFakeModel(
            structured={"expressionMix": [], "parameterOverrides": {}, "motionLabel": None}
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "今天天气不错"))
        # 兜底应返回 None，motion 保持未设置
        assert actions.motion is None, (
            f"无关键词文本 motion 应为 None，实际 {actions.motion!r}"
        )

    def test_k8_priority_regression_keyword_not_trigger_when_motionlabel(self):
        """K8: motionLabel 存在时关键词不触发（K1 反例，短路）。"""
        model = _KeywordFakeModel(
            structured={"expressionMix": [], "parameterOverrides": {}, "motionLabel": "wave"}
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你再见！"))
        # 即使文本含"再见"（关键词→special_02），LLM 显式 wave 必须胜出
        assert actions.motion == "wave", (
            f"motionLabel=wave 存在时关键词(再见→special_02)不得覆盖，实际 {actions.motion!r}（联动 BUG）"
        )


class TestMotionWiringCaseNormalize:
    """T3.1 — motionLabel 大小写归一（v3-fixes §4 M2/M3：strip().lower() 后端归一）。"""

    def _drive(self, motion_label):
        model = _KeywordFakeModel(
            structured={"expressionMix": [], "parameterOverrides": {}, "motionLabel": motion_label}
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你再见！"))
        return actions

    def test_m2_uppercase_normalized(self):
        """M2: motionLabel="Wave" → "wave"（后端归一，前端大小写敏感查表不再 miss）。"""
        actions = self._drive("Wave")
        assert actions.motion == "wave", (
            f"Wave 应归一为 wave，实际 {actions.motion!r}（v3-fixes §4.2 后端归一 → RED）"
        )

    def test_m3_trim_and_lowercase(self):
        """M3: motionLabel=" WAVE " → "wave"（trim+lowercase）。"""
        actions = self._drive(" WAVE ")
        assert actions.motion == "wave", (
            f"' WAVE ' 应 trim+lowercase 为 wave，实际 {actions.motion!r}（RED）"
        )

    def test_m4_none_not_set(self):
        """M4: motionLabel=None → motion 不设置（且无关键词兜底时）。"""
        # 文本含"再见"→ 但此处 mock structured 非 None、motionLabel None → 触发关键词兜底 special_02
        actions = self._drive(None)
        assert actions.motion == "special_02", (
            f"motionLabel=None + 文本再见 → 关键词兜底 special_02，实际 {actions.motion!r}（无兜底 → RED）"
        )


class TestExpressionWeightZero:
    """T3.1 — expressionMix 中 weight=0 项：后端起（transformers）完整保留下发，
    "过滤/不渲染"由消费端（前端）负责（T3.1 任务指令：消费端过滤落点）。

    ⚠️ 落点说明：recon-techdebt-pc §1.1 记载 weight=0 派生冲突（M3 保留 vs M6-M8 T8 过滤）。
    T3.1 明确落点=“消费端过滤/不渲染”——即后端保留下发、前端消费端过滤。故此处断言后端**保留**weight=0
    （数据无损方下发），消费端（前端 .ts）的”不渲染“属前端验证范畴（tsc/手工，不在 pytest 范围）。
    """

    def test_weight_zero_preserved_backend_emission(self):
        """T3.1/后端：expressionMix:[{joy,0.0},{surprise,0.3}] → 后端保留派生（[joy,surprise]，权重信息不丢失）。"""
        model = _KeywordFakeModel(
            structured={
                "expressionMix": [
                    {"name": "joy", "weight": 0.0},
                    {"name": "surprise", "weight": 0.3},
                ],
                "parameterOverrides": {},
                "motionLabel": None,
            }
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你回来了"))
        # 后端保留 weight=0 项（[joy, surprise] 顺序保持，数据完整下发；过滤/不渲染在消费端前端）
        m = _import_motion_module()
        phe = getattr(m, "PROTOCOL_EMOTION_INDEX", None)
        assert phe is not None, "PROTOCOL_EMOTION_INDEX 存在"
        expected = [phe["joy"], phe["surprise"]]
        assert actions.expressions == expected, (
            f"后端应保留 weight=0 项完整下发 {expected}，实际 {actions.expressions!r}（背离后端保留语义）"
        )

    def test_weight_zero_preserved_all_zero(self):
        """T3.1/后端：全 weight=0 也完整下发（不丢数据）；消费端决定是否渲染。"""
        model = _KeywordFakeModel(
            structured={
                "expressionMix": [
                    {"name": "joy", "weight": 0.0},
                    {"name": "surprise", "weight": 0.0},
                ],
                "parameterOverrides": {},
                "motionLabel": None,
            }
        )
        sentence, actions = asyncio_run(_drive_extractor(model, "你回来了"))
        m = _import_motion_module()
        phe = getattr(m, "PROTOCOL_EMOTION_INDEX", None)
        expected = [phe["joy"], phe["surprise"]]
        assert actions.expressions == expected, (
            f"后端应完整下发全 weight=0 项 {expected}，实际 {actions.expressions!r}（后端不应擅自过滤）"
        )


class TestExtractMotionKeywordUnit:
    """K5/K6 — extract_motion_keyword 单元断言（词表/边界，契约 §1.1-§1.3）。"""

    def _func(self):
        m = _import_motion_module()
        fn = getattr(m, "extract_motion_keyword", None)
        assert fn is not None, (
            "extract_motion_keyword 存在（当前实现未新增 → RED）"
        )
        return fn

    def test_k5_english_words_not_in_wordlist(self):
        """K5: 终裁无英文词 — "HELLO"/"hello"/"bye" → None（反例）。"""
        fn = self._func()
        assert fn("HELLO") is None, (
            f"终裁锁无英文词：HELLO 应 None，实际 {fn('HELLO')!r}（词表残留英文词 → RED）"
        )
        assert fn("hello") is None, "hello 应 None（无英文词）"
        assert fn("bye") is None, "bye 应 None（无英文词）"

    def test_k6_same_alias_earliest_hit(self):
        """K6: 同别名最早命中 — "拜拜！再见！" → "special_02" 一次。"""
        fn = self._func()
        assert fn("拜拜！再见！") == "special_02", (
            f"拜拜/再见 同属 special_02 应归一，实际 {fn('拜拜！再见！')!r}（RED）"
        )

    def test_keyword_trim_whitespace(self):
        """契约 §1.2/1.3 — 净化后文本含首尾空白仍命中（子串匹配不变即可，先测不 crash）。"""
        fn = self._func()
        assert fn("  挥手  ") == "wave", (
            f"含空白文本应命中挥手→wave，实际 {fn('  挥手  ')!r}（RED）"
        )

    def test_keyword_no_match_returns_none(self):
        """契约 §1.2 — 无命中返回 None。"""
        fn = self._func()
        assert fn("今天天气不错") is None, (
            f"无关键词文本应返回 None，实际 {fn('今天天气不错')!r}（RED）"
        )

    def test_keyword_blank_returns_none(self):
        """契约 §1.2 — 空/空白文本返回 None。"""
        fn = self._func()
        assert fn("") is None, "空文本应返回 None（RED）"
        assert fn("   ") is None, "空白文本应返回 None（RED）"


def asyncio_run(coro):
    """同步运行单个协程（avoid 全局 asyncio.run 在已有关闭事件循环时冲突）。"""
    import asyncio
    try:
        return asyncio.run(coro)
    except RuntimeError:
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()
