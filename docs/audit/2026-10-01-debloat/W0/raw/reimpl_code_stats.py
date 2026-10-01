#!/usr/bin/env python3
"""code-stats 口径的**独立重实现**（verifier 自写）。

用途：不改 xtask 源码，按 `xtask/src/code_stats/measure.rs` 与 `collect.rs`
里**公开文档化的规则**在 Python 侧重算一遍，用来交叉核对
`cargo run -p xtask -- code-stats` 的数字（实现 bug / 聚合错会在这里露出来）。

规则（照抄自源码注释与实现，逐条对齐）：
- physical_lines: `\\n` 个数；非空且末尾无 `\\n` 补 1；空文件 0。
- inline_test_lines: 逐个 `#[cfg(test)]` 条目做花括号配对掩码（attr 行本身算内联）；
  无 `{` 的条目吃到文件末尾。
- rust_kind: 路径祖先目录含 `src` 才算 src；src 下祖先含 `tests` 或文件名为 `tests.rs`
  → SrcIntegration；src 外 → OtherIntegration。
- crate 集合 = `crates/*`（非隐藏子目录）+ `xtask`。
- 扫目录跳过：隐藏目录、target/build/dist/node_modules/.venv、符号链接。
- Rust prod = Σ(Src 文件 total − inline)；integ = Σ(SrcIntegration+OtherIntegration 的 total)。
- 超限清单对象：非 xtask 且非 OtherIntegration，按 `total` 行数。
- docs = `docs/**/*.md`（含 verifier 自己的 audit 目录，需要时手动扣除）。
"""
import os, sys

SKIPPED = {"target", "build", "dist", "node_modules", ".venv"}


def physical_lines(text):
    if text == "":
        return 0
    n = text.count("\n")
    return n if text.endswith("\n") else n + 1


def _brace_delta(line):
    return line.count("{") - line.count("}")


def _block_end(lines, start):
    depth = 0
    seen = False
    i = start
    while i < len(lines):
        depth += _brace_delta(lines[i])
        if "{" in lines[i]:
            seen = True
        if seen and depth <= 0:
            return i
        i += 1
    return max(len(lines) - 1, 0)


def inline_test_lines(text):
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    masked = [False] * len(lines)
    i = 0
    while i < len(lines):
        t = lines[i].strip()
        if t == "#[cfg(test)]" or t.startswith("#[cfg(test)]"):
            end = _block_end(lines, i)
            for j in range(i, end + 1):
                masked[j] = True
            i = end + 1
        else:
            i += 1
    return sum(1 for m in masked if m)


def collect_files(d):
    out = []
    if not os.path.isdir(d):
        return out
    stack = [d]
    while stack:
        cur = stack.pop()
        for name in sorted(os.listdir(cur)):
            p = os.path.join(cur, name)
            if os.path.islink(p):
                continue
            if os.path.isdir(p):
                if name.startswith(".") or name in SKIPPED:
                    continue
                stack.append(p)
            elif os.path.isfile(p):
                out.append(p)
    out.sort()
    return out


def rust_kind(rel_inside):
    parts = rel_inside.split(os.sep)
    if len(parts) < 2:
        return "other"
    file, dirs = parts[-1], parts[:-1]
    if "src" not in dirs:
        return "other"
    if "tests" in dirs or file == "tests.rs":
        return "src_integ"
    return "src"


def main(root):
    crates = []
    cd = os.path.join(root, "crates")
    if os.path.isdir(cd):
        for n in sorted(os.listdir(cd)):
            if os.path.isdir(os.path.join(cd, n)) and not n.startswith("."):
                crates.append((n, os.path.join(cd, n)))
    xt = os.path.join(root, "xtask")
    if os.path.isdir(xt):
        crates.append(("xtask", xt))

    prod = inline = integ = 0
    over500 = []
    over1000 = []
    per_crate = []
    for name, d in crates:
        cp = ci = cg = 0
        for f in collect_files(d):
            if not f.lower().endswith(".rs"):
                continue
            text = open(f, encoding="utf-8", errors="replace").read()
            total = physical_lines(text)
            il = inline_test_lines(text) if text else 0
            rel_inside = os.path.relpath(f, d)
            kind = rust_kind(rel_inside)
            rel = os.path.relpath(f, root)
            if name != "xtask" and kind != "other":
                row = (rel, total, total if kind == "src_integ" else il)
                if total > 500:
                    over500.append(row)
                if total > 1000:
                    over1000.append(row)
            if kind == "src":
                cp += total - il
                ci += il
            else:
                cg += total
        per_crate.append((name, cp, ci, cg))
        prod += cp
        inline += ci
        integ += cg

    dart_lib = sum(physical_lines(open(f, encoding="utf-8", errors="replace").read())
                   for f in collect_files(os.path.join(root, "shell/flutter/lib")) if f.endswith(".dart"))
    dart_test = sum(physical_lines(open(f, encoding="utf-8", errors="replace").read())
                    for f in collect_files(os.path.join(root, "shell/flutter/test")) if f.endswith(".dart"))
    dart_lib_files = len([f for f in collect_files(os.path.join(root, "shell/flutter/lib")) if f.endswith(".dart")])
    dart_test_files = len([f for f in collect_files(os.path.join(root, "shell/flutter/test")) if f.endswith(".dart")])

    docs_files = [f for f in collect_files(os.path.join(root, "docs")) if f.endswith(".md")]
    own = [f for f in docs_files if "2026-10-01-debloat" in f]
    docs_lines = sum(physical_lines(open(f, encoding="utf-8", errors="replace").read()) for f in docs_files)
    own_lines = sum(physical_lines(open(f, encoding="utf-8", errors="replace").read()) for f in own)

    print("== 独立重实现（code-stats 口径） ==")
    print("rust_prod=%d  rust_inline_test=%d  rust_integ=%d" % (prod, inline, integ))
    print("dart_lib=%d lines / %d files   dart_test=%d lines / %d files"
          % (dart_lib, dart_lib_files, dart_test, dart_test_files))
    print("docs=%d lines / %d files   （其中 verifier 自己的 audit 目录 %d files / %d lines）"
          % (docs_lines, len(docs_files), len(own), own_lines))
    print("docs_excluding_verifier_audit=%d lines / %d files" % (docs_lines - own_lines, len(docs_files) - len(own)))
    print("src .rs >500 = %d   >1000 = %d" % (len(over500), len(over1000)))
    print("-- >1000 --")
    for rel, total, _ in sorted(over1000, key=lambda r: -r[1]):
        print("   %6d  %s" % (total, rel))
    print("-- per-crate (prod/inline/integ) --")
    for n, p, i, g in per_crate:
        print("   %-32s %6d %6d %6d" % (n, p, i, g))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
