#!/usr/bin/env python3
"""Download and bundle a portable (LGPL) ffmpeg + ffprobe into <target>/bin/.

Build source : BtbN/FFmpeg-Builds ``win64-lgpl-shared`` (LGPL-2.1+, no GPL parts)
URL (default): https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/
                ffmpeg-n7.1-latest-win64-lgpl-shared-7.1.zip
Extracted    : ffmpeg.exe, ffprobe.exe and the shared DLLs (av*.dll, sw*.dll,
               postproc-*.dll) — everything pydub / edge-tts needs for the
               TTS audio chain (AudioSegment.from_file -> ffmpeg -> wav).
Verification : run ``ffmpeg -version`` / ``ffprobe -version`` from bin/ and
               require exit code 0 (this also proves the DLLs resolve).

Compliance  : the LGPL license text ships next to bin/ as LICENSE.ffmpeg.txt
              (kept as a tracked repo file under open-llm-vtuber/).

Usage:
  python scripts/bundle_ffmpeg.py [--target <dir>] [--ffmpeg-url <url>]
                                  [--zip-cache <path>]
"""
from __future__ import annotations

import argparse
import io
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

DEFAULT_FFMPEG_URL = (
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/"
    "ffmpeg-n7.1-latest-win64-lgpl-shared-7.1.zip"
)

API_LATEST_URL = "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest"


def resolve_download_url(ffmpeg_url: str | None, log=print) -> str:
    """Return a working download URL.

    Uses the explicit ``ffmpeg_url`` if given.  Otherwise tries the documented
    ``latest`` URL first and, if that 404s, queries the GitHub API for the
    newest ``win64-lgpl-shared`` zip asset.
    """
    if ffmpeg_url:
        return ffmpeg_url
    if _url_exists(DEFAULT_FFMPEG_URL):
        return DEFAULT_FFMPEG_URL
    log("[bundle_ffmpeg] default URL unavailable, querying GitHub API ...")
    req = urllib.request.Request(API_LATEST_URL, headers={"User-Agent": "Live2D-Ai-bundle"})
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = resp.read()
    import json

    release = json.loads(data)
    assets = release.get("assets", [])
    picks = [
        a["browser_download_url"]
        for a in assets
        if a["name"].endswith(".zip") and "win64-lgpl-shared" in a["name"]
    ]
    if not picks:
        raise RuntimeError(
            "Could not locate a win64-lgpl-shared ffmpeg asset in the latest "
            "BtbN release; pass --ffmpeg-url to override."
        )
    picks.sort()
    return picks[-1]


def _url_exists(url: str) -> bool:
    req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": "Live2D-Ai-bundle"})
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status == 200
    except Exception:
        return False


def download_zip(url: str, dest_zip: str) -> str:
    """Download ``url`` into ``dest_zip`` and return the path."""
    print(f"[bundle_ffmpeg] downloading {url}")
    req = urllib.request.Request(url, headers={"User-Agent": "Live2D-Ai-bundle"})
    with urllib.request.urlopen(req, timeout=600) as resp, open(dest_zip, "wb") as fh:
        shutil.copyfileobj(resp, fh)
    size_mb = os.path.getsize(dest_zip) / (1024 * 1024)
    print(f"[bundle_ffmpeg] downloaded {size_mb:.1f} MB -> {dest_zip}")
    return dest_zip


def extract_binaries(zip_path: str, bin_dir: str) -> list[str]:
    """Extract ffmpeg.exe / ffprobe.exe / shared DLLs from the BtbN zip.

    The zip layout is ``<build-name>/bin/ffmpeg.exe`` etc.  We only take the
    ``bin/`` entries, which keeps the bundle free of docs/tools we don't need.
    """
    wanted = ("ffmpeg.exe", "ffprobe.exe")
    os.makedirs(bin_dir, exist_ok=True)
    extracted: list[str] = []
    with zipfile.ZipFile(zip_path) as zf:
        for info in zf.infolist():
            name = info.filename.replace("\\", "/")
            base = os.path.basename(name)
            if "/bin/" not in "/" + name:  # only entries under <build>/bin/
                continue
            if not (base.endswith(".dll") or base in wanted):
                continue
            if info.is_dir():
                continue
            out = os.path.join(bin_dir, base)
            with zf.open(info) as src, open(out, "wb") as dst:
                shutil.copyfileobj(src, dst)
            extracted.append(out)
    return extracted


def verify(bin_dir: str, exes=("ffmpeg.exe", "ffprobe.exe")) -> None:
    """Run both exes from bin/ and require exit code 0."""
    for exe in exes:
        path = os.path.join(bin_dir, exe)
        if not os.path.isfile(path):
            raise RuntimeError(f"[bundle_ffmpeg] missing {exe} in {bin_dir}")
        # cwd = bin_dir so the shared DLLs are found next to the exe
        proc = subprocess.run([path, "-version"], cwd=bin_dir, capture_output=True, timeout=120)
        if proc.returncode != 0:
            tail = (proc.stderr or proc.stdout or b"").decode(errors="replace")[-800:]
            raise RuntimeError(f"[bundle_ffmpeg] {exe} -version failed:\n{tail}")
        first = (proc.stdout or b"").decode(errors="replace").splitlines()[:2]
        print(f"[bundle_ffmpeg] OK: {exe} -> {first[0] if first else ''}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default=None, help="target app dir (default: repo open-llm-vtuber)")
    parser.add_argument("--ffmpeg-url", default=None, help="override the BtbN download URL")
    parser.add_argument("--zip-cache", default=None, help="reuse a previously downloaded zip")
    args = parser.parse_args(argv)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(script_dir)
    target = args.target or os.path.join(repo_root, "Live2D-Ai-pc", "open-llm-vtuber")
    bin_dir = os.path.join(target, "bin")

    if os.path.isfile(os.path.join(bin_dir, "ffmpeg.exe")) and os.path.isfile(
        os.path.join(bin_dir, "ffprobe.exe")
    ):
        print("[bundle_ffmpeg] bin/ffmpeg.exe + ffprobe.exe already present, verifying ...")
        verify(bin_dir)
        return 0

    url = resolve_download_url(args.ffmpeg_url)
    with tempfile.TemporaryDirectory(prefix="live2d-ai-ffmpeg-") as tmp:
        if args.zip_cache and os.path.isfile(args.zip_cache):
            zip_path = args.zip_cache
            print(f"[bundle_ffmpeg] reusing zip cache: {zip_path}")
        else:
            zip_path = os.path.join(tmp, "ffmpeg.zip")
            download_zip(url, zip_path)
        extracted = extract_binaries(zip_path, bin_dir)
        print(f"[bundle_ffmpeg] extracted {len(extracted)} files into {bin_dir}")
        verify(bin_dir)

    license_file = os.path.join(target, "LICENSE.ffmpeg.txt")
    if not os.path.isfile(license_file):
        print(
            f"[bundle_ffmpeg] WARN: {license_file} not found — LGPL compliance "
            "requires shipping it next to the bundled binaries."
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
