//! Flutter Web 前端托管（`GET /app` 与 `GET /app/*`）。
//!
//! 服务 `shell/flutter/build/web/` 下的 Flutter Web 构建产物
//! （`cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`）。
//!
//! `--no-web-resources-cdn` **不是可选项**：缺省构建会把 CanvasKit 指向
//! `https://www.gstatic.com/flutter-canvaskit/<engineRevision>/`，而本机
//! `build/web/canvaskit/` 那份 37 MB 副本根本不被引用——于是**断网即白屏**。
//! 本项目是本地优先的桌宠，不允许依赖 Google CDN。
//!
//! # 为什么必须同源
//! Flutter 端用 `HtmlElementView` 内嵌 `/render` iframe，并以
//! `postMessage(msg, location.origin)` 双向通信且校验 `event.origin`；
//! 只有 Flutter 产物与 `/render` 同源（都由本服务 loopback 提供）才成立。
//!
//! # 安全
//! - 仅 GET；非 GET 回 405。
//! - 路径经 [`normalize_rel`] 校验（拒绝 `..` / 绝对路径 / `\`），读盘前再
//!   canonicalize 确认未越出构建目录。
//! - 构建目录可经 `LIVE2D_AI_FLUTTER_WEB_DIR` 覆盖（默认 `<cwd>/shell/flutter/build/web`）。
//! - 产物缺失 → 503 + 明确指引（不留白屏）。

use std::io::Read;
use std::path::{Path, PathBuf};

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::wasm_assets::normalize_rel;

/// `/app` 前缀。
const APP_PREFIX: &str = "/app";

/// 是否为本模块接管的路径（`/app` 或 `/app/...`；不吞 `/apple`）。
pub(crate) fn is_app_path(path: &str) -> bool {
    path == APP_PREFIX || path.starts_with("/app/")
}

/// Flutter Web 构建目录（env 覆盖 > cwd 相对 > workspace 相对）。
fn build_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LIVE2D_AI_FLUTTER_WEB_DIR") {
        return PathBuf::from(dir);
    }
    let candidates = [
        PathBuf::from("shell/flutter/build/web"),
        PathBuf::from("../shell/flutter/build/web"),
    ];
    for c in candidates {
        if c.is_dir() {
            return c;
        }
    }
    PathBuf::from("shell/flutter/build/web")
}

/// 处理 `/app*`。返回 `None` 表示非本模块路径（交还 dispatch）。
pub fn handle_flutter_app(
    method: &Method,
    path: &str,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    if !is_app_path(path) {
        return None;
    }
    if *method != Method::Get {
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "Flutter 前端仅支持 GET",
        ));
    }
    let root = build_dir();
    if !root.is_dir() {
        return Some(json_error(
            StatusCode(503),
            "flutter_build_missing",
            "Flutter Web 产物缺失：请先执行 `cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`",
        ));
    }
    let rel = path
        .strip_prefix(APP_PREFIX)
        .unwrap_or("")
        .trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    match read_file_safe(&root, rel) {
        Ok(bytes) => Some(bytes_response(StatusCode(200), bytes, flutter_mime(rel))),
        Err(e) => {
            // 无扩展名的路径 = 客户端路由：回落 index.html，避免刷新 404。
            let leaf = rel.rsplit('/').next().unwrap_or("");
            if !leaf.contains('.')
                && let Ok(bytes) = read_file_safe(&root, "index.html")
            {
                return Some(bytes_response(
                    StatusCode(200),
                    bytes,
                    "text/html; charset=utf-8",
                ));
            }
            Some(json_error(StatusCode(404), "not_found", &e))
        }
    }
}

/// Flutter Web 产物常见扩展名 → MIME。
pub(crate) fn flutter_mime(rel: &str) -> &'static str {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".wasm") {
        "application/wasm"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else if lower.ends_with(".woff") {
        "font/woff"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".otf") {
        "font/otf"
    } else {
        // `.map` / `.symbols` / 其它未知后缀：按二进制直出。
        "application/octet-stream"
    }
}

/// 安全读盘：`normalize_rel` → `root.join` → canonicalize 确认在 root 内。
fn read_file_safe(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let rel = normalize_rel(rel).map_err(|e| format!("非法相对路径: {e}"))?;
    let full = root.join(&rel);
    let canon_root = std::fs::canonicalize(root).map_err(|e| format!("root 无法规范化: {e}"))?;
    let canon_full = std::fs::canonicalize(&full).map_err(|e| format!("文件不存在: {e}"))?;
    if !canon_full.starts_with(&canon_root) {
        return Err("越出构建目录".to_string());
    }
    let mut f = std::fs::File::open(&full).map_err(|e| format!("打开失败: {e}"))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| format!("读取失败: {e}"))?;
    Ok(buf)
}

fn bytes_response(
    status: StatusCode,
    bytes: Vec<u8>,
    mime: &'static str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(bytes)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).expect("Content-Type 头"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]).expect("Cache-Control 头"),
        )
}

fn json_error(status: StatusCode, code: &str, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    bytes_response(status, body.into_bytes(), "application/json; charset=utf-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_path_prefix_is_exact() {
        assert!(is_app_path("/app"));
        assert!(is_app_path("/app/"));
        assert!(is_app_path("/app/main.dart.js"));
        assert!(!is_app_path("/apple"));
        assert!(!is_app_path("/"));
        assert!(!is_app_path("/render"));
    }

    #[test]
    fn mime_covers_flutter_artifacts() {
        assert_eq!(flutter_mime("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            flutter_mime("flutter_bootstrap.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(flutter_mime("canvaskit/canvaskit.wasm"), "application/wasm");
        assert_eq!(
            flutter_mime("assets/AssetManifest.json"),
            "application/json; charset=utf-8"
        );
        assert_eq!(flutter_mime("favicon.png"), "image/png");
        assert_eq!(flutter_mime("x.bin"), "application/octet-stream");
    }

    #[test]
    fn non_get_is_method_not_allowed() {
        let resp = handle_flutter_app(&Method::Post, "/app/").expect("本模块接管 /app");
        assert_eq!(resp.status_code().0, 405);
    }

    #[test]
    fn unrelated_path_is_not_handled() {
        assert!(handle_flutter_app(&Method::Get, "/api/v1/chat").is_none());
        assert!(handle_flutter_app(&Method::Get, "/render").is_none());
    }
}
