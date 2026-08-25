"""PC 端 extract_expression_mix / strip_expression_json 单元测试（M3 核心解析）。

覆盖（bai 唯一化上下文，R1-model D2）:
- P1-P13: 基本 JSON 解析/校验/归一化（契约 M1 §7.2 同构于 A1-A13）
- P20: 单行 JSON 不被切分（fence + 单行）
- P21: strip_expression_json 剥离规则
- S1-S10: 双端一致性参数化样本（契约 M2-M3 §3）

构造: _PC_SRC 注入 live2d_model.py，用 Live2dModel("bai") 上下文。
cdi3 白名单由运行时加载 bai.cdi3.json（128 参数），保证参数化样本校验真实白名单行为。
bai 无 motion（adapter.motionAliases={}）→ 任意 motionLabel 均未知 → None。
"""
import json
import os
import sys
import pytest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.live2d_model import (  # noqa: E402
    Live2dModel,
    PROTOCOL_EMOTION_KEYS,
    PROTOCOL_EMOTION_INDEX,
)

# ---------------------------------------------------------------------------
# 辅助：创建 bai Live2dModel（CWD = 仓库根，保证 cdi3 能解析）
# ---------------------------------------------------------------------------

_REPO_ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))


def _make_model(name="bai"):
    """构造 Live2dModel，CWD 指向仓库根保证 cdi3.json 加载。"""
    old_cwd = os.getcwd()
    os.chdir(_REPO_ROOT)
    try:
        return Live2dModel(
            name,
            model_dict_path=os.path.join(
                _REPO_ROOT, "Live2D-Ai-pc", "open-llm-vtuber", "model_dict.json"
            ),
        )
    finally:
        os.chdir(old_cwd)


# ---------------------------------------------------------------------------
# P1-P13: 基本解析/校验（bai，契约 M1 §7.2）
# ---------------------------------------------------------------------------


class TestExtractExpressionMix:
    """extract_expression_mix 基本解析/校验用例（bai 上下文）。"""

    @classmethod
    def setup_class(cls):
        cls.model = _make_model("bai")

    def _extract(self, text):
        return self.model.extract_expression_mix(text)

    # ---- P1 fence json 解析成功 ----
    def test_fence_json_parsed(self):
        result = self._extract(
            '```json {"expressionMix":{"expressions":[{"name":"joy","weight":0.7}]}} ```'
        )
        assert result is not None
        assert result["expressionMix"] == [{"name": "joy", "weight": 0.7}]

    # ---- P2 fence 无 json 标记 ----
    def test_fence_no_json_label(self):
        result = self._extract(
            '``` {"expressionMix":{"expressions":[{"name":"joy","weight":0.7}]}} ```'
        )
        assert result is not None
        assert result["expressionMix"] == [{"name": "joy", "weight": 0.7}]

    # ---- P3 裸 JSON（正文前缀不影响） ----
    def test_naked_json_with_prefix(self):
        result = self._extract(
            '好的！{"expressionMix":{"expressions":[{"name":"joy","weight":0.7}]}}'
        )
        assert result is not None
        assert result["expressionMix"] == [{"name": "joy", "weight": 0.7}]

    # ---- P4 畸形 JSON → None ----
    def test_malformed_json(self):
        result = self._extract('{"expressionMix": {')
        assert result is None

    # ---- P5 无 JSON → None ----
    def test_no_json(self):
        result = self._extract("[joy] 今天天气真好啊！")
        assert result is None

    # ---- P6 未知 name（love → 丢弃，mix 为空） ----
    def test_unknown_name_discarded(self):
        result = self._extract(
            '{"expressionMix":{"expressions":[{"name":"love","weight":1.0}]}}'
        )
        assert result is not None
        assert result["expressionMix"] == []

    # ---- P7 weight 越界（>1 → 丢弃） ----
    def test_weight_out_of_range(self):
        result = self._extract(
            '{"expressionMix":{"expressions":[{"name":"joy","weight":1.5}]}}'
        )
        assert result is not None
        assert result["expressionMix"] == []

    # ---- P8 归一化（sum > 1.0） ----
    def test_normalization(self):
        result = self._extract(
            '{"expressionMix":{"expressions":'
            '[{"name":"joy","weight":0.7},{"name":"surprise","weight":0.6}]}}'
        )
        assert result is not None
        got_mix = result["expressionMix"]
        # joy=0.7/1.3 ≈ 0.53846, surprise=0.6/1.3 ≈ 0.46154
        assert len(got_mix) == 2
        weights = {item["name"]: item["weight"] for item in got_mix}
        assert abs(weights["joy"] - 0.7 / 1.3) <= 1e-4
        assert abs(weights["surprise"] - 0.6 / 1.3) <= 1e-4
        assert abs(sum(w for w in weights.values()) - 1.0) <= 1e-4

    # ---- P9 覆盖合法 key（ParamEyeLOpen ∈ bai cdi3 白名单） ----
    def test_override_valid_key(self):
        result = self._extract(
            '{"parameterOverrides":{"ParamEyeLOpen":1.2}}'
        )
        assert result is not None
        assert result["parameterOverrides"] == {"ParamEyeLOpen": 1.2}

    # ---- P10 覆盖非法 key（NotAParam ∉ cdi3 白名单） ----
    def test_override_invalid_key_discarded(self):
        result = self._extract(
            '{"parameterOverrides":{"NotAParam":1.0}}'
        )
        assert result is not None
        assert result["parameterOverrides"] == {}

    # ---- P11 motionLabel 未知别名 → None ----
    def test_unknown_motion_label(self):
        result = self._extract(
            '{"expressionMix":{"expressions":[{"name":"joy","weight":0.5}]},'
            '"motionLabel":"dance"}'
        )
        assert result is not None
        assert result["motionLabel"] is None
        # mix 应保留（部分生效原则）
        assert result["expressionMix"] == [{"name": "joy", "weight": 0.5}]

    # ---- P12 全字段命中 ----
    def test_all_fields(self):
        result = self._extract(
            '{"expressionMix":{"expressions":[{"name":"joy","weight":0.7},'
            '{"name":"surprise","weight":0.3}]},'
            '"parameterOverrides":{"ParamEyeLOpen":1.2},'
            '"motionLabel":"wave"}'
        )
        assert result is not None
        assert result["expressionMix"] == [
            {"name": "joy", "weight": 0.7},
            {"name": "surprise", "weight": 0.3},
        ]
        assert result["parameterOverrides"] == {"ParamEyeLOpen": 1.2}
        # bai 无 motion（adapter.motionAliases={}）→ 任意 motionLabel（如 wave）均未知 → None
        assert result["motionLabel"] is None

    # ---- P13 空 mix + 合法覆盖（部分生效） ----
    def test_empty_mix_valid_override(self):
        # ParamEyeLOpen ∈ bai cdi3 → 保留
        result = self._extract(
            '{"expressionMix":{"expressions":[]},'
            '"parameterOverrides":{"ParamEyeLOpen":0.8}}'
        )
        assert result is not None
        assert result["expressionMix"] == []
        assert result["parameterOverrides"] == {"ParamEyeLOpen": 0.8}

    # ---- P20 单行 JSON 不被切分 ----
    def test_single_line_json(self):
        """S1 输入单行化（fence 内单行 JSON）→ 解析成功。"""
        text = (
            '```json\n'
            '{"expressionMix":{"expressions":[{"name":"joy","weight":0.7},'
            '{"name":"surprise","weight":0.3}]},'
            '"parameterOverrides":{"ParamEyeLOpen":1.2},"motionLabel":"wave"}\n'
            '```\n'
            "今天天气真好啊！"
        )
        result = self._extract(text)
        assert result is not None
        assert len(result["expressionMix"]) == 2

    # ---- P21 strip_expression_json ----
    def test_strip_fence_removes_json(self):
        """S1: strip_expression_json 输出不含 expressionMix/parameterOverrides/motionLabel。"""
        text = (
            '```json\n'
            '{"expressionMix":{"expressions":[{"name":"joy","weight":0.7}]}}\n'
            '```\n'
            "今天天气真好啊！"
        )
        stripped = self.model.strip_expression_json(text)
        assert "expressionMix" not in stripped
        assert "parameterOverrides" not in stripped
        assert "motionLabel" not in stripped
        assert "今天天气真好啊！" in stripped

    def test_strip_naked_json_success(self):
        """S2: NAKED JSON 解析成功 → 剥离该 JSON 子串。"""
        text = 'I\'m so happy! {"expressionMix":{"expressions":[{"name":"joy","weight":1.0}]}}'
        stripped = self.model.strip_expression_json(text)
        assert "expressionMix" not in stripped
        assert stripped.strip() == "I'm so happy!"

    def test_strip_no_json_returns_original(self):
        """S5: 无 JSON → 原样返回。"""
        text = "[joy] 今天天气真好啊！"
        stripped = self.model.strip_expression_json(text)
        assert stripped == text


# ---------------------------------------------------------------------------
# S1-S10: 双端一致性参数化样本（契约 M2-M3 §3 + verify-M9 PC 实测 + bai 唯一化）
# ---------------------------------------------------------------------------

# 每条: (id, text, expected_mix, expected_overrides, expected_motion, expected_null)
# bai 唯一化（D2）：bai.cdi3.json 128 参数含 ParamMouthForm → S10 overrides 保留 {"ParamMouthForm":0.8}；
# bai 无 motion（adapter.motionAliases={}）→ S1 motionLabel "wave" 未知 → None。
# 旧 niziiro 上下文中 ParamMouthForm ∉ cdi3 / motion 别名为 7 键——已按 bai 事实替换。
S_SAMPLES = [
    # (id, text, mix, overrides, motion, is_null)
    (
        "S1",
        '```json\n{"expressionMix":{"expressions":[{"name":"joy","weight":0.7},'
        '{"name":"surprise","weight":0.3}]},'
        '"parameterOverrides":{"ParamEyeLOpen":1.2},"motionLabel":"wave"}\n```\n'
        "今天天气真好啊！",
        [{"name": "joy", "weight": 0.7}, {"name": "surprise", "weight": 0.3}],
        {"ParamEyeLOpen": 1.2},
        # bai 无 motion（adapter.motionAliases={}）→ motionLabel "wave" 未知 → None
        None,
        False,
    ),
    (
        "S2",
        'I\'m so happy! {"expressionMix":{"expressions":[{"name":"joy","weight":1.0}]}}',
        [{"name": "joy", "weight": 1.0}],
        {},
        None,
        False,
    ),
    (
        "S3",
        '```\n{"expressionMix":{"expressions":[{"name":"sadness","weight":0.8}]}}\n```',
        [{"name": "sadness", "weight": 0.8}],
        {},
        None,
        False,
    ),
    (
        "S4",
        '[joy] 今天天气真好啊！{"expressionMix":{"expressions":[{"name":"anger","weight":0.6}]}}',
        [{"name": "anger", "weight": 0.6}],
        {},
        None,
        False,
    ),
    (
        "S5",
        "[joy] 今天天气真好啊！",
        None,  # null
        None,
        None,
        True,
    ),
    (
        "S6",
        '{"expressionMix": {"expressions": [',
        None,
        None,
        None,
        True,
    ),
    (
        "S7",
        '{"expressionMix":{"expressions":[{"name":"love","weight":1.0},'
        '{"name":"joy","weight":1.5}]}}',
        [],  # love 未知 + joy 越界 → 全丢弃，非 null
        {},
        None,
        False,
    ),
    (
        "S8",
        '{"expressionMix":{"expressions":[{"name":"joy","weight":0.7},'
        '{"name":"surprise","weight":0.6}]}}',
        [{"name": "joy", "weight": 0.7 / 1.3}, {"name": "surprise", "weight": 0.6 / 1.3}],
        {},
        None,
        False,
    ),
    (
        "S9",
        '{"parameterOverrides":{"NotAParam":1.0}}',
        [],
        {},
        None,
        False,
    ),
    (
        "S10",
        # bai 唯一化（D2）：bai.cdi3.json 含 ParamMouthForm → C3 白名单保留 → overrides={ParamMouthForm:0.8}
        # motionLabel "dance" ∉ bai 别名表（bai 无 motion）→ motion=None
        # ref: model-facts-bai.md（128 参数含 ParamMouthForm）+ verify-M9.md §3
        '{"expressionMix":{"expressions":[]},'
        '"parameterOverrides":{"ParamMouthForm":0.8},"motionLabel":"dance"}',
        [],
        {"ParamMouthForm": 0.8},
        None,
        False,
    ),
]


class TestConsistencySamples:
    """S1-S10 双端一致性参数化样本（bai 上下文，R1-model D2）。"""

    @classmethod
    def setup_class(cls):
        cls.model = _make_model("bai")

    @pytest.mark.parametrize(
        "sample_id,text,expected_mix,expected_overrides,expected_motion,expected_null",
        S_SAMPLES,
        ids=[s[0] for s in S_SAMPLES],
    )
    def test_sample(
        self, sample_id, text, expected_mix, expected_overrides, expected_motion, expected_null
    ):
        result = self.model.extract_expression_mix(text)

        if expected_null:
            assert result is None, f"{sample_id}: expected None, got {result}"
            return

        assert result is not None, f"{sample_id}: expected non-None, got None"

        # mix
        got_mix = result["expressionMix"]
        assert len(got_mix) == len(expected_mix), (
            f"{sample_id}: mix length {len(got_mix)} != {len(expected_mix)}"
        )
        for got, exp in zip(got_mix, expected_mix):
            assert got["name"] == exp["name"], (
                f"{sample_id}: name {got['name']} != {exp['name']}"
            )
            assert abs(got["weight"] - exp["weight"]) <= 1e-4, (
                f"{sample_id}: weight {got['weight']} != {exp['weight']}"
            )

        # overrides
        assert result["parameterOverrides"] == expected_overrides, (
            f"{sample_id}: overrides {result['parameterOverrides']} != {expected_overrides}"
        )

        # motion
        assert result["motionLabel"] == expected_motion, (
            f"{sample_id}: motion {result['motionLabel']} != {expected_motion}"
        )

    # S1 附加断言：strip_expression_json 输出不含 JSON 字段
    def test_s1_strip_consistency(self):
        text = S_SAMPLES[0][1]  # S1 input
        stripped = self.model.strip_expression_json(text)
        for forbidden in ("expressionMix", "parameterOverrides", "motionLabel"):
            assert forbidden not in stripped, f"S1 strip: '{forbidden}' found in stripped text"
        assert "今天天气真好啊！" in stripped

    # S8 归一化和校验
    def test_s8_normalization_sum(self):
        result = self.model.extract_expression_mix(S_SAMPLES[7][1])
        assert result is not None
        mix = result["expressionMix"]
        assert len(mix) == 2
        total = sum(item["weight"] for item in mix)
        assert abs(total - 1.0) <= 1e-4, f"S8 sum={total}, expected ~1.0"
