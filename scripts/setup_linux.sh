#!/bin/bash
# Live2Dai — Linux 开发/测试环境准备（轻量，不打包发布）
#
# 用途：
#   - 在 Linux 上跑 root pytest、一致性检查、健康检查
#   - 编写/调试共享配置、跨端契约、Android 纯 JVM 测试思路
#   - 不作为 Linux 发布安装包（主力平台 = Android + Windows）
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
pip install jsonschema

echo "== Done =="
echo "Run: source .venv/bin/activate && python scripts/verify_all.py"
