#!/usr/bin/env bash
# W-VERIFY-0 / Phase B + Phase C(W0b) 取证脚本（verifier 自写；只读仓库）
# 用法: bash run_phaseB.sh <要写入的证据文件>
# 前提: W0b 已提交；同一时刻**只有本进程**在跑全量 cargo（先确认无其它 cargo 进程）。
set -uo pipefail
REPO=/home/skystar/Live2D-Ai-fe
OUT="${1:-/tmp/phaseB.txt}"
export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"
cd "$REPO" || exit 1

{
echo "########## Phase B · W0 后端全量门禁（落在已提交的 W0b 树上）##########"
echo "# 时间: $(date -Iseconds)"
echo "# \$ git rev-parse HEAD"; git rev-parse HEAD
echo "# \$ git status --short"; git status --short
echo "# \$ df -h . | tail -1"; df -h . | tail -1
echo "# 核查：无其它 cargo/rustc 进程"; pgrep -a cargo | grep -v "$$" || echo "(none besides this script)"
echo

echo "########## B1. cargo test --workspace --all-targets ##########"
cargo test --workspace --all-targets 2>&1 | tail -60
echo "B1_PIPE_STATUS=${PIPESTATUS[0]}"
echo

echo "########## B2. cargo test --doc --workspace ##########"
cargo test --doc --workspace 2>&1 | tail -30
echo "B2_PIPE_STATUS=${PIPESTATUS[0]}"
echo

echo "########## B3. cargo fmt --all -- --check ##########"
cargo fmt --all -- --check 2>&1 | tail -40
echo "B3_EXIT=$?"
echo

echo "########## B4. cargo clippy --workspace --all-targets -- -D warnings ##########"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -40
echo "B4_PIPE_STATUS=${PIPESTATUS[0]}"
echo

echo "########## B5. cargo run -p xtask -- rust-ratio ##########"
cargo run -p xtask -- rust-ratio 2>&1 | tail -40
echo "B5_PIPE_STATUS=${PIPESTATUS[0]}"
echo

echo "########## B6. cargo run -p xtask -- code-stats ##########"
cargo run -p xtask -- code-stats 2>&1
echo "B6_PIPE_STATUS=${PIPESTATUS[0]}"
echo

echo "########## B7. code-stats --check（绿向，默认棘轮）##########"
cargo run -p xtask -- code-stats --check 2>&1 | tail -30
echo "B7_PIPE_STATUS=${PIPESTATUS[0]}"
echo
} > "$OUT" 2>&1
echo "written: $OUT"
