#!/usr/bin/env bash
# CosyVoice3 的**薄启动脚本**：只做「找到东西 -> exec 服务」，不写 [tts]、
# 不探活、不改地址、不重试、失败非 0 退出。
#
# 只认**自己旁边**的权重目录（crates/live2d-ai-mod-local-tts/weights/）与
# 仓库外的固定修订号源码缓存。**禁止**回落到 /home/skystar/CosyVoice 3.0。
# 刻意不用 set -u：变量一律先取、再显式判空。
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MOD_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WEIGHTS_DIR="$MOD_DIR/weights"
VENV_PY="$SCRIPT_DIR/.venv/bin/python"

fail() {
  echo "[cosyvoice3] $1" >&2
  exit 1
}

HOME_DIR="$HOME"
if [ -z "$HOME_DIR" ]; then fail "HOME 未设置"; fi
CACHE_ROOT="$LIVE2D_AI_COSYVOICE_HOME"
if [ -z "$CACHE_ROOT" ]; then CACHE_ROOT="$HOME_DIR/.cache/live2d-ai/cosyvoice3"; fi
SRC_DIR="$CACHE_ROOT/CosyVoice"

[ -d "$WEIGHTS_DIR" ] || fail "缺权重目录 $WEIGHTS_DIR：先跑 $SCRIPT_DIR/download.sh"
PT_COUNT="$(find "$WEIGHTS_DIR" -maxdepth 1 -name '*.pt' | wc -l)"
[ "$PT_COUNT" -ge 1 ] || fail "权重目录里没有 *.pt（$WEIGHTS_DIR）：先跑 $SCRIPT_DIR/download.sh"
[ -d "$SRC_DIR" ] || fail "缺 CosyVoice 源码 $SRC_DIR：先跑 $SCRIPT_DIR/download.sh"

PY=""
if [ -x "$VENV_PY" ]; then
  PY="$VENV_PY"
else
  PY="$(command -v python3 || true)"
  if [ -z "$PY" ]; then PY="$(command -v python || true)"; fi
  [ -n "$PY" ] || fail "找不到 Python 解释器：先跑 $SCRIPT_DIR/download.sh"
fi

PORT="$LIVE2D_AI_COSYVOICE_PORT"
if [ -z "$PORT" ]; then PORT="8080"; fi
HOST="$LIVE2D_AI_COSYVOICE_HOST"
if [ -z "$HOST" ]; then HOST="127.0.0.1"; fi

echo "[cosyvoice3] 启动：$PY $SCRIPT_DIR/serve.py --host $HOST --port $PORT"
exec "$PY" "$SCRIPT_DIR/serve.py" --src "$SRC_DIR" --weights "$WEIGHTS_DIR" --host "$HOST" --port "$PORT"
