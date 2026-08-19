#!/bin/bash
# Live2D-Ai — 清理生成物/缓存（不删除 .venv / node_modules / models 等本地运行环境）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== Removing build/cache/log/agent-work clutter =="
rm -rf build cache spoq .pytest_cache .ruff_cache s.log s2.log s_dl.log
rm -rf Live2D-Ai-Android/.gradle Live2D-Ai-Android/.kotlin Live2D-Ai-Android/app/build Live2D-Ai-Android/app/.cxx
find . -type d \( -name __pycache__ -o -name .pytest_cache -o -name .ruff_cache -o -name '*.egg-info' \) -not -path './.git/*' -prune -exec rm -rf {} + 2>/dev/null || true

echo "== Done. Local runtime (.venv/node_modules/models) kept. =="
