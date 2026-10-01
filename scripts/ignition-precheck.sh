#!/usr/bin/env bash
# Live2D-Ai 点火预检（stabilize，2026-09-15）
#
# 对**已经在跑**的服务打一遍 loopback HTTP 预检，输出 PASS/FAIL/SKIP 表。
# 它补的是 `ignite.sh --check`（只验静态托管三件事）够不到的那一层：
#   Mod 注册表、external/voice 注入端点门禁、Mod 启停动作、state 404/503 分界、
#   两个 sidecar 的离线自检。
#
# 用法：
#   ./scripts/ignite.sh                 # 终端 A：先把服务点起来（默认 18080）
#   ./scripts/ignition-precheck.sh      # 终端 B：跑本预检
#   ./scripts/ignition-precheck.sh --port 18100
#   ./scripts/ignition-precheck.sh --token <EXTERNAL_INPUT_TOKEN>
#   ./scripts/ignition-precheck.sh --fsm     # 额外跑五个注册 Mod 的 enable→state→disable 矩阵
#
# 退出码：0 = 无 FAIL；1 = 有 FAIL；2 = 参数/环境错。
#
# 前置约定：
#   * **不改产品代码**，只发 HTTP；会对 Mod 开关做「改完还原」。
#   * 无 Origin 的 curl 只有在服务端开了 `LIVE2D_AI_ALLOW_NO_ORIGIN=1`
#     （或 `--allow-no-origin`）时才放行；本脚本默认给 mutating 请求带
#     loopback Origin，因此**不依赖**那个开关。
#   * TTS / GUI / 麦克风 不在本脚本范围（人机项，见
#     docs/plans/IGNITION-CHECKLIST-stabilize.md）。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PORT=18080
TOKEN="${EXTERNAL_INPUT_TOKEN:-}"
RUN_FSM=0
while [ $# -gt 0 ]; do
  case "$1" in
    --port) shift; PORT="${1:-18080}" ;;
    --port=*) PORT="${1#*=}" ;;
    --token) shift; TOKEN="${1:-}" ;;
    --token=*) TOKEN="${1#*=}" ;;
    --fsm) RUN_FSM=1 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "[warn] 未知参数：$1" >&2 ;;
  esac
  shift || true
done

BASE="http://127.0.0.1:${PORT}"
ORIGIN="Origin: http://127.0.0.1:${PORT}"
JSON="Content-Type: application/json"
AUTH="Authorization: Bearer ${TOKEN}"

FAILS=0
PASSES=0
SKIPS=0

# record <PASS|FAIL|SKIP> <项> <期望> <实得>
record() {
  local st="$1" name="$2" exp="$3" got="$4"
  case "$st" in
    PASS) PASSES=$((PASSES+1)) ;;
    FAIL) FAILS=$((FAILS+1)) ;;
    SKIP) SKIPS=$((SKIPS+1)) ;;
  esac
  printf '  [%-4s] %s\n           期望 %s / 实得 %s\n' "$st" "$name" "$exp" "$got"
}

# check <name> <expected> <actual>
check() {
  local name="$1" exp="$2" got="$3"
  if [ "$got" = "$exp" ]; then record PASS "$name" "$exp" "$got"
  else record FAIL "$name" "$exp" "$got"; fi
}

# inject <url> <text>  →  "status|ok|text" / "status|busy|msg" / "status|<code>|msg"
#
# 主链 supervisor 的 pending 缓冲容量为 1：上一轮还没收口时再注入会拿到
# **200 + ok:false (busy)**——这是契约里刻意设计的合法响应（不是 5xx）。
# 这里退避重试，避免把「主链正忙」误判成点火失败。
inject() {
  python3 - "$1" "$2" "${TOKEN:-}" <<'PY'
import json, sys, time, urllib.request, urllib.error
url, text, token = sys.argv[1], sys.argv[2], sys.argv[3]
body = {"text": text}
if token:
    body["token"] = token
data = json.dumps(body).encode()
origin = "http://" + url.split("//", 1)[1].split("/", 1)[0]
for _ in range(20):
    req = urllib.request.Request(url, data=data,
        headers={"Content-Type": "application/json", "Origin": origin}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            code, d = r.status, json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        code = e.code
        try:
            d = json.loads(e.read().decode() or "{}")
        except Exception:
            d = {}
    except Exception as e:
        print("000|error|%s" % e); sys.exit(0)
    if d.get("ok") is True:
        print("%s|ok|%s" % (code, d.get("text", ""))); sys.exit(0)
    err = d.get("error", {}) or {}
    if err.get("code") != "busy":
        print("%s|%s|%s" % (code, err.get("code", "unknown"), str(err.get("message", ""))[:60]))
        sys.exit(0)
    time.sleep(1)
print("200|busy|主链持续忙碌")
PY
}

echo "==> 点火预检：$BASE（需服务已在跑；本脚本不自启）"

code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 4 "$BASE/api/v1/mods" 2>/dev/null || echo 000)
if [ "$code" != "200" ]; then
  echo "    [FAIL] 服务不可达：GET /api/v1/mods -> $code"
  echo "           先在另一个终端跑：./scripts/ignite.sh"
  exit 1
fi
echo "    服务在线。\n"

# ---------------------------------------------------------------- A. 托管层
echo "-- A. 壳 / 静态托管（ignite.sh --check 的子集，便于一张表看全）"
hdr=$(curl -s -o /dev/null -D - --max-time 5 "$BASE/" 2>/dev/null || true)
root_code=$(printf '%s\n' "$hdr" | awk 'NR==1{print $2}')
root_loc=$(printf '%s\n' "$hdr" | awk 'tolower($1)=="location:"{print $2}' | tr -d '\r' | head -1)
check "GET / → 302 /app/" "302" "$root_code"
check "GET / Location" "/app/" "$root_loc"

app_code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$BASE/app/" 2>/dev/null || echo 000)
if [ "$app_code" = "200" ]; then
  record PASS "GET /app/ → 200" "200" "200"
  for f in index.html main.dart.js; do
    fc=$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 "$BASE/app/$f" 2>/dev/null || echo 000)
    check "GET /app/$f" "200" "$fc"
  done
  if curl -s --max-time 15 "$BASE/app/main.dart.js" 2>/dev/null | grep -q 'gstatic\.com/flutter-canvaskit'; then
    record FAIL "main.dart.js 不依赖 Google CDN" "无 gstatic" "发现 gstatic（断网白屏）"
  else
    record PASS "main.dart.js 不依赖 Google CDN" "无 gstatic" "无 gstatic"
  fi
else
  record SKIP "GET /app/ → 200" "200" "$app_code（Flutter 产物缺失？ignite.sh --build）"
fi

render_code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$BASE/render" 2>/dev/null || echo 000)
if [ "$render_code" = "200" ]; then
  record PASS "GET /render → 200（wasm 渲染面）" "200" "200"
else
  record SKIP "GET /render → 200（wasm 渲染面）" "200" "$render_code（dist 缺失：cd crates/l2d-wasm-demo && trunk build）"
fi

# ---------------------------------------------------------------- B. Mod 注册表
echo ""
echo "-- B. Mod 注册表（五个已注册；wallpaper / pet-desktop / local-llm 的 crate 已删除；缺省只启用 external-input）"
mods_json=$(curl -s --max-time 5 "$BASE/api/v1/mods" 2>/dev/null || echo '{}')
ids=$(printf '%s' "$mods_json" | python3 -c '
import sys,json
d=json.load(sys.stdin)
print(",".join(sorted(m["id"] for m in d.get("mods",[]))))' 2>/dev/null)
check "已注册 Mod id 集合" "director,external-input,memory,persona,voice-input" "$ids"
ext_enabled=$(printf '%s' "$mods_json" | python3 -c '
import sys,json
d=json.load(sys.stdin)
print(next((str(m["enabled"]).lower() for m in d.get("mods",[]) if m["id"]=="external-input"), "missing"))' 2>/dev/null)
check "external-input 缺省启用" "true" "$ext_enabled"

# ---------------------------------------------------------------- B2. 会话 id 宿主能力（L1 基座）
echo ""
echo "-- B2. 会话 id 宿主能力（GET|POST /api/v1/chat/session）"
sess_get=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$BASE/api/v1/chat/session" 2>/dev/null || echo 000)
check "GET /api/v1/chat/session → 200" "200" "$sess_get"
sess_post=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/chat/session" \
  -H "$ORIGIN" -H "$JSON" -d '{"session_id":"precheck-session"}' 2>/dev/null || echo 000)
check "POST /api/v1/chat/session → 200" "200" "$sess_post"
# 非法 id 必须按「不带会话」处理（清掉游标），**不是** 400。
sess_bad=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/chat/session" \
  -H "$ORIGIN" -H "$JSON" -d '{"session_id":"../etc/passwd"}' 2>/dev/null || echo 000)
check "非法 session_id → 200（按不带会话处理）" "200" "$sess_bad"
sess_active=$(curl -s --max-time 5 "$BASE/api/v1/chat/session" 2>/dev/null | python3 -c '
import sys,json
try:
    print(json.load(sys.stdin).get("active_session") or "null")
except Exception:
    print("unreadable")' 2>/dev/null)
check "非法 id 后活动会话被清掉" "null" "$sess_active"
# 复原成合法活动会话，供后面的注入路径跟随。
curl -s -o /dev/null -X POST "$BASE/api/v1/chat/session" -H "$ORIGIN" -H "$JSON" \
  -d '{"session_id":"precheck-session"}' 2>/dev/null || true

# ---------------------------------------------------------------- C. external-input 门禁
echo ""
echo "-- C. external/chat 门禁（loopback + CT + token + 启停）"
H=(-H "$ORIGIN" -H "$JSON")
[ -n "$TOKEN" ] && H+=(-H "$AUTH")

# C1 正常注入（busy 合法，inject 内部退避重试）
r1=$(inject "$BASE/api/v1/external/chat" "点火预检：你好")
c1=${r1%%|*}; f1=$(printf '%s' "$r1" | cut -d'|' -f2)
if [ "$c1" = "200" ] && [ "$f1" = "ok" ]; then
  record PASS "POST 正常注入 → 200 ok:true" "200/ok" "$c1/$f1"
elif [ "$f1" = "busy" ]; then
  record SKIP "POST 正常注入 → 200 ok:true" "200/ok" "主链持续 busy（合法，非缺陷）"
else
  record FAIL "POST 正常注入 → 200 ok:true" "200/ok" "$c1/$f1"
fi

# C2 方法
c2=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$BASE/api/v1/external/chat" -H "$ORIGIN" 2>/dev/null || echo 000)
check "GET external/chat → 405" "405" "$c2"

# C3 非 JSON CT
c3=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/external/chat" -H "$ORIGIN" -H 'Content-Type: text/plain' -d 'hi' 2>/dev/null || echo 000)
check "非 JSON CT → 415" "415" "$c3"

# C4 空 text
c4=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/external/chat" -H "$ORIGIN" -H "$JSON" -d '{"text":"   "}' 2>/dev/null || echo 000)
check "空 text → 400 invalid_payload" "400" "$c4"

# C5 超长（渲染后 > 2000）
c5=$(python3 - "$BASE" <<'PY' 2>/dev/null || echo 000
import json,sys,urllib.request,urllib.error
base=sys.argv[1]; body=json.dumps({"text":"x"*2001}).encode()
r=urllib.request.Request(base+"/api/v1/external/chat",data=body,headers={"Content-Type":"application/json","Origin":base},method="POST")
try:
    urllib.request.urlopen(r,timeout=8); print(200)
except urllib.error.HTTPError as e: print(e.code)
PY
)
check "超长 text → 400 text_too_long" "400" "$c5"

# C6 无 Origin（工具模式开=200，关=403 origin_required）
c6=$(curl -s -o /tmp/pre_c6 -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/external/chat" -H "$JSON" -d '{"text":"无 Origin"}' 2>/dev/null || echo 000)
if [ "$c6" = "200" ]; then record PASS "无 Origin（allow_no_origin=开）" "200 或 403" "200"
elif [ "$c6" = "403" ]; then record PASS "无 Origin（allow_no_origin=关）" "200 或 403" "403 origin_required"
else record FAIL "无 Origin" "200 或 403" "$c6"; fi

# C7 缺 token（仅当配置了 token）
if [ -n "$TOKEN" ]; then
  c7=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/external/chat" -H "$ORIGIN" -H "$JSON" -d '{"text":"无 token"}' 2>/dev/null || echo 000)
  check "已配 token 却缺 token → 401" "401" "$c7"
else
  record SKIP "已配 token 却缺 token → 401" "401" "未配置 EXTERNAL_INPUT_TOKEN"
fi

# C8 停用 → 403 mod_disabled → 复启
d8=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/external-input/disable" -H "$ORIGIN" 2>/dev/null || echo 000)
check "POST …/external-input/disable（无 body 无 CT）" "200" "$d8"
c8=$(curl -s -o /tmp/pre_c8 -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/external/chat" -H "$ORIGIN" -H "$JSON" -d '{"text":"应被拒"}' 2>/dev/null || echo 000)
c8code=$(python3 -c 'import json;print(json.load(open("/tmp/pre_c8"))["error"]["code"])' 2>/dev/null || echo '?')
check "停用后注入 → 403 mod_disabled" "403/mod_disabled" "$c8/$c8code"
e8=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/external-input/enable" -H "$ORIGIN" 2>/dev/null || echo 000)
check "POST …/external-input/enable（无 body 无 CT）" "200" "$e8"

# C9 state 计数键
st_keys=$(curl -s --max-time 5 "$BASE/api/v1/mods/external-input/state" 2>/dev/null | python3 -c '
import sys,json
s=json.load(sys.stdin).get("state",{})
need={"accepts","rejects","busy","ready","v2_ignored"}
missing=sorted(need-set(s.keys()))
print("missing:"+",".join(missing) if missing else "ok:"+",".join(sorted(s.keys())))' 2>/dev/null)
case "$st_keys" in
  ok:*) record PASS "external-input state 必含计数键" "accepts,rejects,busy,ready,v2_ignored" "$st_keys" ;;
  *)    record FAIL "external-input state 必含计数键" "accepts,rejects,busy,ready,v2_ignored" "$st_keys" ;;
esac

# ---------------------------------------------------------------- D. state 404 / 503 分界
echo ""
echo "-- D. Mod 动作 & state 404 vs 503"
u=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$BASE/api/v1/mods/does-not-exist/state" 2>/dev/null || echo 000)
check "未知 Mod state → 404 not_found" "404" "$u"
p=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$BASE/api/v1/mods/persona/state" 2>/dev/null || echo 000)
if [ "$p" = "503" ]; then record PASS "persona state → 503（未实现 state_json）" "503" "503"
elif [ "$p" = "200" ]; then record PASS "persona state → 200（已实现 state_json）" "503/200" "200"
else record FAIL "persona state" "503/200" "$p"; fi

# ---------------------------------------------------------------- E. voice-input
echo ""
echo "-- E. voice/transcript 门禁"
v0=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/voice/transcript" -H "$ORIGIN" -H "$JSON" -d '{"text":"语音预检"}' 2>/dev/null || echo 000)
if [ "$v0" = "403" ]; then record PASS "voice-input 缺省停用 → 403 mod_disabled" "403" "403"
else record SKIP "voice-input 缺省停用 → 403 mod_disabled" "403" "$v0（已被启用？）"; fi
ven=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/voice-input/enable" -H "$ORIGIN" 2>/dev/null || echo 000)
check "启用 voice-input" "200" "$ven"

# ---- L1：唤醒短语 = 能力总闸（空 = 关）；另有手动闸 ----
# 读一个错误码（读不到一律回 unreadable，避免把「读不到」当成「对了」）。
verr() {
  python3 -c '
import sys,json
try:
    d=json.load(sys.stdin)
    e=d.get("error") or {}
    print(e.get("code",""))
except Exception:
    print("unreadable")'
}
# 总闸未配（缺省）⇒ 拒绝 transcript 且**可读**（L1 硬验收句）。
vg=$(curl -s --max-time 5 -X POST "$BASE/api/v1/voice/transcript" -H "$ORIGIN" -H "$JSON" \
  -d '{"text":"语音预检"}' 2>/dev/null | verr 2>/dev/null)
check "总闸未配 → 403 voice_gate_closed" "voice_gate_closed" "$vg"

# 配总闸（唤醒短语「小梦」）+ 手动闸开。
vcfg=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/voice-input/config" \
  -H "$ORIGIN" -H "$JSON" \
  -d '{"config":{"backend":"mock","locale":"zh-CN","wake_phrase":"小梦","manual_enabled":true}}' 2>/dev/null || echo 000)
check "写入唤醒短语（总闸开）" "200" "$vcfg"

# 没听见唤醒词 ⇒ 400 wake_phrase_required。
vw=$(curl -s --max-time 5 -X POST "$BASE/api/v1/voice/transcript" -H "$ORIGIN" -H "$JSON" \
  -d '{"text":"天气不错"}' 2>/dev/null | verr 2>/dev/null)
check "缺唤醒词 → 400 wake_phrase_required" "wake_phrase_required" "$vw"

# 命中：短语被**剥掉**，再做清洗 + locale 归一化（全角空格 U+3000 与零宽 U+200B 都被吃掉）。
VOICE_TEXT=$'  小梦\u3000把\u200b窗户关小一点  '
rv=$(inject "$BASE/api/v1/voice/transcript" "$VOICE_TEXT")
v1=${rv%%|*}; vf=$(printf '%s' "$rv" | cut -d'|' -f2); vt=$(printf '%s' "$rv" | cut -d'|' -f3)
if [ "$v1" = "200" ] && [ "$vf" = "ok" ] && [ "$vt" = "把窗户关小一点" ]; then
  record PASS "命中唤醒词 → 200 + 剥短语 + 清洗/归一化" "200/text=把窗户关小一点" "$v1/$vt"
elif [ "$vf" = "busy" ]; then
  record SKIP "命中唤醒词 → 200 + 剥短语 + 清洗/归一化" "200/text=把窗户关小一点" "主链持续 busy（合法，非缺陷）"
else
  record FAIL "命中唤醒词 → 200 + 剥短语 + 清洗/归一化" "200/text=把窗户关小一点" "$v1/$vf/$vt"
fi

# 手动闸关 ⇒ 403 voice_manual_off（先写配置，再注入）。
curl -s -o /dev/null --max-time 5 -X POST "$BASE/api/v1/mods/voice-input/config" -H "$ORIGIN" -H "$JSON" \
  -d '{"config":{"backend":"mock","locale":"zh-CN","wake_phrase":"小梦","manual_enabled":false}}' 2>/dev/null || true
vm=$(curl -s --max-time 5 -X POST "$BASE/api/v1/voice/transcript" -H "$ORIGIN" -H "$JSON" \
  -d '{"text":"小梦 你好"}' 2>/dev/null | verr 2>/dev/null)
check "手动闸关 → 403 voice_manual_off" "voice_manual_off" "$vm"

# 复原：总闸关（wake_phrase 置空）+ 手动闸开，再停用 voice-input。
curl -s -o /dev/null --max-time 5 -X POST "$BASE/api/v1/mods/voice-input/config" -H "$ORIGIN" -H "$JSON" \
  -d '{"config":{"backend":"mock","locale":"zh-CN","wake_phrase":"","manual_enabled":true}}' 2>/dev/null || true
vd=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/voice-input/disable" -H "$ORIGIN" 2>/dev/null || echo 000)
check "复原 voice-input（config 归零 + disable）" "200" "$vd"

# ---------------------------------------------------------------- F. sidecar 自检
echo ""
echo "-- F. sidecar 离线自检"
if [ -f "$ROOT_DIR/docs/examples/voice-sidecar/voice_sidecar.py" ]; then
  if out=$(python3 "$ROOT_DIR/docs/examples/voice-sidecar/voice_sidecar.py" --selftest 2>&1); then
    record PASS "voice_sidecar.py --selftest" "全部通过" "$(printf '%s' "$out" | tail -1 | cut -c1-60)"
  else
    record FAIL "voice_sidecar.py --selftest" "退出 0" "$(printf '%s' "$out" | tail -1 | cut -c1-60)"
  fi
else
  record SKIP "voice_sidecar.py --selftest" "全部通过" "脚本不存在"
fi
if [ -f "$ROOT_DIR/docs/examples/bilibili-sidecar/bilibili_sidecar.py" ]; then
  if out=$(python3 "$ROOT_DIR/docs/examples/bilibili-sidecar/bilibili_sidecar.py" --selftest 2>&1); then
    record PASS "bilibili_sidecar.py --selftest" "OK" "$(printf '%s' "$out" | tail -1 | cut -c1-60)"
  else
    record FAIL "bilibili_sidecar.py --selftest" "退出 0" "$(printf '%s' "$out" | tail -1 | cut -c1-60)"
  fi
else
  record SKIP "bilibili_sidecar.py --selftest" "OK" "脚本不存在"
fi

# ---------------------------------------------------------------- G. 七 Mod FSM（可选）
if [ "$RUN_FSM" = "1" ]; then
  echo ""
  echo "-- G. 五 Mod enable→state→disable（--fsm）"
  for id in external-input persona voice-input memory director; do
    en=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/$id/enable" -H "$ORIGIN" 2>/dev/null || echo 000)
    sc=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$BASE/api/v1/mods/$id/state" 2>/dev/null || echo 000)
    di=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$BASE/api/v1/mods/$id/disable" -H "$ORIGIN" 2>/dev/null || echo 000)
    if [ "$en" = "200" ] && [ "$di" = "200" ] && { [ "$sc" = "200" ] || [ "$sc" = "503" ]; }; then
      record PASS "$id enable/state/disable" "200 / 200|503 / 200" "$en/$sc/$di"
    else
      record FAIL "$id enable/state/disable" "200 / 200|503 / 200" "$en/$sc/$di"
    fi
  done
  # 保证 external-input 恢复缺省启用
  curl -s -o /dev/null -X POST "$BASE/api/v1/mods/external-input/enable" -H "$ORIGIN" 2>/dev/null || true
fi

# ------------------------------------------------- B2b. 带 session_id 的聊天请求（有副作用，放最后）
echo ""
echo "-- B2b. POST /api/v1/chat 带 session_id（L1 会话绑定入口）"
chat_scoped=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 -X POST "$BASE/api/v1/chat" \
  -H "$ORIGIN" -H "$JSON" -d '{"text":"会话预检","session_id":"precheck-session"}' 2>/dev/null || echo 000)
if [ "$chat_scoped" = "200" ] || [ "$chat_scoped" = "429" ]; then
  record PASS "POST /api/v1/chat 带 session_id" "200|429" "$chat_scoped"
else
  record FAIL "POST /api/v1/chat 带 session_id" "200|429" "$chat_scoped"
fi

# ---------------------------------------------------------------- 汇总
echo ""
echo "=========================================================="
printf '  预检结果：PASS %d / FAIL %d / SKIP %d\n' "$PASSES" "$FAILS" "$SKIPS"
if [ "$FAILS" != "0" ]; then
  echo "  ✗ 有 FAIL：见上面 [FAIL] 行。"
else
  echo "  ✓ 无 FAIL。人机项（舞台肉眼 / 出声 / 口型 / Mod 管理点击）见"
  echo "    docs/plans/IGNITION-CHECKLIST-l1.md"
fi
echo "=========================================================="
[ "$FAILS" = "0" ] || exit 1
