#!/usr/bin/env bash
# MeloTTS 的**薄启动脚本**：首次启用时按 tag 装进仓库内 venv，然后 exec 服务。
# 不写 [tts]、不改地址、不重试、失败非 0 退出。刻意不用 set -u。
#
# 2026-10-10（本轮）改的就是「装不上」那一半：
#   * 旧行为：只要 `.venv/bin/python` 还**可执行**就跳过安装，于是 3.14 的坏 venv
#     让 serve.py 报「No module named melo」——安装失败被「载入模型失败」盖住；
#   * 现行：`.venv` **不能 import melo** 就删掉这**一个**目录、用 Python 3.10 重建。
#     找不到 3.10 直接非 0 退出：系统 python3 是 3.14，而 tag v0.1.2 的 setup.py
#     含 `from pip.req import parse_requirements`（pip>=10 已删除该模块），
#     现代 pip 装不上。**禁止**退回 3.14 再试一次。
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEIGHTS_DIR="$SCRIPT_DIR/weights"
VENV_DIR="$SCRIPT_DIR/.venv"
VENV_PY="$VENV_DIR/bin/python"
SRC_DIR="$SCRIPT_DIR/.src"
CACHE_DIR="$SCRIPT_DIR/.cache"
MELO_TAG="v0.1.2"
MELO_REPO="https://github.com/myshell-ai/MeloTTS.git"
# 中文（ZH 走 ZH_MIX_EN 分支）推理要用的 BERT；**不入库**，下到 .cache 当 HF_HOME。
BERT_MODEL="bert-base-multilingual-uncased"

# 可用的 Python 3.10（**只借它们创建仓库内的 venv**）：
#   1) uv 托管的 CPython 3.10.21；2) conda 的 melo 环境 3.10.20（不往里装包）。
PY310_UV="$HOME/.local/share/uv/python/cpython-3.10.21-linux-x86_64-gnu/bin/python"
PY310_CONDA="$HOME/miniconda3/envs/melo/bin/python"

fail() {
  echo "[melo] $1" >&2
  exit 1
}

for name in config.json checkpoint.pth; do
  [ -f "$WEIGHTS_DIR/$name" ] || fail "缺权重 $WEIGHTS_DIR/$name（先跑 $SCRIPT_DIR/download.sh，checkpoint.pth 走 Git LFS）"
done

# `.venv` 里那份 python 真能 `import melo` 吗？（这就是旧的「存在即跳过」漏掉的一步）
venv_has_melo() {
  [ -x "$VENV_PY" ] || return 1
  "$VENV_PY" -c "import melo" >/dev/null 2>&1
}

# 找一个**真的是 3.10** 的解释器；找不到返回 1（不回落到 3.14）。
find_py310() {
  for candidate in "$PY310_UV" "$PY310_CONDA" "$(command -v python3.10 || true)"; do
    [ -n "$candidate" ] || continue
    [ -x "$candidate" ] || continue
    if "$candidate" -c "import sys; raise SystemExit(0 if sys.version_info[:2] == (3, 10) else 1)" >/dev/null 2>&1; then
      printf "%s\n" "$candidate"
      return 0
    fi
  done
  return 1
}

# 补丁 1（计划书点名的这一处）：tag v0.1.2 的 setup.py 用 `from pip.req import
# parse_requirements`，pip>=10 没有这个模块。删掉该 import 与 parse_requirements
# 那两行，改为直接读 requirements.txt 的非空、非注释行。只改这一处。
patch_setup_py() {
  # $1 = setup.py 的路径（只有 .src 里那份；它不进 site-packages）
  "$VENV_PY" - "$1" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
src = path.read_text(encoding="utf-8")
if "parse_requirements" not in src:
    print("[melo] setup.py 已是读 requirements.txt 的形态，跳过")
    raise SystemExit(0)
lines = src.splitlines(True)
out = []
replaced = False
for line in lines:
    stripped = line.strip()
    if stripped.startswith("from pip.req import"):
        continue
    if stripped.startswith("install_reqs = parse_requirements") or stripped.startswith("reqs = [str(ir.req)"):
        if not replaced:
            out.append(
                "with open(os.path.join(cwd, \"requirements.txt\"), encoding=\"utf-8\") as _req:\n"
                "    reqs = [line.strip() for line in _req if line.strip() and not line.strip().startswith(\"#\")]\n"
            )
            replaced = True
        continue
    out.append(line)
patched = "".join(out)
if not replaced or "parse_requirements" in patched:
    print("[melo] setup.py 补丁没落地：parse_requirements 的形状与预期不符", file=sys.stderr)
    raise SystemExit(1)
path.write_text(patched, encoding="utf-8")
print("[melo] 已修 setup.py：requirements.txt 的非空非注释行 -> install_requires")
PY
}

# 补丁 2（**本机安装侧的桥**，计划书原文只说补丁 1；见报告里的偏差说明）：
# melo/serve.py 调 `TTS(language=..., device=..., config_path=..., ckpt_path=...)`
# 来加载仓库内 weights/，而 tag v0.1.2 的 api.py 还是 `(language, device="auto")`
# 两个参数（master 才有 config_path/ckpt_path）。不补这一处，装完必然 TypeError、
# 8091 永远起不来。只加两个参数与两个「用本地权重」的分支，不动合成/模型逻辑。
patch_api_py() {
  # $1 = api.py 的路径（.src 与 venv 里已安装的副本都要打）
  "$VENV_PY" - "$1" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
src = path.read_text(encoding="utf-8")
if "ckpt_path" in src and "config_path" in src:
    print("[melo] api.py 已接受 config_path/ckpt_path，跳过")
    raise SystemExit(0)
old_sig = "    def __init__(self, \n                language,\n                device='auto'):\n"
new_sig = "    def __init__(self, \n                language,\n                device='auto',\n                config_path=None,\n                ckpt_path=None):\n"
old_cfg = "        hps = load_or_download_config(language)\n"
new_cfg = (
    "        if config_path is None:\n"
    "            hps = load_or_download_config(language)\n"
    "        else:\n"
    "            hps = utils.get_hparams_from_file(config_path)\n"
)
old_ckpt = "        checkpoint_dict = load_or_download_model(language, device)\n"
new_ckpt = (
    "        if ckpt_path is None:\n"
    "            checkpoint_dict = load_or_download_model(language, device)\n"
    "        else:\n"
    "            checkpoint_dict = torch.load(ckpt_path, map_location=device)\n"
)
for old, new in ((old_sig, new_sig), (old_cfg, new_cfg), (old_ckpt, new_ckpt)):
    if src.count(old) != 1:
        print("[melo] api.py 补丁没落地：找不到唯一的锚点 " + repr(old[:40]), file=sys.stderr)
        raise SystemExit(1)
    src = src.replace(old, new)
path.write_text(src, encoding="utf-8")
print("[melo] 已修 api.py：TTS(config_path=..., ckpt_path=...) 用仓库内 weights/")
PY
}

# 补丁 3（**本机安装侧的桥**，计划书原文只说补丁 1；见报告里的偏差说明）：
# tag v0.1.2 的 japanese.py 在 **import 期**就 `MeCab.Tagger()`（`melo/text/japanese.py:367`），
# 而 cleaner.py 无条件 import 它 —— 于是中文链路也被一个 526MB 的 UniDic 全量词典卡住
# （缺 .../unidic/dicdir/mecabrc 时 import 直接 RuntimeError）。mecab-python3 的 wheel
# 把 dicdir 编译死在 unidic 那个位置，`MECABRC` 环境变量并不生效，所以「不下载」只能改这一行。
# 上游 master 的同一行早就改成了下面这样（用 requirements.txt 里已经声明的 unidic_lite）：
#   _TAGGER = MeCab.Tagger("-r " + os.path.join(unidic_lite.DICDIR, "mecabrc") + " -d " + unidic_lite.DICDIR)
# 这里逐字照抄这一行（外加 `import os` / `import unidic_lite`），不动日文的其他逻辑。
patch_japanese_py() {
  # $1 = japanese.py 的路径（.src 与 venv 里已安装的副本都要打）
  "$VENV_PY" - "$1" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
src = path.read_text(encoding="utf-8")
if "unidic_lite.DICDIR" in src:
    print("[melo] japanese.py 已用 unidic_lite 的词典，跳过")
    raise SystemExit(0)
old_imports = "import re\nimport unicodedata\n"
new_imports = "import os\nimport re\nimport unicodedata\n\nimport unidic_lite\n"
old_tagger = "_TAGGER = MeCab.Tagger()\n"
new_tagger = (
    '_TAGGER = MeCab.Tagger("-r " + os.path.join(unidic_lite.DICDIR, "mecabrc")'
    ' + " -d " + unidic_lite.DICDIR)\n'
)
for old, new in ((old_imports, new_imports), (old_tagger, new_tagger)):
    if src.count(old) != 1:
        print("[melo] japanese.py 补丁没落地：找不到唯一的锚点 " + repr(old[:30]), file=sys.stderr)
        raise SystemExit(1)
    src = src.replace(old, new)
path.write_text(src, encoding="utf-8")
print("[melo] 已修 japanese.py：MeCab 用 unidic_lite 的词典（上游 master 的写法）")
PY
}

# ── 首次启用 / 坏 venv：用 Python 3.10 就地重建 ────────────────────────────
if ! venv_has_melo; then
  case "$VENV_DIR" in
    "$SCRIPT_DIR/.venv") ;;
    *) fail "内部错误：拒绝删除 $VENV_DIR（只允许删 $SCRIPT_DIR/.venv）" ;;
  esac
  if [ -d "$VENV_DIR" ]; then
    OLD_VER="$("$VENV_PY" -V 2>&1 || true)"
    echo "[melo] $VENV_DIR 不能 import melo（${OLD_VER:-找不到 python}）：删掉这一个目录后重建"
    rm -rf "$VENV_DIR"
  fi
  BOOT_PY="$(find_py310 || true)"
  if [ -z "$BOOT_PY" ]; then
    fail "找不到 Python 3.10（找过 $PY310_UV 与 $PY310_CONDA）：系统 python3 只有 3.14，而 MeloTTS $MELO_TAG 的 setup.py 用 from pip.req import parse_requirements，现代 pip 装不上；请先装一个 3.10，不要用 3.14 重试"
  fi
  echo "[melo] 用 $BOOT_PY（$("$BOOT_PY" -V 2>&1)）创建 $VENV_DIR"
  "$BOOT_PY" -m venv "$VENV_DIR" || fail "创建 venv 失败（$BOOT_PY）"
  "$VENV_PY" -m pip install --upgrade pip || fail "升级 pip 失败"

  if [ ! -f "$SRC_DIR/setup.py" ]; then
    echo "[melo] 克隆 $MELO_REPO @ $MELO_TAG -> $SRC_DIR"
    rm -rf "$SRC_DIR"
    git clone --branch "$MELO_TAG" --depth 1 "$MELO_REPO" "$SRC_DIR" \
      || fail "克隆 MeloTTS $MELO_TAG 失败（装不上就停在这里，不重试）"
  fi
  patch_setup_py "$SRC_DIR/setup.py" || fail "setup.py 补丁失败（装不上就停在这里，不重试）"
  patch_api_py "$SRC_DIR/melo/api.py" || fail "api.py 补丁失败（装不上就停在这里，不重试）"
  patch_japanese_py "$SRC_DIR/melo/text/japanese.py" \
    || fail "japanese.py 补丁失败（装不上就停在这里，不重试）"

  # 安装期任何「跑 python」的钩子都要落在 venv 里，不是系统的 3.14。
  export PATH="$VENV_DIR/bin:$PATH"
  # requirements.txt 里是 `torch<2.0`：从 PyTorch 的 CPU 索引装（**不装 CUDA 轮**），
  # 1.13.1 / 0.13.1 是 <2.0 的最后一份；先装它俩，后面 `pip install $SRC_DIR`
  # 就不会再去 PyPI 拉 CUDA 轮。
  echo "[melo] 安装 torch==1.13.1 + torchaudio==0.13.1（CPU 索引）"
  "$VENV_PY" -m pip install --index-url https://download.pytorch.org/whl/cpu \
    "torch==1.13.1" "torchaudio==0.13.1" \
    || fail "安装 torch/torchaudio（CPU 轮）失败（装不上就停在这里，不重试）"
  echo "[melo] 安装 MeloTTS $MELO_TAG 的其余依赖（源码：$SRC_DIR）"
  "$VENV_PY" -m pip install "$SRC_DIR" \
    || fail "安装 MeloTTS $MELO_TAG 失败（装不上就停在这里，不重试）"
fi

# ── 已安装副本的补丁修复 ───────────────────────────────────────────────────
# pip 会把源码**拷进** site-packages：只打 `.src` 只对「这次新装」生效。
# 老 venv（用旧版 start.sh 装过、或手工装过）在这里补打一次。
# 补丁是幂等的：已经打过的会打印「跳过」。
SITE_PACKAGES="$VENV_DIR/lib/python3.10/site-packages"
if [ -f "$SITE_PACKAGES/melo/api.py" ]; then
  patch_api_py "$SITE_PACKAGES/melo/api.py" \
    || fail "api.py 补丁失败（venv 里已安装的副本；装不上就停在这里，不重试）"
  patch_japanese_py "$SITE_PACKAGES/melo/text/japanese.py" \
    || fail "japanese.py 补丁失败（venv 里已安装的副本；装不上就停在这里，不重试）"
fi

# ── 中文推理要用的 BERT：不进仓库，落在 melo/.cache 并当 HF_HOME ────────────
export HF_HOME="$CACHE_DIR"
# HF 端点：本机环境里 HF_ENDPOINT=hf-mirror.com，而它对 resolve 请求回 308 **绝对**
# 跳转到 huggingface.co —— huggingface_hub 的元数据校验不接受跨站跳转
# （FileMetadataError），于是每次取文件都失败（只做 HEAD 的探针看不出来：它跟随跳转，
# 真的下载才炸）。所以这里**真下一个小文件**（config.json，约 1KB）来判断端点通不通；
# 不通就改用官方端点（镜像本来就 308 到它）。只试这两个端点：不循环、不重试。
try_hf_endpoint() {
  # $1 非空 = 显式用这个端点；空 = 用环境里现有的那一个
  if [ -n "$1" ]; then export HF_ENDPOINT="$1"; fi
  echo "[melo] 试 HF 端点：${HF_ENDPOINT:-https://huggingface.co（默认）}"
  "$VENV_PY" - <<'PY'
import sys

from huggingface_hub import hf_hub_download

try:
    hf_hub_download("bert-base-multilingual-uncased", "config.json")
except Exception as exc:  # noqa: BLE001
    print("[melo] 这个端点取不到：" + type(exc).__name__ + "：" + str(exc)[:200], file=sys.stderr)
    raise SystemExit(1)
PY
}

BERT_DIR="$CACHE_DIR/huggingface/hub/models--bert-base-multilingual-uncased"
# 「模型下好了没有」看权重文件，不看目录：端点探针已经往这个目录里落过 config.json。
BERT_BIN="$(ls "$BERT_DIR"/snapshots/*/pytorch_model.bin 2>/dev/null | head -n 1 || true)"
if [ -z "$BERT_BIN" ]; then
  echo "[melo] 正在下载 $BERT_MODEL（不入库）"
  if ! try_hf_endpoint; then
    echo "[melo] 环境里的 HF_ENDPOINT 取不到，改用 https://huggingface.co（镜像本来就 308 到它）"
    try_hf_endpoint "https://huggingface.co" \
      || fail "下载 $BERT_MODEL 失败（HF_HOME=$CACHE_DIR）：两个 HF 端点都取不到，装不上就停在这里，不重试"
  fi
  "$VENV_PY" - "$BERT_MODEL" <<'PY' || fail "下载 $BERT_MODEL 失败（HF_HOME=$CACHE_DIR）：见上方原话，装不上就停在这里，不重试"
import sys

from transformers import AutoModelForMaskedLM, AutoTokenizer

model_id = sys.argv[1]
AutoTokenizer.from_pretrained(model_id)
AutoModelForMaskedLM.from_pretrained(model_id)
print("[melo] 已缓存 " + model_id)
PY
fi

# 预热：cleaner.py 在 import 期就要取各语言的 tokenizer（顺带把 japanese.py 的
# MeCab 初始化掉）。在这里跑一次，失败原因就留在 start.sh 的输出里，
# 而不是等到 serve.py 起不来才看见。
"$VENV_PY" -c "import melo.text.cleaner" \
  || fail "导入 melo.text.cleaner 失败（见上方原话，不重试）"

PORT="$LIVE2D_AI_MELO_PORT"
if [ -z "$PORT" ]; then PORT="8091"; fi
HOST="$LIVE2D_AI_MELO_HOST"
if [ -z "$HOST" ]; then HOST="127.0.0.1"; fi

echo "[melo] 启动：$VENV_PY $SCRIPT_DIR/serve.py --host $HOST --port $PORT"
exec "$VENV_PY" "$SCRIPT_DIR/serve.py" --weights "$WEIGHTS_DIR" --host "$HOST" --port "$PORT"
