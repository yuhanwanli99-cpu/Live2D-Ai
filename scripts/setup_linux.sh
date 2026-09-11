#!/bin/bash
# Live2D-Ai — Linux 开发/测试环境准备（轻量，不打包发布）
#
# 用途：
#   - 在 Linux 上跑 root pytest（tests/ 健康检查）
#   - 编写/调试共享配置、跨端契约（Rust 重构后 PC/Android 已归档，见 ANDROID_ARCHIVE_POINTER.md）
#   - 不作为 Linux 发布安装包（主力平台 = Rust 桌面端 + Web）
#
# 用法：
#   bash scripts/setup_linux.sh
set -euo pipefail

echo "== Installing system packages (Python 3 + pip + venv) =="
if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y python3 python3-pip python3-venv git curl
elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y python3 python3-pip git curl
else
    echo "WARN: unsupported package manager; please install Python 3.10+ manually."
fi

echo "== Creating .venv for root tests =="
python3 -m venv .venv
. .venv/bin/activate
pip install --upgrade pip
pip install -r requirements-test.txt

echo "== Done =="
echo "Run: source .venv/bin/activate && python -m pytest tests/ -q"
echo "Rust 门禁: cargo test --workspace && cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings"
