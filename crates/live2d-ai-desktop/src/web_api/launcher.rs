//! 启动器页（0.3.11）：`GET /` 与 `GET /index.html` 返回的这一页。
//!
//! 背景（执行单 `docs/plans/PLAN-0.3.11-launcher-2026-10-20.md` §0/§3）：
//! 0.3.11 撤掉「监听前自动预启动」，点火也不再为 local TTS 空等 600 秒。
//! 桌面进程先听 HTTP，`GET /` 是**启动前的一页**：
//!
//! - **已注册** = `GET /api/v1/mods` 的条数（只读；工厂 0 显示 0）；
//! - **本次确认** = 勾选条数；**本次拉起** = 预启动开着时，勾选里的
//!   `local-tts-melo` / `local-engine` 条数；
//! - 预启动缺省关，拨动它只改页面上的数字：不发请求、不写
//!   `live2d-ai.toml`，也不写 `mods.json`；
//! - 「进入」后按勾选调用**现有** `POST /api/v1/mods/{id}/{enable,disable}`，
//!   再对**当前这一份** `[llm]` / `[tts]` 的 `base_url` 各探测一次
//!   （复用现有 `POST /api/v1/settings/test/{llm,tts}` 的短超时与分类），
//!   然后 `location.assign('/app/')`。失败也进入。
//!
//! 探测**只报告、不改档**（执行单 §5）：0.3.9 之后 LLM / TTS 各只有一份
//! `base_url` 加一个 `mode` 标签，`vendors.*` 已删，没有「另一档」可探；
//! 本版不写 `live2d-ai.toml`、不改 `.env`、不换出站端点。
//!
//! # 范围
//!
//! 本模块只做**托管**：页面 HTML（`assets/launcher.html`，`include_str!`）
//! 与同源字体。页面逻辑是 HTML 里的内联 CSS/JS，**不新增任何 API 路由**——
//! 所有动作都打现有端点（`/api/v1/mods`、`/api/v1/mods/{id}/*`、
//! `/api/v1/app/status`、`/api/v1/settings/test/*`）。
//!
//! # 字体（离线红线）
//!
//! 页面用仓库已有的 Flutter 自托管子集
//! `shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.woff2`，由本服务
//! 同源发出。**禁止** `fonts.googleapis.com` / `fonts.gstatic.com`；
//! 页面里的汉字必须落在 `shell/flutter/assets/fonts/*.ranges.txt`——
//! `font_subset_test.dart` 只扫 Dart，所以本文件有一条 Rust 测试对 HTML 做
//! 同一道检查（含字体字节数与 FNV-1a 哈希比对，换字体没重生成就判红）。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use tiny_http::{Header, Method, Response, StatusCode};

/// 页面标记（`ignite.sh --check` / `ignition-precheck.sh` 用同一串 grep 判「这是
/// 启动器」）。生产代码不需要它，测试用它钉住「页面还得带着这串」——所以只在
/// `cfg(test)` 里编译，避免非测试构建死代码（clippy `-D warnings`）。
#[cfg(test)]
pub(crate) const LAUNCHER_MARKER: &str = "live2d-ai-launcher";

/// 内嵌页面（执行单 §3 指定位置：`crates/live2d-ai-desktop/assets/launcher.html`）。
const LAUNCHER_HTML: &str = include_str!("../../assets/launcher.html");

/// 字体文件名（仓库里已有的 Flutter 自托管子集）。
const FONT_NAME: &str = "NotoSansSC-AiSubset-Regular.woff2";

/// 字体 URL（页面 `@font-face` 引用同源地址）。
const FONT_URL: &str = "/launcher/NotoSansSC-AiSubset-Regular.woff2";

/// 字体在仓库里的相对路径（`scripts/ignite.sh` 从仓库根起服务）。
const FONT_REL: &str = "shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.woff2";

/// 是否为本模块负责的字体路径。
pub(crate) fn is_launcher_asset(path: &str) -> bool {
    path == FONT_URL
}

/// 根路径（`/` 与 `/index.html`）的 GET 响应：启动器页（200 + `no-store`）。
pub(crate) fn root_response() -> Response<Cursor<Vec<u8>>> {
    text_response(
        StatusCode(200),
        LAUNCHER_HTML.as_bytes().to_vec(),
        "text/html; charset=utf-8",
        "no-store",
    )
}

/// 本模块接管的字体请求。返回 `None` = 不是本模块路径（交还调用方）。
pub(crate) fn handle_launcher_asset(
    method: &Method,
    path: &str,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if !is_launcher_asset(path) {
        return None;
    }
    if *method != Method::Get {
        return Some(text_response(
            StatusCode(405),
            b"launcher font: GET only".to_vec(),
            "text/plain; charset=utf-8",
            "no-store",
        ));
    }
    match resolve_font() {
        Ok(bytes) => Some(text_response(
            StatusCode(200),
            bytes,
            "font/woff2",
            // 文件名固定、内容随字体更新；本机服务，长缓存没有收益上的坏处。
            "public, max-age=86400",
        )),
        Err(hint) => Some(text_response(
            StatusCode(404),
            hint.into_bytes(),
            "text/plain; charset=utf-8",
            "no-store",
        )),
    }
}

/// 找字体文件：先看 `LIVE2D_AI_FLUTTER_WEB_DIR`（已构建产物），再按仓库根
/// 相对路径找源文件。缺失时回 `Err`（带**找过的每个路径**，排障不留白）。
fn resolve_font() -> Result<Vec<u8>, String> {
    let candidates = font_candidates();
    for candidate in &candidates {
        if let Ok(bytes) = std::fs::read(candidate) {
            return Ok(bytes);
        }
    }
    let tried = candidates
        .iter()
        .map(|c| c.display().to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    Err(format!(
        "启动器字体缺失：{FONT_REL} 不在任何候选路径下（已找过：{tried}）。\
         本页仍可打开（浏览器会用本机字体），但离线红线要求同源自托管字体。"
    ))
}

/// 字体候选路径（按优先级；第一个存在的胜出）。
fn font_candidates() -> Vec<PathBuf> {
    let rel = Path::new(FONT_REL);
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("LIVE2D_AI_FLUTTER_WEB_DIR") {
        // Flutter 产物把 `assets/fonts/` 复制到 `build/web/assets/assets/fonts/`。
        out.push(Path::new(&dir).join("assets/assets/fonts").join(FONT_NAME));
    }
    out.push(rel.to_path_buf());
    out.push(Path::new("..").join(rel));
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && let Some(root) = crate::web_api::flutter_app::repo_root(dir)
    {
        out.push(root.join(rel));
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Some(root) = crate::web_api::flutter_app::repo_root(&cwd)
    {
        out.push(root.join(rel));
    }
    out
}

fn text_response(
    status: StatusCode,
    body: Vec<u8>,
    mime: &'static str,
    cache: &'static str,
) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).expect("Content-Type 头"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], cache.as_bytes()).expect("Cache-Control 头"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 页面是内嵌的那一份：根路径 200 的正文必须与 `include_str!` 逐字一致。
    #[test]
    fn root_serves_the_embedded_launcher_page() {
        let resp = root_response();
        assert_eq!(resp.status_code().0, 200, "GET / 必须是 200（不再是 302）");
        let headers = resp.headers();
        let ct = headers
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .map(|h| h.value.as_str().to_string())
            .expect("Content-Type 头");
        assert!(ct.starts_with("text/html"), "启动器必须是 HTML：{ct}");
        let cache = headers
            .iter()
            .find(|h| h.field.equiv("Cache-Control"))
            .map(|h| h.value.as_str().to_string())
            .expect("Cache-Control 头");
        assert_eq!(cache, "no-store", "启动器页不得被缓存");
        let mut body = String::new();
        use std::io::Read;
        resp.into_reader()
            .read_to_string(&mut body)
            .expect("读 body");
        assert_eq!(body, LAUNCHER_HTML, "根路径正文必须就是内嵌页面");
    }

    /// 页面标记：`ignite.sh --check` 与 `ignition-precheck.sh` 用同一串判「这是
    /// 启动器」。**改这里 = 同时改两个脚本的判据**。
    #[test]
    fn launcher_html_carries_the_marker() {
        assert!(
            LAUNCHER_HTML.contains(LAUNCHER_MARKER),
            "页面缺少启动器标记 {LAUNCHER_MARKER}"
        );
    }

    /// 离线红线：页面正文不得引用 Google CDN（字体 / CanvasKit 都不行）。
    #[test]
    fn launcher_html_has_no_google_cdn_reference() {
        for banned in [
            "gstatic.com/flutter-canvaskit",
            "fonts.googleapis.com",
            "fonts.gstatic.com",
            "www.gstatic.com",
        ] {
            assert!(
                !LAUNCHER_HTML.contains(banned),
                "启动器页不得引用 {banned}（离线红线）"
            );
        }
    }

    /// 页面只做「三个数字 + 预启动 + 勾选 + 进入」：**没有数字输入框**，
    /// 也没有任何地址 / 密钥 / 厂商 / 模型名编辑（执行单 §3）。
    #[test]
    fn launcher_html_has_no_number_or_endpoint_inputs() {
        let lower = LAUNCHER_HTML.to_ascii_lowercase();
        assert!(
            !lower.contains("type=\"number\""),
            "三个数字必须由列表算出，不能手填"
        );
        for banned in ["base_url", "api_key", "vendor", "model"] {
            assert!(
                !lower.contains(banned),
                "启动器页不得出现 {banned} 编辑（不是第二套设置）"
            );
        }
        // 三个数字的标签都在（只读展示 = `<dd>`，不是输入框）。
        for label in ["已注册", "本次确认", "本次拉起"] {
            assert!(LAUNCHER_HTML.contains(label), "缺少数字标签 {label}");
        }
        assert!(
            LAUNCHER_HTML.contains("type=\"checkbox\""),
            "确认只能通过勾选现有 id"
        );
    }

    /// 「本次拉起」只认两个会生进程的 id（执行单 §3）。
    #[test]
    fn launcher_html_knows_the_launch_class_ids() {
        for id in ["local-tts-melo", "local-engine"] {
            assert!(LAUNCHER_HTML.contains(id), "缺少拉起类 id {id}");
        }
    }

    /// 字体同源：页面引用 `/launcher/...woff2`，且该路由由本模块接管。
    #[test]
    fn launcher_html_uses_the_self_hosted_font() {
        assert!(
            LAUNCHER_HTML.contains(FONT_URL),
            "页面必须用同源自托管字体 {FONT_URL}"
        );
        assert!(is_launcher_asset(FONT_URL));
        assert!(!is_launcher_asset("/launcher/other.woff2"));
        assert!(!is_launcher_asset("/app/"));
    }

    /// **字体子集门禁**（`font_subset_test.dart` 只扫 Dart）：页面里的每个字符
    /// 都必须落在 `NotoSansSC-AiSubset-Regular.ranges.txt` 里，否则真机缺字会
    /// 去 `fonts.gstatic.com` 拉回退字体——有网看不见，断网就露馅。
    #[test]
    fn every_launcher_html_char_is_covered_by_the_font_subset() {
        let (ranges, bytes, fnv1a) = parse_ranges(&font_ranges_text());
        let missing: Vec<char> = LAUNCHER_HTML
            .chars()
            .filter(|c| !matches!(c, '\n' | '\r' | '\t'))
            .filter(|c| !in_ranges(&ranges, *c as u32))
            .collect();
        assert!(
            missing.is_empty(),
            "启动器页有子集外字符（会触发 gstatic 回退字体）：{:?}",
            missing
        );
        // 覆盖表是从字体文件派生的：字节数与 FNV-1a 必须对上，换了字体没重生成
        // 覆盖表就判红（与 Dart 侧同一条纪律）。
        let font = std::fs::read(font_path()).expect("字体文件必须存在");
        assert_eq!(font.len() as u64, bytes, "字体字节数与 ranges 表不符");
        assert_eq!(fnv1a32(&font), fnv1a, "字体 FNV-1a 与 ranges 表不符");
    }

    /// 字体路由：同源发出 `font/woff2`，内容与仓库文件逐字节一致。
    #[test]
    fn launcher_font_route_serves_the_repo_font() {
        let resp = handle_launcher_asset(&Method::Get, FONT_URL).expect("本模块接管字体路径");
        assert_eq!(resp.status_code().0, 200);
        let headers = resp.headers();
        let ct = headers
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .map(|h| h.value.as_str().to_string())
            .expect("Content-Type 头");
        assert_eq!(ct, "font/woff2");
        let mut body = Vec::new();
        use std::io::Read;
        resp.into_reader().read_to_end(&mut body).expect("读字体");
        assert_eq!(
            body,
            std::fs::read(font_path()).expect("字体文件必须存在"),
            "字体路由必须原样发出仓库里的子集字体"
        );
    }

    /// 非字体的 `/launcher/*` 不接管；字体路径的非 GET 回 405。
    #[test]
    fn launcher_asset_scope_is_exact() {
        assert!(handle_launcher_asset(&Method::Get, "/launcher/x.woff2").is_none());
        let resp = handle_launcher_asset(&Method::Post, FONT_URL).expect("字体路径归本模块");
        assert_eq!(resp.status_code().0, 405);
    }

    // ---- 子集覆盖表解析 / 路径工具 -----------------------------------------

    fn font_ranges_text() -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.ranges.txt"),
        )
        .expect("ranges.txt 必须存在（scripts/font_subset_ranges.py 生成）")
    }

    fn font_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.woff2")
    }

    /// 解析 `.ranges.txt`：返回（闭区间表, 字体字节数, FNV-1a）。
    fn parse_ranges(text: &str) -> (Vec<(u32, u32)>, u64, u32) {
        let mut ranges = Vec::new();
        let mut bytes = None;
        let mut fnv = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("# bytes:") {
                bytes = rest.trim().parse::<u64>().ok();
                continue;
            }
            if let Some(rest) = line.strip_prefix("# fnv1a32:") {
                fnv = u32::from_str_radix(rest.trim(), 16).ok();
                continue;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (lo, hi) = match line.split_once('-') {
                Some((a, b)) => (
                    u32::from_str_radix(a, 16).expect("区间下界"),
                    u32::from_str_radix(b, 16).expect("区间上界"),
                ),
                None => {
                    let cp = u32::from_str_radix(line, 16).expect("单码点");
                    (cp, cp)
                }
            };
            ranges.push((lo, hi));
        }
        (
            ranges,
            bytes.expect("ranges.txt 缺少 # bytes:"),
            fnv.expect("ranges.txt 缺少 # fnv1a32:"),
        )
    }

    fn in_ranges(ranges: &[(u32, u32)], cp: u32) -> bool {
        ranges
            .binary_search_by(|(lo, hi)| {
                if cp < *lo {
                    std::cmp::Ordering::Greater
                } else if cp > *hi {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    fn fnv1a32(data: &[u8]) -> u32 {
        let mut h: u32 = 0x811C_9DC5;
        for b in data {
            h ^= u32::from(*b);
            h = h.wrapping_mul(0x0100_0193);
        }
        h
    }
}
