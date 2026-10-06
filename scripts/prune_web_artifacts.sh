#!/usr/bin/env bash
# Flutter Web 产物**清减 + 预算门禁**（E4 真减 / E13 门禁化；2026-10-06 团队轮 W2）。
#
# 本脚本回答两个问题，缺一不可：
#   1) 死重还在不在？（*.symbols / skwasm* / wimp* —— 本构建的加载路径永不 fetch 的文件）
#   2) 目录还超不超预算？（默认 35 MiB，与 xtask/src/code_stats/mod.rs 的
#      FLUTTER_WEB_BUDGET_MIB 同值，本脚本启动时读它做漂移比对）
#
# ---- 删什么（依据是**实测网络请求**，不是推断）--------------------------------
#   1) canvaskit/**/*.symbols          —— 调试符号 sidecar（wasm 栈回溯用），任何渲染器都不加载；
#   2) canvaskit/skwasm*               —— 只在 renderer == "skwasm" 时加载（本构建不是）；
#   3) canvaskit/wimp*                 —— 同上（skwasm 的单线程变体）。
#
#   取证：2026-10-05 / 2026-10-06 两次真实浏览器会话的 Network 转储里
#   skwasm* / wimp* / *.symbols 命中 **0 次**，而
#   /app/canvaskit/chromium/canvaskit.{js,wasm} 各 **200 ×4**
#   ⇒ 本机生效的是 chromium 变体（源分析：docs/plans/DECISION-artifact-budget-2026-10-06.md §1.3；
#   原始转储：docs/verification/evidence-2026-10-06/net.json）。
#
# ---- 红线（删了就坏；--check 逐条断言存在）------------------------------------
#   * canvaskit/canvaskit.{js,wasm}            非 Chromium 浏览器（无 break-iterator /
#     WebCodecs 图像解码）的回落 —— 删了那些浏览器当场不可用；
#   * canvaskit/chromium/canvaskit.{js,wasm}   **本机 Chrome 实际加载的那份**；
#   * index.html / main.dart.js / flutter.js / flutter_bootstrap.js
#     （CDN 探针清单与加载决策的真源，见 scripts/lib/cdn_probe.sh）；
#   * assets/**（含 NOTICES，许可义务）与 font-fallback/**（E10 离线字体镜像）。
#   * canvaskit/webparagraph/** 本轮**保留**（B 档）：上游 preferWebParagraph 一旦默认打开
#     就会 404，3.6 MiB 的收益不值得这个未来风险（DECISION §2.1）。
#
# ---- 为什么必须脚本化 ----------------------------------------------------------
#   flutter build web 会把引擎缓存里的整套 CanvasKit（13 文件 + 2 子目录）**原样拷回**
#   ⇒ 手删产物 = 下一次构建悄悄反弹，数字回去而没人发现。真减必须是**构建后步骤**，
#   配 --check 守住；CI 见 .github/workflows/flutter-checks.yml 的
#   flutter-web-offline-artifacts job（构建 → 清减 → --check → ignite.sh --check-dir）。
#
# ---- 用法 ----------------------------------------------------------------------
#   scripts/prune_web_artifacts.sh [--dir DIR]          # 执行清减
#   scripts/prune_web_artifacts.sh --check [--dir DIR]  # 只读：残留 / 红线缺失 / 超预算 ⇒ 非 0
#   scripts/prune_web_artifacts.sh --help
#
# 环境变量：
#   PRUNE_WEB_DIR       产物目录（等价 --dir；默认 <repo>/shell/flutter/build/web）
#   PRUNE_WEB_RENDERER  覆盖渲染器判定（canvaskit|skwasm）；默认从 flutter_bootstrap.js 的
#                       buildConfig 读。读不到就**判红**，绝不猜 —— 猜错清单会静默缺件。
#
# 退出码：0 = 通过 / 2 = 用法或环境不可判定（判红，不静默跳过）
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_DIR="$ROOT/shell/flutter/build/web"
DIR="${PRUNE_WEB_DIR:-$DEFAULT_DIR}"
MODE="prune"
BUDGET_MIB=35
MIB=1048576
XTASK_MOD="$ROOT/xtask/src/code_stats/mod.rs"

usage() { sed -n '/^# ---- 用法/,/^# 退出码/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

while [ $# -gt 0 ]; do
  case "$1" in
    --check) MODE="check" ;;
    --dir) shift; DIR="${1:-}" ;;
    --dir=*) DIR="${1#*=}" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "[error] 未知参数：$1（--help 看用法）" >&2; exit 2 ;;
  esac
  shift || true
done

if [ -z "$DIR" ]; then echo "[error] --dir 需要一个目录参数" >&2; exit 2; fi

# 相对路径按仓库根解析（脚本内 cd 过，不改调用者的 cwd）。
case "$DIR" in
  /*) : ;;
  *) DIR="$ROOT/$DIR" ;;
esac

# ---- 渲染器判定（清单安全的前提）---------------------------------------------
detect_renderer() {
  if [ -n "${PRUNE_WEB_RENDERER:-}" ]; then printf '%s\n' "$PRUNE_WEB_RENDERER"; return 0; fi
  local boot="$DIR/flutter_bootstrap.js"
  if [ ! -f "$boot" ]; then
    echo "[FAIL] 找不到 $boot —— 无法判定渲染器（判红，不猜）" >&2
    return 1
  fi
  if grep -qE '"renderer"[[:space:]]*:[[:space:]]*"skwasm"' "$boot"; then printf 'skwasm\n'; return 0; fi
  if grep -qE '"renderer"[[:space:]]*:[[:space:]]*"canvaskit"' "$boot"; then printf 'canvaskit\n'; return 0; fi
  echo "[FAIL] $boot 里读不到 buildConfig 的 renderer —— 清减清单无法安全推导。" >&2
  echo "        人工确认渲染器后用 PRUNE_WEB_RENDERER=<canvaskit|skwasm> 覆盖（判红，不猜）。" >&2
  return 1
}

# 死重清单：逐行绝对路径。$1=renderer
kill_list() {
  local renderer="$1"
  # 两个 find 的集合**有交集**（wimp.js.symbols 同时命中 *.symbols 与 wimp*），
  # 必须 sort -u：否则执行模式会对同一文件 stat 第二次而中途失败（首跑实测踩到）。
  {
    # 1) 调试符号 sidecar：任何渲染器都不加载
    find "$DIR/canvaskit" -type f -name '*.symbols'
    # 2) 非当前渲染器的 wasm 变体（skwasm 构建的清单本轮未定义 ⇒ 只清符号，见 main）
    if [ "$renderer" = "canvaskit" ]; then
      find "$DIR/canvaskit" -type f \( -name 'skwasm*' -o -name 'wimp*' \) -not -name '*.symbols'
    fi
  } 2>/dev/null | sort -u
}

# 红线清单（--check 逐条断言存在且非空）
KEEP_LIST="
index.html
main.dart.js
flutter.js
flutter_bootstrap.js
canvaskit/canvaskit.js
canvaskit/canvaskit.wasm
canvaskit/chromium/canvaskit.js
canvaskit/chromium/canvaskit.wasm
assets/NOTICES
"

# 目录字节和：与 xtask code_stats 的 dir_size 同口径（常规文件字节和，symlink 跳过）
dir_bytes() { find "$1" -type f -printf '%s\n' | awk '{s+=$1} END{print s+0}'; }
dir_files() { find "$1" -type f | wc -l | tr -d ' '; }
mib() { awk -v b="$1" 'BEGIN{printf "%.2f", b/1048576}'; }

# xtask 预算漂移比对：判据真源是 xtask 常量，本脚本重复一份只为让产物门禁**不依赖
# xtask 二进制**（DECISION §2.2 第 3 点：独立探针）。两处不一致 = 判据漂移 ⇒ 判红。
xtask_budget_mib() {
  [ -f "$XTASK_MOD" ] || return 0
  sed -n 's/.*FLUTTER_WEB_BUDGET_MIB:[[:space:]]*u64[[:space:]]*=[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$XTASK_MOD" | head -1
}

if [ ! -d "$DIR" ]; then
  echo "[FAIL] 产物目录不存在：$DIR" >&2
  echo "       先构建：cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn" >&2
  echo "       （目录缺失判红，不当成「跳过」—— 那会退化成假绿）" >&2
  exit 2
fi

RENDERER="$(detect_renderer)" || exit 2

if [ "$RENDERER" = "skwasm" ]; then
  echo "[warn] 本构建渲染器 = skwasm：本轮清减清单只覆盖 canvaskit 构建。" >&2
  echo "       只清 *.symbols，**不删任何 wasm 变体**（skwasm 构建的回退变体边界未实测）。" >&2
fi

FAIL=0

# ---- check 模式 ---------------------------------------------------------------
if [ "$MODE" = "check" ]; then
  echo "==> 产物清减检查：$DIR（渲染器 = $RENDERER，预算 $BUDGET_MIB MiB）"

  KILLED=0
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if [ "$KILLED" = "0" ]; then echo "    [FAIL] 死重残留（应被清减）："; fi
    printf '           %s (%s B)\n' "${f#"$DIR"/}" "$(stat -c%s "$f")"
    KILLED=$((KILLED + 1))
  done < <(kill_list "$RENDERER")
  if [ "$KILLED" = "0" ]; then
    echo "    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）"
  else
    echo "           清减：scripts/prune_web_artifacts.sh --dir $DIR"
    FAIL=1
  fi

  MISSING=0
  for rel in $KEEP_LIST; do
    if [ ! -s "$DIR/$rel" ]; then
      echo "    [FAIL] 红线文件缺失/空：$rel（被清减脚本多删了？）"
      MISSING=$((MISSING + 1))
    fi
  done
  if [ "$MISSING" = "0" ]; then
    echo "    [ok]   红线文件齐全（$(printf '%s\n' $KEEP_LIST | wc -l | tr -d ' ') 项，含 chromium/ 与 base canvaskit）"
  else
    FAIL=1
  fi
  if [ ! -d "$DIR/font-fallback" ] || [ "$(dir_files "$DIR/font-fallback")" = "0" ]; then
    echo "    [FAIL] font-fallback/ 缺失或为空（E10 离线字体镜像红线）"
    FAIL=1
  fi

  BYTES="$(dir_bytes "$DIR")"
  LIMIT=$((BUDGET_MIB * MIB))
  if [ "$BYTES" -le "$LIMIT" ]; then
    echo "    [ok]   目录体积 $BYTES B = $(mib "$BYTES") MiB ≤ $BUDGET_MIB MiB（$(dir_files "$DIR") 个文件）"
  else
    echo "    [FAIL] 目录体积 $BYTES B = $(mib "$BYTES") MiB > $BUDGET_MIB MiB 预算"
    FAIL=1
  fi

  XB="$(xtask_budget_mib)"
  if [ -z "$XB" ]; then
    echo "    [warn] 读不到 $XTASK_MOD 的 FLUTTER_WEB_BUDGET_MIB（不影响本门禁）"
  elif [ "$XB" != "$BUDGET_MIB" ]; then
    echo "    [FAIL] 预算判据漂移：xtask=$XB MiB vs 本脚本=$BUDGET_MIB MiB"
    echo "           两处必须同值（改预算要同时改 xtask/src/code_stats/mod.rs 与本脚本并写明理由）"
    FAIL=1
  else
    echo "    [ok]   预算与 xtask FLUTTER_WEB_BUDGET_MIB 一致（$XB MiB）"
  fi

  if [ "$FAIL" = "0" ]; then echo "==> 通过"; else echo "==> 失败：见上面的 [FAIL]"; fi
  exit "$FAIL"
fi

# ---- 执行清减 ---------------------------------------------------------------
if [ ! -s "$DIR/index.html" ]; then
  echo "[error] $DIR 不像 Flutter Web 产物（缺 index.html）—— 拒绝清减" >&2
  exit 2
fi

BEFORE_BYTES="$(dir_bytes "$DIR")"
BEFORE_FILES="$(dir_files "$DIR")"
echo "==> 产物清减：$DIR（渲染器 = $RENDERER）"
echo "    清减前：$BEFORE_BYTES B = $(mib "$BEFORE_BYTES") MiB / $BEFORE_FILES 个文件"

REMOVED=0
REMOVED_BYTES=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  sz="$(stat -c%s "$f")"
  rm -f -- "$f"
  printf '    [-] %s (%s B)\n' "${f#"$DIR"/}" "$sz"
  REMOVED=$((REMOVED + 1))
  REMOVED_BYTES=$((REMOVED_BYTES + sz))
done < <(kill_list "$RENDERER")

AFTER_BYTES="$(dir_bytes "$DIR")"
AFTER_FILES="$(dir_files "$DIR")"
echo "==> 清减完成：删除 $REMOVED 项 / $REMOVED_BYTES B = $(mib "$REMOVED_BYTES") MiB"
echo "    清减后：$AFTER_BYTES B = $(mib "$AFTER_BYTES") MiB / $AFTER_FILES 个文件"
echo "    复验：$0 --check --dir $DIR（同时断言死重为 0、红线齐全、≤ $BUDGET_MIB MiB）"
