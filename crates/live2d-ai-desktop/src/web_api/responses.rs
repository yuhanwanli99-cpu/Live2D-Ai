//! Web API 通用响应构造器（D-P0B 拆分承载，2026-08-29）。
//!
//! `mod.rs` 已贴 500 上限；本文件集中 `not_implemented_response` /
//! `not_found_response` / `forbidden_logs_response` / `json_error_response`
//! 四个 helper，避免 mod.rs 重复 `Header::from_bytes` 样板。
//!
//! **不**导出为 `pub`——仅同 crate 内可见；测试在 `web_api::responses` 名字
//! 空间下访问。

use std::io::Cursor;

use tiny_http::{Header, Response, StatusCode};

use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// D1 §5：501 + `{"error":{"code":"not_implemented","endpoint":...}}`。
pub(crate) fn not_implemented(endpoint: &'static str) -> Response<Cursor<Vec<u8>>> {
    #[derive(serde::Serialize)]
    struct NotImplBody<'a> {
        error: NotImplError<'a>,
    }
    #[derive(serde::Serialize)]
    struct NotImplError<'a> {
        code: &'static str,
        message: &'static str,
        endpoint: &'a str,
    }
    let body = NotImplBody {
        error: NotImplError {
            code: "not_implemented",
            message: "本端点已冻结但实现待 D3/D4 落地",
            endpoint,
        },
    };
    json_error(
        StatusCode(501),
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}

/// P0：404 一律走统一错误响应（避免 CORS 旁路）。
pub(crate) fn not_found(path: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(
        "not_found",
        format!("路径 {path:?} 不存在"),
        serde_json::json!({"path": path}),
    );
    json_error(
        StatusCode(404),
        serde_json::to_vec(&ErrorResponse::new(detail)).unwrap_or_default(),
    )
}

/// `/api/v1/logs*` 系列端点 dev_mode=false 时的统一 403 响应（不暴露日志存在性）。
pub(crate) fn forbidden_logs() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "dev_mode_required",
        "logs 端点需 dev_mode=true 开启（StatusContext 持有开关）",
    );
    json_error(
        StatusCode(403),
        serde_json::to_vec(&ErrorResponse::new(detail)).unwrap_or_default(),
    )
}

/// 通用 JSON 错误响应（status + body bytes；body 已是序列化好的 JSON）。
pub(crate) fn json_error(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}
