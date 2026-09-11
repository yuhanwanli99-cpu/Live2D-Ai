# external-input Mod 使用文档

> 节点 E5-T3。外部文本注入端点 `POST /api/v1/external/chat` → `supervisor.say` → LLM/TTS 主链路。

## 1. 端点概览

| 项目 | 值 |
|---|---|
| 方法 | `POST` |
| 路径 | `/api/v1/external/chat` |
| Content-Type | `application/json`（必填） |
| Origin | 同源 loopback（`http://127.0.0.1:<port>` / `http://localhost:<port>`）；缺省时走工具客户端模式 |
| 监听 | 仅 `127.0.0.1`（loopback-only，**不对外暴露**） |
| 响应 | `application/json; charset=utf-8` |

### 鉴权（v1）

鉴权 token 来源于**环境变量** `EXTERNAL_INPUT_TOKEN`，而非 Mod settings 的 `token` 字段。

- **env 未设** → 不鉴权，任何 loopback 客户端均可直接调用（向后兼容）。
- **env 已设** → 要求请求满足以下任一条件（二选一即可）：
  1. 请求体 JSON 中 `token` 字段值与 env 相等；**或**
  2. `Authorization: Bearer <token>` 与 env 相等。

> 说明：env-token 设计为 v1 简化——handler 不访问 `ModRegistry`/`Mod config`，保持与
> `ServerContext` 解耦。Mod settings 中的 `token`（secret）仅用于前端渲染，
> host 在启动 server 前应把设置值写入环境变量（E6 再接入回环）。

### 为何用 env 变量而非 settings？

```
┌──────────────┐  env EXTERNAL_INPUT_TOKEN  ┌──────────────────┐
│ 外部调用方   │ ────────────────────────▶ │ live2d-ai-desktop │─▶ supervisor.say
│ (loopback)   │                           │  web_api handler    │
└──────────────┘                           └────────┬───────────┘
                                                      │
                                         Mod settings "token"（secret）
                                               (前端渲染用)
```

`ServerContext` 没有 `mod_registry` 槽位回环（v1），handler 只能访问
`ServerContext.security` 与 `supervisor_slot`，因此鉴权 token 从环境变量读取。

## 2. 请求体

```jsonc
{
  "text": "要注入的文本，最多 2000 字符，不能为空。",
  "token": "可选，与 env EXTERNAL_INPUT_TOKEN 匹配时放行"  // 仅 env 设了才需
}
```

### 校验规则

- `text`：**必填**，字符串，`trim()` 后非空，≤2000 字符。
- `token`：**可选**，字符串（仅鉴权用）。
- JSON 解析失败 / `text` 非字符串 / 空 / 超长 → `400 invalid_payload` 或 `400 text_too_long`。

## 3. 响应规范

### 成功

```jsonc
{
  "ok": true,
  "endpoint": "external.chat"
}
```

HTTP 状态：`200 OK`。

### 鉴权失败

```jsonc
{
  "error": {
    "code": "unauthorized",
    "message": "token 缺失或不匹配（env EXTERNAL_INPUT_TOKEN 已设）"
  }
}
```

HTTP 状态：`401 Unauthorized`。

### 其它错误

| 状态 | `error.code` | 条件 |
|---|---|---|
| 400 | `invalid_payload` | JSON 解析失败 / `text` 缺失 / 非字符串 / 为空 |
| 400 | `text_too_long` | `text` 超过 2000 字符 |
| 403 | `origin_denied` / `origin_required` | Origin 非 loopback，或缺 Origin 且 `allow_no_origin=false` |
| 405 | `method_not_allowed` | 非 POST |
| 415 | `unsupported_media_type` | Content-Type 非 `application/json` |
| 503 | `supervisor_unavailable` | supervisor 未就绪 |
| 200 |（`ok:false`） | `error.code: "busy"` — supervisor pending 缓冲已满 |

> 响应体统一为 `{"ok":...}`（成功）或 `{"error":{"code","message"}}`（失败），
> `kind`/`type` 字段不添加，保持 v1 简洁。

## 4. curl 示例

### 4.1 无 token（env 未设，向后兼容）

```bash
curl -X POST http://127.0.0.1:18099/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"你好，喝水提醒！"}'
```

### 4.2 env 已设，body `token` 匹配

```bash
export EXTERNAL_INPUT_TOKEN="your-secret-token"

curl -X POST http://127.0.0.1:18099/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"定时提醒：现在喝水","token":"your-secret-token"}'
```

### 4.3 env 已设，`Authorization: Bearer` 匹配

```bash
curl -X POST http://127.0.0.1:18099/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-token" \
  -d '{"text":"webhook 转发的消息"}'
```

### 4.4 env 已设，令牌缺失 → 401

```bash
curl -X POST http://127.0.0.1:18099/api/v1/external/chat \
  -H "Content-Type: application/json" \
  -d '{"text":"hi"}'
# → 401 {"error":{"code":"unauthorized",...}}
```

## 5. Python 示例（webhook 转发）

```python
import json, urllib.request

def send_to_live2d(text, token=None):
    body = {"text": text}
    if token:
        body["token"] = token
    req = urllib.request.Request(
        "http://127.0.0.1:18099/api/v1/external/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req) as resp:
        return resp.status, resp.read().decode()

print(send_to_live2d("收到新邮件提醒"))
```

## 6. 安全说明

- **loopback-only**：server 绑定 `127.0.0.1`，仅本地进程可达。**切勿**在防火墙/SOHO 路由转发该端口到 WAN。
- **token 可选**：无 token 时，任何本地进程均可注入文本（含滥发）。建议**本地用途设 token**，
  尤其当其他本地服务（浏览器扩展、消息机器人）可能被攻击者劫持时。
- **密钥不出环**：token 不写入日志/WS/导出；通过 `X-API-Key`/body/`Authorization`
  在请求层面校验，不持久化于响应头。
- **Origin 校验**：同源 loopback 校验由 `web_api::security` 复用，跨域请求默认拒绝
  （不回 `Access-Control-Allow-Origin`）。
- **方法+Content-Type**：仅 `POST` + `application/json`，防御表单 CSRF。

## 7. 启用配合（Mod 管理 → external-input）

在前端「Mod 管理」面板启用 **external-input** Mod 后：

1. **填写 `token`（secret）**：用于前端渲染；host 启动前需把该值写入环境变量。
   ```bash
   # 典例：由 xtask/desktop 启动脚本读取 settings → 写入 env → 再 exec server
   export EXTERNAL_INPUT_TOKEN="与设置抽屉一致的值"
   ```
2. **监听端口**：设置抽屉的 `listen_port` 由 Mod schema 声明，当前 v1 仍由
   `web_api` 配置（`AppSettings.web.port`，默认 `18099`）决定；E6 再对接 Mod config。
3. **启用 `enabled`**：v1 不增加 Mod-enable gate，say 通道始终可达；
   E6 在 handler 加入 `mod_registry` enabled 校验。

### 7.1 与 cron 定时提醒配合

```bash
# 每 2 小时提醒喝水
echo '0 */2 * * * curl -X POST http://127.0.0.1:18099/api/v1/external/chat -H "Content-Type: application/json" -d "{\"text\":\"⏰ 喝水提醒：当前时间 $(date +%H:%M)\",\"token\":\"your-secret-token\"}"' | crontab -
```

### 7.2 与 webhook 转发配合

```bash
# 例如 GitHub webhook → 本地脚本 → Live2D 说 "你有新的 PR"
# webhook-playground 转发到本地端口，脚本调用本端点。
```

## 8. 鉴权纯函数

```rust
/// - env_token 为 None 时始终放行；
/// - env_token 为 Some(t) 时要求 body_token 或 auth_header（Bearer）匹配。
pub fn check_token(
    body_token: Option<&str>,
    auth_header: Option<&str>,
    env_token: Option<&str>,
) -> bool
```

- **来源**：`EXTERNAL_INPUT_TOKEN` 环境变量（`std::env::var`）。
- **优先级**：body `token` > `Authorization: Bearer <token>`。
- **测试**：`external_routes::tests` 对 `check_token`/`bearer_token` 做纯函数
  单测（env 未设 / body 匹配 / body 不匹配 / Bearer 匹配 / Bearer 格式错误），
  无全局 env 副作用。
