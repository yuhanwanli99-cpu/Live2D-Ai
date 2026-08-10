"""PC 端 Live2dModel 回归 + 一致性测试（M3 核心）。

覆盖:
- P14: extract_emotion 并存回归（关键词路径行为不变）
- P17: 裁决1 — model_dict.json niziiro_mao emotionMap 8 索引一致（共享+运行时两处）
- P19: prompt 文件内容断言（live2d_expression_prompt.txt 含规格/占位符/示例/单行约束）

构造: _PC_SRC 注入 live2d_model.py。
"""
import json
import os
import sys
import pytest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.live2d_model import (  # noqa: E402
    Live2dModel,
    PROTOCOL_EMOTION_KEYS,
    PROTOCOL_EMOTION_INDEX,
)

_REPO_ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))


def _make_model(name="niziiro_mao"):
    old_cwd = os.getcwd()
    os.chdir(_REPO_ROOT)
    try:
        return Live2dModel(
            name,
            model_dict_path=os.path.join(
                _REPO_ROOT, "Live2Dai-pc", "open-llm-vtuber", "model_dict.json"
            ),
        )
    finally:
        os.chdir(old_cwd)


class TestExtractEmotionRegression:
    """P14: extract_emotion 关键词路径回归（行为不变）。"""

    @classmethod
    def setup_class(cls):
        cls.model = _make_model("niziiro_mao")

    def test_extract_emotion_returns_index_list(self):
        """关键词路径返回索引列表，签名/行为不变。"""
        result = self.model.extract_emotion("[joy] 你好喵~！")
        assert isinstance(result, list)
        # joy → PROTOCOL_EMOTION_INDEX["joy"] = 5
        assert 5 in result

    def test_extract_emotion_multiple_keywords(self):
        """多关键词返回多个索引。"""
        result = self.model.extract_emotion("[joy] 你好 [surprise] 诶？")
        assert 5 in result  # joy
        assert 7 in result  # surprise

    def test_extract_emotion_no_keyword(self):
        """无关键词返回空列表。"""
        result = self.model.extract_emotion("你好喵~！")
        assert result == []

    def test_remove_emotion_keywords(self):
        """关键词剥离后文本干净。"""
        cleaned = self.model.remove_emotion_keywords("[joy] 你好喵~！")
        assert "joy" not in cleaned.lower() or "[joy]" not in cleaned
        assert "你好喵~！" in cleaned


class TestModelDictConsistency:
    """P17: 裁决1 — model_dict.json emotionMap 8 索引一致。"""

    def test_pc_runtime_model_dict_8_index(self):
        """PC 运行时 model_dict.json niziiro_mao emotionMap == PROTOCOL_EMOTION_INDEX。"""
        runtime_path = os.path.join(
            _REPO_ROOT, "Live2Dai-pc", "open-llm-vtuber", "model_dict.json"
        )
        with open(runtime_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        mao = next(m for m in data if m["name"] == "niziiro_mao")
        emo_map = mao["emotionMap"]

        # 逐 key 断言（字符串 key；值 = 8 索引）
        assert emo_map == PROTOCOL_EMOTION_INDEX, (
            f"model_dict.json emotionMap:\n{emo_map}\n!=\nPROTOCOL_EMOTION_INDEX:\n{PROTOCOL_EMOTION_INDEX}"
        )

    def test_shared_model_dict_8_index(self):
        """shared/model_dict.json niziiro_mao emotionMap == PROTOCOL_EMOTION_INDEX。"""
        shared_path = os.path.join(_REPO_ROOT, "shared", "model_dict.json")
        if not os.path.isfile(shared_path):
            pytest.skip("shared/model_dict.json 不存在")
        with open(shared_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        # shared/model_dict.json 是 dict 结构（key 为模型名），不同于运行时的 list
        if isinstance(data, dict):
            # 结构: {"niziiro_mao": {...}, ...}
            mao = data.get("niziiro_mao")
            if isinstance(mao, dict) and "emotionMap" in mao:
                assert mao["emotionMap"] == PROTOCOL_EMOTION_INDEX, (
                    f"shared emotionMap:\n{mao['emotionMap']}\n!=\n{PROTOCOL_EMOTION_INDEX}"
                )
            else:
                pytest.skip("shared/model_dict.json 结构中未找到 niziiro_mao.emotionMap")
        elif isinstance(data, list):
            mao = next((m for m in data if m.get("name") == "niziiro_mao"), None)
            if mao:
                assert mao["emotionMap"] == PROTOCOL_EMOTION_INDEX
            else:
                pytest.skip("shared/model_dict.json 列表中未找到 niziiro_mao")

    def test_protocol_8_keys_ordered(self):
        """PROTOCOL_EMOTION_KEYS 元组顺序符合裁决1索引映射。"""
        assert PROTOCOL_EMOTION_KEYS == (
            "neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"
        )
        assert PROTOCOL_EMOTION_INDEX["neutral"] == 0
        assert PROTOCOL_EMOTION_INDEX["joy"] == 5
        assert PROTOCOL_EMOTION_INDEX["surprise"] == 7


ADAPTER_PATH = os.path.join(
    _REPO_ROOT, "shared", "model-adapter", "niziiro_mao.adapter.json"
)
CDI3_PATH = os.path.join(
    _REPO_ROOT,
    "Live2Dai-pc",
    "open-llm-vtuber",
    "live2d-models",
    "niziiro_mao",
    "runtime",
    "niziiro_mao.cdi3.json",
)

_ADAPTER_MOTION_ALIAS_KEYS = frozenset(
    {"idle", "mtn_02", "mtn_03", "mtn_04", "wave", "special_02", "special_03"}
)
_ADAPTER_VOWEL_KEYS = frozenset({"A", "I", "U", "E", "O", "SIL"})
_ADAPTER_EMOTION_KEYS = (
    "neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"
)


def _load_adapter_json():
    with open(ADAPTER_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def _load_cdi3_parameter_ids():
    with open(CDI3_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)
    return {par["Id"] for par in data["Parameters"]}


class TestAdapterLoad:
    """T1.3 — _load_model_adapter() 加载断言（contract-v3-adapter A4/A5/A9）。

    RED 目标：实现尚读硬编码、未接 adapter。以下依赖 `_load_model_adapter()`
    的用例预期 FAIL；数据不变式用例（emotion/motion 集合对拍）当下即 PASS，
    作为实现后不得破坏的守卫。
    """

    @classmethod
    def setup_class(cls):
        cls.adapter = _load_adapter_json()

    def _loader(self):
        import open_llm_vtuber.live2d_model as m
        return getattr(m, "_load_model_adapter", None)

    def test_load_model_adapter_symbol_exists(self):
        """A4 — 加载函数存在（RED：实现未新增 _load_model_adapter）。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（当前实现缺此函数 → RED）"

    def test_load_returns_dict(self):
        """A4 — 返回 dict 而非 None（RED）。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（RED）"
        result = loader()
        assert isinstance(result, dict), "返回 dict"

    def test_motion_aliases_keys_7(self):
        """A4 — motionAliases 键集 = 7 键（来自 adapter，非旧硬编码）。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（RED）"
        keys = set(loader()["motionAliases"].keys())
        assert keys == set(_ADAPTER_MOTION_ALIAS_KEYS), (
            f"motionAliases 键集应 {sorted(_ADAPTER_MOTION_ALIAS_KEYS)}，实际 {sorted(keys)}"
        )

    def test_vowel_param_map_keys_6(self):
        """A4 — vowelParamMap 键集 = {A,I,U,E,O,SIL}。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（RED）"
        keys = set(loader()["vowelParamMap"].keys())
        assert keys == set(_ADAPTER_VOWEL_KEYS), (
            f"vowelParamMap 键集应 {sorted(_ADAPTER_VOWEL_KEYS)}，实际 {sorted(keys)}"
        )

    def test_emotion_index_8_keys_unique_values(self):
        """A4 — emotionIndex 8 key、值 0-7 唯一。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（RED）"
        emo = loader()["emotionIndex"]
        assert tuple(emo.keys()) == _ADAPTER_EMOTION_KEYS
        assert set(emo.values()) == set(range(8)), "emotionIndex 值 0-7 唯一"

    def test_load_failure_returns_none(self):
        """A5 — adapter 路径错误/JSON 损坏 → 返回 None（调用方回退硬编码）。"""
        loader = self._loader()
        assert loader is not None, "_load_model_adapter 存在（RED）"
        assert (
            loader("shared/model-adapter/does_not_exist.json") is None
        ), "缺失路径应返回 None（回退门）"

    def test_motion_label_aliases_derived_from_adapter(self):
        """motionLabel 校验集来自 adapter 键（不再独立硬编码；数据不变式守卫）。"""
        from open_llm_vtuber.live2d_model import _MOTION_LABEL_ALIASES
        assert set(_MOTION_LABEL_ALIASES) == set(self.adapter["motionAliases"].keys()), (
            "_MOTION_LABEL_ALIASES 应等于 adapter.motionAliases 键集"
        )

    def test_protocol_emotion_index_matches_adapter(self):
        """数据不变式：PROTOCOL_EMOTION_INDEX == adapter.emotionIndex（A8 方向）。"""
        assert PROTOCOL_EMOTION_INDEX == self.adapter["emotionIndex"], (
            "PROTOCOL_EMOTION_INDEX 与 adapter.emotionIndex 一致"
        )


class TestAdapterVisemeWhitelist:
    """T1.3 — 前端口型参数数据断言（contract-v3-fixes §5.3 V1/V2/V4，数据层可测部分）。

    注：仅针对 adapter 数据 + cdi3 白名单做机械可校验断言（非前端 hook 运行时）。
    前端 applyParamsNow 运行时行为（V3）需 jsdom/TS 基建，标记 [UNSURE]，见工作摘要。
    """

    @classmethod
    def setup_class(cls):
        cls.adapter = _load_adapter_json()
        cls.whitelist = _load_cdi3_parameter_ids()

    def test_vowel_param_names_subset_of_cdi3(self):
        """V1/C28 — vowelParamMap 全部参数名 ⊆ cdi3 白名单。"""
        for label, params in self.adapter["vowelParamMap"].items():
            for pname in params:
                assert pname in self.whitelist, (
                    f"vowelParamMap[{label}][{pname}] 不在 cdi3 白名单"
                )

    def test_cdi3_has_mouth_up_down(self):
        """前置：cdi3 含 ParamMouthUp/ParamMouthDown（T1.1 已确认）。"""
        assert "ParamMouthUp" in self.whitelist
        assert "ParamMouthDown" in self.whitelist

    def test_mouth_up_down_present_for_each_vowel(self):
        """V2 — cdi3 含 MouthUp/Down 时：adapter.vowelParamMap 每元音追加
        ParamMouthUp/Down（当前 adapter 未追加 → RED）。"""
        for label in ("A", "I", "U", "E", "O", "SIL"):
            params = self.adapter["vowelParamMap"].get(label, {})
            assert "ParamMouthUp" in params, (
                f"vowelParamMap[{label}] 缺 ParamMouthUp（cdi3 含此参数 → 应追加，RED）"
            )
            assert "ParamMouthDown" in params, (
                f"vowelParamMap[{label}] 缺 ParamMouthDown（→ RED）"
            )

    def test_mouth_up_down_a_matches_raw(self):
        """V2/数值 — A 元音 ParamMouthUp/Down == mo(A)（cdi3 含此参数时）。"""
        if "ParamMouthUp" not in self.whitelist:
            pytest.skip("cdi3 不含 ParamMouthUp")
        mo_a = self.adapter["vowelRawMap"]["A"][0]
        params = self.adapter["vowelParamMap"].get("A", {})
        assert params.get("ParamMouthUp") == pytest.approx(mo_a, abs=1e-6), (
            "A.ParamMouthUp 应等于 mo(A)（当前缺失 → RED）"
        )
        assert params.get("ParamMouthDown") == pytest.approx(mo_a, abs=1e-6)

    def test_sil_mouth_and_all_params_zero(self):
        """V2/V4 — SIL 行 MouthUp/Down 及全部参数 0（当前缺 MouthUp/Down 键 → 视实现追加状态断言）。"""
        params = self.adapter["vowelParamMap"]["SIL"]
        for v in params.values():
            assert v == 0.0, f"SIL 参数应全 0：{params}"
        # cdi3 含 MouthUp/Down 时，SIL 的 MouthUp/Down 应归 0（若键已追加）
        for pname in ("ParamMouthUp", "ParamMouthDown"):
            if pname in params:
                assert params[pname] == 0.0, f"SIL.{pname} 应归 0（当前值 {params[pname]}）"

    def test_adapter_vowel_param_equals_contract_baseline(self):
        """A7/U — 五元音参数与契约基线精确一致（0.56 修正值）。"""
        vpm = self.adapter["vowelParamMap"]
        assert vpm["U"]["ParamU"] == pytest.approx(0.56, abs=1e-6), "U.ParamU=0.56（D4 修正）"
        assert vpm["O"]["ParamU"] == pytest.approx(0.21, abs=1e-6)
        assert vpm["E"]["ParamA"] == pytest.approx(0.15, abs=1e-6)


class TestPromptContent:
    """P19: prompt 文件内容断言。"""

    @classmethod
    def setup_class(cls):
        cls.prompt_path = os.path.join(
            _REPO_ROOT,
            "Live2Dai-pc",
            "open-llm-vtuber",
            "prompts",
            "utils",
            "live2d_expression_prompt.txt",
        )
        if not os.path.isfile(cls.prompt_path):
            pytest.skip(f"prompt 文件不存在: {cls.prompt_path}")
        with open(cls.prompt_path, "r", encoding="utf-8") as f:
            cls.content = f.read()

    def test_contains_expression_mix_schema(self):
        """含 expressionMix JSON schema 示例。"""
        assert '{"expressionMix"' in self.content, "prompt 缺少 expressionMix JSON 示例"

    def test_contains_placeholder(self):
        """含 [<insert_emomap_keys>] 占位符。"""
        assert "[<insert_emomap_keys>]" in self.content, "prompt 缺少 emomap_keys 占位符"

    def test_contains_chinese_example(self):
        """含中文示例（fence + 正文）。"""
        assert (
            "今天过得怎么样？" in self.content
            or "诶？！真的假的？！" in self.content
        ), "prompt 缺少中文示例"

    def test_contains_english_example(self):
        """含英文示例（裸 JSON）。"""
        assert "I'm so happy" in self.content, "prompt 缺少英文示例"

    def test_contains_single_line_constraint(self):
        """含单行 JSON 约束字样。"""
        assert (
            "ONE single line" in self.content
            or "ONE line" in self.content
            or "单行" in self.content
        ), "prompt 缺少单行 JSON 约束"

    def test_contains_not_code_block_wording(self):
        """含 'NOT a code block' 措辞（与 persona "不要提供代码块"解耦）。"""
        assert (
            "NOT a code block" in self.content
            or "not a code block" in self.content
            or "system protocol" in self.content
        ), "prompt 缺少 NOT a code block 声明"


# ---------------------------------------------------------------------------
# T4.1: extract_motion_keyword 单元测试 + 双端词表对拍（K5/K6/K7，契约 v3-features §1.4）
#
# RED 目标：实现未新增 extract_motion_keyword / MOTION_KEYWORDS → 以下断言预期 FAIL。
# ❌ 未读 src/ 实现内部。词表中文 6 词基准来自契约 §1.1（与 Android KeywordMotionTable 对齐）。
# ---------------------------------------------------------------------------

# 终裁 K7（用户定案）：PC 6 中文词与 Android 完全重合（6/6）——词表 = 挥手/招招手/打招呼→wave、
# 再见/拜拜/掰掰→special_02（无"你好"、无英文词，双端一致优先）。
# ⚠️ 覆盖旧版 §1.1 的"你好"→wave 与 hello/hi 等英文词（kugutu 蓝本 [UNSURE-FT1] 已被终裁否决）。
_PC_CHINESE_KEYWORDS = frozenset(
    {"挥手", "招招手", "打招呼", "再见", "拜拜", "掰掰"}
)
# Android KeywordMotionTable.KEYWORDS（契约 §1.1 实读）== PC 中文词表（终裁：完全重合 6/6，无你好、无英文词）
_ANDROID_CHINESE_KEYWORDS = frozenset(
    {"挥手", "招招手", "打招呼", "再见", "拜拜", "掰掰"}
)


def _motion_keyword_fn():
    import open_llm_vtuber.live2d_model as m
    fn = getattr(m, "extract_motion_keyword", None)
    assert fn is not None, (
        "extract_motion_keyword 存在（当前实现未新增 → RED）"
    )
    return fn


def _motion_keywords_dict():
    import open_llm_vtuber.live2d_model as m
    d = getattr(m, "MOTION_KEYWORDS", None)
    assert d is not None, (
        "MOTION_KEYWORDS 词表存在（当前实现未新增 → RED）"
    )
    assert isinstance(d, dict), "MOTION_KEYWORDS 应为 dict[str, str]"
    return d


class TestExtractMotionKeyword:
    """K5/K6 + 边界 — extract_motion_keyword 签名/返回/词表边界（契约 §1.1-1.4）。"""

    def test_signature_returns_str_or_none(self):
        """返回 str（别名）或 None；无命中/空白 → None。"""
        fn = _motion_keyword_fn()
        assert fn("挥手") == "wave"
        assert isinstance(fn("挥手"), str)
        assert fn("今天天气不错") is None

    def test_k5_english_words_not_in_wordlist(self):
        """K5: 英文词不在词表（终裁：无英文词）——"HELLO"/"hello" → None（反例）。"""
        fn = _motion_keyword_fn()
        assert fn("HELLO") is None, (
            f"终裁锁无英文词：HELLO 应返回 None，实际 {fn('HELLO')!r}（词表残留英文词 → RED）"
        )
        assert fn("hello") is None, "hello 应 None（无英文词）"
        assert fn("bye") is None, "bye 应 None（无英文词）"

    def test_k6_same_alias_earliest_hit(self):
        """K6: "拜拜！再见！" → "special_02" 一次（同别名归一）。"""
        fn = _motion_keyword_fn()
        assert fn("拜拜！再见！") == "special_02", (
            f"拜拜/再见→special_02，实际 {fn('拜拜！再见！')!r}（RED）"
        )

    def test_chinese_alias_mapping(self):
        """中文 6 词 → 别名映射（wave/special_02 两组，契约 §1.1）。"""
        fn = _motion_keyword_fn()
        for kw in ("挥手", "招招手", "打招呼"):
            assert fn(f"你{kw}！") == "wave", f"{kw}→wave（RED）"
        for kw in ("再见", "拜拜", "掰掰"):
            assert fn(f"你{kw}！") == "special_02", f"{kw}→special_02（RED）"

    def test_k7_pc_chinese_wordlist_equals_android(self):
        """K7: PC 中文键集 == Android 完全重合（6/6）——无"你好"、无英文词。"""
        d = _motion_keywords_dict()
        pc_chinese = {k for k in d.keys() if isinstance(k, str) and any("\u4e00" <= c <= "\u9fff" for c in k)}
        assert pc_chinese == _ANDROID_CHINESE_KEYWORDS, (
            f"PC 中文 6 词应 == Android 6/6（无你好无英文词），实际差异：\n"
            f"  多出 {sorted(pc_chinese - _ANDROID_CHINESE_KEYWORDS)}\n"
            f"  缺失 {sorted(_ANDROID_CHINESE_KEYWORDS - pc_chinese)}\n"
            f"  （若多出含'你好'或英文词——终裁为 6/6，需删除）"
        )
        # 无英文词：英文键若存在则不得映射到 wave/special_02（反例由 test_k5_english_words_not_in_wordlist 覆盖）
        assert "你好" not in _ANDROID_CHINESE_KEYWORDS, "终裁：Android 词表不含你好"

    def test_k7_wave_special02_grouping(self):
        """K7 补充: 中文分组正确（wave 组: 挥手/招招手/打招呼 3 词；special_02 组: 再见/拜拜/掰掰 3 词）。"""
        d = _motion_keywords_dict()
        wave_kw = {k for k, v in d.items() if v == "wave" and any("\u4e00" <= c <= "\u9fff" for c in k)}
        sp_kw = {k for k, v in d.items() if v == "special_02" and any("\u4e00" <= c <= "\u9fff" for c in k)}
        assert wave_kw == {"挥手", "招招手", "打招呼"}, (
            f"wave 组应为 3 中文词（无你好），实际 {sorted(wave_kw)}"
        )
        assert sp_kw == {"再见", "拜拜", "掰掰"}, (
            f"special_02 组应为 3 中文词，实际 {sorted(sp_kw)}"
        )

    def test_k7_android_6_words_resolvable(self):
        """K7 对拍: Android 6 词在 PC 侧每个都能解析到 wave/special_02（PC==Android，一一对应）。"""
        fn = _motion_keyword_fn()
        for kw in _ANDROID_CHINESE_KEYWORDS:
            r = fn(kw)
            assert r in ("wave", "special_02"), f"Android 词 {kw!r} 应解析到别名，实际 {r!r}"

    def test_blank_text_returns_none(self):
        """空/空白文本返回 None。"""
        fn = _motion_keyword_fn()
        assert fn("") is None
        assert fn("   ") is None
