#!/usr/bin/env python3
"""W-VERIFY-2A：`cargo test --list` 的**多重集差**（verifier 自写，v2）。

修正点：Running 行的格式是 `Running <kind> <相对路径> (<target/debug/deps/STEM-HASH>)`，
二进制标识必须取**括号里的 stem**（v1 错取了 `<相对路径>`，把所有 lib 目标都并成 `lib.rs`）。

用法: python3 list_diff2.py <before_raw.txt> <after_raw.txt> <before_tree_root>
"""
import os
import re
import sys
from collections import Counter, defaultdict

# cargo 的两种形态都要接：
#   "Running unittests src/lib.rs (target/debug/deps/xxx-hash)"      ← lib/bin/examples
#   "Running tests/pet_desktop_state.rs (target/debug/deps/xxx-hash)" ← 集成测试（**没有 kind 词**）
RUNNING = re.compile(
    r"^\s+(?:Running\s+(?:(?P<kind>unittests)\s+)?(?P<rel>\S+)|Doc-tests\s+(?P<rel2>\S+))"
    r"\s+\((?P<bin>\S+)\)"
)
TESTLINE = re.compile(r"^(?P<name>\S.*): test$")


def owner_map(tree_root):
    """integration/example 测试文件名 → 所属 crate 目录（用**目录位置**归属）。"""
    owners = {}
    for crate in os.listdir(os.path.join(tree_root, "crates")):
        for sub in ("tests", "examples", "benches"):
            d = os.path.join(tree_root, "crates", crate, sub)
            if not os.path.isdir(d):
                continue
            for root, _, files in os.walk(d):
                for f in files:
                    if f.endswith(".rs"):
                        owners[os.path.splitext(f)[0]] = crate
    d = os.path.join(tree_root, "xtask")
    for sub in ("tests", "examples"):
        p = os.path.join(d, sub)
        if os.path.isdir(p):
            for root, _, files in os.walk(p):
                for f in files:
                    if f.endswith(".rs"):
                        owners[os.path.splitext(f)[0]] = "xtask"
    return owners


def parse(path):
    out = Counter()
    bins = {}
    current = None
    for line in open(path, encoding="utf-8", errors="replace"):
        m = RUNNING.match(line)
        if m:
            stem = os.path.basename(m.group("bin")).rsplit("-", 1)[0]
            current = stem
            rel = m.group("rel") or m.group("rel2") or "?"
            kind = m.group("kind") or ("integration" if rel.startswith("tests/") else "?")
            bins[stem] = (kind, rel)
            continue
        m = TESTLINE.match(line.rstrip("\n"))
        if m and current is not None:
            out[(current, m.group("name"))] += 1
    return out, bins


def crate_of(stem, bins, owners):
    """binary stem → 归属（crate 名）。lib/bin 目标的 stem 通常就是 crate 的 target 名。"""
    if stem in owners:
        return owners[stem]
    norm = stem.replace("_", "-")
    for crate in set(owners.values()) | {
        "l2d", "l2d-wasm-demo", "live2d-ai-core", "live2d-ai-desktop", "live2d-ai-runtime",
        "live2d-ai-mod-system", "live2d-ai-mod-external-input", "live2d-ai-mod-persona",
        "live2d-ai-mod-template", "live2d-ai-mod-voice-input", "live2d-ai-mod-memory",
        "live2d-ai-mod-director", "xtask",
    }:
        if norm == crate.replace("-", "_") or norm == crate:
            return crate
    # lib/bin 目标的 stem 就是 package 名（下划线形式）⇒ 直接归一化成 crate 名。
    # 被删的 3 个 crate 在 before 树里没有 tests/ 目录，走不到 owners，必须靠这一条。
    if re.fullmatch(r"[a-z0-9_]+", stem):
        return stem.replace("_", "-")
    return "?(%s)" % stem


def main(before_path, after_path, before_root):
    owners = owner_map(before_root)
    before, bins_b = parse(before_path)
    after, bins_a = parse(after_path)
    removed = before - after
    added = after - before
    kept = before & after

    print("== 总体 ==")
    print("before 用例数 = %d   after 用例数 = %d   （二进制目标：before %d / after %d）"
          % (sum(before.values()), sum(after.values()), len(bins_b), len(bins_a)))
    print("removed=%d  added=%d  kept=%d" % (sum(removed.values()), sum(added.values()), sum(kept.values())))

    print("\n== 退出集合按 binary/crate 归类 ==")
    by_bin = defaultdict(list)
    for (stem, name), n in removed.items():
        by_bin[stem].append((name, n))
    total = 0
    for stem in sorted(by_bin):
        kind, rel = bins_b.get(stem, ("?", "?"))
        crate = crate_of(stem, bins_b, owners)
        cnt = sum(n for _, n in by_bin[stem])
        total += cnt
        print("  %-34s crate=%-30s 退出=%3d  (%s %s)" % (stem, crate, cnt, kind, rel))
    print("  合计退出 = %d" % total)

    print("\n== 退出集合逐条（crate :: binary :: name）==")
    for stem in sorted(by_bin, key=lambda s: (crate_of(s, bins_b, owners), s)):
        crate = crate_of(stem, bins_b, owners)
        for name, n in sorted(by_bin[stem]):
            print("  %-28s %-34s %s%s" % (crate, stem, name, "" if n == 1 else "  x%d" % n))

    print("\n== 新增集合（应为空）==")
    print("  （无）" if not added else "")
    for (stem, name), n in sorted(added.items()):
        print("  + %s::%s" % (stem, name))

    deleted_crates = {"live2d-ai-mod-local-llm", "live2d-ai-mod-wallpaper", "live2d-ai-mod-pet-desktop"}
    mainline = {k: v for k, v in removed.items() if crate_of(k[0], bins_b, owners) not in deleted_crates}
    print("\n== 主链集合差（不属于被删 crate 的 binary）==")
    print("  主链 removed = %d   %s" % (sum(mainline.values()), "✅ 0" if not mainline else "❌ 非 0"))
    for (stem, name), n in sorted(mainline.items()):
        print("  ! %s::%s" % (stem, name))

    # 逐 crate 计数
    per = Counter()
    for (stem, _), n in removed.items():
        per[crate_of(stem, bins_b, owners)] += n
    print("\n== 退出用例按 crate 计数 ==")
    for c, n in sorted(per.items()):
        print("  %-30s %d" % (c, n))

    guard = "web_api::cli_entry::tests::default_mods_manifest_enables_external_input_only"
    print("\n== 守卫测试（after 树）==")
    hits = [k for k in after if k[1] == guard]
    print("  '%s' → %s" % (guard, hits if hits else "❌ 未找到"))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
