# Live2D-Ai 可观测性 / 健康自检骨架

> 目标：不依赖 API Key，也能在本地快速判断“配置、模型、双端一致性、核心依赖”是否健康。

## 1. 健康自检分层

| 层 | 检查内容 | 无 Key 可跑 |
| --- | --- | --- |
| 配置层 | shared 文件存在、可解析、双端副本一致 | ✅ |
| 模型层 | model_registry 合法、Live2D 资产存在 | ✅ |
| 协议层 | emotion-protocol / adapter / PC model_dict 三方一致 | ✅ |
| 核心逻辑层 | Android 状态机、TTS 降级链、PC TTS 队列 | ✅（单测） |
| 服务层 | LLM / TTS 云端连通性 | ❌ 需要 Key / 网络 |

## 2. 本地健康检查脚本

入口：`scripts/health_check.py`

当前检查项：

1. `shared/` 关键文件存在
2. `shared/persona.yaml` 可被 YAML 解析
3. `shared/model_registry.json` 通过 schema 校验（若 jsonschema 可用）
4. Android assets 中的 `persona.yaml`、`mcp_tools.json` 与 shared 字节一致
5. PC `conf.yaml` 存在且引用 `shared/persona.yaml`
6. PC Live2D 模型目录存在（`live2d-models/bai/runtime/bai.model3.json`）
7. 不读取任何 API Key，不发起网络请求

## 3. 日志规范（已落地：logsetup）

入口 `run_server.py` 调用 `open_llm_vtuber.logsetup.setup_logging()`，双 sink：

| sink | 形态 | 用途 |
| --- | --- | --- |
| 控制台（stderr） | `HH:mm:ss | 级别 | 标签 │ 消息`（彩色紧凑） | 运行态一眼可读 |
| 文件 `logs/server.log` | 毫秒时间戳 + `模块:函数:行号`，10MB 轮转保留 5 份 | 排障回放全貌 |

- 级别由 `LIVE2DAI_LOG_LEVEL` 控制（默认 INFO；设置中心可改）；非法值回落 INFO 并提示。
- uvicorn 访问日志逐请求刷屏：非 DEBUG 时压到 WARNING。
- 各模块用 `get_logger("标签")` 取 logger：`tts` / `audio` / `flow` / …，未绑定落 `app`。
- 逐 chunk 的耗时明细（封装/口型等）为 DEBUG 级单行格式；INFO 层只留关键节点。
- `logs/` 与 `*.log` 均已 gitignore；根目录遗留的 `server.log` 为历史手工重定向产物，可删。

## 4. 关键路径日志建议（部分落地）

- LLM：首 token 延迟、请求耗时、失败原因、降级 provider
- TTS：首音延迟、合成耗时、降级原因、播放中断次数
- Live2D：后端（LEGACY/ENGINE）、帧率、模型加载状态
- 状态机：异常 fail-safe 次数、非法事件次数

## 4. 验收

- `python scripts/health_check.py` 无 Key、无网络可运行，退出码 0 表示本地基础健康。
- 健康检查不修改任何文件。

---

## 5. 链路错误码与日志（Rust + Flutter，2026-09-11 落地）

> 上面 §3/§4 描述的是**已归档的 Python 版**（`Live2D-Ai-pc/`，tag `py-legacy`）。
> 当前主线是 Rust 核心 + Flutter 前端，可观测性契约如下。

### 5.1 三条路径，同一个码

一次链路错误（LLM / TTS / 解码 / 背压）必须**同时**能被三种消费者看见，
并且三处用的是**同一个** `code` 字符串——用户才能拿界面上看到的码去日志里搜：

| 消费者 | 载体 | 位置 |
| --- | --- | --- |
| 后端日志 | `tracing::error!(code=…, stage=…, epoch=…, fatal=…, hint=…)` | `supervisor/handlers.rs`（stdout + 文件双 sink） |
| 前端 | `AppEvent::Error` → WS `error` 帧 | `app_event.rs` + `web_api/ws/events.rs` |
| 终端（`--chat`） | 同一份 `AppErrorEvent::log_line()` | stdout 单行 |

**为什么写成契约**：2026-09-11 之前这三条**全都断着**——错误只有 `Display`
散文、只 `println!` 进 stdout（文件 sink 里一个字都没有）、且 `AppEvent` 压根
没有错误变体（WS 从来不发 `error` 帧）。用户看到的界面只有「本轮失败」，
日志里什么都没有，于是投诉「后端出错无具体错误代码 / 前端无法知道错误信息」。

### 5.2 错误码表（`ErrorKind::code()`）

形态 `<stage>_<suffix>`；上游 HTTP 状态码直接进码（401 与 429 必须一眼分开）。
`hint()` 给一句**可执行**提示（如 401 → 检查 `api_key_env` 指向的环境变量在
后端进程环境里是否已设置）；`is_fatal()` 与 supervisor 的排空策略同口径。

| kind | code | fatal |
| --- | --- | --- |
| `Llm(Status 401)` | `llm_upstream_401` | 否（已生成语音继续播完） |
| `Llm(Http)` | `llm_transport` | 否 |
| `Llm(InvalidBaseUrl)` | `llm_base_url_invalid` | 否 |
| `Llm(Sse)` | `llm_sse_parse` | 否 |
| `Tts(Status 404)` | `tts_upstream_404` | 是 |
| `Decode(TruncatedPcm)` | `decode_pcm_unaligned` | 是 |
| `Backpressure` | `tts_backpressure` | 是 |
| `IncompleteTools` | `tools_incomplete` | 否 |

回归：`conversation/error_code.rs`（码/阶段/致命性/提示）、
`app_event.rs`（投影）、`web_api/tests_ws.rs`（`error` 帧形状）、
Flutter `test/ws_frame_test.dart` + `test/ui_state_tracker_test.dart`（上屏文案）。

### 5.3 HTTP 请求级日志（`web_api/dispatch.rs`）

| 条件 | 级别 | 说明 |
| --- | --- | --- |
| 状态 ≥ 500 | `error` | 真故障 |
| 状态 ≥ 400 | `warn` | 用户看得见的拒绝（含 403 安全校验：`code`/`origin`/`content_type` 都记） |
| mutating 成功（POST/PATCH/PUT/DELETE） | `info` | 「我改过什么」的审计轨迹 |
| 其它成功读取 | `debug` | GET 太密，不占默认档位 |

**不记录请求体**：设置补丁里含提示词、端点地址等用户内容，且没有任何诊断
问题需要它。设置写盘失败另在 `settings_routes` 打 `code` + `status`。

### 5.4 配置快照必须与磁盘一致

`file_watcher` 检测到**外部修改**时，除 `supervisor.reload()`（重建 client）
之外还必须刷新 `StatusContext` 快照（`refresh_from_disk`）。否则：

1. `GET /api/v1/settings` 回旧值（界面显示的和磁盘上的不是一回事）；
2. 下一次界面「保存」以**旧快照**为基准整份写回，把手改的内容**覆盖掉**。

解析失败（编辑器保存到一半）时保留旧快照并打 `warn`——不把坏配置换上界面。
回归：`web_api/tests_reload.rs::refresh_from_disk_*`。
