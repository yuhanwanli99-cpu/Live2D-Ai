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

/// 根路径（`/` 与 `/index.html`）→ **302 到 `/app/`**。
///
/// # 为什么要有这个跳转（2026-09-11 用户反馈）
///
/// 「之前残留老前端 `/` 和 `/app` 不一样」。确实：`/` 曾经是**原生 JS 前端**
/// （`index_html.rs` + `index.html`/`app.js`/`chat.js`/`style.css`），
/// 与 `/app` 的 Flutter 前端是**两套不同的界面**。同一个服务、同一个端口，
/// 打开哪个 URL 看到哪个产品——而且老那套不包含这一轮的任何界面改动。
///
/// AGENTS.md 早就把 JS 前端定为遗留（「只修致命缺陷、不再加新功能」），
/// 那就**不该让它继续占着根路径**：留着就是「写好了但过时了还看得到」。
/// 现在 `/` 是 Flutter 前端的入口，老前端整套已删除。
///
/// 302（不是 301）：这是「界面入口暂时在这儿」的语义，不是永久别名；
/// 哪天要换入口，301 会被浏览器永久缓存住，很难撤。
fn root_redirect() -> Response<std::io::Cursor<Vec<u8>>> {
    use tiny_http::Header;
    Response::from_data(Vec::new())
        .with_status_code(StatusCode(302))
        .with_header(Header::from_bytes(&b"Location"[..], &b"/app/"[..]).expect("Location 头"))
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).expect("Cache-Control 头"),
        )
}

/// 是否为根路径（`/` 或 `/index.html`）。
pub(crate) fn is_root_path(path: &str) -> bool {
    path == "/" || path == "/index.html"
}

/// 处理 `/`（→ `/app/`）与 `/app*`。返回 `None` 表示非本模块路径（交还 dispatch）。
pub fn handle_flutter_app(
    method: &Method,
    path: &str,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    if is_root_path(path) {
        // 非 GET 交给 dispatch 走既有的 405 / 404 语义（别在这里造第二套）。
        return if *method == Method::Get {
            Some(root_redirect())
        } else {
            None
        };
    }
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

    /// 根路径 → 302 到 `/app/`（2026-09-11：不再让老 JS 前端占着 `/`）。
    ///
    /// 这条守的是「**一个服务只有一个界面入口**」：`/` 与 `/app` 曾经是两套
    /// 不同的前端，用户打开哪个 URL 看到哪个产品。
    #[test]
    fn root_paths_redirect_to_the_flutter_app() {
        for path in ["/", "/index.html"] {
            assert!(is_root_path(path), "{path} 应被认作根路径");
            let resp = handle_flutter_app(&Method::Get, path).expect("本模块接管根路径");
            assert_eq!(resp.status_code().0, 302, "{path} 应是 302");
            let loc = resp
                .headers()
                .iter()
                .find(|h| h.field.equiv("Location"))
                .map(|h| h.value.as_str().to_string())
                .expect("Location 头");
            assert_eq!(loc, "/app/", "{path} 应跳到 Flutter 前端");
        }
        // 别的路径不受影响。
        assert!(!is_root_path("/app/"));
        assert!(!is_root_path("/app"));
        assert!(!is_root_path("/render"));
    }

    /// 根路径的非 GET 不在这里拦——交回 dispatch 走既有的 method 语义。
    #[test]
    fn root_path_non_get_is_left_to_dispatch() {
        assert!(handle_flutter_app(&Method::Post, "/").is_none());
        assert!(handle_flutter_app(&Method::Patch, "/index.html").is_none());
    }

    #[test]
    fn unrelated_path_is_not_handled() {
        assert!(handle_flutter_app(&Method::Get, "/api/v1/chat").is_none());
        assert!(handle_flutter_app(&Method::Get, "/render").is_none());
    }
}
