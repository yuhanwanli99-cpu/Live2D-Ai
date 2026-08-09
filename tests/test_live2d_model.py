"""PC 端 Live2dModel 回归 + 一致性测试（M3 核心）。

覆盖:
- P14: extract_emotion 并存回归（关键词路径行为不变）
- P17: 裁决1 — model_dict.json mao_pro emotionMap 8 索引一致（共享+运行时两处）
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


def _make_model(name="mao_pro"):
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
        cls.model = _make_model("mao_pro")

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
        """PC 运行时 model_dict.json mao_pro emotionMap == PROTOCOL_EMOTION_INDEX。"""
        runtime_path = os.path.join(
            _REPO_ROOT, "Live2Dai-pc", "open-llm-vtuber", "model_dict.json"
        )
        with open(runtime_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        mao = next(m for m in data if m["name"] == "mao_pro")
        emo_map = mao["emotionMap"]

        # 逐 key 断言（字符串 key；值 = 8 索引）
        assert emo_map == PROTOCOL_EMOTION_INDEX, (
            f"model_dict.json emotionMap:\n{emo_map}\n!=\nPROTOCOL_EMOTION_INDEX:\n{PROTOCOL_EMOTION_INDEX}"
        )

    def test_shared_model_dict_8_index(self):
        """shared/model_dict.json mao_pro emotionMap == PROTOCOL_EMOTION_INDEX。"""
        shared_path = os.path.join(_REPO_ROOT, "shared", "model_dict.json")
        if not os.path.isfile(shared_path):
            pytest.skip("shared/model_dict.json 不存在")
        with open(shared_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        # shared/model_dict.json 是 dict 结构（key 为模型名），不同于运行时的 list
        if isinstance(data, dict):
            # 结构: {"mao_pro": {...}, ...}
            mao = data.get("mao_pro") or data.get("mao_pro", {})
            if isinstance(mao, dict) and "emotionMap" in mao:
                assert mao["emotionMap"] == PROTOCOL_EMOTION_INDEX, (
                    f"shared emotionMap:\n{mao['emotionMap']}\n!=\n{PROTOCOL_EMOTION_INDEX}"
                )
            else:
                pytest.skip("shared/model_dict.json 结构中未找到 mao_pro.emotionMap")
        elif isinstance(data, list):
            mao = next((m for m in data if m.get("name") == "mao_pro"), None)
            if mao:
                assert mao["emotionMap"] == PROTOCOL_EMOTION_INDEX
            else:
                pytest.skip("shared/model_dict.json 列表中未找到 mao_pro")

    def test_protocol_8_keys_ordered(self):
        """PROTOCOL_EMOTION_KEYS 元组顺序符合裁决1索引映射。"""
        assert PROTOCOL_EMOTION_KEYS == (
            "neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"
        )
        assert PROTOCOL_EMOTION_INDEX["neutral"] == 0
        assert PROTOCOL_EMOTION_INDEX["joy"] == 5
        assert PROTOCOL_EMOTION_INDEX["surprise"] == 7


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
            "主人回来啦喵~！" in self.content
            or "小喵好想你" in self.content
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
