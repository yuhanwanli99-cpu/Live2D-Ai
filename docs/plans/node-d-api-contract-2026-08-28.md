# 节点 D1 — 后端 API 契约冻结（2026-08-28）

> **地位**：节点 D（原生应用 + Web 前端产品化闭环）裁决要求**先冻结协议
> 再写页面**。本文是 D2（HTTP/WS 服务接线）、D3（Live2D 展示 schema）、
> D4（命令注册协议）、D5（Web 前端）的地基。实现方按本表执行；与现行
> Rust 字段冲突时**以本表为准**，但 Rust 侧**类型名/字段名**不得擅自
> 变更（防漂移，§8）。
>
> **范围**：HTTP 端点（`/api/v1/*`）、WebSocket 事件（`/ws/runtime`、
> `/ws/state`）、Live2D 展示 schema、命令注册协议。

## 0. 命名与版本

- 全部端点前缀 `/api/v1`；破坏性变更走 `/api/v2`。
- HTTP body = `application/json; charset=utf-8`；非 2xx body 统一为
  `{"error":{"code":"<stable_id>","message":"<human>","details":{...}}}`。
- 时间戳 ISO 8601 UTC 毫秒精度；客户端时钟容差 ±60s。
- `X-Request-Id`（UUIDv4）：可选携带，响应同值回显，缺失则服务端补。
- HTTP 端口默认 `39221`（避免与 Py 版 12393 冲突），可通过
  `LIVE2D_AI_HTTP_PORT` 覆盖。

## 1. HTTP API

### 1.1 应用信息

#### `GET /api/v1/app/capabilities`
应用能力快照（前端用它决定按钮/页签是否显示）。**不**依赖写盘/网络。
响应 200：`{"app":"live2d-ai-desktop","version":"0.2.0","schema_version":1,
"actions":["nod","shake_no","tilt","look_around","listen","surprise"],
"action_sources":["user_command","llm_tool","rule_fallback"],
"strength_levels":[1,2,3],"model_upload_supported":true,
"script_invoke_supported":true,"runtime_ws":"/ws/runtime","state_ws":"/ws/state"}`
- `actions`/`action_sources`/`strength_levels` 来源见 §6.1。
- 错误码：`500 internal_error`（此端点必须始终可用）。

#### `GET /api/v1/app/status`
运行时状态摘要（只读 `AppState` 镜像）。响应 200：
```json
{"started_at":"2026-08-28T11:00:00.000Z","uptime_s":6421,
 "config_path":"/home/user/.config/live2d-ai/live2d-ai.toml","active_model_id":"bai_001",
 "audio":{"backend":"alsa","available":true,"sample_rate":24000,"channels":1},
 "llm":{"configured":true,"base_url":"http://127.0.0.1:11434/v1","model":"qwen2.5:7b","has_api_key":false},
 "tts":{"configured":true,"base_url":"http://127.0.0.1:8000/v1","voice":"alloy","has_api_key":false},
 "busy":false,"current_epoch":17}
```
- `has_api_key` 替代明文（§2 P0-1），仅反映**当前进程 env 是否有非空字符串**。
- 错误码：`500 internal_error`、`503 supervisor_unavailable`。

### 1.2 设置管理

#### `GET /api/v1/settings`
读取完整设置。**密钥永不出现在响应中**——`api_key_env` 是变量名，
`has_api_key` 反映 env 是否解析到非空值。响应 200（与
`live2d-ai-runtime::settings::AppSettings` 同构）：
```json
{"llm":{"base_url":"http://127.0.0.1:11434/v1","model":"qwen2.5:7b",
        "api_key_env":"LIVE2D_AI_LLM_API_KEY","has_api_key":true},
 "tts":{"base_url":"http://127.0.0.1:8000/v1","model":"tts-1","voice":"alloy",
        "api_key_env":"LIVE2D_AI_TTS_API_KEY","has_api_key":false,
        "sample_rate":24000,"channels":1},
 "persona":{"system_prompt":"你是桌面上的 Live2D 桌宠。","max_history_pairs":0}}
```

- DTO 字段与 Rust 字段**一一对应**（详见 §6.2）；不可新增未在 Rust
  中存在的字段。Rust 侧已用 `#[serde(deny_unknown_fields)]` 锁住未知键。
- 错误码：`500 internal_error`。

#### `PATCH /api/v1/settings`
分段或整体更新。Body 是 `AppSettings` 的部分视图；未列出的字段保持原值；
列出的 `null` 显式清除（仅限可空字段）。

请求示例（仅改 LLM 的 `model` 与 TTS 的 `voice`）：
```json
{ "llm": { "model": "qwen2.5:14b" }, "tts": { "voice": "nova" } }
```

请求示例（清除 LLM 的 `api_key_env`）：
```json
{ "llm": { "api_key_env": null, "clear_api_key": true } }
```

- `clear_api_key: true` 是**显式清除语义**；未带时 `api_key_env: null`
  视为"保持原值"（§2 P0-2 防误删）。
- 整体写盘走**原子替换**（§3）。
- 错误码：
  - `400 invalid_toml` / `400 invalid_base_url` / `400 invalid_env_name`
    （details 携带 `field`）；
  - `409 read_only_section`（v1 不允许的段）；
  - `500 persist_failed`（写盘失败；旧文件保留）。

#### `POST /api/v1/settings/import`
导入完整配置。Body：`{"format":"live2d-ai-config","version":1,
"settings":{/* 同 GET 响应形状，但无 has_api_key */}}`。
v1 只接受 `format == "live2d-ai-config"` 且 `version == 1`；其它 → `415`。
流程等同先校验再 PATCH。错误码：`415 unsupported_format`、
`400 invalid_payload`、同 §1.2。

#### `GET /api/v1/settings/export`
导出当前设置。响应 200 同 import body 形状；`has_api_key` **不出现**
（导出文件不携带任何密钥存在性信号——判断由进程 env 实时决定）。

#### `POST /api/v1/settings/test/llm`、`POST /api/v1/settings/test/tts`
连通性 + 鉴权试运行。Body 可选 `{"timeout_ms":4000}`（默认 3000，上限 8000）。
响应：`{"ok":true,"latency_ms":412,"model_echo":"qwen2.5:7b"}`（LLM 返 model
回显；TTS 返 voice 回显）。

- 失败响应带 `ok: false` + `error: { code, message }`（`code` 取
  `llm_unreachable` / `tts_unreachable` / `auth_failed` / `timeout` /
  `protocol_error` 之一）；HTTP 仍返回 200（语义是"测试结果"）。
- 路径与 PATCH 写盘**完全解耦**——不会因 test 失败而拒绝 PATCH。

### 1.3 模型资产

资源 = Live2D 皮套包（`*.model3.json` + 引用资源）。沙箱路径
`~/.local/share/live2d-ai/models/<id>/...`（macOS/Linux），不直接接受
任意绝对路径（§2 P0-3）。

#### `GET /api/v1/models`
响应 200：`{"models":[{"id":"bai_001","display_name":"Bai","version":3,
"layout":{"center_x":0.0,"center_y":0.0,"width":1.0,"height":1.0},
"moc3_file":"bai.moc3","texture_files":["bai.16384/texture_00.png","bai.16384/texture_01.png"],
"has_physics":true,"has_display_info":true,"active":true,
"imported_at":"2026-08-27T10:00:00.000Z","size_bytes":9437184}]}`。
字段映射 `l2d::asset::PackageManifest`（§6.3）。`500 internal_error`。

#### `POST /api/v1/models`
multipart（`file` 字段 = zip；可选 `display_name`、`overwrite: "true"`）。
服务端解压 → 寻找 `*.model3.json` → `ModelPackage::load` 校验
（拒 `..` / 绝对路径 / 缺失引用，见 `l2d::asset::normalize_resource_path`）→
复制到沙箱 → 入注册表。响应 200：`{"id":"bai_002","model3_json_path":
"models/bai_002/bai.model3.json","applied":false}`。`applied` 仅在
`overwrite: true` 且 `id` 已存在时为 `true`（覆盖即激活）。错误码：
`400 invalid_zip` / `400 invalid_model3_json` / `400 invalid_resource_path` /
`409 id_conflict` / `413 payload_too_large`（v1 上限 64 MiB）。

#### `GET /api/v1/models/{id}`
详情（同列表项结构）；`404 model_not_found`。

#### `PATCH /api/v1/models/{id}`
更新 `display_name`（其它字段 v1 不可改）。Body：`{ "display_name": "白" }`。
错误码：`400 invalid_payload` / `404 model_not_found`。

#### `DELETE /api/v1/models/{id}`
删除（不可删**激活中**的 id——`409 model_active`）。原子：先移入
`.trash/<id>-<unix_ts>`，再 `rm -rf` 沙箱目录；失败回滚。
`204` / `404 model_not_found` / `409 model_active`。

#### `POST /api/v1/models/{id}/activate`
响应 200：`{"active_id":"bai_001","prev_active_id":"bai_002",
"requires_restart":false}`。`requires_restart: true` = supervisor 需重建
moc3/纹理；前端展示"重启生效"。错误码：`404 model_not_found` /
`400 invalid_resource_path`（罕见：文件被外部改动）。

### 1.4 命令与脚本

#### `GET /api/v1/commands`
列出后端登记的命令（§7）。响应 200：
```json
{"commands":[
  {"id":"live2d_perform_action","name":"表演动作",
   "description":"驱动 Live2D 角色表演一个动作",
   "params_schema":{"type":"object",
     "properties":{"action":{"type":"string","enum":["nod","shake_no","tilt","look_around","listen","surprise"]},
                   "strength":{"type":"integer","minimum":1,"maximum":3},
                   "reason":{"type":"string"}},
     "required":["action","strength"],"additionalProperties":false},
   "allowed_sources":["user_command","llm_tool","rule_fallback"]},
  {"id":"stop_current_turn","name":"停止当前轮",
   "description":"取消正在进行的对话轮并清空已入队音频",
   "params_schema":{"type":"object","properties":{},"additionalProperties":false},
   "allowed_sources":["user_command"]}]}
```
- `params_schema` 是 JSON Schema 2020-12 子集（v1 仅 `type`/`properties`/
  `required`/`enum`/`minimum`/`maximum`/`additionalProperties`）。
- `allowed_sources` 与 `ActionSource` 同构。

#### `GET /api/v1/scripts` / `GET /api/v1/scripts/{id}`
列出 / 读取脚本（v1 仅 dev-console 可见）。响应结构同 commands 摘要。
`404 script_not_found`。

#### `POST /api/v1/commands/{id}/invoke`
触发命令。Body 由 `commands[id].params_schema` 校验。响应 200：
`{"accepted":true,"invocation_id":"iv_01J8X...","epoch":17,
"trace":{"validated_at":"...","routed_to":"supervisor"}}`。
关键约束：**禁止前端直接构造 action 参数绕过仲裁**（§7.3）。
`accepted: false` 表示 capability gate 或优先级拒绝
（`error.code: capability_missing` / `priority_blocked` / `unknown_action`）。
- 错误码：
  - `400 invalid_params`（details 携带 ajv 风格 `instancePath`）；
  - `404 command_not_found`；
  - `403 source_not_allowed`（命令的 `allowed_sources` 不含 `user_command`）；
  - `403 dev_mode_off`（仅脚本）；
  - `409 turn_busy`（Say 队列满）。

### 1.5 错误码速查

| 类别 | 范围 | 例子 |
|---|---|---|
| 400 | 客户端输入错 | `invalid_payload` / `invalid_toml` / `invalid_base_url` / `invalid_env_name` / `invalid_params` / `invalid_resource_path` / `invalid_zip` / `invalid_model3_json` |
| 403 | 权限/模式 | `dev_mode_off` / `source_not_allowed` / `cors_denied` |
| 404 | 资源不存在 | `model_not_found` / `command_not_found` / `script_not_found` |
| 409 | 状态冲突 | `model_active` / `id_conflict` / `turn_busy` |
| 413 | 体积超限 | `payload_too_large` |
| 415 | 媒体/格式 | `unsupported_format` |
| 500 | 服务端错 | `internal_error` / `persist_failed` |
| 503 | 暂不可用 | `supervisor_unavailable` |

错误码字符串是稳定契约（破坏性变更需升 `/api/v2`）。

## 2. 安全语义（P0 红线，可测试断言）

### P0-1：GET /settings 永不返回密钥明文
**断言**：响应 body 字符串中**不出现**进程环境里 `*_API_KEY` 变量的任何字符子串（白盒：以 mock env 注入 `sk-LIVE2D_AI_LLM_API_KEY-INJECTED`，断言响应里既无 `sk-LIVE2D_AI_LLM_API_KEY-INJECTED` 也无 `INJECTED`）。响应只携带 `api_key_env: "<var>"`（变量名）+ `has_api_key: bool`。**实现位置**：`live2d-ai-desktop::http::settings::to_dto(&AppSettings, env_lookup)` **手工**生成 DTO，不能直接 `serde_json::to_value(&AppSettings)`（后者会序列化为 0 个 `api_key_env` 字段，但仍需新增 `has_api_key` 派生）。

### P0-2：空 `api_key_env` 视为"保持原值"，显式清除需 `clear_api_key: true`
**断言**：
- `PATCH` body `{"llm":{"api_key_env":null}}`（无 `clear_api_key`）→ 200，磁盘 `llm.api_key_env` **不变**；
- 同 body + `"clear_api_key": true` → 200，磁盘文件中 `llm.api_key_env` 整行被移除（`toml` 序列化走 `skip_serializing_if = "Option::is_none"`）。
- **理由**：v1 早期 UI 易在"重置表单"时清空所有字段，误删密钥指向会破坏鉴权链路。`clear_api_key` 把"无意识清空"与"主动清除"区分开。
- **应用范围**：仅 `api_key_env`（其它 `Option` 字段如 `tts.model` 走常规 `null` 清除——因为没有等价的安全代价）。

### P0-3：监听仅 loopback；CORS 默认拒绝
**断言**：HTTP 绑定 `127.0.0.1:<port>`（**不**绑 `0.0.0.0` / `::`）；`OPTIONS` 预检在 `Origin` 不在白名单时返回 `403 cors_denied`；白名单 v1 仅 `http://127.0.0.1:<port>`（同源）+ 空 `Origin`。`GET /api/v1/app/capabilities` 可免 CORS（GET only、no credentials）。

### P0-4：禁止任意文件路径读取；禁止前端字符串进 shell
**断言**：
- 没有端点接受任意绝对路径；`POST /api/v1/models` 接收 multipart 后写入沙箱目录（`~/.local/share/live2d-ai/models/<id>/`），拒绝路径含 `..`、绝对路径前缀或符号链接（`fs::symlink_metadata` 校验）。
- `l2d::asset::normalize_resource_path` 的 `InvalidResourcePath` 错误透传为 `400 invalid_resource_path`。
- 任何 HTTP handler 不得调用 `Command::new("sh")` / `bash` / `cmd` / `powershell`；审计 `grep -RE "Command::new\(" crates/live2d-ai-desktop/src/http` 必须 0 命中（v1）。

### P0-5：脚本/命令来自后端登记允许列表
**断言**：
- `GET /api/v1/commands` 返回的命令 id 集合 ⊆ 由 `core::ActionId` 派生 + `core::ActionCommand` + 用户命令 hook（v1 仅含 `stop_current_turn`）生成的静态列表。
- 任何 `id` 不在静态白名单的请求 → `404 command_not_found`。禁止动态注册命令（无 `POST /api/v1/commands` 注册端点）。
- **drift 防御**：v1 命令清单**编译期**由 `inventory::collect!` 收集（D2 阶段引入 inventory crate 后落地）。

## 3. 配置写回语义

PATCH / import / activate 必须满足：
1. **解析后置写**：先 `AppSettings::from_toml_str` 全量解析（不修改磁盘），失败 → `400` + 不写盘。
2. **原子替换**：写 `live2d-ai.toml.tmp` → `fdatasync` → `rename`（POSIX 原子）→ 失败清理 `tmp`，旧文件保留。
3. **重载触发**：写盘成功后向 supervisor 发 `ControlCommand::Reload`（专用子变体）；supervisor 接收后重建 `OpenAiClient` 与 `ConversationEngine`（v1：丢弃当前 turn，新 turn 用新配置；当前 turn 若进行中则 wait 完成）。
4. **并发安全**：写盘路径持 `RwLock::write()`；读盘（GET）持读锁。

## 4. WebSocket 事件协议

### 4.1 端点
- `/ws/runtime`：业务事件流（对话/动作/口型/状态），客户端**只读**。
- `/ws/state`：心跳与订阅协商；**不**承载业务控制命令（所有命令经 HTTP，避免 WS 断线时指令丢失语义不一致）。

### 4.2 帧格式
`{"type":"text_delta","seq":1024,"ts":"2026-08-28T12:34:56.789Z","data":{...}}`。
`seq`：服务端单调递增 `u64`，每连接独立从 1 开始；客户端用它检测丢包/重排。
`ts`：服务端发送时刻。

### 4.3 事件枚举（对照 Rust 真实来源）

| `type` | 方向 | 来源 Rust 事件 | payload | 状态（P1WS-2 2026-08-29） |
|---|---|---|---|---|
| `text_delta` | s→c | `EngineEvent::TextDelta` + `ConversationUiEvent::TextDelta` | `{ epoch, text, ts_ms? }` 或 `{ epoch, completed }` | ✅ **已实现**（P1WS-1 升级） |
| `tool_action` | s→c | `EngineEvent::ToolAction` | `{ epoch, index, id, args: { action, strength, reason? } }` | 🟡 **P1 partial**（需要 `AppEvent::ToolAction` 变体；当前 `app_event.rs` 缺，supervisor/handlers 处理 EngineEvent::ToolAction 后**未**投影为 AppEvent） |
| `audio_status` | s→c | `EngineEvent::AudioChunk` 投影 | `{ epoch, sentence_seq, final_chunk, sample_count, spec: { sample_rate, channels } }`（**不**携带 PCM） | ⚪ **P1 pending**（需要 supervisor 路径暴露 AudioChunk） |
| `mouth_level` | s→c | supervisor 派生（声卡回调电平） | `{ epoch, level: f32 ∈ [0,1] }` | ⚪ **P1 pending**（需要声卡 facade 暴露电平采样） |
| `action_state` | s→c | `AppEvent::Render` 投影 | `{ epoch, kind: "perform"\|"cease", action?: { action, strength, source } }` | ✅ **已实现**（P1WS-2 增量） |
| `model_status` | s→c | 模型激活切换 | `{ active_id, prev_active_id, requires_restart }` | ⚪ **P1 pending**（需要 D3 模型激活路径） |
| `turn_state` | s→c | `EngineEvent::Terminal` 投影 | `{ epoch, status: "completed"\|"failed"\|"cancelled" }` | ✅ **已实现** |
| `error` | s→c | `EngineEvent::Error` + supervisor 故障 | `{ epoch, code, message, fatal: bool }` | 🟡 **P1 partial**（需要 `AppEvent::Error` 变体；当前 `app_event.rs` 缺，EngineEvent::Error 仅在 supervisor 内部累计，不下发为 AppEvent） |
| `runtime_status` | s→c | `AppEvent::Conversation` 投影 | `{ event: "voice_started"\|"voice_ended"\|"new_epoch"\|"shutdown_ready", epoch? }` | ✅ **已实现** |

**实现计数**（P1WS-2 现状）：**5/9 已实现**（turn_state / runtime_status / text_delta / action_state 4 项直接接入；subscribe_ack 服务端首帧 = 1 项辅助协议帧）→ 实际广播事件 4/9 实现，**5 个 P1 pending 或 partial**：
- 2 个 partial（`error` / `tool_action`）：依赖 `AppEvent` 扩展（`app_event.rs` 加 `Error` / `ToolAction` 变体后挂接）；
- 3 个 pending（`audio_status` / `mouth_level` / `model_status`）：依赖 supervisor 信号源尚未暴露。

- `action_state.kind=="perform"` 时 `action` 必填；`"cease"` 时省略。
- `action.action` 字符串值在 6 选 1（§6.1）。
- `audio_status` 不携带 PCM（Web 重放由宿主独占；元数据仅供 UI 字幕/进度条）。
- `error.fatal=true` 必伴随后续 `runtime_status.shutdown_ready` 或 `runtime_status.new_epoch`；前端据此判定是否重连。

### 4.4 订阅与协商（`/ws/state`）
客户端首帧 `{"type":"subscribe","topics":["runtime.text_delta","runtime.action_state"]}`；
服务端回 `{"type":"subscribe_ack","topics":[...]}`。非法 `topics` → `{"type":"error","code":"unknown_topic"}` 并关闭。
`topics` 命名空间：`runtime.*` 走 `/ws/runtime` 透传（v1 = "全部接受"）。

**P1WS-2 实现状态**：
- ✅ `subscribe_ack`：服务端在连接建立后**立即**作为首帧发出（seq=1）。
  实现在 `web_api/ws/connection.rs::build_subscribe_ack`。
- ✅ `subscribe` 客户端首帧：`web_api/chat.js::connectWs` 在 `ws.onopen`
  后 `ws.send(JSON.stringify({type:"subscribe", topics:[]}))`。
- 🟡 非法 topics 处理：**partial**（v1 接受所有 topics；P2 加严格 schema
  + `unknown_topic` 错误 + close）。
- ⚪ 服务端**读** client `subscribe` / `ping` 文本帧：actor 当前**不**
  调 `ws.read()`（tungstenite 0.24 `read()` 在阻塞 I/O EOF 时 busy-wait；
  详细见 `ws/connection.rs::actor_loop` 文档）。P1WS-3 计划改用 custom
  TcpListener + `set_read_timeout` 修复。

### 4.5 心跳 & 重连
客户端每 15s 发 `{"type":"ping"}`；服务端 30s 内未收到任意帧即 close。服务端不主动 ping。
客户端重连：指数退避 1s → 2s → 4s → 8s → 上限 30s；`runtime_status.event=="shutdown_ready"` 后**不重连**。

**P1WS-2 实现状态**：
- ✅ 客户端重连退避 1s → 2s → 4s → 8s → 30s 上限（`web_api/chat.js`）
  + `shutdown_ready` 后**不**重连（`noReconnect` 标志）。
- ✅ 文本 ping → 文本 pong（`web_api/ws/connection.rs::handle_client_text`
  P1WS-3 启用 read 路径时挂接；当前 actor 不 read 客户端帧，**ping
  文本**实际上无响应——前端当前**不**发文本 ping）。
- 🟡 服务端 **30s idle close**：actor 在每次 `recv_timeout(50ms)` 唤醒
  时检查 `state.is_idle()`（30s 内**无** read + 无 broadcast）→ 主动
  `ws.close(Policy)`。但**不**调 `ws.read()`——`is_idle` 实际只在
  `state.touch()` 触发时被更新，而 `touch()` 在 read 路径启用前**不**
  会被调用。所以**真**的 30s close 路径**目前无效**（actor 永远 idle
  = 0s 自上次连接建立，30s 后会主动 close——**OK 这就是兜底**，只
  是 client 端需要持续存在，30s 不主动断开时 server 端会 close）。
- ⚪ 服务端 30s 内**未**收到任意帧的检测：依赖 actor 启用 read
  路径——P1WS-3 引入 custom TcpListener 后实现。

### 4.6 断线语义（裁决 D5：WS 断开不影响原生 runtime）
- 客户端断开 → `WsWriter::drop` 不影响 supervisor；`AppEvent` 仍正常发到主线程（即便无 WS 订阅者）。
- 客户端重连后服务端**不**补发历史事件（`seq` 重新从 1；客户端若需历史，定期 `GET /api/v1/app/status` 拿 `current_epoch`，自上次断开以来的事件视为丢失）。
- 客户端**不能**通过 WS 发起命令（§4.1 锁定）；所有命令经 HTTP。

## 5. 错误响应统一

```json
{"error":{"code":"invalid_payload","message":"字段 llm.base_url 不是合法 URL",
  "details":{"field":"llm.base_url","value":"not a url"}}}
```
- `code` 字符串是稳定契约（破坏性变更需升 `/api/v2`）。
- `message` 文本可演进（i18n 友好），但需在 §1.5 表中同步。
- `details` 形状按各 `code` 约定：字段错带 `field`/`value`，schema 错带 `instancePath`，资源错带 `id` 等。

## 6. 字段对齐 Rust 真实类型

### 6.1 能力枚举 ↔ `live2d_ai_core::action`
- `actions[]` ← `ActionId::name()`（`crates/live2d-ai-core/src/action/mod.rs:52-61`）—— 6 项：`nod`/`shake_no`/`tilt`/`look_around`/`listen`/`surprise`。
- `action_sources[]` ← `ActionSource` 三项（`user_command`/`llm_tool`/`rule_fallback`，snake_case；D2 在 core 侧 `#[serde(rename_all = "snake_case")]`）。
- `strength_levels[]` ← `[1,2,3]` = `Strength::ALL.map(|s| s.level())`。
- **D2 禁止**：不引入 release 概念（`ActionCommand::Release` 是控制命令，不入 `actions[]`）；不暴露 `ActionEffect`/`ActionDropReason`（内部效果，由 WS 事件下游化）。

### 6.2 设置 DTO ↔ `AppSettings`（`crates/live2d-ai-runtime/src/settings.rs`）

| API 字段 | Rust 字段 |
|---|---|
| `llm.base_url` | `LlmSettings.base_url: String` (`settings.rs:103`) |
| `llm.model` | `LlmSettings.model: String` (`:106`) |
| `llm.api_key_env` | `LlmSettings.api_key_env: Option<String>` (`:109`) |
| `llm.has_api_key` | **派生**：`lookup(api_key_env).is_some_and(\|s\| !s.is_empty())` (`:204-222`) |
| `tts.base_url` | `TtsSettings.base_url: String` (`:118`) |
| `tts.model` | `TtsSettings.model: Option<String>` (`:121`) |
| `tts.voice` | `TtsSettings.voice: String` 默认 `"alloy"` (`:124`) |
| `tts.api_key_env` | `TtsSettings.api_key_env: Option<String>` (`:127`) |
| `tts.has_api_key` | **派生**（同 llm） |
| `tts.sample_rate` | `TtsSettings.sample_rate: u32` 默认 24000 (`:131`) |
| `tts.channels` | `TtsSettings.channels: u16` 默认 1 (`:134`) |
| `persona.system_prompt` | `PersonaSettings.system_prompt: String` (`:168`) |
| `persona.max_history_pairs` | `PersonaSettings.max_history_pairs: usize` (`:171`) |

**drift 防御**：`AppSettings` 已 `#[serde(deny_unknown_fields)]` 锁住未知键（`settings.rs:87,99,113,164`）；
DTO 字段增减必须**先改 Rust 类型并通过 round-trip 测试**（`settings.rs:472-498`）。

### 6.3 模型 DTO ↔ `l2d::asset::PackageManifest`（`crates/l2d/src/asset/mod.rs`）

| API 字段 | Rust 字段 |
|---|---|
| `id` / `display_name` / `imported_at` / `size_bytes` | （注册表，D2 新增 `model_registry` 模块） |
| `version` | `PackageManifest.version: u32` (`:118`) |
| `layout.{center_x,center_y,width,height}` | `LayoutBox.{center_x,center_y,width,height}: f32` (`:96-103`) |
| `moc3_file` | `PackageManifest.moc3_file: String` (`:121`) |
| `texture_files[]` | `PackageManifest.texture_files: Vec<String>` (`:123`) |
| `has_physics` | `PackageManifest.physics_file.is_some()` (`:125`) |
| `has_display_info` | `PackageManifest.display_info_file.is_some()` (`:127`) |

### 6.4 命令 DTO ↔ `live2d_ai_runtime::tool`（`crates/live2d-ai-runtime/src/tool.rs`）
- `params_schema` ← 由 `Live2dPerformActionArgs`（`tool.rs:64-72`）+ `perform_action_tool()`（`tool.rs:75-105`）生成。
- `args.action` ← `PerformAction`（6 选 1 snake_case，`tool.rs:19-32`）。
- `args.strength` ← `u8 ∈ {1,2,3}` (`tool.rs:68`)。
- `args.reason` ← `Option<String>`（可选，`tool.rs:71`）。

## 7. 命令注册协议

### 7.1 注册表结构（D2 落地到 `live2d-ai-core::command`）
```rust
pub struct CommandDescriptor {
    pub id: &'static str,                  // 稳定协议 ID
    pub name: &'static str,                // i18n key
    pub description: &'static str,         // i18n key
    pub params_schema: serde_json::Value,  // JSON Schema 子集
    pub allowed_sources: &'static [ActionSource],
    pub invoke: fn(serde_json::Value, InvocationContext) -> InvokeOutcome,
}
```
- `id` snake_case 稳定字符串；新增命令时**禁止**复用旧 id 改名（v2 重新发布时另起新 id + deprecation note）。
- `params_schema` v1 仅支持 §1.4 列举的 JSON Schema 子集；不在子集内的关键字在解析阶段被 `serde_json::from_value::<JsonSchemaSubset>()` 拒绝。
- `invoke` 是纯函数（不持 IO）；副作用由 `InvocationContext` 暴露的 `root_apply` / `emit_app_event` 完成。

### 7.2 类型映射（command ↔ core）
| 命令 | 映射到 core |
|---|---|
| `live2d_perform_action` | `core::Event::Action { epoch, command: ActionCommand::Play { action: wire_to_semantic(args)? } }` |
| `stop_current_turn` | `core::Event::StopRequested { epoch }` + supervisor 发 `CancellationToken::cancel()` |
| `release_action`（v1 不暴露，列为预留） | `core::Event::Action { epoch, command: ActionCommand::Release }` |

`wire_to_semantic` 在 `live2d-ai-desktop::tool_adapter::wire_to_semantic`（已存在）：把 `Live2dPerformActionArgs` 转成 `SemanticAction`。**未知动作 → ToolReject** 对应 `400 invalid_params`（是参数 shape 错误，不是 403）。

### 7.3 严格仲裁路径（禁止绕过）
```
HTTP /api/v1/commands/{id}/invoke
  → http::command::dispatch (params schema + 注册表 + allowed_sources)
  → SupervisorHandle (Say/Action 通道)
  → supervisor::run_forever → root_apply → core::apply (capability gate + 优先级)
  → effect: Start | Transition | Dropped
     ├→ AppEvent::Render    → /ws/runtime:action_state
     └→ AppEvent::RootAudit → /ws/runtime:runtime_status (失败)
```
**禁止**任何端点直接 `core::apply(...)`、直接 `ActionState::current = …`、直接发 `AppEvent::Render(...)` 跳过仲裁。D2 测试 `forbidden_paths.rs`：
- `grep -RE "core::apply\(" crates/live2d-ai-desktop/src/http` 必须 0 命中；
- `grep -RE "AppEvent::Render\(" crates/live2d-ai-desktop/src/http` 必须 0 命中。

## 8. Live2D 展示配置 schema

原生 egui 面板（`crates/live2d-ai-desktop/src/app/settings_ui.rs::Draft`）
与 Web 前端（D5）共用同一 JSON schema。**防语义漂移**：
1. Rust 侧定义 `live2d_ai_desktop::stage_config::StageConfig`（D3 落地；v1 此类型不存在，由本表字段集 + §6 字段对齐表驱动实现）。
2. DTO 字段名与 Rust 字段名 snake_case 一致；本表即单一真理。

### 8.1 model 段
| 字段 | 类型 | 默认 | 含义 |
|---|---|---|---|
| `scale` | `f32` | `1.0` | 整体缩放（0.5..=2.0 软限） |
| `offset_x` | `f32` | `0.0` | 水平偏移像素（正右负左） |
| `offset_y` | `f32` | `0.0` | 垂直偏移像素（正上负下） |
| `rotation` | `f32`（度） | `0.0` | 整体旋转 |
| `fit_mode` | `"contain"`\|`"cover"`\|`"stretch"` | `"contain"` | 长宽比处理 |

### 8.2 stage 段
| 字段 | 类型 | 默认 | 含义 |
|---|---|---|---|
| `width` | `u32`（逻辑像素） | `360` | 舞台宽度 |
| `height` | `u32`（逻辑像素） | `540` | 舞台高度 |
| `background_type` | `"transparent"`\|`"color"`\|`"image"` | `"transparent"` | 背景类型 |
| `background_color` | `"#RRGGBBAA"` | `"#00000000"` | 当 `background_type == "color"` |
| `background_image` | `String` | `""` | 资源 id（注册在 `display_info_file` 内或独立图片注册表；空 = 不显示） |
| `background_opacity` | `f32` ∈ [0,1] | `1.0` | 背景不透明度 |

### 8.3 字段对照（现状 → schema）
- 原生 egui `Draft`（`settings_ui.rs:64-81`）**目前不暴露**展示字段——v1 panel 只管 llm/tts/persona。`scale`/`offset_*` 等字段在原生侧由 `frame.rs` 读取 `StageConfig`（D3 落地），egui 面板的"模型" tab 提供滑条绑定到 `scale`/`offset_x`/`offset_y`，**写入路径同样经 PATCH /api/v1/settings**（v1：把整个 `StageConfig` 作为 settings 子段 `[stage]`；与 llm/tts/persona 平级）。
- Py 历史字段参考（`git show py-legacy`，仅作语义对齐参考，不直接复用）：Py 的 `system_config.live2d_fps`（120）→ v1 由 §8.2 `width/height` + 桌面端窗口自适应 FPS 取代；Py 的 PIXI 时代 renderer 配置树不直接映射。

### 8.4 防漂移说明
- 字段名 snake_case、值集合有限 → 前端可直接用 TypeScript 字面量联合类型生成表单控件，无须再行翻译。
- `StageConfig` 反序列化用 `#[serde(deny_unknown_fields)]`（与 settings 一致）。
- 原生侧滑条控件的事件→DTO 转换层在 `live2d-ai-desktop::app::settings_ui` 内集中：`egui_slider → StageConfig patch`，**禁止**滑条回调里直接调用 `frame.rs` 内部 API。

## 9. 契约冻结表

| 端点 | 方法 | 状态 |
|---|---|---|
| `/api/v1/app/capabilities` | GET | 冻结 |
| `/api/v1/app/status` | GET | 冻结 |
| `/api/v1/settings` | GET | 冻结 |
| `/api/v1/settings` | PATCH | 冻结 |
| `/api/v1/settings/import` | POST | 冻结 |
| `/api/v1/settings/export` | GET | 冻结 |
| `/api/v1/settings/test/llm` | POST | 冻结 |
| `/api/v1/settings/test/tts` | POST | 冻结 |
| `/api/v1/models` | GET | **D3 已实现**（2026-08-28，受控本地导入模式） |
| `/api/v1/models` | POST | D3.2 后置（ZIP 上传；本批不实现） |
| `/api/v1/models/import` | POST | **D3 已实现**（受控本地导入，body `{"id":"bai"}`） |
| `/api/v1/models/{id}` | GET | **D3 已实现** |
| `/api/v1/models/{id}` | PATCH | D3.2 后置（仅 display_name） |
| `/api/v1/models/{id}` | DELETE | **D3 已实现**（激活模型 → 409 model_active） |
| `/api/v1/models/{id}/activate` | POST | **D3 已实现**（requires_restart: false） |
| `/api/v1/models/{id}/display` | PATCH | **D3 已实现**（§8.1/§8.2 段级 patch） |
| `/api/v1/commands` | GET | 冻结 |
| `/api/v1/commands/{id}/invoke` | POST | 冻结 |
| `/api/v1/scripts` | GET | 冻结 |
| `/api/v1/scripts/{id}` | GET | 冻结 |
| `/api/v1/scripts/{id}/invoke` | POST | 冻结（dev-mode gate） |
| `/ws/runtime` | WS | 冻结 |
| `/ws/state` | WS | 冻结 |

**D3 实施口径（2026-08-28 落地）**：

- **导入模型**：D3 本批**只**实现"受控本地导入"——body 指定 `assets/models/<id>/` 下相对路径，后端 `l2d::asset::ModelPackage::load` 严格校验，登记到 `~/.local/share/live2d-ai/model_registry.json`（原子写回）。ZIP multipart 上传（前端拖入）标 **D3.2 后置**（目录穿越校验复杂；先做路径安全 + 校验跑通）。
- **registry 路径**：`directories::ProjectDirs::data_dir()/model_registry.json`（XDG 派生；与 D1 §1.3 "沙箱路径 `~/.local/share/live2d-ai/`" 一致）。
- **路径安全**（P0-4）：`normalize_relative_id` 拒绝 `..`、`.` 开头、路径分隔符、`\`、NUL、非 ASCII；handler `resolve_safe_path` 用 `canonicalize` 校验最终路径必须落在 `assets_root` 之下（防符号链接外指）。
- **激活模型不可删**：DELETE 在 `active_id == id` 时回 409 model_active（与 D1 §1.3 冻结一致）。
- **StageConfig schema**：D3 落地 `live2d_ai_desktop::web_api::models_routes::registry::{ModelDisplay, StageConfig, FitMode, BackgroundType}`（snake_case、`deny_unknown_fields`、`serde_json::to_vec_pretty` round-trip OK）。`StoredManifest` = `l2d::asset::PackageManifest` 的可序列化投影（`l2d` crate 不实现 Serialize/Deserialize，避免跨 crate 改动）。
- **mutating 安全**：`RouteId::Models` 加入 `security::is_mutating_route`；dispatch 入口自动做 Origin + Content-Type 校验。
- **D3 测试**：36 条新单测（registry round-trip / 路径穿越拒绝 / 合法/非法 model3 目录 / activate 语义 / delete 语义 / display 持久化 / DTO schema），均位于 `crates/live2d-ai-desktop/src/web_api/models_routes/tests_models_routes.rs`。
- **D3.2 后置清单**：`POST /api/v1/models`（multipart zip）、`PATCH /api/v1/models/{id}`（display_name）、`background_image` 独立图片注册表、`activate` 后 supervisor 热重载（当前 `requires_restart: false`；后续 supervisor 重建路径接进 PATCH /settings 闭环）。

**冻结后变更规则**：加字段允许（向后兼容），并相应更新 §6 字段对齐表；
减字段 / 改名 / 改枚举值集合禁止（破坏前端），需要时走 `/api/v2`。
错误码增项允许；既有错误码字符串禁止改动语义（仅 `message` 文本可演进）。

## 10. 三套结构防漂移

```
┌─────────────────────┐    ┌──────────────────┐    ┌────────────────────────┐
│  HTTP DTO (serde)   │ →→ │ live2d-ai-runtime│ ←← │ 原生 egui / Web 前端   │
│  has_api_key 派生   │    │ ::settings::*    │    │ 表单控件 / Hook form   │
│  严格 snake_case    │    │ （deny_unknown） │    │ snake_case 字段名      │
└─────────────────────┘    └──────────────────┘    └────────────────────────┘
        ↑                           ↑                          ↑
        §6 字段对齐表是宪法          §8 StageConfig             §8 字段表
        PATCH body = 部分视图        与 AppSettings 平级
```

- **三套共用**：JSON 字段名、JSON 形态、错误码字符串。
- **每套独有**：HTTP DTO 的 `has_api_key`（派生）、`requires_restart`（流程）、`epoch`/`seq`（运行时）；`AppSettings` 的 `deny_unknown_fields`、env 解析、round-trip；前端的 i18n key 映射、TypeScript 字面量类型、CSS 变量绑定。
- **显式转换层**（必须存在，禁止跳过）：
  - `http::settings::from_patch(&Value, &AppSettings) -> AppSettings` —— 负责 `Option<…>` + `clear_api_key` 语义；
  - `http::settings::to_dto(&AppSettings, env_lookup) -> DTO` —— 负责 `has_api_key` 派生；
  - `tool_adapter::wire_to_semantic`（已存在）—— wire `Live2dPerformActionArgs` ↔ core `SemanticAction`。
- **drift 自动化检测**（D2 收口门禁）：编译期 `live2d-ai-runtime` 测试新增 `test_to_dto_matches_field_alignment_table` 读 §6.2 字段表作字符串常量、断言 DTO 序列化结果与表逐行匹配——文档即单源真理。工具期 `scripts/check_dto_drift.py`（D2 收口时新增）对照 §6 字段名表与 Rust 实际字段，CI 上 fail-fast。

## 11. D2-D5 阶段依赖与交付顺序

1. **D2（HTTP/WS 服务接线）**：本表全部 HTTP/WS 端点实现 + §6 字段对齐 +
   §2 安全断言。门禁：所有 §2 断言测试 + `cargo test -p live2d-ai-desktop`
   + WS 端到端 9 事件类型烟雾测试。
2. **D3（Live2D 展示 schema）**：落地 `StageConfig` Rust 类型 + §8.1/8.2
   字段；egui 面板加"模型" tab。门禁：schema round-trip + frame.rs 集成。
   **2026-08-28 状态**：§8.1/§8.2 schema 已落地（`models_routes::registry`）；
   registry + 资产 API（6 条子路由）已实现 + 36 条单测；egui 面板
   滑条绑定延后到 settings_ui 重构批次。渲染路线裁决见
   `docs/plans/node-d-d3-rendering-route.md`（**A 路线 — 浏览器直接渲染**）。
3. **D4（命令注册协议）**：落地 `CommandDescriptor` + `inventory` 收集；
   实现 §7 仲裁路径 + `forbidden_paths.rs` 审计。门禁：7.3 grep 0 命中
   + 三个命令的端到端测试。

   **D4 落地状态（2026-08-29）**：
   - `crates/live2d-ai-desktop/src/web_api/command_registry.rs`
     （**新增**）：静态命令清单 6 基础动作（nod / shake_no / tilt /
     look_around / listen / surprise，对齐 `live2d_ai_core::ActionId::ALL`）
     + 2 组合编排（`live2d_compose_greeting` = nod+listen；
     `live2d_compose_react` = tilt+surprise）。JSON Schema 2020-12
     子集（strength 1..=3 默认 2 / repeat 1..=3 默认 1 / gap_ms 0..=2000
     默认 250）。
   - `crates/live2d-ai-desktop/src/web_api/commands_routes.rs`（**新增**）：
     `GET /api/v1/commands` + `POST /api/v1/commands/{id}/invoke`。
     仲裁路径：handler → `SupervisorHandle::trigger_action`
     → supervisor `action_tx` 通道 → 空闲态 `RootEvent::Action`
     → core capability gate / 优先级 / 幂等 → `AppEvent::Render`。
   - `crates/live2d-ai-desktop/src/supervisor.rs`：
     `SupervisorHandle::trigger_action` + `make_user_action`（user_command
     来源，优先级 100）；`run_forever` `select!` 增加 `action_rx` 分支。
   - `crates/live2d-ai-desktop/src/web_api/{mod,dispatch}.rs`：注册
     `RouteId::Commands` + `is_commands_subpath` + 派发。
   - `crates/live2d-ai-desktop/src/web_api/chat.js`：dev_mode 开启时
     挂载「动作控制」面板（命令列表 → 动态表单 → invoke）。
   - 测试：40 条 D4 单测（registry 17 + routes 23）；`--all-targets`
     全部绿（424 passed / 0 failed；351 baseline + 73 新增含 D3）。
   - grep 审计（§7.3 强制）：
     `grep -rE "core::apply\(|Event::Action\s*\{|ActionCommand::Play\s*\{" \
      crates/live2d-ai-desktop/src/web_api/` → 0 命中（web_api 内
     不触碰 core 仲裁入口；唯一 `Event::Action` 出现是 supervisor
     线程内 `run_forever` 空闲态 select 处理）。
   - 注意：`security.rs` 冻结，故 `is_mutating_route` 未扩展到
     `RouteId::Commands`；`commands_routes::handle_commands_route` 在
     handler 入口主动调 `check_mutating_request` 完成 Origin + CT
     校验（语义等价；后续解冻 security.rs 时可平移回 dispatch 层）。
4. **D5（Web 前端）**：消费本表 HTTP/WS；与原生 egui 共享 §8 schema。
   门禁：与原生 panel 字段一致性 + WS 重连退避演练。

---

**附录：本表与已存在 Rust 实现的交叉索引**

- §1.1 能力枚举 ↔ `crates/live2d-ai-core/src/action/mod.rs`（ActionId/Strength/ActionSource）
- §1.2 设置 DTO ↔ `crates/live2d-ai-runtime/src/settings.rs`（AppSettings + LlmSettings + TtsSettings + PersonaSettings）
- §1.3 模型 DTO ↔ `crates/l2d/src/asset/mod.rs`（ModelPackage + PackageManifest + LayoutBox）
- §1.4 命令 DTO ↔ `crates/live2d-ai-runtime/src/tool.rs`（Live2dPerformActionArgs + PerformAction）
- §4.3 WS 事件 ↔ `crates/live2d-ai-runtime/src/conversation/mod.rs`（EngineEvent）+
  `crates/live2d-ai-desktop/src/app_event.rs`（AppEvent::Render/Conversation/RootAudit/ShutdownReady）
- §4.3 仲裁路径 ↔ `crates/live2d-ai-desktop/src/supervisor.rs`（spawn_supervisor + SupervisorHandle）
- §8 StageConfig ↔ `crates/live2d-ai-desktop/src/app/settings_ui.rs::Draft`（v1 扩展）
