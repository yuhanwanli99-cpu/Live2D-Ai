"""Comparable colorfulness metric across light/dark UIs.

HSV saturation is meaningless on near-black pixels ((max-min)/max explodes for
tiny absolute differences), so measure ABSOLUTE chroma and deviation-from-neutral
in 0..255 units instead.
"""
import sys
import numpy as np
from PIL import Image

path, crop = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else None)
im = Image.open(path).convert("RGB")
if crop:
    l, t, r, b = [int(x) for x in crop.split(",")]
    im = im.crop((l, t, r, b))
a = np.asarray(im).astype(np.float32)
R, G, B = a[:, :, 0], a[:, :, 1], a[:, :, 2]
L = 0.2126 * R + 0.7152 * G + 0.0722 * B
mx, mn = a.max(axis=2), a.min(axis=2)
chroma = mx - mn                                   # absolute 0..255
dev = (np.abs(R - L) + np.abs(G - L) + np.abs(B - L)) / 3.0  # deviation from neutral
gam = L / 255.0                                    # gamma-encoded brightness

print(f"{path} {crop or 'full'}")
print(f"  brightness  mean={gam.mean():.3f}  p10={np.percentile(gam,10):.3f} p50={np.percentile(gam,50):.3f} p90={np.percentile(gam,90):.3f}")
print(f"  chroma      mean={chroma.mean():.1f}/255  p50={np.percentile(chroma,50):.1f} p90={np.percentile(chroma,90):.1f} p99={np.percentile(chroma,99):.1f}")
print(f"  neutral-dev mean={dev.mean():.1f}/255  p50={np.percentile(dev,50):.1f} p90={np.percentile(dev,90):.1f}")
print(f"  frac chroma>6 = {np.mean(chroma>6):.3f}   frac chroma>12 = {np.mean(chroma>12):.3f}")
# distinct tone ladder: cluster brightness into 4px buckets and report the biggest ones
h = (L // 4).astype(int)
vals, cnt = np.unique(h, return_counts=True)
order = np.argsort(-cnt)[:8]
print("  dominant brightness buckets (x4, share): " +
      ", ".join(f"{vals[i]*4}-{vals[i]*4+3}:{100*cnt[i]/h.size:.1f}%" for i in order))
