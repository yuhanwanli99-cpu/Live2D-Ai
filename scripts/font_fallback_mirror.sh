#!/usr/bin/env bash
# 字体回落镜像（R4-T4「字体回落结构性离线化」的可复现/可审计半边）。
#
# 引擎在「已打包字体缺这个字形」时会去下载 Noto 回落字体，默认 base URL 是
#   https://fonts.gstatic.com/s/
# 本项目把它改成同源相对路径（shell/flutter/web/flutter_bootstrap.js 的
# fontFallbackBaseUrl: 'font-fallback/'）。本脚本负责把**精选的整族**字体镜像到
#   shell/flutter/web/font-fallback/<family>/v<ver>/<file>.woff2
# 路径与引擎回落表的 url **逐字一致**（引擎做 Uri.parse(baseUrl).resolve(font.url)）。
#
# 用法：
#   scripts/font_fallback_mirror.sh            # 按引擎表下载/刷新镜像，重写 MANIFEST.txt
#   scripts/font_fallback_mirror.sh --check    # 只读本地文件 + MANIFEST.txt 校验（**断网可跑**）
#   scripts/font_fallback_mirror.sh --help
#
# 环境变量：
#   FONT_FAMILIES   覆盖镜像字族（空格分隔）；默认 5 族，见 DEFAULT_FAMILIES
#   FLUTTER_ROOT    覆盖 Flutter SDK 根（默认由 flutter 可执行文件位置推得）
#   MIRROR_BASE_URL 下载源；默认 https://fonts.gstatic.com/s/
#   TARGET_DIR      镜像目录；默认 <repo>/shell/flutter/web/font-fallback
#
# 纪律：**同一字族要么整族都在、要么整族都缺**。引擎把 404 当该字体「永久失败」
# （font_fallback_service.dart: _isPermanentStatus），半族会让一部分字形静默变
# 豆腐块，且不会报错。--check 因此同时校验「磁盘集合 == 引擎表里这些字族的全集」。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_FAMILIES="notocoloremoji notosanssymbols2 notosanssymbols notomusic notosansmath"

FAMILIES="${FONT_FAMILIES:-$DEFAULT_FAMILIES}"
MIRROR_BASE_URL="${MIRROR_BASE_URL:-https://fonts.gstatic.com/s/}"
TARGET_DIR="${TARGET_DIR:-$ROOT/shell/flutter/web/font-fallback}"
MANIFEST="$TARGET_DIR/MANIFEST.txt"

if [[ "$MIRROR_BASE_URL" != */ ]]; then
  echo "错误：MIRROR_BASE_URL 必须以 / 结尾（引擎用 Uri.resolve，缺尾斜杠会吃掉最后一段）" >&2
  exit 2
fi

usage() { sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

resolve_flutter_root() {
  if [[ -n "${FLUTTER_ROOT:-}" ]]; then printf '%s\n' "$FLUTTER_ROOT"; return; fi
  local exe
  exe="$(command -v flutter || true)"
  if [[ -z "$exe" ]]; then echo "错误：找不到 flutter（请把 \$HOME/flutter/bin 加进 PATH 或设 FLUTTER_ROOT）" >&2; exit 2; fi
  (cd "$(dirname "$exe")/.." && pwd)
}

FLUTTER_ROOT_RESOLVED="$(resolve_flutter_root)"
TABLE="$FLUTTER_ROOT_RESOLVED/bin/cache/flutter_web_sdk/lib/_engine/engine/font_fallback_data.dart"

# 引擎回落表（生成文件，勿手改）→ 这些字族的全部 url，按表内顺序输出。
engine_urls() {
  python3 - "$TABLE" "$FAMILIES" <<'PY'
import re, sys
table, fams = sys.argv[1], set(sys.argv[2].split())
src = open(table, encoding='utf-8').read()
for _name, url in re.findall(r"NotoFont\(\s*'([^']*)',\s*'([^']*)'", src):
    if url.split('/')[0] in fams:
        print(url)
PY
}

require_table() {
  if [[ ! -f "$TABLE" ]]; then
    echo "错误：引擎回落表不存在：$TABLE" >&2
    echo "      （用 FLUTTER_ROOT 指向正确的 Flutter SDK；本文件随 SDK 一同安装）" >&2
    exit 2
  fi
}

mode="${1:-mirror}"
case "$mode" in
  --help|-h) usage; exit 0 ;;
  --check) ;;
  mirror|--mirror) ;;
  *) echo "未知参数：$mode（见 --help）" >&2; exit 2 ;;
esac

require_table
mapfile -t URLS < <(engine_urls)
if [[ "${#URLS[@]}" -eq 0 ]]; then
  echo "错误：引擎表里找不到字族 『$FAMILIES』 的任何文件（字族名拼写？Flutter 升级改名？）" >&2
  exit 1
fi

family_of() { printf '%s\n' "${1%%/*}"; }

print_expected_summary() {
  local url fam
  declare -A counts=()
  for url in "${URLS[@]}"; do fam="$(family_of "$url")"; counts[$fam]=$(( ${counts[$fam]:-0} + 1 )); done
  echo "引擎表字族（整族镜像，共 ${#URLS[@]} 文件）："
  for fam in $FAMILIES; do printf '  %-18s %s\n' "$fam" "${counts[$fam]:-0}"; done
}

# ─────────────────────────── --check：只读本地 ───────────────────────────
if [[ "$mode" == "--check" ]]; then
  fail=0
  echo "== font_fallback_mirror.sh --check =="
  echo "TARGET_DIR  = $TARGET_DIR"
  echo "引擎回落表  = $TABLE"
  echo "（本节不访问网络：只读本地文件与 MANIFEST.txt）"
  echo
  print_expected_summary
  echo

  if [[ ! -f "$MANIFEST" ]]; then
    echo "FAIL 缺少清单文件：$MANIFEST" >&2
    echo "RESULT: FAIL"; exit 1
  fi

  declare -A expected=() on_disk=()
  manifest_rows=0
  while IFS=$'\t' read -r fam url bytes sha; do
    if [[ -z "${fam:-}" || "$fam" == \#* ]]; then continue; fi
    manifest_rows=$((manifest_rows+1))
    expected["$url"]="$bytes $sha"
    f="$TARGET_DIR/$url"
    if [[ ! -f "$f" ]]; then
      echo "FAIL 缺失文件    $url"; fail=1; continue
    fi
    got_bytes="$(stat -c %s "$f")"
    got_sha="$(sha256sum "$f" | cut -d' ' -f1)"
    if [[ "$got_bytes" != "$bytes" ]]; then
      echo "FAIL 字节数不符  $url 清单=$bytes 实际=$got_bytes"; fail=1
    fi
    if [[ "$got_sha" != "$sha" ]]; then
      echo "FAIL sha256 不符 $url 清单=$sha 实际=$got_sha"; fail=1
    fi
  done < "$MANIFEST"

  # 磁盘实测集合（排除三个非字体文件）
  while IFS= read -r rel; do
    case "$rel" in
      MANIFEST.txt|OFL.txt|README.md) continue ;;
    esac
    on_disk["$rel"]=1
    if [[ -z "${expected[$rel]:-}" ]]; then
      echo "FAIL 多出文件    $rel（不在清单里 ⇒ 清单不再可信）"; fail=1
    fi
  done < <(cd "$TARGET_DIR" && find . -type f -printf '%P\n' | sed 's|^\./||' | sort)

  # 完整性：磁盘集合必须 == 引擎表里这些字族的全集（不许半族）
  missing=0 extra=0
  for url in "${URLS[@]}"; do
    if [[ -z "${expected[$url]:-}" ]]; then echo "FAIL 引擎表有、清单没有（半族！）：$url"; missing=$((missing+1)); fi
  done
  for url in "${!expected[@]}"; do
    found=0
    for e in "${URLS[@]}"; do if [[ "$e" == "$url" ]]; then found=1; break; fi; done
    if [[ "$found" != 1 ]]; then echo "FAIL 清单有、引擎表没有（陈旧条目）：$url"; extra=$((extra+1)); fi
  done
  if [[ "$missing" -ne 0 || "$extra" -ne 0 ]]; then fail=1; fi

  total_bytes=0
  for url in "${!expected[@]}"; do total_bytes=$(( total_bytes + $(stat -c %s "$TARGET_DIR/$url" 2>/dev/null || echo 0) )); done
  echo
  echo "清单条目 $manifest_rows · 磁盘字体文件 ${#on_disk[@]} · 引擎表期望 ${#URLS[@]} · 合计 $total_bytes 字节"
  if [[ "$fail" -eq 0 ]]; then
    echo "RESULT: PASS（清单 == 磁盘 == 引擎表全集；逐文件 sha256/bytes 相符）"
    exit 0
  fi
  echo "RESULT: FAIL" >&2
  exit 1
fi

# ─────────────────────────── 默认：下载/刷新 ───────────────────────────
echo "== font_fallback_mirror.sh（下载镜像） =="
echo "来源        = $MIRROR_BASE_URL"
echo "目标        = $TARGET_DIR"
echo "引擎回落表  = $TABLE"
print_expected_summary
echo
mkdir -p "$TARGET_DIR"
tmp_manifest="$(mktemp)"
trap 'rm -f "$tmp_manifest"' EXIT

{
  echo "# font-fallback mirror manifest —— 由 scripts/font_fallback_mirror.sh 生成，请勿手改。"
  echo "# 用途：scripts/font_fallback_mirror.sh --check 逐文件核对 bytes/sha256（只读本地，断网可跑）。"
  echo "# 来源：$MIRROR_BASE_URL （Noto 回落字体；上游许可见同目录 OFL.txt）"
  echo "# 引擎回落表：\$FLUTTER_ROOT/bin/cache/flutter_web_sdk/lib/_engine/engine/font_fallback_data.dart"
  echo "# 运行期消费方：shell/flutter/web/flutter_bootstrap.js 的 fontFallbackBaseUrl=\"font-fallback/\""
  echo "# 字族（整族镜像）：$FAMILIES"
  printf '# 列：<family>\t<url>\t<bytes>\t<sha256>\n'
} > "$tmp_manifest"

downloaded=0
for url in "${URLS[@]}"; do
  dest="$TARGET_DIR/$url"
  mkdir -p "$(dirname "$dest")"
  tmp="$(mktemp "$dest.XXXXXX")"
  if ! curl -fsSL --retry 3 --retry-delay 1 --max-time 60 -o "$tmp" "$MIRROR_BASE_URL$url"; then
    rm -f "$tmp"
    echo "FAIL 下载失败：$MIRROR_BASE_URL$url" >&2
    exit 1
  fi
  bytes="$(stat -c %s "$tmp")"
  if [[ "$bytes" -le 0 ]]; then rm -f "$tmp"; echo "FAIL 下载为空：$url" >&2; exit 1; fi
  mv "$tmp" "$dest"
  sha="$(sha256sum "$dest" | cut -d' ' -f1)"
  printf '%s\t%s\t%s\t%s\n' "$(family_of "$url")" "$url" "$bytes" "$sha" >> "$tmp_manifest"
  printf '  ok %8s  %s\n' "$bytes" "$url"
  downloaded=$((downloaded+1))
done

mv "$tmp_manifest" "$MANIFEST"
trap - EXIT
echo
echo "已写入 $downloaded 个文件 + $MANIFEST"
echo "自检："
"${BASH_SOURCE[0]}" --check
