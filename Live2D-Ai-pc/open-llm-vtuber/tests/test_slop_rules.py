# -*- coding: utf-8 -*-
"""SLOP 台词治理测试（PLAN-V3 §P2 台词质量治理）。

覆盖四块：
  1) 规则加载边界：缺失/坏 JSON/坏条目跳过、env 路径覆盖；
  2) 规则应用：命中移除、自定义替换、行首孤立标点清理；
  3) tts_filter 集成：规则只影响 TTS 文本这一既有管线语义被真正接通；
  4) 离线挖掘器：合成 .jsonl / .json 历史上的候选统计与 role 过滤。
"""
import importlib.util
import json
import re
from pathlib import Path

import pytest
from open_llm_vtuber.utils import slop_rules
from open_llm_vtuber.utils.tts_preprocessor import tts_filter


def _load_miner():
    """从仓库根 scripts/ 直接按路径加载 slop_miner（非包成员）。"""
    here = Path(__file__).resolve()
    for cand in (
        here.parents[3] / "Live2D-Ai" / "scripts",
        here.parents[2] / "scripts",
        here.parents[3] / "scripts",
    ):
        if (cand / "slop_miner.py").is_file():
            spec = importlib.util.spec_from_file_location("slop_miner", cand / "slop_miner.py")
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            return module
    raise FileNotFoundError("slop_miner.py not found")


@pytest.fixture(autouse=True)
def _clean_runtime_cache():
    """每个用例前后清空运行时规则缓存，避免污染其他测试的 tts_filter 行为。"""
    slop_rules.reset_runtime_rules()
    yield
    slop_rules.reset_runtime_rules()


def _write_rules(tmp_path, rules):
    p = tmp_path / "rules.json"
    p.write_text(json.dumps({"version": 1, "rules": rules}, ensure_ascii=False), encoding="utf-8")
    return p


# ═══════════════ 1) 规则加载 ═══════════════


class TestLoadSlopRules:
    def test_valid_file_compiles(self, tmp_path):
        p = _write_rules(
            tmp_path,
            [
                {"pattern": "^总而言之[，,]?", "note": "总结腔"},
                {"pattern": "值得注意的是", "replacement": "提醒一句，", "note": "插入语"},
            ],
        )
        rules = slop_rules.load_slop_rules(p)
        assert len(rules) == 2
        assert rules[0].replacement == ""
        assert rules[1].replacement == "提醒一句，"

    def test_missing_file_returns_empty(self, tmp_path):
        assert slop_rules.load_slop_rules(tmp_path / "nope.json") == ()

    def test_invalid_json_returns_empty(self, tmp_path):
        p = tmp_path / "broken.json"
        p.write_text("{not json", encoding="utf-8")
        assert slop_rules.load_slop_rules(p) == ()

    def test_bad_entries_skipped_good_kept(self, tmp_path):
        p = _write_rules(
            tmp_path,
            [
                {"replacement": "", "note": "缺 pattern"},
                {"pattern": "([unclosed", "note": "正则非法"},
                {"pattern": "众所周知[，,]?", "note": "合法"},
                "not-a-dict",
            ],
        )
        rules = slop_rules.load_slop_rules(p)
        assert len(rules) == 1
        assert rules[0].pattern.pattern == "众所周知[，,]?"

    def test_env_path_override(self, tmp_path, monkeypatch):
        env_p = _write_rules(tmp_path, [{"pattern": "BBB"}])
        monkeypatch.setenv(slop_rules.ENV_RULES_PATH, str(env_p))
        assert slop_rules.default_rules_path() == env_p
        assert all(r.pattern.pattern == "BBB" for r in slop_rules.load_slop_rules())


# ═══════════════ 2) 规则应用 ═══════════════


def _rules(*triples):
    return tuple(slop_rules.SlopRule(re.compile(p), rep, note) for p, rep, note in triples)


class TestApplySlopRules:
    def test_hit_removed_and_leading_punct_tidy(self):
        rules = _rules(("^总而言之[，,]?", "", "n"))
        assert slop_rules.apply_slop_rules("总而言之，今天天气不错。", rules) == "今天天气不错。"

    def test_custom_replacement(self):
        rules = _rules(("值得注意的是[，,:：]?", "提醒一句，", "n"))
        assert slop_rules.apply_slop_rules("值得注意的是，明天下雨。", rules) == "提醒一句，明天下雨。"

    def test_orphan_leading_comma_stripped(self):
        rules = _rules(("众所周知[，,]", "", "n"))
        # 「，众所周知，他来了」→ 移除后残留行首「，」→ 应被清理
        assert slop_rules.apply_slop_rules("，众所周知，他来了。", rules) == "他来了。"

    def test_no_rules_noop(self):
        assert slop_rules.apply_slop_rules("原文不动。", ()) == "原文不动。"


# ═══════════════ 3) tts_filter 集成 ═══════════════


class TestTtsFilterIntegration:
    @staticmethod
    def _filter(text):
        return tts_filter(
            text=text,
            remove_special_char=False,
            ignore_brackets=False,
            ignore_parentheses=False,
            ignore_asterisks=False,
            ignore_angle_brackets=False,
        )

    def test_rule_applied_through_tts_filter(self, tmp_path):
        p = _write_rules(tmp_path, [{"pattern": "^总而言之[，,]?", "replacement": ""}])
        slop_rules.reload_runtime_rules(p)
        assert self._filter("总而言之，你好呀。") == "你好呀。"

    def test_missing_default_rules_keeps_text(self, tmp_path, monkeypatch):
        monkeypatch.setenv(slop_rules.ENV_RULES_PATH, str(tmp_path / "absent.json"))
        slop_rules.reset_runtime_rules()
        assert self._filter("原样输出。") == "原样输出。"


# ═══════════════ 4) 离线挖掘器 ═══════════════


class TestSlopMiner:
    def test_mine_jsonl_and_role_filter(self, tmp_path):
        m = _load_miner()
        f = tmp_path / "hist.jsonl"
        lines = []
        for _ in range(3):
            msg = {"role": "ai", "content": "总而言之，今天天气不错。我们去公园吧。"}
            lines.append(json.dumps(msg, ensure_ascii=False))
        human = {"role": "human", "content": "总而言之，人类的话不算数。"}
        lines.append(json.dumps(human, ensure_ascii=False))
        lines.append(json.dumps({"role": "ai", "content": "好的呀。"}, ensure_ascii=False))
        newline_char = chr(10)
        f.write_text(newline_char.join(lines), encoding="utf-8")

        report = m.mine([str(f)], role="ai", min_count=3, top=10)
        assert report["scanned_messages"] == 4  # 5 条中 human 被过滤
        texts = [c["text"] for c in report["clauses"]]
        assert "今天天气不错" in texts and "我们去公园吧" in texts
        assert "人类的话不算数" not in texts
        assert report["cliche_family_hits"].get("总结腔开头") == 3

    def test_mine_json_history_array(self, tmp_path):
        m = _load_miner()
        f = tmp_path / "bai_001.json"
        data = [{"role": "metadata"}]
        for _ in range(4):
            data.append({"role": "ai", "content": "作为一个AI助手，我很高兴帮你。"})
        f.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

        report = m.mine([str(f)], role="ai", min_count=2, top=10)
        joined = json.dumps(report, ensure_ascii=False)
        assert "作为一个AI助手" in joined
        assert report["cliche_family_hits"].get("AI自指") == 4

    def test_mine_empty_inputs_reports_zero(self):
        m = _load_miner()
        report = m.mine(["/nonexistent-path/x.json"], role="ai", min_count=1)
        assert report["scanned_messages"] == 0
