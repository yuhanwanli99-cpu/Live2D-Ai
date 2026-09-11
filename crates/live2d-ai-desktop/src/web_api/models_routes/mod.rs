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
mod tests_models_routes;

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

fn default_paths() -> (PathBuf, PathBuf) {
    let data_dir = directories::ProjectDirs::from("dev", "live2d-ai", "live2d-ai")
        .map(|p| p.data_dir().to_path_buf())
        .or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .map(|p| p.join("live2d-ai"))
        })
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local");
                p.push("share");
                p.push("live2d-ai");
                p
            })
        })
        .unwrap_or_else(|| PathBuf::from("live2d-ai-data"));
    (
        data_dir.join("models"),
        data_dir.join("model_registry.json"),
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
