"""shared 层 — RED 测试（PC 聚焦最小集：S1-S5）

依据:
  - contract-shared.md §S1-S5
  - architecture-shared.md（persona.yaml 结构 / emotion-protocol H+I / registry / bai.adapter）

目标态路径（shared/ 单源真理）:
  - shared/persona.yaml                 → 「凛 (Lin)」（已达成 → PASS）
  - shared/emotion-protocol.md          → 新增 ## H（emotionParamMap）与 ## I（mood）段（现状无 → RED）
  - shared/model_registry.json          → 新增 bai default=true，niziiro_mao default=false（现状无 bai → RED）
  - shared/model-adapter/bai.adapter.json（现状不存在 → RED）

说明：本文件断言 shared/ 下**数据/文档文件**（非 src 实现），不触碰 src 实现代码。
"""
import json
import re
from pathlib import Path

import pytest

# shared/ 根目录：tests/ → open-llm-vtuber/ → Live2Dai-pc/ → Live2Dai/ → shared
_SHARED_ROOT = Path(__file__).resolve().parents[3] / "shared"

YAML_AVAILABLE = True
try:
    import yaml
except ImportError:  # 无 PyYAML 时 S1-04/S2 结构断言降级为文本说明，但契约要求 YAML 有效
    YAML_AVAILABLE = False

# 8 个合法 emotion key（contract-shared C1）
VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}
# emotion-protocol.md H 段白-免费版参数组的参数名（architecture S4-04 -- 标准 Live2D 参数）
ADAPTER_PARAM_NAMES = {
    "ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
    "ParamMouthForm", "ParamMouthOpenY", "ParamBreath",
    "ParamEyeBallX", "ParamEyeBallY",
}
# fade 场景名（contract S2-04）
FADE_SCENES = {"emotionSwitch", "emotionToNeutral", "microExpression", "lipSync"}


# ═══════════════════════════════════════════════════════════════
# S1 · persona.yaml「凛」
# ═══════════════════════════════════════════════════════════════

class TestPersonaRin:
    _PATH = _SHARED_ROOT / "persona.yaml"

    def _load(self) -> dict:
        if not self._PATH.exists():
            pytest.fail(f"{self._PATH} 不存在（RED）")
        with open(self._PATH, encoding="utf-8") as f:
            return yaml.safe_load(f)

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s101_name_is_rin(self):
        # S1-01 · name 字段精确匹配 "凛 (Lin)"
        data = self._load()
        assert data.get("name") == "凛 (Lin)", f"name={data.get('name')!r}，期望 '凛 (Lin)'"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s102_system_prompt_keywords(self):
        # S1-02 · system_prompt 含 "天才少女"、"傲娇"、"18岁"
        data = self._load()
        prompt = data.get("system_prompt", "")
        assert "天才少女" in prompt, "system_prompt 缺关键词 '天才少女'"
        assert "傲娇" in prompt, "system_prompt 缺关键词 '傲娇'"
        assert ("18岁" in prompt) or ("18 岁" in prompt), "system_prompt 缺 '18岁'"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s103_system_prompt_contains_majority_emotion_keys(self):
        # S1-03 · system_prompt 含 8 个 emotion key 至少 6 个
        data = self._load()
        prompt = data.get("system_prompt", "")
        present = {k for k in VALID_EMOTIONS if k in prompt}
        assert len(present) >= 6, \
            f"system_prompt 仅含 {len(present)} 个 emotion key（要求 ≥6）：{present}"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s104_yaml_is_valid(self):
        # S1-04 · YAML 语法有效（safe_load 不抛 YAMLError）
        data = self._load()
        assert isinstance(data, dict)


# ═══════════════════════════════════════════════════════════════
# S2 · emotion-protocol.md H 段（emotionParamMap 8×3 + fade）
# ═══════════════════════════════════════════════════════════════

def _extract_section(md_text: str, title: str) -> str:
    """提取 `## X` 起始到下一个 `## ` 前的段文本。"""
    lines = md_text.splitlines()
    start = None
    for i, ln in enumerate(lines):
        if ln.strip().startswith("## ") and title in ln:
            start = i
            break
    if start is None:
        return ""
    out = []
    for ln in lines[start + 1:]:
        if ln.strip().startswith("## "):
            break
        out.append(ln)
    return "\n".join(out)


class TestEmotionProtocolSectionH:
    _PATH = _SHARED_ROOT / "emotion-protocol.md"

    def _load_text(self) -> str:
        if not self._PATH.exists():
            pytest.fail(f"{self._PATH} 不存在")
        return self._PATH.read_text(encoding="utf-8")

    def test_s201_h_section_exists(self):
        # S2 前置 · H 段标记存在（现状无 → RED）
        text = self._load_text()
        assert re.search(r"^#{2,3}\s*H\b", text, re.M), "emotion-protocol.md 缺 ## H 段"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s201_emotion_param_map_has_8_keys(self):
        # S2-01 · H 段 emotion_param_map 含 8 个顶级 key（集合相等）
        h = _extract_section(self._load_text(), "H")
        emap = _parse_emotion_param_map(h)
        assert set(emap.keys()) == VALID_EMOTIONS, \
            f"emotion_param_map key={set(emap.keys())}，期望 {VALID_EMOTIONS}"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s202_each_emotion_has_3_strengths_with_4_params(self):
        # S2-02 · 每个 emotion 含 _1/_2/_3，每子 key 含 ≥4 参数
        h = _extract_section(self._load_text(), "H")
        emap = _parse_emotion_param_map(h)
        for emotion, strengths in emap.items():
            for level in ("_1", "_2", "_3"):
                assert level in strengths, f"{emotion}.{level} 缺失"
                params = strengths[level]
                assert isinstance(params, dict), f"{emotion}.{level} 应为 dict"
                assert len(params) >= 4, f"{emotion}.{level} 参数数 {len(params)} < 4"

    @pytest.mark.skipif(not YAML_AVAILABLE, reason="PyYAML 不可用")
    def test_s203_all_param_values_in_range(self):
        # S2-03 · 所有参数值 ∈ [-1.0, 2.0]
        h = _extract_section(self._load_text(), "H")
        emap = _parse_emotion_param_map(h)
        for emotion, strengths in emap.items():
            for level, params in strengths.items():
                for name, value in params.items():
                    assert -1.0 <= value <= 2.0, \
                        f"{emotion}.{level}.{name}={value} 越界 [-1,2]"

    def test_s204_fade_spec_present_and_scenes_in_range(self):
        # S2-04 · fade 规范存在且含 4 种场景，值 ∈ [20,1000] ms
        h = _extract_section(self._load_text(), "H")
        found = set()
        for scene in FADE_SCENES:
            m = re.search(rf"{scene}\s*[|:]\s*(\d+)", h)
            if not m:
                # 兼容"| emotionSwitch | 300 |"表格或"emotionSwitch: 300"
                m = re.search(rf"([^\n]*{scene}[^\n]*)", h)
                nums = re.findall(r"\b(\d+)\s*ms\b", m.group(1)) if m else []
                if nums:
                    found.add(scene)
                    for n in nums:
                        assert 20 <= int(n) <= 1000, f"{scene} fade 值 {n} 越界 [20,1000]"
                continue
            val = int(m.group(1))
            assert 20 <= val <= 1000, f"{scene} fade={val} 越界 [20,1000]"
            found.add(scene)
        missing = FADE_SCENES - found
        assert not missing, f"fade 缺场景：{missing}"

    def test_s205_referenced_param_names_are_live2d_params(self):
        # S2-05 · 引用的参数名是白-免费版可用参数子集（architecture S4-04 标准清单）
        h = _extract_section(self._load_text(), "H")
        if not YAML_AVAILABLE:
            emap = {}
        else:
            emap = _parse_emotion_param_map(h)
        for emotion, strengths in emap.items():
            for level, params in strengths.items():
                for name in params:
                    assert name in ADAPTER_PARAM_NAMES, \
                        f"参数 {name} 不在 {ADAPTER_PARAM_NAMES}"


# ═══════════════════════════════════════════════════════════════
# S3 · emotion-protocol.md I 段（mood 协议）
# ═══════════════════════════════════════════════════════════════

class TestMoodProtocolSectionI:
    _PATH = _SHARED_ROOT / "emotion-protocol.md"

    def _load_text(self) -> str:
        if not self._PATH.exists():
            pytest.fail(f"{self._PATH} 不存在")
        return self._PATH.read_text(encoding="utf-8")

    def test_s301_mood_section_exists(self):
        text = self._load_text()
        assert re.search(r"^#{2,3}\s*I\b", text, re.M), "emotion-protocol.md 缺 ## I 段"

    def test_s301_mood_range_defined(self):
        # S3-01 · I 段声明 mood 范围 [-100, +100]
        i = _extract_section(self._load_text(), "I")
        assert ("-100" in i and "+100" in i) or ("100" in i), "I 段缺 mood 范围声明"

    def test_s302_at_least_8_scoring_events_with_signed_delta(self):
        # S3-02 · 打分事件表 ≥8 个，且 Δmood 正负有别
        i = _extract_section(self._load_text(), "I")
        # 计数事件行：形如 "| 事件 | Δmood |" 的表格行，或 "- 事件 +N / -N"
        event_rows = re.findall(
            r"(?m)^.*?(?:\Δmood|[+\-]\d+).*$", i
        )
        # 过滤表头/分隔行，保留实际打分行
        scoring = [r for r in event_rows if not r.strip().startswith("|---")]
        assert len(scoring) >= 8, f"I 段打分事件行 < 8（实际 {len(scoring)}）"
        # 正负有别：至少同时出现正偏移与负偏移
        has_plus = bool(re.search(r"[+]\d", i))
        has_minus = bool(re.search(r"-\s?\d", i))
        assert has_plus and has_minus, "I 段打分应同时含正偏移和负偏移"

    def test_s303_decay_defined(self):
        # S3-03 · 衰减速率 -2/分钟 + 下限 -40
        i = _extract_section(self._load_text(), "I")
        assert "-40" in i, "I 段缺衰减下限 -40"
        assert re.search(r"-?\s*2", i), "I 段缺衰减速率（-2/分钟）"


# ═══════════════════════════════════════════════════════════════
# S4 · bai.adapter.json（新建）
# ═══════════════════════════════════════════════════════════════

class TestBaiAdapter:
    _PATH = _SHARED_ROOT / "model-adapter" / "bai.adapter.json"

    def _load(self) -> dict:
        if not self._PATH.exists():
            pytest.fail(f"{self._PATH} 不存在（RED：文件待新建）")
        with open(self._PATH, encoding="utf-8") as f:
            return json.load(f)

    def test_s401_json_is_valid(self):
        # S4-01 · JSON 语法有效
        data = self._load()
        assert isinstance(data, dict)

    def test_s402_model_id_is_bai(self):
        # S4-02 · modelId = "bai"
        data = self._load()
        assert data.get("modelId") == "bai", f"modelId={data.get('modelId')!r}"

    def test_s403_emotion_param_map_has_24_entries(self):
        # S4-03 · emotionParamMap 含 8 emotion × 3 strength
        emap = self._load().get("emotionParamMap")
        assert emap is not None, "缺 emotionParamMap"
        assert set(emap.keys()) == VALID_EMOTIONS
        for emotion, strengths in emap.items():
            assert set(strengths.keys()) == {"_1", "_2", "_3"}, \
                f"{emotion} 缺 _1/_2/_3"
        total = sum(len(v) for v in emap.values())
        assert total == 24, f"emotionParamMap 应有 24 子条目，实际 {total}"

    def test_s404_param_names_subset_of_live2d_params(self):
        # S4-04 · emotionParamMap 参数名 ⊆ 标准参数清单
        emap = self._load().get("emotionParamMap", {})
        for emotion, strengths in emap.items():
            for level, params in strengths.items():
                for name in params:
                    assert name in ADAPTER_PARAM_NAMES, \
                        f"{emotion}.{level}.{name} 不在 {ADAPTER_PARAM_NAMES}"

    def test_s405_vowel_param_map_schema(self):
        # S4-05 · vowelParamMap 含 A/I/U/E/O/SIL，每个含 ParamMouthOpenY + ParamMouthForm
        vmap = self._load().get("vowelParamMap")
        assert vmap is not None, "缺 vowelParamMap"
        assert set(vmap.keys()) == {"A", "I", "U", "E", "O", "SIL"}, \
            f"vowelParamMap keys={set(vmap.keys())}"
        for vowel, params in vmap.items():
            assert "ParamMouthOpenY" in params, f"{vowel} 缺 ParamMouthOpenY"
            assert "ParamMouthForm" in params, f"{vowel} 缺 ParamMouthForm"

    def test_s406_motion_aliases_at_most_idle(self):
        # S4-06 · motionAliases ≤ 1 条（仅 idle）
        aliases = self._load().get("motionAliases", {})
        assert isinstance(aliases, dict)
        assert len(aliases) <= 1, f"motionAliases 应 ≤1，实际 {list(aliases.keys())}"
        if aliases:
            assert "idle" in aliases, "若含 motion，应仅 'idle'"


# ═══════════════════════════════════════════════════════════════
# S5 · model_registry.json
# ═══════════════════════════════════════════════════════════════

class TestModelRegistry:
    _PATH = _SHARED_ROOT / "model_registry.json"

    def _load(self) -> dict:
        if not self._PATH.exists():
            pytest.fail(f"{self._PATH} 不存在")
        with open(self._PATH, encoding="utf-8") as f:
            return json.load(f)

    def _model_by_id(self, data: dict, model_id: str):
        for m in data.get("models", []):
            if m.get("id") == model_id:
                return m
        return None

    def test_s501_bai_in_models(self):
        # S5-01 · models[] 含 id="bai"
        data = self._load()
        assert self._model_by_id(data, "bai") is not None, "models[] 缺 'bai' 条目"

    def test_s502_bai_is_default(self):
        # S5-02 · bai.default = true
        bai = self._model_by_id(self._load(), "bai")
        assert bai is not None and bai.get("default") is True, "bai.default 应为 true"

    def test_s503_niziiro_mao_not_default(self):
        # S5-03 · niziiro_mao.default = false（非 true 或缺失）
        mao = self._model_by_id(self._load(), "niziiro_mao")
        if mao is not None:
            assert mao.get("default") is False, "niziiro_mao.default 应非 true"


# ── 辅助：从 H 段文本解析 emotion_param_map ──

def _parse_emotion_param_map(h_section: str) -> dict:
    """从 H 段文本提取 emotion_param_map 的 YAML dict。

    兼容两种落位：H 段正文为纯 YAML 块；或 emotion_param_map: 起头的缩进 YAML。
    解析失败 → 返回 {} → 上层断言 key 集合失败（RED）。
    """
    idx = h_section.find("emotion_param_map")
    if idx < 0:
        return {}
    # 从 emotion_param_map 前的行开始，到 fade 段/下一个顶层标题截断
    lines = h_section[idx:].splitlines()
    # 跳过 "emotion_param_map:" 自身及其所在行后，取缩进子块
    body = [lines[0]]
    indent = None
    for ln in lines[1:]:
        if indent is None:
            stripped = ln.lstrip()
            if not stripped or stripped.startswith("#") or stripped.startswith("|"):
                break
            indent = len(ln) - len(stripped)
        cur_indent = len(ln) - len(ln.lstrip())
        if stripped_start(ln):
            break
        if cur_indent <= 0 and ln.strip():
            break
        body.append(ln)
    text = "\n".join(body)
    try:
        data = yaml.safe_load(text) or {}
    except Exception:
        return {}
    # 兼容：YAML 顶层为 {emotion_param_map: {...}} 时解嵌套，直接返回表情映射表
    if isinstance(data, dict) and "emotion_param_map" in data:
        return data["emotion_param_map"]
    return data


def stripped_start(ln: str) -> bool:
    s = ln.strip()
    # 遇到 fade 段标题或新的顶层标题终止
    if s.startswith("## ") or s.startswith("### "):
        return True
    if s.startswith("fade") or "fadeMs" in s:
        return True
    return False
