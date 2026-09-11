//! 模型路由 handlers（HTTP handler + DTO + 错误响应 helper）。
//!
//! 拆分承载：`mod.rs` 路由表 + 本文件 handler 主体；`registry.rs` 负责
//! 数据层（持久化 / 路径安全 / DTO 类型）。
//!
//! 关键不变量：
//! - 所有 mutating 路径都走 [`super::match_model_route`] 的 method 校验
//!   （路由表 + 路径段解析）；
//! - 所有 `id` 都先经 [`super::registry::normalize_relative_id`] 规范化
//!   ——handler 内**不**接受任意字符串；
//! - 写盘失败回滚内存状态（见 `handle_import` / `handle_activate` /
//!   `handle_delete` / `handle_display`）；
//! - 错误响应一律走 [`bad_request`] / [`not_found_response`] /
//!   [`conflict_response`] / [`persist_failed_response`] 四个 helper。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use tiny_http::{Header, Response, StatusCode};

use crate::web_api::app_routes::json_response;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

use super::ModelStore;
use super::dto::{
    ActivateResponse, DisplayPatchBody, ImportRequest, ImportResponse, LayoutDto, ModelListItem,
    ModelListResponse,
};
use super::registry::{
    ModelDisplay, ModelEntry, ModelRegistry, ModelSource, StoredManifest, default_stage,
    normalize_relative_id,
};
use super::util::{compute_dir_size, find_model3_json, now_iso8601};

// =====================================================================
// 路径安全：拒绝 `..` / 绝对路径 / 符号链接外指
// =====================================================================

/// 把 id + assets 根路径解析为安全绝对路径。
///
/// - id 先经 [`normalize_relative_id`] 规范化；
/// - 拼接后用 `canonicalize` 解析符号链接；
/// - 校验最终路径必须以 `assets_root` 的 `canonicalize` 结果为前缀
///   （防 `..` 跳出、符号链接外指）。
/// - 失败原因用字符串携带（handler 映射到 400 invalid_resource_path）。
pub(crate) fn resolve_safe_path(assets_root: &Path, id: &str) -> Result<PathBuf, String> {
    let normalized = normalize_relative_id(id).map_err(|e| format!("id 非法（{e}）: {id:?}"))?;
    let candidate = assets_root.join(&normalized);
    let canonical_root = assets_root
        .canonicalize()
        .map_err(|e| format!("assets_root 无法规范化: {e}"))?;
    let canonical_candidate = candidate.canonicalize().or_else(|_| {
        // 不存在：退化为"父目录规范 + 拼剩余段"。
        let parent = candidate
            .parent()
            .ok_or_else(|| format!("路径无父目录: {}", candidate.display()))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| format!("父目录无法规范化（{}）: {e}", parent.display()))?;
        let name = candidate
            .file_name()
            .ok_or_else(|| format!("路径无末段: {}", candidate.display()))?;
        Ok::<PathBuf, String>(canonical_parent.join(name))
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!(
            "路径越界：{} 不在 {} 之下",
            canonical_candidate.display(),
            canonical_root.display()
        ));
    }
    Ok(canonical_candidate)
}

// =====================================================================
// Handlers
// =====================================================================

/// `GET /api/v1/models` 处理器。
pub(crate) fn handle_list(store: &ModelStore) -> Response<Cursor<Vec<u8>>> {
    let snapshot = match store.inner.read() {
        Ok(g) => g.clone(),
        Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
    };
    let mut items: Vec<ModelListItem> = snapshot
        .entries
        .values()
        .map(|e| entry_to_list_item(store, e, snapshot.active_id.as_deref()))
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    json_response(StatusCode(200), &ModelListResponse { models: items })
}

fn entry_to_list_item(
    store: &ModelStore,
    entry: &ModelEntry,
    active_id: Option<&str>,
) -> ModelListItem {
    // 绝对路径每次由 assets_root + 相对路径派生（P1-4：避免 WSL/HOME
    // 迁移后旧 registry 失效）。
    let abs_dir = store.assets_root.join(&entry.relative_dir);
    let size = compute_dir_size(&abs_dir).unwrap_or(0);
    ModelListItem {
        id: entry.id.clone(),
        display_name: entry.display_name.clone(),
        version: entry.manifest.version,
        layout: LayoutDto::from(&entry.manifest.layout),
        moc3_file: entry.manifest.moc3_file.clone(),
        texture_files: entry.manifest.texture_files.clone(),
        has_physics: entry.manifest.physics_file.is_some(),
        has_display_info: entry.manifest.display_info_file.is_some(),
        active: active_id == Some(entry.id.as_str()),
        imported_at: entry.imported_at.clone(),
        size_bytes: size,
    }
}

/// `GET /api/v1/models/{id}` 处理器。
pub(crate) fn handle_get(store: &ModelStore, id: &str) -> Response<Cursor<Vec<u8>>> {
    let normalized = match normalize_relative_id(id) {
        Ok(s) => s,
        Err(e) => return bad_request("invalid_resource_path", &format!("id 非法: {e}")),
    };
    let snapshot = match store.inner.read() {
        Ok(g) => g.clone(),
        Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
    };
    match snapshot.entries.get(&normalized) {
        Some(e) => {
            let item = entry_to_list_item(store, e, snapshot.active_id.as_deref());
            json_response(StatusCode(200), &item)
        }
        None => not_found_response("model_not_found", &format!("模型 {normalized:?} 不存在")),
    }
}

/// `POST /api/v1/models/import` 处理器（受控本地导入）。
pub(crate) fn handle_import(store: &ModelStore, body_str: &str) -> Response<Cursor<Vec<u8>>> {
    let req: ImportRequest = match serde_json::from_str(body_str) {
        Ok(r) => r,
        Err(e) => {
            return bad_request("invalid_payload", &format!("请求 body JSON 解析失败: {e}"));
        }
    };
    let id = match normalize_relative_id(&req.id) {
        Ok(s) => s,
        Err(e) => return bad_request("invalid_resource_path", &format!("id 非法: {e}")),
    };

    let abs_dir = match resolve_safe_path(&store.assets_root, &id) {
        Ok(p) => p,
        Err(e) => return bad_request("invalid_resource_path", &e),
    };
    if !abs_dir.is_dir() {
        return bad_request(
            "invalid_resource_path",
            &format!("目录不存在: {}", abs_dir.display()),
        );
    }

    let model3_path = match find_model3_json(&abs_dir) {
        Some(p) => p,
        None => {
            return bad_request(
                "invalid_model3_json",
                &format!("目录 {} 下找不到 *.model3.json", abs_dir.display()),
            );
        }
    };
    let package = match l2d::asset::ModelPackage::load(&model3_path) {
        Ok(p) => p,
        Err(e) => {
            return bad_request(
                "invalid_model3_json",
                &format!("ModelPackage::load 失败: {e}"),
            );
        }
    };

    let manifest = StoredManifest::from(package.manifest());
    // P1-4：只存相对 `assets_root` 的路径；handler 端按需派生绝对路径。
    // `model3_path` 必须在 `abs_dir` 之下（`find_model3_json` 只扫
    // `abs_dir` 顶层），因此相对路径 = `id/<file_name>`。
    let model3_rel_path = format!(
        "{}/{}",
        id,
        model3_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("model.model3.json"),
    );
    let new_entry = ModelEntry {
        id: id.clone(),
        display_name: id.clone(),
        source: ModelSource::LocalImport,
        relative_dir: id.clone(),
        model3_rel_path: model3_rel_path.clone(),
        manifest,
        display: ModelDisplay::default(),
        stage: default_stage(),
        imported_at: now_iso8601(),
    };
    // 关键：先在写锁中插入（同时判重），**释放写锁**再调 `persist`。
    {
        let mut g = match store.inner.write() {
            Ok(g) => g,
            Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
        };
        if g.entries.contains_key(&id) {
            return conflict_response(
                "id_conflict",
                &format!("模型 {id:?} 已存在；v1 不支持 overwrite"),
            );
        }
        g.entries.insert(id.clone(), new_entry);
    }
    if let Err(e) = store.persist() {
        // 回滚：内存改回原状态。
        if let Ok(mut g) = store.inner.write() {
            g.entries.remove(&id);
        }
        return persist_failed_response(&e);
    }
    // `ImportResponse.model3_json_path` 保持原 D1 §6.3 契约：相对
    // `data_dir`（`assets_root` 父目录），不是相对 `assets_root`。
    let parent_data = store.assets_root.parent().unwrap_or(&store.assets_root);
    let rel_model3 = model3_path
        .strip_prefix(parent_data)
        .unwrap_or(&model3_path)
        .to_string_lossy()
        .to_string();
    json_response(
        StatusCode(200),
        &ImportResponse {
            id,
            model3_json_path: rel_model3,
            applied: false,
        },
    )
}

/// `POST /api/v1/models/{id}/activate` 处理器。
pub(crate) fn handle_activate(store: &ModelStore, id: &str) -> Response<Cursor<Vec<u8>>> {
    let normalized = match normalize_relative_id(id) {
        Ok(s) => s,
        Err(e) => return bad_request("invalid_resource_path", &format!("id 非法: {e}")),
    };
    // 关键：必须先**释放**写锁再 `persist`（`persist` 内部 `read()` 在
    // 写锁未释放时死锁——`std::sync::RwLock` 不支持递归）。先把要返回的
    // 数据从写锁中克隆出来，释放写锁后再做副作用（原子写盘）。
    let (prev, model_url): (Option<String>, String) = {
        let mut g = match store.inner.write() {
            Ok(g) => g,
            Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
        };
        if !g.entries.contains_key(&normalized) {
            return not_found_response("model_not_found", &format!("模型 {normalized:?} 不存在"));
        }
        let prev = g.active_id.clone();
        // 克隆 model3_rel_path 用于构造 runtime URL（在释放写锁前）。
        let model_url = g
            .entries
            .get(&normalized)
            .map(|e| format!("/models/{}", e.model3_rel_path))
            .unwrap_or_else(|| format!("/models/{normalized}/runtime/{normalized}.model3.json"));
        g.active_id = Some(normalized.clone());
        (prev, model_url)
    };
    // 写锁已释放；现在调 `persist` 安全。
    if let Err(e) = store.persist() {
        // 回滚 active_id（重新拿写锁）。
        if let Ok(mut g) = store.inner.write() {
            g.active_id = prev;
        }
        return persist_failed_response(&e);
    }
    json_response(
        StatusCode(200),
        &ActivateResponse {
            active_id: normalized,
            prev_active_id: prev,
            // D3.2 热重载完成前，激活只改 registry 不重载 renderer/supervisor，
            // 如实上报 requires_restart=true（避免前端误以为已生效）。
            requires_restart: true,
            model_url,
        },
    )
}

/// `PATCH /api/v1/models/{id}/display` 处理器。
pub(crate) fn handle_display(
    store: &ModelStore,
    id: &str,
    body_str: &str,
) -> Response<Cursor<Vec<u8>>> {
    let normalized = match normalize_relative_id(id) {
        Ok(s) => s,
        Err(e) => return bad_request("invalid_resource_path", &format!("id 非法: {e}")),
    };
    let body: DisplayPatchBody = match serde_json::from_str(body_str) {
        Ok(b) => b,
        Err(e) => {
            return bad_request("invalid_payload", &format!("请求 body 解析失败: {e}"));
        }
    };
    // 产品级最小校验（scale>0、width/height>0、opacity∈[0,1]、hex color）
    // —— 失败 → 400 invalid_params（与 invalid_payload 区分：前者表结构对、
    // 值越界；后者表结构错）。
    if let Err(e) = body.validate() {
        return bad_request("invalid_params", &e);
    }
    // 关键：先在写锁中完成内存变更，**释放写锁**再调 `persist`。
    let updated_entry: Option<ModelEntry> = {
        let mut g = match store.inner.write() {
            Ok(g) => g,
            Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
        };
        let entry = match g.entries.get_mut(&normalized) {
            Some(e) => e,
            None => {
                return not_found_response(
                    "model_not_found",
                    &format!("模型 {normalized:?} 不存在"),
                );
            }
        };
        if let Some(model_patch) = body.model {
            model_patch.apply_into(&mut entry.display);
        }
        if let Some(stage_patch) = body.stage {
            stage_patch.apply_into(&mut entry.stage);
        }
        Some(entry.clone())
    };
    if let Err(e) = store.persist() {
        // 写盘失败：reload 内存 state（best-effort：从磁盘读回）。
        if let Ok(reloaded) = ModelRegistry::load_from_path(&store.registry_path)
            && let Ok(mut g) = store.inner.write()
        {
            *g = reloaded;
        }
        return persist_failed_response(&e);
    }
    // 200 + 回最新 entry 摘要。
    let active = store.inner.read().ok().and_then(|g| g.active_id.clone());
    let item = entry_to_list_item(
        store,
        &updated_entry.expect("entry just set"),
        active.as_deref(),
    );
    json_response(StatusCode(200), &item)
}

/// `DELETE /api/v1/models/{id}` 处理器。
pub(crate) fn handle_delete(store: &ModelStore, id: &str) -> Response<Cursor<Vec<u8>>> {
    let normalized = match normalize_relative_id(id) {
        Ok(s) => s,
        Err(e) => return bad_request("invalid_resource_path", &format!("id 非法: {e}")),
    };
    // 关键：先在写锁中完成内存变更，**释放写锁**再调 `persist`（避免死锁）。
    let prev_entry: Option<ModelEntry> = {
        let mut g = match store.inner.write() {
            Ok(g) => g,
            Err(e) => return persist_failed_response(&format!("registry 锁毒化: {e}")),
        };
        if !g.entries.contains_key(&normalized) {
            return not_found_response("model_not_found", &format!("模型 {normalized:?} 不存在"));
        }
        if g.active_id.as_deref() == Some(normalized.as_str()) {
            return conflict_response(
                "model_active",
                "激活中的模型不可删除；请先 activate 另一个模型",
            );
        }
        g.entries.remove(&normalized)
    };
    if let Err(e) = store.persist() {
        // 回滚：把删除的 entry 插回去。
        if let Some(e) = prev_entry
            && let Ok(mut g) = store.inner.write()
        {
            g.entries.insert(normalized.clone(), e);
        }
        return persist_failed_response(&e);
    }
    Response::from_data(Vec::new()).with_status_code(StatusCode(204))
}

// ----- 错误响应 helper -----

pub(crate) fn bad_request(code: &'static str, message: &str) -> Response<Cursor<Vec<u8>>> {
    json_error_response(StatusCode(400), code, message)
}

pub(crate) fn not_found_response(code: &'static str, message: &str) -> Response<Cursor<Vec<u8>>> {
    json_error_response(StatusCode(404), code, message)
}

pub(crate) fn conflict_response(code: &'static str, message: &str) -> Response<Cursor<Vec<u8>>> {
    json_error_response(StatusCode(409), code, message)
}

pub(crate) fn persist_failed_response(message: &str) -> Response<Cursor<Vec<u8>>> {
    json_error_response(StatusCode(500), "persist_failed", message)
}

fn json_error_response(
    status: StatusCode,
    code: &'static str,
    message: &str,
) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(code, message);
    let body = serde_json::to_vec(&ErrorResponse::new(detail)).unwrap_or_default();
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type"),
        )
}
