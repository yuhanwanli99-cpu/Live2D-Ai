#!/usr/bin/env bash
# Live2D-Ai 一键构建+启动脚本（WSL）
# 用法：
#   ./scripts/run-web.sh              # 构建 + 启动（默认 18080）
#   ./scripts/run-web.sh 18100        # 指定端口
#   ./scripts/run-web.sh --no-build   # 不重新构建，直接启动
set -e
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

PORT="${2:-18080}"
if [ "$1" != "--no-build" ]; then
  echo "==> 构建 release WASM（l2d-wasm-demo）"
  unset NO_COLOR
  (cd crates/l2d-wasm-demo && trunk build)
  echo "==> 构建 live2d-ai-desktop …"
  cargo build -p live2d-ai-desktop
fi
# 清理可能残留的旧进程，避免端口冲突/旧二进制
pkill -f 'live2d-ai-desktop --web' 2>/dev/null || true
sleep 0.5

echo "==> 启动 web 服务 @ http://127.0.0.1:${PORT}"
echo "    浏览器打开后请 Ctrl+Shift+R 硬刷新生效"
if [ -d "crates/l2d-wasm-demo/dist" ]; then
  echo "==> WASM dist 构建产物："
  ls -la crates/l2d-wasm-demo/dist/*_bg.wasm
fi
LIVE2D_AI_WS_AUDIO=1 ./target/debug/live2d-ai-desktop --web --http-port "${PORT}"
