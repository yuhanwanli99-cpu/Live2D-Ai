//! /render 页面与 wasm-bindgen 产物文件名解析（0.4.7 rc.5）。
//!
//! trunk 0.22 的 release 产物里 **js 与 wasm 的文件名各自带内容 hash**，两者
//! 不同（2026-11-09 Windows 打包面实测：`l2d-wasm-demo-a5fb29b722ce6403.js`
//! 与 `l2d-wasm-demo-1abe76bb6b842901_bg.wasm`）。旧实现只从 `*_bg.wasm` 取
//! 一个 hash，再用同一个 hash 同时拼 js 与 wasm 的名字——js 因此指向**不存在的
//! 文件**（`/render/l2d-wasm-demo-<wasm-hash>.js` → 404），`init()` 失败，
//! 舞台永远停在「模型加载中…」（集成包 dist047 + Windows Chrome headless 复现）。
//!
//! 页面按 `dist/` 里**真实存在的文件名**生成，不猜 hash：先扫 `*_bg.wasm` 得到
//! crate 前缀，再在同一目录找同 crate 的 `.js` loader，两个真实文件名分别填进
//! `import init from ...` 与 `module_or_path`；缺 wasm 或缺 js 都回 503
//! `wasm_unavailable`，绝不生成注定失败的页面。
//!
//! 纯函数：入参只有 `dist` 目录路径，单测可用临时目录构造任意布局。

use std::path::Path;

/// 本页只服务 wasm-demo 这一个 crate 的产物（trunk 文件名前缀）。
const CRATE_PREFIX: &str = "l2d-wasm-demo-";

/// 一次渲染页面要用的**真实文件名**（相对 `dist/`，不带 `/render/` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderAssets {
    /// wasm-bindgen 的 JS loader，例如 `l2d-wasm-demo-<hash>.js`。
    pub js_file: String,
    /// wasm-bindgen 的 wasm 模块，例如 `l2d-wasm-demo-<另一个 hash>_bg.wasm`。
    pub wasm_file: String,
}

/// 在 `dist/` 里找**真实存在**的 js + wasm 文件名；缺任意一个 → `None`。
///
/// 文件名全部来自目录列表（不猜、不拼 hash）。同一 crate 有多组 hash 时优先
/// 同名 hash 的 `<prefix>.js`（旧布局 / 其他 bundler），否则取排序后的第一个
/// 同 crate js；两者都必须真实存在于目录里。
pub fn find_render_assets(dist_dir: &Path) -> Option<RenderAssets> {
    let entries = std::fs::read_dir(dist_dir).ok()?;
    let mut wasm_files: Vec<String> = Vec::new();
    let mut js_files: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with("_bg.wasm") {
            wasm_files.push(name);
        } else if name.ends_with(".js") {
            js_files.push(name);
        }
    }
    wasm_files.sort();
    js_files.sort();
    for wasm_file in &wasm_files {
        let Some(prefix) = wasm_file.strip_suffix("_bg.wasm") else {
            continue;
        };
        if !prefix.starts_with(CRATE_PREFIX) {
            continue; // 不是本 crate 的产物
        }
        // 1) 同名 hash 的 js（旧布局 / 其他 bundler）；2) 否则同 crate 的第一个 js。
        let same_hash_js = format!("{prefix}.js");
        let js_file = if js_files.contains(&same_hash_js) {
            same_hash_js
        } else {
            match js_files.iter().find(|js| {
                js.strip_suffix(".js")
                    .is_some_and(|stem| stem.starts_with(CRATE_PREFIX))
            }) {
                Some(js) => js.clone(),
                None => continue,
            }
        };
        return Some(RenderAssets {
            js_file,
            wasm_file: wasm_file.clone(),
        });
    }
    None
}

/// 渲染页 HTML（runtime 生成，引用 dist 下**真实的** wasm-bindgen 产物）。
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
pub fn render_html(assets: &RenderAssets) -> String {
    let RenderAssets { js_file, wasm_file } = assets;
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
 /* 默认 display:none。要看它：地址后加 ?hud=1（见 body 上的内联脚本）。
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
 /* 只在 ?hud=1 时显示状态栏。内联，不引任何依赖。 */
 if (location.search.indexOf('hud=1') >= 0) {{ document.body.classList.add('hud'); }}
</script>
<script type="module">
import init from '/render/{js_file}';
await init({{ module_or_path: '/render/{wasm_file}' }});
</script>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 trunk release 布局：js 与 wasm 是两个不同的内容 hash。
    fn assets() -> RenderAssets {
        RenderAssets {
            js_file: "l2d-wasm-demo-a5fb29b722ce6403.js".to_string(),
            wasm_file: "l2d-wasm-demo-1abe76bb6b842901_bg.wasm".to_string(),
        }
    }

    fn temp_dist(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "l2d-render-page-{}-{}-{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dist");
        root
    }

    /// ① js 与 wasm hash 不同 → HTML 分别引用两个**真实**文件名（旧 bug 回归）。
    #[test]
    fn render_html_uses_real_js_and_wasm_names() {
        let html = render_html(&assets());
        assert!(
            html.contains("/render/l2d-wasm-demo-a5fb29b722ce6403.js"),
            "import 必须引用真实 js 名: {html}"
        );
        assert!(
            html.contains("/render/l2d-wasm-demo-1abe76bb6b842901_bg.wasm"),
            "module_or_path 必须引用真实 wasm 名: {html}"
        );
        // 旧 bug：用 wasm 的 hash 去拼 js 名 → 404 → 舞台停在「模型加载中」。
        assert!(
            !html.contains("/render/l2d-wasm-demo-1abe76bb6b842901.js"),
            "不得假设 js 与 wasm 的 hash 相同: {html}"
        );
        assert!(html.contains("id=\"canvas\""));
        assert!(html.contains("module_or_path"));
    }

    /// 状态栏**默认不可见**（2026-09-11 用户裁决「舞台背影全黑/全白即可」）。
    ///
    /// 这条守的是「舞台底部不会出现一块深色横条」：`#status` 的样式里必须有
    /// `display:none`，且只有 `body.hud` 才把它打开——而 `body.hud` 只在
    /// `?hud=1` 时由内联脚本加上。
    #[test]
    fn render_html_hides_hud_unless_explicitly_requested() {
        let html = render_html(&assets());
        assert!(
            html.contains("#status { display:none;"),
            "#status 必须默认隐藏，否则舞台底部会有一条深色横条"
        );
        assert!(
            html.contains("body.hud #status { display:block; }"),
            "必须保留「?hud=1」这条调试入口"
        );
        assert!(
            html.contains("hud=1"),
            "缺少开启 HUD 的判据（内联脚本读 location.search）"
        );
        // 舞台本体必须铺满整个 iframe（不能有布局边）。
        assert!(html.contains("#stage { position:absolute;inset:0; }"));
    }

    /// dist 里的真实文件名（两 hash 不同）→ 按文件解析，不拼 hash。
    #[test]
    fn find_render_assets_accepts_differing_hashes() {
        let dir = temp_dist("pair");
        std::fs::write(dir.join("l2d-wasm-demo-a5fb29b722ce6403.js"), b"js").expect("js");
        std::fs::write(dir.join("l2d-wasm-demo-1abe76bb6b842901_bg.wasm"), b"wasm").expect("wasm");
        let found = find_render_assets(&dir).expect("两个真实文件都在");
        assert_eq!(found, assets());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ② 只有 wasm 没有 js → 产物不完整，返回 None（调用方回 503）。
    #[test]
    fn find_render_assets_requires_a_real_js() {
        let dir = temp_dist("wasm-only");
        std::fs::write(dir.join("l2d-wasm-demo-1abe76bb6b842901_bg.wasm"), b"wasm").expect("wasm");
        assert!(
            find_render_assets(&dir).is_none(),
            "只有 wasm 没有 js 时不得生成页面"
        );
        // 只有 js 没有 wasm 同理。
        let dir2 = temp_dist("js-only");
        std::fs::write(dir2.join("l2d-wasm-demo-a5fb29b722ce6403.js"), b"js").expect("js");
        assert!(find_render_assets(&dir2).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    /// 同名 hash 的 js 优先（旧布局 / 其他 bundler）；别的 crate 的产物不认。
    #[test]
    fn find_render_assets_prefers_same_hash_and_ignores_other_crates() {
        let dir = temp_dist("same-hash");
        std::fs::write(dir.join("l2d-wasm-demo-abc.js"), b"js-abc").expect("js abc");
        std::fs::write(dir.join("l2d-wasm-demo-def.js"), b"js-def").expect("js def");
        std::fs::write(dir.join("l2d-wasm-demo-abc_bg.wasm"), b"wasm").expect("wasm");
        let found = find_render_assets(&dir).expect("同 hash 对");
        assert_eq!(found.js_file, "l2d-wasm-demo-abc.js");
        assert_eq!(found.wasm_file, "l2d-wasm-demo-abc_bg.wasm");
        let _ = std::fs::remove_dir_all(&dir);

        // 不是本 crate 的 wasm：不认（哪怕目录里恰好有本 crate 的 js）。
        let other = temp_dist("other-crate");
        std::fs::write(other.join("other-crate-abc_bg.wasm"), b"wasm").expect("wasm");
        std::fs::write(other.join("l2d-wasm-demo-abc.js"), b"js").expect("js");
        assert!(find_render_assets(&other).is_none());
        let _ = std::fs::remove_dir_all(&other);
    }

    /// 目录不存在 / 空目录 → None（不 panic）。
    #[test]
    fn find_render_assets_handles_missing_dir() {
        let missing = std::env::temp_dir().join("l2d-render-page-does-not-exist-047");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(find_render_assets(&missing).is_none());
    }
}
