#!/usr/bin/env python3
"""零依赖 PNG 统计（stdlib only：zlib + struct）。

为什么自己写：本机没有 PIL / numpy（也不许 pip 装），而本轮的浏览器验收
证据（截图）必须用像素说话——不做像素统计就只能"看截图说没问题"，
那正是本项目最重的教训（自检说谎比没有自检更坏）。

用法：
  python3 png_stats.py IMG.png [--crop l,t,r,b] [--json]
  python3 png_stats.py IMG.png --colors ff0000,0000ff --tol 24 [--json]
  python3 png_stats.py A.png --diff B.png [--json]

输出（--json 时）：
  {w,h,pixels, brightness:{mean,p10,p50,p90}, chroma:{mean,p50,p90},
   dominant:[{hex,share}...], colors:[{hex,share}...], diff:{other,fraction,differing}}
"""
import json
import struct
import sys
import zlib


def read_png(path):
    data = open(path, 'rb').read()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise SystemExit('not a png: %s' % path)
    pos, idat, plte = 8, b'', None
    w = h = bd = ct = None
    while pos + 8 <= len(data):
        (ln,) = struct.unpack('>I', data[pos:pos + 4])
        typ = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + ln]
        pos += 12 + ln
        if typ == b'IHDR':
            w, h, bd, ct, _comp, _filt, inter = struct.unpack('>IIBBBBB', body)
            if inter:
                raise SystemExit('interlaced png unsupported')
            if bd != 8:
                raise SystemExit('bit depth %d unsupported' % bd)
        elif typ == b'IDAT':
            idat += body
        elif typ == b'PLTE':
            plte = body
        elif typ == b'IEND':
            break
    raw = zlib.decompress(idat)
    ch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ct]
    stride = w * ch
    out = bytearray()
    prev = bytearray(stride)
    p = 0
    for _y in range(h):
        f = raw[p]
        p += 1
        line = bytearray(raw[p:p + stride])
        p += stride
        if f == 1:
            for i in range(ch, stride):
                line[i] = (line[i] + line[i - ch]) & 255
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                a = line[i - ch] if i >= ch else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i - ch] if i >= ch else 0
                b = prev[i]
                c = prev[i - ch] if i >= ch else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        out += line
        prev = line
    px = bytearray(w * h * 3)
    if ct in (2, 6):
        for i in range(w * h):
            px[i * 3] = out[i * ch]
            px[i * 3 + 1] = out[i * ch + 1]
            px[i * 3 + 2] = out[i * ch + 2]
    elif ct in (0, 4):
        for i in range(w * h):
            v = out[i * ch]
            px[i * 3] = v
            px[i * 3 + 1] = v
            px[i * 3 + 2] = v
    elif ct == 3:
        for i in range(w * h):
            idx = out[i]
            px[i * 3:i * 3 + 3] = plte[idx * 3:idx * 3 + 3]
    return w, h, px


def clamp_crop(w, h, crop):
    if not crop:
        return 0, 0, w, h
    l, t, r, b = [int(x) for x in crop.split(',')]
    return max(0, l), max(0, t), min(w, r), min(h, b)


def stats(w, h, px, crop):
    l, t, r, b = clamp_crop(w, h, crop)
    n = 0
    bright = []
    chroma = []
    hist = {}
    sr = sg = sb = 0
    for y in range(t, b):
        row = y * w * 3
        for x in range(l, r):
            i = row + x * 3
            R, G, B = px[i], px[i + 1], px[i + 2]
            L = 0.2126 * R + 0.7152 * G + 0.0722 * B
            bright.append(L)
            chroma.append(max(R, G, B) - min(R, G, B))
            sr += R; sg += G; sb += B
            key = (((R >> 4) << 8) | ((G >> 4) << 4) | (B >> 4))
            hist[key] = hist.get(key, 0) + 1
            n += 1
    bright.sort()
    chroma.sort()

    def pct(a, p):
        return a[min(len(a) - 1, int(len(a) * p))] if a else 0.0

    dom = sorted(hist.items(), key=lambda kv: -kv[1])[:8]
    return {
        'pixels': n,
        # 逐通道均值（0..255）+ redness：用来判「背景图到底有没有上屏」——
        # 舞台 iframe 在无头截图里恒为白，背景验证只能落在**壳那一半**上。
        'channels': {'r': round(sr / n, 2), 'g': round(sg / n, 2), 'b': round(sb / n, 2)},
        'redness': round((sr - (sg + sb) / 2) / n, 2),
        'brightness': {
            'mean': round(sum(bright) / n / 255.0, 4),
            'p10': round(pct(bright, .10) / 255.0, 4),
            'p50': round(pct(bright, .50) / 255.0, 4),
            'p90': round(pct(bright, .90) / 255.0, 4),
        },
        'chroma': {
            'mean': round(sum(chroma) / n, 2),
            'p50': round(pct(chroma, .50), 1),
            'p90': round(pct(chroma, .90), 1),
        },
        'dominant': [
            {'hex': '%02x%02x%02x' % (((k >> 8) & 15) * 17, ((k >> 4) & 15) * 17, (k & 15) * 17),
             'share': round(c / n, 4)} for k, c in dom
        ],
    }


def color_shares(w, h, px, crop, colors, tol):
    """命中数 + **实测均值色**（observed）+ 精确命中数（exact）。

    为什么要 observed：只报"目标色有没有出现"无法回答"渲染出来的到底是不是它"。
    observed 是命中像素的 RGB 均值——它就是屏幕上真实画出来的那个颜色，
    亮度级差必须用它算，而不是用目标常量算（否则等于自证）。
    """
    l, t, r, b = clamp_crop(w, h, crop)
    targets = []
    for c in colors:
        c = c.strip().lstrip('#')
        targets.append((c, int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16)))
    hits = {c: 0 for c, _r, _g, _b in targets}
    exact = {c: 0 for c, _r, _g, _b in targets}
    sums = {c: [0, 0, 0] for c, _r, _g, _b in targets}
    n = 0
    for y in range(t, b):
        row = y * w * 3
        for x in range(l, r):
            i = row + x * 3
            R, G, B = px[i], px[i + 1], px[i + 2]
            for c, tr, tg, tb in targets:
                if abs(R - tr) <= tol and abs(G - tg) <= tol and abs(B - tb) <= tol:
                    hits[c] += 1
                    if R == tr and G == tg and B == tb:
                        exact[c] += 1
                    s = sums[c]
                    s[0] += R
                    s[1] += G
                    s[2] += B
            n += 1
    out = []
    for c, _r, _g, _b in targets:
        h_ = hits[c]
        obs = None
        if h_:
            s = sums[c]
            obs = '%02x%02x%02x' % (round(s[0] / h_), round(s[1] / h_), round(s[2] / h_))
        out.append({'hex': c, 'share': round(h_ / n, 4) if n else 0.0, 'hits': h_,
                    'exact': exact[c], 'observed': obs})
    return out


def diff(w, h, px, other):
    w2, h2, px2 = read_png(other)
    if (w, h) != (w2, h2):
        return {'other': other, 'fraction': None, 'note': 'size mismatch %dx%d vs %dx%d' % (w, h, w2, h2)}
    n = w * h
    d = 0
    for i in range(n):
        j = i * 3
        if abs(px[j] - px2[j]) + abs(px[j + 1] - px2[j + 1]) + abs(px[j + 2] - px2[j + 2]) > 12:
            d += 1
    return {'other': other, 'fraction': round(d / n, 4), 'differing': d, 'pixels': n}


def main(argv):
    path = argv[1]
    crop = None
    colors = None
    tol = 16
    other = None
    as_json = '--json' in argv
    i = 2
    while i < len(argv):
        if argv[i] == '--crop':
            crop = argv[i + 1]; i += 2
        elif argv[i] == '--colors':
            colors = argv[i + 1].split(','); i += 2
        elif argv[i] == '--tol':
            tol = int(argv[i + 1]); i += 2
        elif argv[i] == '--diff':
            other = argv[i + 1]; i += 2
        else:
            i += 1
    w, h, px = read_png(path)
    out = {'file': path, 'w': w, 'h': h, 'crop': crop or 'full'}
    out.update(stats(w, h, px, crop))
    if colors:
        out['colors'] = color_shares(w, h, px, crop, colors, tol)
    if other:
        out['diff'] = diff(w, h, px, other)
    if as_json:
        print(json.dumps(out, ensure_ascii=False))
    else:
        print('%s %s %dx%d' % (path, out['crop'], w, h))
        print('  brightness mean=%.3f p10=%.3f p50=%.3f p90=%.3f' % (
            out['brightness']['mean'], out['brightness']['p10'],
            out['brightness']['p50'], out['brightness']['p90']))
        print('  chroma mean=%.1f p50=%.1f p90=%.1f' % (
            out['chroma']['mean'], out['chroma']['p50'], out['chroma']['p90']))
        print('  dominant: ' + ', '.join('%s:%.1f%%' % (d['hex'], d['share'] * 100) for d in out['dominant']))
        for c in out.get('colors', []):
            print('  color %s share=%.4f' % (c['hex'], c['share']))
        if 'diff' in out:
            print('  diff %s' % json.dumps(out['diff']))
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
