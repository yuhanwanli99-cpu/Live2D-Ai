#!/usr/bin/env python3
"""Finish an existing PC staging tree without re-running uv sync.

Use after `scripts/assemble_zip.py` has already populated
`build/staging/Live2Dai-pc/open-llm-vtuber/.venv` (e.g. when the full
pipeline was interrupted). It performs the remaining steps:

  1. absorb the standalone Python into .venv/Scripts
  2. rewrite pyvenv.cfg
  3. bundle ffmpeg
  4. run sentinel preflight
  5. write the Windows zip
  6. optional verify_package

Usage:
  python scripts/finish_pc_package.py [--staging build/staging] [--out build] [--no-verify]
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile
import zipfile

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

import assemble_zip as az  # noqa: E402
import bundle_ffmpeg  # noqa: E402
import check_resources  # noqa: E402
import verify_package  # noqa: E402

PYTHON_VERSION = az.PYTHON_VERSION


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=None)
    parser.add_argument("--staging", default=None)
    parser.add_argument("--out", default=None)
    parser.add_argument("--no-verify", action="store_true")
    parser.add_argument("--python-version", default=PYTHON_VERSION)
    args = parser.parse_args(argv)

    repo = args.repo or os.path.dirname(SCRIPT_DIR)
    out_dir = args.out or os.path.join(repo, "build")
    staging = args.staging or os.path.join(out_dir, "staging")
    dst_app = os.path.join(staging, "Live2Dai-pc", "open-llm-vtuber")
    venv_dir = os.path.join(dst_app, ".venv")

    if not os.path.isdir(venv_dir):
        raise RuntimeError(f"staging .venv not found: {venv_dir}")

    base_dir = az.ensure_managed_python(args.python_version)
    copied = az.absorb_standalone_python(base_dir, venv_dir)
    print(f"[finish_pc_package] absorbed {len(copied)} standalone runtime items")
    az.rewrite_pyvenv_cfg(venv_dir, ".")

    rc = bundle_ffmpeg.main(["--target", dst_app])
    if rc != 0:
        raise RuntimeError("bundle_ffmpeg failed")

    rc = check_resources.main(["--root", staging])
    if rc != 0:
        raise RuntimeError("resource sentinel check failed")

    sha = az.git_short_sha(repo)
    zip_name = f"live2dai-pc-windows-{sha}.zip"
    zip_path = os.path.join(out_dir, zip_name)
    with tempfile.TemporaryDirectory(prefix="live2dai-zip-") as _tmp:
        with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
            added = az.add_tree_to_zip(zf, staging, "")
        print(f"[finish_pc_package] wrote {zip_path} ({os.path.getsize(zip_path) / 1e6:.1f} MB, {added} files)")

    if not args.no_verify:
        rc = verify_package.main(["--zip", zip_path])
        if rc != 0:
            raise RuntimeError("verify_package failed")

    print(f"[finish_pc_package] DONE: {zip_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
