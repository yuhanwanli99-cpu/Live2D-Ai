#!/usr/bin/env python3
"""Conservative secret-pattern scan for the current PC public source tree.
GitHub CI additionally runs Gitleaks over full history.
"""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PREFIXES = ("Live2D-Ai-pc/open-llm-vtuber/", "shared/")
SKIP_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".zip", ".lock"}
PATTERNS = {
    "OpenAI-style key": re.compile(r"\bsk-[A-Za-z0-9_-]{20,}\b"),
    "GitHub token": re.compile(r"\b(?:ghp|github_pat)_[A-Za-z0-9_]{20,}\b"),
    "AWS access key": re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
}
PLACEHOLDERS = ("YOUR API KEY", "your key", "somethingelse", "${", "example")


def tracked_files() -> list[Path]:
    output = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return [ROOT / line for line in output.splitlines() if line.startswith(PREFIXES)]


def main() -> None:
    findings: list[str] = []
    for path in tracked_files():
        if not path.is_file() or path.suffix.lower() in SKIP_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            if any(marker.lower() in line.lower() for marker in PLACEHOLDERS):
                continue
            for label, pattern in PATTERNS.items():
                if pattern.search(line):
                    findings.append(f"{path.relative_to(ROOT)}:{line_number}: {label}")
    if findings:
        raise SystemExit("potential secrets found:\n" + "\n".join(findings))
    print("current PC public tree secret-pattern scan: ok")


if __name__ == "__main__":
    main()
