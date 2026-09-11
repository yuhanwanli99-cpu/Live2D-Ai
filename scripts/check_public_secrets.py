#!/usr/bin/env python3
"""Conservative secret-pattern scan for the current repository's tracked tree.

Rust 重构后只扫描 Rust workspace（crates/, xtask/）+ 顶层配置/脚本/测试，跳过
二进制、压缩包、锁文件。GitHub CI 额外会跑 Gitleaks 覆盖完整历史。
"""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# 仅扫描源码相关前缀；构建产物/锁文件/历史档案目录不在扫描范围。
INCLUDED_PREFIXES = (
    "crates/",
    "xtask/",
    "shared/",
    "tests/",
    "scripts/",
)
# 顶层文档/Markdown/CI 配置等。
INCLUDED_TOP_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "rustfmt.toml",
    ".gitignore",
    "README.md",
    "AGENTS.md",
    "ANDROID_ARCHIVE_POINTER.md",
}
SKIP_SUFFIXES = {
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".wav", ".mp3", ".ogg",
    ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".lock", ".ttf", ".otf",
    ".woff", ".woff2", ".mp4", ".mov", ".pdf",
}
PATTERNS = {
    "OpenAI-style key": re.compile(r"\bsk-[A-Za-z0-9_-]{20,}\b"),
    "GitHub token": re.compile(r"\b(?:ghp|github_pat)_[A-Za-z0-9_]{20,}\b"),
    "AWS access key": re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
}
PLACEHOLDERS = ("YOUR API KEY", "your key", "somethingelse", "${", "example")
# 测试代码里的假密钥（sk-* 测试种子、INJECTED 注入样例等）是常规做法，跳过：
# - 路径含 /tests/ 或 *_tests.rs / tests.rs
# - 行内含 "mod tests" / "#[test]" / "INJECTED" / "do-not-leak" / "super-secret" 等测试标记
TEST_PATH_MARKERS = ("/tests/", "tests.rs", "_tests.rs", "tests/")
TEST_LINE_MARKERS = (
    "mod tests",
    "#[test]",
    "#[cfg(test)]",
    "INJECTED",
    "do-not-leak",
    "super-secret",
    "test-secret",
    "sk-test",
)


def _is_test_path(rel: str) -> bool:
    return any(m in rel for m in TEST_PATH_MARKERS)


def _is_test_line(line: str) -> bool:
    return any(m.lower() in line.lower() for m in TEST_LINE_MARKERS)


def tracked_files() -> list[Path]:
    output = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    paths: list[Path] = []
    for line in output.splitlines():
        rel = line.strip()
        if not rel:
            continue
        if rel in INCLUDED_TOP_FILES:
            paths.append(ROOT / rel)
            continue
        if rel.startswith(INCLUDED_PREFIXES):
            paths.append(ROOT / rel)
    return paths


def main() -> None:
    findings: list[str] = []
    scanned = 0
    for path in tracked_files():
        if not path.is_file() or path.suffix.lower() in SKIP_SUFFIXES:
            continue
        scanned += 1
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            if any(marker.lower() in line.lower() for marker in PLACEHOLDERS):
                continue
            if _is_test_line(line) or _is_test_path(path.relative_to(ROOT).as_posix()):
                continue
            for label, pattern in PATTERNS.items():
                if pattern.search(line):
                    findings.append(f"{path.relative_to(ROOT)}:{line_number}: {label}")
    if findings:
        raise SystemExit("potential secrets found:\n" + "\n".join(findings))
    print(f"repo secret-pattern scan: ok ({scanned} files scanned)")


if __name__ == "__main__":
    main()
