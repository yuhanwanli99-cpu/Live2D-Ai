# 外部事件注入契约 `POST /api/v1/external/chat`（0.2.0-rc.1）

> 让**主 UI 之外**的事件（直播弹幕 / 礼物、本机脚本、消息回调）说给角色听，
> 走与聊天框**完全相同**的主链路：`say → LLM → TTS → 口型 → Live2D`。
> 这是**唯一**对外暴露的文本注入端点；B 站协议抓取**不在主仓**（见 §0）。

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
  "token": "可选；与生效 token 匹配时放行（env 已设才需要）"
}
```

**校验**：
- `text` 缺失 / 非字符串 / 空 → `400 invalid_payload`；
- `token` 存在但非字符串 → `400 invalid_payload`；
- 模板渲染后长度 > 2000 → `400 text_too_long`。

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

- 依赖 `blivedm`；只处理 `DANMU_MSG` 与 `SEND_GIFT`，
  其它 cmd 只 log（`_on_unknown_cmd`）。
- 环境变量：`BILI_ROOM_ID`（**真实房间号**，短号需先转换）、
  `BILI_SESSDATA`（登录态 cookie，**账号凭据，勿入库**）、
  `LIVE2D_AI_URL`、`EXTERNAL_INPUT_TOKEN`、`SIDECAR_DRY_RUN=1`（干跑）。
- 清洗：去换行/控制符、压空白、截断；再 POST 本端点。
- **SEND_GIFT_V2 灰度**：B 站正灰度新的礼物结构；blivedm 未支持时会落到
  `_on_unknown_cmd`，**只 log 不注入**。后续等 blivedm 上游支持再补回调；
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
| HTTP handler（安全 + 门禁 + 模板 + token） | `crates/live2d-ai-desktop/src/web_api/external_routes.rs` |
| Mod（静态 settings_spec / 模板纯函数 / say） | `crates/live2d-ai-mod-external-input/src/lib.rs` |
| 缺口断言 | `mod_count_is_three`（工厂表）、`external_routes::tests`（门禁 / token / Bearer / 模板膨胀） |
