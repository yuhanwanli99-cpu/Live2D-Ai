#!/usr/bin/env python3
"""Sentinel-file preflight check for the Live2Dai-pc portable package.

Based purely on filesystem existence (``os.path.isfile``) — never on git
tracking state, so it works identically on a fresh CI checkout and on a
staging copy.

Usage:
  python scripts/check_resources.py [--root <repo-or-staging-root>]
"""
from __future__ import annotations

import argparse
import os
import sys

# (relative path, severity) — severity: "required" fails the build, "warn" doesn't
SENTINELS: tuple[tuple[str, str], ...] = (
    ("shared/persona.yaml", "required"),
    ("shared/model_registry.json", "required"),
    ("shared/mcp_tools.json", "required"),
    ("Live2Dai-pc/open-llm-vtuber/frontend/index.html", "required"),
    ("Live2Dai-pc/open-llm-vtuber/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json", "required"),
    ("Live2Dai-pc/open-llm-vtuber/bin/ffmpeg.exe", "required"),
    ("Live2Dai-pc/open-llm-vtuber/bin/ffprobe.exe", "required"),
)


def collect_missing(root: str) -> list[tuple[str, str]]:
    """Return [(relpath, severity)] for every sentinel that is missing."""
    missing = []
    for rel, severity in SENTINELS:
        if not os.path.isfile(os.path.join(root, rel)):
            missing.append((rel, severity))
    return missing


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=None, help="repo / staging root (default: script's repo)")
    args = parser.parse_args(argv)

    root = args.root or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    missing = collect_missing(root)

    failed = False
    for rel, severity in missing:
        flag = "REQUIRED" if severity == "required" else "warn"
        print(f"[check_resources] {flag}: missing {rel}")
        if severity == "required":
            failed = True

    if missing:
        print(f"[check_resources] {len(missing)} sentinel(s) missing under {root}")
    else:
        print(f"[check_resources] all {len(SENTINELS)} sentinels present under {root}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
