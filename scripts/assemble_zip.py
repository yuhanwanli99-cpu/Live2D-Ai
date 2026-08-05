#!/usr/bin/env python3
"""Assemble the Live2Dai-pc portable Windows zip (uv-venv-bundle).

Pipeline (mirrors CI job ``build-pc-windows`` in .github/workflows/build-upload.yml):

  1. Stage a pristine copy of ``shared/`` + ``Live2Dai-pc/open-llm-vtuber/``
     (runtime dirs / user config / dev venv excluded).
  2. ``uv python install 3.12`` + ``uv sync --python 3.12 --frozen`` → a
     complete ``.venv`` with ALL dependencies (torch, onnxruntime, ...).
  3. Absorb the uv-managed python-build-standalone INTO ``.venv/Scripts/``
     (python.exe, pythonw.exe, python312.dll, python3.dll, vcruntime DLLs,
     DLLs/, Lib/) so the venv entry point is the REAL relocatable
     interpreter — NOT uv's trampoline, which hard-codes the base python's
     absolute path in its binary and dies on machines without it
     (P0 portability fix, empirically verified with all Python installs
     hidden).  pyvenv.cfg ``home = .`` (informational — the standalone exe
     resolves its stdlib relative to itself).
  4. ``scripts/bundle_ffmpeg.py`` → ``bin/`` (ffmpeg.exe + ffprobe.exe + DLLs).
  5. ``scripts/check_resources.py`` → sentinel preflight on the staging tree.
  6. Zip everything, preserving the repo layout (zip root == repo root:
     ``shared/`` + ``Live2Dai-pc/open-llm-vtuber/``), excluding runtime
     dirs (models/ avatars/ cache/ logs/) and other non-shipping entries.
  7. ``scripts/verify_package.py`` on the produced zip.

Usage:
  python scripts/assemble_zip.py [--repo <root>] [--out <dir>] [--staging <dir>]
                                 [--skip-sync] [--no-ffmpeg] [--no-verify]
                                 [--ffmpeg-url <url>] [--python-version 3.12]
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

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

import bundle_ffmpeg  # noqa: E402
import check_resources  # noqa: E402
import verify_package  # noqa: E402

PYTHON_VERSION = "3.12"

# Runtime subset of the uv-managed python-build-standalone that is absorbed
# INTO .venv/Scripts/ so the venv entry point is the REAL standalone
# interpreter (not uv's trampoline — see P0 fix note below).  The standalone
# exe locates its stdlib strictly relative to the exe file, so Lib/ and DLLs/
# must sit next to it.  Lib/site-packages (bundled pip) is deliberately
# skipped — the venv has its own deps at .venv/Lib/site-packages.
STANDALONE_RUNTIME_ITEMS = (
    "python.exe",
    "pythonw.exe",
    "python312.dll",
    "python3.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "DLLs",
    "Lib",  # site-packages subdir excluded inside absorb_standalone_python()
)

# Top-level entries of open-llm-vtuber/ that are never copied into staging
# (recreated in the staging tree by later pipeline steps).
STAGING_TOP_EXCLUDES = frozenset(
    {".venv", "bin", "python", "models", "avatars", "cache", "logs", "build", "dist"}
)
# Excluded at every depth during staging copy.
STAGING_GLOBAL_EXCLUDES = frozenset({".git", "__pycache__", ".onboarding_done"})
STAGING_FILE_EXCLUDES = (".pyc", ".pyo")

# Zip-exclusion: runtime-generated dirs at the top level of open-llm-vtuber/
# only (NOT src/open_llm_vtuber/asr/models — silero_vad.onnx lives there and
# must ship).
ZIP_TOP_RUNTIME_DIRS = frozenset({"models", "avatars", "cache", "logs"})
ZIP_GLOBAL_EXCLUDES = frozenset({".git", "__pycache__", ".onboarding_done", "build", ".github"})
ZIP_FILE_EXCLUDES = (".pyc", ".pyo")
OPEN_LLM_PREFIX = "Live2Dai-pc/open-llm-vtuber/"


def copy_tree_filtered(src: str, dst: str) -> int:
    """Copy ``src`` → ``dst`` with the staging exclusion rules.

    Directory-name exclusions apply ONLY at the top level of the copied tree
    (deep dirs like ``src/open_llm_vtuber/asr/models`` are preserved).
    Returns the number of files copied.
    """
    os.makedirs(dst, exist_ok=True)
    count = 0
    for root, dirs, files in os.walk(src):
        rel = os.path.relpath(root, src)
        depth = 0 if rel == "." else rel.count(os.sep) + 1

        kept_dirs = []
        for d in sorted(dirs):
            if depth == 0 and d in STAGING_TOP_EXCLUDES:
                continue
            if d in STAGING_GLOBAL_EXCLUDES:
                continue
            kept_dirs.append(d)
        dirs[:] = kept_dirs

        for f in sorted(files):
            if f.endswith(STAGING_FILE_EXCLUDES):
                continue
            s = os.path.join(root, f)
            d = os.path.join(dst, rel, f) if rel != "." else os.path.join(dst, f)
            os.makedirs(os.path.dirname(d), exist_ok=True)
            shutil.copy2(s, d)
            count += 1
    return count


def run(cmd: list[str], cwd: str | None = None, timeout: int = 1800) -> subprocess.CompletedProcess:
    """Run a command, printing output live; raise on non-zero exit."""
    print(f"[assemble_zip] $ {' '.join(cmd)}" + (f"  (cwd={cwd})" if cwd else ""))
    proc = subprocess.run(cmd, cwd=cwd, timeout=timeout)
    if proc.returncode != 0:
        raise RuntimeError(f"command failed ({proc.returncode}): {' '.join(cmd)}")
    return proc


def ensure_managed_python(version: str) -> str:
    """Ensure a uv-managed python-build-standalone of ``version`` is installed.

    Returns the real (symlink-resolved) directory containing python.exe.
    """
    probe = subprocess.run(["uv", "python", "find", version], capture_output=True, text=True)
    if probe.returncode != 0:
        run(["uv", "python", "install", version])
    else:
        print(f"[assemble_zip] uv managed python {version} already installed")

    find = subprocess.run(["uv", "python", "find", version], capture_output=True, text=True, check=True)
    exe_path = find.stdout.strip()
    if not exe_path:
        raise RuntimeError(f"uv python find {version} returned nothing")
    real = os.path.realpath(exe_path)
    return os.path.dirname(real)


def absorb_standalone_python(base_dir: str, venv_dir: str) -> list[str]:
    """Make .venv/Scripts/ a self-contained portable Python (P0 fix).

    uv (Windows) creates .venv/Scripts/python.exe as a *trampoline* binary
    with the base interpreter's absolute path hard-coded inside it.  On a
    machine without that exact path the venv dies with "uv trampoline failed
    to spawn Python child process" — rewriting pyvenv.cfg home does NOT help
    (verified empirically; the trampoline ignores it).  Even copying just the
    real standalone python.exe over the trampoline is not enough: the
    standalone exe locates its stdlib relative to the exe file, so without
    Lib/ + DLLs/ next to it it falls back to a *registered system Python*
    (another fake-green on dev machines).

    This function copies the standalone interpreter's runtime subset INTO
    .venv/Scripts/ (python.exe, pythonw.exe, python312.dll, python3.dll,
    vcruntime DLLs, DLLs/, Lib/ minus site-packages), so .venv/Scripts/
    python.exe is the real, fully relocatable interpreter and works on a
    machine with no Python at all.  Returns the copied paths.
    """
    scripts = os.path.join(venv_dir, "Scripts")
    os.makedirs(scripts, exist_ok=True)
    copied: list[str] = []
    for item in STANDALONE_RUNTIME_ITEMS:
        src = os.path.join(base_dir, item)
        dst = os.path.join(scripts, item)
        if os.path.isdir(src):
            if os.path.isdir(dst):
                shutil.rmtree(dst)
            shutil.copytree(src, dst)
        elif os.path.isfile(src):
            shutil.copy2(src, dst)
        else:
            print(f"[assemble_zip] WARN: standalone runtime item missing: {src}")
            continue
        copied.append(dst)
    # bundled pip inside the standalone Lib/site-packages is redundant — drop it
    pip_sp = os.path.join(scripts, "Lib", "site-packages")
    if os.path.isdir(pip_sp):
        shutil.rmtree(pip_sp)
        print("[assemble_zip] removed standalone Lib/site-packages (bundled pip)")
    return copied


def rewrite_pyvenv_cfg(venv_dir: str, home: str) -> str:
    """Set ``home`` in ``<venv_dir>/pyvenv.cfg`` (CRLF, as uv writes it).

    Returns the new pyvenv.cfg content.
    """
    cfg_path = os.path.join(venv_dir, "pyvenv.cfg")
    with open(cfg_path, "rb") as fh:
        raw = fh.read().decode("utf-8")
    lines = []
    replaced = False
    for line in raw.splitlines():
        if re.match(r"^home\s*=", line):
            lines.append(f"home = {home}")
            replaced = True
        else:
            lines.append(line)
    if not replaced:
        lines.append(f"home = {home}")
    content = "\r\n".join(lines) + "\r\n"
    with open(cfg_path, "w", encoding="utf-8", newline="") as fh:
        fh.write(content)
    print(f"[assemble_zip] pyvenv.cfg home -> {home} ({cfg_path})")
    return content


def zip_excluded(relpath: str, is_dir: bool) -> bool:
    """Zip-exclusion predicate (pure, unit-tested). ``relpath`` uses '/'."""
    relpath = relpath.replace("\\", "/")
    name = relpath.rsplit("/", 1)[-1]
    if is_dir:
        if name in ZIP_GLOBAL_EXCLUDES:
            return True
        # top-level runtime dirs of open-llm-vtuber only
        if relpath.startswith(OPEN_LLM_PREFIX) and "/" not in relpath[len(OPEN_LLM_PREFIX):]:
            if name in ZIP_TOP_RUNTIME_DIRS:
                return True
        return False
    if name == "conf.yaml":
        return True  # user config — generated from conf.yaml.template on first run
    if name.endswith(ZIP_FILE_EXCLUDES):
        return True
    if name.endswith(".old") and ".venv/Lib/site-packages/torch/lib" in relpath:
        return True  # stale torch lib backups (plan §3.2 sketch)
    return False


def add_tree_to_zip(zf: zipfile.ZipFile, src_dir: str, arc_prefix: str) -> int:
    """Recursively add ``src_dir`` to the zip under ``arc_prefix`` (staging-relative)."""
    added = 0
    for root, dirs, files in os.walk(src_dir):
        rel_root = os.path.relpath(root, src_dir)
        for d in sorted(dirs):
            rel = rel_root.replace("\\", "/") if rel_root != "." else ""
            arc = f"{arc_prefix}/{rel}/{d}".replace("//", "/") if rel else f"{arc_prefix}/{d}"
            if zip_excluded(arc, is_dir=True):
                dirs.remove(d)
        for f in sorted(files):
            rel = rel_root.replace("\\", "/") if rel_root != "." else ""
            arc = f"{arc_prefix}/{rel}/{f}".replace("//", "/") if rel else f"{arc_prefix}/{f}"
            if zip_excluded(arc, is_dir=False):
                continue
            zf.write(os.path.join(root, f), arc, compress_type=zipfile.ZIP_DEFLATED, compresslevel=6)
            added += 1
    return added


def git_short_sha(repo_root: str) -> str:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--short=7", "HEAD"], cwd=repo_root, capture_output=True, text=True
        )
        sha = out.stdout.strip()
        if re.fullmatch(r"[0-9a-f]{7}", sha):
            return sha
    except Exception:
        pass
    return "local"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=None, help="repo root (default: script's parent)")
    parser.add_argument("--out", default=None, help="output dir for the zip (default: <repo>/build)")
    parser.add_argument("--staging", default=None, help="staging dir (default: <repo>/build/staging)")
    parser.add_argument("--skip-sync", action="store_true", help="reuse existing staging .venv")
    parser.add_argument("--no-ffmpeg", action="store_true", help="skip ffmpeg bundling")
    parser.add_argument("--no-verify", action="store_true", help="skip verify_package step")
    parser.add_argument("--ffmpeg-url", default=None, help="override ffmpeg download URL")
    parser.add_argument("--python-version", default=PYTHON_VERSION, help=f"managed python version (default {PYTHON_VERSION})")
    args = parser.parse_args(argv)

    repo = args.repo or os.path.dirname(SCRIPT_DIR)
    out_dir = args.out or os.path.join(repo, "build")
    staging = args.staging or os.path.join(out_dir, "staging")
    os.makedirs(out_dir, exist_ok=True)

    src_app = os.path.join(repo, "Live2Dai-pc", "open-llm-vtuber")
    src_shared = os.path.join(repo, "shared")
    dst_app = os.path.join(staging, "Live2Dai-pc", "open-llm-vtuber")
    dst_shared = os.path.join(staging, "shared")

    # --skip-sync: preserve the previous staging .venv + bin/ across the re-stage
    saved = None
    if args.skip_sync:
        old_venv = os.path.join(dst_app, ".venv")
        old_bin = os.path.join(dst_app, "bin")
        if not os.path.isdir(old_venv):
            raise RuntimeError(
                "--skip-sync requires an existing staging .venv; run the full "
                "pipeline once without --skip-sync first."
            )
        saved = tempfile.mkdtemp(prefix="live2dai-skip-sync-")
        shutil.move(old_venv, os.path.join(saved, ".venv"))
        if os.path.isdir(old_bin):
            shutil.move(old_bin, os.path.join(saved, "bin"))

    steps: list[str] = []

    # ── 1. stage copy ───────────────────────────────────────────────────────
    if os.path.isdir(staging):
        print(f"[assemble_zip] clearing staging {staging}")
        shutil.rmtree(staging)
    os.makedirs(dst_app, exist_ok=True)
    n_app = copy_tree_filtered(src_app, dst_app)
    n_shared = copy_tree_filtered(src_shared, dst_shared)
    print(f"[assemble_zip] staged {n_app} app files + {n_shared} shared files")
    steps.append("stage")

    # ── 2. uv sync → .venv (managed python 3.12) ────────────────────────────
    if not args.skip_sync:
        base_dir = ensure_managed_python(args.python_version)
        steps.append("uv-python")
        try:
            run(["uv", "sync", "--python", args.python_version, "--frozen"], cwd=dst_app, timeout=3600)
        except RuntimeError as exc:
            print(f"[assemble_zip] --frozen failed ({exc}); retrying without --frozen ...")
            run(["uv", "sync", "--python", args.python_version], cwd=dst_app, timeout=3600)
    else:
        base_dir = ensure_managed_python(args.python_version)
        print(f"[assemble_zip] --skip-sync: restoring previous staging .venv (saved under {saved})")
        shutil.copytree(os.path.join(saved, ".venv"), os.path.join(dst_app, ".venv"))
        if os.path.isdir(os.path.join(saved, "bin")):
            shutil.copytree(os.path.join(saved, "bin"), os.path.join(dst_app, "bin"))
        shutil.rmtree(saved, ignore_errors=True)
    steps.append("uv-sync")

    # ── 3. absorb the managed interpreter into .venv/Scripts (P0 fix) ──────
    venv_dir = os.path.join(dst_app, ".venv")
    copied = absorb_standalone_python(base_dir, venv_dir)
    print(
        f"[assemble_zip] absorbed {len(copied)} standalone runtime items into "
        f"{os.path.join(venv_dir, 'Scripts')}"
    )
    rewrite_pyvenv_cfg(venv_dir, ".")
    steps.append("python-base")

    # ── 4. portable ffmpeg → bin/ ────────────────────────────────────────────
    if not args.no_ffmpeg:
        rc = bundle_ffmpeg.main(["--target", dst_app] + (["--ffmpeg-url", args.ffmpeg_url] if args.ffmpeg_url else []))
        if rc != 0:
            raise RuntimeError("bundle_ffmpeg failed")
        steps.append("ffmpeg")
    else:
        print("[assemble_zip] --no-ffmpeg: skipped (bin/ expected pre-populated)")

    # ── 5. sentinel preflight ───────────────────────────────────────────────
    rc = check_resources.main(["--root", staging])
    if rc != 0:
        raise RuntimeError("resource sentinel check failed — refusing to build the zip")
    steps.append("check-resources")

    # ── 6. assemble the zip (repo-root semantics preserved) ─────────────────
    sha = git_short_sha(repo)
    zip_name = f"live2dai-pc-windows-{sha}.zip"
    zip_path = os.path.join(out_dir, zip_name)
    with tempfile.TemporaryDirectory(prefix="live2dai-zip-") as tmp:
        # Compress-Archive-style: zip the staging *contents* so the zip root
        # equals the repo root (contains shared/ and Live2Dai-pc/).
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
            added = add_tree_to_zip(zf, staging, "")
        print(f"[assemble_zip] wrote {zip_path} ({os.path.getsize(zip_path) / 1e6:.1f} MB, {added} files)")
    steps.append("zip")

    # ── 7. verify the artifact ──────────────────────────────────────────────
    if not args.no_verify:
        rc = verify_package.main(["--zip", zip_path])
        if rc != 0:
            raise RuntimeError("verify_package failed")
        steps.append("verify")

    size_mb = os.path.getsize(zip_path) / (1024 * 1024)
    print(f"\n[assemble_zip] DONE in {len(steps)} steps: {', '.join(steps)}")
    print(f"[assemble_zip] artifact: {zip_path} ({size_mb:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
