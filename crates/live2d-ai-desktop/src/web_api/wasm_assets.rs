//! Web 静态资产：Live2D 模型资产 + WASM 渲染页（节点 E E2）。
//!
//! 服务两条**读盘**路径（非 API，纯静态，无 mutating）：
//! - `/models/<rel>`  → 读 `<cwd>/assets/models/<rel>` 返回（Bai 模型资产，
//!   与原生桌宠的 `DEFAULT_BAI_MODEL3_RELATIVE` 同源，相对 cwd 查找）。
//! - `/render`        → 返回 WASM loader HTML（注入 wasm-bindgen 脚本）。
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

/// WASM 渲染页 HTML（runtime 生成，引用 dist 下的 wasm-bindgen 产物）。
///
/// `hash` 是 trunk 产物的 hash（形如 `3f6a9add9989bde5`），从 `dist/`
/// 目录扫描得到（`*_bg.wasm` 文件名前缀）。
///
/// # 状态栏默认**不显示**（2026-09-11）
///
/// 用户裁决「舞台背影全黑/全白即可，中央不要放舞台贴图」。
/// 这条状态栏是一块压在整个舞台底部的深色横条——它是**调试指标**，
/// 而产品界面里的加载进度 / FPS / 错误都由 Flutter 壳自己画
/// （`StageHost` 的覆盖层 + `Live2DStage` 的角标），所以它对用户是纯噪声。
///
/// 想说清楚的是：它此前**不是**布局问题（早已是 `position:absolute` 覆盖层），
/// 而是**可见性问题**——不管怎么排，它都画在舞台上。
/// 现在改成只在 `?hud=1` 时出现；`/render` 仍然可以单独打开来调试
/// （`http://127.0.0.1:18080/render?hud=1`）。
pub fn render_html(hash: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Live2D-Ai · 模型渲染</title>
<style>
 :root {{ color-scheme: dark; }}
 html,body {{ margin:0;height:100%;background:#101418;color:#d7dde3;font-family:system-ui,sans-serif;overflow:hidden; }}
 /* #stage 铺满画布；#status 是**覆盖层**：
    否则 #status 文案变长会挤压 canvas 的 CSS 高度，而窗口尺寸未变不触发 resize，
    canvas 位图与显示盒宽高比不一致 → 浏览器非等比拉伸 → 模型被压矮（2026-09-10 实测）。 */
 #stage {{ position:absolute;inset:0; }}
 canvas {{ width:100%;height:100%;display:block; }}
 /* 默认 display:none。要看它：地址后加 `?hud=1`（见 body 上的内联脚本）。
    左下角小片 + 半透明底：它现在压在任何主题色的舞台上，
    纯色文字在浅色主题上会读不出来。 */
 #status {{ display:none;position:absolute;left:8px;bottom:8px;margin:0;padding:4px 8px;
            max-width:calc(100% - 16px);max-height:40%;overflow:auto;
            font-family:ui-monospace,"SF Mono","Courier New",monospace;
            font-size:12px;white-space:pre;word-break:keep-all;
            color:#9fd0a0;background:rgba(10,12,16,.72);
            border:1px solid #2a323b;border-radius:6px;pointer-events:none; }}
 body.hud #status {{ display:block; }}
</style>
</head>
<body>
<div id="stage"><canvas id="canvas"></canvas></div>
<pre id="status">初始化中…</pre>
<script>
 /* 只在 `?hud=1` 时显示状态栏。内联，不引任何依赖。 */
 if (location.search.indexOf('hud=1') >= 0) {{ document.body.classList.add('hud'); }}
</script>
<script type="module">
import init from '/render/l2d-wasm-demo-{hash}.js';
await init({{ module_or_path: '/render/l2d-wasm-demo-{hash}_bg.wasm' }});
</script>
</body>
</html>"#
    )
}

/// 从 `dist/` 目录找 wasm-bindgen 产物的 hash（`*_bg.wasm` 文件名前缀）。
pub fn wasm_hash_from_dist(dist_dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dist_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(base) = name.strip_suffix("_bg.wasm") {
            // 形如 `l2d-wasm-demo-3f6a9add9989bde5_bg.wasm` → `l2d-wasm-demo-3f6a9add9989bde5`
            if let Some(hash) = base.rsplit_once("l2d-wasm-demo-") {
                return Some(hash.1.to_string());
            }
        }
    }
    None
}

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
        if path.starts_with("/models/") || path.starts_with("/render") {
            return Some(method_not_allowed(path));
        }
        return None;
    }
    if path == "/render" {
        // scan dist 的 hash
        let dist_dir = wasm_dist_dir();
        return Some(match wasm_hash_from_dist(&dist_dir) {
            Some(hash) => html_response(
                StatusCode(200),
                &render_html(&hash),
                "text/html; charset=utf-8",
            ),
            None => json_error(
                StatusCode(503),
                "wasm_unavailable",
                "WASM 产物缺失：请先 trunk build l2d-wasm-demo",
            ),
        });
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
    None
}

/// `<cwd>/assets/models`（**唯一模型根**；与模型库 registry / import 同源）。
///
/// 别名到 [`crate::web_api::model_root::model_root`]：这里曾经自带一份 cwd 解析，
/// 而 registry 另写 XDG——那正是「激活了但没换皮」的成因。现在只有一处定义。
fn assets_models_dir() -> PathBuf {
    crate::web_api::model_root::model_root()
}

/// `crates/l2d-wasm-demo/dist`（workspace 相对路径）。
fn wasm_dist_dir() -> PathBuf {
    // 相对 cwd；若从 workspace 根启动则定位到 crates/l2d-wasm-demo/dist。
    let candidates = [
        PathBuf::from("crates/l2d-wasm-demo/dist"),
        PathBuf::from("../crates/l2d-wasm-demo/dist"),
        PathBuf::from("/home/skystar/deepseekharness/Live2D-Ai/crates/l2d-wasm-demo/dist"),
    ];
    for c in candidates {
        if c.is_dir() {
            return c;
        }
    }
    PathBuf::from("crates/l2d-wasm-demo/dist")
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

    #[test]
    fn render_html_contains_wasm_loader() {
        let html = render_html("abc123");
        assert!(html.contains("/render/l2d-wasm-demo-abc123.js"));
        assert!(html.contains("/render/l2d-wasm-demo-abc123_bg.wasm"));
        assert!(html.contains("id=\"canvas\""));
    }

    /// 状态栏**默认不可见**（2026-09-11 用户裁决「舞台背影全黑/全白即可」）。
    ///
    /// 这条守的是「舞台底部不会出现一块深色横条」：`#status` 的样式里必须有
    /// `display:none`，且只有 `body.hud` 才把它打开——而 `body.hud` 只在
    /// `?hud=1` 时由内联脚本加上。
    #[test]
    fn render_html_hides_hud_unless_explicitly_requested() {
        let html = render_html("abc123");
        assert!(
            html.contains("#status { display:none;"),
            "#status 必须默认隐藏，否则舞台底部会有一条深色横条"
        );
        assert!(
            html.contains("body.hud #status { display:block; }"),
            "必须保留 `?hud=1` 这条调试入口"
        );
        assert!(
            html.contains("hud=1"),
            "缺少开启 HUD 的判据（内联脚本读 location.search）"
        );
        // 舞台本体必须铺满整个 iframe（不能有布局边）。
        assert!(html.contains("#stage { position:absolute;inset:0; }"));
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
}
