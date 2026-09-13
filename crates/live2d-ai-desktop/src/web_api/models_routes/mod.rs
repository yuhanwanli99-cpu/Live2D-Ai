//! 模型资产路由（`/api/v1/models*`，D3 落地，2026-08-28）。
//!
//! 设计要点（裁决 `docs/plans/node-d-d3-rendering-route.md`）：
//! - **本地导入优先**：本批**只**接受"受控本地导入"——body 指定
//!   `assets/models/<id>/` 下的相对路径（id 形式），后端校验合法
//!   model3 目录后登记到 registry。ZIP 上传（前端拖入）标 D3.2 后置。
//! - **路径安全**（P0-4）：拒绝 `..`、绝对路径、符号链接外指；统一用
//!   [`registry::normalize_relative_id`] 做规范化。
//! - **registry 原子写回**：复用 `runtime::settings::patch::plan_atomic_write`
//!   语义（写 tmp → `fdatasync` → rename）——`registry::atomic_write_json` 包装。
//! - **激活模型不可删**：DELETE 在 `active_id == id` 时回 409 model_active
//!   （与 D1 §1.3 冻结一致）。
//! - **mutating 安全**：`POST /api/v1/models*` 系列加进
//!   `is_mutating_route` 列表（`security.rs` 单点修改），dispatch 入口
//!   自动做 Origin + Content-Type 校验。
//!
//! 文件拆分：
//! - [`mod@registry`]：路径安全、原子写回、DTO 类型、ModelRegistry。
//! - [`mod@handlers`]：handler 函数 + 错误响应 helper。
//! - 本文件：路由表 + 顶层 dispatch + ModelStore 共享态。
//!
//! 端点（与 D1 §1.3 字段对齐表 §6.3 一致）：
//! - `GET    /api/v1/models`              → list
//! - `POST   /api/v1/models/import`       → import_local (body: {"id": "bai"})
//! - `GET    /api/v1/models/{id}`         → get
//! - `DELETE /api/v1/models/{id}`         → delete
//! - `POST   /api/v1/models/{id}/activate`→ activate
//! - `PATCH  /api/v1/models/{id}/display`→ display
//!
//! 注：D1 §1.3 冻结的端点表里 `POST /api/v1/models` (multipart zip) 与
//! `PATCH /api/v1/models/{id}` (display_name) 标 D3.2 后置；本批**不**实现。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tiny_http::{Method, Response};

pub mod dto;
pub mod handlers;
pub mod registry;
mod util;

#[cfg(test)]
mod tests_models_common;
#[cfg(test)]
mod tests_models_core;
#[cfg(test)]
mod tests_models_handlers;
#[cfg(test)]
mod tests_models_p1;

pub(crate) use registry::{ModelRegistry, atomic_write_json};

use handlers::{
    handle_activate, handle_delete, handle_display, handle_get, handle_import, handle_list,
};

// =====================================================================
// 路由表（method+path → 子路由 ID）
// =====================================================================

/// 模型子路由 ID（models_routes 内部细分；不污染 web_api::RouteId 命名空间）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelRouteId {
    List,
    Import,
    Get,
    Delete,
    Activate,
    Display,
}

/// 解析 models 子路由（含路径参数 `{id}`）。
pub(crate) fn match_model_route(method: &Method, path: &str) -> Option<ModelRouteId> {
    let prefix = "/api/v1/models";
    if path == prefix && *method == Method::Get {
        return Some(ModelRouteId::List);
    }
    if path == "/api/v1/models/import" {
        if *method == Method::Post {
            return Some(ModelRouteId::Import);
        }
        return None;
    }
    let rest = path.strip_prefix(prefix)?.strip_prefix('/')?;
    if rest.is_empty() {
        return None;
    }
    let mut parts = rest.split('/');
    let id = parts.next()?;
    if id.is_empty() || id.contains("..") || id.starts_with('/') || id.contains('\\') {
        return None;
    }
    // `id` 段不允许保留关键字 `import`（与 `/api/v1/models/import` 同名冲突）。
    if id == "import" {
        return None;
    }
    match parts.next() {
        None => {
            if *method == Method::Get {
                Some(ModelRouteId::Get)
            } else if *method == Method::Delete {
                Some(ModelRouteId::Delete)
            } else {
                None
            }
        }
        Some("activate") => {
            if *method == Method::Post && parts.next().is_none() {
                Some(ModelRouteId::Activate)
            } else {
                None
            }
        }
        Some("display") => {
            if *method == Method::Patch && parts.next().is_none() {
                Some(ModelRouteId::Display)
            } else {
                None
            }
        }
        _ => None,
    }
}

// =====================================================================
// 跨 handler 共享：ModelStore
// =====================================================================

/// 共享模型注册表 + assets 根目录。
#[derive(Clone)]
pub struct ModelStore {
    pub inner: Arc<RwLock<ModelRegistry>>,
    pub assets_root: PathBuf,
    pub registry_path: PathBuf,
}

impl ModelStore {
    pub fn new(assets_root: PathBuf, registry_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ModelRegistry::default())),
            assets_root,
            registry_path,
        }
    }

    pub fn from_disk(assets_root: PathBuf, registry_path: PathBuf) -> Self {
        let store = Self::new(assets_root, registry_path);
        match ModelRegistry::load_from_path(&store.registry_path) {
            Ok(reg) => {
                if let Ok(mut g) = store.inner.write() {
                    *g = reg;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // 首次启动正常路径。
            }
            Err(e) => {
                // P1-5：损坏 registry 不再静默丢失——先备份原文件到
                // `<path>.corrupt-<unix_secs>`，再用空 registry 启动。
                // 这样用户可手工从备份恢复或审计内容；后续 persist 仍
                // 会覆盖 registry_path（损坏副本保留在 `.corrupt-*` 里）。
                let ts = util::now_unix_secs();
                let backup = append_corrupt_suffix(&store.registry_path, ts);
                match fs::copy(&store.registry_path, &backup) {
                    Ok(n) => eprintln!(
                        "[warn] registry 损坏（{}），已备份为 {}（{} 字节）：{e}。使用空 registry。",
                        store.registry_path.display(),
                        backup.display(),
                        n,
                    ),
                    Err(copy_err) => eprintln!(
                        "[warn] registry 损坏（{}），且备份失败（{}）：{copy_err}。原文件保留，使用空 registry：{e}",
                        store.registry_path.display(),
                        backup.display(),
                    ),
                }
            }
        }
        store
    }

    pub fn persist(&self) -> Result<(), String> {
        let snapshot = {
            let g = self
                .inner
                .read()
                .map_err(|e| format!("registry 锁毒化: {e}"))?;
            g.clone()
        };
        atomic_write_json(&self.registry_path, &snapshot)
    }
}

/// 模型根与 registry 路径——**都来自 [`crate::web_api::model_root`]**。
///
/// 2026-09-12（rc.2）：这里以前返回 XDG `~/.local/share/live2d-ai/models`（`assets_root`）
/// 与同目录下的 `model_registry.json`，而静态 `/models/*` 读的是 `<cwd>/assets/models/`
/// ——**两个根**，于是「导入并激活后皮套不换」。现在只有一个根：仓库 `assets/models/`。
/// XDG 那条路径已删除（本机该目录从未存在，无内容需迁移）。
/// `GET /api/v1/app/status` 的 `active_model_id`：**舞台此刻显示哪个模型**。
///
/// 语义不是「registry 里恰好写了什么」，而是「用户看到的是哪个」，逐级回落：
///
/// 1. registry 有 `active_id`（用户 activate 过）→ **就是它**；
/// 2. 否则回落到渲染面自带的默认模型（`l2d-wasm-demo::DEFAULT_MODEL_URL`
///    = `/models/bai/runtime/bai.model3.json`）——stage 首帧加载的就是它，
///    所以报它是**如实**；**且只在那个文件真的存在时**才报（否则就是新的假广告：
///    状态栏说有模型，舞台上什么都没有）；
/// 3. 两者都不成立 → `""`（诚实：没有模型可显示）。
///
/// 2026-09-12（rc.2）之前这里恒为 `"bai_001"`——一个 registry 里根本不存在的
/// 字符串，所以「激活了但状态栏不变」是必然的。
pub(crate) fn active_model_id(store: &ModelStore) -> String {
    let registered = store.inner.read().ok().and_then(|g| g.active_id.clone());
    pick_active_model_id(
        registered.as_deref(),
        crate::web_api::model_root::builtin_model_id(),
        crate::web_api::model_root::builtin_model3_path().is_file(),
    )
}

/// [`active_model_id`] 的纯决策内核（不碰注册表与文件系统，便于穷举用例）。
fn pick_active_model_id(
    registered: Option<&str>,
    builtin_id: &str,
    builtin_present: bool,
) -> String {
    if let Some(id) = registered
        && !id.is_empty()
    {
        return id.to_string();
    }
    if builtin_present && !builtin_id.is_empty() {
        return builtin_id.to_string();
    }
    String::new()
}

fn default_paths() -> (PathBuf, PathBuf) {
    (
        crate::web_api::model_root::model_root(),
        crate::web_api::model_root::registry_path(),
    )
}

/// 暴露给 `web_api::mod` 的默认 store（从磁盘加载）。
pub(crate) fn default_store() -> ModelStore {
    let (assets_root, registry_path) = default_paths();
    ModelStore::from_disk(assets_root, registry_path)
}

// =====================================================================
// 顶层 dispatch 入口
// =====================================================================

/// 顶层 dispatch 入口（仅命中本批支持的 6 条子路由）。
pub(crate) fn dispatch(
    store: &ModelStore,
    method: &Method,
    path: &str,
    body_str: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(sub) = match_model_route(method, path) else {
        return handlers::not_found_response("not_found", &format!("路径 {path:?} 不存在"));
    };
    match sub {
        ModelRouteId::List => handle_list(store),
        ModelRouteId::Get => {
            let id = extract_id_from_get(path);
            handle_get(store, &id)
        }
        ModelRouteId::Delete => {
            let id = extract_id_from_get(path);
            handle_delete(store, &id)
        }
        ModelRouteId::Import => handle_import(store, body_str),
        ModelRouteId::Activate => {
            let id = extract_id_from_activate(path);
            handle_activate(store, &id)
        }
        ModelRouteId::Display => {
            let id = extract_id_from_display(path);
            handle_display(store, &id, body_str)
        }
    }
}

fn extract_id_from_get(path: &str) -> String {
    path.strip_prefix("/api/v1/models/")
        .unwrap_or("")
        .to_string()
}

fn extract_id_from_activate(path: &str) -> String {
    let rest = path.strip_prefix("/api/v1/models/").unwrap_or("");
    rest.strip_suffix("/activate").unwrap_or("").to_string()
}

fn extract_id_from_display(path: &str) -> String {
    let rest = path.strip_prefix("/api/v1/models/").unwrap_or("");
    rest.strip_suffix("/display").unwrap_or("").to_string()
}

/// 给损坏 registry 文件名追加 `.corrupt-<ts>` 后缀（保留下原始扩展名，
/// 便于手工查阅）。`ts` 通常是 unix 秒（u64/i64 都可以）。
fn append_corrupt_suffix(path: &Path, ts: i64) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".corrupt-{ts}"));
    PathBuf::from(s)
}

#[cfg(test)]
mod active_model_id_tests {
    use super::pick_active_model_id;

    #[test]
    fn registered_wins() {
        assert_eq!(pick_active_model_id(Some("neko"), "bai", true), "neko");
    }

    /// 没有激活过任何模型时，报**stage 实际会加载的那个**——而不是空串，
    /// 也不是一个 registry 里不存在的 id。
    #[test]
    fn falls_back_to_the_builtin_that_actually_exists() {
        assert_eq!(pick_active_model_id(None, "bai", true), "bai");
    }

    /// 内置模型文件不在磁盘上时**不许**报它：那会变成「状态栏说有模型、
    /// 舞台上什么都没有」的新假广告。
    #[test]
    fn missing_builtin_reports_nothing() {
        assert_eq!(pick_active_model_id(None, "bai", false), "");
        assert_eq!(pick_active_model_id(None, "", true), "");
    }

    #[test]
    fn empty_registered_value_is_treated_as_absent() {
        assert_eq!(pick_active_model_id(Some(""), "bai", true), "bai");
        assert_eq!(pick_active_model_id(Some(""), "bai", false), "");
    }
}
