# BATCH-0008 — Phase 1 · lib/api/ 全量（协议与请求层）

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/api/api_client.dart | 267 | 全读：`_guard` 把网络异常统一成 `ApiException('network_error')`；`_errorFrom` 优先取 `error.code`；`sendChat/stopChat` 的 baseline 旁路「转发即丢」；`patchSettings` 是 PATCH 不是 PUT（有实测注释）✓ |
| lib/api/ws_frame.dart | 573 | 全读：9 个 type 分支 + 宽容解析（`seq` 走 `_intOrNull` 的理由写在 :412-416）；`formatWsError` 单一格式；`backoffForAttempt` 抽成纯函数 ✓；`message` 缺字段回落空串而不是整帧原文（有注释说明为什么）✓ |
| lib/api/ws_client.dart | 178 | 全读：`ensureConnected` 按 `readyState` 判活（僵尸 socket 摘除）；`_detach` 摘干净回调；`shutdown_ready` 先 close 再退避 ✓ —— **但没有任何心跳看门狗（F-0008-1）** |
| lib/api/ws_liveness.dart · ws_status.dart | 95 | 纯函数：CONNECTING/OPEN 算活，CLOSING/CLOSED 是僵尸；`isProblem` = disconnected/closed |
| lib/api/env_api.dart | 160 | 全读：**`EnvKey` 刻意没有 value 字段**（:25-27）；只有 PUT 往上走、GET 只有布尔 —— G 红线在本文件结构上成立 ✓ |
| lib/api/diagnostics_api.dart | 184 | 全读：`logs()` 不再发无效查询参数（501 事故的收口）；`AppCapabilities` **不持有**未实现能力开关（假广告已在两侧同时删除）✓ |
| lib/api/mods_api.dart | 444 | 读 20-130：`ModSettingField.secret` 服务端不回值、界面渲染密码框 ✓ |
| lib/api/settings_models.dart | 931 | 读结构 + `LlmSettingsView` 全段：`hasApiKey` 单一语义、`maxTokens` 回生效值而非 Option、`show_reasoning` 缺省 false（两侧一致）、`defaultMaxTokens=4096` 与 Rust `DEFAULT_MAX_TOKENS` **实测一致** |
| 契约核对（只读） | — | `crates/live2d-ai-desktop/src/web_api/ws/events.rs:66-120` 逐帧核对：`turn_state`/`runtime_status`/`text_delta`/`reasoning_delta`/`text_fallback` 的字段名与 Dart 侧**逐字一致**；`error` 帧由 `AppEvent::Error` 投影（rc.1 补齐，注释记录了它曾长期是空类） |
| test 抽查 | — | `ws_frame_test.dart`(529/40 例) · `settings_api_test.dart`(521/32 例) · `env_api_test.dart`(100/5 例) · `admin_api_test.dart`(396/19 例) · `ws_liveness_test.dart`(53) —— **四份都没有源码扫描式断言**，`env_api_test` 用 `MockClient` 断言真实请求（method/path/header/body）✓ |

## 发现
- **F-0008-1（P1）** `HeartbeatEvent` 被解析、被下发，但**两个消费者都丢弃**；前端**没有任何心跳看门狗** ⇒ 半开连接不可检测 ⇒ 「POST 成功但收不到回复」幽灵态复现。

## 本批核对过、不成发现的（正面记录）
- **`llm.show_reasoning` 默认 false ⇒ 前端整套「思考折叠区」不可达？** 逐层查证后**证伪**：`llm_section.dart:137` 有 `ToggleField` 真实接线，`settings_controller.dart:104/376` 进了草稿与剪枝，`settings_api_test.dart:262-293` 钉了三态。推理模型的产品开关两侧一致。
- WS 帧字段与 Rust 投影**逐字对齐**（`epoch`/`status`/`event`/`text`/`completed`/`seq`/`ts`），新增帧（`reasoning_delta`/`text_fallback`）走独立 type（旧客户端落 `UnknownWsEvent`），符合 Q 红线「只增不改」。
- `_errorFrom` 在 body 非 JSON 时会**把整个响应体当错误文案**（api_client.dart:255/261）——只在网关/代理返回 HTML 时发生，记 P3 备忘（`http_<status>` 这个合成码在后端日志里搜不到，与「拿码搜日志」的契约有微瑕）。
- `diagnostics_api.logs()` 里 `_decode(response.body)` 被调用两次（日志体可达 MAX_LINES），双解析一次；调用频率低，不成发现。
- `ws_client.connect()` 无重入保护（连调两次会丢掉前一个 socket 而不 close），但全仓只有一个调用点（main.dart:661），B 维度的「幂等性」在此不构成缺陷。

## 本批未核实
- F-0008-1 的**触发条件**（半开连接多久出现一次）需真机观察（笔记本休眠/唤醒、NAT 超时）。机制层面已确认「前端无任何超时判据」。
- 后端 `logs()` 的 `MAX_LINES` 与前端 `LogLine` 列表的渲染性能（长日志滚动）未测。
