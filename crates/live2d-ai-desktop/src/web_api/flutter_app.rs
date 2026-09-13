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
//! - 构建目录可经 `LIVE2D_AI_FLUTTER_WEB_DIR` 覆盖，否则按候选序列探测（见
//!   [`build_dir_candidates`]）：cwd 相对 → 可执行文件相对 → 仓库根相对。
//! - 产物缺失 → 503 + **列出实际找过的路径**（不留白屏，也不留一片空白的排障）。

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

/// 构建目录相对仓库根的路径。
const BUILD_REL: &str = "shell/flutter/build/web";

/// Flutter Web 构建目录候选，**按优先级**排列（第一个存在的胜出）。
///
/// 为什么需要这么多候选：产物目录是「谁编谁决定」的（`flutter build web` 落在
/// `shell/flutter/build/web`），而服务进程的 cwd 却可以是仓库根、`crates/`、
/// 甚至别处。只有一个相对 cwd 的候选时，cwd 一换就退化成 503——用户看到的是
/// 「界面打不开」，而不是「路径没对上」。这里把三种锚点都写下来：
///
/// 1. `LIVE2D_AI_FLUTTER_WEB_DIR`（显式覆盖；`scripts/ignite.sh` 会设置它）
/// 2. **cwd 相对**：`shell/flutter/build/web`、`../shell/flutter/build/web`
/// 3. **可执行文件相对**：`target/debug/` 这类目录往上找仓库根
/// 4. **cwd 往上找仓库根**：从子目录（如 `crates/live2d-ai-desktop`）启动也能命中
fn build_dir_candidates() -> Vec<PathBuf> {
    let rel = Path::new(BUILD_REL);
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("LIVE2D_AI_FLUTTER_WEB_DIR") {
        out.push(PathBuf::from(dir));
    }
    out.push(rel.to_path_buf());
    out.push(Path::new("..").join(rel));
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join(rel));
        if let Some(root) = repo_root(dir) {
            out.push(root.join(rel));
        }
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Some(root) = repo_root(&cwd)
    {
        out.push(root.join(rel));
    }
    out
}

/// 从 `start` 起向上找**仓库根**：同时含 `Cargo.toml` 与 `shell/flutter` 的最近祖先。
///
/// 两个条件都要：`crates/*/Cargo.toml` 遍地都是，单看 `Cargo.toml` 会在
/// `crates/live2d-ai-desktop/` 就停下来，拼出来的路径必然不对。
fn repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("Cargo.toml").is_file() && d.join("shell/flutter").is_dir())
        .map(Path::to_path_buf)
}

/// 取候选里第一个存在的目录。
fn resolve_build_dir() -> Option<PathBuf> {
    build_dir_candidates().into_iter().find(|c| c.is_dir())
}

/// 产物缺失时的 503 文案：指引 + **实际找过的路径**（含 cwd）。
fn missing_build_hint(candidates: &[PathBuf]) -> String {
    let tried = candidates
        .iter()
        .map(|c| c.display().to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    let cwd = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_else(|_| "<未知>".to_string());
    format!(
        "Flutter Web 产物缺失：请先执行 \
         `cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`。\
         已找过的路径（cwd={cwd}）：{tried}"
    )
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
    let Some(root) = resolve_build_dir() else {
        return Some(json_error(
            StatusCode(503),
            "flutter_build_missing",
            &missing_build_hint(&build_dir_candidates()),
        ));
    };
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

    /// cwd 相对候选永远在列表里（env 覆盖只是**插到最前**，不是唯一来源）。
    #[test]
    fn candidates_always_include_the_cwd_relative_default() {
        let candidates = build_dir_candidates();
        assert!(!candidates.is_empty(), "候选不能为空");
        assert!(
            candidates.contains(&PathBuf::from(BUILD_REL)),
            "缺少 cwd 相对候选：{candidates:?}"
        );
    }

    /// 从 crate 目录往上能找到**仓库根**（`crates/live2d-ai-desktop` 自身也有
    /// `Cargo.toml`，所以判定必须额外要求 `shell/flutter`，否则会在这里就停住）。
    #[test]
    fn repo_root_skips_the_crate_manifest() {
        let root = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("应能找到仓库根");
        assert!(
            root.join("shell/flutter").is_dir() && root.join("crates/live2d-ai-desktop").is_dir(),
            "仓库根不对：{}",
            root.display()
        );
    }

    #[test]
    fn repo_root_is_none_outside_a_repo() {
        assert!(repo_root(Path::new("/")).is_none());
    }

    /// 503 文案必须**列出每个找过的路径**——只给一句「请先构建」等于排障时一片空白。
    #[test]
    fn missing_build_hint_lists_every_tried_path() {
        let candidates = vec![
            PathBuf::from("/tmp/definitely/not/here"),
            PathBuf::from("shell/flutter/build/web"),
        ];
        let hint = missing_build_hint(&candidates);
        for c in &candidates {
            assert!(
                hint.contains(&c.display().to_string()),
                "文案缺少路径 {}：{hint}",
                c.display()
            );
        }
        assert!(
            hint.contains("flutter build web"),
            "文案缺少构建指引：{hint}"
        );
    }
}
