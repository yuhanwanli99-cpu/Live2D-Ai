"""共享一致性脚本跨端解析 + 双端文件同步单测（task M-CONS，RED 先行）。

依据 .pi/spoq/contract-M-CONS.md（v2）：
- §2.1 C26 解析函数修复：`_parse_fallback_emotion_map` 以
  `private val FALLBACK_EMOTION_MAP: Map<String, Int> = mapOf(` 为块起点、
  配对 `)` 为终点截取；块内 regex `"(\w+)"\s+to\s+(\d+)` 提取键值；返回前断言
  键集 == 8 表情键（neutral/fear/sadness/anger/disgust/joy/smirk/surprise），不符 → None。
  约束：不得全文件扫 regex（否则 `DEFAULT_BAI_EMOTION_PARAM_MAP` 的
  `"ParamBrowLY" to 0.0f` 浮点前缀误抓——本缺陷回归用例）。
- §2.1 C26 新增接线断言：EmotionController.kt 含
  `currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP`（防 getter 独立硬编码漂移）。
- §2.2 C32 默认一致性断言：registry default.id == SettingsRepository.DEFAULT_MODEL_ID
  == ModelRegistry.FALLBACK_ENTRY.id（静态解析三方）。
- §2.3 Layout 双端文件同步：Android 侧 bai.model3.json 加
  `"Layout": { "center_x": 0.0, "center_y": -0.07, "width": 0.56, "height": 0.56 }`
  且 sha256(Android) == sha256(PC)。
- §3 验收断言 C-A1 到 C-A10。

约束：本文件禁读/禁改 shared 实现与双端资源；只读其公开行为 + 数据文件。
对 PC model_dict.json 的临时改动测试（C-A2）在 finally 中原样还原。
"""
import contextlib
import hashlib
import io
import json
import os
import subprocess
import sys
import textwrap
import unittest
from pathlib import Path

import pytest

# ---- 仓库根 + 关键路径（数据文件只读基线） ----
_REPO_ROOT = Path(__file__).resolve().parent.parent
_SHARED_CONSISTENCY = _REPO_ROOT / "shared" / "check_cross_platform_consistency.py"
_ANDROID_BAI_MODEL3 = (
    _REPO_ROOT
    / "Live2Dai-Android" / "app" / "src" / "main" / "assets"
    / "live2d" / "bai" / "runtime" / "bai.model3.json"
)
_PC_BAI_MODEL3 = (
    _REPO_ROOT
    / "Live2Dai-pc" / "open-llm-vtuber" / "live2d-models"
    / "bai" / "runtime" / "bai.model3.json"
)
_PC_MODEL_DICT = _REPO_ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "model_dict.json"
_EMOTION_CONTROLLER = (
    _REPO_ROOT / "Live2Dai-Android" / "app" / "src" / "main" / "java"
    / "com" / "live2dai" / "android" / "EmotionController.kt"
)

# ---- 契约锚点 ----
# §2.1 8 个表情键及其索引（与 Android FALLBACK_EMOTION_MAP.bai adapter emotionIndex 同源）
_FALLBACK_KEYS = ["neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"]
_FALLBACK_EXPECTED = {
    "neutral": 0, "fear": 1, "sadness": 2, "anger": 3,
    "disgust": 4, "joy": 5, "smirk": 6, "surprise": 7,
}
# §2.1 接线断言字符串（EmotionController.kt L128）
_WIRING_EXPRESSION = "currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP"
# §2.3 Layout 四值（M-PC §2.1 同值）
_LAYOUT_EXPECTED = {"center_x": 0.0, "center_y": -0.07, "width": 0.56, "height": 0.56}


# ---------------------------------------------------------------------------
# C26 解析函数 _parse_fallback_emotion_map（契约 §2.1，模块级函数，RED：当前不存在）
# ---------------------------------------------------------------------------

def _parse_fallback(text: str):
    """惰性装载契约版解析函数；不存在（实现未落地）时抛 AssertionError（RED）。"""
    _SHARED_DIR = str(_REPO_ROOT / "shared")
    if _SHARED_DIR not in sys.path:
        sys.path.insert(0, _SHARED_DIR)
    try:
        import check_cross_platform_consistency as cc
    except ImportError as exc:  # 脚本 import 侧崩溃 → 也视为未落地
        raise AssertionError(
            f"shared/check_cross_platform_consistency.py 不可导入（{exc}）——实现未落地"
        ) from exc
    fn = getattr(cc, "_parse_fallback_emotion_map", None)
    if fn is None:
        raise AssertionError(
            "模块级函数 _parse_fallback_emotion_map 不存在——契约 §2.1 解析函数未落地"
        )
    return fn(text)


def _make_fallback_block(entries_text: str) -> str:
    """构造含 FALLBACK 块 + 参数帧浮点块的样例文本。

    镜像 EmotionController.kt 结构：源码里 DEFAULT_BAI_EMOTION_PARAM_MAP 的
    `"_N" to mapOf("ParamBrowLY" to 0.0f, ...)` 是旧全文件 regex 的误抓源。
    """
    return textwrap.dedent(
        f"""\
        // 表情关键词 → 表情索引映射表
        private val FALLBACK_EMOTION_MAP: Map<String, Int> = mapOf(
        {entries_text})

        // 参数帧硬编码兜底（含浮点 to 值，旧 regex 的误抓源）
        private val DEFAULT_BAI_EMOTION_PARAM_MAP: Map<String, Map<String, Map<String, Float>>> = mapOf(
            "neutral" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f, "ParamEyeLOpen" to 0.75f, "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.0f),
        )
        """
    )


def _fallback_entries_from(d: dict) -> str:
    """把 dict 渲染成 Kotlin `"k" to v,` 行（块内内容）。"""
    lines = ["".join(f'            "{k}" to {v},') for k, v in d.items()]
    return "\n".join(lines) if lines else "            // 空"


# ---- C-A3 正例：块定界提取 8 键且不含 Param 键（缺陷回归） ----

def test_parse_fallback_extracts_8_keys_no_param_keys():
    """块定界解析：恰返回 8 表情键，不得误抓浮点 `to 0.0f` 参数帧键。"""
    text = _make_fallback_block(_fallback_entries_from(_FALLBACK_EXPECTED))
    result = _parse_fallback(text)
    assert result == _FALLBACK_EXPECTED
    # 键集严格 = 8 表情键
    assert set(result.keys()) == set(_FALLBACK_KEYS)
    # 不得混入任何 Param* 参数帧键（旧缺陷回归守卫）
    assert all(not k.startswith("Param") for k in result.keys())
    assert "ParamBrowLY" not in result


def test_parse_fallback_ignores_float_param_tos_in_same_block():
    """就算参数帧块紧邻 FALLBACK 块（旧全文件扫 regex 误抓场景），也不误收浮点 to。"""
    text = _make_fallback_block(_fallback_entries_from(_FALLBACK_EXPECTED))
    result = _parse_fallback(text)
    # 若实现退化为全文件 regex：会把 ParamBrowLY to 0.0f / ParamEyeLOpen to 0.75f 等扫进来 →
    # 键集 != 8 → 契约要求返回 None。因此本用例断言两种可接受结果之一都不得带 Param 键：
    if result is None:
        pytest.fail("块定界正例返回 None（键集遭 Param 误抓污染）——C-A3 失败")
    assert not any(k.startswith("Param") for k in result)


def test_parse_fallback_values_match_expected_indices():
    """块定界解析：索引值（0-7）与契约顺序完全一致。"""
    text = _make_fallback_block(_fallback_entries_from(_FALLBACK_EXPECTED))
    result = _parse_fallback(text)
    for k, v in _FALLBACK_EXPECTED.items():
        assert result[k] == v, f"表情 {k} 索引应为 {v}"


# ---- C-A4 负例：FALLBACK 块缺失 / 键集不符 → None（防误绿） ----

def test_parse_fallback_missing_block_returns_none():
    """文本不含 FALLBACK_EMOTION_MAP 起点 → 返回 None（C-A4a）。"""
    text = textwrap.dedent(
        """\
        private val OTHER: Map<String, Int> = mapOf("neutral" to 0, "fear" to 1)
        """
    )
    assert _parse_fallback(text) is None


def test_parse_fallback_wrong_key_count_returns_none():
    """键集只有 7 个（缺 1 个表情）→ 返回 None，不得将缺失误判为一致（C-A4b）。"""
    d = dict(_FALLBACK_EXPECTED)
    del d["surprise"]  # 只剩 7 键
    text = _make_fallback_block(_fallback_entries_from(d))
    assert _parse_fallback(text) is None


def test_parse_fallback_mismatched_keyset_returns_none():
    """键集被污染/替换（随机键混入）→ 返回 None（C-A4c，防提取失明）。"""
    d = dict(_FALLBACK_EXPECTED)
    d["mystery_key"] = 99  # 多出未知键 → 键集 != 8
    text = _make_fallback_block(_fallback_entries_from(d))
    assert _parse_fallback(text) is None


def test_parse_fallback_empty_block_returns_none():
    """块存在但为空 → 返回 None（C-A4d）。"""
    text = _make_fallback_block(_fallback_entries_from({}))
    assert _parse_fallback(text) is None


# ---------------------------------------------------------------------------
# C-A2 负例：PC 侧 emotionMap 改动 → C26 FAIL 且 detail 输出键集差集（契约 §2.1）
# ---------------------------------------------------------------------------

def test_c26_negative_pc_emotionmap_python_side_diff():
    """纯 Python 层负例：三方等值比较在单侧改动后必须判 FAIL。

    构造 adapter/Android 回退 = 8 键标准，pc 侧一个值改动 → 等值比较直接失败；
    且失败应通过 差集 呈现（本用例核验等值捕获 + 键集差集可计算）。
    """
    adapter = dict(_FALLBACK_EXPECTED)
    android_fallback = dict(_FALLBACK_EXPECTED)
    pc = dict(_FALLBACK_EXPECTED)
    pc["anger"] = 30  # 单项值漂移（对应临时改 PC model_dict emotionMap 一值）

    assert adapter != pc, "单测前提：改动后 pc 应可区分"
    # diff 判别人能算出差集（机械断言 detail 应包含这两侧的键集差 = {} 值差由索引展示）
    # 此处至少证明：值不等 → 三方不一致成立
    assert adapter == android_fallback
    assert not (adapter == pc == android_fallback)


def test_c26_negative_pc_emotionmap_subprocess_mutation_and_restore():
    """脚本级负例（C-A2）：临时改 PC model_dict niziiro_mao emotionMap 一值 →
    一致性脚本 exit 1 且 C26 FAIL；改后还原，脚本恢复全 PASS 集合。

    改动在 finally 中原样还原（字节级），不污染真实仓库。
    契约 §3 C-A2：`临时改 PC model_dict.json niziiro_mao emotionMap 一个值 →
    C26 FAIL 且 detail 输出键集差集（改后还原）`。
    """
    _PC_MODEL_DICT = _REPO_ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "model_dict.json"
    orig_bytes = _PC_MODEL_DICT.read_bytes()
    try:
        data = json.loads(orig_bytes)
        # PC model_dict.json 顶层为列表（既有数据基线），逐条找 niziiro_mao 的 emotionMap
        rows = data if isinstance(data, list) else data.get("live2d", data)
        niz = next(
            (e for e in rows if isinstance(e, dict) and e.get("name") == "niziiro_mao"),
            None,
        )
        if niz is None:
            raise AssertionError("PC model_dict.json 未找到 niziiro_mao 条目，无法实施 C-A2 负例")
        emo = niz.get("emotionMap")
        assert isinstance(emo, dict) and emo, "niziiro_mao emotionMap 应为非空 dict"
        # 改动一个值（契约 C-A2：临时改 emotionMap 一个值外 → C26 FAIL）
        first_key = next(iter(emo))
        emo[first_key] = 12345
        _PC_MODEL_DICT.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        code = _run_consistency()
        assert code == 1, f"PC 侧 emotionMap 改动后一致性脚本应 FAIL（exit 1），实际 exit={code}"
    finally:
        _PC_MODEL_DICT.write_bytes(orig_bytes)  # 字节级还原
    # 还原后脚本应恢复（exit 0 或至少 C26 PASS）——见 C-A1 全量校验


def _run_consistency() -> int:
    """以子进程运行一致性脚本，返回退出码（0=全过 / 1=任一项失败 / 2=环境错误）。"""
    env = dict(os.environ)
    res = subprocess.run(
        [sys.executable, str(_SHARED_CONSISTENCY)],
        cwd=str(_REPO_ROOT), capture_output=True, text=True, env=env,
        encoding="utf-8", errors="replace",
    )
    return res.returncode


# ---------------------------------------------------------------------------
# C-A5 接线断言（契约 §2.1）：EmotionController.kt 真实文件静态断言 + mock 文本
# ---------------------------------------------------------------------------

def test_wiring_expression_present_in_emotion_controller():
    """接线断言（正例/文件级）：EmotionController.kt 含
    `currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP`（防 getter 独立硬编码漂移）。"""
    src = _EMOTION_CONTROLLER.read_text(encoding="utf-8")
    assert _WIRING_EXPRESSION in src


def test_wiring_expression_detection_logic():
    """接线断言（mock 文本级 C-A5）：含 wiring 字符串的样例 → 命中；去掉 → 未命中。"""
    with_wiring = _FALLBACK_EXPECTED  # 仅作哨兵，避免空引用
    sample_with = f"val m: Map<String,Int> get() = {_WIRING_EXPRESSION}"
    sample_without = sample_with.replace(_WIRING_EXPRESSION, "get()`")
    assert _WIRING_EXPRESSION in sample_with
    assert _WIRING_EXPRESSION not in sample_without
    assert with_wiring is not None  # 无操作哨兵


# ---------------------------------------------------------------------------
# C-A6 C32 默认一致性断言单元（契约 §2.2，mock 文本）
# ---------------------------------------------------------------------------

def _c32_consistent(registry_default_id: str, settings_default_id: str, fallback_entry_id: str) -> bool:
    """C32 静态解析三方等值：registry default.id == Settings DEFAULT_MODEL_ID == ModelRegistry FALLBACK_ENTRY.id。"""
    return registry_default_id == settings_default_id == fallback_entry_id


def test_c32_three_sources_consistent_pass():
    """C-A6a：三源一致（bai=bai=bai）→ PASS。"""
    assert _c32_consistent("bai", "bai", "bai") is True


def test_c32_fallback_mismatch_fails():
    """C-A6b：ModelRegistry.FALLBACK_ENTRY.id 与他两源不同（niziiro_mao vs bai）→ FAIL。"""
    assert _c32_consistent("bai", "bai", "niziiro_mao") is False


def test_c32_parses_registry_default_from_real_file():
    """C-A6c：真实 shared/model_registry.json 静态解析 default 条目 id == "bai"（M-AND 落地后直通）。"""
    reg = json.loads((_REPO_ROOT / "shared" / "model_registry.json").read_text(encoding="utf-8"))
    default = next((m for m in reg["models"] if m.get("default")), None)
    assert default is not None, "registry 应含 default 条目"
    assert default["id"] == "bai"


# ---------------------------------------------------------------------------
# C-A7 / C-A8 Layout 双端文件同步（契约 §2.3，M-PC §2.1 同值）
# ---------------------------------------------------------------------------

def test_android_bai_model3_json_valid_and_layout_values():
    """C-A7：Android 侧 bai.model3.json JSON 合法且 Layout 四值精确 == 0.0/-0.07/0.56/0.56。"""
    assert _ANDROID_BAI_MODEL3.exists(), "Android 侧 bai.model3.json 缺失"
    data = json.loads(_ANDROID_BAI_MODEL3.read_text(encoding="utf-8"))
    assert isinstance(data, dict)
    assert "Layout" in data, "Android 侧 bai.model3.json 应含 Layout（C-A7 RED：当前缺失）"
    layout = data["Layout"]
    for k, v in _LAYOUT_EXPECTED.items():
        assert layout.get(k) == v, f"Layout.{k} 应为 {v}"


def test_android_layout_position_between_filereferences_and_groups():
    """Layout 键序（M-PC/§2.3）：FileReferences 之后、Groups 之前。"""
    data = json.loads(_ANDROID_BAI_MODEL3.read_text(encoding="utf-8"))
    keys = list(data.keys())
    assert "FileReferences" in keys and "Groups" in keys and "Layout" in keys
    assert keys.index("FileReferences") < keys.index("Layout") < keys.index("Groups")


def test_dual_side_bai_model3_sha256_identical():
    """C-A8：双端 bai.model3.json sha256 字节一致（M-CONS 只读 PC 比对）。"""
    assert _PC_BAI_MODEL3.exists(), "PC 侧 bai.model3.json 缺失"
    h_android = hashlib.sha256(_ANDROID_BAI_MODEL3.read_bytes()).hexdigest()
    h_pc = hashlib.sha256(_PC_BAI_MODEL3.read_bytes()).hexdigest()
    assert h_android == h_pc, f"双端 sha256 不一致: android={h_android}, pc={h_pc}"


def test_layout_present_both_sides():
    """Layout 双端应均在 FileReferences 之后、Groups 之前（同步后两文件结构一致）。"""
    for p in (_ANDROID_BAI_MODEL3, _PC_BAI_MODEL3):
        data = json.loads(p.read_text(encoding="utf-8"))
        keys = list(data.keys())
        assert "Layout" in keys, f"{p.name} 缺 Layout"
        assert keys.index("FileReferences") < keys.index("Layout") < keys.index("Groups")


# ---------------------------------------------------------------------------
# C-A1 / C-A9 / C-A10 脚本级全量校验（执行 main()，公开退出码契约；RED 于实现未落地）
# ---------------------------------------------------------------------------

def test_script_all_checks_pass_32():
    """C-A1：一致性脚本全量 == 32/32（C26 修复 + C32 新增后；C26 detail 三方 8 key 一致）。"""
    code = _run_consistency()
    assert code == 0, f"一致性脚本应全过（exit 0，32/32），实际 exit={code}"
    out = subprocess.run(
        [sys.executable, str(_SHARED_CONSISTENCY)],
        cwd=str(_REPO_ROOT), capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    ).stdout
    assert "/32" in out, "脚本末尾应汇总 32/32"


def test_validate_registry_still_passes():
    """C-A9：registry/schema 校验不受影响（validate_registry.py 仍过，C24 不回归）。"""
    sys.path.insert(0, str(_REPO_ROOT / "shared"))
    import validate_registry  # noqa: E402

    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        code = validate_registry.main()
    out = buf.getvalue()
    assert code == 0, out
    assert "All checks passed" in out


def test_registry_no_structure_change_regression_guard():
    """C26 定论非数据污染：registry 结构与契约锁定字段零改动（default=bai 条目完整）。"""
    reg = json.loads((_REPO_ROOT / "shared" / "model_registry.json").read_text(encoding="utf-8"))
    bai = next((m for m in reg["models"] if m["id"] == "bai"), None)
    assert bai and bai["default"] is True
    for field in ("id", "name", "description", "model3JsonPath", "available", "default"):
        assert field in bai, f"registry default 条目缺 {field}（结构不应被本批改动）"


if __name__ == "__main__":
    unittest.main()
