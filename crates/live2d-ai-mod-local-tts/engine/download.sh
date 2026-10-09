#!/usr/bin/env bash
# CosyVoice3 引擎：下载「源码（仓库外） + 权重（本 crate）」。唯一入口。
#
# 为什么源码落在仓库外：*.py 计入 xtask 的 rust-ratio 分母，把整棵 CosyVoice
# 放进工作树会把占比打穿。这里按固定修订号做一份**干净**副本。
#
# 任何一步失败都**非 0 退出**（Mod 不会把失败写成成功）。
# 刻意不用 set -u：变量一律先取、再显式判空。
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MOD_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WEIGHTS_DIR="$MOD_DIR/weights"
VENV_DIR="$SCRIPT_DIR/.venv"

COSYVOICE_GIT="$LIVE2D_AI_COSYVOICE_GIT"
if [ -z "$COSYVOICE_GIT" ]; then
  COSYVOICE_GIT="https://github.com/FunAudioLLM/CosyVoice.git"
fi
COSYVOICE_REV="$LIVE2D_AI_COSYVOICE_REV"
if [ -z "$COSYVOICE_REV" ]; then
  COSYVOICE_REV="074ca6dc9e80a2f424f1f74b48bdd7d3fea531cc"
fi
HF_REPO="$LIVE2D_AI_COSYVOICE_HF_REPO"
if [ -z "$HF_REPO" ]; then
  HF_REPO="FunAudioLLM/Fun-CosyVoice3-0.5B-2512"
fi
HF_REV="$LIVE2D_AI_COSYVOICE_HF_REV"
if [ -z "$HF_REV" ]; then
  HF_REV="29e01c4e8d000f4bcd70751be16fa94bf3d85a18"
fi

HOME_DIR="$HOME"
if [ -z "$HOME_DIR" ]; then
  echo "[cosyvoice3] HOME 未设置：无法确定仓库外的缓存目录" >&2
  exit 1
fi
CACHE_ROOT="$LIVE2D_AI_COSYVOICE_HOME"
if [ -z "$CACHE_ROOT" ]; then
  CACHE_ROOT="$HOME_DIR/.cache/live2d-ai/cosyvoice3"
fi
SRC_DIR="$CACHE_ROOT/CosyVoice"

BOOT_PY="$(command -v python3 || true)"
if [ -z "$BOOT_PY" ]; then
  echo "[cosyvoice3] PATH 上没有 python3：先装 Python 3.10+" >&2
  exit 1
fi

# ── 1) 源码（仓库外，固定修订号；已存在则校验当前 HEAD 是否一致） ──────────
mkdir -p "$CACHE_ROOT"
if [ ! -d "$SRC_DIR/.git" ]; then
  command -v git >/dev/null 2>&1 || { echo "[cosyvoice3] 需要 git" >&2; exit 1; }
  echo "[cosyvoice3] 克隆 $COSYVOICE_GIT -> $SRC_DIR"
  git clone "$COSYVOICE_GIT" "$SRC_DIR" || { echo "[cosyvoice3] git clone 失败" >&2; exit 1; }
fi
git -C "$SRC_DIR" fetch --all --tags --quiet || { echo "[cosyvoice3] git fetch 失败" >&2; exit 1; }
git -C "$SRC_DIR" checkout --quiet "$COSYVOICE_REV" || {
  echo "[cosyvoice3] 切到修订号 $COSYVOICE_REV 失败" >&2; exit 1; }
git -C "$SRC_DIR" submodule update --init --recursive --quiet || {
  echo "[cosyvoice3] 子模块（含 Matcha-TTS，MIT）初始化失败" >&2; exit 1; }
echo "[cosyvoice3] 源码就绪：$SRC_DIR @ $COSYVOICE_REV"

# ── 2) Python 依赖装进 engine/.venv（隐藏目录：不进 rust-ratio 统计） ───────
if [ ! -x "$VENV_DIR/bin/python" ]; then
  "$BOOT_PY" -m venv "$VENV_DIR" || { echo "[cosyvoice3] 创建 venv 失败" >&2; exit 1; }
fi
"$VENV_DIR/bin/python" -m pip install --upgrade pip || { echo "[cosyvoice3] 升级 pip 失败" >&2; exit 1; }
if [ -f "$SRC_DIR/requirements.txt" ]; then
  "$VENV_DIR/bin/python" -m pip install -r "$SRC_DIR/requirements.txt" || {
    echo "[cosyvoice3] 安装 $SRC_DIR/requirements.txt 失败" >&2; exit 1; }
fi
echo "[cosyvoice3] 解释器就绪：$VENV_DIR/bin/python"

# ── 3) 权重（本 crate 的 weights/，进 .gitignore） ─────────────────────────
HF_CLI="$(command -v hf || true)"
if [ -z "$HF_CLI" ]; then
  HF_CLI="$(command -v huggingface-cli || true)"
fi
if [ -z "$HF_CLI" ]; then
  echo "[cosyvoice3] 找不到 hf / huggingface-cli：先 pip install -U huggingface_hub" >&2
  exit 1
fi
mkdir -p "$WEIGHTS_DIR"
echo "[cosyvoice3] 下载权重 $HF_REPO @ $HF_REV -> $WEIGHTS_DIR"
"$HF_CLI" download "$HF_REPO" --revision "$HF_REV" --local-dir "$WEIGHTS_DIR" || {
  echo "[cosyvoice3] 下载权重失败（$HF_REPO @ $HF_REV）" >&2; exit 1; }

# 记录修订号（合规：再分发时要能说出是哪一版）。
printf '%s\n' "$HF_REPO@$HF_REV" > "$WEIGHTS_DIR/REVISION"

PT_COUNT="$(find "$WEIGHTS_DIR" -maxdepth 1 -name '*.pt' | wc -l)"
if [ "$PT_COUNT" -lt 1 ]; then
  echo "[cosyvoice3] 权重目录里没有 *.pt：下载看起来不完整 -> $WEIGHTS_DIR" >&2
  exit 1
fi
echo "[cosyvoice3] 完成：权重 $WEIGHTS_DIR（$PT_COUNT 个 .pt），源码 $SRC_DIR"
