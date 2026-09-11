//! 本地 Web 安全边界（D-P0B，2026-08-29）。
//!
//! 责任：
//! - `check_ws_origin`：WS 升级前对 `Origin` 头做同源校验（P0-2 复审）。
//! - `check_mutating_request`：HTTP mutating 端点（PATCH /settings、POST
//!   /chat、POST /chat/stop、POST /settings/test/*）在 dispatch 前做
//!   `Origin` + `Content-Type` 校验（P0-3 复审）。
//!
//! 裁决（与 `docs/plans/node-d-api-contract-2026-08-28.md` §2 P0-3 复审一致）：
//! - **合法 Origin**：`http://127.0.0.1:<port>` 或 `http://localhost:<port>`
//!   且端口必须等于当前 server 监听端口。
//! - **无 Origin 客户端**（curl / python 直连等非浏览器）：默认**拒绝**
//!   403，除非显式开启 `LIVE2D_AI_ALLOW_NO_ORIGIN=1` 环境变量或 CLI flag
//!   `--allow-no-origin`（给工具客户端留口，但**不**是默认）。
//! - **mutating Content-Type**：必须为 `application/json` 或
//!   `application/json; charset=...`（防简单表单 CSRF：浏览器 `<form>`
//!   默认 `text/plain` / `application/x-www-form-urlencoded`，二者均被拒）。
//! - **GET / 只读端点**：`Origin` 校验**不**强制（GET 不改状态；前端
//!   fetch / 同源 script 可豁免）。
//! - **无 body mutating**（POST /chat/stop）：Content-Type 校验**宽容**为
//!   `application/json` 缺省放行（`/chat/stop` 无 body，但前端可发
//!   `Content-Type: application/json`；v1 不强制）。
//!
//! 设计：
//! - 全部函数为 **纯函数**（无 IO / 无状态）；可单测。
//! - 错误枚举 [`MutatingCheckError`] 三态：缺 Origin / Origin 不在白名单 /
//!   Content-Type 非法。
//! - 头部取值用 `Option<&str>` 即可，**不**依赖 `tiny_http::Request` 类型
//!   ——这样 `run_request_loop` 解析头时只抽两个字段（Origin / Content-Type），
//!   减少与 tiny_http 的耦合。
//!
//! 不变量：
//! - 这些函数**不**做 `request.method()` 判断；调用方（dispatch）先看
//!   `match_route` 决定 route 是不是 mutating，再决定是否调本模块。
//!
//! 单元测试搬到 `tests_security.rs`（同 ws.rs / tests_ws.rs 拆分模式）
//! ——保持本文件 ≤500。

use tiny_http::{Header, Response, StatusCode};

use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// 安全上下文：跨 dispatch 与 WS 升级共享的「同源白名单 + 工具客户端开关」。
///
/// `port` 必须是 server 实际绑定的 loopback 端口（来自 [`start_server`] 的
/// `port` 参数；由 `run_request_loop` / `handle_ws_request` 透传）。
/// `allow_no_origin` = `true` 时无 Origin 的 mutating 请求**也**放行（仅
/// 工具客户端 / 脚本场景；浏览器 fetch 一定带 Origin）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityContext {
    /// server 监听端口（白名单 Origin 中的 port 必须等于此值）。
    pub port: u16,
    /// `true` = 放行无 Origin 的 mutating 请求（仅工具客户端）。
    pub allow_no_origin: bool,
}

impl SecurityContext {
    /// 构造安全上下文。
    pub const fn new(port: u16, allow_no_origin: bool) -> Self {
        Self {
            port,
            allow_no_origin,
        }
    }
}

/// 解析环境变量 `LIVE2D_AI_ALLOW_NO_ORIGIN=1` / `true`（其它值视为 false）。
///
/// 启动时由 [`cli_entry`] 调用一次。测试可直接调 [`is_truthy_env_value`]
/// 验证子函数（避开 env 串行问题）。
#[allow(dead_code)] // 由 cli_entry 启动时调用；测试覆盖子函数。
pub fn read_allow_no_origin_from_env() -> bool {
    is_truthy_env_value(&std::env::var("LIVE2D_AI_ALLOW_NO_ORIGIN").unwrap_or_default())
}

/// 内部辅助：判定环境变量值是否为 truthy（`1` / `true` / `yes`，大小写不敏感）。
pub fn is_truthy_env_value(v: &str) -> bool {
    let t = v.trim();
    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
}

/// WS 升级前的 Origin 校验（**纯函数**）。
///
/// - `origin` = `Some(s)`：`s` 必须形如 `http://127.0.0.1:<port>` 或
///   `http://localhost:<port>` 且端口等于 `port`；
/// - `origin` = `None`：**拒绝**（返回 `false`；不区分 dev flag，WS 升级
///   路径不走 `allow_no_origin`——浏览器 WS 一定带 Origin，工具客户端
///   用 `--allow-no-origin` + HTTP 即可，WS 不开此门）。
///
/// **为何 WS 路径不开 dev flag**：浏览器永远带 Origin；不带 Origin 的
/// WS 客户端**几乎总是**恶意探测或脚本。HTTP mutating 路径允许 dev flag
/// 是因为很多脚本/CLI 工具（curl）习惯发无 Origin POST（浏览器不存在
/// 这种场景），但 WS 场景下"无 Origin 客户端 = 异常"。
#[allow(dead_code)] // 由 ws::handle_ws_request 调用；当前未在 mod.rs 装配。
pub fn check_ws_origin(origin: Option<&str>, port: u16) -> bool {
    let Some(o) = origin else {
        return false;
    };
    is_same_loopback_origin(o, port)
}

/// HTTP mutating 端点校验（**纯函数**）。
///
/// - `origin` = `Some(s)`：必须通过 [`check_ws_origin`]（同源白名单）；
/// - `origin` = `None`：当 `ctx.allow_no_origin` 时放行；否则拒；
/// - `content_type` = `Some(ct)`：必须为 `application/json`（前缀匹配，
///   `application/json; charset=utf-8` 也算通过）；
/// - `content_type` = `None`：根据 `body_present` 区分：
///   - `body_present = true`（PATCH/POST 带 body）→ 拒（防 text/plain CSRF）；
///   - `body_present = false`（无 body mutating，如 `/chat/stop`）→ 放行
///     （前端不强制 Content-Type；后端 handler 自身会拒空 body 错误）。
///
/// **为何无 body 宽容**：浏览器 fetch 即便发 `Content-Length: 0` 也常带
/// `Content-Type: text/plain;charset=UTF-8`（form 默认），而 `/chat/stop`
/// 这种"无 body 命令"在后端 handler 内部已用 `body.is_empty()` 兜底。
/// 因此这里**只**在 body 真存在时严格校验 CT。
///
/// **工具客户端特殊路径**：当 `origin=None` 且 `ctx.allow_no_origin=true`
/// 时（脚本 / CLI 工具模式），Content-Type 校验**也**跳过——curl 等工具
/// 习惯发无 Origin + 无 Content-Type 的 POST；这是 `allow_no_origin` 旗
/// 标的隐含语义。
pub fn check_mutating_request(
    origin: Option<&str>,
    content_type: Option<&str>,
    ctx: &SecurityContext,
    body_present: bool,
) -> Result<(), MutatingCheckError> {
    // 1) Origin 校验。
    if !is_allowed_origin(origin, ctx) {
        return Err(MutatingCheckError::from_origin(origin));
    }
    // 2) Content-Type 校验：
    //    - body 不存在：永远放行（前端 fetch 即便发 text/plain 也可）；
    //    - body 存在且 origin = Some：必须 application/json；
    //    - body 存在且 origin = None + allow_no_origin = true（工具模式）：
    //      跳过（curl/python 等工具不强制发 CT）。
    if body_present
        && !is_json_content_type(content_type)
        && !(origin.is_none() && ctx.allow_no_origin)
    {
        return Err(MutatingCheckError::BadContentType {
            got: content_type.map(|s| s.to_string()),
        });
    }
    Ok(())
}

/// 同源白名单判定（**纯函数**）。
///
/// `origin` 形如 `http://127.0.0.1:<port>` 或 `http://localhost:<port>`，
/// 端口必须等于 `ctx.port`。
pub fn is_same_loopback_origin(origin: &str, port: u16) -> bool {
    let lower = origin.trim().to_ascii_lowercase();
    lower == format!("http://127.0.0.1:{port}") || lower == format!("http://localhost:{port}")
}

/// Origin 是否放行（含无 Origin 工具客户端分支）。
///
/// - `Some(s)` → 走 [`is_same_loopback_origin`]；
/// - `None` → 走 `ctx.allow_no_origin` 开关。
pub fn is_allowed_origin(origin: Option<&str>, ctx: &SecurityContext) -> bool {
    match origin {
        Some(o) => is_same_loopback_origin(o, ctx.port),
        None => ctx.allow_no_origin,
    }
}

/// Content-Type 是否为 `application/json`（前缀匹配）。
///
/// 接受：
/// - `application/json`
/// - `application/json; charset=utf-8`（或任意 charset）
/// - 大小写不敏感（`Application/JSON` 也通过）
///
/// 拒绝：
/// - `text/plain`
/// - `application/x-www-form-urlencoded`
/// - `multipart/form-data`
/// - 缺省 / 空 / 其它
pub fn is_json_content_type(content_type: Option<&str>) -> bool {
    let Some(ct) = content_type else {
        return false;
    };
    let lower = ct.trim().to_ascii_lowercase();
    let main = lower.split(';').next().unwrap_or("").trim();
    main == "application/json"
}

/// mutating 校验的错误分类。
///
/// 错误体统一映射为 403（Origin 缺 / 错 + Content-Type 错；v1 简化错误面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutatingCheckError {
    /// `Origin` 头缺失；`allow_no_origin=false` 拒绝。
    NoOrigin,
    /// `Origin` 头非 loopback 同源。
    BadOrigin {
        /// 实际收到的值（None = 缺省）。
        got: Option<String>,
    },
    /// `Content-Type` 非 `application/json`（仅 body 存在时检查）。
    BadContentType {
        /// 实际收到的值（None = 缺省）。
        got: Option<String>,
    },
}

impl MutatingCheckError {
    /// 从 `origin` 派生具体子类。
    fn from_origin(origin: Option<&str>) -> Self {
        match origin {
            None => Self::NoOrigin,
            Some(o) => Self::BadOrigin {
                got: Some(o.to_string()),
            },
        }
    }

    /// 错误码（稳定契约字符串；D1 §5 风格）。
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::NoOrigin => "origin_required",
            Self::BadOrigin { .. } => "origin_denied",
            Self::BadContentType { .. } => "content_type_required",
        }
    }

    /// HTTP 状态码（v1 统一 403，简化错误面）。
    pub fn as_status(&self) -> u16 {
        403
    }

    /// 错误体 message（人类可读；i18n 友好——可演进）。
    pub fn as_message(&self) -> String {
        match self {
            Self::NoOrigin => "mutating 请求需带 Origin 头（loopback 同源）；如为工具客户端请设置 LIVE2D_AI_ALLOW_NO_ORIGIN=1".into(),
            Self::BadOrigin { got: Some(g) } => {
                format!("Origin {g:?} 不在 loopback 同源白名单")
            }
            Self::BadOrigin { got: None } => "Origin 不在 loopback 同源白名单".into(),
            Self::BadContentType { got: Some(g) } => {
                format!("mutating 请求需 Content-Type: application/json（收到 {g:?}）")
            }
            Self::BadContentType { got: None } => {
                "mutating 请求需 Content-Type: application/json（缺省）".into()
            }
        }
    }
}

/// 构造 mutating 校验失败的 403 响应（统一 JSON 错误体）。
pub fn mutating_check_error_response(
    err: &MutatingCheckError,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(
        err.as_code(),
        err.as_message(),
        serde_json::json!({"status": err.as_status()}),
    );
    let body = serde_json::to_vec(&ErrorResponse::new(detail))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    Response::from_data(body)
        .with_status_code(StatusCode(err.as_status()))
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}

// =====================================================================
// 内部辅助：从 tiny_http::Header 切片抽 (origin, content_type)
// =====================================================================

/// 从 `tiny_http` 头部集合中抽 `Origin` 与 `Content-Type`。
///
/// `equiv` 不区分大小写；`Origin` 在浏览器/标准 fetch 总是带
/// `Origin: <scheme>://<host>:<port>`。缺省返回 `None`。
///
/// **仅抽这两个**——避免把整张 headers 表传入 dispatch（保持签名轻）。
pub fn extract_origin_and_ct(headers: &[tiny_http::Header]) -> (Option<String>, Option<String>) {
    let mut origin: Option<String> = None;
    let mut ct: Option<String> = None;
    for h in headers {
        if h.field.equiv("Origin") && origin.is_none() {
            origin = Some(h.value.as_str().to_string());
        } else if h.field.equiv("Content-Type") && ct.is_none() {
            ct = Some(h.value.as_str().to_string());
        }
    }
    (origin, ct)
}

/// 路由 + method 是否需要 mutating 安全校验。
///
/// 与 [`crate::web_api::RouteId`] 同步：
/// - **恒 mutating**（只看 route）：`SettingsPatch` / `SettingsTestLlm` /
///   `SettingsTestTts` / `ChatPost` / `ChatStop`；
/// - **按 method 区分**：
///   - `Models` + GET（list/get）= 只读；
///     `Models` + POST（import / activate）/ PATCH（display）/
///     DELETE（delete）= mutating。
/// - **恒只读**：`AppCapabilities` / `AppStatus` / `SettingsGet` /
///   `LogsGet` / `LogsLevels` / `NotImplemented` /
///   `NotFound`。
///
/// 2026-09-11：动作命令端点（`RouteId::Commands`）随动作系统整体删除。
pub fn is_mutating_route(route: crate::web_api::RouteId, method: &tiny_http::Method) -> bool {
    use crate::web_api::RouteId;
    use tiny_http::Method;
    match route {
        // Models 路由按 method 区分：GET list/get 只读；POST import /
        // POST activate / PATCH display / DELETE delete 全部 mutating。
        RouteId::Models => matches!(*method, Method::Post | Method::Patch | Method::Delete),
        // 其余 mutating 端点（method 由 dispatch 入口先做校验）。
        RouteId::SettingsPatch
        | RouteId::SettingsTestLlm
        | RouteId::SettingsTestTts
        | RouteId::ChatPost
        | RouteId::ChatStop => true,
        // 只读端点。
        RouteId::AppCapabilities
        | RouteId::AppStatus
        | RouteId::SettingsGet
        | RouteId::LogsGet
        | RouteId::LogsLevels
        | RouteId::NotImplemented
        | RouteId::NotFound => false,
    }
}
