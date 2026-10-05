#!/usr/bin/env python3
"""Conservative secret-pattern scan for the current repository's tracked tree.

扫描面（tracked tree，用 git ls-files 枚举）：Rust workspace（crates/、xtask/、
shared/、tests/、scripts/）、前端（shell/**，含 Dart）、文档（docs/**）与 CI 配置
（.github/**），外加 INCLUDED_TOP_FILES 里的顶层文件；SKIP_SUFFIXES 跳过二进制、
压缩包、字体与锁文件。

门禁位置：.github/workflows/secret-scan.yml —— 每个 PR 与 push main 都跑，
**没有 paths 过滤**（密钥可能落在 docs/**、.github/**、shell/**）。
本地等价命令：python3 scripts/check_public_secrets.py

**没有 Gitleaks**：本仓不存在任何 Gitleaks 配置，也没有覆盖完整历史的密钥扫描；
本脚本是红线「密钥不进仓库」目前在 CI 里的**唯一**自动化门禁。它的扫描面是
git ls-files 的**当前树**，历史提交不在其中。（2026-10-05 审计 F-0049-01：原先此处
写「GitHub CI 额外会跑 Gitleaks 覆盖完整历史」——那句话**不成立**，已删除。）

豁免是**行级**的（TEST_LINE_MARKERS / PLACEHOLDERS），刻意偏宽：测试里的假密钥
种子（sk-…-INJECTED 之类）必须不误报，否则门禁会因为噪声被关掉。代价是**与豁免
标记同一行的真密钥会被跳过**——这是已知取舍，收紧它属于另一条改动。
"""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# 仅扫描源码/文档/CI 相关前缀；构建产物（被 gitignore，不在 tracked tree）与
# 锁文件不扫。2026-10-05（F-0049-01）：补 shell/**（Dart）、docs/**、.github/** ——
# 此前这三处**不在扫描面内**，一个 sk-… 粘进 .dart 或文档可以静默进仓库。
INCLUDED_PREFIXES = (
    "crates/",
    "xtask/",
    "shared/",
    "tests/",
    "scripts/",
    "shell/",
    "docs/",
    ".github/",
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
    # 2026-10-06（审计 F-0616-01）：上面四条都只认「云服务商的密钥形状」，于是
    # scripts/deploy_android.sh 里写死的**个人设备锁屏密码**（一个 5 位数字，形如
    # 「口令变量 = 字面量」）与同一行的中文写法，既不在扫描面、也不在任何 CI
    # workflow 里，却随 v0.1.0-rc.1 的根提交进了公开历史。下面三条补的就是这一类：
    # **自己发明口令的赋值**（不是云厂商 key 形状）。文字口令与纯数字口令分开认；
    # 中文「密码[:：=]」单列一条（\b 对 CJK 不成立）。
    # 前缀刻意**不加** \b：PHONE_PASSWORD 这类写法里 `_` 与 `P` 之间没有词边界，
    # 加了 \b 就会漏掉最常见的那种命名（正是本次真凭据的写法）。
    # 误报控制：$ / { 开头（环境变量、模板占位）与 PLACEHOLDERS 一律跳过；
    # 实测在全部 tracked 文件上只命中那条真凭据（0 误报）。
    "hardcoded credential (quoted)": re.compile(
        r"(?i)(?:password|passwd|passphrase|pwd|pin)\s*[:=]\s*[\"'](?![\s\"']*[\$\{<])[^\"']{3,}[\"']"
    ),
    "hardcoded numeric credential": re.compile(
        r"(?i)(?:password|passwd|pwd|pin|密码)\s*[:=：]\s*[\"']?\d{4,8}[\"']?(?!\d)"
    ),
    # 值必须**像凭据**（4–8 位数字，或引号里的字面量）：
    # 文档里讨论「中文『密码:』」这种写法不该被判红（本轮实测过这个误报）。
    "hardcoded credential (cjk)": re.compile(
        r"密码\s*[:：=]\s*(?![\$\{])(?:\d{4,8}|[\"']{1}[^\"']{3,}[\"']{1})"
    ),
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
