//! Web 静态资产：Live2D 模型资产 + WASM 渲染页（节点 E E2）。
//!
//! 服务两条**读盘**路径（非 API，纯静态，无 mutating）：
//! - `/models/<rel>`  → 读 `<cwd>/assets/models/<rel>` 返回（Bai 模型资产，
//!   与原生桌宠的 `DEFAULT_BAI_MODEL3_RELATIVE` 同源，相对 cwd 查找）。
//! - `/render`        → 返回 WASM loader HTML（注入 wasm-bindgen 脚本；0.4.7 rc.5 起按 dist/ 真实文件名生成，见 render_page 模块）。
//! - `/render/<file>` → 读 `crates/l2d-wasm-demo/dist/<file>`（wasm / js）。
//!
//! # 为什么读盘而非 include_str!
//! wasm 产物较大（dev ~23MB，release ~5MB），`include_str!` 会把二进制塞进
//! 可执行文件；这里按路径读盘，因此这些静态资源**不入库**（wasm-demo 的
//! `dist/` 是构建产物）。`/models/*` 模型资产同理（Bai 在 `assets/models/`，
//! gitignore 不入库，用户本地合法持有）。
//!
//! # 安全
//! - 路径白名单：`/models/` 与 `/render/` 的 `rel` 经 `normalize_rel` 校验
//!   （拒绝 `..`、绝对路径、`\` 分隔符），**不**按任意路径读盘。
//! - 仅 GET；非 GET 回 405。

use std::io::Read;
use std::path::{Path, PathBuf};

use tiny_http::{Method, Response, StatusCode};

/// 归一化资产相对路径（拒绝 `..`、绝对路径、`\`、空段+NUL；允许 `[A-Za-z0-9._/-]`）。
pub fn normalize_rel(rel: &str) -> Result<String, String> {
    if rel.is_empty() {
        return Err("路径为空".to_string());
    }
    if rel.contains('\\') || rel.contains('\0') {
        return Err("路径含非法分隔符或 NUL".to_string());
    }
    if rel.starts_with('/') {
        return Err("路径为绝对路径".to_string());
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in rel.split('/') {
        match seg {
            "" | "." => continue,
            ".." => return Err("路径含 `..` 段".to_string()),
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        return Err("路径为空".to_string());
    }
    Ok(parts.join("/"))
}

/// 处理 `/models/*` 与 `/render*` 的静态资产请求。
///
/// 返回 `Some(response)` 表示已处理；`None` 表示非本模块路径（caller 继续 dispatch）。
///
/// # 查询串
///
/// 剥查询串的**正主**是请求循环入口的 [`crate::web_api::strip_query`]
/// （2026-09-11：那里修掉了 `/api/v1/logs?limit=200` → 501 的老 bug）。
/// 这里再剥一次是**防御性**的：`handle_assets` 也会被测试单独调用，
/// 而 `/render?hud=1` 正是 HUD 的开关——漏剥一次就永远打不开。
pub fn handle_assets(method: &Method, path: &str) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    let path: &str = crate::web_api::strip_query(path);
    if *method != Method::Get {
        // 非 GET：若是我们的路径前缀回 405，否则交给 dispatch（可能 404）。
        if path.starts_with("/models/")
            || path.starts_with("/actions/")
            || path.starts_with("/render")
        {
            return Some(method_not_allowed(path));
        }
        return None;
    }
    if path == "/render" {
        return Some(render_response_for_dist(&wasm_dist_dir()));
    }
    if let Some(rel) = path.strip_prefix("/render/") {
        // 读 dist/<rel>（rel 是 wasm-bindgen 的 .js / _bg.wasm）
        let dist_dir = wasm_dist_dir();
        return match read_file_safe(&dist_dir, rel) {
            Ok(bytes) => {
                let mime = wasm_mime(rel);
                Some(bytes_response(StatusCode(200), bytes, mime))
            }
            Err(e) => Some(json_error(StatusCode(404), "not_found", &e.to_string())),
        };
    }
    if let Some(rel) = path.strip_prefix("/models/") {
        // 读 <cwd>/assets/models/<rel>
        let rel = match normalize_rel(rel) {
            Ok(r) => r,
            Err(_) => return Some(json_error(StatusCode(400), "invalid_path", "非法模型路径")),
        };
        let assets_root = assets_models_dir();
        return match read_file_safe(&assets_root, &rel) {
            Ok(bytes) => {
                let mime = model_mime(&rel);
                Some(bytes_response(StatusCode(200), bytes, mime))
            }
            Err(e) => Some(json_error(StatusCode(404), "not_found", &e.to_string())),
        };
    }
    // `/actions/*` -> <cwd>/assets/actions/*（2026-09-16 P1-1：外置动作预设表）。
    // 与 `/models` 同一条读盘纪律：normalize_rel + canonicalize 防越界。
    if let Some(rel) = path.strip_prefix("/actions/") {
        let rel = match normalize_rel(rel) {
            Ok(r) => r,
            Err(_) => {
                return Some(json_error(
                    StatusCode(400),
                    "invalid_path",
                    "非法动作资产路径",
                ));
            }
        };
        let root = assets_actions_dir();
        return match read_file_safe(&root, &rel) {
            Ok(bytes) => Some(bytes_response(StatusCode(200), bytes, asset_mime(&rel))),
            Err(e) => Some(json_error(StatusCode(404), "not_found", &e.to_string())),
        };
    }
    None
}

/// /render 页面响应（dist 目录可注入：路由与单测走同一条路径）。
///
/// 0.4.7 rc.5：页面引用的 js / wasm 文件名取自 dist/ 里**真实存在**的文件
/// （trunk release 的 js 与 wasm 各自内容 hash，不能假设相同）；缺任意一个都回
/// 503 wasm_unavailable，不生成注定 404 的死页面。
fn render_response_for_dist(dist_dir: &Path) -> Response<std::io::Cursor<Vec<u8>>> {
    match crate::web_api::render_page::find_render_assets(dist_dir) {
        Some(assets) => html_response(
            StatusCode(200),
            &crate::web_api::render_page::render_html(&assets),
            "text/html; charset=utf-8",
        ),
        None => json_error(
            StatusCode(503),
            "wasm_unavailable",
            "WASM 产物缺失或不完整：请先 trunk build l2d-wasm-demo（需要 *_bg.wasm 与同 crate 前缀的 .js）",
        ),
    }
}

/// `<cwd>/assets/models`（**唯一模型根**；与模型库 registry / import 同源）。
///
/// 别名到 [`crate::web_api::model_root::model_root`]：这里曾经自带一份 cwd 解析，
/// 而 registry 另写 XDG——那正是「激活了但没换皮」的成因。现在只有一处定义。
fn assets_models_dir() -> PathBuf {
    crate::web_api::model_root::model_root()
}

/// `<assets>/actions`（动作预设等**用户可改**的表；与 `/models` 同属 assets 树）。
///
/// 0.4.0 rc.1：与模型根同源的 `assets/actions` 不存在时，再看 exe 旁边的
/// `assets/actions`（打包布局；`web/` 与 `assets/` 同在 exe 同级）。
fn assets_actions_dir() -> PathBuf {
    let derived = crate::web_api::model_root::model_root()
        .parent()
        .map(|assets| assets.join("actions"))
        .unwrap_or_else(|| PathBuf::from("assets/actions"));
    if derived.is_dir() {
        return derived;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(packaged) = crate::host_layout::packaged_actions_dir(&exe)
        && packaged.is_dir()
    {
        return packaged;
    }
    derived
}

/// 渲染面 wasm 产物目录（第一个存在的候选胜出）。
///
/// 候选：cwd 相对 → exe 旁边的 `web/render/`（0.4.0 rc.1 打包布局，pack 把
/// `dist/` 放这里）→ 从 cwd 往上找仓库根的 `crates/l2d-wasm-demo/dist`。
fn wasm_dist_dir() -> PathBuf {
    let default = PathBuf::from("crates/l2d-wasm-demo/dist");
    let mut candidates = vec![
        default.clone(),
        PathBuf::from("../crates/l2d-wasm-demo/dist"),
    ];
    if let Ok(exe) = std::env::current_exe()
        && let Some(packaged) = crate::host_layout::packaged_wasm_dist_dir(&exe)
    {
        candidates.push(packaged);
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Some(root) = crate::web_api::flutter_app::repo_root(&cwd)
    {
        candidates.push(root.join("crates/l2d-wasm-demo/dist"));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_dir())
        .unwrap_or(default)
}

/// 安全读盘：先 normalize_rel，再 `root.join(rel)`，且拒绝越出 root。
fn read_file_safe(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let rel = normalize_rel(rel).map_err(|e| format!("非法相对路径: {e}"))?;
    let full = root.join(&rel);
    // canonicalize 防符号链接越界 + 确认在 root 下。
    let canon_root = std::fs::canonicalize(root).map_err(|e| format!("root 无法规范化: {e}"))?;
    let canon_full = std::fs::canonicalize(&full).map_err(|e| format!("文件不存在: {e}"))?;
    if !canon_full.starts_with(&canon_root) {
        return Err("越出 assets 根目录".to_string());
    }
    let mut f = std::fs::File::open(&full).map_err(|e| format!("打开失败: {e}"))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| format!("读取失败: {e}"))?;
    Ok(buf)
}

fn wasm_mime(rel: &str) -> &'static str {
    if rel.ends_with(".wasm") {
        "application/wasm"
    } else if rel.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

/// `/actions/*` 的 MIME（目前只有 JSON，保留图片分支以便将来放图标）。
fn asset_mime(rel: &str) -> &'static str {
    if rel.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if rel.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    }
}

fn model_mime(rel: &str) -> &'static str {
    if rel.ends_with(".png") {
        "image/png"
    } else if rel.ends_with(".json") {
        // 覆盖 .json / .model3.json / .physics3.json / .cdi3.json / .vtube.json。
        "application/json; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn html_response(
    status: StatusCode,
    body: &str,
    mime: &'static str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    bytes_response(status, body.as_bytes().to_vec(), mime)
}

fn bytes_response(
    status: StatusCode,
    bytes: Vec<u8>,
    mime: &'static str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    use tiny_http::Header;
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
    html_response(status, &body, "application/json; charset=utf-8")
}

fn method_not_allowed(path: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    json_error(
        StatusCode(405),
        "method_not_allowed",
        &format!("{path:?} 仅支持 GET"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rel_rejects_traversal() {
        assert!(normalize_rel("../etc/passwd").is_err());
        assert!(normalize_rel("a/../../b").is_err());
        assert!(normalize_rel("/etc/passwd").is_err());
        assert!(normalize_rel("a\\b").is_err());
        assert!(normalize_rel("").is_err());
        assert!(normalize_rel("models/bai/runtime/bai.moc3").is_ok());
    }

    /// `?hud=1` 必须能路由到 `/render`（查询串在匹配前剥掉）。
    ///
    /// 这条不是「顺便测一下」：HUD 开关**就是**查询串，
    /// 而全仓库的路由都是精确匹配——不剥就会 404，HUD 永远打不开。
    #[test]
    fn render_route_matches_with_query_string() {
        // 不带查询串：归本模块管。
        let plain = handle_assets(&Method::Get, "/render").expect("应处理 /render");
        // 带查询串：**同样归本模块管**（否则 `?hud=1` 会 404）。
        let query = handle_assets(&Method::Get, "/render?hud=1").expect("应处理 /render?hud=1");
        // 不断言 200：`dist/` 是构建产物，测试环境里可能还没 trunk build，
        // 那时回 503 `wasm_unavailable` 是**正确**行为。
        // 真正要守的是「查询串不改变路由结果」——把两种写法直接对比。
        assert_eq!(
            plain.status_code().0,
            query.status_code().0,
            "带不带 `?hud=1` 必须命中同一条路由"
        );
        assert_ne!(plain.status_code().0, 404, "`/render` 不该是 404");
        // 别的路径仍然不归本模块管（不能因为剥了 `?` 就把任意路径都吃掉）。
        assert!(handle_assets(&Method::Get, "/api/v1/settings?x=1").is_none());
        assert!(handle_assets(&Method::Get, "/renderx").is_none());
    }

    #[test]
    fn mime_map() {
        assert_eq!(
            model_mime("bai/runtime/bai.model3.json"),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            model_mime("bai/runtime/bai.moc3"),
            "application/octet-stream"
        );
        assert_eq!(model_mime("bai/runtime/texture_00.png"), "image/png");
        assert!(model_mime("bai/runtime/bai.16384").starts_with("application/"));
    }

    /// `/actions/*`（2026-09-16 P1-1）：外置动作预设表必须可静态取到。
    #[test]
    fn actions_route_serves_presets_json() {
        let resp = handle_assets(&Method::Get, "/actions/presets.json")
            .expect("应处理 /actions/presets.json");
        assert_eq!(
            resp.status_code().0,
            200,
            "出厂 assets/actions/presets.json 必须存在"
        );
        // 非 GET / 路径穿越都有明确处置，不会被当成别的路由。
        assert!(handle_assets(&Method::Post, "/actions/presets.json").is_some());
        assert_eq!(
            handle_assets(&Method::Get, "/actions/../secret")
                .expect("仍归本模块管")
                .status_code()
                .0,
            400
        );
    }

    /// 0.4.7 rc.5：dist 里只有 wasm 没有 js 时回 503（不生成注定 404 的死页面）；
    /// 两个真实文件名不同则回 200。
    #[test]
    fn render_route_reports_503_for_incomplete_dist_and_200_for_real_pair() {
        let root = std::env::temp_dir().join(format!(
            "l2d-render-dist-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dist");

        // 只有 wasm：产物不完整 → 503（不能 200 一个引用不存在 js 的页面）。
        std::fs::write(root.join("l2d-wasm-demo-1abe76bb6b842901_bg.wasm"), b"wasm")
            .expect("写 wasm");
        assert_eq!(
            render_response_for_dist(&root).status_code().0,
            503,
            "只有 wasm 没有 js 必须 503 wasm_unavailable"
        );

        // 补上真实 js（与 wasm 不同 hash）→ 200。
        std::fs::write(root.join("l2d-wasm-demo-a5fb29b722ce6403.js"), b"js").expect("写 js");
        assert_eq!(
            render_response_for_dist(&root).status_code().0,
            200,
            "js 与 wasm 都真实存在时必须 200"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
