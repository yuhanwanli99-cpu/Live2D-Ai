#!/usr/bin/env bash
# Live2D-Ai 点火脚本（2026-09-10 定，2026-10-13 改「等就绪再给地址」，2026-10-20 0.3.11
# 撤掉「等本机 TTS 就绪」与启动前清理 Melo 子进程）——核心链路闭环。
#
# 它做五件事：
#   1) 预检：读 live2d-ai.toml 的 LLM/TTS 端点，只探测**当前写下的**那一份地址
#      （只报告，不改档、不写 toml）；
#   2) 确保渲染面产物：crates/l2d-wasm-demo/dist（/render 用）；
#   3) 确保前端产物：shell/flutter/build/web（/app/ 用，可选 --build 自动构建）；
#   4) 后台启动 live2d-ai-desktop --web，脚本前台等待：GET / = 200（启动器页，
#      最长 90 秒；**不再**为 [tts].mode=local 等 GET {tts}/models 600 秒）。
#      旧构建（/ 仍是 302 → /app/）回落等 GET /app/ = 200——这是启动器落地前的
#      兼容，不删。服务进程退出 = 提前停下。
#   5) 就绪后打印点火入口 http://127.0.0.1:${PORT}/（启动器页）并用默认浏览器打开
#      （WSL 走 Windows 的 Start-Process；LIVE2D_AI_NO_BROWSER=1 只打印）。页面未就绪
#      不打印成功横幅、不打开浏览器。脚本不退出（服务停了它才停）；点火体检另开终端
#      跑 --check。
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
# 说明（0.3.11）：本脚本**不拉起任何 Mod**，也不跑 melo/start.sh；缺省二进制不拥有那些
#       进程，所以也**不清理**用户自己拉起的 Melo。启动器页上的「预启动」开关由页面按
#       现有启停接口触发。本脚本不下载模型、不启动推理进程。

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
    -h|--help) sed -n '2,/^$/p' "$0"; exit 0 ;;
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

# 取 HTTP 状态码：**只留三位**。
#
# curl -w "%{http_code}" 在失败时自己会打印 000 并返回非 0，再跟一句
# || echo 000 就会把失败码拼成 000000（执行单 §4 点名的旧写法）。
# 这里统一成「拿不到就是 000」，调用方只比较三位串。
http_code() {
  local url="$1" max="$2" code
  max="${max:-4}"
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time "$max" "$url" 2>/dev/null || true)
  printf '%s' "${code:0:3}"
}

if [ "$DO_CHECK" = "1" ]; then
  base="http://127.0.0.1:${PORT}"
  fail=0
  echo "==> 点火体检：$base（--check 不自启服务；请先在另一终端跑 ./scripts/ignite.sh）"

  # 0.3.11：GET / 是启动器页（200 + 标记），不再是 302 到 /app/。
  root_code=$(http_code "$base/" 5)
  root_body=$(curl -s --max-time 5 "$base/" 2>/dev/null || true)
  if [ "$root_code" = "200" ] && grep -q "live2d-ai-launcher" <<<"$root_body"; then
    echo "    [ok]   GET / → 200 启动器页（含 live2d-ai-launcher 标记）"
    # 启动器正文同样不得引用 Google CDN（离线红线；与 /app/ 产物同一份判据）。
    cdn_judge "/" static "$root_body" || fail=1
  else
    echo "    [FAIL] GET / 期望 200 启动器页（正文含 live2d-ai-launcher），实得 ${root_code:-无响应}"
    fail=1
  fi

  app_code=$(http_code "$base/app/" 10)
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
    echo "==> 体检通过（Windows 浏览器打开 ${base}/ 启动器页做最后一步肉眼看/听）"
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
# 四行输出 + mapfile：不用 `read -r a b c`（空 TTS 会被 `read` 吃掉，行数错位），
# 也不用 `print(a, b, c, d)`（同一原因）。
#
# **0.3.9：就绪等待只认 `[tts].mode`**（"local" 缺省 / "cloud"）。旧口径按
# base_url 主机名判「是否本机」——云端模式下把地址填成局域网/本机代理时会去等一个
# 永远不会起的 Melo；现在 mode=cloud 一律不等（核心只打 [tts].base_url，
# 出声端点在云端，本机 Melo 进程与本跳无关）。
mapfile -t ENDPOINTS < <(python3 - <<'PY'
import pathlib, tomllib
from urllib.parse import urlparse
llm = "http://127.0.0.1:11434/v1"
tts = ""
mode = "local"
p = pathlib.Path("live2d-ai.toml")
if p.is_file():
    d = tomllib.loads(p.read_text(encoding="utf-8"))
    llm = d.get("llm", {}).get("base_url", llm)
    tts = d.get("tts", {}).get("base_url", "")
    mode = d.get("tts", {}).get("mode", "local")
host = (urlparse(tts).hostname or "").lower()
print(llm)
print(tts)
print(mode)
print("1" if host in ("127.0.0.1", "localhost", "::1") else "0")
PY
)
LLM_BASE="${ENDPOINTS[0]}"
TTS_BASE="${ENDPOINTS[1]}"
TTS_MODE="${ENDPOINTS[2]}"
TTS_HOST_LOCAL="${ENDPOINTS[3]}"
# 0.3.11：点火**不再**等本机 TTS 就绪。
#
# 旧行为（0.3.9–0.3.10）：[tts].mode=local 时空等 GET {tts}/models 最长 600 秒，
# 超时 exit 1——维护者 2026-10-20 的点火日志里服务明明已就绪却卡了 10 分钟。
# 现在：不等；缺省二进制也不拉起 Mod / melo/start.sh，Melo 进程由用户自己准备。
#
# 判据仍**只认 [tts].mode**（不看 base_url 主机名）：cloud 继续「不等本机进程」，
# local 也无条件不等。pytest 对这两句文案有断言（tests/test_shell_scripts_syntax.py）。
#
# 2026-10-19 修（rc.4 验证抓到）：这里原来把两句话写成
# `echo "...（$([ ... ] && echo 甲 || echo 乙））"`——用**全角** `）` 去收
# `$( ... )`，而命令替换只认 ASCII `)`，于是 `$(` 一直不闭合，bash 读到
# EOF 报 `unexpected EOF while looking for matching "`（**整脚本跑不起来**）。
# 现在先算成两个变量再插值：既避开全角/半角陷阱，也不再在双引号里嵌套双引号。
TTS_WAIT_TEXT="云端端点，不等本机进程"
if [ "$TTS_MODE" = "local" ]; then
  TTS_WAIT_TEXT="本机端点，点火不等它就绪（缺省不自动拉起）"
fi
if [ "$TTS_HOST_LOCAL" = "1" ]; then
  TTS_HOST_TEXT="本机 / 局域网"
else
  TTS_HOST_TEXT="远程"
fi
echo "    配置 LLM base : $LLM_BASE"
echo "    配置 TTS base : ${TTS_BASE:-（未配置，选型待定）}"
echo "    配置 TTS mode : $TTS_MODE（$TTS_WAIT_TEXT）"
echo "    TTS 地址主机  : $TTS_HOST_TEXT"

probe() {
  local name="$1" base="$2"
  local b="${base%/}"
  local code
  code=$(http_code "${b}/models" 4)
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
  echo "    [warn] $name 未就绪：$base（继续；也可先启动你的实现）"
  return 1
}
# 只探测**当前写下的那一份**地址（执行单 §5：只报告，不改 mode / base_url、
# 不写 toml）。探不到只是 [warn]，不阻塞启动与地址打印。
echo "==> 探测当前端点（只报告，不改档）"
probe "LLM" "$LLM_BASE" || true
if [ -n "$TTS_BASE" ]; then
  probe "TTS" "$TTS_BASE" || true
else
  echo "    [skip] TTS 未配置：本轮只跑「文本 → LLM」，不出声、不驱动口型"
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

# 0.3.11：启动前**不再**清理同级的 melo/serve.py 与 engine/ 进程。
#
# 理由（执行单 §1/§4）：缺省二进制不拥有这些进程，用户可能自己拉起 Melo；
# 点火脚本没有资格杀它。替换本服务的那条 pkill（上面 live2d-ai-desktop --web）
# 保留。0.3.9 起 local-tts 已移出主仓，它的进程生命周期归第二仓自己管。

echo
echo "==> 启动 web 服务（音频单源=browser：由浏览器出声并驱动口型）"
echo "==> 就绪判据：GET / = 200（启动器页；/ 仍是 302 的旧构建回落 GET /app/ = 200），"
echo "    就绪前**不给**页面地址、不开浏览器；不再等待本机 TTS。服务在后台，本脚本前台等待。"

./target/debug/live2d-ai-desktop --web --http-port "${PORT}" &
SVC_PID=$!

# INT/TERM：进程还活着就把它杀掉（已在退出的进程不再补一刀）。
cleanup() {
  if kill -0 "$SVC_PID" 2>/dev/null; then
    kill "$SVC_PID" 2>/dev/null || true
  fi
}
trap cleanup INT TERM

# ---- 5a) 等启动器页就绪：GET / = 200（最长 90 秒）。旧构建（302 → /app/）回落。
# 0.3.11：不再有「等本机 TTS 就绪」那一段（执行单 §4）。进程先死就停。
ROOT_MODE=""
page_ready() {
  local deadline=$((SECONDS + 90)) code app_code
  while :; do
    code=$(http_code "http://127.0.0.1:$PORT/" 4)
    if [ "$code" = "200" ]; then
      # 启动器页已就绪；顺手确认 /app/ 产物也在（点「进入」不该吃 503）。
      app_code=$(http_code "http://127.0.0.1:$PORT/app/" 10)
      if [ "$app_code" = "200" ]; then
        ROOT_MODE="launcher"
        return 0
      fi
      echo "[error] 启动器页已 200，但 GET /app/ 不是 200（实得 $app_code）——Flutter 产物缺失？"
      exit 1
    fi
    if [ "$code" = "302" ]; then
      # 启动器落地前的旧构建：/ 仍是 302 → /app/。中间 rc 兼容（执行单 §4），不删。
      app_code=$(http_code "http://127.0.0.1:$PORT/app/" 10)
      if [ "$app_code" = "200" ]; then
        ROOT_MODE="legacy-app"
        return 0
      fi
    fi
    if ! kill -0 "$SVC_PID" 2>/dev/null; then
      echo "[error] 服务进程已退出（页面从未就绪，GET / 一直是 $code）——不打开浏览器"
      exit 1
    fi
    if [ $SECONDS -ge "$deadline" ]; then
      echo "[error] 90 秒内页面未就绪（GET / 一直是 $code）——不打开浏览器；日志见上方服务输出"
      exit 1
    fi
    sleep 1
  done
}
echo "==> 等待启动器页就绪（GET http://127.0.0.1:$PORT/ = 200，最长 90 秒）…"
page_ready
if [ "$ROOT_MODE" = "launcher" ]; then
  echo "    [ok] 启动器页就绪：GET / = 200"
else
  echo "    [ok] 旧构建页面就绪：GET /app/ = 200（/ 仍 302；启动器落地后这里变成 GET / = 200）"
fi

# ---- 5c) 全部就绪：横幅 + 默认浏览器。--------------------------------------
ENTRY_URL="http://127.0.0.1:$PORT/"
APP_URL="$ENTRY_URL"app/
echo
echo "=========================================================="
echo "  点火入口： $ENTRY_URL"
echo "  Flutter ： $APP_URL"
echo "  渲染面  ： $ENTRY_URL"render
echo "  Mod 状态： $ENTRY_URL"api/v1/mods
echo "  自测清单： docs/verification/ignition-core-loop.md"
echo "=========================================================="
echo

if [ "${LIVE2D_AI_NO_BROWSER:-0}" = "1" ]; then
  echo "==> LIVE2D_AI_NO_BROWSER=1：不自动打开浏览器（地址在上面）"
elif [ -r /proc/version ] && grep -qi microsoft /proc/version && command -v powershell.exe >/dev/null 2>&1; then
  # WSL：本脚本（在 WSL 里）没有默认浏览器，交给 Windows 侧的 Start-Process。
  echo "==> 用 Windows 默认浏览器打开 $ENTRY_URL"
  powershell.exe -NoProfile -Command "Start-Process '$ENTRY_URL'" >/dev/null 2>&1 &
else
  if command -v xdg-open >/dev/null 2>&1; then
    echo "==> 用默认浏览器打开 $ENTRY_URL"
    xdg-open "$ENTRY_URL" >/dev/null 2>&1 &
  else
    echo "[warn] 找不到可用的打开方式（WSL 无 powershell.exe，本机无 xdg-open），请手动打开：$ENTRY_URL"
  fi
fi

# ---- 5d) 脚本不退出：前台等服务，服务停了才停。------------------------------
wait "$SVC_PID"
exit $?
