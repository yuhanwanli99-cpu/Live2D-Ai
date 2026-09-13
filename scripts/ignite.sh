#!/usr/bin/env bash
# Live2D-Ai 点火脚本（2026-09-10）—— 核心链路闭环：Flutter 前端 + Rust 后端。
#
# 它做四件事：
#   1) 预检：读 live2d-ai.toml，探测你提供的 OpenAI 兼容 LLM/TTS 端点；
#   2) 确保渲染面产物：crates/l2d-wasm-demo/dist（/render 用）；
#   3) 确保前端产物：shell/flutter/build/web（/app/ 用，可选 --build 自动构建）；
#   4) 启动 live2d-ai-desktop --web，并打印点火入口 URL。
#
# 用法：
#   ./scripts/ignite.sh                 # 预检 + 启动（不重新构建）
#   ./scripts/ignite.sh --build         # 先构建 Rust + Flutter Web 再启动
#   ./scripts/ignite.sh --port 18100    # 指定端口（默认 18080）
#   ./scripts/ignite.sh --check [--port N]  # 只对**已启动**的服务做点火体检，不自启
#
# 说明：本地 LLM/TTS 由你自己提供（任何 OpenAI 兼容实现），本脚本只做探测与提示，
#       不会下载模型、不会启动推理进程。两个本地 Mod 也会在后台自动探活。

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

PORT=18080
DO_BUILD=0
DO_CHECK=0
while [ $# -gt 0 ]; do
  case "$1" in
    --build) DO_BUILD=1 ;;
    --check) DO_CHECK=1 ;;
    --port) shift; PORT="${1:-18080}" ;;
    --port=*) PORT="${1#*=}" ;;
    -h|--help) sed -n "2,17p" "$0"; exit 0 ;;
    *) echo "[warn] 未知参数：$1" ;;
  esac
  shift || true
done

# ---- 0) 点火体检（--check）------------------------------------------------
# 只戳**已经在跑**的那个服务，验证三件事：`/` 302 到 `/app/`、`/app/` 200、
# 产物不引用 Google CDN 的 CanvasKit。断言写在脚本里，是因为这三条都只有
# 「真的起过一次服务」才验得到——单元测试覆盖不到托管层与产物内容。
# 不自启服务（否则端口冲突时的失败会伪装成「探针失败」）。
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
  for f in index.html main.dart.js; do
    fcode=$(curl -s -o /dev/null -w "%{http_code}" --max-time 15 "$base/app/$f" 2>/dev/null || echo 000)
    if [ "$fcode" != "200" ]; then
      echo "    [FAIL] GET /app/$f 期望 200，实得 $fcode"
      fail=1
    elif curl -s --max-time 15 "$base/app/$f" 2>/dev/null | grep -q 'gstatic\.com/flutter-canvaskit'; then
      echo "    [FAIL] /app/$f 仍引用 Google CDN 的 CanvasKit —— 断网会白屏"
      echo "           重新构建：cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn"
      fail=1
    else
      echo "    [ok]   /app/$f 不依赖 Google CDN"
    fi
  done

  if [ "$fail" = "0" ]; then
    echo "==> 体检通过（Windows 浏览器打开 ${base}/app/ 做最后一步肉眼看/听）"
  else
    echo "==> 体检失败：见上面的 [FAIL]"
  fi
  exit "$fail"
fi

echo "==> Live2D-Ai 点火检查"

# ---- 1) 读配置里的 LLM/TTS 端点 -------------------------------------------
if ! command -v python3 >/dev/null 2>&1; then
  echo "[error] 需要 python3 读取 live2d-ai.toml"; exit 1
fi
read -r LLM_BASE TTS_BASE <<EOF
$(python3 - <<PY
import pathlib, tomllib
llm = "http://127.0.0.1:11434/v1"
tts = ""
p = pathlib.Path("live2d-ai.toml")
if p.is_file():
    d = tomllib.loads(p.read_text(encoding="utf-8"))
    llm = d.get("llm", {}).get("base_url", llm)
    tts = d.get("tts", {}).get("base_url", "")
print(llm, tts)
PY
)
EOF
echo "    配置 LLM base : $LLM_BASE"
echo "    配置 TTS base : ${TTS_BASE:-（未配置，选型待定）}"

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
if [ -n "$TTS_BASE" ]; then
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

echo
echo "=========================================================="
echo "  点火入口： http://127.0.0.1:${PORT}/app/"
echo "  渲染面  ： http://127.0.0.1:${PORT}/render"
echo "  Mod 状态： http://127.0.0.1:${PORT}/api/v1/mods"
echo "  自测清单： docs/verification/ignition-core-loop.md"
echo "=========================================================="
echo
echo "==> 启动 web 服务（音频单源=browser：由浏览器出声并驱动口型）"
exec ./target/debug/live2d-ai-desktop --web --http-port "${PORT}"
