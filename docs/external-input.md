# 外部事件注入契约 `POST /api/v1/external/chat`（0.2.0-rc.1）

> 让**主 UI 之外**的事件（直播弹幕 / 礼物、本机脚本、消息回调）说给角色听，
> 走与聊天框**完全相同**的主链路：`say → LLM → TTS → 口型 → Live2D`。
> 这是**唯一**对外暴露的文本注入端点；B 站协议抓取**不在主仓**（见 §0）。
> 壳内的「测试注入」（不经过 HTTP、不需要 token）见 §5.3。

---

## 0. 边界：B 站抓取不在主仓，在 Win sidecar

**主仓（Rust）只做两件事**：提供本端点 + 把文本塞进主链路。
B 站协议、blivedm、WSS 长连接、鉴权 cookie、开放平台 SDK **全部不进 Rust**，
它们住在一个**独立的 Windows sidecar**（Python）里：

| 位置 | 做什么 | 交付物 |
|---|---|---|
| **主仓 Rust**（本仓库） | `POST /api/v1/external/chat`：loopback 校验、可选 token、启停门禁、模板渲染、`supervisor.say` | `crates/live2d-ai-desktop/src/web_api/external_routes.rs` + `live2d-ai-mod-external-input` |
| **Win sidecar**（示例目录，非主仓） | 连 B 站、收 `DANMU_MSG` / `SEND_GIFT`、清洗成一行纯文本、POST 本端点 | `docs/examples/bilibili-sidecar/`（Python + blivedm） |
| **主链路** | 与人设 / TTS / 口型共用一条链 | `supervisor` / `live2d-ai-runtime` |

> 为什么不把协议编进 Rust：B 站接口/风控变化快，且 blivedm 是 Python 生态；
> 把一个易变的外部协议编译进本地优先的桌面核心，会让「主链路可编译」依赖于
> 一个随时会坏的第三方接口。sidecar 挂了只影响弹幕注入，**不影响主链路**。

**定位：sidecar 是增强，不是唯一主路径。** 不装 sidecar 时这个能力照样可用——
本机脚本、消息回调、产品面板的「测试注入」（§5.3）都能把文本送进同一条主链；
sidecar 只负责把 B 站弹幕/礼物翻译成一行纯文本。两边互不依赖：sidecar 挂了，
其它入口与主链一行都不受影响。

---

## 1. 端点概览

| 项目 | 值 |
|---|---|
| 方法 | `POST`（其它方法 `405`） |
| 路径 | `/api/v1/external/chat` |
| Content-Type | `application/json`（必填，前缀匹配；否则 `415`） |
| 监听 | 仅 `127.0.0.1`（loopback-only，**不对外暴露**） |
| 端口 | `live2d-ai.toml` 的 `[web].port`；`./scripts/ignite.sh` 默认 **18080**（`--port N` 可改） |
| Origin | 同源 loopback（`http://127.0.0.1:<port>` / `http://localhost:<port>`）；无 Origin 的 curl/脚本走 `allow_no_origin`（`LIVE2D_AI_ALLOW_NO_ORIGIN=1` 或少进 `--allow-no-origin`） |
| 响应 | `application/json; charset=utf-8` |

---

## 2. 鉴权（token，可选）

**来源优先级**：环境变量 `EXTERNAL_INPUT_TOKEN` **优先**；未设/为空时回落到
Mod config 的 `token`（前端 Mod 设置里的 secret 字段）；**两者都空 = 不鉴权**。

**请求侧二选一**（匹配值即可）：
1. 请求体 JSON 里的 `token` 字段；或
2. 请求头 `Authorization: Bearer <token>`。

**错误码**：token 已配置但请求缺失/不匹配 → `401 unauthorized`。

> 为什么 env 优先：部署期密钥不该落进 `mods.json` / 配置目录；
> Mod 设置里的 `token` 是给「懒得配环境变量」的本机场景的回落。
> 两种方式 handler 都不写日志、不落盘。

---

## 3. 请求体

```jsonc
{
  "text": "要注入的文本，必填，trim() 后非空，渲染后最多 2000 字符。",
  "token": "可选；与生效 token 匹配时放行（env 已设才需要）",
  "v2_ignored": 0
}
```

**校验**：
- `text` 缺失 / 非字符串 / 空 → `400 invalid_payload`；
- `token` 存在但非字符串 → `400 invalid_payload`；
- `v2_ignored` 存在但非非负整数 → `400 invalid_payload`；
- 模板渲染后长度 > 2000 → `400 text_too_long`。

> `v2_ignored` 是 sidecar 的可选自报字段：它统计「本地 blivedm 没认出来的
> `SEND_GIFT_V2` 条数」。服务端**覆盖写**进可观察计数（见 §5.1），不参与注入。
> 不报 = 保持上一次的值；报 0 = 归零。

---

## 4. 响应

### 4.1 成功（已注入）

```json
{ "ok": true, "endpoint": "external.chat" }
```

HTTP `200`。

### 4.2 忙碌（supervisor pending 缓冲已满）

```json
{ "ok": false, "endpoint": "external.chat",
  "error": { "code": "busy", "message": "supervisor 忙碌（pending 缓冲已满），本条文本已被丢弃" } }
```

HTTP `200`：请求格式没问题，但**本条文本已被丢弃**——发送方应自行退避重试。
sidecar 收到 `ok:false` 时打印一行告警即可，不必退出。

### 4.3 错误

| 状态 | `error.code` | 条件 |
|---|---|---|
| 400 | `invalid_payload` | JSON 解析失败 / `text` 缺失、非字符串、为空 |
| 400 | `text_too_long` | 渲染后 > 2000 字符 |
| 401 | `unauthorized` | token 已配置但请求缺失/不匹配 |
| 403 | `mod_disabled` | `external-input` Mod 已注册但**停用**（见 §5） |
| 403 | `origin_denied` / `origin_required` | Origin 非 loopback，或缺 Origin 且 `allow_no_origin=false` |
| 405 | `method_not_allowed` | 非 POST |
| 415 | `unsupported_media_type` | Content-Type 非 `application/json` |
| 503 | `supervisor_unavailable` | supervisor 未就绪 |

---

## 5. 启停语义（唯一真源 = Mod 的 `enabled`）

`external-input` 是一个标准 Mod，**它的 manifest 开关就是本端点的启用开关**：

- **启用**：`mods.json` 里 `{"mods":{"external-input":{"enabled":true}}}`，
  或前端「Mod 管理」打开，或 `POST /api/v1/mods/external-input/enable`。
  **0.2.0-rc.1 起缺省启用**（`mods.json` 不存在时的内建 manifest）。
- **停用**：`POST /api/v1/mods/external-input/disable`（或改 `mods.json` 后重启）。
  此后本端点返回 `403 mod_disabled` —— **明确告知发送方**，不静默吞掉文本。
- settings 表单里**没有**第二个 `enabled` 字段：避免「Mod 开关」与「配置开关」两处真相。
- 注册表里没有该 Mod（自定义装配 / 单测上下文）→ **不设门禁**：端点是 web_api 的核心 say
  能力，不因一个可选 Mod 缺失而失效。生产装配始终包含它。

---

## 5.1 可观察计数（`GET /api/v1/mods/external-input/state`）

「今天到底注入成功几条、被挡了几条、忙丢了几条」不该靠翻日志猜。Wave 3 起，
handler 在分支里记账，读者从 Mod 运行态读：

```bash
curl -s http://127.0.0.1:18080/api/v1/mods/external-input/state
# {"id":"external-input","enabled":true,"state":{
#    "accepts":12,"rejects":1,"busy":3,"v2_ignored":0,"ready":true}}
```

| `state` 键 | 含义 | 对应 handler 分支 |
|---|---|---|
| `accepts` | 文本已进主链路（`supervisor.say` 返回 `true`） | 200 `ok:true` |
| `busy` | supervisor 忙（pending 缓冲满），文本被丢弃 | 200 `ok:false` / code `busy` |
| `rejects` | **策略拒绝**：token 鉴权失败（401）或 Mod 停用（403 `mod_disabled`） | 401 / 403 |
| `v2_ignored` | sidecar 上报的 `SEND_GIFT_V2` 忽略累计值（覆盖写） | body 可选字段，见 §3 |
| `ready` | 该 Mod 运行时是否已 `start`（启用即 `true`） | — |
| `token_set` | 是否已配置令牌（env `EXTERNAL_INPUT_TOKEN` **或** Mod config `token`）；**只报存在性，绝不回显明文** | — |
| `inject_via_command` | 命令通道 `test_inject` 是否可用（恒 `true`，见 §5.3） | — |

> L1 产品级波次只**追加**了上面这一个展示性键：既有键（四个计数 + `ready` +
> `token_set`）一个都没删，老读者不受影响。

三条边界（**与实现逐条一致**，见 `counters.rs` 头注）：

- `rejects` **不含** 400（负载非法 / 超长）、405、415 与 503（supervisor 未就绪）：
  前几类是调用方/服务装配问题，不是「一条合法事件被策略挡下」；
- 计数是**进程级** `AtomicU64`（handler 与 runtime 拿不到彼此引用），**不持久化**，
  进程重启归零——它是运行观察值，不是审计账本；
- `v2_ignored` 是**覆盖写**：sidecar 报的是自身累计值，重复上报同一值幂等。

`token_set` 是**布尔存在性**，不是令牌本身：界面据此说「已设置令牌」/
「未设令牌：仅本机可用」。任何响应、日志、WS 帧都不出现令牌明文。

---

## 5.2 重置计数（`POST /api/v1/mods/external-input/command`）

「重置计数」是一次性动作（改运行态、**不改配置**），走产品级加强波次新增的通用
命令通道（与 `enable`/`disable`/`config` 同一个 `POST /api/v1/mods/{id}/{action}` 路由）：

```bash
curl -s -X POST http://127.0.0.1:18080/api/v1/mods/external-input/command \
  -H "Content-Type: application/json" \
  -d '{"command":"reset_counters"}'
# {"ok":true,"result":{"reset":true,"before":{"accepts":12,"rejects":1,"busy":3,"v2_ignored":0}}}
```

- `before` 是**清零前**的快照，面板据此说「清零前：…」；之后四个计数全为 0。
- 错误：`400 bad_request`（body 缺非空 `command`）/ `404 not_found`（Mod 不在注册表）/
  `409 unsupported_command`（该 Mod 不认识这条命令）/ `409 command_failed`（执行失败）/
  `503 command_unavailable`（未启用或 Mod worker 正忙，**可重试**）。
- 计数是进程级、不持久化：进程重启本来就归零，`reset_counters` 只是把这件事做成
  按钮（前端「Mod 管理」→ external-input 面板的「重置计数」，二次确认后才发）。
- 与 §7 同类：mutating 请求需 `Content-Type: application/json` + loopback Origin；
  无 Origin 的 curl 仍需服务端 `allow_no_origin`。

---

## 5.3 壳内测试注入（`command test_inject`）

Mod 面板（外部事件接入）上的「测试注入」按钮走的是 **Mod 命令通道**，不是 HTTP 端点：

```bash
curl -s -X POST http://127.0.0.1:18080/api/v1/mods/external-input/command \
  -H "Content-Type: application/json" \
  -d '{"command":"test_inject","args":{"text":"主播好"}}'
# {"ok":true,"result":{
#    "ok":true,"injected_text":"[弹幕] 主播好","accepted":true,
#    "endpoint":"mod.command.test_inject",
#    "note":"与 POST /api/v1/external/chat 共用同一渲染口径与同一 say_tx；区别是本命令走 Mod 命令通道、不需要 token"}}
```

`args` 契约：

| 字段 | 必填 | 语义 |
|---|---|---|
| `text` | 是 | 待注入文本；`trim()` 后非空，否则命令失败（文案「text 必填且非空」） |
| `prefix` | 否（默认 `true`） | `true` = 按 Mod 配置的 `prefix` + `text_template` 渲染（与 HTTP 端点**逐字一致**）；`false` = 文本已是最终形态，**逐字注入**（不再套用配置前缀/模板） |

> `prefix:false` 的用途**单一**：产品面板把「按当前配置本地渲染好的示例」填在输入框里
> （用户看到什么就注入什么），发送时用 `false` 告诉服务端「这段就是最终文本」，
> 避免 `[弹幕] [弹幕] 主播好` 这种双重前缀。它不是第二条渲染逻辑：默认路径仍然
> 逐字走 `render_from_config`（与 HTTP 端点同函数），只有显式 `false` 时才跳过套用。

返回字段：`ok` / `injected_text`（真正进主链的那串字）/ `accepted`（主链是否接受；
忙时为 `false`，本条已被丢弃，可重试）/ `endpoint` / `note`。命令本身只在取参失败
或渲染后超长时失败（`409 command_failed`，文案含**实际长度与上限**）。

**与 HTTP 端点的关系**（这是本命令存在的意义：不复制第二套逻辑）：

- **同一条渲染口径**：两者都走 `render_injected_text` / `render_from_config`，
  长度上限同为**渲染后 2000 字符**；
- **同一个 `say_tx`**：人设、TTS、口型、以及可观察计数（`accepts` / `busy`）
  两条入口记的是**同一本账**；
- **区别**：命令通道只在本机同源 UI 内可达，因此**不需要 token**，也没有 HTTP 的
  Origin / Content-Type 校验；未启用时由 host 直接回 `503 command_unavailable`。

## 5.4 `403 mod_disabled` 与 `503 command_unavailable` 对照

同一次「Mod 被停用」，两条入口给两个码——这不是不一致，而是两层协议的固有差异：

| | HTTP 端点 `POST /api/v1/external/chat` | 命令通道 `POST /api/v1/mods/external-input/command` |
|---|---|---|
| 停用时的码 | `403 mod_disabled` | `503 command_unavailable`（「未启用或正忙」，**可重试**） |
| 谁判的 | handler 自己的启停门禁（读 Mod manifest 的 `enabled`） | 通用命令路由（Mod 没在跑就不派发命令） |
| `rejects` 计数 | **计入**（策略拒绝，见 §5.1） | **不计入**（`rejects` 只统计 HTTP 策略拒绝） |
| 语义 | 策略拒绝：目标关闭 / 无权限 | 能力暂不可用：稍后重试 |
| 面板落点 | 「测试注入」失败文案 + 停用态静态说明 | 同一处（面板同时点明两个码，避免用户只认一个） |

---

## 6. 文本模板 / 前缀（可选）

Mod 配置（`settings_spec` v2）里有两个字符串字段，注入前按顺序渲染：

| 字段 | 语义 |
|---|---|
| `text_template` | `{text}` 是外部文本占位符；空/纯空白 = 原样；含占位符则替换全部；非空但不含 `{text}` 则视为字面前缀、文本追加其后 |
| `prefix` | 拼在模板结果**之前**（例如 `[弹幕] `） |

渲染是纯函数（`live2d_ai_mod_external_input::render_from_config`），长度为
**渲染后**判定。推荐把「弹幕/礼物」这类语境标记放在**服务端**的 `prefix`，
这样换 sidecar、换游戏都不用改脚本。

其余字段：`listen_port` 仅是**提示值**（真实端口来自 `live2d-ai.toml`，
本 Mod 不开 socket）；`token` 见 §2。

---

## 7. curl 示例

端口以 `ignite.sh` 默认 **18080** 为例；无 Origin 的 curl 需服务端开了
`allow_no_origin`（开发 / 工具客户端场景）。

```bash
# 7.1 无 token（服务端未配 token；Mod 缺省启用）
curl -X POST http://127.0.0.1:18080/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"你好，直播间的第一条弹幕！"}'

# 7.2 env 已设 token：放在 body
curl -X POST http://127.0.0.1:18080/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"收到一条系统提醒","token":"your-secret-token"}'

# 7.3 env 已设 token：放在 Authorization 头（推荐，body 更干净）
curl -X POST http://127.0.0.1:18080/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-token" \
  -d '{"text":"webhook 转发的消息"}'

# 7.4 令牌缺失 → 401
curl -i -X POST http://127.0.0.1:18080/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"hi"}'

# 7.5 Mod 被停用 → 403 mod_disabled
curl -i -X POST http://127.0.0.1:18080/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"hi"}'
```

Python（webhook 转发）最小示例：

```python
import json, urllib.request

def send_to_live2d(text, token=None):
    body = {"text": text}
    if token:
        body["token"] = token
    req = urllib.request.Request(
        "http://127.0.0.1:18080/api/v1/external/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req) as resp:
        return resp.status, resp.read().decode()

print(send_to_live2d("收到新邮件提醒"))
```

---

## 8. B 站弹幕 / 礼物（Win sidecar 示例）

完整示例：`docs/examples/bilibili-sidecar/`（`bilibili_sidecar.py` +
`requirements.txt` + README）。要点：

- 依赖 `blivedm`；只处理 `DANMU_MSG` 与礼物（`SEND_GIFT` /
  `SEND_GIFT_V2`），其它 cmd 不注入（由 blivedm 自己 log 一次）。
- 环境变量：`BILI_ROOM_ID`（**真实房间号**，短号需先转换）、
  `BILI_SESSDATA`（登录态 cookie，**账号凭据，勿入库**）、
  `LIVE2D_AI_URL`、`EXTERNAL_INPUT_TOKEN`、`SIDECAR_DRY_RUN=1`（干跑）、
  `SIDECAR_MIN_INTERVAL_MS`（节流缺省值，命令行 `--min-interval-ms` 覆盖）。
- 清洗：去换行/控制符、压空白、截断；再 POST 本端点。
- **节流（可开关）**：`--min-interval-ms N`，缺省 `1000`（每秒最多一条），
  `0` = 关闭；窗口内的弹幕**只打印丢弃日志、不注入**。命令行覆盖
  `SIDECAR_MIN_INTERVAL_MS`。
- **离线自检**：`python bilibili_sidecar.py --selftest`（**16 项断言**：清洗 / 节流 /
  dry-run / 上报体 / SEND_GIFT_V2 兜底判定），**不需要** blivedm、aiohttp、网络或
  Live2D-Ai 进程也能跑。
- **SEND_GIFT_V2（已查证上游）**：blivedm 官方仓库在 PR #86（2026-08-11，v1.1.7）
  已支持 `SEND_GIFT_V2`——它把 protobuf 展开后**复用同一 `_on_gift` 回调**，
  所以本 sidecar 一个回调同时覆盖经典与 v2，无需再写第二个。若本地仍是旧版
  blivedm（或 PyPI 上停在 0.1.1 的旧包），`SEND_GIFT_V2` 会落到
  兜底检测（`_is_unknown_gift_v2`，见 `bilibili_sidecar.py`）：**只 log 不注入**，
  并累加 `v2_ignored`，随每次注入上报给服务端（见 §3 / §5.1）——旧版也不至于
  悄悄丢礼物。**注意**：blivedm 的 `BaseHandler.handle` 遇到未知 cmd 只自己 log
  一次就 return，并不调用 `_on_unknown_cmd`；所以 sidecar 用 `handle` 覆盖 +
  cmd 表判定，而不是依赖那个回调。
  **不要**在 Rust 主仓手写 B 站协议解析。

---

## 9. 安全说明

- **loopback-only**：server 绑定 `127.0.0.1`，仅本机可达。**切勿**把端口转发到 WAN。
- **token 可选**：无 token 时任何**本机**进程都能注入（含滥发）。建议本地用途设 token，
  尤其当机器上还有浏览器扩展 / 消息机器人等可被劫持的本地服务时。
- **密钥不出环**：token 不写日志 / 不写 WS / 不导出；只在请求层校验。
- **Origin 校验**：跨域请求默认拒绝（不回 `Access-Control-Allow-Origin`）。
- **方法 + Content-Type**：仅 `POST` + `application/json`，防表单 CSRF。

---

## 10. 与旧 Python 版文档的关系

`docs/architecture/external-text-input.md` 描述的是**已归档的 Python
（open-llm-vtuber）实现**（`/api/external/chat`、`wait_reply`、
`X-Live2DAI-Token`…），**不是**本仓当前契约。当前契约以本文件为准。

---

## 11. 实现与测试索引

| 层 | 位置 |
|---|---|
| HTTP handler（安全 + 门禁 + 模板 + token + 计数） | `crates/live2d-ai-desktop/src/web_api/external_routes.rs` |
| Mod（静态 settings_spec / 模板纯函数 / say / state_json / `command`） | `crates/live2d-ai-mod-external-input/src/lib.rs` |
| `test_inject`（取参 / 长度口径 / 与端点同渲染） | `crates/live2d-ai-mod-external-input/src/inject.rs` |
| 命令通道（`reset_counters` / `test_inject` → 409/503 分类） | `crates/live2d-ai-desktop/src/web_api/mods_routes.rs` |
| 产品面板（测试注入 / 计数摘要 / 令牌两态 / 模板示例 / 重置） | `shell/flutter/lib/settings/mods/external_input_panel.dart` |
| 可观察计数（AtomicU64 + 语义表） | `crates/live2d-ai-mod-external-input/src/counters.rs` |
| sidecar（清洗 / 节流 / 自检 / v2 上报） | `docs/examples/bilibili-sidecar/bilibili_sidecar.py` |
| 缺口断言 | `mod_count_is_five`（工厂表）、`external_routes::tests`（门禁 / token / Bearer / 模板膨胀 / **计数 3+1+1** / v2_ignored）、`counters::tests`（计数契约） |
