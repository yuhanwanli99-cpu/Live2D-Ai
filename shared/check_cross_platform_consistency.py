#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""跨端一致性检查脚本（task-e2e，plan-e2e.md §3 C1-C21 机器化执行）。

用途：自动验证 Android 与 PC 两端是否共享同一套模型注册表约定、双端资源
存在性、查询语义一致性、回退策略一致性与持久化语义一致性。

运行方式（仓库根目录下，或任意目录下均可）：
    python shared/check_cross_platform_consistency.py

输出：逐项打印 `[PASS]` / `[FAIL]`（含检查说明与依据），结尾汇总。
退出码：0 = 全部通过；1 = 任一项失败；2 = 环境错误（缺依赖/关键文件缺失）。

实现约束（与既有约定一致）：
- 仅依赖 Python 标准库 + jsonschema（validate_registry.py 的既有依赖）+ 轻量
  open_llm_vtuber.model_registry（仅 stdlib）。**不 import service_context**：
  根测试环境无 fastapi/torch/mcp 等重依赖（task-pc 已确立该约定）。C2 通过
  AST 提取 service_context.py 中 `_resolve_shared_path`/`_find_repo_shared_dir`
  的真实函数体执行（零转写，test-pc 已验证的既有技术）。
- Android 端查询语义无法在 Python 中执行（Kotlin/Robolectric），脚本依赖
  C1（assets 与 shared 字节一致 ⇒ 两端数据相同）+ 源码静态检查 + Android 单测
  （ModelRegistryTest）作为二阶证据；对照 plan-e2e.md §5.2 的约定。

各检查与 plan-e2e.md §3 检查点编号一一对应；静态检查会在说明中标注依据。
"""
from __future__ import annotations

import ast
import json
import re
import sys
from pathlib import Path

# Windows 控制台默认 GBK 可能无法输出部分字符；强制 UTF-8，保证逐项结果可读且 CI 可解析。
try:  # Python 3.7+
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
except (AttributeError, OSError):
    pass

ROOT = Path(__file__).resolve().parent.parent

# 关键路径
SHARED_REGISTRY = ROOT / "shared" / "model_registry.json"
SHARED_SCHEMA = ROOT / "shared" / "model_registry.schema.json"
VALIDATE_SCRIPT = ROOT / "shared" / "validate_registry.py"
ANDROID_ASSETS_REGISTRY = (
    ROOT / "Live2Dai-Android" / "app" / "src" / "main" / "assets" / "model_registry.json"
)
ANDROID_LIVE2D_ROOT = ROOT / "Live2Dai-Android" / "app" / "src" / "main" / "assets" / "live2d"
PC_LIVE2D_ROOT = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "live2d-models"
ANDROID_REGISTRY_KT = (
    ROOT
    / "Live2Dai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2dai"
    / "android"
    / "data"
    / "ModelRegistry.kt"
)
ANDROID_SETTINGS_KT = (
    ROOT
    / "Live2Dai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2dai"
    / "android"
    / "data"
    / "SettingsRepository.kt"
)
ANDROID_MAIN_ACTIVITY_KT = (
    ROOT
    / "Live2Dai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2dai"
    / "android"
    / "MainActivity.kt"
)
PC_MODEL_REGISTRY_PY = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "src" / "open_llm_vtuber" / "model_registry.py"
PC_SERVICE_CONTEXT_PY = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "src" / "open_llm_vtuber" / "service_context.py"
PC_CONF_YAML = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "conf.yaml"
ANDROID_BUILD_GRADLE = ROOT / "Live2Dai-Android" / "app" / "build.gradle.kts"
SHARED_EMOTION_PROTOCOL = ROOT / "shared" / "emotion-protocol.md"
SHARED_PERSONA = ROOT / "shared" / "persona.yaml"
ANDROID_ASSETS_PERSONA = ROOT / "Live2Dai-Android" / "app" / "src" / "main" / "assets" / "persona.yaml"

# 结果收集：(check_id, title, ok, detail)
_RESULTS: list[tuple[str, str, bool, str]] = []


def record(check_id: str, title: str, ok: bool, detail: str = "") -> bool:
    """记录一条检查结果并打印。"""
    _RESULTS.append((check_id, title, ok, detail))
    status = "PASS" if ok else "FAIL"
    suffix = f"  [{detail}]" if detail else ""
    print(f"[{status}] {check_id} - {title}{suffix}")
    return ok


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# 工具：轻量 import（不触碰重依赖）
# ---------------------------------------------------------------------------

def _load_registry_and_validate() -> tuple[dict, object, object]:
    """加载 registry JSON + validate_registry + PC ModelRegistry（轻量）。"""
    sys.path.insert(0, str(ROOT / "shared"))
    sys.path.insert(0, str(ROOT / "Live2Dai-pc" / "open-llm-vtuber" / "src"))
    import validate_registry  # noqa: F401  （轻量，仅 stdlib + jsonschema）
    from open_llm_vtuber.model_registry import ModelRegistry  # noqa: F401

    registry = json.loads(SHARED_REGISTRY.read_text(encoding="utf-8"))
    return registry, validate_registry, ModelRegistry


def _extract_resolve_shared_path() -> callable:
    """AST 提取 service_context.py 中 _resolve_shared_path/_find_repo_shared_dir 并执行。

    返回可调用的 `_resolve_shared_path(config_path) -> str`（真实函数体，零转写）。
    找不到函数 → 抛 LookupError。
    """
    text = read_text(PC_SERVICE_CONTEXT_PY)
    tree = ast.parse(text)
    wanted = ("_find_repo_shared_dir", "_resolve_shared_path")
    sources: list[str] = []
    for node in tree.body:
        if isinstance(node, ast.FunctionDef) and node.name in wanted:
            seg = ast.get_source_segment(text, node)
            if seg is None:
                raise LookupError(f"cannot extract source of {node.name}")
            sources.append(seg)
    if len(sources) != 2:
        raise LookupError(
            f"service_context.py 未找到 {wanted}（找到 {len(sources)} 个函数）"
        )
    ns: dict = {"os": __import__("os"), "__file__": str(PC_SERVICE_CONTEXT_PY)}
    exec("\n\n".join(sources), ns)  # noqa: S102 — 提取的是本仓库自身代码
    return ns["_resolve_shared_path"]


# ---------------------------------------------------------------------------
# C1-C5 注册表源头一致性
# ---------------------------------------------------------------------------

def check_registry_source(registry, validate_registry) -> None:
    # C1: Android assets 与 shared 字节级一致
    if ANDROID_ASSETS_REGISTRY.is_file():
        same = (
            SHARED_REGISTRY.read_bytes() == ANDROID_ASSETS_REGISTRY.read_bytes()
        )
        record(
            "C1",
            "Android assets/model_registry.json 与 shared/model_registry.json 字节级一致",
            same,
            "identical bytes" if same else "bytes differ (build 产物过期，请重跑 Gradle copyModelRegistry)",
        )
    else:
        record(
            "C1",
            "Android assets/model_registry.json 与 shared/model_registry.json 字节级一致",
            False,
            f"assets 产物缺失（{ANDROID_ASSETS_REGISTRY}；Gradle clean 后未重跑构建，该文件由 copyModelRegistry 生成）",
        )

    # C2: PC 端 _resolve_shared_path 解析到 shared/model_registry.json
    try:
        resolver = _extract_resolve_shared_path()
        resolved = Path(resolver("shared/model_registry.json"))
        expected = SHARED_REGISTRY
        ok = resolved == expected and resolved.is_file()
        record(
            "C2",
            "PC service_context._resolve_shared_path 解析到 shared/model_registry.json（绝对路径）",
            ok,
            str(resolved) if ok else f"resolved={resolved}, expected={expected}",
        )
    except (LookupError, OSError) as exc:
        record(
            "C2",
            "PC service_context._resolve_shared_path 解析到 shared/model_registry.json（绝对路径）",
            False,
            f"AST 提取/执行失败: {exc}",
        )

    # C3: Gradle copyModelRegistry 源路径正确
    gradle = read_text(ANDROID_BUILD_GRADLE)
    c3_ok = (
        'rootProject.projectDir.parentFile.resolve("shared/model_registry.json")'
        in gradle
        and "copyModelRegistry" in gradle
        and "from(registrySourceFile)" in gradle
    )
    record(
        "C3",
        "Gradle copyModelRegistry 源路径 = rootProject 父目录 shared/model_registry.json",
        c3_ok,
        "app/build.gradle.kts 静态检查" if c3_ok else "build.gradle.kts 未找到预期的 registrySourceFile/copyModelRegistry",
    )

    # C4: validate_registry.py 正向校验通过
    code = validate_registry.main()
    record(
        "C4",
        "shared/validate_registry.py 正向校验通过（exit 0）",
        code == 0,
        f"exit={code}",
    )

    # C5: validate_registry.PLATFORM_ROOTS 指向正确目录
    roots = validate_registry.PLATFORM_ROOTS
    android_root = Path(roots["android"]).resolve()
    pc_root = Path(roots["pc"]).resolve()
    c5_ok = android_root == ANDROID_LIVE2D_ROOT.resolve() and pc_root == PC_LIVE2D_ROOT.resolve()
    record(
        "C5",
        "validate_registry.PLATFORM_ROOTS 指向 assets/live2d 与 live2d-models",
        c5_ok,
        f"android={android_root}, pc={pc_root}",
    )


# ---------------------------------------------------------------------------
# C6-C8 模型 ID 约定一致性
# ---------------------------------------------------------------------------

def check_id_convention(registry, ModelRegistry) -> None:
    models = registry["models"]
    pc = ModelRegistry(str(SHARED_REGISTRY))
    android_kt = read_text(ANDROID_REGISTRY_KT)
    pc_py = read_text(PC_MODEL_REGISTRY_PY)

    # C6: 两端均直接使用 JSON id，无转写；id 命名规范小写蛇形
    ids_ok = all(re.fullmatch(r"[a-z][a-z0-9_]*", m["id"]) for m in models)
    pc_uses_raw_id = '"id": "id"' in pc_py and "raw[\"id\"]" in pc_py
    android_uses_raw_id = "val id: String" in android_kt and "it.id == id" in android_kt
    pc_ids_match = [e.id for e in pc.get_all()] == [m["id"] for m in models]
    c6_ok = ids_ok and pc_uses_raw_id and android_uses_raw_id and pc_ids_match
    record(
        "C6",
        "两端注册表模型 ID 命名规范一致（小写蛇形）且均直接使用 JSON id 无转写",
        c6_ok,
        "PC get_all ids == JSON ids；Android ModelEntry 直映射（静态）" if c6_ok else f"ids_snake={ids_ok} pc_raw={pc_uses_raw_id} android_raw={android_uses_raw_id} pc_ids_match={pc_ids_match}",
    )

    # C7: model3JsonPath 语义一致（纯相对路径，不含平台前缀）
    paths_ok = all(
        isinstance(m["model3JsonPath"], str)
        and not m["model3JsonPath"].startswith(("live2d/", "live2d-models/"))
        and m["model3JsonPath"].startswith(m["id"] + "/")
        for m in models
    )
    pc_paths_match = all(
        pc.get_model3_json_path(m["id"]) == m["model3JsonPath"] for m in models
    )
    android_uses_same = "model3JsonPath" in android_kt
    c7_ok = paths_ok and pc_paths_match and android_uses_same
    record(
        "C7",
        "两端 model3JsonPath 语义一致（相对路径、不含平台前缀）",
        c7_ok,
        "PC get_model3_json_path == JSON 值；Android 同源（C1 + 静态）" if c7_ok else f"paths_ok={paths_ok} pc_match={pc_paths_match}",
    )

    # C8: 默认模型 ID 一致（Android SettingsRepository / PC conf.yaml / registry default）
    default_entries = [m for m in models if m.get("default") is True]
    reg_default_ok = len(default_entries) == 1 and default_entries[0]["id"] == "mao_pro"
    settings_kt = read_text(ANDROID_SETTINGS_KT)
    android_default_ok = 'DEFAULT_MODEL_ID = "mao_pro"' in settings_kt
    conf = read_text(PC_CONF_YAML)
    pc_default_ok = bool(
        re.search(r"^\s*live2d_model_name\s*:\s*['\"]mao_pro['\"]\s*$", conf, re.M)
    )
    c8_ok = reg_default_ok and android_default_ok and pc_default_ok
    record(
        "C8",
        "默认模型 ID 一致（registry default / Android DEFAULT_MODEL_ID / PC conf.yaml 均为 mao_pro）",
        c8_ok,
        f"registry={reg_default_ok} android={android_default_ok} pc={pc_default_ok}",
    )


# ---------------------------------------------------------------------------
# C9-C11 模型资源文件存在性
# ---------------------------------------------------------------------------

def check_resources(registry) -> None:
    models = registry["models"]

    # C9: available=true 的模型 model3.json 在双端都存在
    avail_ok = True
    avail_detail: list[str] = []
    for m in models:
        if m.get("available") is not True:
            continue
        rel = m["model3JsonPath"]
        a = (ANDROID_LIVE2D_ROOT / rel).is_file()
        p = (PC_LIVE2D_ROOT / rel).is_file()
        if not (a and p):
            avail_ok = False
            avail_detail.append(f"{m['id']}: android={'Y' if a else 'N'}, pc={'Y' if p else 'N'}")
    record(
        "C9",
        "所有 available=true 模型的 model3.json 双端均存在",
        avail_ok,
        "; ".join(avail_detail) or "mao_pro: android OK, pc OK; shizuku: android OK, pc OK",
    )

    # C10: available=false 的模型文件双端均缺失（planned 状态，符合设计）
    unavail_ok = True
    unavail_detail: list[str] = []
    for m in models:
        if m.get("available") is not False:
            continue
        rel = m["model3JsonPath"]
        a = (ANDROID_LIVE2D_ROOT / rel).is_file()
        p = (PC_LIVE2D_ROOT / rel).is_file()
        if a or p:
            unavail_ok = False
            unavail_detail.append(f"{m['id']}: android={'Y' if a else 'N'}, pc={'Y' if p else 'N'}")
    record(
        "C10",
        "available=false 的模型文件双端均缺失（符合 planned 设计）",
        unavail_ok,
        "; ".join(unavail_detail) or "hiyori/haru 双端均 MISSING（正确）",
    )

    # C11: shizuku 资源完整性 —— 按 model3.json FileReferences 全量核对
    #      （plan 原文写 textures/motions 目录，真实模型用 shizuku.1024/ + motion/，
    #       本检查以 FileReferences 实际引用的文件为准，覆盖更强）
    shizuku_model3 = PC_LIVE2D_ROOT / "shizuku" / "runtime" / "shizuku.model3.json"
    if shizuku_model3.is_file():
        refs = json.loads(shizuku_model3.read_text(encoding="utf-8")).get("FileReferences", {})
        missing: list[str] = []
        moc = refs.get("Moc")
        if moc and not (shizuku_model3.parent / moc).is_file():
            missing.append(moc)
        for tex in refs.get("Textures") or []:
            if not (shizuku_model3.parent / tex).is_file():
                missing.append(tex)
        for grp, motions in (refs.get("Motions") or {}).items():
            for mt in motions or []:
                f = mt.get("File")
                if f and not (shizuku_model3.parent / f).is_file():
                    missing.append(f"{grp}/{f}")
        n_motion = sum(len(v or []) for v in (refs.get("Motions") or {}).values())
        c11_ok = (not missing) and n_motion >= 1
        detail = (
            f"moc3 ✓ textures({len(refs.get('Textures') or [])}) ✓ motions({n_motion}) ✓"
            if c11_ok
            else f"missing={missing}"
        )
    else:
        c11_ok = False
        detail = "shizuku.model3.json 缺失"
    record(
        "C11",
        "PC 端 shizuku 资源完整（moc3 / 纹理 / motions 按 model3.json FileReferences 核对）",
        c11_ok,
        detail,
    )


# ---------------------------------------------------------------------------
# C12-C16 查询接口语义一致性
# ---------------------------------------------------------------------------

def check_query_semantics(registry, ModelRegistry) -> None:
    models = registry["models"]
    pc = ModelRegistry(str(SHARED_REGISTRY))
    android_kt = read_text(ANDROID_REGISTRY_KT)

    # C12: get_all() 条目数一致（4）
    n_json = len(models)
    n_pc = len(pc.get_all())
    c12_ok = n_json == 4 and n_pc == 4
    record(
        "C12",
        "get_all() 条目数一致（4：mao_pro/shizuku/hiyori/haru）",
        c12_ok,
        f"json={n_json}, pc={n_pc}（Android 同源见 C1）",
    )

    # C13: get_available() 过滤结果一致 {mao_pro, shizuku}
    pc_avail = sorted(e.id for e in pc.get_available())
    expect = sorted(m["id"] for m in models if m.get("available") is True)
    # DIFF-2：Android 排除 restrictPlatform=='pc'；当前数据无 pc 限定条目 ⇒ 等价
    no_pc_restricted = all(m.get("restrictPlatform") != "pc" for m in models)
    c13_ok = pc_avail == expect == ["mao_pro", "shizuku"] and no_pc_restricted
    record(
        "C13",
        "get_available() 过滤结果一致（{mao_pro, shizuku}）",
        c13_ok,
        f"pc={pc_avail}（Android 过滤 restrictPlatform=='pc' 当前数据为空 ⇒ 等价，DIFF-2 记录在案）"
        if c13_ok
        else f"pc={pc_avail}, expect={expect}, no_pc_restricted={no_pc_restricted}",
    )

    # C14: get_default() 返回值一致（mao_pro）
    pc_default = pc.get_default().id
    reg_default = next(m["id"] for m in models if m.get("default") is True)
    reg_default_avail = next(m["available"] for m in models if m.get("default") is True)
    c14_ok = pc_default == reg_default == "mao_pro" and reg_default_avail is True
    record(
        "C14",
        "get_default() 返回一致（mao_pro，唯一 default 且 available）",
        c14_ok,
        f"pc={pc_default}, registry_default={reg_default}",
    )

    # C15: get_by_id('nonexistent') 行为一致（null/None，不抛异常）
    c15_ok = pc.get_by_id("nonexistent") is None and "firstOrNull { it.id == id }" in android_kt
    record(
        "C15",
        "get_by_id('nonexistent') 行为一致（PC None / Android null，不抛异常）",
        c15_ok,
        "pc=None, android=firstOrNull→null（静态）" if c15_ok else f"pc={pc.get_by_id('nonexistent')}",
    )

    # C16: get_model3_json_path(id) 返回路径一致（字符串完全相同）
    pc_paths_ok = all(
        pc.get_model3_json_path(m["id"]) == m["model3JsonPath"] for m in models
    )
    android_uses_same = "getById(id)?.model3JsonPath" in android_kt
    c16_ok = pc_paths_ok and android_uses_same
    record(
        "C16",
        "get_model3_json_path(id) 路径字符串完全一致（不含平台前缀）",
        c16_ok,
        "pc == JSON 值；android 返回同源 model3JsonPath（静态）" if c16_ok else f"pc_paths_ok={pc_paths_ok}",
    )


# ---------------------------------------------------------------------------
# C17-C19 回退策略一致性
# ---------------------------------------------------------------------------

def check_fallback(registry, ModelRegistry) -> None:
    android_kt = read_text(ANDROID_REGISTRY_KT)
    pc_svc = read_text(PC_SERVICE_CONTEXT_PY)
    main_kt = read_text(ANDROID_MAIN_ACTIVITY_KT)

    # C17: 注册表缺失/损坏 → 两端均回退 mao_pro，不崩溃
    android_fb = "fun fallback()" in android_kt and "FALLBACK_ENTRY" in android_kt
    pc_fb = (
        "return None" in pc_svc
        and '_fallback_live2d_model_name' in pc_svc
        and '"mao_pro"' in pc_svc
    )
    c17_ok = android_fb and pc_fb
    record(
        "C17",
        "注册表缺失/损坏 → 两端均回退 mao_pro 不崩溃",
        c17_ok,
        "Android parse()/load() 捕获→fallback()（静态）；PC _load_model_registry→None→硬编码 mao_pro（静态）"
        if c17_ok
        else f"android_fallback={android_fb}, pc_fallback={pc_fb}",
    )

    # C18: selectedModelId 指向不存在 ID → 两端回退默认模型
    android_c18 = "getById(modelId) ?: registry.getDefault()" in main_kt
    pc_c18 = "resolve_model_name" in pc_svc and "get_default()" in pc_svc
    c18_ok = android_c18 and pc_c18
    record(
        "C18",
        "所选模型 ID 不存在 → 两端均回退默认模型",
        c18_ok,
        "Android MainActivity `getById ?: getDefault`（静态）；PC _resolve_live2d_model_name → registry 默认（resolve_model_name，单测覆盖）"
        if c18_ok
        else f"android={android_c18}, pc={pc_c18}",
    )

    # C19: get_default() available 守卫差异（DIFF-1）—— 记录在案 + 数据安全
    pc_guards = "model.default and model.available" in read_text(PC_MODEL_REGISTRY_PY)
    android_no_guard = "firstOrNull { it.default }" in android_kt
    data_safe = all(
        not (m.get("default") is True and m.get("available") is not True)
        for m in registry["models"]
    )
    c19_ok = pc_guards and android_no_guard and data_safe
    record(
        "C19",
        "get_default() available 守卫差异已记录（DIFF-1），当前数据满足 default⇒available 不可达",
        c19_ok,
        "PC 要求 default&&available，Android 首分支不查 available；真实数据无 default 但不可用条目"
        if c19_ok
        else f"pc_guard={pc_guards}, android_plain={android_no_guard}, data_safe={data_safe}",
    )


# ---------------------------------------------------------------------------
# C20-C21 持久化语义一致性
# ---------------------------------------------------------------------------

def check_shared_protocol() -> None:
    """C22-C23: 共享表情协议 + persona 一致性。"""

    # C22: shared/emotion-protocol.md 存在且含 B/C/D/E/F 段落标题
    c22_ok = False
    c22_detail = ""
    if SHARED_EMOTION_PROTOCOL.is_file():
        content = read_text(SHARED_EMOTION_PROTOCOL)
        required_sections = [
            "## B. JSON schema",
            "## C. 字段约束",
            "## D. 示例",
            "## E. 降级规则",
            "## F. motion 映射表",
        ]
        missing = [s for s in required_sections if s not in content]
        if not missing:
            c22_ok = True
            c22_detail = "B/C/D/E/F 全部段落标题存在"
        else:
            c22_detail = f"缺少段落标题: {missing}"
    else:
        c22_detail = f"文件不存在: {SHARED_EMOTION_PROTOCOL}"
    record(
        "C22",
        "shared/emotion-protocol.md 存在且含 B/C/D/E/F 全部段落标题",
        c22_ok,
        c22_detail,
    )

    # C23: shared/persona.yaml 与 Android assets 副本字节一致
    if SHARED_PERSONA.is_file() and ANDROID_ASSETS_PERSONA.is_file():
        c23_ok = SHARED_PERSONA.read_bytes() == ANDROID_ASSETS_PERSONA.read_bytes()
        c23_detail = "identical bytes" if c23_ok else "bytes differ（persona 同步已过期，请重新同步 shared→assets）"
    else:
        c23_ok = False
        c23_detail = f"文件缺失: shared={'Y' if SHARED_PERSONA.is_file() else 'N'}, assets={'Y' if ANDROID_ASSETS_PERSONA.is_file() else 'N'}"
    record(
        "C23",
        "shared/persona.yaml 与 Android assets/persona.yaml 字节一致（防再次错位）",
        c23_ok,
        c23_detail,
    )


def check_persistence() -> None:
    settings_kt = read_text(ANDROID_SETTINGS_KT)
    conf = read_text(PC_CONF_YAML)
    android_kt = read_text(ANDROID_REGISTRY_KT)
    pc_py = read_text(PC_MODEL_REGISTRY_PY)

    # C20: 两端持久化键正确，默认值一致
    android_key = 'KEY_SELECTED_MODEL_ID = "selected_model_id"' in settings_kt
    android_default = 'DEFAULT_MODEL_ID = "mao_pro"' in settings_kt
    pc_key = bool(
        re.search(r"^\s*live2d_model_name\s*:", conf, re.M)
    )
    pc_default = bool(
        re.search(r"^\s*live2d_model_name\s*:\s*['\"]mao_pro['\"]\s*$", conf, re.M)
    )
    c20_ok = android_key and android_default and pc_key and pc_default
    record(
        "C20",
        "两端持久化键正确（Android 'selected_model_id' / PC conf.yaml live2d_model_name），默认值 mao_pro",
        c20_ok,
        f"android_key={android_key}, android_default={android_default}, pc_key={pc_key}, pc_default={pc_default}",
    )

    # C21: 两端持久化独立（crossPlatformSync=no_sync，无网络同步代码）
    files = [android_kt, pc_py, read_text(PC_SERVICE_CONTEXT_PY)]
    hits = [kw for kw in ("firebase", "Firebase", "cloud sync") if any(kw in f for f in files)]
    c21_ok = not hits
    record(
        "C21",
        "两端持久化独立（无 Firebase/API/cloud sync 同步代码）",
        c21_ok,
        "registry 相关文件零命中" if c21_ok else f"命中关键字: {hits}",
    )


def main() -> int:
    try:
        registry, validate_registry, ModelRegistry = _load_registry_and_validate()
    except ImportError as exc:
        print(f"ERROR: 依赖缺失: {exc}（需要 jsonschema；python -m pip install jsonschema）")
        return 2

    print("=" * 72)
    print("跨端一致性检查（plan-e2e.md §3 C1-C21）")
    print("=" * 72)

    print("\n--- 3.1 注册表源头一致性 ---")
    check_registry_source(registry, validate_registry)

    print("\n--- 3.2 模型 ID 约定一致性 ---")
    check_id_convention(registry, ModelRegistry)

    print("\n--- 3.3 模型资源文件存在性 ---")
    check_resources(registry)

    print("\n--- 3.4 查询接口语义一致性 ---")
    check_query_semantics(registry, ModelRegistry)

    print("\n--- 3.5 回退策略一致性 ---")
    check_fallback(registry, ModelRegistry)

    print("\n--- 3.6 持久化语义一致性 ---")
    check_persistence()

    print("\n--- 3.7 共享表情协议 + persona 一致性 ---")
    check_shared_protocol()

    print("\n" + "=" * 72)
    fails = [r for r in _RESULTS if not r[2]]
    passed = len(_RESULTS) - len(fails)
    print(f"结果: {passed}/{len(_RESULTS)} 通过, {len(fails)} 失败")
    if fails:
        print("失败项:")
        for cid, title, _ok, detail in fails:
            print(f"  - {cid} {title}: {detail}")
        return 1
    print("All cross-platform consistency checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
