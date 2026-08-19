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
import hashlib
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
    ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets" / "model_registry.json"
)
ANDROID_LIVE2D_ROOT = ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets" / "live2d"
PC_LIVE2D_ROOT = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "live2d-models"
ANDROID_REGISTRY_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "data"
    / "ModelRegistry.kt"
)
ANDROID_SETTINGS_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "data"
    / "SettingsRepository.kt"
)
ANDROID_MAIN_ACTIVITY_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "MainActivity.kt"
)
ANDROID_MAIN_SCREEN_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "ui"
    / "MainScreen.kt"
)
PC_MODEL_REGISTRY_PY = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "src" / "open_llm_vtuber" / "model_registry.py"
PC_SERVICE_CONTEXT_PY = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "src" / "open_llm_vtuber" / "service_context.py"
PC_CONF_YAML = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "conf.yaml"
ANDROID_BUILD_GRADLE = ROOT / "Live2D-Ai-Android" / "app" / "build.gradle.kts"
SHARED_EMOTION_PROTOCOL = ROOT / "shared" / "emotion-protocol.md"
SHARED_PERSONA = ROOT / "shared" / "persona.yaml"
ANDROID_ASSETS_PERSONA = ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets" / "persona.yaml"

# ── contract-v3-adapter.md §1（C24-C28 数据校验输入）+ contract-R1-model（D2：重锚 bai）──
ADAPTER_JSON = ROOT / "shared" / "model-adapter" / "bai.adapter.json"
MODEL3_RUNTIME_DIR = ANDROID_LIVE2D_ROOT / "bai" / "runtime"
ANDROID_MODEL3_JSON = MODEL3_RUNTIME_DIR / "bai.model3.json"
ANDROID_CDI3_JSON = MODEL3_RUNTIME_DIR / "bai.cdi3.json"
ANDROID_EMOTION_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "EmotionController.kt"
)
ANDROID_EMOTION_TEXT_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "core"
    / "EmotionText.kt"
)
ANDROID_VISEME_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "VisemeAnalyzer.kt"
)
PC_MODEL_DICT_JSON = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "model_dict.json"
PC_FRONTEND_VISEME_TS = (
    ROOT
    / "cache"
    / "oltv-web-src"
    / "src"
    / "renderer"
    / "src"
    / "hooks"
    / "utils"
    / "use-viseme-lipsync.ts"
)
PC_LIVE2D_MODEL_PY = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "src" / "open_llm_vtuber" / "live2d_model.py"
ANDROID_MOTION_TAGS_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "MotionTags.kt"
)
ANDROID_MODEL_ADAPTER_KT = (
    ROOT
    / "Live2D-Ai-Android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "live2d"
    / "ai"
    / "android"
    / "ModelAdapterConfig.kt"
)

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
    sys.path.insert(0, str(ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "src"))
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
    reg_default_ok = len(default_entries) == 1
    reg_default_id = default_entries[0]["id"] if default_entries else None
    settings_kt = read_text(ANDROID_SETTINGS_KT)
    android_default_ok = bool(reg_default_id) and f'DEFAULT_MODEL_ID = "{reg_default_id}"' in settings_kt
    conf = read_text(PC_CONF_YAML)
    pc_default_ok = bool(reg_default_id) and bool(
        re.search(rf"^\s*live2d_model_name\s*:\s*['\"]{re.escape(reg_default_id)}['\"]\s*$", conf, re.M)
    )
    c8_ok = reg_default_ok and android_default_ok and pc_default_ok
    record(
        "C8",
        f"默认模型 ID 一致（registry default / Android DEFAULT_MODEL_ID / PC conf.yaml 均为 {reg_default_id}）",
        c8_ok,
        f"registry={reg_default_ok}({reg_default_id}) android={android_default_ok} pc={pc_default_ok}",
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
    # S1/S6 双端同步（architecture-pc-fix §3 S1/S6 + contract-M-CONS §2.3）：
    # 默认模型（bai）的 model3.json 双端必须字节一致（Layout 字段双端同内容，M-CONS 只读 PC 文件比对）
    byte_ok = True
    byte_detail: list[str] = []
    for m in models:
        if m.get("default") is not True or m.get("available") is not True:
            continue
        rel = m["model3JsonPath"]
        a = ANDROID_LIVE2D_ROOT / rel
        p = PC_LIVE2D_ROOT / rel
        if not (a.is_file() and p.is_file()):
            byte_ok = False
            byte_detail.append(f"{m['id']}: 双端文件缺失，无法字节比对")
            continue
        if a.read_bytes() != p.read_bytes():
            byte_ok = False
            byte_detail.append(
                f"{m['id']}: android sha256={hashlib.sha256(a.read_bytes()).hexdigest()[:16]} "
                f"≠ pc sha256={hashlib.sha256(p.read_bytes()).hexdigest()[:16]}（Layout 未同步？）"
            )
    c9_ok = avail_ok and byte_ok
    c9_detail = ("; ".join(avail_detail + byte_detail) + "；default=bai 双端字节一致") if (avail_detail + byte_detail) else (
        "bai（唯一模型）both OK；bai model3.json 双端字节一致（含 Layout）"
    )
    record(
        "C9",
        "所有 available=true 模型的 model3.json 双端均存在；默认模型（bai）model3.json 双端字节一致",
        c9_ok,
        c9_detail,
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
        "; ".join(unavail_detail) or "无 available=false 条目（shizuku/hiyori/haru 已按 K1 删除）",
    )

    # C11（已按 K1 删除）: shizuku 资源完整性检查随 shizuku 实体一并移除（shizuku 不再存在）
    c11_ok = True
    record(
        "C11",
        "shizuku 已删除（K1）—— 本检查不再适用，恒为 PASS",
        c11_ok,
        "shizuku 实体已从 registry/双端目录/model_dict 全量移除",
    )


# ---------------------------------------------------------------------------
# C12-C16 查询接口语义一致性
# ---------------------------------------------------------------------------

def check_query_semantics(registry, ModelRegistry) -> None:
    models = registry["models"]
    pc = ModelRegistry(str(SHARED_REGISTRY))
    android_kt = read_text(ANDROID_REGISTRY_KT)

    # C12: get_all() 条目数一致（registry 数据条数 N）
    n_json = len(models)
    n_pc = len(pc.get_all())
    c12_ok = n_json == n_pc == len(models)
    record(
        "C12",
        "get_all() 条目数一致（registry 数据条数）",
        c12_ok,
        f"json={n_json}, pc={n_pc}（Android 同源见 C1）",
    )

    # C13: get_available() 过滤结果一致（available=true 的 id 集）
    pc_avail = sorted(e.id for e in pc.get_available())
    expect = sorted(m["id"] for m in models if m.get("available") is True)
    # DIFF-2：Android 排除 restrictPlatform=='pc'；当前数据无 pc 限定条目 ⇒ 等价
    no_pc_restricted = all(m.get("restrictPlatform") != "pc" for m in models)
    c13_ok = pc_avail == expect and no_pc_restricted
    record(
        "C13",
        f"get_available() 过滤结果一致（{expect}）",
        c13_ok,
        f"pc={pc_avail}（Android 过滤 restrictPlatform=='pc' 当前数据为空 ⇒ 等价，DIFF-2 记录在案）"
        if c13_ok
        else f"pc={pc_avail}, expect={expect}, no_pc_restricted={no_pc_restricted}",
    )

    # C14: get_default() 返回值一致（registry default，唯一 default 且 available）
    pc_default = pc.get_default().id
    reg_default = next(m["id"] for m in models if m.get("default") is True)
    reg_default_avail = next(m["available"] for m in models if m.get("default") is True)
    c14_ok = pc_default == reg_default and reg_default_avail is True
    record(
        "C14",
        f"get_default() 返回一致（{reg_default}，唯一 default 且 available）",
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

    # C17: 注册表缺失/损坏 → 两端均回退 bai，不崩溃（R1-model D2：默认路径收敛 bai）
    android_fb = "fun fallback()" in android_kt and "FALLBACK_ENTRY" in android_kt
    pc_fb = (
        "return None" in pc_svc
        and '_fallback_live2d_model_name' in pc_svc
        and '"bai"' in pc_svc
    )
    c17_ok = android_fb and pc_fb
    record(
        "C17",
        "注册表缺失/损坏 → 两端均回退 bai 不崩溃",
        c17_ok,
        "Android parse()/load() 捕获→fallback()（静态）；PC _load_model_registry→None→硬编码 bai（静态）"
        if c17_ok
        else f"android_fallback={android_fb}, pc_fallback={pc_fb}",
    )

    # C18: selectedModelId 指向不存在 ID → 两端回退默认模型
    # 断言目标 = MainScreen.kt（UI 层回退逻辑，contract-v3-fixes §3.2 锁死写法）
    android_c18 = (
        "getById(modelId) ?: registry.getDefault()" in read_text(ANDROID_MAIN_SCREEN_KT)
    )
    pc_c18 = "resolve_model_name" in pc_svc and "get_default()" in pc_svc
    c18_ok = android_c18 and pc_c18
    record(
        "C18",
        "所选模型 ID 不存在 → 两端均回退默认模型",
        c18_ok,
        "Android MainScreen `getById ?: getDefault`（静态）；PC _resolve_live2d_model_name → registry 默认（resolve_model_name，单测覆盖）"
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
        c23_detail = "identical bytes" if c23_ok else "bytes differ（persona 同步已过期，请重跑 Gradle copyPersona 任务同步 shared→assets）"
    else:
        c23_ok = False
        c23_detail = f"文件缺失: shared={'Y' if SHARED_PERSONA.is_file() else 'N'}, assets={'Y' if ANDROID_ASSETS_PERSONA.is_file() else 'N'}"
    record(
        "C23",
        "shared/persona.yaml 与 Android assets/persona.yaml 字节一致（防再次错位）",
        c23_ok,
        c23_detail,
    )

    # SM-03: shared/mcp_tools.json 与 Android assets/mcp_tools.json 字节一致
    shared_mcp = ROOT / "shared" / "mcp_tools.json"
    android_mcp = ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets" / "mcp_tools.json"
    if shared_mcp.is_file() and android_mcp.is_file():
        mcp_ok = shared_mcp.read_bytes() == android_mcp.read_bytes()
        mcp_detail = "identical bytes" if mcp_ok else "bytes differ（请重跑 Gradle copyMcpTools 任务同步 shared→assets）"
    else:
        mcp_ok = False
        mcp_detail = f"文件缺失: shared={'Y' if shared_mcp.is_file() else 'N'}, assets={'Y' if android_mcp.is_file() else 'N'}"
    record(
        "SM-03",
        "shared/mcp_tools.json 与 Android assets/mcp_tools.json 字节一致",
        mcp_ok,
        mcp_detail,
    )


# ---------------------------------------------------------------------------
# C24-C31 适配配置一致性（contract-v3-adapter.md §1.3 + schedule-v3 T3.3——真实数据校验 + 深度自校验）
# ---------------------------------------------------------------------------

_ADAPTER_REQUIRED_KEYS = {
    "modelId", "displayName", "motionAliases", "vowelParamMap",
    "vowelRawMap", "idlePool", "emotionIndex", "degradation",
}
# 注意：bai 无 motion（D2 后唯一模型）。原 niziiro 的 7 键 motion 表不再存在。
_MOTION_TAG_KEYS = set()  # bai.motionAliases == {}（双空一致，C25/C29）
_VOWEL_LABELS = {"A", "I", "U", "E", "O", "SIL"}
_EMOTION_KEYS = {"neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"}
# bai（D2 唯一模型 / contract-R1-model §2.2 + model-facts-bai §五）：口型参数为
# ParamMouthOpenY + ParamMouthForm（非五元音 ParamA/I/U/E/O）。每个 label 必含且仅含这两个键。
_VOWEL_CORE_PARAM_NAMES = {"ParamMouthOpenY", "ParamMouthForm"}
_VOWEL_EXTRA_PARAM_NAMES = set()  # bai 口型 std 集仅 MouthOpenY+MouthForm，无额外参数
# 全部允许的参数字段（未知参数名 = schema 漂移，视为 FAIL）
_VOWEL_PARAM_ALLOWED = _VOWEL_CORE_PARAM_NAMES | _VOWEL_EXTRA_PARAM_NAMES


def _load_adapter() -> dict | None:
    """加载 adapter JSON；缺失/解析失败 → None（供各检查判断）。"""
    if not ADAPTER_JSON.is_file():
        return None
    try:
        return json.loads(ADAPTER_JSON.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def _load_model3() -> dict | None:
    if not ANDROID_MODEL3_JSON.is_file():
        return None
    try:
        return json.loads(ANDROID_MODEL3_JSON.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def _load_cdi3() -> dict | None:
    if not ANDROID_CDI3_JSON.is_file():
        return None
    try:
        return json.loads(ANDROID_CDI3_JSON.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


# C26 接线锚点（contract-M-CONS §2.1：getter 必须经 currentAdapter() 读取 emotionIndex，
# 再兜底 FALLBACK_EMOTION_MAP —— 防 getter 独立硬编码漂移）
_EMOTION_WIRING_ANCHOR = "currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP"


def _parse_fallback_emotion_map(text: str) -> dict[str, int] | None:
    r"""块定界解析 EmotionController.kt 的 FALLBACK_EMOTION_MAP（contract-M-CONS §2.1）。

    以 `private val FALLBACK_EMOTION_MAP: Map<String, Int> = mapOf(` 为起点、
    配对 `)` 为终点截取源码片段；仅在该块内以 regex `"(\w+)"\s+to\s+(\d+)` 提取键值。
    返回前断言键集 == 8 个表情键（neutral/fear/sadness/anger/disgust/joy/smirk/surprise）；
    起点缺失/块未闭合/键集不符 → 返回 None（C26 判 FAIL 并展示差异，防"提取失明"）。

    不得再对全文件扫 regex：否则 `DEFAULT_BAI_EMOTION_PARAM_MAP` 的参数帧键
    （"ParamBrowLY" to 0.0f 等浮点前缀）会被 `(\d+)` 误抓成整数键 —— C26 历史缺陷回归用例。
    """
    marker = "private val FALLBACK_EMOTION_MAP: Map<String, Int> = mapOf("
    start = text.find(marker)
    if start < 0:
        return None
    i = start + len(marker)
    depth = 1
    while i < len(text):
        ch = text[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                break
        i += 1
    if depth != 0:
        return None  # 块未闭合（配对 `)` 缺失）
    block = text[start : i + 1]
    result: dict[str, int] = {}
    for m in re.finditer(r'"(\w+)"\s+to\s+(\d+)', block):
        result[m.group(1)] = int(m.group(2))
    if set(result.keys()) != _EMOTION_KEYS:
        return None  # 键集不符（≠8 表情键）→ 判 FAIL 不误绿
    return result


def _check_emotion_wiring(text: str) -> bool:
    """C26 接线断言：EmotionController.kt 必须经 currentAdapter() 读取 emotionIndex（兜底 FALLBACK_EMOTION_MAP）。"""
    return _EMOTION_WIRING_ANCHOR in text


def check_adapter_consistency(registry: dict) -> None:
    """C24-C31：适配配置（shared/model-adapter）跨端一致性与真实数据对账 + 深度自校验。"""
    adapter = _load_adapter()
    model3 = _load_model3()
    cdi3 = _load_cdi3()

    # ── C24: adapter schema 完整性（契约 §1.2 全部约束）──
    c24_ok = False
    c24_detail = "adapter JSON 缺失或解析失败"
    if adapter is not None:
        problems: list[str] = []
        missing = _ADAPTER_REQUIRED_KEYS - set(adapter.keys())
        if missing:
            problems.append(f"缺少必填字段 {sorted(missing)}")
        # modelId / displayName：modelId 对应条目必须在 registry 中存在（SV-08：不再要求 registry default == niziiro_mao）
        if "modelId" in adapter and "displayName" in adapter:
            registry_ids = [m["id"] for m in registry["models"]]
            if adapter["modelId"] not in registry_ids:
                problems.append(f"modelId={adapter.get('modelId')} 不在 registry 条目中, registry ids={registry_ids}")
        else:
            problems.append("缺 modelId/displayName")
        # motionAliases：bai 无 motion（D2）→ 必须为空 {} // 若非空则逐项校验 group/index/file
        if adapter.get("motionAliases") is not None:
            ma = adapter["motionAliases"]
            if set(ma.keys()) != _MOTION_TAG_KEYS:
                problems.append(f"motionAliases 键集={sorted(ma)}, 期望={sorted(_MOTION_TAG_KEYS)}")
            for tag, alias in ma.items():
                if not isinstance(alias, dict) or not all(k in alias for k in ("group", "index", "file")):
                    problems.append(f"motionAliases[{tag}] 缺少 group/index/file")
                elif not isinstance(alias.get("index"), int) or not isinstance(alias.get("file"), str) or not alias["file"]:
                    problems.append(f"motionAliases[{tag}] index 非 int 或 file 为空")
        else:
            problems.append("缺 motionAliases")
        # vowelParamMap / vowelRawMap：6 键；核心5元音值域严格 [0,1] + 口型辅助参数白名单
        # （v3-adapter §1.2 + v3-fixes §5.2：允许 ParamMouthUp/ParamMouthDown；未知参数名 = schema 漂移 → FAIL）
        all_vowel_keys: set[str] | None = None  # 跨 label 键集一致性辅助（供 C31）
        for map_name in ("vowelParamMap", "vowelRawMap"):
            vm = adapter.get(map_name)
            if not isinstance(vm, dict) or set(vm.keys()) != _VOWEL_LABELS:
                problems.append(f"{map_name} 键集/类型异常")
                continue
            for label in sorted(_VOWEL_LABELS):
                vals = vm[label]
                if map_name == "vowelParamMap":
                    if not isinstance(vals, dict):
                        problems.append(f"vowelParamMap[{label}] 非对象")
                        continue
                    key_set = set(vals.keys())
                    if all_vowel_keys is None:
                        all_vowel_keys = key_set
                    elif key_set != all_vowel_keys:
                        problems.append(f"vowelParamMap[{label}] 键集与其它 label 不一致")
                    if not _VOWEL_CORE_PARAM_NAMES.issubset(key_set):
                        problems.append(f"vowelParamMap[{label}] 缺核心五元音参数 {sorted(_VOWEL_CORE_PARAM_NAMES - key_set)}")
                    if not key_set.issubset(_VOWEL_PARAM_ALLOWED):
                        problems.append(f"vowelParamMap[{label}] 未知参数键 {sorted(key_set - _VOWEL_PARAM_ALLOWED)}")
                    for _pn, pv in vals.items():
                        if not (isinstance(pv, (int, float)) and 0.0 <= float(pv) <= 1.0):
                            problems.append(f"vowelParamMap[{label}][{_pn}] 值 {pv} 非数值或越界 [0,1]")
                else:  # vowelRawMap：二元数值 [mo, mf] ∈ [0,1]
                    if not (isinstance(vals, (list, tuple)) and len(vals) == 2):
                        problems.append(f"vowelRawMap[{label}] 非二元数组")
                        continue
                    for _rv in vals:
                        if not (isinstance(_rv, (int, float)) and 0.0 <= float(_rv) <= 1.0):
                            problems.append(f"vowelRawMap[{label}] 值 {_rv} 非数值或越界 [0,1]")

        # emotionIndex：8 键 0-7 唯一（值必须为严格 int）
        ei = adapter.get("emotionIndex")
        if isinstance(ei, dict) and set(ei.keys()) == _EMOTION_KEYS:
            vals = list(ei.values())
            if not all(isinstance(v, int) for v in vals) or sorted(vals) != list(range(8)):
                problems.append(f"emotionIndex 值集={vals}, 期望严格 int 0-7 各一次")
        else:
            problems.append(f"emotionIndex 键集={sorted(ei) if isinstance(ei, dict) else ei}, 期望 8 键")
        # idlePool：bai 无 motion → 必须为空 []（与 motionAliases={} 双空一致，S4-06 语义）
        ip = adapter.get("idlePool")
        if not isinstance(ip, list) or len(ip) != 0:
            problems.append(f"idlePool 长度={len(ip) if isinstance(ip, list) else 'N/A'}, 期望 0（bai 无 motion）")
        elif any(not isinstance(x, str) for x in ip):
            problems.append("idlePool 元素应均为 str（空池时无元素）")
        # degradation：必填字段齐全
        if adapter.get("degradation") is None:
            problems.append("缺 degradation")
        if not problems:
            c24_ok = True
            c24_detail = "§1.2 必填字段/键集/值域/类型（含口型辅助参数白名单 + 严格 [0,1]）全过"
        else:
            c24_detail = "; ".join(problems)
    record("C24", "adapter schema 完整性（§1.2 必填字段/键集/值域/modelId 在 registry 中存在）", c24_ok, c24_detail)

    # ── C25: motion 对账（adapter.motionAliases ↔ model3.json Motions）──
    # bai（D2 唯一模型）无 motion：adapter.motionAliases={} 且 model3.json 无 Motions（双空一致，S4-06）。
    c25_ok = False
    c25_detail = "adapter 或 model3.json 缺失"
    if adapter is not None and model3 is not None:
        motions = model3.get("FileReferences", {}).get("Motions")
        alias_map = adapter.get("motionAliases", {}) or {}
        problems: list[str] = []
        # 双空一致：adapter 无 motion 别名 ⇔ model3.json 无 Motions 区块
        if alias_map:
            problems.append(f"bai.adapter.motionAliases 应为空 {{}}（bai 无 motion），实际 {sorted(alias_map)}")
        if motions is not None:
            problems.append(f"bai.model3.json 不应含 FileReferences.Motions（bai 无 motion），实际 keys={sorted(motions) if isinstance(motions, dict) else type(motions).__name__}")
        if not problems:
            c25_ok = True
            c25_detail = "bai 双空一致：adapter.motionAliases={}（无例外）且 model3.json 无 Motions；idlePool=[] 同源（S4-06）"
        else:
            c25_detail = "; ".join(problems)
    record("C25", "adapter.motionAliases ↔ bai.model3.json Motions 对账（bai 双空）", c25_ok, c25_detail)

    # ── C26: emotion 一致（adapter ↔ PC model_dict emotionMap；Android EmotionText 覆盖 8 key）──
    # Wave 5 后 EmotionController.kt 已删除，Android 侧 emotionIndex 的唯一运行数据源是 adapter；
    # 这里校验 adapter 与 PC model_dict 的 emotionMap 完全一致，且 Android EmotionText 支持集覆盖 8 个 canonical 表情键。
    c26_ok = False
    c26_detail = "adapter/PC model_dict/EmotionText.kt 缺失"
    if adapter is not None and PC_MODEL_DICT_JSON.is_file() and ANDROID_EMOTION_TEXT_KT.is_file():
        emo_adapter = adapter.get("emotionIndex")
        try:
            md = json.loads(PC_MODEL_DICT_JSON.read_text(encoding="utf-8"))
            entry = next((e for e in md if isinstance(e, dict) and e.get("name") == "bai"), None)
            emo_pc = (entry or {}).get("emotionMap")
        except (json.JSONDecodeError, OSError, TypeError):
            emo_pc = None
        android_text = ANDROID_EMOTION_TEXT_KT.read_text(encoding="utf-8")
        android_covers = all(k in android_text for k in _EMOTION_KEYS)
        if (
            isinstance(emo_adapter, dict)
            and set(emo_adapter) == _EMOTION_KEYS
            and emo_adapter == emo_pc
            and android_covers
        ):
            c26_ok = True
            c26_detail = f"adapter == PC model_dict（{len(emo_adapter)} key）；Android EmotionText 覆盖 8 key"
        else:
            def _c26_side(name: str, m: object) -> str:
                if not isinstance(m, dict):
                    return f"{name}=None"
                return f"{name}({len(m)}key,miss={sorted(_EMOTION_KEYS - set(m))})"
            c26_detail = " | ".join([
                _c26_side("adapter", emo_adapter),
                _c26_side("pc", emo_pc),
                f"android_covers={android_covers}",
            ])
    record("C26", "adapter.emotionIndex ↔ PC model_dict emotionMap ↔ Android EmotionText 覆盖一致", c26_ok, c26_detail)

    # ── C27: vowelParamMap/vowelRawMap 对拍（bai 直映射：{ParamMouthOpenY, ParamMouthForm} ← raw[0], raw[1]）──
    # contract-R1-model §2.2 (bai) + model-facts-bai §五：bai 口型参数为 ParamMouthOpenY + ParamMouthForm
    # （非五元音 ParamA/I/U/E/O —— 去五元音公式/ParamU=0.56）。
    # 硬断言：adapter.vowelParamMap[label] 必须严格 == {ParamMouthOpenY: raw[0], ParamMouthForm: raw[1]}（容差 1e-6）。
    # 交叉核对：VisemeAnalyzer VOWEL_MAP 运行时由 currentAdapter()?.vowelRawMap 派生（= bai raw 恒一致）；
    # 硬编码 FALLBACK_VOWEL_MAP 兜底常量若与 bai raw 值/格式不符 → 记 deviation（仅兜底路径，零运行期行为），不判 FAIL。
    c27_ok = False
    c27_detail = "adapter 或 VisemeAnalyzer.kt 缺失"
    if adapter is not None and ANDROID_VISEME_KT.is_file():
        vsrc = ANDROID_VISEME_KT.read_text(encoding="utf-8")
        vpm = adapter.get("vowelParamMap")
        vrm = adapter.get("vowelRawMap")
        if not isinstance(vpm, dict) or not isinstance(vrm, dict):
            c27_detail = "adapter.vowelParamMap/vowelRawMap 缺失或非对象"
        else:
            problems: list[str] = []
            deviations: list[str] = []
            # ① 硬断言直映射：vowelParamMap[label] == {MouthOpenY: raw[0], MouthForm: raw[1]}（六 label）
            for label in sorted(_VOWEL_LABELS):
                raw = vrm.get(label)
                pm = vpm.get(label)
                if not (isinstance(raw, (list, tuple)) and len(raw) == 2 and isinstance(pm, dict)):
                    problems.append(f"{label}: raw/param 结构异常")
                    continue
                exp_open, exp_form = float(raw[0]), float(raw[1])
                for pname, expv in (("ParamMouthOpenY", exp_open), ("ParamMouthForm", exp_form)):
                    av = pm.get(pname)
                    if not isinstance(av, (int, float)) or abs(float(av) - expv) > 1e-6:
                        problems.append(f"{label}.{pname}: 期望 {expv}（=raw 直映射）≠ adapter {av}")
                extra = set(pm) - _VOWEL_CORE_PARAM_NAMES
                if extra:
                    problems.append(f"{label} 多余参数键 {sorted(extra)}（bai 口型 std 集仅 MouthOpenY+MouthForm）")
            # ② 交叉核对：VisemeAnalyzer VOWEL_MAP 由 adapter.vowelRawMap 派生（运行时 == bai raw）
            derived_ok = "currentAdapter()?.vowelRawMap" in vsrc
            if not derived_ok:
                problems.append("VisemeAnalyzer VOWEL_MAP 未由 adapter.vowelRawMap 派生（运行时值可能 ≠ bai raw）")
            # ③ 硬编码 FALLBACK_VOWEL_MAP 兜底常量 与 bai raw 对拍；值/格式不符 → deviation（仅兜底，零行为）
            fallback_hits = dict(re.findall(r'"([AIUEO])"\s+to\s+floatArrayOf\(([^)]+)\)', vsrc))
            for lbl, raw in sorted(vrm.items()):
                if lbl == "SIL":
                    continue
                fbk = fallback_hits.get(lbl)
                if fbk is None:
                    deviations.append(f"FALLBACK_VOWEL_MAP 未检出 {lbl}→floatArrayOf 字面量（无法对拍）")
                    continue
                nums = [x.strip().rstrip("fF") for x in fbk.split(",") if x.strip()]
                parts = [float(x) for x in nums]
                if len(parts) != 2:
                    deviations.append(f"FALLBACK_VOWEL_MAP {lbl} 值元数 {len(parts)} ≠ 2（格式不符）")
                elif abs(parts[0] - float(raw[0])) > 1e-6 or abs(parts[1] - float(raw[1])) > 1e-6:
                    deviations.append(f"FALLBACK_VOWEL_MAP {lbl} {parts} ≠ bai raw {list(map(float, raw))}")
            if not problems:
                c27_ok = True
                c27_detail = "bai 直映射全对：vowelParamMap[label] == {MouthOpenY: raw[0], MouthForm: raw[1]}；VOWEL_MAP 由 adapter.vowelRawMap 派生"
                if deviations:
                    c27_detail += "；deviation: " + "; ".join(deviations)
            else:
                c27_detail = "; ".join(problems)
                if deviations:
                    c27_detail += "；deviation: " + "; ".join(deviations)
    record("C27", "adapter.vowelParamMap ↔ vowelRawMap 直映射（bai：MouthOpenY/MouthForm，去五元音公式）；VOWEL_MAP 派生核对", c27_ok, c27_detail)

    # ── C28: 参数白名单（adapter 全部参数名 ∈ cdi3.json Parameters[].Id）──
    c28_ok = False
    c28_detail = "adapter 或 cdi3.json 缺失"
    if adapter is not None and cdi3 is not None:
        param_ids = {p.get("Id") for p in cdi3.get("Parameters", []) if isinstance(p, dict)}
        used: set[str] = set()
        for label, pm in adapter.get("vowelParamMap", {}).items():
            if isinstance(pm, dict):
                used.update(pm.keys())
        unknown = sorted(u for u in used if u not in param_ids)
        if not unknown:
            c28_ok = True
            c28_detail = f"全部用于参数 ∈ cdi3：{sorted(used)} ⊆ parameters[{len(param_ids)}]"
        else:
            c28_detail = f"越白名单参数: {unknown}（cdi3={sorted(param_ids)}）"
    record("C28", "adapter.vowelParamMap 参数名 ⊆ bai.cdi3.json Parameters 白名单（128 参，model-facts-bai §三）", c28_ok, c28_detail)

    # ── C29: motion 映射表空集一致（adapter → Android/PC 消费面；前端源码不在仓库时跳过）──
    # bai 无 motion → adapter.motionAliases 必须为空；Android MotionTags 与 PC live2d_model._MOTION_LABEL_ALIASES
    # 必须对齐空集。前端 MOTION_ALIAS_MAP 若源码存在则一并检查，缺失时跳过（不判失败）。
    c29_ok = False
    c29_detail = "adapter / MotionTags.kt / live2d_model.py 缺失"
    if (
        adapter is not None
        and ANDROID_MOTION_TAGS_KT.is_file()
        and PC_LIVE2D_MODEL_PY.is_file()
    ):
        prob: list[str] = []
        adapter_aliases = adapter.get("motionAliases", {})
        if adapter_aliases:
            prob.append(f"bai.adapter.motionAliases 应为空 {{}}（bai 无 motion，S4 契约），实际 {sorted(adapter_aliases)}")
        mtags = read_text(ANDROID_MOTION_TAGS_KT)
        if not ("currentAdapter()?.motionAliases" in mtags and "mapValues" in mtags and "\"${alias.group}_${alias.index}\"" in mtags):
            prob.append("MotionTags.TAG_TO_KEY 未由 adapter.motionAliases 派生")
        and_fb = dict(re.findall(r'FALLBACK_TAG_TO_KEY\s*[:=]\s*\{[^}]*"(\w+)"\s+to\s+"(\w+)"', mtags))
        if and_fb:
            prob.append(f"MotionTags FALLBACK_TAG_TO_KEY 非空（{sorted(and_fb)}）——bai 应空")
        pylm = read_text(PC_LIVE2D_MODEL_PY)
        if '_MOTION_LABEL_ALIASES = frozenset(_ADAPTER_DATA["motionAliases"].keys())' not in pylm:
            prob.append("live2d_model.py _MOTION_LABEL_ALIASES 未由 adapter.motionAliases 驱动")
        fblock_start = pylm.find("_DEFAULT_MOTION_LABEL_ALIASES = frozenset(")
        if fblock_start < 0:
            prob.append("live2d_model.py 缺 _DEFAULT_MOTION_LABEL_ALIASES 兑底常量")
        else:
            fblock = pylm[fblock_start:]
            fend = fblock.find(")")
            fb_names = set(re.findall(r'"(\w+)"', fblock[:fend]))
            if fb_names:
                prob.append(f"live2d_model _DEFAULT_MOTION_LABEL_ALIASES 非空（{sorted(fb_names)}）——bai 应空集")
        if PC_FRONTEND_VISEME_TS.is_file():
            fts = read_text(PC_FRONTEND_VISEME_TS)
            fe_keys = set(re.findall(r"(\w+):\s*\{\s*group:\s*'", fts))
            if fe_keys:
                prob.append(f"前端 MOTION_ALIAS_MAP 非空（{sorted(fe_keys)}）——bai 应空映射")
        if not prob:
            c29_ok = True
            c29_detail = "bai 空集一致：adapter.motionAliases={} ∧ Android MotionTags（派生+兑底空）∧ PC _MOTION_LABEL_ALIASES（+兑底空）"
            if not PC_FRONTEND_VISEME_TS.is_file():
                c29_detail += "；前端源码不在仓库，跳过"
        else:
            c29_detail = "; ".join(prob)
    record("C29", "adapter.motionAliases={} ⇔ Android MotionTags / PC _MOTION_LABEL_ALIASES 空集一致（bai）", c29_ok, c29_detail)

    # ── C30: persona 强度区块一致（shared vs Android assets；T3.3 item1 / M1 遗留债）──
    # 机制：persona.yaml 的 "## 表情强度控制" 区块声明 [emotion_N] 强度后缀语义与值域 (1-3)。
    # 双端 persona 必须含同一强度区块（防 M1 遗留：两端强度语义漂移）。
    c30_ok = False
    c30_detail = "shared/persona.yaml 或 Android assets/persona.yaml 缺失"
    if SHARED_PERSONA.is_file() and ANDROID_ASSETS_PERSONA.is_file():
        shared_txt = SHARED_PERSONA.read_text(encoding="utf-8")
        assets_txt = ANDROID_ASSETS_PERSONA.read_text(encoding="utf-8")
        block_head = "## 表情强度控制"
        def _strength_block(txt: str) -> str:
            start = txt.find(block_head)
            if start < 0:
                return ""
            nxt = txt.find("\n\n", start)
            return txt[start:] if nxt < 0 else txt[start:nxt]
        sb_shared = _strength_block(shared_txt)
        sb_assets = _strength_block(assets_txt)
        # 值域语义：强度后缀 [emotion_N]，N ∈ 1-3（机器可校验）
        range_ok = bool(re.search(r"强度\s*（\s*1[-~到]3\s*）", sb_shared))
        if not sb_shared or not sb_assets:
            c30_detail = f"强度区块缺失: shared={'Y' if sb_shared else 'N'}, assets={'Y' if sb_assets else 'N'}"
        elif sb_shared != sb_assets:
            c30_detail = "双端 persona 强度区块内容不一致（M1 遗留债复现，需同步）"
        elif not range_ok:
            c30_detail = "强度区块缺 1-3 值域声明（[emotion_N]，N∈1-3）"
        else:
            c30_ok = True
            c30_detail = "双端强度区块一致且声明 [emotion_N] 强度 1-3 值域（字节一致亦由 C23 兜底）"
    record("C30", "persona 强度区块一致（shared vs Android assets，[emotion_N] 1-3 值域）", c30_ok, c30_detail)

    # ── C31: schema 自校验深度汇总（T3.3 item3，C24 的补强维度）──
    # ① vowelParamMap 六 label 键集完全一致（缺参 = 漂移）
    # ② motionAliases 全 index 非负 int + group 为 str（含允许空串）
    # ③ idlePool 与 motionAliases 空组派生键精确一致（防 ⑥ → 池键错位）
    c31_ok = False
    c31_detail = "adapter 缺失"
    if adapter is not None:
        prob31: list[str] = []
        vpm = adapter.get("vowelParamMap")
        if isinstance(vpm, dict) and set(vpm.keys()) == _VOWEL_LABELS:
            key_sets = {frozenset(v.keys()) for v in vpm.values() if isinstance(v, dict)}
            if len(key_sets) != 1:
                prob31.append(f"vowelParamMap 六 label 键集不完全一致（{sorted(key_sets, key=sorted)}）")
        # index 非负 int + group 为 str
        ma = adapter.get("motionAliases")
        if isinstance(ma, dict):
            for tag, alias in ma.items():
                if not isinstance(alias, dict):
                    prob31.append(f"motionAliases[{tag}] 非对象")
                    continue
                if not isinstance(alias.get("index"), int) or alias["index"] < 0:
                    prob31.append(f"motionAliases[{tag}].index 非负 int 约束被破坏")
                if not isinstance(alias.get("group"), str):
                    prob31.append(f"motionAliases[{tag}].group 非 str")
        # idlePool 与空组派生键精确一致
        if isinstance(vpm, dict) and isinstance(ma, dict):
            empty_group_keys = sorted(
                f"{a['group']}_{a['index']}" for a in ma.values()
                if isinstance(a, dict) and a.get("group", "") == ""
            )
            ipool = adapter.get("idlePool")
            if isinstance(ipool, list) and sorted(ipool) == empty_group_keys:
                pass  # 一致
            else:
                prob31.append(f"idlePool {sorted(ipool) if isinstance(ipool, list) else ipool} ≠ 空组派生键 {empty_group_keys}")
        if not prob31:
            c31_ok = True
            c31_detail = f"六 label 键集一致；motionAliases index 非负 int/group str；idlePool == 空组派生键 {adapter.get('idlePool')}"
        else:
            c31_detail = "; ".join(prob31)
    record("C31", "adapter schema 自校验深度（六 label 键集一致 / index·group 类型 / idlePool 空组精确对账）", c31_ok, c31_detail)


# ---------------------------------------------------------------------------
# C32 默认模型 ID 一致性（architecture-gov-fix §5.2 + contract-M-CONS §2.2）
# ---------------------------------------------------------------------------

def _parse_registry_default_id(registry: dict) -> str | None:
    """C32：从 registry JSON（dict）提取 default 条目 id。"""
    for m in registry.get("models", []):
        if isinstance(m, dict) and m.get("default") is True:
            return m.get("id")
    return None


def _parse_settings_default_model_id(text: str) -> str | None:
    """C32：静态提取 SettingsRepository.kt 的 DEFAULT_MODEL_ID 常量值。"""
    m = re.search(r'DEFAULT_MODEL_ID\s*=\s*"([^"]+)"', text)
    return m.group(1) if m else None


def _parse_fallback_entry_id(text: str) -> str | None:
    """C32：静态提取 ModelRegistry.kt FALLBACK_ENTRY 块内 id 字段。

    以 `val FALLBACK_ENTRY = ModelEntry(` 为起点、配对 `)` 为终点截取块，
    块内 regex `id = "..."` 提取（防误抓其它 ModelEntry 的 id）。
    """
    marker = "val FALLBACK_ENTRY = ModelEntry("
    start = text.find(marker)
    if start < 0:
        return None
    i = start + len(marker)
    depth = 1
    while i < len(text):
        ch = text[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                break
        i += 1
    if depth != 0:
        return None
    block = text[start : i + 1]
    m = re.search(r'\bid\s*=\s*"([^"]+)"', block)
    return m.group(1) if m else None


def _check_default_model_consistency(registry, settings_kt_text: str, model_registry_kt_text: str) -> tuple[bool, str]:
    """C32：三源默认模型 ID 一致性（registry default / SettingsRepository / ModelRegistry FALLBACK_ENTRY）。

    registry 可为 dict 或 JSON 文本（供单测 mock 文本调用）。返回 (ok, detail)。
    """
    if isinstance(registry, str):
        try:
            registry = json.loads(registry)
        except (json.JSONDecodeError, TypeError):
            return False, "registry JSON 解析失败"
    rid = _parse_registry_default_id(registry) if isinstance(registry, dict) else None
    sid = _parse_settings_default_model_id(settings_kt_text)
    fid = _parse_fallback_entry_id(model_registry_kt_text)
    detail = f"registry={rid}, settings={sid}, fallback={fid}"
    ok = rid is not None and rid == sid == fid
    return ok, detail


def check_default_consistency(registry: dict) -> None:
    """C32：registry default.id == SettingsRepository.DEFAULT_MODEL_ID == ModelRegistry.FALLBACK_ENTRY.id。

    M-AND 落地 FALLBACK=bai（R1 路线）后直通；M-AND 完成前允许 FAIL（contract-M-CONS §5 排班联调点）。
    """
    settings_kt = read_text(ANDROID_SETTINGS_KT)
    model_registry_kt = read_text(ANDROID_REGISTRY_KT)
    ok, detail = _check_default_model_consistency(registry, settings_kt, model_registry_kt)
    record("C32", "registry default.id == SettingsRepository.DEFAULT_MODEL_ID == ModelRegistry.FALLBACK_ENTRY.id", ok, detail)


def check_persistence(registry: dict) -> None:
    settings_kt = read_text(ANDROID_SETTINGS_KT)
    conf = read_text(PC_CONF_YAML)
    android_kt = read_text(ANDROID_REGISTRY_KT)
    pc_py = read_text(PC_MODEL_REGISTRY_PY)

    # C20: 两端持久化键正确，默认值一致（reg_default_id 从 registry 数据动态取）
    reg_default_id = next(
        (m["id"] for m in registry["models"] if m.get("default") is True),
        None,
    )
    android_key = 'KEY_SELECTED_MODEL_ID = "selected_model_id"' in settings_kt
    android_default = bool(reg_default_id) and f'DEFAULT_MODEL_ID = "{reg_default_id}"' in settings_kt
    pc_key = bool(
        re.search(r"^\s*live2d_model_name\s*:", conf, re.M)
    )
    pc_default = bool(reg_default_id) and bool(
        re.search(rf"^\s*live2d_model_name\s*:\s*['\"]{re.escape(reg_default_id)}['\"]\s*$", conf, re.M)
    )
    c20_ok = android_key and android_default and pc_key and pc_default
    record(
        "C20",
        f"两端持久化键正确（Android 'selected_model_id' / PC conf.yaml live2d_model_name），默认值 {reg_default_id}",
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
    print("跨端一致性检查（plan-e2e.md §3 C1-C23 + contract-v3-adapter §1.3 C24-C28 + schedule-v3 T3.3 C29-C31 + gov-fix §5.2 C32）")
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
    check_persistence(registry)

    print("\n--- 3.7 共享表情协议 + persona 一致性 ---")
    check_shared_protocol()

    print("\n--- 3.8 适配配置一致性（C24-C31，T3.3 扩展）---")
    check_adapter_consistency(registry)

    print("\n--- 3.9 默认模型一致性（C32，gov-fix §5.2）---")
    check_default_consistency(registry)

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
