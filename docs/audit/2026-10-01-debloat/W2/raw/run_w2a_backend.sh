#!/usr/bin/env bash
# W-VERIFY-2A：在**归档副本**上独立跑 W2-A 的后端全量门禁 + 集合差取证。
# 共用 target 目录以省一次全依赖编译（两个源树路径不同，但 registry 依赖产物可复用）。
set -uo pipefail
REPO=/home/skystar/Live2D-Ai-fe
RAW=$REPO/docs/audit/2026-10-01-debloat/W2/raw
export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"
export CARGO_TARGET_DIR=/tmp/w2_verify_target
OUT=$RAW/w2a_backend.txt
: > "$OUT"

BEFORE=d140604f   # checkpoint/pre-d1-dormant
AFTER=42ca9a37

mkdir -p "$RAW" /tmp/w2a_before /tmp/w2a_after
git -C "$REPO" archive "$BEFORE" | tar -x -C /tmp/w2a_before
git -C "$REPO" archive "$AFTER"  | tar -x -C /tmp/w2a_after

{
echo "########## W-VERIFY-2A 后端门禁（归档副本；CARGO_TARGET_DIR=$CARGO_TARGET_DIR）##########"
echo "# 时间: $(date -Iseconds)"
echo "# BEFORE=$BEFORE ($(git -C "$REPO" rev-parse $BEFORE))  = checkpoint/pre-d1-dormant"
echo "# AFTER =$AFTER  ($(git -C "$REPO" rev-parse $AFTER))"
echo "# \$ df -h / | tail -1"; df -h / | tail -1
echo "# 其它 cargo 进程（应为空或 rust-debloat 的）"; pgrep -a cargo | head -5 || true
echo

echo "########## A. AFTER 树 --list（先编译，后续复用）##########"
cd /tmp/w2a_after
cargo test --workspace --all-targets -- --list 2>&1 | tee "$RAW/w2a_after_list_raw.txt" | tail -3
echo "LIST_AFTER_PIPE=${PIPESTATUS[0]}"
echo

echo "########## B. AFTER 树：cargo test --workspace --all-targets（全量运行）##########"
cargo test --workspace --all-targets 2>&1 | tee "$RAW/w2a_after_test_raw.txt" | tail -5
echo "TEST_PIPE=${PIPESTATUS[0]}"
echo

echo "########## C. AFTER 树：cargo test --doc --workspace ##########"
cargo test --doc --workspace 2>&1 | tail -8
echo "DOC_PIPE=${PIPESTATUS[0]}"
echo

echo "########## D. AFTER 树：cargo fmt --all -- --check ##########"
cargo fmt --all -- --check 2>&1 | tail -10
echo "FMT_EXIT=$?"
echo

echo "########## E. AFTER 树：cargo clippy --workspace --all-targets -- -D warnings ##########"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
echo "CLIPPY_PIPE=${PIPESTATUS[0]}"
echo

echo "########## F. AFTER 树：cargo run -p xtask -- code-stats ##########"
cargo run -q -p xtask -- code-stats 2>&1
echo "CODESTATS_PIPE=${PIPESTATUS[0]}"
echo

echo "########## G. AFTER 树：cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo ##########"
cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo 2>&1 | tail -6
echo "WASM_PIPE=${PIPESTATUS[0]}"
echo

echo "########## H. BEFORE 树 --list（复用同一 target 目录）##########"
cd /tmp/w2a_before
cargo test --workspace --all-targets -- --list 2>&1 | tee "$RAW/w2a_before_list_raw.txt" | tail -3
echo "LIST_BEFORE_PIPE=${PIPESTATUS[0]}"
echo
echo "DONE $(date -Iseconds)"
} >> "$OUT" 2>&1
