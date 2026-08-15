#!/usr/bin/env python3
"""M1-A3 边缘像素断言脚本 —— contract-R1-render.md §3 M1-A3（联合验证批次）

行为契约：
- 对基线截图（batch1 开工前 `verify/render_baseline_before.png`）与收尾截图
  （`verify/render_after.png`）采样角色轮廓外环 16px 区域的 RGB 残留
- 判据：平均 RGB 残留 < 8/255（非预乘白边特征 = 残留 > 40/255，修复后应显著下降）
- 输入截图背景须为黑/透明（契约 §3 M1-A3/§4 M1-A3 前置）
- 输出 JSON + 控制台；退出码：判据失败非 0（可被 pytest/CI/批次4 联合验证直接消费）

用法：
    python scripts/verify_edge_alpha.py verify/render_baseline_before.png verify/render_after.png
    python scripts/verify_edge_alpha.py --after verify/render_after.png --threshold 8
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# 契约锁定判据（M1-A3）：平均 RGB 残留 < 8/255 视为"白边消除"
DEFAULT_ROUND = 16          # 采样外环宽度（px）
DEFAULT_THRESHOLD = 8       # 残留上限（/255）
WHITE_FRINGE_SIGNATURE = 40 # 非预乘白边通常 > 40/255

BG_ESTIMATE_FRACTION = 0.05  # 背景估算取样：四角各取画布 5% 边带


def _bg_rgb(rgb: np.ndarray) -> np.ndarray:
    """从四角边带估算背景色（黑/深色），返回 3 元素 RGB。"""
    h, w, _ = rgb.shape
    n = max(2, int(min(h, w) * BG_ESTIMATE_FRACTION))
    corners = np.concatenate(
        [
            rgb[:n, :n].reshape(-1, 3),
            rgb[:n, -n:].reshape(-1, 3),
            rgb[-n:, :n].reshape(-1, 3),
            rgb[-n:, -n:].reshape(-1, 3),
        ],
        axis=0,
    )
    return corners.mean(axis=0)


def analyze(png_path: Path) -> dict:
    """采样角色轮廓外环，返回平均 RGB 残留与判定统计。

    自适应两条路径：
      1) 图像含透明 alpha 且存在透明区 → 直接在透明区外环采样（契约"透明区"字面语义）。
      2) 全不透明（Android screencap, 深色背景实测）→ 以角落估算背景，角色=与背景差异显著
         的前景 mask，采样恰好位于角色轮廓外、背景侧的 16px 环带；白边=环带偏亮（RGB 残留高）。
    归一化到 /255。
    """
    img = Image.open(png_path).convert("RGBA")
    rgb = np.asarray(img.convert("RGB"), dtype=np.float32)
    alpha = np.asarray(img.split()[3], dtype=np.float32)
    h, w, _ = rgb.shape

    transparent_px = int((alpha < 16).sum())

    if transparent_px > 0:
        # 路径 1：真实透明背景 → 采样透明区外环
        bg = _bg_rgb(rgb)
        nrm = rgb / 255.0
        bgn = bg / 255.0
        resid = np.abs(nrm - bgn).mean(axis=2)          # 每个像素相对背景的偏差
        transparent_mask = alpha < 16
        ring = _ring_mask(transparent_mask, DEFAULT_ROUND, h, w)
        ring_px = resid[ring]
        avg = float(ring_px.mean()) if ring_px.size else 0.0
        mode = "transparent-alpha"
        return _pack(mode, png_path, avg, int(ring_px.size), int(transparent_px))

    # 路径 2：全不透明（深色背景）→ 角色=背景前景，外环=角色轮廓外 16px
    bg = _bg_rgb(rgb)
    nrm = rgb / 255.0
    bgn = bg / 255.0
    resid = np.abs(nrm - bgn).mean(axis=2)               # 相对背景的多通道偏差
    silh = resid > min(0.10, max(0.035, bgn.mean() + 0.06))  # 前景（角色）mask：明显异于背景
    ring = _ring_mask(silh, DEFAULT_ROUND, h, w) & ~silh      # 角色轮廓外环（背景侧）
    # 环带内 RGB 相对背景的亮向残留（只取正向亮度——白边是变亮）
    r_ring, g_ring, b_ring = nrm[..., 0][ring], nrm[..., 1][ring], nrm[..., 2][ring]
    if ring.sum() == 0:
        mode, avg, count = "unknown-no-ring", 0.0, 0
    else:
        bright_bg = np.stack([bgn[0], bgn[1], bgn[2]])
        residual = np.maximum(
            np.array([r_ring.mean(), g_ring.mean(), b_ring.mean()]) - bright_bg,
            0.0,
        )
        avg = float(np.linalg.norm(residual))            # RGB 亮向残留范数
        count = int(ring.sum())
        mode = "opaque-dark-bg-edge-ring"
    return _pack(mode, png_path, avg, count, transparent_px)


def _ring_mask(mask: np.ndarray, radius: int, h: int, w: int) -> np.ndarray:
    """mask 向外扩张 radius 后的环带，不包含 mask 自身（8 邻域形态学近似）。"""
    if not mask.any():
        return np.zeros_like(mask, dtype=bool)
    cur = mask.copy()
    steps = max(1, int(radius / 2.0))       # 每步约 2px 扩张
    for _ in range(steps):
        padded = np.pad(cur, 1, mode="constant").astype(bool)
        nbr = (
            padded[:-2, :-2] | padded[:-2, 1:-1] | padded[:-2, 2:] |
            padded[1:-1, :-2] | padded[1:-1, 1:-1] | padded[1:-1, 2:] |
            padded[2:, :-2] | padded[2:, 1:-1] | padded[2:, 2:]
        )
        cur = cur | nbr[:h, :w]
    return cur & ~mask


def _pack(mode: str, png: Path, avg: float, ring_count: int, transparent_px: int) -> dict:
    return {
        "file": str(png),
        "mode": mode,
        "avg_rgb_residual": round(avg, 4),
        "avg_rgb_residual_scaled_255": round(avg * 255.0, 2),
        "ring_sample_px": ring_count,
        "transparent_px": transparent_px,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="M1-A3 边缘像素断言（contract-R1-render §3）")
    ap.add_argument("baseline", nargs="?", default=None, help="基线截图路径（expected 白边残留，留档）")
    ap.add_argument("after", nargs="?", default=None, help="收尾截图路径（assert 残留 < threshold）")
    ap.add_argument("--ring", type=int, default=DEFAULT_ROUND, help="外环采样宽度 px（默认 16）")
    ap.add_argument("--threshold", type=int, default=DEFAULT_THRESHOLD,
                    help="平均 RGB 残留上限 /255（默认 8，契约 M1-A3）")
    args = ap.parse_args()

    base = Path(".").resolve()
    after = Path(args.after).resolve() if args.after else base / ".pi/spoq/verify/render_after.png"
    baseline = Path(args.baseline).resolve() if args.baseline else \
        base / ".pi/spoq/verify/render_baseline_before.png"

    report = {"contract": "contract-R1-render.md §3 M1-A3",
              "ring_px": args.ring, "threshold_255": args.threshold}

    if baseline.is_file():
        ba = analyze(baseline)
        report["baseline"] = ba
        print("== edge-alpha (M1-A3) ==")
        print("baseline: %s mode=%s avg_resid=%.2f/255 (白边特征>%d/255)"
              % (ba["file"], ba["mode"], ba["avg_rgb_residual_scaled_255"], WHITE_FRINGE_SIGNATURE))
    else:
        print("[warn] 基线截图缺失（%s）——契约 §5 禁止旧图顶替，须本批次开工前现拍" % baseline)

    if not after.is_file():
        print("[FAIL] 收尾截图缺失（%s）——无法执行 M1-A3 断言（批次4 联合验证前置）" % after)
        return 2

    a = analyze(after)
    report["after"] = a
    ok = a["avg_rgb_residual_scaled_255"] < float(args.threshold)
    report["after_ok"] = ok
    print("after:    %s mode=%s avg_resid=%.2f/255 (阈值<%d/255) -> %s"
          % (a["file"], a["mode"], a["avg_rgb_residual_scaled_255"], args.threshold,
             "PASS" if ok else "FAIL"))

    out = base / ".pi/spoq/verify" / "edge_alpha_report.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("report: %s" % out)
    print("RESULT: %s" % ("PASS (exit 0)" if ok else "FAIL (exit 1)"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
