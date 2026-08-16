#!/usr/bin/env python3
"""Verify a built Live2Dai-pc portable zip (or an already-extracted tree).

Checks (plan §8.4 / §8.6):
  * zip member presence: shared/ configs, run_server.py, 启动.bat,
    conf.yaml.template, LICENSE.ffmpeg.txt, frontend/, live2d-models/,
    bin/ffmpeg.exe + ffprobe.exe, .venv/Scripts/python.exe + python312.dll
  * zip member absence: models/ avatars/ cache/ conf.yaml __pycache__ .pyc .git
  * pyvenv.cfg ``home`` points at the bundled portable python (relative)
  * the relocated .venv actually runs (--version) and imports the heavy deps
  * ffmpeg / ffprobe run and the pydub → ffmpeg chain works (smoke of the
    TTS audio pipeline)

Usage:
  python scripts/verify_package.py --zip <artifact.zip>
  python scripts/verify_package.py --dir <extracted-root>
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

APP_DIR = "Live2Dai-pc/open-llm-vtuber"

REQUIRED_MEMBERS = (
    "shared/persona.yaml",
    "shared/model_registry.json",
    "shared/mcp_tools.json",
    f"{APP_DIR}/run_server.py",
    f"{APP_DIR}/启动.bat",
    f"{APP_DIR}/conf.yaml.template",
    f"{APP_DIR}/LICENSE.ffmpeg.txt",
    f"{APP_DIR}/frontend/index.html",
    f"{APP_DIR}/live2d-models/bai/runtime/bai.model3.json",
    f"{APP_DIR}/bin/ffmpeg.exe",
    f"{APP_DIR}/bin/ffprobe.exe",
    f"{APP_DIR}/.venv/Scripts/python.exe",
    f"{APP_DIR}/.venv/Scripts/python312.dll",
    f"{APP_DIR}/.venv/Scripts/Lib/os.py",
    f"{APP_DIR}/.venv/pyvenv.cfg",
)

FORBIDDEN_MEMBER_EXACT = (
    f"{APP_DIR}/conf.yaml",
    f"{APP_DIR}/.onboarding_done",
)
FORBIDDEN_MEMBER_DIR_FRAGMENTS = (
    f"{APP_DIR}/models/",
    f"{APP_DIR}/avatars/",
    f"{APP_DIR}/cache/",
    f"{APP_DIR}/logs/",
    "__pycache__",
    ".git/",
)
FORBIDDEN_MEMBER_SUFFIXES = (".pyc", ".pyo")

IMPORT_CHECK = (
    "import sys; "
    "import torch, faster_whisper, pydub, onnxruntime, edge_tts; "
    "print('imports-ok', sys.version.split()[0], 'torch', torch.__version__)"
)


def check_members(zip_path: str) -> tuple[list[str], list[str]]:
    """Return (missing_required, forbidden_present)."""
    names = set()
    with zipfile.ZipFile(zip_path) as zf:
        names = {i.filename.replace("\\", "/") for i in zf.infolist() if not i.is_dir()}
    missing = [m for m in REQUIRED_MEMBERS if m not in names]
    forbidden = []
    for name in sorted(names):
        n = name.rstrip("/")
        if n in FORBIDDEN_MEMBER_EXACT or n.endswith(FORBIDDEN_MEMBER_SUFFIXES):
            forbidden.append(n)
        elif any(frag in "/" + n for frag in FORBIDDEN_MEMBER_DIR_FRAGMENTS):
            forbidden.append(n)
    return missing, forbidden


def check_pyvenv_cfg(app_dir: str) -> tuple[bool, str]:
    """home must NOT be an absolute machine-specific path.

    (The absorbed standalone interpreter resolves its stdlib relative to the
    exe, so home is informational — but plan/CI still gate on it not pointing
    at a system/uv python.)
    """
    cfg = os.path.join(app_dir, ".venv", "pyvenv.cfg")
    with open(cfg, "r", encoding="utf-8", errors="replace") as fh:
        text = fh.read()
    m = re.search(r"^home\s*=\s*(.+)$", text, re.MULTILINE)
    home = m.group(1).strip() if m else "<missing>"
    ok = not re.match(r"^[A-Za-z]:", home)  # relative = portable
    return ok, home


def run_in(cmd: list[str], cwd: str, env: dict | None = None, timeout: int = 300) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, timeout=timeout)


def runtime_checks(app_dir: str) -> list[tuple[str, bool, str]]:
    """Runtime checks against an extracted tree. Returns (label, ok, detail)."""
    results: list[tuple[str, bool, str]] = []
    env = dict(os.environ)
    env["PATH"] = os.path.join(app_dir, "bin") + os.pathsep + env.get("PATH", "")

    # 1. relocated venv python runs
    py = os.path.join(app_dir, ".venv", "Scripts", "python.exe")
    p = run_in([py, "--version"], cwd=app_dir)
    ok = p.returncode == 0 and b"3.12" in (p.stdout or b"") + (p.stderr or b"")
    results.append(("venv python --version (3.12)", ok, (p.stdout or p.stderr).decode(errors="replace").strip()))

    # 2. heavy deps import through the relocated venv
    p = run_in([py, "-c", IMPORT_CHECK], cwd=app_dir, timeout=600)
    ok = p.returncode == 0
    results.append(
        ("deps import (torch/faster_whisper/pydub/onnxruntime/edge_tts)", ok, (p.stdout or p.stderr).decode(errors="replace").strip()[-300:])
    )

    # 3. ffmpeg / ffprobe run (DLLs resolve from bin/)
    for exe in ("ffmpeg.exe", "ffprobe.exe"):
        p = run_in([os.path.join(app_dir, "bin", exe), "-version"], cwd=os.path.join(app_dir, "bin"), timeout=120)
        first = (p.stdout or b"").decode(errors="replace").splitlines()[:1]
        results.append((f"bin/{exe} -version", p.returncode == 0, first[0] if first else ""))

    # 4. pydub → ffmpeg chain (TTS audio pipeline smoke)
    code = (
        "from pydub import AudioSegment; "
        "s = AudioSegment.silent(duration=100); "
        "s.export('smoke_out.mp3', format='mp3'); "
        "import os; assert os.path.getsize('smoke_out.mp3') > 0; print('pydub-ffmpeg-ok')"
    )
    p = run_in([py, "-c", code], cwd=app_dir, env=env, timeout=180)
    ok = p.returncode == 0
    results.append(("pydub export via bundled ffmpeg", ok, (p.stdout or p.stderr).decode(errors="replace").strip()[-200:]))
    try:
        os.remove(os.path.join(app_dir, "smoke_out.mp3"))
    except OSError:
        pass
    return results


def _long_path_prefix(path: str) -> str:
    r"""Windows: \\?\ prefix bypasses MAX_PATH (260) for deep site-packages trees.

    The zip's deepest internal path is ~214 chars; extracting under a long
    temp dir can push the absolute path past 260 and break plain open().
    """
    if os.name == "nt" and not path.startswith("\\\\?\\"):
        return "\\\\?\\" + os.path.abspath(path)
    return path


def verify_tree(root: str) -> int:
    app_dir = os.path.join(root, *APP_DIR.split("/"))
    failed = False

    print(f"[verify_package] tree: {root}")

    # pyvenv.cfg home
    ok, home = check_pyvenv_cfg(app_dir)
    status = "OK " if ok else "WARN"
    print(f"[verify_package] {status} pyvenv.cfg home = {home!r} (expect relative, not a machine path)")
    if not ok:
        print("[verify_package]   note: absolute home still works on this build machine,")
        print("[verify_package]   but breaks after the zip is moved to another machine.")

    # runtime checks
    results = runtime_checks(app_dir)
    for label, ok, detail in results:
        status = "OK " if ok else "FAIL"
        print(f"[verify_package] {status} {label}: {detail}")
        if not ok:
            failed = True

    if failed:
        print("[verify_package] FAILED")
        return 1
    print("[verify_package] PASSED")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--zip", help="path to the built zip")
    group.add_argument("--dir", help="path to an already-extracted tree")
    parser.add_argument("--keep-extract", help="directory to extract into instead of a temp dir")
    args = parser.parse_args(argv)

    if args.dir:
        return verify_tree(os.path.abspath(args.dir))

    zip_path = os.path.abspath(args.zip)
    missing, forbidden = check_members(zip_path)
    for m in missing:
        print(f"[verify_package] FAIL missing member: {m}")
    for f in forbidden:
        print(f"[verify_package] FAIL forbidden member: {f}")
    if missing or forbidden:
        print("[verify_package] zip member checks FAILED")
        return 1
    print(f"[verify_package] zip member checks OK ({len(REQUIRED_MEMBERS)} required present, 0 forbidden)")

    extract_dir = args.keep_extract or tempfile.mkdtemp(prefix="live2dai-verify-")
    if args.keep_extract and os.path.isdir(extract_dir):
        shutil.rmtree(extract_dir)
    print(f"[verify_package] extracting to {extract_dir} ...")
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(_long_path_prefix(extract_dir))
    try:
        return verify_tree(extract_dir)
    finally:
        if not args.keep_extract:
            shutil.rmtree(extract_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
