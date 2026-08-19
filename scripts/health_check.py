#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Live2D-Ai 本地健康自检（无 Key / 无网络）。

检查项：
  1. shared 关键文件存在
  2. shared/persona.yaml 可解析
  3. shared/model_registry.json 是合法 JSON
  4. Android assets 中 persona/mcp_tools 与 shared 字节一致
  5. PC conf.yaml 存在且引用 shared/persona.yaml
  6. PC Live2D 模型目录/入口存在
  7. 根级生成物/日志不混入源码（提示性）

退出码：0 = 基础健康；1 = 有失败项。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FAILED = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "PASS" if ok else "FAIL"
    print(f"[{status}] {name}" + (f"  [{detail}]" if detail else ""))
    if not ok:
        FAILED.append(name)


def main() -> int:
    print("== Live2D-Ai health check (no key / no network) ==\n")

    # 1. shared 关键文件
    shared_files = [
        "persona.yaml",
        "model_registry.json",
        "model_dict.json",
        "mcp_tools.json",
        "emotion-protocol.md",
        "model-adapter/bai.adapter.json",
    ]
    for rel in shared_files:
        p = ROOT / "shared" / rel
        check(f"shared/{rel} exists", p.is_file(), str(p) if not p.exists() else "")

    # 2. persona.yaml 可解析
    persona = ROOT / "shared" / "persona.yaml"
    if persona.exists():
        try:
            import yaml  # type: ignore
            data = yaml.safe_load(persona.read_text(encoding="utf-8"))
            ok = isinstance(data, dict) and bool(data.get("name"))
            check("shared/persona.yaml parse + name", ok,
                  data.get("name") if isinstance(data, dict) else "not a mapping")
        except ImportError:
            text = persona.read_text(encoding="utf-8")
            check("shared/persona.yaml basic parse (no PyYAML)",
                  "name:" in text and "system_prompt:" in text)
        except Exception as exc:
            check("shared/persona.yaml parse", False, str(exc))
    else:
        check("shared/persona.yaml parse", False, "missing")

    # 3. model_registry.json 合法 JSON
    registry = ROOT / "shared" / "model_registry.json"
    if registry.exists():
        try:
            data = json.loads(registry.read_text(encoding="utf-8"))
            ok = isinstance(data, dict) and isinstance(data.get("models"), list) and len(data["models"]) > 0
            check("shared/model_registry.json valid JSON + models", ok,
                  f"{len(data.get('models', []))} models" if ok else "invalid shape")
        except Exception as exc:
            check("shared/model_registry.json valid JSON", False, str(exc))
    else:
        check("shared/model_registry.json valid JSON", False, "missing")

    # 4. Android assets 与 shared 字节一致
    sync_pairs = [
        ("persona.yaml", "persona.yaml"),
        ("mcp_tools.json", "mcp_tools.json"),
    ]
    for shared_rel, android_rel in sync_pairs:
        src = ROOT / "shared" / shared_rel
        dst = ROOT / "Live2D-Ai-Android" / "app" / "src" / "main" / "assets" / android_rel
        if src.exists() and dst.exists():
            check(f"Android assets/{android_rel} == shared/{shared_rel}",
                  src.read_bytes() == dst.read_bytes())
        else:
            check(f"Android assets/{android_rel} == shared/{shared_rel}",
                  False, "missing file")

    # 5. PC conf.yaml 引用 shared/persona.yaml
    conf = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "conf.yaml"
    if conf.exists():
        text = conf.read_text(encoding="utf-8")
        check("PC conf.yaml exists + load_persona_from_file",
              "load_persona_from_file" in text and "shared/persona.yaml" in text)
    else:
        check("PC conf.yaml exists + load_persona_from_file", False, "missing conf.yaml")

    # 6. PC Live2D 模型入口
    pc_model = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber" / "live2d-models" / "bai" / "runtime" / "bai.model3.json"
    check("PC live2d model bai.model3.json", pc_model.exists(), str(pc_model) if not pc_model.exists() else "")

    # 7. 提示性：根级不应有 build/cache/apk/log 混入
    # build/ 和 cache/ 是打包/运行时/测试会生成的目录，交给 clean.sh 清理，不判失败。
    clutter = [p for p in ROOT.iterdir() if p.name in {
        "s.log", "s2.log", "s_dl.log"
    } or p.name.startswith("Live2D-Ai-debug") and p.suffix == ".apk"]
    if clutter:
        check("root no build/cache/apk/log clutter", False,
              ", ".join(p.name for p in clutter))
    else:
        check("root no build/cache/apk/log clutter", True)

    print()
    if FAILED:
        print(f"FAILED ({len(FAILED)}): " + ", ".join(FAILED))
        return 1
    print("All local health checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
