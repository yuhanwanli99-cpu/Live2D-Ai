//! Web API 入口（节点 D2 接线，2026-08-28）。
//!
//! 责任：
//! - [`start_server`]：在 `127.0.0.1:<port>` 起 tiny_http，循环 accept；
//! - [`route`]：根据 method+path 派发到具体 handler（**纯函数**，便于单测）；
//! - [`ServerContext`]：跨 handler 共享 [`StatusContext`]、config 路径、
//!   可选 [`SupervisorHandle`] 与 [`ws::Broadcaster`]。
//!
//! 安全（P0 红线）：
//! - 监听仅 loopback（`127.0.0.1`，**不**绑 `0.0.0.0` / `::`）；
//! - **不**回 `Access-Control-Allow-Origin`（默认拒绝跨域）；
//! - GET 响应走脱敏视图，**不**含密钥；
//! - handler 内不调用 `Command::new`（P0-4 审计由 `forbidden_paths.rs` 保证）。
//! - **D-P0B 2026-08-29**：WS 升级前做 `Origin` 同源校验；HTTP mutating
//!   端点 dispatch 前做 `Origin` + `Content-Type` 校验。详见 [`security`] 模块。
//!
//! 路径派发表（method + path → 路由 ID）：
//! - `GET /` 或 `GET /index.html` → **302 到 `/app/`**（Flutter 前端入口；
//!   原生 JS 前端已于 2026-09-11 删除，见 `flutter_app::root_redirect`）
//! - `GET /app*` → Flutter Web 产物（`shell/flutter/build/web/`）
//! - `GET /api/v1/app/capabilities` → `app.capabilities`
//! - `GET /api/v1/app/status` → `app.status`
//! - `GET /api/v1/settings` → `settings.get`
//! - `PATCH /api/v1/settings` → `settings.patch`
//! - `POST /api/v1/settings/test/llm` → `settings.test_llm`
//! - `POST /api/v1/settings/test/tts` → `settings.test_tts`
//! - `POST /api/v1/chat` → `chat.post`（W2：HTTP 对话入口）
//! - `POST /api/v1/chat/stop` → `chat.stop`（W2：停止当前轮）
//! - `GET /api/v1/logs`（dev_mode=true） → `logs.get`
//! - `GET /api/v1/logs/levels`（dev_mode=true） → `logs.levels`
//! - `GET / POST / PATCH / DELETE /api/v1/models*` → `models`（D3 落地，2026-08-28）
//! - `GET /ws/runtime` / `GET /ws/state` → WS 升级（独立处理，不经 dispatch）
//! - 其它（scripts）→ `not_implemented`（501；D4 填）
//!
//! 2026-09-11 用户裁决：「LLM 不暴露任何工具，只做对话」+「从前后端抹去相关
//! 字段不做实现」——D4 的动作命令端点（`GET /api/v1/commands` 与
//! `POST /api/v1/commands/{id}/invoke`）及其注册表已整体删除，
//! `RouteId::Commands` 不再存在；`/api/v1/commands*` 现在落回
//! `RouteId::NotImplemented`（501），与其它未实现前缀一致。

use std::sync::Arc;

use tiny_http::{Method, Server};

use live2d_ai_runtime::AppSettings;

use crate::supervisor::SupervisorHandle;
use crate::web_api::app_routes::{CapabilitiesContext, StatusContext};

/// Web API 子模块。
pub mod app_routes;
pub mod chat_routes;
pub mod cli_entry;
/// HTTP 派发（method+path → RouteId → handler；D-P0B 安全校验 + D-P0C
/// 动态装配 supervisor）。拆分承载：保持 `mod.rs` ≤ 500。
pub mod dispatch;
pub mod dto;
/// 外部文本注入端点（节点 E5-T3）：`POST /api/v1/external/chat → say →
/// 主链路`。静态前置路由（见 `run_request_loop` E5-T3 判定点）。
pub mod external_routes;
/// Flutter Web 前端托管（`/app` 与 `/app/*`；2026-09-10 点火轮）。
pub mod flutter_app;
pub mod log_routes;
/// **唯一模型根**（rc.2，2026-09-12）：静态 `/models/*` 与模型库 registry /
/// import 共用的那一个根。两个根会导致「激活了但没换皮」，见模块头注。
pub(crate) mod model_root;
/// 模型资产路由（D3，2026-08-28）：`/api/v1/models*` 端点 + registry
/// + 路径安全 + 原子写回。子模块化（`models_routes::handlers` 等），
/// 本文件仅声明入口。
pub mod models_routes;
/// Mod 管理 HTTP API（节点 E5 补充，2026-08-30）：
/// `GET /api/v1/mods` 与 `POST /api/v1/mods/{id}/{action}`。
pub mod mods_routes;
/// 通用 JSON 错误响应 helper（D-P0B 拆分承载，2026-08-29）。
pub(crate) mod responses;
/// 本地 Web 安全边界（D-P0B，2026-08-29）。
pub mod security;
pub mod settings_routes;
/// Supervisor 运行时槽位（D-P0C 拆分承载）。
pub mod supervisor_slot;
#[cfg(test)]
mod tests_dev_mode;
#[cfg(test)]
mod tests_mod;
/// D-P0C（2026-08-29）首次配置闭环：动态装配 supervisor + apply_status。
#[cfg(test)]
mod tests_p0c;
#[cfg(test)]
mod tests_reload;
/// 安全边界单元测试（D-P0B，2026-08-29）。
#[cfg(test)]
mod tests_security;
/// 安全边界 dispatch 端到端测试（D-P0B，2026-08-29）。
#[cfg(test)]
mod tests_security_e2e;
#[cfg(test)]
mod tests_ws;
/// Web 静态资产：/models/* 模型资产 + /render* WASM 渲染页（节点 E E2）。
pub mod wasm_assets;
/// WebSocket 桥（W2 任务，2026-08-29）。
pub mod ws;

/// 配置文件热重载文件监听器（2026-08-31）。
pub mod file_watcher;

/// 路由 ID（method+path 解析结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteId {
    /// `GET /api/v1/app/capabilities`
    AppCapabilities,
    /// `GET /api/v1/app/status`
    AppStatus,
    /// `GET /api/v1/settings`
    SettingsGet,
    /// `PATCH /api/v1/settings`
    SettingsPatch,
    /// `POST /api/v1/settings/test/llm`
    SettingsTestLlm,
    /// `POST /api/v1/settings/test/tts`
    SettingsTestTts,
    /// `POST /api/v1/chat`（W2 任务）
    ChatPost,
    /// `POST /api/v1/chat/stop`（W2 任务）
    ChatStop,
    /// `GET /api/v1/logs`（dev_mode 门控；W3 任务）。
    LogsGet,
    /// `GET /api/v1/logs/levels`（dev_mode 门控；W3 任务）。
    LogsLevels,
    /// `GET /api/v1/models`、`GET /api/v1/models/{id}`、`POST
    /// /api/v1/models/import`、`POST /api/v1/models/{id}/activate`、
    /// `PATCH /api/v1/models/{id}/display`、`DELETE /api/v1/models/{id}`
    /// —— 全部统一到 `Models`，由 `models_routes::dispatch` 内部按子路由
    /// 派发（D3 2026-08-28）。
    Models,
    /// 已冻结但本批未实现（D3/D5 填充）。
    NotImplemented,
    /// 路径完全未匹配。
    NotFound,
}

impl RouteId {
    /// 路由稳定 ID 字符串（用于错误体 `endpoint` 字段与日志）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppCapabilities => "app.capabilities",
            Self::AppStatus => "app.status",
            Self::SettingsGet => "settings.get",
            Self::SettingsPatch => "settings.patch",
            Self::SettingsTestLlm => "settings.test_llm",
            Self::SettingsTestTts => "settings.test_tts",
            Self::ChatPost => "chat.post",
            Self::ChatStop => "chat.stop",
            Self::LogsGet => "logs.get",
            Self::LogsLevels => "logs.levels",
            Self::Models => "models",
            Self::NotImplemented => "not_implemented",
            Self::NotFound => "not_found",
        }
    }
}

/// 剥掉 URL 的查询串（`tiny_http` 的 `request.url()` **带** `?...`）。
///
/// # 为什么必须有（2026-09-11 修）
///
/// 全仓库的路由（[`match_route`]）都是**精确路径匹配**，而 `request.url()`
/// 把查询串也带进来了 —— 于是 `GET /api/v1/logs?limit=200` 匹配不上
/// `/api/v1/logs`，一路掉到 `NotImplemented`（**501**）。
/// Flutter 前端恰好就是这么调的（诊断面板取日志），所以那个面板在成品里
/// **从来没成功过**：它拿到的不是日志，是一个 501。
///
/// 路径匹配**不该**看见查询串——这是 HTTP 的常识，不是特例。
/// 所以剥在**请求循环的入口**（一处），而不是每遇到一个带参端点就补一次。
///
/// 目前没有任何端点读查询参数（要限流/过滤都在服务端内置常量里），
/// 所以剥掉不会丢信息；哪天要支持查询参数，应该在 `dispatch` 里显式传
/// 解析好的 `query`，而不是让路径匹配去猜。
pub fn strip_query(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// 路径匹配（**纯函数**，便于单测）。
///
/// 不读 body、不查 method 是否允许——method 校验在外层做。
///
/// WS 端点（`/ws/runtime`、`/ws/state`）由 `start_server` 单独走升级路径，
/// **不**出现在 `match_route` 结果里（避免被 `dispatch` 误派发）。
pub fn match_route(method: &Method, path: &str) -> RouteId {
    if path == "/api/v1/app/capabilities" && *method == Method::Get {
        RouteId::AppCapabilities
    } else if path == "/api/v1/app/status" && *method == Method::Get {
        RouteId::AppStatus
    } else if path == "/api/v1/settings" {
        if *method == Method::Get {
            RouteId::SettingsGet
        } else if *method == Method::Patch {
            RouteId::SettingsPatch
        } else {
            RouteId::NotFound
        }
    } else if path == "/api/v1/settings/test/llm" && *method == Method::Post {
        RouteId::SettingsTestLlm
    } else if path == "/api/v1/settings/test/tts" && *method == Method::Post {
        RouteId::SettingsTestTts
    } else if path == "/api/v1/chat" {
        // method 校验放在 handler 内：ChatPost 总会路由到 handle_chat，
        // 由 handler 自己判 method 返 405。这样 `GET /api/v1/chat` 不会
        // 落到 NotImplemented 误报"未实现"。
        RouteId::ChatPost
    } else if path == "/api/v1/chat/stop" {
        RouteId::ChatStop
    } else if path == "/api/v1/logs" && *method == Method::Get {
        RouteId::LogsGet
    } else if path == "/api/v1/logs/levels" && *method == Method::Get {
        RouteId::LogsLevels
    } else if path == "/api/v1/models"
        || path == "/api/v1/models/import"
        || (path.starts_with("/api/v1/models/") && is_models_subpath(path))
    {
        // D3（2026-08-28）：`/api/v1/models*` 系列统一到 `RouteId::Models`，
        // 由 `models_routes::dispatch` 内部按子路由细分（List/Import/Get/
        // Delete/Activate/Display）。`is_models_subpath` 校验路径形态合法
        // （避免任意字符串被接受为 id）。
        RouteId::Models
    } else if path.starts_with("/api/v1/") {
        // 已冻结但本批未实现（D3/D5）。
        RouteId::NotImplemented
    } else {
        RouteId::NotFound
    }
}

/// 路径形态合法性校验（仅用于路由表匹配；handler 内再走 `match_model_route`）。
///
/// 合法形态：
/// - `/api/v1/models/{id}` — id 不含 `/`、不含 `..`、不含 `\\`；
/// - `/api/v1/models/{id}/activate` — 仅 POST 允许（method 在 handler 内判）；
/// - `/api/v1/models/{id}/display` — 仅 PATCH 允许（method 在 handler 内判）。
fn is_models_subpath(path: &str) -> bool {
    let rest = path.strip_prefix("/api/v1/models/").unwrap_or("");
    if rest.is_empty() {
        return false;
    }
    let mut parts = rest.split('/');
    let id = parts.next().unwrap_or("");
    if id.is_empty() || id.contains("..") || id.contains('\\') {
        return false;
    }
    match parts.next() {
        None => true,
        Some("activate") => parts.next().is_none(),
        Some("display") => parts.next().is_none(),
        _ => false,
    }
}

/// 跨 handler 共享上下文。
#[derive(Clone)]
pub struct ServerContext {
    /// settings 视图与写盘路径。
    pub status_ctx: Arc<StatusContext>,
    /// capabilities 上下文（保留 hooks：D2 引入 uptime / 启动时戳会启用）。
    #[allow(dead_code)]
    pub capabilities_ctx: Arc<CapabilitiesContext>,
    /// **可更新的运行时 supervisor 槽位**（D-P0C 2026-08-29）。
    ///
    /// - 类型：`Arc<RwLock<Option<Arc<SupervisorHandle>>>>`；
    /// - **None** 槽位 ≠ 进程无能力：表示「当前未持有 supervisor 句柄」
    ///   （首次启动无配置 / 用户尚未填表 / 配置不完整无法创建）；
    /// - **Some(arc)** 槽位表示「supervisor 已就绪」——
    ///   `POST /api/v1/chat` 走 arc，`PATCH` 写盘后走 `arc.reload()`。
    ///
    /// 槽位 API 见 [`supervisor_slot::SupervisorSlot`]（D-P0C 拆分承载）。
    pub supervisor_slot: supervisor_slot::SupervisorSlot,
    /// WS 广播器：把 supervisor 线程发来的 [`AppEvent`] 经 [`ws::app_event_to_ws_frame`]
    /// 投影后转发给所有 `/ws/runtime` 与 `/ws/state` 订阅者。**始终**存在
    /// （空广播器 = 0 订阅者），handler 可无条件调用 `ctx.broadcaster.broadcast(...)`。
    pub broadcaster: crate::web_api::ws::Broadcaster,
    /// 安全上下文（D-P0B 2026-08-29）：dispatch / WS 升级读取决定是否放行。
    /// 默认 = `SecurityContext::new(0, false)`；`start_server` 装配时由
    /// `cli_entry` 用真实 port 覆盖。
    pub security: crate::web_api::security::SecurityContext,
    /// 模型 registry 共享句柄（D3 2026-08-28）。
    /// 启动时 `ServerContext::new` 调用 [`models_routes::default_store`]
    /// 从磁盘加载（缺失 = 空 registry，损坏 = stderr 警告 + 空 registry）。
    pub models: crate::web_api::models_routes::ModelStore,
    /// Mod registry 共享句柄（节点 E5，2026-08-30）。
    /// 启动时 `cli_entry` 装配真实 `ModRegistry`（静态编译的
    /// `AVAILABLE_MOD_FACTORIES` + manifest）；其它路径（测试 / 缺省）
    /// 用空 registry 占位。handler 经 `lock()` 调用 enable/disable/restart/list。
    pub mod_registry: std::sync::Arc<std::sync::Mutex<crate::mod_registry::ModRegistry>>,
}

impl ServerContext {
    /// 构造 server 上下文（启动时调用一次）。
    ///
    /// `dev_mode_override` = `Some(b)` 时**强制**用 `b`（CLI `--dev-mode` 路径）；
    /// `None` 时从 `settings.dev_mode` 派生。安全上下文默认严格模式。
    /// supervisor 槽位初始 = 空 `RwLock<None>`（首次配置闭环场景）。
    pub fn new(
        settings: AppSettings,
        config_path: String,
        dev_mode_override: Option<bool>,
    ) -> Self {
        Self {
            status_ctx: Arc::new(StatusContext::new(settings, config_path, dev_mode_override)),
            capabilities_ctx: Arc::new(CapabilitiesContext::new()),
            supervisor_slot: supervisor_slot::SupervisorSlot::new(),
            broadcaster: crate::web_api::ws::Broadcaster::new(),
            security: crate::web_api::security::SecurityContext::new(0, false),
            models: crate::web_api::models_routes::default_store(),
            mod_registry: Arc::new(std::sync::Mutex::new(
                crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
            )),
        }
    }

    /// 注入 supervisor 句柄（链式 builder；`web` 模式启动时若配置可解析
    /// 则调用）。直接写槽位（写锁短暂持锁）。
    pub fn with_supervisor(self, supervisor: Arc<SupervisorHandle>) -> Self {
        self.supervisor_slot.set(supervisor);
        self
    }

    /// 注入安全上下文（链式 builder）。`cli_entry` 启动时调用，传入真实
    /// 端口 + 是否允许无 Origin 工具客户端（`LIVE2D_AI_ALLOW_NO_ORIGIN`）。
    #[allow(dead_code)] // 由 cli_entry::run_web_mode 装配时调用。
    pub fn with_security(mut self, sec: crate::web_api::security::SecurityContext) -> Self {
        self.security = sec;
        self
    }

    /// 注入 Mod registry（链式 builder）。`cli_entry` 在 web 启动时调用，
    /// 传入静态编译的 `AVAILABLE_MOD_FACTORIES` 装配后的 `ModRegistry`。
    pub fn with_mod_registry(mut self, registry: crate::mod_registry::ModRegistry) -> Self {
        self.mod_registry = Arc::new(std::sync::Mutex::new(registry));
        self
    }

    /// 替换当前 settings（PATCH 写回成功后调用）。D2 起 D3 模型激活也用。
    #[allow(dead_code)]
    pub fn swap_settings(&self, new: AppSettings) {
        self.status_ctx.swap_settings(new);
    }

    /// 触发 supervisor 热重载（仅当槽位非空）。非阻塞幂等。
    ///
    /// D-P0C 语义：槽位空时**不再**静默 no-op——调用方（dispatch）会看
    /// 槽位状态决定 `apply_status`（`NoSupervisor` / 动态创建路径）。
    #[allow(dead_code)] // 保留 API：未来 chat 模式 / egui 路径仍可调用
    pub fn request_reload(&self) {
        if let Some(s) = self.supervisor_slot.try_get() {
            s.reload();
        }
    }

    /// **新**（D-P0C）：从槽位借出当前 supervisor 句柄的 Arc 克隆。
    pub fn try_get_supervisor(&self) -> Option<Arc<SupervisorHandle>> {
        self.supervisor_slot.try_get()
    }

    /// **新**（D-P0C）：在 PATCH 200 写盘成功后由 dispatch 调用——
    /// 试图动态装配 supervisor。详见 [`supervisor_slot::ensure_after_patch`]。
    pub fn ensure_supervisor_after_patch(&self) -> ApplyStatus {
        self.supervisor_slot.ensure_after_patch(
            &self.status_ctx.config_path,
            self.broadcaster.clone(),
            &self.status_ctx,
            self.mod_registry.clone(),
        )
    }

    /// **新**（D-P0C）：cli_entry 退出时回收 supervisor 线程。
    pub fn take_supervisor_for_reclaim(&self) -> Option<Arc<SupervisorHandle>> {
        self.supervisor_slot.take()
    }
}

/// PATCH 写盘后 supervisor 装载/重载状态在 dispatch 层转发（详见
/// [`crate::web_api::dto::ApplyStatus`]）。
pub use crate::web_api::dto::ApplyStatus;

/// 启动 HTTP server（阻塞循环；线程在 caller）。返回 `ListenAddr` 便于测试。
///
/// WS 端点（`/ws/runtime`、`/ws/state`）由 `is_ws_path` 命中后**直接**走升级
/// 路径：`req.upgrade()` 把控制权交给 `tungstenite::accept` 与 `handle_ws_request`，
/// 不再回到标准 dispatch 循环（避免 dispatch 误派发到 `NotImplemented`）。
///
/// 2026-08-29 拆分：实际循环抽到 [`run_request_loop`]；本函数仅做"绑定 +
/// 启动 + 打印日志"三件事。这样 e2e 测试可以直接传一个预绑定的 server
/// （port 0 → 拿到 OS 分配端口）跑循环，避免"必须等 server 退出才知
/// 端口"的麻烦。
pub fn start_server(ctx: ServerContext, port: u16) -> std::io::Result<tiny_http::ListenAddr> {
    let addr = format!("127.0.0.1:{port}");
    #[allow(clippy::io_other_error)]
    let server = Server::http(&addr).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("绑定 {addr} 失败: {e}"))
    })?;
    let bound = server.server_addr();
    eprintln!("web-api: 已监听 127.0.0.1:{port}");

    // **D-P0B**：把监听端口注入 ctx.security；`allow_no_origin` 走环境变量
    // `LIVE2D_AI_ALLOW_NO_ORIGIN`（caller 已在 ctx 里设的 flag 保留）。
    // 这样 cli_entry 不用动；`with_security` 仍可显式覆盖。
    let allow_no_origin =
        ctx.security.allow_no_origin || crate::web_api::security::read_allow_no_origin_from_env();
    let ctx = ServerContext {
        security: crate::web_api::security::SecurityContext::new(port, allow_no_origin),
        ..ctx
    };

    run_request_loop(server, ctx);
    Ok(bound)
}

/// 在已绑定的 `tiny_http::Server` 上跑 accept 循环（阻塞）。
///
/// **D5 修复**：WS 升级后立即派生 `"ws-conn"` 线程，主循环不阻塞。
/// **D-P0B**：WS 升级前由 `check_request_origin` 校验 Origin；HTTP
/// mutating 端点 dispatch 前在 [`dispatch_with_security`] 做 Origin + CT
/// 校验。
pub fn run_request_loop(server: tiny_http::Server, ctx: ServerContext) {
    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        // **带查询串的原始 URL**（日志用）与**剥掉查询串的路径**（路由用）。
        // 两者必须分开：路由是精确匹配，看见 `?limit=200` 就会掉进 501。
        let raw_url = request.url().to_string();
        let path = strip_query(&raw_url).to_string();
        if crate::web_api::ws::is_ws_path(&path) {
            if method == Method::Get {
                // **D-P0B**：Origin 校验**先于** handle_ws_request——这样
                // 403 响应能落盘到客户端（handle_ws_request 内部 consume
                // request 后 caller 无法再 respond）。
                if let Err(resp) =
                    crate::web_api::ws::check_request_origin(&request, ctx.security.port)
                {
                    let _ = request.respond(resp);
                    continue;
                }
                let bc = ctx.broadcaster.clone();
                if let Err(_resp) = crate::web_api::ws::handle_ws_request(request, bc) {
                    // 101 已发 + WS 立即关；客户端看到 "connection closed"。
                    // Origin 拒绝 403 路径不归这里（已在上方 respond）。
                    tracing::debug!(
                        path = %path,
                        "WS handle 返回 Err（426 协议不兼容 / 503 超额）；101 已发，stream 已 drop"
                    );
                }
            } else {
                let resp = crate::web_api::ws::ws_method_or_path_error(&method, &path);
                let _ = request.respond(resp);
            }
            continue;
        }
        // 节点 E E2：Web 静态资产（/models/* 模型资产 + /render* WASM 渲染页）。
        // 先于 dispatch 处理（纯读盘静态，非 API；GET 无 body 不 consume）。
        if let Some(resp) = crate::web_api::wasm_assets::handle_assets(&method, &path) {
            let _ = request.respond(resp);
            continue;
        }
        // 点火轮（2026-09-10）：Flutter Web 前端（`/app` 与 `/app/*`）。
        // 与 `/render` 同源是本轮闭环的前提（Flutter 侧 iframe 的 postMessage 校验 origin）。
        if let Some(resp) = crate::web_api::flutter_app::handle_flutter_app(&method, &path) {
            let _ = request.respond(resp);
            continue;
        }
        // HTTP：抽 (origin, content_type) → dispatch_with_security。
        let headers = request.headers().to_vec();
        let (origin, content_type) = crate::web_api::security::extract_origin_and_ct(&headers);
        let body_str = read_body_from_request(&mut request);
        // 节点 E5-T3：外部文本注入端点（POST /api/v1/external/chat → say →
        // 主链路）。前置于 dispatch_with_security，自身完成 loopback + json
        // 校验（详见 external_routes 头注）。
        if let Some(resp) = crate::web_api::external_routes::handle_external_chat(
            &ctx,
            &method,
            &path,
            &body_str,
            origin.as_deref(),
            content_type.as_deref(),
        ) {
            let _ = request.respond(resp);
            continue;
        }
        // 节点 E5-T3（2026-08-30）：Mod 管理路由（GET /api/v1/mods +
        // POST /api/v1/mods/{id}/{action}）。前置于 dispatch：自身完成
        // loopback + json 校验（GET 不校验 Origin，POST 复用 security 判定）。
        if let Some(resp) = crate::web_api::mods_routes::handle_mods_route(
            &ctx,
            &method,
            &path,
            &body_str,
            origin.as_deref(),
            content_type.as_deref(),
        ) {
            let _ = request.respond(resp);
            continue;
        }

        let response = dispatch_with_security(
            &ctx,
            &method,
            &path,
            &body_str,
            origin.as_deref(),
            content_type.as_deref(),
        );
        let _ = request.respond(response);
    }
}

/// 派发（保持 mod.rs 简洁：实际实现见 [`dispatch`] 模块）。
///
/// 旧 API 路径：`web_api::dispatch` / `web_api::dispatch_with_security`
/// 仍可直接调用（re-export）。
#[allow(unused_imports)]
pub use crate::web_api::dispatch::{dispatch, dispatch_with_security};

/// 从 tiny_http request 读 body 字符串（最多 1 MiB；超出截断）。
fn read_body_from_request(request: &mut tiny_http::Request) -> String {
    use std::io::Read;
    const MAX_BYTES: u64 = 1024 * 1024;
    let mut buf = Vec::with_capacity(512);
    let mut limited = request.as_reader().take(MAX_BYTES);
    let _ = limited.read_to_end(&mut buf);
    String::from_utf8(buf).unwrap_or_default()
}

// 错误响应 helper 已拆到 [`responses`] 模块（保持 mod.rs ≤500）。
// 派发函数主体已拆到 [`dispatch`] 模块（D-P0C 2026-08-29）。
//
// 内联测试块已拆到独立 `tests_mod.rs` / `tests_html.rs`（与既有
// `tests_dev_mode.rs` / `tests_reload.rs` 同模式），保持主体可扩展。
