# 外部文字接入（External Text Input）— 接口契约【历史 / 已归档 Python 实现】

> ⚠️ **本文描述的是已归档的 Python 端（`Live2D-Ai-pc/open-llm-vtuber`），不是当前契约。**
> 当前 Rust 实现只有**一个**外部注入端点 `POST /api/v1/external/chat`，
> 契约以 **[docs/external-input.md](../external-input.md)** 为准
> （差异很大：没有 `wait_reply` / `client_uid` / `interrupt` / `/api/external/status`，
> 鉴权是 env/mod token 而非 `X-Live2DAI-Token`，端口与路由也不同）。
> 本文保留仅为记录 Python 时代的设计与安全模型；**不要**按本文对接当前后端。
>
> 目标：让**本机/内网的其它程序**把一句话交给角色，走与聊天框输入**完全相同**的内部 LLM 链路。
> 仅 PC 端（`Live2D-Ai-pc/open-llm-vtuber`）。实现：`src/open_llm_vtuber/external_input.py`
> + `WebSocketHandler.inject_text_input()`；前端开关：`renderer/src/external-input.ts`。

---

## 1. 为什么"不是第二条链路"

外部文字**不新建**任何管线，而是复用前端发 `{"type":"text-input"}` 时走的同一条路径：

```
POST /api/external/chat
  → ExternalInputGate.authorize()                   开关 + 来源 + 令牌
  → WebSocketHandler.inject_text_input()            选目标会话 / 忙碌判定 / 回显
  → WebSocketHandler._handle_conversation_trigger()  ← 前端 text-input 的同一入口
  → conversations.handle_conversation_trigger(msg_type="text-input")
  → process_single_conversation()                   LLM 流式 → 情绪 → TTS → 口型/字幕
```

因此：**同一 persona、同一记忆（history_uid）、同一 mood_engine 触发、同一 TTS 与口型**。
不存在"外部消息走简化链路"的差异，也不需要为它单独配置模型。

---

## 2. 安全模型（默认关闭）

后端默认 `host: 0.0.0.0` 且 CORS 全放开，因此本接口自己把门：

| 状态 | 行为 |
| --- | --- |
| 开关关闭（**默认**） | 所有 `/api/external/chat` 请求 → `403`，并计入"已拒绝" |
| 开启 + 回环来源（127.0.0.1 / ::1） | 放行；若设置了令牌仍需校验令牌 |
| 开启 + 非回环来源 + 未勾选"允许局域网" | `403`，提示需在设置页开启 |
| 开启 + 允许局域网 + **无令牌** | 拒绝落成此状态：`POST /api/external/settings` 直接 `400` |
| 开启 + 允许局域网 + 有令牌 | 校验 `X-Live2DAI-Token` / `Authorization: Bearer`，`secrets.compare_digest` 定时安全比较 |

其他边界：
- **开关本身只能从本机修改**（`/api/external/settings`、`/api/external/token` 对非回环来源返回 403），
  避免"远程调用者自己把远程开关打开"的提权路径。
- 单条文本上限 `MAX_TEXT_CHARS = 2000`（超出 `413`）；`wait_reply` 等待上限 `180s`。
- 状态落盘 `Live2D-Ai-pc/open-llm-vtuber/external_input.json`（含令牌，**已 gitignore**）。

---

## 3. 端点

### `GET /api/external/status`
返回开关状态、在线前端会话、计数与最近记录（供设置页展示）。

```json
{
  "ok": true,
  "enabled": false,
  "allow_remote": false,
  "token_set": false,
  "endpoint": "/api/external/chat",
  "clients": ["c1f2..."],
  "accepted": 3,
  "rejected": 1,
  "recent": [{ "at": 1787227890, "source": "127.0.0.1", "text": "你好", "ok": true, "detail": "" }],
  "limits": { "max_text_chars": 2000, "max_wait_reply_s": 180.0 }
}
```

### `POST /api/external/settings` （仅本机）
Body 任意字段可选：`{"enabled":bool, "allow_remote":bool, "token":str, "token_clear":true, "regenerate_token":true}`
返回与 status 同结构，另带 `persisted`（落盘是否成功；失败时本次运行仍生效）。

### `GET /api/external/token` （仅本机）
`{"ok":true,"token":"..."}` — 供设置页复制粘贴给外部程序。

### `POST /api/external/chat`
| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `text` | str | **必填**，要说给角色听的话（≤2000 字符） |
| `client_uid` | str | 可选，目标前端窗口；缺省 = 最近连接的那个 |
| `interrupt` | bool | 可选，目标正在说话时是否打断（默认 false → `409`） |
| `wait_reply` | bool | 可选，是否等到本轮回复文本再返回（默认 false，立即 `202`） |
| `timeout_s` | float | 可选，`wait_reply` 的等待上限，封顶 180 |
| `token` | str | 可选，令牌也可放 body（推荐用请求头） |

响应：

| 码 | 含义 |
| --- | --- |
| `202` | 已接收（fire-and-forget，或 `wait_reply` 超时但对话仍在继续） |
| `200` | `wait_reply=true` 且本轮完成：`{"reply":"...","completed":true}` |
| `400` | `text` 缺失/空 |
| `403` | 开关关闭 / 来源不允许 / 令牌不正确 |
| `409` | 没有已连接的前端窗口，或角色正在说话且未 `interrupt` |
| `413` | 文本过长 |
| `502` | 对话内部异常（`wait_reply` 期间） |

`wait_reply` 用 `asyncio.shield` 包裹本轮任务：**超时只结束"等待"，不会取消正在进行的对话**。

---

## 4. 用法示例

```bash
# 1) 让角色说一句（不等回复）
curl -X POST http://127.0.0.1:12393/api/external/chat \
  -H 'Content-Type: application/json' \
  -d '{"text":"提醒我该喝水了"}'

# 2) 等回复文本（最多 30s）
curl -X POST http://127.0.0.1:12393/api/external/chat \
  -H 'Content-Type: application/json' \
  -d '{"text":"今天天气怎么样？","wait_reply":true,"timeout_s":30}'

# 3) 局域网调用（需在设置页勾选"允许局域网"并生成令牌）
curl -X POST http://192.168.1.7:12393/api/external/chat \
  -H 'Content-Type: application/json' \
  -H 'X-Live2DAI-Token: <令牌>' \
  -d '{"text":"有人按门铃了","interrupt":true}'
```

---

## 5. 前端表现

- 注入成功后，后端会向该会话推一条 `{"type":"external-input","text":...,"source":...}`；
  前端把它渲染成**带「外部接入」角标的用户气泡**（`renderer/src/chat-ui.ts`），
  并顶部弹一条轻提示 —— 用户始终知道"角色为什么突然开口"。
- 聊天面板头部在开关打开时显示常亮胶囊「外部接入中」。
- 设置页「外部接入」分区：开关 / 立即断开 / 允许局域网 / 令牌生成与清除 /
  可复制的 curl 示例 / 在线窗口数 / 接收与拒绝计数 / 最近 10 条记录（5s 轮询）。

---

## 6. 测试覆盖

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 鉴权矩阵 + HTTP | `tests/test_external_input.py` | 默认关闭、回环/远程、令牌有无与错误、文本校验、`wait_reply`、409/413、最近记录 |
| 注入语义 | 同上 `TestInjectTextInput` | 目标选择（最近会话）、未知 uid、忙碌拒绝、`interrupt` 打断、回显消息内容 |
| 前端纯逻辑 | `renderer/tests/external-input.test.ts` | 范围描述、开关前置校验、curl 生成、最近记录格式化、API 错误冒泡 |
| 端到端接线 | 一次性脚本（见 CHANGELOG 验证段） | 真实 `server.py` 装配下的 20 项检查全过 |
