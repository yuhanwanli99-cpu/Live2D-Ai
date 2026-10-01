#!/usr/bin/env bash
# `code-stats --check` 红-绿双向（planner 指定：「临时造一个 >1000 行文件 → 必须红；删掉 → 必须绿」）
#
# 做法：**不碰仓库**。在 /tmp 造一棵合成 fixture 树（crates/demo/src/huge.rs 等），
# 用**仓库编译出来的真 xtask 二进制**、以 cwd = fixture 运行 —— 这正是工具自己的扫描根。
# 这样既满足「真造文件」的判据，又不需要往 crates/*/src 里写任何东西。
set -uo pipefail
REPO=/home/skystar/Live2D-Ai-fe
BIN="$REPO/target/debug/xtask"
FIX=/tmp/cs_fixture
OUT="${1:-$REPO/docs/audit/2026-10-01-debloat/W0/raw/w0b_check_redgreen.txt}"

{
echo "########## code-stats --check 红-绿双向（合成 fixture + 真二进制）##########"
echo "# 二进制: $BIN"
ls -la "$BIN" 2>&1
echo "# 时间: $(date -Iseconds)"
echo

rm -rf "$FIX"
mkdir -p "$FIX/crates/demo/src" "$FIX/crates/demo/tests" "$FIX/docs" "$FIX/shell/flutter/lib" "$FIX/shell/flutter/test"
cat > "$FIX/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/demo"]
EOF
# 一个 1100 行的生产文件（>1000 ⇒ over-1000 门禁必须红）
python3 - "$FIX/crates/demo/src/huge.rs" <<'PY'
import sys
with open(sys.argv[1], 'w') as f:
    f.write("// huge fixture\n")
    for i in range(1100):
        f.write("pub fn f%04d() -> u32 { %d }\n" % (i, i))
PY
printf 'pub fn small() {}\n' > "$FIX/crates/demo/src/lib.rs"
printf '# demo\n' > "$FIX/crates/demo/Cargo.toml"
printf '# d\n' > "$FIX/docs/a.md"
printf '// dart\n' > "$FIX/shell/flutter/lib/a.dart"
printf '// dart test\n' > "$FIX/shell/flutter/test/a_test.dart"
echo "# fixture 结构:"; find "$FIX" -type f | sort | sed "s|$FIX|.|"
echo "# huge.rs 行数: $(wc -l < "$FIX/crates/demo/src/huge.rs")"
echo

echo "########## [红] 存在 1100 行文件 → --check 必须非零 ##########"
cd "$FIX" && "$BIN" code-stats --check 2>&1 | tail -25
echo "RED_EXIT=${PIPESTATUS[0]}"
echo

echo "########## [绿] 删掉该死文件 → --check 必须为 0 ##########"
rm "$FIX/crates/demo/src/huge.rs"
cd "$FIX" && "$BIN" code-stats --check 2>&1 | tail -25
echo "GREEN_EXIT=${PIPESTATUS[0]}"
echo

echo "########## [附加-红] 用 --max-* 把阈值压到 0（在真仓库上，不写任何文件）##########"
cd "$REPO"
cargo run -q -p xtask -- code-stats --check --only over-1000 --max-src-rs-1000 0 2>&1 | tail -12
echo "EXTRA_RED_EXIT=${PIPESTATUS[0]}"
echo
echo "########## [附加-绿] --max-src-rs-1000 4（等于现状）##########"
cargo run -q -p xtask -- code-stats --check --only over-1000 --max-src-rs-1000 4 2>&1 | tail -12
echo "EXTRA_GREEN_EXIT=${PIPESTATUS[0]}"
echo
echo "########## [附加] --strict-plan（PLAN §5 目标；按设计现在应为红）##########"
cargo run -q -p xtask -- code-stats --check --strict-plan 2>&1 | tail -20
echo "STRICT_PLAN_EXIT=${PIPESTATUS[0]}"
echo
echo "########## 收尾：确认仓库未被本脚本改动 ##########"
cd "$REPO" && git status --short
} > "$OUT" 2>&1
echo "written: $OUT"
