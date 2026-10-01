#!/usr/bin/env python3
"""两条 `#[cfg(test)]` 边界规则的对比复算（verifier 自写，只读）。

规则 A（现实现，measure.rs::inline_test_lines）：逐个 cfg(test) 条目做花括号配对；
  但**没有花括号的条目并不会吃到文件末尾**——block_end 会继续往后扫，
  在遇到第一行「含 `{` 且括号收支归零」的行（例如 `use serde::{Deserialize};`）时提前收尾。
规则 B（measure.rs 自己的文档口径）：「若该条目没有花括号（`#[cfg(test)] mod x;` /
  `use ...;`），则算到文件末尾」。

输出：两条规则下的 rust prod / inline，与 PLAN §2（prod 67,381 / inline 16,006，剔除 xtask）
逐条对照，并列出差异最大的文件。
"""
import importlib.util
import os

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("ri", os.path.join(HERE, "reimpl_code_stats.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

PLAN_PROD = 67381   # PLAN §2 67,806 减去其 xtask 425
PLAN_INLINE = 16006


def doc_rule_mask(text):
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    mask = [False] * len(lines)
    i = 0
    while i < len(lines):
        t = lines[i].strip()
        if t == "#[cfg(test)]" or t.startswith("#[cfg(test)]"):
            j, has_brace = i, False
            while j < len(lines):
                if "{" in lines[j]:
                    has_brace = True
                    break
                if ";" in lines[j]:
                    break
                j += 1
            end = m._block_end(lines, i) if has_brace else len(lines) - 1
            for k in range(i, end + 1):
                mask[k] = True
            i = end + 1
        else:
            i += 1
    return sum(mask)


def main():
    prod_a = inline_a = prod_b = inline_b = 0
    rows = []
    for n in sorted(os.listdir("crates")):
        d = os.path.join("crates", n)
        if not os.path.isdir(d):
            continue
        for f in m.collect_files(d):
            if not f.endswith(".rs") or m.rust_kind(os.path.relpath(f, d)) != "src":
                continue
            text = open(f, encoding="utf-8", errors="replace").read()
            total = m.physical_lines(text)
            a = m.inline_test_lines(text)
            b = doc_rule_mask(text)
            prod_a += total - a
            inline_a += a
            prod_b += total - b
            inline_b += b
            if b - a > 100:
                rows.append((b - a, total, a, b, os.path.relpath(f)))
    print("== Rust src（不含 xtask）==")
    print("规则A 现实现   : prod=%d inline=%d" % (prod_a, inline_a))
    print("规则B 文档口径 : prod=%d inline=%d" % (prod_b, inline_b))
    print("PLAN §2        : prod=%d inline=%d" % (PLAN_PROD, PLAN_INLINE))
    print("→ 规则A vs PLAN: prod %+d (%+.2f%%)" % (prod_a - PLAN_PROD, (prod_a - PLAN_PROD) / PLAN_PROD * 100))
    print("→ 规则B vs PLAN: prod %+d (%+.2f%%)" % (prod_b - PLAN_PROD, (prod_b - PLAN_PROD) / PLAN_PROD * 100))
    print()
    print("== 差异文件（文档口径多掩掉的行数 > 100）==")
    for delta, total, a, b, rel in sorted(rows, key=lambda r: -r[0]):
        print("   +%4d  total=%5d 规则A掩码=%5d 规则B掩码=%5d  %s" % (delta, total, a, b, rel))
    print("受影响文件数=%d  合计低估内联=%d 行" % (len(rows), sum(r[0] for r in rows)))


if __name__ == "__main__":
    main()
