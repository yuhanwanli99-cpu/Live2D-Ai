#!/usr/bin/env bash
# MeloTTS 的**薄启动脚本**：首次启用装 venv，然后 exec 服务。
# 不写 [tts]、不改地址、不重试、失败非 0 退出。刻意不用 set -u。
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEIGHTS_DIR="$SCRIPT_DIR/weights"
VENV_DIR="$SCRIPT_DIR/.venv"
VENV_PY="$VENV_DIR/bin/python"
MELO_TAG="v0.1.2"
MELO_GIT="git+https://github.com/myshell-ai/MeloTTS.git@$MELO_TAG"

fail() {
  echo "[melo] $1" >&2
  exit 1
}

for name in config.json checkpoint.pth; do
  [ -f "$WEIGHTS_DIR/$name" ] || fail "缺权重 $WEIGHTS_DIR/$name（先跑 $SCRIPT_DIR/download.sh，checkpoint.pth 走 Git LFS）"
done

# ── 首次启用：把 MeloTTS 按 tag 装进 Mod 目录里的 .venv ────────────────────
# .venv 是隐藏目录：不进 git、也不进 rust-ratio 统计。
if [ ! -x "$VENV_PY" ]; then
  BOOT_PY="$(command -v python3 || true)"
  if [ -z "$BOOT_PY" ]; then BOOT_PY="$(command -v python || true)"; fi
  [ -n "$BOOT_PY" ] || fail "找不到 Python 解释器（python3 / python）"
  echo "[melo] 首次启用：创建 $VENV_DIR"
  "$BOOT_PY" -m venv "$VENV_DIR" || fail "创建 venv 失败"
  "$VENV_PY" -m pip install --upgrade pip || fail "升级 pip 失败"
  echo "[melo] 按 $MELO_TAG 安装 MeloTTS"
  "$VENV_PY" -m pip install "$MELO_GIT" || fail "按 $MELO_TAG 安装 MeloTTS 失败（装不上就停在这里，不重试）"
fi

PORT="$LIVE2D_AI_MELO_PORT"
if [ -z "$PORT" ]; then PORT="8091"; fi
HOST="$LIVE2D_AI_MELO_HOST"
if [ -z "$HOST" ]; then HOST="127.0.0.1"; fi

echo "[melo] 启动：$VENV_PY $SCRIPT_DIR/serve.py --host $HOST --port $PORT"
exec "$VENV_PY" "$SCRIPT_DIR/serve.py" --weights "$WEIGHTS_DIR" --host "$HOST" --port "$PORT"
