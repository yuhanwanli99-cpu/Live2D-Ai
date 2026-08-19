#!/usr/bin/env bash
# Live2D-Ai — WSL2/Linux 一键启动（PC 端主力入口）
# 访问 http://localhost:12393 （WSL2 localhost 自动转发到 Windows 浏览器）
# 依赖：WSL2 原生 venv，见 Live2D-Ai-pc/README.md「WSL2 环境准备」
set -euo pipefail

ProjectRoot="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VtuberDir="$ProjectRoot/open-llm-vtuber"
VenvDir="${LIVE2DAI_WSL_VENV:-$HOME/Live2D-Ai/.venv}"
PY="$VenvDir/bin/python"

if [ ! -x "$PY" ]; then
    echo "[Live2D-Ai] 未找到 WSL2 venv 的 Python: $PY"
    echo "[Live2D-Ai] 请先执行 Live2D-Ai-pc/README.md 的「WSL2 环境准备」"
    echo "[Live2D-Ai] 或通过环境变量 LIVE2DAI_WSL_VENV 指定 venv 路径"
    exit 1
fi

if ! "$PY" -c "import open_llm_vtuber, fastapi, edge_tts" >/dev/null 2>&1; then
    echo "[Live2D-Ai] venv 缺少核心依赖，请先安装：" >&2
    echo "[Live2D-Ai]   uv pip install -e '$VtuberDir'" >&2
    exit 1
fi

cd "$VtuberDir"
echo "[Live2D-Ai] 启动后端... http://localhost:12393"
exec "$PY" run_server.py
