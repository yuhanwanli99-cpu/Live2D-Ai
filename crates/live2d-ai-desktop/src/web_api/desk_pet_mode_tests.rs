//! `POST /api/v1/desk-pet/mode` 的路由测试（执行单 §5 点名要的那条在第一个）。

use std::io::{Cursor, Read};

use live2d_ai_runtime::AppSettings;
use tiny_http::{Method, Response};

use crate::web_api::desk_pet_mode::DESK_PET_MODE_PATH;
use crate::web_api::security::SecurityContext;
use crate::web_api::{ServerContext, dispatch, dispatch_with_security};

fn ctx() -> ServerContext {
    ServerContext::new(
        AppSettings::default(),
        "/tmp/l2d-mode-test.toml".into(),
        None,
    )
    .with_security(SecurityContext::new(18080, false))
}

fn body_of(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut out = String::new();
    let _ = resp.into_reader().read_to_string(&mut out);
    out
}

/// 执行单 §5：`mode=pet` 不调用 `POST /api/v1/mods/discovered`。
///
/// 三重钉住：① handler 源码不引用 discovered 端点 / mods_routes / 进程表；
/// ② 新路径不落在 `/api/v1/mods/` 前缀里，结构上就进不了那条前置路由
/// （`FolderProcesses::running_ids` 只在 unix test 编译，Windows 侧改用这个
/// 与 `MODS_PREFIX` 同源的判据）；③ 响应体里没有任何 discovered 字段。
///
/// # 为什么 `ok` 的真值随平台变（不要写死成 `"ok":true`）
///
/// HTTP 层对「窗没开起来」是**刻意**回 200 的（调用方据此决定要不要
/// `window.close()`，见 `desk_pet_mode.rs` 末尾注释），所以状态码 200 与
/// `"mode":"pet"` 两边都成立。变的只有 `ok`：
///
/// - **Linux/macOS**：`desk_pet::mode::apply` 在开窗前就按 `cfg!(windows)`
///   判掉 `pet`，响应是 `{"ok":false,"mode":"pet","reason":"仅 Windows"}`；
/// - **Windows**：`cfg(test)` 使真开窗分支不参与编译，走「不真开窗」分支，
///   响应是 `{"ok":true,"mode":"pet"}`。
///
/// 结构断言（200、mode 回填、不落 `/api/v1/mods/` 前缀、handler 不引用
/// discovered、响应体无 discovered 字段）**与平台无关**，必须继续两边都跑；
/// 只有 `ok` 这一项按平台分叉。刻意不 `#[ignore]`、不删测试。
#[test]
fn mode_pet_never_calls_discovered() {
    let ctx = ctx();
    let resp = dispatch(&ctx, &Method::Post, DESK_PET_MODE_PATH, r#"{"mode":"pet"}"#);
    // 结构断言 ①：窗开没开起来都是 200（失败走 body 里的 ok:false + reason）。
    assert_eq!(resp.status_code().0, 200);
    let body = body_of(resp);
    let expected_ok = cfg!(windows);
    assert!(
        body.contains(if expected_ok {
            "\"ok\":true"
        } else {
            "\"ok\":false"
        }),
        "ok 应为 {expected_ok}（平台 {}）：{body}",
        std::env::consts::OS
    );
    if expected_ok {
        assert!(!body.contains("reason"), "成功不该带 reason：{body}");
    } else {
        assert!(
            body.contains("\"reason\":\"仅 Windows\""),
            "非 Windows 的 pet 必须给出「仅 Windows」这个原因：{body}"
        );
    }
    assert!(body.contains("\"mode\":\"pet\""), "{body}");
    assert!(!body.contains("discovered"), "{body}");
    assert!(
        !DESK_PET_MODE_PATH.starts_with("/api/v1/mods/"),
        "discovered 是 /api/v1/mods/ 下的前置路由；本端点不能落在那个前缀"
    );

    let source = include_str!("desk_pet_mode.rs");
    for forbidden in [
        "/api/v1/mods/discovered",
        "handle_mods_route",
        "discovered_select",
        "folder_processes",
        "mod_registry",
    ] {
        assert!(
            !source.contains(forbidden),
            "mode 端点不该引用启动器/Mod 链路：{forbidden}"
        );
    }
}

#[test]
fn mode_web_returns_ok_with_the_same_shape() {
    let resp = dispatch(
        &ctx(),
        &Method::Post,
        DESK_PET_MODE_PATH,
        r#"{"mode":"web"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    let body = body_of(resp);
    assert!(body.contains("\"ok\":true"), "{body}");
    assert!(body.contains("\"mode\":\"web\""), "{body}");
}

#[test]
fn malformed_mode_and_wrong_method_are_rejected() {
    let bad = dispatch(
        &ctx(),
        &Method::Post,
        DESK_PET_MODE_PATH,
        r#"{"mode":"desktop"}"#,
    );
    assert_eq!(bad.status_code().0, 400);
    let empty = dispatch(&ctx(), &Method::Post, DESK_PET_MODE_PATH, "");
    assert_eq!(empty.status_code().0, 400);
    let get = dispatch(&ctx(), &Method::Get, DESK_PET_MODE_PATH, "");
    assert_eq!(get.status_code().0, 405);
}

/// Origin / Content-Type 与 `/api/v1/chat` 同一套（都走 dispatch_with_security）：
/// 严格 ctx 下缺 Origin 的 mutating 请求 403 + `origin_required`。
#[test]
fn mode_endpoint_uses_the_chat_origin_guard() {
    let resp = dispatch_with_security(
        &ctx(),
        &Method::Post,
        DESK_PET_MODE_PATH,
        r#"{"mode":"pet"}"#,
        None,
        Some("application/json"),
    );
    assert_eq!(resp.status_code().0, 403);
    assert!(body_of(resp).contains("origin_required"));
    assert!(
        crate::web_api::security::is_mutating_route(
            crate::web_api::RouteId::DeskPetMode,
            &Method::Post
        ),
        "新端点必须是 mutating，才会走同一套校验"
    );
}
