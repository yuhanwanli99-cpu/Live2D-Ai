#!/usr/bin/env python3
"""Build and validate a source-only PC Preview publication tree.

This script uses a strict whitelist approach: only Git-tracked files under the
PC, shared and top-level test trees are candidates, plus a small explicit allow
list for release/legal documents.  Everything else is ignored, and a final
assertion scan guarantees that no forbidden runtime/model/secret artifacts leak
into the published tree.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PC = ROOT / "Live2D-Ai-pc" / "open-llm-vtuber"

# Root-level legal / release documents that must be shipped.
TOP_FILES = (
    "README.md",
    "LICENSE",
    "NOTICE",
    "CREDITS.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "CODE_OF_CONDUCT.md",
    "COMMERCIAL-LICENSE.md",
    "LICENSE-SCOPE.md",
)

# Git-tracked prefixes that are eligible for inclusion.
TRACKED_PREFIXES = (
    "Live2D-Ai-pc/open-llm-vtuber/",
    "shared/",
    "tests/",
)

# Untracked files that are nonetheless intentionally part of the Preview.
EXPLICIT_ALLOW = {
    "docs/legal/pc-preview-publication.md",
    "docs/releases/v0.2.0-preview.1.md",
    "scripts/slop_miner.py",
    # PC one-click launcher (WSL2/Linux).  Explicitly allow-listed by exact path
    # so no OTHER Live2D-Ai-pc root files are pulled into the public tree.
    "Live2D-Ai-pc/start.sh",
}

# --- Exclusion predicates (used both for filtering the whitelist and for the
# final assertion scan). ------------------------------------------------------

FORBIDDEN_DIR_PARTS: frozenset[str] = frozenset(
    {
        ".git",
        "__pycache__",
        ".pytest_cache",
        ".ruff_cache",
        "dist",
        "build",
        ".venv",
        "venv",
        "node_modules",
        "cache",
        "chat_history",
        "logs",
        "tmp",
        "private",
        ".uv-cache",
        ".uv-python",
        ".hf-cache",
    }
)

FORBIDDEN_SUBSTRINGS: tuple[str, ...] = (
    "Live2D-Ai-Android/",
    "frontend/libs/",
    "renderer/public/libs/",
    "live2d-models/",
    "cache/",
    "chat_history/",
    "logs/",
    "tmp/",
    "private/",
    "node_modules/",
    ".venv/",
    ".uv-cache/",
    ".uv-python/",
    ".hf-cache/",
)

FORBIDDEN_NAMES: frozenset[str] = frozenset(
    {"conf.yaml", "external_input.json", "background_state.json", "live2dcubismcore.min.js"}
)

FORBIDDEN_SUFFIXES: frozenset[str] = frozenset(
    {".pyc", ".pyo", ".moc3", ".apk", ".aar", ".db", ".sqlite", ".sqlite3"}
)

MODEL_JSON_SUFFIXES: tuple[str, ...] = (
    ".model3.json",
    ".motion3.json",
    ".physics3.json",
    ".pose3.json",
    ".userdata3.json",
    ".exp3.json",
)

# --- Secret scanning ---------------------------------------------------------

SECRET_PATTERNS: dict[str, re.Pattern[str]] = {
    "OpenAI-style key": re.compile(
        r"(?<![A-Za-z0-9_-])sk-[A-Za-z0-9_-]{20,}(?![A-Za-z0-9_-])"
    ),
    "GitHub token": re.compile(
        r"\b(?:ghp|github_pat|gho|ghu|ghs)_[A-Za-z0-9_]{20,}\b"
    ),
    "AWS access key": re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    "private key": re.compile(
        r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |PRIVATE )?PRIVATE KEY-----"
    ),
}

SECRET_SKIP_SUFFIXES: frozenset[str] = frozenset(
    {
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".webp",
        ".ico",
        ".wav",
        ".mp3",
        ".ogg",
        ".zip",
        ".moc3",
        ".apk",
        ".aar",
        ".lock",
    }
)

SECRET_PLACEHOLDERS: tuple[str, ...] = (
    "YOUR API KEY",
    "your key",
    "user-supplied",
    "user-key",
    "somethingelse",
    "${",
    "example",
    "placeholder",
    "fake",
    "test",
)

# Local-machine absolute paths that must never leak into the public tree.
ABSOLUTE_LOCAL_PATH_PATTERNS: dict[str, re.Pattern[str]] = {
    "linux home": re.compile(r"/home/[A-Za-z0-9_.-]+/"),
    "wsl mount": re.compile(r"/mnt/[A-Za-z](?:/|$)"),
    "mac home": re.compile(r"/Users/[A-Za-z0-9_.-]+/"),
    "windows users": re.compile(r"[A-Za-z]:\\\\Users\\\\"),
}


def is_forbidden(relative: Path) -> bool:
    """Return True if *relative* (relative to the repo root or output root)
    matches any forbidden runtime/model/binary/internal pattern.
    """
    rel_str = relative.as_posix()
    name = relative.name
    lname = name.lower()

    if name.startswith(".env"):
        return True
    if any(part in FORBIDDEN_DIR_PARTS for part in relative.parts):
        return True
    if any(sub in rel_str for sub in FORBIDDEN_SUBSTRINGS):
        return True
    if relative.name in FORBIDDEN_NAMES:
        return True
    if any(lname.endswith(suf) for suf in MODEL_JSON_SUFFIXES):
        return True
    if relative.suffix.lower() in FORBIDDEN_SUFFIXES:
        return True
    return False


def git_tracked_files() -> set[str]:
    """Return the set of tracked file paths (POSIX) under the allowed prefixes."""
    try:
        output = subprocess.check_output(
            ["git", "ls-files"],
            cwd=ROOT,
            text=True,
        )
    except subprocess.CalledProcessError as exc:
        raise SystemExit(f"failed to list git tracked files: {exc}") from exc

    tracked: set[str] = set()
    for line in output.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith(TRACKED_PREFIXES) or line in TOP_FILES:
            tracked.add(line)
    return tracked


def allowed_files() -> set[str]:
    """Return the strict whitelist of source files to copy."""
    allowed = git_tracked_files()
    allowed.update(TOP_FILES)
    allowed.update(EXPLICIT_ALLOW)
    return allowed


def copy_allowed_files(target: Path) -> None:
    """Copy every allowed file into *target*, preserving directory layout."""
    for rel_str in sorted(allowed_files()):
        rel = Path(rel_str)
        if is_forbidden(rel):
            continue
        src = ROOT / rel
        if not src.is_file():
            # Untracked explicit-allow files may be missing; skip rather than fail.
            continue
        dst = target / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)


def assert_no_forbidden_files(target: Path) -> None:
    """Fail with SystemExit if the output tree contains forbidden paths."""
    forbidden: list[Path] = []
    for path in target.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(target)
        if is_forbidden(rel):
            forbidden.append(rel)
    if forbidden:
        raise SystemExit(
            "forbidden files/patterns found in Preview tree:\n"
            + "\n".join(f"  {p.as_posix()}" for p in forbidden)
        )


def scan_secrets(target: Path) -> list[str]:
    """Scan the output tree for common secret patterns.  Returns findings."""
    findings: list[str] = []
    for path in target.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() in SECRET_SKIP_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_number, line in enumerate(text.splitlines(), start=1):
            lowered = line.lower()
            if any(marker.lower() in lowered for marker in SECRET_PLACEHOLDERS):
                continue
            for label, pattern in SECRET_PATTERNS.items():
                if pattern.search(line):
                    findings.append(
                        f"{path.relative_to(target).as_posix()}:{line_number}: {label}"
                    )
    return findings


def assert_no_secrets(target: Path) -> None:
    """Fail with SystemExit if any potential secret is found in the output."""
    findings = scan_secrets(target)
    if findings:
        raise SystemExit(
            "potential secrets found in Preview tree:\n"
            + "\n".join(f"  {f}" for f in findings)
        )


def scan_local_paths(target: Path) -> list[str]:
    """Scan the output tree for local-machine absolute paths.  Returns findings.

    The ``tests/`` tree is skipped: test fixtures routinely contain path-shaped
    strings (``C:\\Users\\...``, ``/home/...``) that are intentionally not
    machine-specific leaks.  Every other shipped file must be clean.
    """
    findings: list[str] = []
    for path in target.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(target)
        if rel.parts and rel.parts[0] == "tests":
            continue
        if path.suffix.lower() in SECRET_SKIP_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_number, line in enumerate(text.splitlines(), start=1):
            lowered = line.lower()
            if any(marker.lower() in lowered for marker in SECRET_PLACEHOLDERS):
                continue
            for label, pattern in ABSOLUTE_LOCAL_PATH_PATTERNS.items():
                if pattern.search(line):
                    findings.append(
                        f"{rel.as_posix()}:{line_number}: {label}"
                    )
    return findings


def assert_no_local_paths(target: Path) -> None:
    """Fail with SystemExit if any absolute local path is found in the output."""
    findings = scan_local_paths(target)
    if findings:
        raise SystemExit(
            "absolute local paths found in Preview tree:\n"
            + "\n".join(f"  {f}" for f in findings)
        )


def validate(target: Path) -> None:
    """Run all post-build checks.  Any failure raises SystemExit."""
    assert_no_forbidden_files(target)
    assert_no_secrets(target)
    assert_no_local_paths(target)

    required = [
        target / "LICENSE",
        target / "NOTICE",
        target / "CHANGELOG.md",
        target / "LICENSE-SCOPE.md",
        target / "docs/legal/pc-preview-publication.md",
        target / "docs/releases/v0.2.0-preview.1.md",
        target / "Live2D-Ai-pc/open-llm-vtuber/pyproject.toml",
        target / "Live2D-Ai-pc/open-llm-vtuber/uv.lock",
        target / "Live2D-Ai-pc/open-llm-vtuber/renderer/package-lock.json",
        target / "Live2D-Ai-pc/open-llm-vtuber/renderer/src/main.ts",
        target / "Live2D-Ai-pc/open-llm-vtuber/renderer/tests/choreography.test.ts",
        target / "Live2D-Ai-pc/open-llm-vtuber/tests/test_config_reload.py",
        target / "shared/persona.yaml",
        target / "shared/validate_registry.py",
        target / "tests/test_pc_zero_config.py",
        target / "scripts/slop_miner.py",
        target / "Live2D-Ai-pc/start.sh",
    ]
    missing = [str(path.relative_to(target)) for path in required if not path.is_file()]
    if missing:
        raise SystemExit(
            "required Preview files missing:\n" + "\n".join(f"  {m}" for m in missing)
        )


def build(target: Path) -> Path:
    """Build the Preview tree, validate it, and create a zip archive."""
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True, exist_ok=True)

    copy_allowed_files(target)
    validate(target)

    archive = shutil.make_archive(
        str(target), "zip", root_dir=target.parent, base_dir=target.name
    )
    print(target)
    print(archive)
    return Path(archive)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build a strict-whitelist PC Preview source archive."
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "dist" / "Live2D-Ai-PC-v0.2.0-preview.1",
    )
    args = parser.parse_args()
    build(args.output.resolve())


if __name__ == "__main__":
    main()
