#!/usr/bin/env bash
# Live2D-Ai 点火脚本（2026-09-10 定，2026-10-13 改「等就绪再给地址」）——核心链路闭环。
#
# 它做五件事：
#   1) 预检：读 live2d-ai.toml（LLM/TTS 端点 + TTS 是否本机），探测云端端点；
#   2) 确保渲染面产物：crates/l2d-wasm-demo/dist（/render 用）；
#   3) 确保前端产物：shell/flutter/build/web（/app/ 用，可选 --build 自动构建）；
#   4) 后台启动 live2d-ai-desktop --web，脚本前台等待：先等 GET /app/ = 200
#      （最长 90 秒）；本机 TTS 再等 GET {tts.base_url}/models = 200（最长 600 秒）；
#      服务进程退出 = 提前停下。
#   5) 全部就绪才打印点火入口，并用默认浏览器打开页面（WSL 走 Windows 的
#      Start-Process；LIVE2D_AI_NO_BROWSER=1 只打印）。页面未就绪或本机 TTS
#      超时：不打印成功横幅、不打开浏览器。脚本不退出（服务停了它才停）；
#      点火体检另开终端跑 --check。
#
# 用法：
#   ./scripts/ignite.sh                 # 预检 + 启动 + 等就绪 + 打开浏览器
#   ./scripts/ignite.sh --build         # 先构建 Rust + Flutter Web 再启动
#   ./scripts/ignite.sh --port 18100    # 指定端口（默认 18080）
#   ./scripts/ignite.sh --check [--port N]  # 只对**已启动**的服务做点火体检，不自启
#   ./scripts/ignite.sh --check-dir DIR     # 只对**构建产物目录**做离线体检（不需要服务；CI 用）
#
# 服务在后台启动，本脚本前台等待就绪并保持不退出（Ctrl+C 会停掉服务）。
#
# 说明：本地 LLM / 云端 TTS 由你自己提供（任何 OpenAI 兼容实现）；**本机 TTS
#       （MeloTTS / CosyVoice3）由桌面进程拉起**，GET /models = 200 之前不打印
#       页面地址。本脚本不下载模型、不启动推理进程。两个本地 Mod 也会在后台自动探活。

set -uo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

# 前端产物目录一律锚定到**本仓库根**（上面已 cd 到根）。
# 后端只认 `LIVE2D_AI_FLUTTER_WEB_DIR` + 若干 cwd 相对候选，cwd 一变就退化成
# `/app/` 503；这里显式给绝对路径，使那种误判从根上不可能。
# 若外部已设成别的值，明说会被覆盖——否则用户会以为它生效了。
if [ -n "${LIVE2D_AI_FLUTTER_WEB_DIR:-}" ] && [ "$LIVE2D_AI_FLUTTER_WEB_DIR" != "$PWD/shell/flutter/build/web" ]; then
  echo "[warn] 忽略外部 LIVE2D_AI_FLUTTER_WEB_DIR=$LIVE2D_AI_FLUTTER_WEB_DIR，改用仓库内产物目录"
fi
export LIVE2D_AI_FLUTTER_WEB_DIR="$PWD/shell/flutter/build/web"

# 导入 ./.env（如 DEEPSEEK_API_KEY）。
#
# 2026-09-12（rc.2）起 **`.env` 就是密钥的唯一真源**：Rust 侧自己读 `.env`
# （`live2d_ai_runtime::secrets`，优先级 `.env` > 进程环境），前端也能经
# `PUT /api/v1/env` 写它并热重载。所以下面这段 `set -a; . ./.env` **不再是
# 必需**——保留只是为了兼容「别的工具/Mod 也读进程环境」的用法。
# 只回显变量名，不回显值。
if [ -f .env ]; then
  set -a; . ./.env; set +a
  echo "==> 已导出 .env 到进程环境（Rust 侧也直接读 .env）：$(grep -oE "^[A-Z_]+=" .env | tr -d "=" | tr "\n" " ")"
fi

# loopback 一律不经代理：本机 LLM / TTS（127.0.0.1:8091 等）不该被丢给 http_proxy。
# 本机实测：环境里 `no_proxy=…,127.*` 这个 **glob 对 reqwest 无效**（它只认字面量），
# 于是连不上 127.0.0.1:1 的用例被代理回成 502、本机 8091 的请求也可能绕道代理。
# 这里补上**字面量**（幂等：已有则原样带上），代理继续只管外网。
export NO_PROXY="${NO_PROXY:+$NO_PROXY,}127.0.0.1,localhost,::1"
export no_proxy="${no_proxy:+$no_proxy,}127.0.0.1,localhost,::1"

PORT=18080
DO_BUILD=0
DO_CHECK=0
DO_CHECK_DIR=""
DO_CHECK_DIR_SET=0
while [ $# -gt 0 ]; do
  case "$1" in
    --build) DO_BUILD=1 ;;
    --check) DO_CHECK=1 ;;
    --check-dir) shift; DO_CHECK_DIR_SET=1; DO_CHECK_DIR="${1:-}" ;;
    --check-dir=*) DO_CHECK_DIR="${1#*=}" ;;
    --port) shift; PORT="${1:-18080}" ;;
    --port=*) PORT="${1#*=}" ;;
    -h|--help) sed -n "2,23p" "$0"; exit 0 ;;
    *) echo "[warn] 未知参数：$1" ;;
  esac
  shift || true
done

# ---- 0) 点火体检（--check）------------------------------------------------
# 只戳**已经在跑**的那个服务，验证三件事：`/` 302 到 `/app/`、`/app/` 200、
# 产物不引用 Google CDN 的 CanvasKit。断言写在脚本里，是因为这三条都只有
# 「真的起过一次服务」才验得到——单元测试覆盖不到托管层与产物内容。
# 不自启服务（否则端口冲突时的失败会伪装成「探针失败」）。
# ---- CDN 探针判据（唯一真源）------------------------------------------------
# 清单与判定都在 scripts/lib/cdn_probe.sh —— 与 scripts/ignition-precheck.sh
# **共用同一份**。不要在本地再抄一份（F-0050-01：错误探针被复制到两处）。
CDN_PROBE_LIB="$PWD/scripts/lib/cdn_probe.sh"
if [ ! -f "$CDN_PROBE_LIB" ]; then
  echo "[error] 缺少 $CDN_PROBE_LIB —— 离线红线判据不可用（判红，不静默跳过）"
  exit 2
fi
# shellcheck source=scripts/lib/cdn_probe.sh
. "$CDN_PROBE_LIB"

if [ "$DO_CHECK" = "1" ]; then
  base="http://127.0.0.1:${PORT}"
  fail=0
  echo "==> 点火体检：$base（--check 不自启服务；请先在另一终端跑 ./scripts/ignite.sh）"

  headers=$(curl -s -o /dev/null -D - --max-time 5 "$base/" 2>/dev/null || true)
  root_code=$(printf '%s\n' "$headers" | awk 'NR==1{print $2}')
  root_loc=$(printf '%s\n' "$headers" | awk 'tolower($1)=="location:"{print $2}' | tr -d '\r' | head -1)
  if [ "$root_code" = "302" ] && [ "$root_loc" = "/app/" ]; then
    echo "    [ok]   GET / → 302 Location: /app/"
  else
    echo "    [FAIL] GET / 期望 302 → /app/，实得 ${root_code:-无响应} Location=${root_loc:-无}"
    fail=1
  fi

  app_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$base/app/" 2>/dev/null || echo 000)
  if [ "$app_code" = "200" ]; then
    echo "    [ok]   GET /app/ → 200"
  else
    echo "    [FAIL] GET /app/ 期望 200，实得 $app_code"
    fail=1
  fi

  # CDN 依赖只看**服务实际吐出的字节**（与浏览器拿到的一致），不看本地文件。
  # 探针清单与判定在下方 CDN_PROBE_SPEC / cdn_scan_http（**唯一真源**）。
  cdn_scan_http "$base" || fail=1

  if [ "$fail" = "0" ]; then
    echo "==> 体检通过（Windows 浏览器打开 ${base}/app/ 做最后一步肉眼看/听）"
  else
    echo "==> 体检失败：见上面的 [FAIL]"
  fi
  exit "$fail"
fi

# ---- 0b) 产物离线体检（--check-dir）：只看构建产物目录，不需要服务 --------
# 给 CI 用：flutter build web --no-web-resources-cdn 之后直接判产物。
# 红线 K 的失效面在**构建期**，这一层就能抓住；--check 那层再多验托管层。
if [ "$DO_CHECK_DIR_SET" = "1" ]; then
  if [ -z "$DO_CHECK_DIR" ]; then
    echo "[error] --check-dir 需要一个目录参数"
    exit 2
  fi
  echo "==> 产物离线体检：$DO_CHECK_DIR（--check-dir；不需要服务）"
  if cdn_scan_dir "$DO_CHECK_DIR"; then
    echo "==> 产物体检通过（未发现 Google CDN CanvasKit 依赖）"
    exit 0
  fi
  echo "==> 产物体检失败：见上面的 [FAIL]"
  exit 1
fi

echo "==> Live2D-Ai 点火检查"

# ---- 1) 读配置里的 LLM/TTS 端点 + TTS 是否本机 ------------------------------
if ! command -v python3 >/dev/null 2>&1; then
  echo "[error] 需要 python3 读取 live2d-ai.toml"; exit 1
fi
# 三行输出 + mapfile：不用 `read -r a b c`（空 TTS 会被 `read` 吃掉，行数错位），
# 也不用 `print(a, b, c)`（同一原因）。主机名按 urlparse 归一（127.0.0.1 /
# localhost / ::1 视为本机 = tts_local=1）。
mapfile -t ENDPOINTS < <(python3 - <<'PY'
import pathlib, tomllib
from urllib.parse import urlparse
llm = "http://127.0.0.1:11434/v1"
tts = ""
p = pathlib.Path("live2d-ai.toml")
if p.is_file():
    d = tomllib.loads(p.read_text(encoding="utf-8"))
    llm = d.get("llm", {}).get("base_url", llm)
    tts = d.get("tts", {}).get("base_url", "")
host = (urlparse(tts).hostname or "").lower()
print(llm)
print(tts)
print("1" if host in ("127.0.0.1", "localhost", "::1") else "0")
PY
)
LLM_BASE="${ENDPOINTS[0]}"
TTS_BASE="${ENDPOINTS[1]}"
TTS_LOCAL="${ENDPOINTS[2]}"
echo "    配置 LLM base : $LLM_BASE"
echo "    配置 TTS base : ${TTS_BASE:-（未配置，选型待定）}"
echo "    TTS 本机      : $([ "$TTS_LOCAL" = "1" ] && echo 是 || echo 否)"

probe() {
  local name="$1" base="$2"
  local b="${base%/}"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 4 "${b}/models" 2>/dev/null || echo 000)
  if [ "$code" = "200" ]; then
    echo "    [ok]   $name 就绪：${b}/models"
    return 0
  fi
  # 云端端点（如 DeepSeek）需鉴权：401/403 说明**可达**，密钥由 api_key_env 注入。
  if [ "$code" = "401" ] || [ "$code" = "403" ]; then
    echo "    [ok]   $name 可达（HTTP $code，需 API key —— 由配置里的 api_key_env 提供）"
    return 0
  fi
  if curl -fsS --max-time 4 "${b%/v1}/api/tags" >/dev/null 2>&1; then
    echo "    [ok]   $name 就绪（ollama /api/tags）"
    return 0
  fi
  echo "    [warn] $name 未就绪：$base（继续；Mod 会在后台重试探活，也可先启动你的实现）"
  return 1
}
echo "==> 探测本地实现（OpenAI 标准接口）"
probe "LLM" "$LLM_BASE" || true
if [ "$TTS_LOCAL" = "1" ]; then
  # 本机 TTS 由桌面进程**拉起**：预检探不到是常态（模型加载要几十秒到几分钟），
  # 不再报「未就绪，继续」——真正的就绪判据在启动后的就绪等待里（GET /models = 200）。
  echo "    [ok]   TTS 本机（${TTS_BASE%/}/models）：由本次点火拉起；GET /models = 200 之后才给页面地址"
elif [ -n "$TTS_BASE" ]; then
  probe "TTS" "$TTS_BASE" || true
else
  echo "    [skip] TTS 未配置（选型待定）：本轮只跑「文本 → LLM」，不出声、不驱动口型"
fi

# ---- 2) 渲染面产物（/render） --------------------------------------------
if [ ! -f "crates/l2d-wasm-demo/dist/index.html" ]; then
  echo "==> 渲染面产物缺失，正在用 trunk 构建 l2d-wasm-demo …"
  # trunk 会先跑 `cargo metadata`（需网络解析全平台依赖）；
  # 且 NO_COLOR=1 会被它当成 `--no-color 1` 报错，故先 unset。
  (cd crates/l2d-wasm-demo && unset NO_COLOR && if [ -n "${https_proxy:-}" ]; then export CARGO_HTTP_PROXY="$https_proxy"; fi && trunk build)
fi

# ---- 3) 前端产物（/app/） ------------------------------------------------
# `--no-web-resources-cdn` 不是可选项：缺省构建会把 CanvasKit 指向
# https://www.gstatic.com/flutter-canvaskit/<engineRevision>/，而本机
# build/web/canvaskit/ 那份 37 MB 副本根本不被引用 —— 于是**断网即白屏**。
# 本项目的定位是本地优先的桌宠，不允许依赖 Google CDN。
FLUTTER_BUILD_FLAGS="--release --base-href /app/ --no-web-resources-cdn"
if [ ! -f "shell/flutter/build/web/index.html" ]; then
  if [ "$DO_BUILD" = "1" ]; then
    echo "==> 构建 Flutter Web（--base-href /app/ --no-web-resources-cdn）…"
    (cd shell/flutter && flutter pub get && flutter build web $FLUTTER_BUILD_FLAGS)
  else
    echo "[warn] Flutter Web 产物缺失：shell/flutter/build/web/index.html"
    echo "        运行 ./scripts/ignite.sh --build，或手动：cd shell/flutter && flutter build web $FLUTTER_BUILD_FLAGS"
  fi
elif grep -q 'gstatic\.com/flutter-canvaskit' shell/flutter/build/web/main.dart.js 2>/dev/null; then
  # 挡住「产物早就存在、所以 --build 不会重建」这个陷阱：CDN 依赖只在**构建期**
  # 决定，启动期补不了。这里主动探测并吼一声，否则问题只在断网时才暴露。
  echo "[warn] 前端产物仍引用 Google CDN 的 CanvasKit —— **断网会白屏**！"
  echo "        必须重新构建（--build 对已存在的产物不会自动重建）：
        cd shell/flutter && flutter build web $FLUTTER_BUILD_FLAGS"
fi

# ---- 4) 后端二进制 -------------------------------------------------------
if [ "$DO_BUILD" = "1" ] || [ ! -x "target/debug/live2d-ai-desktop" ]; then
  echo "==> 构建 live2d-ai-desktop …"
  cargo build -p live2d-ai-desktop
fi

pkill -f "live2d-ai-desktop --web" 2>/dev/null || true
sleep 0.5

# 本地 TTS 子进程（MeloTTS / CosyVoice3）是**外部长驻进程**，由桌面端 spawn。
# 桌面端被 **SIGKILL/强杀**（不走 shutdown()）时，它们会被 reparent 成孤儿并继续
# 占着端口 —— 本机实测：8091 上残留旧 melo → 新 melo 绑定 `Address already in use`
# 立即退出 → 主链对着卡死的旧进程发请求得到 `tts_upstream_502`。
#
# 新代码已给 spawn 出的子进程装 `PR_SET_PDEATHSIG`（父死即内核收掉，见
# `crates/live2d-ai-mod-local-tts/src/child.rs`）；但**升级之前**留下的孤儿还在，
# 这里按**仓库内路径**精确点名清掉（只认本仓的 melo/serve.py 与 engine/，
# 不碰其它进程），使生命周期与刚修好的口径一致。
for pat in "crates/live2d-ai-mod-local-tts/melo/serve.py" \
           "crates/live2d-ai-mod-local-tts/engine/"; do
  stale="$(pgrep -f "$PWD/$pat" 2>/dev/null | tr '\n' ' ' || true)"
  if [ -n "${stale// /}" ]; then
    echo "[warn] 清理残留的本地 TTS 子进程（旧桌面被强杀留下的孤儿）：$pat → pid ${stale}"
    pkill -f "$PWD/$pat" 2>/dev/null || true
  fi
done
sleep 0.5

echo
echo "==> 启动 web 服务（音频单源=browser：由浏览器出声并驱动口型）"
echo "==> 就绪判据：GET /app/ = 200（本机 TTS 还要再等 GET {tts}/models = 200），"
echo "    就绪前**不给**页面地址、不开浏览器。服务在后台，本脚本前台等待。"

./target/debug/live2d-ai-desktop --web --http-port "${PORT}" &
SVC_PID=$!

# INT/TERM：进程还活着就把它杀掉（已在退出的进程不再补一刀）。
cleanup() {
  if kill -0 "$SVC_PID" 2>/dev/null; then
    kill "$SVC_PID" 2>/dev/null || true
  fi
}
trap cleanup INT TERM

# ---- 5a) 等页面就绪：GET /app/ = 200，超时 90 秒；进程先死就停。------------
page_ready() {
  local deadline=$((SECONDS + 90)) code
  while :; do
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 4 "http://127.0.0.1:${PORT}/app/" 2>/dev/null || echo 000)
    if [ "$code" = "200" ]; then
      return 0
    fi
    if ! kill -0 "$SVC_PID" 2>/dev/null; then
      echo "[error] 服务进程已退出（页面从未就绪，GET /app/ 一直是 $code）——不打开浏览器"
      exit 1
    fi
    if [ $SECONDS -ge "$deadline" ]; then
      echo "[error] 90 秒内页面未就绪（GET /app/ 一直是 $code）——不打开浏览器；日志见上方服务输出"
      exit 1
    fi
    sleep 1
  done
}
echo "==> 等待页面就绪（GET http://127.0.0.1:${PORT}/app/ = 200，最长 90 秒）…"
page_ready
echo "    [ok] 页面就绪：GET /app/ = 200"

# ---- 5b) 仅本机 TTS：等 GET {tts}/models = 200，超时 600 秒，15 秒一行「仍在等待」。
if [ "$TTS_LOCAL" = "1" ]; then
  TTS_MODELS_URL="${TTS_BASE%/}/models"
  tts_ready() {
    local tts_deadline=$((SECONDS + 600)) code last_note=0
    while :; do
      code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 4 "$TTS_MODELS_URL" 2>/dev/null || echo 000)
      if [ "$code" = "200" ]; then
        return 0
      fi
      if ! kill -0 "$SVC_PID" 2>/dev/null; then
        echo "[error] 服务进程已退出，本机 TTS 不会就绪——不打开浏览器"
        exit 1
      fi
      if [ $SECONDS -ge "$tts_deadline" ]; then
        echo "[error] 600 秒内本机 TTS 未就绪（GET $TTS_MODELS_URL 一直是 $code）——不打开浏览器；"
        echo "        页面其实已就绪，可稍后手动打开 http://127.0.0.1:${PORT}/app/"
        exit 1
      fi
      if [ $((SECONDS - last_note)) -ge 15 ]; then
        echo "    仍在等待本机 TTS（GET $TTS_MODELS_URL = $code，剩余 $((tts_deadline - SECONDS)) 秒）"
        last_note=$SECONDS
      fi
      sleep 2
    done
  }
  echo "==> 等待本机 TTS 就绪（GET $TTS_MODELS_URL = 200，最长 600 秒；由本次点火拉起）…"
  tts_ready
  echo "    [ok] 本机 TTS 就绪：GET $TTS_MODELS_URL = 200"
fi

# ---- 5c) 全部就绪：横幅 + 默认浏览器。--------------------------------------
APP_URL="http://127.0.0.1:${PORT}/app/"
echo
echo "=========================================================="
echo "  点火入口： $APP_URL"
echo "  渲染面  ： http://127.0.0.1:${PORT}/render"
echo "  Mod 状态： http://127.0.0.1:${PORT}/api/v1/mods"
echo "  自测清单： docs/verification/ignition-core-loop.md"
if [ "$TTS_LOCAL" = "1" ]; then
  echo "  本地 TTS： GET $TTS_MODELS_URL = 200（本机，由本次点火拉起）"
fi
echo "=========================================================="
echo

if [ "${LIVE2D_AI_NO_BROWSER:-0}" = "1" ]; then
  echo "==> LIVE2D_AI_NO_BROWSER=1：不自动打开浏览器（地址在上面）"
elif [ -r /proc/version ] && grep -qi microsoft /proc/version && command -v powershell.exe >/dev/null 2>&1; then
  # WSL：本脚本（在 WSL 里）没有默认浏览器，交给 Windows 侧的 Start-Process。
  echo "==> 用 Windows 默认浏览器打开 $APP_URL"
  powershell.exe -NoProfile -Command "Start-Process '$APP_URL'" >/dev/null 2>&1 &
else
  if command -v xdg-open >/dev/null 2>&1; then
    echo "==> 用默认浏览器打开 $APP_URL"
    xdg-open "$APP_URL" >/dev/null 2>&1 &
  else
    echo "[warn] 找不到可用的打开方式（WSL 无 powershell.exe，本机无 xdg-open），请手动打开：$APP_URL"
  fi
fi

# ---- 5d) 脚本不退出：前台等服务，服务停了才停。------------------------------
wait "$SVC_PID"
exit $?
