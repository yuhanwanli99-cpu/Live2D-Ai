#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Live2Dai 一键地基验证（无 Key 优先）。

用法：
  python scripts/verify_all.py            # 本地健康 + 根测试 + 一致性
  python scripts/verify_all.py --android  # 额外跑 Android JVM 单测
  python scripts/verify_all.py --pc       # 额外跑 PC pytest

退出码：0 = 全部通过；1 = 有失败。
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run(title: str, cmd: list[str], cwd: Path | None = None) -> bool:
    print(f"\n===== {title} =====")
    try:
        proc = subprocess.run(cmd, cwd=cwd or ROOT)
        ok = proc.returncode == 0
    except FileNotFoundError as exc:
        print(f"  command not found: {exc}")
        ok = False
    print(f"  -> {'PASS' if ok else 'FAIL'}")
    return ok


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--android", action="store_true", help="run Android JVM unit tests")
    parser.add_argument("--pc", action="store_true", help="run PC pytest")
    args = parser.parse_args()

    results = []
    results.append(run("health check", [sys.executable, "scripts/health_check.py"]))
    results.append(run("shared registry validation", [sys.executable, "shared/validate_registry.py"]))
    results.append(run("cross-platform consistency", [sys.executable, "shared/check_cross_platform_consistency.py"]))
    results.append(run("root pytest", [sys.executable, "-m", "pytest", "tests/", "-q"]))

    if args.android:
        gradlew = ROOT / "Live2Dai-Android" / "gradlew.bat" if sys.platform.startswith("win") else ROOT / "Live2Dai-Android" / "gradlew"
        results.append(run("Android JVM tests", [str(gradlew), ":app:testDebugUnitTest"], ROOT / "Live2Dai-Android"))

    if args.pc:
        py = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / ".venv" / "Scripts" / "python.exe"
        if not py.exists():
            py = ROOT / "Live2Dai-pc" / "open-llm-vtuber" / ".venv" / "bin" / "python"
        if not py.exists():
            py = Path(sys.executable)
        results.append(run("PC pytest", [str(py), "-m", "pytest", "Live2Dai-pc/open-llm-vtuber/tests/", "-q"]))

    failed = sum(1 for ok in results if not ok)
    print(f"\n===== summary: {len(results) - failed}/{len(results)} passed =====")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
