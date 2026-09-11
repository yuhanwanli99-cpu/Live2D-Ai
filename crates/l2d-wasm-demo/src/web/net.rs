//! WASM 端网络 / URL / 清单解析工具（仅在 `target_arch = "wasm32"` 下编译）。
//!
//! 承载：
//! - [`fetch_bytes`]：fetch URL → 完整字节（非 2xx 视为错误）。
//! - [`model_url_from_query`]：从 `?model=` 取清单地址，缺省回退 [`super::DEFAULT_MODEL_URL`]。
//! - [`resolve_relative`]：把清单相对引用拼到 model3 URL 所在目录下。
//! - [`extract_refs`] / [`ModelRefs`]：model3.json 的 `FileReferences` 提取。

use std::collections::BTreeSet;

use js_sys::Uint8Array;
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

use super::DEFAULT_MODEL_URL;

/// fetch 一个 URL 并返回完整字节（非 2xx 视为错误，携带状态码）。
pub(crate) async fn fetch_bytes(window: &Window, url: &str) -> Result<Vec<u8>, String> {
    let promise = window.fetch_with_str(url);
    let resp_value = JsFuture::from(promise).await.map_err(|v| js_str(&v))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "fetch 响应不是 Response 对象".to_owned())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buf_promise = resp.array_buffer().map_err(|v| js_str(&v))?;
    let buf = JsFuture::from(buf_promise).await.map_err(|v| js_str(&v))?;
    Ok(Uint8Array::new(&buf).to_vec())
}

pub(crate) fn js_str(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

/// query `?model=<url>`（缺省 [`DEFAULT_MODEL_URL`]；空值同样回退缺省）。
pub(crate) fn model_url_from_query(window: &Window) -> String {
    let search = window.location().search().unwrap_or_default();
    let trimmed = search.strip_prefix('?').unwrap_or(&search);
    if let Ok(params) = web_sys::UrlSearchParams::new_with_str(trimmed) {
        if let Some(m) = params.get("model") {
            if !m.is_empty() {
                return m;
            }
        }
    }
    DEFAULT_MODEL_URL.to_owned()
}

/// 把清单相对引用拼到 model3 URL 所在目录下（只做目录拼接，不改写查询串）。
pub(crate) fn resolve_relative(model_url: &str, rel: &str) -> String {
    let dir = match model_url.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    };
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{dir}/{rel}")
    }
}

/// model3.json 的 `FileReferences` 提取（demo 只负责取 URL 集合；
/// 严格的清单解析与字节组装全部交给 `ModelPackage::from_memory`）。
pub(crate) struct ModelRefs {
    pub(crate) moc: String,
    pub(crate) textures: Vec<String>,
    pub(crate) physics: Option<String>,
    pub(crate) cdi: Option<String>,
}

impl ModelRefs {
    /// 去重后的全部引用（保持首次出现顺序）。
    pub(crate) fn unique_paths(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        let all = self
            .textures
            .iter()
            .chain(self.physics.iter())
            .chain(self.cdi.iter())
            .chain(std::iter::once(&self.moc));
        for path in all {
            if seen.insert(path.as_str()) {
                ordered.push(path.clone());
            }
        }
        ordered
    }
}

pub(crate) fn extract_refs(bytes: &[u8]) -> Result<ModelRefs, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("model3.json 不是合法 JSON：{e}"))?;
    let refs = value
        .get("FileReferences")
        .ok_or_else(|| "model3.json 缺 FileReferences".to_owned())?;
    let as_str = |key: &str| refs.get(key).and_then(Value::as_str).map(str::to_owned);
    let moc = as_str("Moc").ok_or_else(|| "FileReferences.Moc 缺失或不是字符串".to_owned())?;
    let textures = refs
        .get("Textures")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(ModelRefs {
        moc,
        textures,
        physics: as_str("Physics"),
        cdi: as_str("DisplayInfo"),
    })
}
