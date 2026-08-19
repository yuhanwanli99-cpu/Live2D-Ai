#!/usr/bin/env python3
"""Live2D-Ai 冒烟测试预检脚本。

只做本地可自动检查的项，真机/桌面操作见 docs/verification/smoke-checklist.md。
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FAILED = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(f"[{'PASS' if ok else 'FAIL'}] {name}" + (f"  [{detail}]" if detail else ""))
    if not ok:
        FAILED.append(name)


def main() -> int:
    print("== Live2D-Ai smoke preflight ==")
    check("shared/persona.yaml exists", (ROOT / "shared" / "persona.yaml").is_file())
    check("PC conf.yaml exists", (ROOT / "Live2D-Ai-pc/open-llm-vtuber/conf.yaml").is_file())
    conf = ROOT / "Live2D-Ai-pc/open-llm-vtuber/conf.yaml"
    if conf.exists():
        text = conf.read_text(encoding="utf-8")
        check("PC ASR enabled", "asr_model: 'faster_whisper'" in text or "asr_model: \"faster_whisper\"" in text)
        check("PC faster-whisper model dir exists", (ROOT / "Live2D-Ai-pc/open-llm-vtuber/models/whisper").is_dir())
    check("Android APK exists", (ROOT / "Live2D-Ai-Android/app/build/outputs/apk/debug/app-debug.apk").is_file())
    check("PC frontend built", (ROOT / "Live2D-Ai-pc/open-llm-vtuber/frontend/index.html").is_file())

    print()
    print("Manual steps: see docs/verification/smoke-checklist.md")
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
