#!/usr/bin/env python3
"""独立断链核验器（verifier 自写，非实施者产物）。

用法: python3 check_links.py <markdown-file> [<markdown-file> ...]
逐条输出：每个相对链接 → 解析后的绝对路径 → EXISTS / MISSING。
只认「相对链接」（不含 :// 、不以 # 开头、不以 mailto: 开头）；
http(s) 外部链接不判存在性（离线纪律另有门禁），只计数。
"""
import os, re, sys

LINK = re.compile(r'\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)')

def check(path):
    base = os.path.dirname(os.path.abspath(path))
    text = open(path, encoding='utf-8').read()
    # 去掉围栏代码块，避免把示例链接当链接
    text_nocode = re.sub(r'```.*?```', '', text, flags=re.S)
    total = missing = external = anchor = 0
    for m in LINK.finditer(text_nocode):
        target = m.group(1)
        total += 1
        if target.startswith('#') or target.startswith('mailto:'):
            anchor += 1
            continue
        if '://' in target:
            external += 1
            continue
        clean = target.split('#', 1)[0].split('?', 1)[0]
        if not clean:
            anchor += 1
            continue
        resolved = os.path.normpath(os.path.join(base, clean))
        ok = os.path.exists(resolved)
        if not ok:
            missing += 1
        print("  %-6s %-70s -> %s" % ("OK" if ok else "MISSING", target, resolved))
    print("[%s] links=%d missing=%d external=%d anchor_only=%d" %
          (path, total, missing, external, anchor))
    return missing

if __name__ == '__main__':
    rc = 0
    for p in sys.argv[1:]:
        rc += check(p)
    print("TOTAL_MISSING=%d" % rc)
    sys.exit(1 if rc else 0)
