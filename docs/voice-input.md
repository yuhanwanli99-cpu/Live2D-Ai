# 语音转写注入契约 `POST /api/v1/voice/transcript`（0.2.0-rc.3 / Wave 3 轨 A）

> 把**一段音频**的转写说给角色听，走与聊天框**完全相同**的主链：
> `clean_transcript → locale 归一化 → say → LLM → TTS → 口型 → Live2D`。
> ASR 本体**不在主仓**——它在用户机器上的独立 sidecar 进程
> （示例：[`docs/examples/voice-sidecar/`](examples/voice-sidecar/README.md)）。

**钉死的选择**：新增专用 loopback 端点（`docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`
§3A 选项 B），**不是**复用 `/api/v1/external/chat`。理由见 §8。

**Wave 3 轨 A 的变更**（`0.2.0-rc.3` 内，**不 bump 版本**）：`backend` / `locale`
从「配置占位」变成**可测行为**——handler 读 Mod config 走明确分支、locale 真影响
文本归一化、成功响应回显 `backend` / `locale`；sidecar 侧补齐 6 类失败的
逐条退避（§4.4）。

**产品级加强波次的变更**（同样 `0.2.0-rc.3` 内，**不 bump 版本**）：前端 Mod 面板
把三个字段说成人话并显示**当前生效值**（§1.3）；新增 Mod 一次性命令 `selftest`
（面板「检查配置」按钮，§5.1）；sidecar 退出码做成「码 → 一句话人话 → 处置」表（§4.5）。

---

## 0. 边界：ASR 不在主仓，在 sidecar

| 位置 | 做什么 | 交付物 |
|---|---|---|
| **主仓 Rust**（本仓库） | 本端点：loopback 校验、token、启停门禁、`clean_transcript` + locale 归一化、`supervisor.say` | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` + `live2d-ai-mod-voice-input` |
| **sidecar**（示例目录，非主仓） | 拿音频 → 调 ASR CLI → 清洗 → POST 本端点 | `docs/examples/voice-sidecar/`（Python，**仅标准库**） |
| **主链路** | 与人设 / TTS / 口型共用一条链 | `supervisor` / `live2d-ai-runtime` |

**红线**（`PLAN-voice-input.md` §Forbidden + `AGENTS.md`）：

- **不**把任何云 / 本地 ASR SDK 静态链进主 binary；
- **不**在 Rust 侧开 socket（`VoiceBackend::opens_network()` 恒 `false`）、
  **不**在 Mod 里读进程环境；
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
| 请求体 | `{"text":"...", "token"?: "..."}`（token 可选，见 §3） |
| 成功响应 | `{"ok":true,"text":"<清洗+归一化后文本>","backend":"mock|sidecar","locale":"<生效 locale>"}` |
| 落点 | `supervisor.say(<清洗+归一化后文本>)`（与 `/api/v1/external/chat` 同一个 `say` 入口） |
| `backend` | 来自 Mod config（缺省 `mock`）；Rust **不开 socket**，两分支都只做本地说 `say` |
| `locale` | 来自 Mod config（缺省 `zh-CN`）；**只**影响 text 归一化（§6.1） |

**前置路由**：本端点与 `/api/v1/external/chat` 一样是 `run_request_loop` 里的
**静态前置路由**（`wasm_assets` / `flutter_app` 之后，`dispatch_with_security` 之前），
因此它**自己**完成 Origin + Content-Type 校验，不走通用 mutating 校验。

### 1.1 `backend` 分支（Wave 3 起不再是死配置）

handler 从 Mod config 读 `backend` 并走**明确分支**；两个分支的**本地落点相同**
（清洗 + 归一化 → `say`），差别只在输入语义，且成功响应回显 `backend`
（**可观察、可回归**）：

| `backend` | 谁喂文本 | Rust 侧动作 | 网络 |
|---|---|---|---|
| `mock`（缺省） | 集成方 / 测试 | `RustRoute::LocalInject`：清洗 + 归一化 → `say` | **无** |
| `sidecar` | 外部 ASR 进程**推**到本端点 | `RustRoute::AcceptPush`：同一落点 | **无** |

**「Rust 不碰网络」是结构性的**，不是口头约定：`VoiceBackend::opens_network()`
恒 `false`，`RustRoute` 枚举里**没有网络变体**。回归：
- Mod 侧 `no_backend_opens_network`（两个 backend 都断言）；
- handler 侧 `backend_branches_never_wait_on_network`（黑洞 LLM 端点下仍立即 `200`）。

sidecar 侧（发送方）会遇到的失败与退避见 §4.4；完整演示见 sidecar README §3。

### 1.2 `locale`（Wave 3 起真影响归一化）

handler 从 Mod config 读 `locale`（缺省 `zh-CN`）并传给
`prepare_transcript`。它**只**影响 text 归一化档（§6.1）：

- **不影响 ASR 引擎选型**——引擎在 sidecar，由用户的 `--transcriber` 决定；
- **不随请求发给 sidecar**——sidecar 是推模式，只 POST 文本；
- 成功响应回显生效的 `locale`。

### 1.3 说人话：这三个字段到底是什么

这一段与前端 Mod 面板上的说明**逐字同源**——用户不读源码也能答上来：

- **`backend = mock`**：**没有 ASR**。由集成方 / 测试**直接喂**转写文本，
  屏幕上不会凭空出现你说的话。
- **`backend = sidecar`**：**外部 ASR 进程**（在用户机器上）自己识别，再把文本
  **推**到 `POST /api/v1/voice/transcript`。**Rust 侧从不开 socket、也不会主动
  请求 sidecar**——这是推模式，不是拉模式。
- **`locale`**：只影响转写文本的**归一化**（CJK 词间空格 / 中英边界，§6.1），
  **不是「识别语言开关」**。识别语言由 sidecar 的 `--transcriber` 决定；
  `locale` 也不会随请求发给 sidecar。

写错时的行为是**宽容**的（不会让链路死掉），但自检会点名（§5.1）：
未知 `backend` 值 → 按 `mock` 处理；非法 `locale`（空值除外）→ 归一化落到「拉丁」档。

---

## 2. 请求体

```jsonc
{
  "text": "转写文本（必填，字符串；清洗 + 归一化后不得为空，最多 2000 字符）",
  "token": "可选；与生效 token 匹配时放行（服务端配了 token 才需要）"
}
```

**校验顺序**（决定你看到哪个码）：

1. `text` 缺失 / 非字符串 → `400 invalid_payload`；
2. `token` 存在但非字符串 → `400 invalid_payload`；
3. token 已配置但请求缺失/不匹配 → `401 unauthorized`；
4. **清洗后为空** → `400 empty_transcript`（空白 / 零宽字符 / 控制字符）；
5. **清洗 + 归一化后** > 2000 字符 → `400 text_too_long`。

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
{ "ok": true, "text": "你好世界", "backend": "mock", "locale": "zh-CN" }
```

HTTP `200`。`text` 是**清洗 + locale 归一化后**的文本——发送方据此确认服务端
真的处理过（规则见 §6）。`backend` / `locale` 是 Wave 3 新增的**可观察字段**，
用来确认「分支真的走了 / 哪一档归一化生效」。

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
| 400 | `text_too_long` | **清洗 + 归一化后** > 2000 字符 |
| 401 | `unauthorized` | token 已配置但请求缺失 / 不匹配 |
| 403 | `mod_disabled` | `voice-input` Mod 已注册但**停用**（见 §5） |
| 403 | `origin_denied` / `origin_required` | Origin 非 loopback 同源，或缺 Origin 且 `allow_no_origin=false` |
| 405 | `method_not_allowed` | 非 POST |
| 415 | `unsupported_media_type` | Content-Type 非 `application/json` |
| 503 | `supervisor_unavailable` | supervisor 未就绪 |
| 200 | `busy`（`ok:false`） | 主链忙碌，本条已丢弃（§4.2） |

### 4.4 sidecar 失败分类与逐条退避（`401 / 403 / busy / empty / timeout / transport`）

**发送方视角**的失败只有 6 类：前 4 类是**服务端回包**（带 `error.code`），
后 2 类是**发送方本地**（没有 `error.code`）。sidecar 的 `RETRY_POLICY`
是代码侧真相，`--selftest` 逐条断言，README §5.1 是同一张表。

| 失败（`code`） | 触发 | sidecar 退出码 | 策略 | 处置 |
|---|---|---|---|---|
| `unauthorized` | 401 | 4 | `fix` | 对齐 token 与服务端后再发；盲目重试只会继续 401 |
| `mod_disabled` | 403 | 4 | `fix` | 先启用 `voice-input` Mod 再发 |
| `busy` | 200 + `ok:false` | 5 | `backoff` | 等 2–5 s 再发（本条已丢弃） |
| `empty_transcript` | 400 | 4 | `drop` | 换一段音频 / 重说；重发同样为空 |
| `timeout` | 本机等待 > `--timeout` | 4 | `backoff` | 等 1–2 s 再发；连续超时先看后端日志 |
| `transport` | 连接被拒 / 不可达 | 4 | `backoff` | 先确认服务已点火，再退避 1–2 s 重试 |

`timeout` 与 `transport` 归**同一个退出码 4**，但打印不同分类
（`report_send_failure` 判定 `TimeoutError` / `socket.timeout`）。

### 4.5 sidecar 退出码表（码 → 一句话人话 → 处置）

这是 **sidecar 脚本**（`docs/examples/voice-sidecar/voice_sidecar.py`）的进程退出码，
与服务端 `error.code` 是**两层**：退出码是「脚本怎么结束的」，`error.code` 是
「服务端为什么拒绝」。代码侧真源是脚本里的 `EXIT_*` 常量；`--selftest` 逐条断言，
sidecar README §5 是同一张表。

| 退出码 | 一句话人话 | 触发 | 处置 |
|---|---|---|---|
| `0` | 成功 | HTTP 2xx 且响应 `ok:true` | 无——转写已注入主链 |
| `2` | 参数或依赖错 | 缺 `--audio`、音频文件不存在 / 读不到、`--transcriber` 写法非法、`cmd:` 里的可执行文件不存在 | 先修命令 / 路径 / 装好 ASR CLI 再重跑；**本地**问题，重试无用 |
| `3` | 转写失败 | 同名 `.txt` 读失败、`cmd:` 非 0 退出或超时、转写清洗后为空 | 换一段音频 / 重说 / 检查 ASR 命令；重发同样为空 |
| `4` | 推送失败 | HTTP 非 2xx（`400` / `401` / `403` / `405` / `415` / `503`）**或**请求发不出去（连接被拒 = 服务没点火；超时） | 按 §4.3 的 `error.code` 逐条处置；`transport` / `timeout` 退避 1–2 s 重试 |
| `5` | 服务端 `ok:false` | 目前只有 `busy`：主链忙，本条已丢弃 | 等 2–5 s 再发（立即重试只会继续 `busy`） |

> **`0` 之外的码没有「重试就好」这回事**：`2` 是本地错、`3` 是这段音频没内容、
> `5` 是主链忙。只有 `4` 里的 `transport` / `timeout` 与 `5` 值得退避重试，
> 其余**先修再发**（与 §4.4 的 `fix` / `backoff` / `drop` 策略一一对应）。

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

### 5.1 配置自检：Mod 一次性命令 `selftest`（面板「检查配置」）

前端 Mod 面板的「检查配置」按钮走基座的一次性命令通道：

```http
POST /api/v1/mods/voice-input/command
Content-Type: application/json

{"command":"selftest"}
```

成功 `200 {"ok":true,"result":{…}}`；未知命令 `409 unsupported_command`；
Mod 未启用 / worker 正忙 `503 command_unavailable`（**可重试**）。`result` 由
`live2d_ai_mod_voice_input::config_selftest` 产生，**只回结论、不回 token 明文**：

| key | 语义 |
|---|---|
| `ok` | `problems` 为空（`backend` 与 `locale` 都自洽） |
| `backend` / `backend_valid` / `backend_defaulted` | 生效后端 / 是否已知 / 是否走了缺省 |
| `locale` / `locale_valid` / `locale_defaulted` / `locale_profile` | 生效 locale / 是否合法 BCP-47 / 是否缺省 / 归一化档（`cjk` / `latin`） |
| `token_set` | 是否配了 token（**只回布尔**，绝不回明文 / 长度） |
| `route` / `opens_network` | 本地路由（`local_inject` / `accept_push`）/ Rust 是否开 socket（恒 `false`） |
| `problems` | 硬问题：`backend` 配错、`locale` 非法 |
| `notes` | 提醒：字段走了缺省、`backend=sidecar` 未设 token |

它把「宽容回落」造成的**名实不符**（界面写着 `sidecar`、实际按 `mock` 跑）变成一次
可点的自检；回归见 Mod crate 的 `selftest_*` / `command_*`。

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

### 6.1 locale 归一化（真影响，不是死配置）

清洗之后**再加一步** `normalize_for_locale(text, locale)`（handler 与 Mod 注入
共用入口 `prepare_transcript`）。`locale` 只影响**这一档策略**：

| 档 | 语言标签 | 规则 | 例 |
|---|---|---|---|
| `Cjk` | `zh*` / `ja*` / `ko*`（缺省 `zh-CN`） | 删掉**两个 CJK 字符之间**的空格（ASR 分词伪影） | `"你好 世界"` → `"你好世界"` |
| `Latin` | 其余（含 `en-US`） | 保留空格；在 CJK 与 ASCII 词字符直接相邻处补一个空格 | `"打开空调wifi"` → `"打开空调 wifi"` |

- **不受影响的**：ASCII 词之间的空格（`"打开 wifi 开关"` 原样）；标点不动。
- **幂等**：对已是本档结果的文本再跑一次不变（回归 `locale_normalization_is_idempotent`）。
- **只影响 text 归一化，不影响 ASR 引擎选型**：引擎由 sidecar 的 `--transcriber`
  决定（`docs/examples/voice-sidecar/README.md` §8）；`locale` **不**随请求发出去。
- 空 / 未知语言 → `Latin` 档（`locale_from_config` 只把**空**回落成 `zh-CN`）。

回归（Mod crate）：`locale_profile_classifies_language_families` /
`locale_cjk_removes_inter_cjk_spaces` /
`locale_latin_keeps_spaces_and_spaces_mixed_boundaries` /
`prepare_transcript_cleans_then_normalizes`；
handler：`success_returns_200_with_cleaned_text`（缺省 zh-CN）、
`sidecar_backend_branch_uses_config_locale`（`en-US` 档）。

---

## 7. curl 示例

端口以 `ignite.sh` 默认 **18080** 为例；无 Origin 的 curl 需服务端开了
`allow_no_origin`（开发 / 工具客户端场景）。执行前先启用 Mod：

```bash
# 必须带 loopback Origin；不带 Origin 的纯 curl 需服务端开
# LIVE2D_AI_ALLOW_NO_ORIGIN=1（见 docs/external-input.md §1）
curl -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/enable \
  -H 'Origin: http://127.0.0.1:18080'
```

```bash
# 7.1 无 token（服务端未配 token）
curl -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
  -H "Content-Type: application/json" \
  -d '{"text":"把窗户关小一点"}'

# 7.2 清洗 + 归一化有效（响应的 text 是处理后的结果）
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

# 7.7 切 backend / locale（Mod config；启用中会立即 restart 生效）
curl -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/config \
  -H "Content-Type: application/json" \
  -d '{"config":{"backend":"sidecar","locale":"en-US"}}'
```

完整 sidecar（音频 → 转写 → 本端点）见
[`docs/examples/voice-sidecar/`](examples/voice-sidecar/README.md)：

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py \
  --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run
```

---

## 8. 职责边界：何时用 `/voice/transcript`、何时用 `/external/chat`

两条端点**落点完全相同**（`supervisor.say` → 同一条主链）。**按输入来源选**，
不要按「哪个方便」选：

| 你的输入是… | 用哪条 | 为什么 |
|---|---|---|
| 一段**音频**的转写（用户说话 / 录音文件 / ASR CLI 输出） | `/api/v1/voice/transcript` | 需要 `clean_transcript` + locale 归一化；「有音频没听清」有独立的 `empty_transcript` |
| 弹幕 / 礼物 / webhook / 任意**外部文本事件** | `/api/v1/external/chat` | 需要 Mod 的 `prefix` / `text_template` 渲染；缺省启用、直播刚需 |
| 你自己在键盘上打的一句话 | 前端聊天框（WS） | 一次输入 = 一次会话，不需要外部注入口 |
| 想给「转写」加前缀 / 模板 | `/api/v1/external/chat` | voice 端点**刻意没有** `text_template` / `prefix`（语音不做模板渲染；要改文案请改人设） |

**一页对照**（voice-input vs external-input，逐维度）：

| 维度 | `/api/v1/external/chat`（external-input） | `/api/v1/voice/transcript`（voice-input） |
|---|---|---|
| 入口端点 | `POST /api/v1/external/chat` | `POST /api/v1/voice/transcript` |
| 文本来源 | 弹幕 / 礼物 / webhook / 任意外部文本事件（外部进程 POST） | 一段音频的 ASR 转写（sidecar 识别后**推**到端点；Rust 不开 socket） |
| 清洗语义 | **无清洗**；只做 `prefix` / `text_template` 渲染（要什么文案自己给） | `clean_transcript`（零宽 / 控制符 / 空白折叠）+ locale 归一化；空白转写 → `400 empty_transcript` |
| token 语义 | `EXTERNAL_INPUT_TOKEN`（env 优先）→ Mod config `token` → 不鉴权；请求侧 body `token` 或 `Authorization: Bearer`；**永不回显** | 同左，只是 env 名换成 `VOICE_INPUT_TOKEN`、回落 voice-input Mod config `token` |
| locale | 无（不涉及文本归一化） | Mod config `locale`（缺省 `zh-CN`）；**只**影响归一化档（CJK 词间空格 / 中英边界，§6.1），不选 ASR 引擎、不发给 sidecar |
| 典型场景 | 直播间弹幕 / 礼物上屏（缺省启用，装好就能用） | 语音对讲 / 录音转写（缺省停用，等用户装好 ASR） |
| 失败码 | `invalid_payload` / `text_too_long` / `unauthorized` / `mod_disabled` / `origin_denied` / `origin_required` / `405` / `415` / `503` / `busy`（200 `ok:false`） | 同左 + **`empty_transcript`**；sidecar 进程退出码另见 §4.5 |
| 上游 Mod / 启停 | `external-input`，**缺省启用**（直播刚需） | `voice-input`，**缺省停用**（用户没装 ASR 前启用只会 403） |
| Mod config | `token` / `prefix` / `text_template` / `listen_port` | `token` / `locale` / `backend` |
| 成功响应 | `{"ok":true,"endpoint":"external.chat",…}` | `{"ok":true,"text":"…","backend":"…","locale":"…"}` |

**为什么选专用端点（已钉死，Wave 2 计划 §3A 选项 B）**：

1. **语义不同**：语音侧有自己的 `token` / `locale` / `backend`；把它们塞进弹幕端点
   会让两种输入共用一份配置，「哪个开关管哪边」无法回答。
2. **要在主链路上证明清洗真的跑了**：复用 external 等于让 `voice-input` crate
   继续当摆设（`clean_transcript` 只被自家单测调用）。专用端点把
   「转写 → 清洗 → 归一化 → say」变成可演示、可 curl、可回归的一条链路；响应回显
   处理后的文本 + `backend` / `locale`，肉眼即可验收。
3. **错误面不同**：语音特有「有音频但没听清」= `empty_transcript`，
   与「请求结构错」分开。
4. **启停独立**：`voice-input` 缺省停用、`external-input` 缺省启用——
   两条端点各有各的门禁，互不牵连。

> 计划里把这个二选一列为「集成 PR 的决定」；Wave 2 A 轨就是它，
> 所以 `docs/examples/voice-sidecar/README.md` 的「本分支未实现」占位已整体重写。

> **交叉引用写在这里**（`docs/external-input.md` 由 external-input 轨道维护，
> 本轨不修改它）：`/api/v1/external/chat` 的完整契约——token 优先级、
> `prefix` / `text_template` 渲染语义、6 条 curl、错误表——**以
> `docs/external-input.md` 为权威**；本页 §8 只做上面这张一页对照。

---

## 9. 安全说明

- **loopback-only**：server 绑定 `127.0.0.1`，仅本机可达。**切勿**把端口转发到 WAN。
- **token 可选**：无 token 时任何**本机**进程都能注入（含滥发）。本机还有浏览器扩展 /
  消息机器人等可被劫持的服务时，建议设 token。
- **密钥不出环**：token 不写日志 / 不写 WS / 不导出 / 不回显；只在请求层校验。
- **Origin 校验**：跨域请求默认拒绝（不回 `Access-Control-Allow-Origin`）。
- **方法 + Content-Type**：仅 `POST` + `application/json`，防表单 CSRF。
- **长度上限**：清洗 + 归一化后 2000 字符（错误文本里回显的是**实际字符数**，不含内容）。
- **不开 socket**：`backend=sidecar` 是**推模式**——Rust 从不主动连 sidecar / ASR，
  因此没有「SSRF 到内网 ASR 端口」这条面。

---

## 10. 实现与测试索引

| 层 | 位置 |
|---|---|
| HTTP handler（安全 + 门禁 + token + backend/locale + 清洗归一化 + say） | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` |
| handler 回归（**29 条**） | `crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs` |
| Mod（静态 `settings_spec` / `clean_transcript` / `normalize_for_locale` / `prepare_transcript` / `inject_transcript` / `RustRoute`） | `crates/live2d-ai-mod-voice-input/src/lib.rs` + `src/normalize.rs` |
| Mod 配置自检（纯函数，`command("selftest")` 的真源） | `crates/live2d-ai-mod-voice-input/src/selftest.rs` |
| Mod 回归（**29 条**） | `crates/live2d-ai-mod-voice-input/src/tests.rs` |
| 产品面板（人话解释 / 当前生效值 / 检查配置 / 出错怎么办） | `shell/flutter/lib/settings/mods/voice_input_panel.dart` |
| 面板回归（**7 条**） | `shell/flutter/test/voice_input_panel_test.dart` |
| sidecar 示例 + `--selftest`（70 项）+ 退避表 | `docs/examples/voice-sidecar/` |
| 接线清单 | `docs/plans/parallel-mods/REGISTER-voice-sidecar-v1.md` |

回归覆盖面（`voice_routes_tests.rs`，29 条）：路径不命中 → `None`、405（多种方法）、
415（错 CT / 缺 CT）、403（坏 Origin / 缺 Origin）、403 `mod_disabled`、
门禁三态（停用 403 / 启用过门禁 / 不在册不设门禁）、`invalid_payload` 四种形态、
`empty_transcript` 与 `invalid_payload` 区分、`text_too_long` + 2000 边界、
长度按清洗后判定、401（无 token / 错 token / Bearer 大小写 / body token）、
`effective_token` 优先级纯函数、`check_token` / `bearer_token` 纯函数、
200 成功回显处理后文本 + `backend` / `locale`（缺省 mock/zh-CN）、
**`sidecar` 分支 + config `en-US` 归一化**、**黑洞端点下不等网络**、
200 busy（`ok:false`，确定性构造 in-flight 回合）、空转写不占 `say` 缓冲、
**`backend` 配错回落 mock 在响应里可观察**、**非法 `locale` 回显并按拉丁档归一化**。

Mod crate（`tests.rs`，29 条）另外覆盖：`backend` / `locale` 配置解析与回落、
清洗与两档归一化的纯函数、注入只碰 `say_tx`（`action_tx` 休眠）、
**配置自检 `selftest`**（缺省自洽 / backend 配错点名 / locale 非法点名 /
token 明文绝不出现在结果里 / sidecar 未设 token 只提醒）与命令通道
（`selftest` 被认识、未知命令回 `UnsupportedCommand`）。
