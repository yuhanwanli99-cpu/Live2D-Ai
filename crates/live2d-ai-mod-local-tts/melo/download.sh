#!/usr/bin/env bash
# MeloTTS 中文权重下载：config.json（普通入库，这里只是补齐）+ checkpoint.pth（LFS）。
# 任何一步失败都非 0 退出。刻意不用 set -u：变量一律先取、再显式判空。
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEIGHTS_DIR="$SCRIPT_DIR/weights"
HF_REPO="$LIVE2D_AI_MELO_HF_REPO"
if [ -z "$HF_REPO" ]; then HF_REPO="myshell-ai/MeloTTS-Chinese"; fi
HF_REV="$LIVE2D_AI_MELO_HF_REV"
if [ -z "$HF_REV" ]; then HF_REV="af5d207a364ea4208c6f589c89f57f88414bdd16"; fi

HF_CLI="$(command -v hf || true)"
if [ -z "$HF_CLI" ]; then HF_CLI="$(command -v huggingface-cli || true)"; fi
if [ -z "$HF_CLI" ]; then
  echo "[melo] 找不到 hf / huggingface-cli：先 pip install -U huggingface_hub" >&2
  exit 1
fi

mkdir -p "$WEIGHTS_DIR"
echo "[melo] 下载 $HF_REPO @ $HF_REV -> $WEIGHTS_DIR"
"$HF_CLI" download "$HF_REPO" --revision "$HF_REV" --local-dir "$WEIGHTS_DIR" \
  --include "config.json" --include "checkpoint.pth" || {
  echo "[melo] 下载失败（$HF_REPO @ $HF_REV）" >&2; exit 1; }

for name in config.json checkpoint.pth; do
  if [ ! -f "$WEIGHTS_DIR/$name" ]; then
    echo "[melo] 缺 $WEIGHTS_DIR/$name：下载看起来不完整" >&2
    exit 1
  fi
done
printf '%s\n' "$HF_REPO@$HF_REV" > "$WEIGHTS_DIR/REVISION"
echo "[melo] 完成：$WEIGHTS_DIR"
