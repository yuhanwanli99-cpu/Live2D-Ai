# 0.3.9 rc.3 A2A 搬家：接口契约（冻结，2026-10-19）

> 执行单：`docs/plans/PLAN-0.3.9-core-boundary-2026-10-19.md` §3。
> 本文件是两块并行实现的**唯一接口真源**；改接口先改本文件并同步两块。

## 1. 物理边界

- **第二仓** `/home/skystar/Live2D-Ai-mods` 的 `crates/live2d-ai-mod-a2a` = **协议核**：
  agent card、JSON-RPC 解析 / dispatch / 拒绝表、`message/send`、`tasks/get`、
  进程内任务表（上限 32）、上游失败脱敏。**不依赖 tiny_http / 宿主 Context / 密钥**。
- **主仓** `/home/skystar/Live2D-Ai` 的 `crates/live2d-ai-desktop/src/web_api/a2a_gate.rs`
  （新文件；`a2a_routes.rs` 及其子目录 / 测试删除）= **宿主门**：loopback + Origin +
  Content-Type 校验、「Mod 停用则不调用」、把协议核结果翻成 tiny_http 响应、
  注入上游 chat 闭包（读 `[llm]` + `secrets::lookup` + `OpenAiClient`）。
- 线格式与 0.3.8 **逐字节一致**：方法名、错误码、字段、状态码都不许变。

## 2. 协议核冻结接口（crate `live2d-ai-mod-a2a`，只依赖 serde_json）

```rust
pub const A2A_PATH: &str = "/api/v1/a2a";
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";
pub const PROTOCOL_VERSION: &str = "0.3.0";
pub const MOD_ID: &str = "a2a";
pub const TASK_STORE_CAP: usize = 32;

/// 请求时生成 agent card（version = 注入的宿主版本；url 只指向本服务 A2A_PATH）。
pub fn agent_card(version: &str, port: u16) -> serde_json::Value;

/// 上游对话：一段文本 → 一段正文；Err = **已脱敏**的失败描述。
pub type ChatFn<'a> = dyn Fn(&str) -> Result<String, String> + 'a;

/// 日志级别（协议核不直接依赖 tracing 的宿主 target）。
pub enum LogLevel { Info, Warn }

/// 协议核：一次请求 → 响应 JSON + HTTP 状态码。任务表属于实例。
pub struct A2aCore { /* Mutex<TaskStore> 等 */ }

impl A2aCore {
    /// `version` = 宿主版本串（env!("CARGO_PKG_VERSION") 由宿主传入）。
    pub fn new(version: String) -> Self;

    /// GET card / POST JSON-RPC 的统一入口（**只做协议**，不校验 Origin/CT）。
    /// - `path == AGENT_CARD_PATH`：返回 card；
    /// - `path == A2A_PATH`：解析 + dispatch；
    /// - 其它 path：返回 404 形状（宿主不会调用）。
    pub fn handle(
        &self,
        method: &str,          // "GET" / "POST"（其它 → 405 响应）
        path: &str,
        body: &str,
        port: u16,             // card 的 url 用
        chat: &ChatFn<'_>,
        log: &LogFn<'_>,
    ) -> A2aResponse;
}

pub struct A2aResponse {
    pub status: u16,                  // 200 / 404 / 405
    pub json: serde_json::Value,      // 响应体（宿主序列化，Content-Type 由宿主加）
    pub allow_get: bool,              // 405 时是否补 Allow: GET（card）
}

/// 宿主日志注入：level + 一行文本（宿主决定 target / 结构化字段）。
pub type LogFn<'a> = dyn Fn(LogLevel, &str) + 'a;
```

### 2.1 协议核内部表（从主仓 0.3.8 逐字迁入，行为不变）

- 解析：非 JSON → `-32700`；非对象 / jsonrpc != "2.0" / 缺 method → `-32600`；
  id 非 string|number|null → `-32600`。
- 方法表：`message/send`、`tasks/get`；
  `message/stream` / `tasks/cancel` / `tasks/resubscribe` / `tasks/list` /
  `agent/getAuthenticatedExtendedCard` → `-32004`；
  `tasks/pushNotificationConfig/*` → `-32003`；其它 → `-32601`。
- `message/send`：抽 user text（多段 text 用 \n 拼；空 / 非 text part → `-32602`），
  调 chat；chat Err → `-32603` + 同一条已脱敏描述；成功 → completed Task
  （`status.message.parts[0].text` 与 `artifacts[0].parts[0].text` **逐字相同**），
  存进任务表（FIFO 淘汰）。
- `tasks/get`：缺 params.id → `-32602`；未知 id → `-32001`；命中 → 同一份 Task。
- `sanitize_error` 保留现有实现（Bearer / sk- 脱敏 + 300 字符截断）。
- **不打上游**的范围外方法：dispatch 层直接拒绝，chat 一次都不调（回归断言）。

## 3. 宿主门职责（主仓）

1. 路径匹配 `AGENT_CARD_PATH` / `A2A_PATH`，且 `ModRegistry` 里 `id == "a2a"` 且
   `enabled == true`；否则（停用）**不调协议核、不打上游**，回 `403` + JSON
   `{"error":{"code":"mod_disabled","message":...}}`（与另一个 Mod 停用端点同款；
   两处路径同一套，写进测试）。工厂数 6 → 7，`mod_count_is_six` 改名 `mod_count_is_seven`。
2. loopback 绑定即边界；GET card 不套 mutating 校验；POST 走既有
   `is_json_content_type`（否则 415）+ `is_allowed_origin`（否则既有
   `mutating_check_error_response`）。
3. 调 `A2aCore::handle`，把 `A2aResponse.status` / `json` 翻成 tiny_http 响应
   （Content-Type: application/json; charset=utf-8 + Cache-Control: no-cache）。
4. POST 派生工作线程（上游要等 LLM；HTTP 循环单线程）。
5. 上游闭包 = 现有 `a2a_llm` 逻辑搬家（读设置快照 `[llm]`、`secrets::lookup`、
   `OpenAiClient`、错误只回稳定码）。**密钥值只在宿主闭包里用一次，不进协议核、
   不进响应、不进日志。**

## 4. 停止条件（两块共同）

- 第二仓 `cargo test`（workspace）= 全绿；`fmt --check` / `clippy -D warnings` 全绿。
- 主仓 `cargo test --workspace --all-targets` 全绿（含 A2A 门测试：停用 403 两路径、
  启用线格式、范围外方法不打上游）。
- 线格式回归：把 0.3.8 的 `a2a_routes_tests_*.rs` 断言逐条迁到新位置（协议断言迁进
  第二仓 crate 测试；宿主门断言迁进主仓 `a2a_gate` 测试）。
