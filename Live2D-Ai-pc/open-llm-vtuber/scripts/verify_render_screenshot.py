#!/usr/bin/env python3
"""S4 无头渲染验证脚本 —— contract-M-PC §2.4 / §3 A8/A9/A10

行为契约：
- 输入：页面 URL（默认 http://localhost:12393）、输出目录（默认 <repo-root>/.pi/spoq/model-previews/）
- 截 800×600 四帧：T0 / T+3s / T+6s / T+30s（系统 Edge/Chrome headless，SwiftShader 软渲染保 WebGL）
- PIL 分析每帧：非透明像素 bbox 比例 + 质心；相邻帧像素 diff 计数
- 输出 JSON {bbox_w_ratio, bbox_h_ratio, centroid_y_ratio, diff_frames:[...]} + PNG 留档
- 退出码：断言失败非 0（可被 pytest/vitest/CI 直接消费）

用法：
    python scripts/verify_render_screenshot.py                     # 修复后验证（断言全绿）
    python scripts/verify_render_screenshot.py --baseline          # 修复前基线（预期断言红，留档 before/）
    python scripts/verify_render_screenshot.py --url http://localhost:5173 --out-dir out/
"""

from __future__ import annotations

import argparse
import base64
import json
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

import websocket
from PIL import Image

SCREEN_W = 800
SCREEN_H = 600

# 帧时间线：T0 / T+3s / T+6s / T+30s（virtual-time-budget = 加载余量 + 帧偏移）
FRAME_OFFSETS_MS = [0, 3000, 6000, 30000]
BASE_LOAD_MS = 4000

ALPHA_THRESHOLD = 16        # 非透明判定阈值
DIFF_THRESHOLD = 16         # 相邻帧像素 diff 阈值（任一通道差）
SIGNIFICANT_RATIO = 0.005   # "显著变化"：变化像素 ≥ 屏幕像素 0.5%

# A8 断言阈值（contract-M-PC §3，800×600 设计值；S4 实测校准容差 ±20%）
BBOX_W_MIN, BBOX_W_MAX = 0.12, 0.30
BBOX_H_MIN, BBOX_H_MAX = 0.60, 0.95
CENTROID_MIN = 0.50          # 质心 y 在画布中线偏下（> 0.5）
SIGNIFICANT_CHANGES_MIN = 2  # A10：30s 窗口 ≥2 次显著变化（≥2 次眨眼）

REPO_ROOT = Path(__file__).resolve().parents[3]

EDGE_CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
]


def find_browser() -> Path:
    for cand in EDGE_CANDIDATES:
        p = Path(cand)
        if p.is_file():
            return p
    found = shutil.which("msedge") or shutil.which("chrome") or shutil.which("chromium")
    if found:
        return Path(found)
    raise RuntimeError(
        "未找到系统 Edge/Chrome（headless 截图不可用）——请安装 Edge 或指定浏览器路径"
    )


def capture(browser: Path, url: str, out_dir: Path, prefix: str) -> list[Path]:
    """CDP 真实时间截图四帧（T0/T+3s/T+6s/T+30s），动画真实运行（眨眼/呼吸可见）。

    单浏览器会话：加载后立即截 T0，随后真实等待 3s/3s/24s 截其余帧。
    若 CDP 不可用（缺 ws 依赖/端口冲突），回退到 --screenshot + --virtual-time-budget 模式。
    """
    try:
        return _cdp_capture(browser, url, out_dir, prefix)
    except Exception as exc:  # noqa: BLE001 —— 回退模式
        print("[warn] CDP 截图失败（%s），回退 virtual-time 模式" % exc)
        frames = []
        for i, offset in enumerate(FRAME_OFFSETS_MS):
            name = "t0" if offset == 0 else ("t%ds" % (offset // 1000))
            png = out_dir / ("%s-%s.png" % (prefix, name))
            _capture_flag(browser, url, png, BASE_LOAD_MS + offset)
            frames.append(png)
        return frames


def _capture_flag(browser: Path, url: str, out_png: Path, virtual_ms: int) -> None:
    """headless --screenshot 单帧（--default-background-color 保留透明通道）。"""
    out_png.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(browser),
        "--headless=new",
        "--disable-gpu",
        "--use-angle=swiftshader",
        "--enable-unsafe-swiftshader",
        "--hide-scrollbars",
        "--default-background-color=00000000",
        "--window-size=%d,%d" % (SCREEN_W, SCREEN_H),
        "--virtual-time-budget=%d" % virtual_ms,
        "--screenshot=%s" % str(out_png),
        url,
    ]
    proc = subprocess.run(cmd, capture_output=True, timeout=120)
    if not out_png.is_file():
        raise RuntimeError(
            "截图失败: %s\nstderr: %s" % (out_png, proc.stderr.decode(errors="replace")[-2000:])
        )


def _cdp_capture(browser: Path, url: str, out_dir: Path, prefix: str) -> list[Path]:
    """CDP 真实时间截图四帧（主路径）。"""
    out_dir.mkdir(parents=True, exist_ok=True)
    import random
    port = 9200 + random.randint(0, 400)
    proc = subprocess.Popen(
        [
            str(browser),
            "--headless=new",
            "--disable-gpu",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--hide-scrollbars",
            "--default-background-color=00000000",
            "--force-device-scale-factor=1",
            "--window-size=%d,%d" % (SCREEN_W, SCREEN_H),
            "--remote-debugging-port=%d" % port,
            "--remote-allow-origins=*",
            url,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        ws_url = None
        for _ in range(60):
            try:
                with urllib.request.urlopen("http://127.0.0.1:%d/json" % port, timeout=1) as resp:
                    targets = json.loads(resp.read().decode())
                page = next(t for t in targets if t.get("type") == "page")
                ws_url = page["webSocketDebuggerUrl"]
                break
            except Exception:
                if proc.poll() is not None:
                    raise RuntimeError("浏览器进程提前退出（CDP 不可用）")
                time.sleep(0.2)
        if ws_url is None:
            raise RuntimeError("无法连接 CDP /json")
        ws = websocket.create_connection(ws_url, timeout=15)
        _cdp_send(ws, "Page.enable")
        _cdp_send(ws, "Emulation.setDeviceMetricsOverride",
                  {"width": SCREEN_W, "height": SCREEN_H, "deviceScaleFactor": 1, "mobile": False})
        time.sleep(5)  # 真实等待页面 + 模型加载
        frames = []
        waits = [0, 3, 3, 24]  # T0 / +3s / +3s / +24s（累计 0/3/6/30s）
        for i, wait in enumerate(waits):
            if wait:
                time.sleep(wait)
            name = "t0" if FRAME_OFFSETS_MS[i] == 0 else ("t%ds" % (FRAME_OFFSETS_MS[i] // 1000))
            png = out_dir / ("%s-%s.png" % (prefix, name))
            msg = _cdp_send(ws, "Page.captureScreenshot", {"format": "png"})
            png.write_bytes(base64.b64decode(msg["result"]["data"]))
            frames.append(png)
        ws.close()
        return frames
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()


def _cdp_send(ws: websocket.WebSocket, method: str, params: dict | None = None) -> dict:
    req_id = _cdp_send.counter
    _cdp_send.counter += 1
    ws.send(json.dumps({"id": req_id, "method": method, "params": params or {}}))
    while True:
        msg = json.loads(ws.recv())
        if msg.get("id") == req_id:
            return msg


_cdp_send.counter = 1

def analyze(png_path: Path) -> dict:
    """PIL 分析单帧：非透明像素 bbox 比例 + 质心。"""
    im = Image.open(png_path).convert("RGBA")
    alpha = im.split()[3]
    mask = alpha.point(lambda a: 255 if a > ALPHA_THRESHOLD else 0)
    bbox = mask.getbbox()
    if bbox is None:
        return {"bbox_w_ratio": 0.0, "bbox_h_ratio": 0.0, "centroid_y_ratio": 0.5, "opaque_px": 0}
    w_ratio = bbox[2] / SCREEN_W
    h_ratio = bbox[3] / SCREEN_H
    # 质心 y：非透明像素的 alpha 加权平均 y
    px = im.load()
    sum_y = 0
    count = 0
    for y in range(bbox[1], bbox[3]):
        for x in range(bbox[0], bbox[2]):
            a = px[x, y][3]
            if a > ALPHA_THRESHOLD:
                sum_y += y * a
                count += a
    centroid_y_ratio = (sum_y / count) / SCREEN_H if count else 0.5
    return {
        "bbox_w_ratio": round(w_ratio, 4),
        "bbox_h_ratio": round(h_ratio, 4),
        "centroid_y_ratio": round(centroid_y_ratio, 4),
        "opaque_px": count,
    }


def diff_count(a_path: Path, b_path: Path) -> dict:
    """相邻帧像素 diff：任一通道差 > DIFF_THRESHOLD 的像素数。"""
    ia = Image.open(a_path).convert("RGBA")
    ib = Image.open(b_path).convert("RGBA")
    pa, pb = ia.load(), ib.load()
    changed = 0
    for y in range(SCREEN_H):
        for x in range(SCREEN_W):
            ca, cb = pa[x, y], pb[x, y]
            if any(abs(ca[i] - cb[i]) > DIFF_THRESHOLD for i in range(4)):
                changed += 1
    return {
        "pair": "%s->%s" % (a_path.stem, b_path.stem),
        "changed_px": changed,
        "changed_ratio": round(changed / (SCREEN_W * SCREEN_H), 4),
        "significant": changed >= SIGNIFICANT_RATIO * SCREEN_W * SCREEN_H,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="S4 无头渲染验证（contract-M-PC §2.4）")
    ap.add_argument("--url", default="http://localhost:12393", help="页面 URL（默认 http://localhost:12393）")
    ap.add_argument("--out-dir", default=str(REPO_ROOT / ".pi" / "spoq" / "model-previews"),
                    help="输出目录（JSON + PNG 留档）")
    ap.add_argument("--baseline", action="store_true",
                    help="基线模式：输出到 before/ 子目录并执行断言（预期红，留档修复前证据）")
    ap.add_argument("--browser", default="", help="浏览器可执行文件路径（默认自动探测 Edge/Chrome）")
    args = ap.parse_args()

    out_root = Path(args.out_dir)
    sub_dir = "before" if args.baseline else "after"
    out_dir = out_root / sub_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    browser = Path(args.browser) if args.browser else find_browser()

    # 1) 四帧截图（CDP 真实时间：动画运行）
    frames = capture(browser, args.url, out_dir, sub_dir)

    # 2) PIL 分析
    analysis = [analyze(p) for p in frames]
    diffs = [
        diff_count(frames[i], frames[i + 1])
        for i in range(len(frames) - 1)
    ]

    report = {
        "url": args.url,
        "mode": "baseline" if args.baseline else "verify",
        "screen": [SCREEN_W, SCREEN_H],
        "frames": [
            {"file": p.name, "offset_ms": FRAME_OFFSETS_MS[i], **analysis[i]}
            for i, p in enumerate(frames)
        ],
        "bbox_w_ratio": max(a["bbox_w_ratio"] for a in analysis),
        "bbox_h_ratio": max(a["bbox_h_ratio"] for a in analysis),
        "centroid_y_ratio": max(a["centroid_y_ratio"] for a in analysis),
        "diff_frames": diffs,
    }

    # 3) 断言（A8 + A10）
    w_ok = BBOX_W_MIN <= report["bbox_w_ratio"] <= BBOX_W_MAX
    h_ok = BBOX_H_MIN <= report["bbox_h_ratio"] <= BBOX_H_MAX
    c_ok = report["centroid_y_ratio"] > CENTROID_MIN
    d_ok = any(d["changed_px"] > 0 for d in diffs)
    sig_ok = sum(1 for d in diffs if d["significant"]) >= SIGNIFICANT_CHANGES_MIN
    assertions = {
        "A8_bbox_w_in_12_30": w_ok,
        "A8_bbox_h_in_60_95": h_ok,
        "A8_centroid_y_below_midline": c_ok,
        "A8_diff_frames_nonzero": d_ok,
        "A10_significant_changes_ge_2": sig_ok,
    }
    report["assertions"] = assertions
    passed = all(assertions.values())

    json_path = out_dir / ("report-%s.json" % sub_dir)
    json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    # 4) 控制台摘要
    print("== S4 render screenshot verify ==")
    print("mode: %s  url: %s" % (report["mode"], args.url))
    print("frames:")
    for f in report["frames"]:
        print("  %-28s bbox_w=%.3f bbox_h=%.3f centroid_y=%.3f" % (
            f["file"], f["bbox_w_ratio"], f["bbox_h_ratio"], f["centroid_y_ratio"]))
    print("max bbox_w_ratio=%.3f (期望 %.2f-%.2f) %s" % (report["bbox_w_ratio"], BBOX_W_MIN, BBOX_W_MAX, "OK" if w_ok else "FAIL"))
    print("max bbox_h_ratio=%.3f (期望 %.2f-%.2f) %s" % (report["bbox_h_ratio"], BBOX_H_MIN, BBOX_H_MAX, "OK" if h_ok else "FAIL"))
    print("max centroid_y_ratio=%.3f (期望 >%.2f) %s" % (report["centroid_y_ratio"], CENTROID_MIN, "OK" if c_ok else "FAIL"))
    print("diff_frames: %s" % json.dumps([(d["pair"], d["changed_px"], d["significant"]) for d in diffs]))
    print("assertions: %s" % json.dumps(assertions))
    print("report: %s" % json_path)
    print("RESULT: %s" % ("PASS (exit 0)" if passed else "FAIL (exit 1)"))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
