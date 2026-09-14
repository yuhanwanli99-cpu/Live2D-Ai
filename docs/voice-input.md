# 语音转写注入契约 `POST /api/v1/voice/transcript`（0.2.0-rc.3 / Wave 2 A 轨）

> 把**一段音频**的转写说给角色听，走与聊天框**完全相同**的主链：
> `clean_transcript → say → LLM → TTS → 口型 → Live2D`。
> ASR 本体**不在主仓**——它在用户机器上的独立 sidecar 进程
> （示例：[`docs/examples/voice-sidecar/`](examples/voice-sidecar/README.md)）。

**钉死的选择**：新增专用 loopback 端点（`docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`
§3A 选项 B），**不是**复用 `/api/v1/external/chat`。理由见 §8。

---

## 0. 边界：ASR 不在主仓，在 sidecar

| 位置 | 做什么 | 交付物 |
|---|---|---|
| **主仓 Rust**（本仓库） | 本端点：loopback 校验、token、启停门禁、`clean_transcript`、`supervisor.say` | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` + `live2d-ai-mod-voice-input` |
| **sidecar**（示例目录，非主仓） | 拿音频 → 调 ASR CLI → 清洗 → POST 本端点 | `docs/examples/voice-sidecar/`（Python，**仅标准库**） |
| **主链路** | 与人设 / TTS / 口型共用一条链 | `supervisor` / `live2d-ai-runtime` |

**红线**（`PLAN-voice-input.md` §Forbidden + `AGENTS.md`）：

- **不**把任何云 / 本地 ASR SDK 静态链进主 binary；
- **不**在 Rust 侧开 socket、**不**在 Mod 里读进程环境；
- **不**把 B 站协议之类的外部协议塞进 Rust；
- 清洗**只有一份实现**（`live2d_ai_mod_voice_input::clean_transcript`），
  handler **不得**内联等价逻辑——「专用端点证明 Mod 真的在主链路上」正是本端点存在的理由之一。

---

## 1. 端点概览

| 项目 | 值 |
|---|---|
| 方法 | `POST`（其它方法 `405`） |
| 路径 | `/api/v1/voice/transcript` |
| Content-Type | `application/json`（必填，前缀匹配；否则 `415`） |
| 监听 | 仅 `127.0.0.1`（loopback-only，**不对外暴露**） |
| 端口 | `live2d-ai.toml` 的 `[web].port`；`./scripts/ignite.sh` 默认 **18080**（`--port N` 可改） |
| Origin | 同源 loopback（`http://127.0.0.1:<port>` / `http://localhost:<port>`）；无 Origin 的 curl / 脚本走 `allow_no_origin`（`LIVE2D_AI_ALLOW_NO_ORIGIN=1` 或 `--allow-no-origin`） |
| 响应 | `application/json; charset=utf-8` |
| 落点 | `supervisor.say(<清洗后文本>)`（与 `/api/v1/external/chat` 同一个 `say` 入口） |

**前置路由**：本端点与 `/api/v1/external/chat` 一样是 `run_request_loop` 里的
**静态前置路由**（`wasm_assets` / `flutter_app` 之后，`dispatch_with_security` 之前），
因此它**自己**完成 Origin + Content-Type 校验，不走通用 mutating 校验。

---

## 2. 请求体

```jsonc
{
  "text": "转写文本（必填，字符串；清洗后不得为空，清洗后最多 2000 字符）",
  "token": "可选；与生效 token 匹配时放行（服务端配了 token 才需要）"
}
```

**校验顺序**（决定你看到哪个码）：

1. `text` 缺失 / 非字符串 → `400 invalid_payload`；
2. `token` 存在但非字符串 → `400 invalid_payload`；
3. token 已配置但请求缺失/不匹配 → `401 unauthorized`；
4. **清洗后为空** → `400 empty_transcript`（空白 / 零宽字符 / 控制字符）；
5. **清洗后** > 2000 字符 → `400 text_too_long`。

> `invalid_payload` 与 `empty_transcript` 是**两件事**：前者是请求结构错（改脚本），
> 后者是「有音频但没听清 / 全是静音」（重说）。分开报，发送方才知道下一步做什么。

---

## 3. 鉴权（token，可选）

**来源优先级**（`voice_routes::effective_token`，纯函数，有回归）：

1. 环境变量 `VOICE_INPUT_TOKEN`（**优先**）——部署期密钥，不落配置文件；
2. 回落到 `voice-input` Mod config 的 `token`（前端 Mod 设置里的 secret 字段）；
3. 两者都空 / 纯空白 → **不鉴权**（仅 loopback，向后兼容）。

**请求侧二选一**（匹配值即可）：

1. 请求体 JSON 里的 `token`；或
2. 请求头 `Authorization: Bearer <token>`（前缀大小写不敏感）。

**错误码**：token 已配置但缺失 / 不匹配 → `401 unauthorized`。

**token 永不回显**：任何响应体 / 错误信息 / 日志都不含 token 明文
（回归 `config_token_missing_401` 直接断言 401 响应体里搜不到密钥）。
sidecar 的 `--dry-run` 同样把 token 打码成 `***`。

---

## 4. 响应与错误码表

### 4.1 成功

```json
{ "ok": true, "text": "把窗户关小一点" }
```

HTTP `200`。`text` 是**清洗后**的文本——发送方据此确认服务端真的洗过了
（清洗语义见 §6）。

### 4.2 主链忙碌

```json
{ "ok": false, "error": { "code": "busy",
  "message": "supervisor 忙碌（pending 缓冲已满），本条转写已被丢弃" } }
```

HTTP **`200`**（刻意，**不用** 5xx）：请求格式没问题，是主链当前忙，
本条已丢弃；发送方**退避重试**即可。用 5xx 会诱导脚本 / 反向代理按
「服务故障」重试到刷屏。

### 4.3 错误码总表

| 状态 | `error.code` | 条件 |
|---|---|---|
| 400 | `invalid_payload` | JSON 解析失败 / `text` 缺失、非字符串 / `token` 非字符串 |
| 400 | `empty_transcript` | **清洗后**为空（纯空白 / 零宽字符 / 控制字符） |
| 400 | `text_too_long` | **清洗后** > 2000 字符 |
| 401 | `unauthorized` | token 已配置但请求缺失 / 不匹配 |
| 403 | `mod_disabled` | `voice-input` Mod 已注册但**停用**（见 §5） |
| 403 | `origin_denied` / `origin_required` | Origin 非 loopback 同源，或缺 Origin 且 `allow_no_origin=false` |
| 405 | `method_not_allowed` | 非 POST |
| 415 | `unsupported_media_type` | Content-Type 非 `application/json` |
| 503 | `supervisor_unavailable` | supervisor 未就绪 |
| 200 | `busy`（`ok:false`） | 主链忙碌，本条已丢弃（§4.2） |

sidecar 侧把这些映射成进程退出码（`0/2/3/4/5`）：
[`docs/examples/voice-sidecar/README.md` §5](examples/voice-sidecar/README.md)。

---

## 5. 启停语义（唯一真源 = Mod 的 `enabled`）

`voice-input` 是一个标准 Mod，**它的 manifest 开关就是本端点的启用开关**：

- **启用**：`mods.json` 里 `{"mods":{"voice-input":{"enabled":true}}}`，
  或前端「Mod 管理」打开，或 `POST /api/v1/mods/voice-input/enable`。
- **停用**（**缺省**）：此后本端点返回 `403 mod_disabled`——**明确告知发送方**，
  不静默吞掉转写。
- settings 表单里**没有**第二个 `enabled` 字段（启停唯一真源 = manifest）。
- 注册表里**没有**该 Mod（自定义装配 / 单测上下文）→ **不设门禁**：
  端点是 web_api 的核心 `say` 能力，不因一个可选 Mod 缺失而失效
  （与 `external_routes` 同口径）。

> 为什么缺省停用（而 `external-input` 缺省启用）：直播弹幕是「装好就能用」，
> 语音还需要用户自己装 ASR —— 没装之前启用它只会让每一次注入都 403。

---

## 6. 清洗（复用 `live2d_ai_mod_voice_input::clean_transcript`）

清洗是 **Mod crate 的纯函数**，handler 直接调用（**不重写**）：

| 规则 | 例 |
|---|---|
| 连续空白（含全角空格 `U+3000`、`\t`、`\n`、`\r`）折叠成一个半角空格 | `"你好\n\t世界"` → `"你好 世界"` |
| 去掉零宽字符 `U+200B` / `U+FEFF` / `U+200C` / `U+200D` 与其它控制字符（Unicode `Cc`） | `"\uFEFF你\u200B好\u0007"` → `"你好"` |
| 去掉首尾空白 | `"  你好  "` → `"你好"` |
| 结果为空 → `None` → `400 empty_transcript` | `"   "` / `"\u200B"` |

**空转写绝不进 `say`**：那会造出一次空输入回合，与「空句不进 TTS」是同一条纪律
（`AGENTS.md`「语音输出约定」）。回归 `empty_transcript_never_reaches_say`
用「紧接着一条正常转写仍能被接受」证明前一条没有占用 `say` 缓冲。

---

## 7. curl 示例

端口以 `ignite.sh` 默认 **18080** 为例；无 Origin 的 curl 需服务端开了
`allow_no_origin`（开发 / 工具客户端场景）。执行前先启用 Mod：

```bash
curl -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/enable
```

```bash
# 7.1 无 token（服务端未配 token）
curl -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"把窗户关小一点"}'

# 7.2 清洗有效：全角空格 + 零宽字符（响应的 text 是清洗后结果）
curl -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"  把\u3000窗户\u200b关小一点  "}'

# 7.3 token 放 body（服务端配了 token）
curl -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"帮我记一下","token":"your-secret-token"}'

# 7.4 token 放 Authorization 头（推荐，body 更干净）
curl -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-token" \
  -d '{"text":"帮我记一下"}'

# 7.5 空转写 → 400 empty_transcript（不是 invalid_payload）
curl -i -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"   "}'

# 7.6 Mod 停用 → 403 mod_disabled；supervisor 未就绪 → 503 supervisor_unavailable
curl -i -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"hi"}'
```

完整 sidecar（音频 → 转写 → 本端点）见
[`docs/examples/voice-sidecar/`](examples/voice-sidecar/README.md)：

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py \
  --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run
```

---

## 8. 与 `/api/v1/external/chat` 的差异 / 为什么选专用端点

两条端点**落点完全相同**（`supervisor.say` → 同一条主链），差别全在**输入侧**：

| 维度 | `/api/v1/external/chat` | `/api/v1/voice/transcript` |
|---|---|---|
| 语义 | 任意外部事件（弹幕 / 礼物 / webhook） | **语音转写**（一段音频 → 一句话） |
| 上游 Mod | `external-input` | `voice-input` |
| 清洗 | 无（只有 `prefix` / `text_template` 渲染） | **`clean_transcript`**（零宽 / 控制符 / 空白折叠） |
| 错误面 | `invalid_payload` / `text_too_long` | 多一个 **`empty_transcript`** |
| token env | `EXTERNAL_INPUT_TOKEN` | `VOICE_INPUT_TOKEN` |
| Mod config | `token` / `prefix` / `text_template` / `listen_port` | `token` / `locale` / `backend` |
| 成功响应 | `{"ok":true,"endpoint":"external.chat"}` | `{"ok":true,"text":"<清洗后文本>"}` |
| 缺省启用 | **是**（直播刚需） | **否**（等用户装好 ASR） |

**为什么选专用端点（已钉死，Wave 2 计划 §3A 选项 B）**：

1. **语义不同**：语音侧有自己的 `token` / `locale` / `backend`；把它们塞进弹幕端点
   会让两种输入共用一份配置，「哪个开关管哪边」无法回答。
2. **要在主链路上证明清洗真的跑了**：复用 external 等于让 `voice-input` crate
   继续当摆设（`clean_transcript` 只被自家单测调用）。专用端点把
   「转写 → 清洗 → say」变成可演示、可 curl、可回归的一条链路；响应回显
   清洗后文本，肉眼即可验收。
3. **错误面不同**：语音特有「有音频但没听清」= `empty_transcript`，
   与「请求结构错」分开。
4. **启停独立**：`voice-input` 缺省停用、`external-input` 缺省启用——
   两条端点各有各的门禁，互不牵连。

> 计划里把这个二选一列为「集成 PR 的决定」；本轮（Wave 2 A 轨）就是它，
> 所以 `docs/examples/voice-sidecar/README.md` 的「本分支未实现」占位已整体重写。

---

## 9. 安全说明

- **loopback-only**：server 绑定 `127.0.0.1`，仅本机可达。**切勿**把端口转发到 WAN。
- **token 可选**：无 token 时任何**本机**进程都能注入（含滥发）。本机还有浏览器扩展 /
  消息机器人等可被劫持的服务时，建议设 token。
- **密钥不出环**：token 不写日志 / 不写 WS / 不导出 / 不回显；只在请求层校验。
- **Origin 校验**：跨域请求默认拒绝（不回 `Access-Control-Allow-Origin`）。
- **方法 + Content-Type**：仅 `POST` + `application/json`，防表单 CSRF。
- **长度上限**：清洗后 2000 字符（错误文本里回显的是**实际字符数**，不含内容）。

---

## 10. 实现与测试索引

| 层 | 位置 |
|---|---|
| HTTP handler（安全 + 门禁 + token + 清洗 + say） | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` |
| handler 回归（25 条） | `crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs` |
| Mod（静态 `settings_spec` / `clean_transcript` / `inject_transcript`） | `crates/live2d-ai-mod-voice-input/src/lib.rs` |
| sidecar 示例 + `--selftest` | `docs/examples/voice-sidecar/` |
| 接线清单 | `docs/plans/parallel-mods/REGISTER-voice-sidecar-v1.md` |

回归覆盖面（`voice_routes_tests.rs`，25 条）：路径不命中 → `None`、405（多种方法）、
415（错 CT / 缺 CT）、403（坏 Origin / 缺 Origin）、403 `mod_disabled`、
门禁三态（停用 403 / 启用过门禁 / 不在册不设门禁）、`invalid_payload` 四种形态、
`empty_transcript` 与 `invalid_payload` 区分、`text_too_long` + 2000 边界、
长度按清洗后判定、401（无 token / 错 token / Bearer 大小写 / body token）、
`effective_token` 优先级纯函数、`check_token` / `bearer_token` 纯函数、
200 成功回显清洗后文本（空白折叠 + 零宽移除）、200 busy（`ok:false`，确定性构造
in-flight 回合）、空转写不占 `say` 缓冲。
