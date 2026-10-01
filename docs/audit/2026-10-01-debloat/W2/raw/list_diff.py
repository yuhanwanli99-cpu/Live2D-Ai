#!/usr/bin/env python3
"""W-VERIFY-2A：两棵归档树上 `cargo test --list` 的**多重集差**（verifier 自写）。

不采信实施者的「64 条」口径：自己解析 before/after 两份原始输出，
以 (binary_stem, test_name) 为键做多重集差，然后归类到 crate。

用法: python3 list_diff.py before_raw.txt after_raw.txt
"""
import re
import sys
from collections import Counter, defaultdict

RUNNING = re.compile(r"^\s+Running\s+(?P<kind>\S+)\s+(?P<path>\S+)")
TESTLINE = re.compile(r"^(?P<name>\S.*): test$")


def parse(path):
    """→ {(binary_stem, test_name): count}，另存 binary_stem → kind/path。"""
    out = Counter()
    meta = {}
    current = None
    for line in open(path, encoding="utf-8", errors="replace"):
        m = RUNNING.match(line)
        if m:
            raw = m.group("path")
            stem = raw.split("/")[-1].split("-")[0]
            current = stem
            meta[stem] = (m.group("kind"), raw)
            continue
        m = TESTLINE.match(line.rstrip("\n"))
        if m and current is not None:
            out[(current, m.group("name"))] += 1
    return out, meta


def main(before_path, after_path):
    before, meta_b = parse(before_path)
    after, meta_a = parse(after_path)
    removed = before - after
    added = after - before
    kept = before & after

    print("== 总体 ==")
    print("before 用例数（含重复计）= %d   目标数=%d" % (sum(before.values()), len(meta_b)))
    print("after  用例数（含重复计）= %d   目标数=%d" % (sum(after.values()), len(meta_a)))
    print("removed=%d  added=%d  kept=%d" % (sum(removed.values()), sum(added.values()), sum(kept.values())))

    print("\n== 退出集合按 binary 归类 ==")
    by_bin = defaultdict(list)
    for (stem, name), n in removed.items():
        by_bin[stem].append((name, n))
    for stem in sorted(by_bin):
        kind, path = meta_b.get(stem, ("?", "?"))
        print("  %-46s kind=%-14s 退出=%d" % (stem, kind, sum(n for _, n in by_bin[stem])))
    print("  合计退出 = %d" % sum(sum(n for _, n in v) for v in by_bin.values()))

    print("\n== 退出集合逐条（binary :: name）==")
    for stem in sorted(by_bin):
        for name, n in sorted(by_bin[stem]):
            print("  %s::%s%s" % (stem, name, "" if n == 1 else "  x%d" % n))

    print("\n== 新增集合（应为空）==")
    if not added:
        print("  （无）")
    for (stem, name), n in sorted(added.items()):
        print("  + %s::%s%s" % (stem, name, "" if n == 1 else "  x%d" % n))

    # 主链 = 不属于三个被删 crate 的任何 binary
    deleted_prefixes = ("live2d_ai_mod_local_llm", "live2d_ai_mod_wallpaper", "live2d_ai_mod_pet_desktop")
    deleted_stems = {s for s in set(meta_b) | set(meta_a) if s.startswith(deleted_prefixes)}
    # 集成测试 binary 名（pet_desktop_state）也要算进去
    deleted_stems |= {s for s in set(meta_b) | set(meta_a) if s.startswith("pet_desktop_state")}
    mainline = {k: v for k, v in removed.items() if k[0] not in deleted_stems}
    print("\n== 主链集合差（不属于被删 crate 的 binary）==")
    print("  被识别的「被删 crate」binary: %s" % sorted(deleted_stems))
    print("  主链 removed = %d   %s" % (sum(mainline.values()), "✅ 0" if not mainline else "❌ 非 0"))
    for (stem, name), n in sorted(mainline.items()):
        print("  ! %s::%s" % (stem, name))

    # 守卫测试是否 after 仍在
    guard = ("live2d_ai_desktop", "web_api::cli_entry::tests::default_mods_manifest_enables_external_input_only")
    print("\n== 守卫测试（after 树）==")
    hits = [(k, v) for k, v in after.items() if k[1] == guard[1]]
    print("  '%s' 在 after 树命中: %s" % (guard[1], hits if hits else "❌ 未找到"))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
