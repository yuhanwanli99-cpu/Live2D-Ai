//! `POST /api/v1/desk-pet/mode`：产品路径里「网页 ↔ 桌宠」的切换端点。
//!
//! 请求：`{"mode":"pet"}` 或 `{"mode":"web"}`；成功 `{"ok":true,"mode":…}`，
//! 窗没开起来仍是 200 + `{"ok":false,"mode":…,"reason":…}`。
//!
//! 边界（执行单 §2 / §4）：`pet` 只用当前 `mods.desk-pet.config` 开窗，
//! **不写盘、不 enable 工厂、不调用启动器的 discovered 端点**；`web` 关窗并
//! 用系统浏览器打开 `/app/`（不是 `GET /`），不重启进程、不重跑「进入」、
//! 不重探测 LLM/TTS、不杀任何 Mod 进程。
//!
//! 校验走 `dispatch_with_security` → `is_mutating_route` → `check_mutating_request`，
//! 与 `/api/v1/chat` 是同一套（`Origin` + `Content-Type`），这里不另写一份。

use std::io::Cursor;

use serde::Deserialize;
use tiny_http::{Method, Response, StatusCode};

pub const DESK_PET_MODE_PATH: &str = "/api/v1/desk-pet/mode";

type Resp = Response<Cursor<Vec<u8>>>;

#[derive(Debug, Deserialize)]
struct ModeRequest {
    mode: String,
}

fn error(status: u16, code: &'static str, message: &str) -> Resp {
    crate::web_api::app_routes::json_response(
        StatusCode(status),
        &crate::web_api::dto::ErrorResponse::new(crate::web_api::dto::ErrorDetail::new(
            code, message,
        )),
    )
}

/// method / body 校验 → 交给 `desk_pet::mode::apply`（与右键菜单同一个函数）。
pub fn handle_desk_pet_mode(method: &Method, body: &str) -> Resp {
    if *method != Method::Post {
        return error(405, "method_not_allowed", "仅支持 POST");
    }
    let parsed: ModeRequest = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            return error(
                400,
                "invalid_mode",
                "请求体必须是 {\"mode\":\"pet\"|\"web\"}",
            );
        }
    };
    let Some(mode) = crate::desk_pet::mode::Mode::parse(&parsed.mode) else {
        return error(400, "invalid_mode", "mode 只能是 pet 或 web");
    };
    let outcome = crate::desk_pet::mode::apply(mode);
    let mut payload = serde_json::json!({ "ok": outcome.ok, "mode": outcome.mode.as_str() });
    if let Some(reason) = outcome.reason {
        payload["reason"] = serde_json::Value::String(reason);
    }
    // 200 也用于「窗没开起来」：调用方据此决定要不要 window.close()。
    crate::web_api::app_routes::json_response(StatusCode(200), &payload)
}
