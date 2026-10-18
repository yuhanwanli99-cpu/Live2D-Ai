//! 舞台热换模型（wasm-only 绑定层，0.3.5 起；0.3.6 T1 改为**原子换核心**）。
//!
//! 状态机（代际守卫 / 成功收尾 / 失败只回 error）住在平台无关的
//! [`crate::swap_core`]：wasm 与 native fake loader 单测共用同一份逻辑。
//! 本文件只做 wasm 绑定：
//! - fetch + 解码模型（[`super::load_model_from_url`]）；
//! - **修法 B**：在**同一个** `GpuContext` 上新建 `ModelRendererCore` 装载，成功才
//!   `*core = fresh` 原子替换——失败时旧核心一个字段都没被碰过；
//! - 换模成功后按新 model id 重载 `/actions/field_map.json`；
//! - 分阶段 console 日志（`[swap ...ms]`，0.3.6 T1 取证用；真机再炸时能定位阶段）。
//!
//! **不再回滚 load 旧 URL**：0.3.5 的失败路径会「重载旧 URL 再 load 一次」，那是
//! 对**同一个 renderer 的第二次 `load_model`**（0.3.5 手测 panic 栈顶就落在它调用的
//! `create_buffer_init` 上）。原子替换把这条路径整体消掉：失败 = 旧模型原样还在。

use std::future::Future;
use std::pin::Pin;

use l2d::model::LoadedModel;

use super::{emit_event, load_field_map, load_model_from_url, status};
use crate::preset::{FieldMap, FieldMapLoad};
use crate::swap_core::{self, FieldMapReloader, ModelLoader, SwapHost};
use crate::web::surface::{self, SharedState};

/// 换模分阶段日志（只写 console；不写页面状态栏，避免把调试信息当产品状态）。
fn phase(msg: &str) {
    let now = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
        "[swap {now:.1}ms] {msg}"
    )));
}

/// wasm 宿主：把状态机要求的副作用接到 `SharedState` / console / 父页消息。
struct WasmHost {
    state: SharedState,
}

impl SwapHost for WasmHost {
    type Model = LoadedModel;

    fn begin_generation(&mut self) -> u64 {
        let mut st = self.state.borrow_mut();
        st.model_generation += 1;
        st.model_generation
    }

    fn generation(&self) -> u64 {
        self.state.borrow().model_generation
    }

    fn model_epoch(&self) -> u64 {
        self.state.borrow().model_epoch
    }

    fn advance_model_epoch(&mut self) -> u64 {
        let mut st = self.state.borrow_mut();
        st.model_epoch += 1;
        st.model_epoch
    }

    fn current_model_id(&self) -> String {
        crate::preset::model_id_from_url(&self.state.borrow().current_model_url)
    }

    /// **原子替换**（修法 B）：新核心从零装载，全部成功才换进 `FrameState`。
    ///
    /// 失败时 `Err` 已在 `load_fresh` 内部 return，旧 `core` 未被赋值过。
    fn install(&mut self, loaded: &LoadedModel) -> Result<(), String> {
        let mut st = self.state.borrow_mut();
        let fresh = st.core.load_fresh(loaded).map_err(|e| e.to_string())?;
        // layout 基准属于「新模型的逻辑变换」，必须在替换前取好：
        // 下一帧 `apply_bridge_effects` 会用它重建用户缩放 / 平移。
        st.last_layout_transform = fresh.transform();
        st.core = fresh;
        Ok(())
    }

    fn revoke_fields(&mut self) {
        let mut st = self.state.borrow_mut();
        let surface::FrameState { bridge, core, .. } = &mut *st;
        bridge.fields.revoke_all(core);
    }

    fn set_current_url(&mut self, url: String) {
        self.state.borrow_mut().current_model_url = url;
    }

    /// 字段表由 load_field_map 按**新 model id** 重建，这里只装进运行时。
    fn apply_field_map(&mut self, map: FieldMap) {
        self.state.borrow_mut().bridge.fields.set_map(map);
    }

    fn status(&mut self, msg: &str) {
        status(msg);
    }

    fn phase(&mut self, msg: &str) {
        phase(msg);
    }

    fn emit(&mut self, ty: &str, payload: serde_json::Value) {
        emit_event(ty, payload);
    }
}

/// 模型拉取器：持有 `Window`（fetch 需要它），每次 load 起一个 boxed future。
struct WasmModelLoader {
    window: web_sys::Window,
}

impl ModelLoader for WasmModelLoader {
    type Model = LoadedModel;
    type Fut = Pin<Box<dyn Future<Output = Result<LoadedModel, String>>>>;

    fn load(&self, url: &str) -> Self::Fut {
        let window = self.window.clone();
        let url = url.to_owned();
        Box::pin(async move { load_model_from_url(&window, &url).await })
    }
}

/// 字段映射重载器：wasm 侧是异步 fetch（取不到时回退**新 model id** 的内建表）。
struct WasmFieldMapReloader {
    window: web_sys::Window,
}

impl FieldMapReloader for WasmFieldMapReloader {
    type Fut = Pin<Box<dyn Future<Output = FieldMapLoad>>>;

    fn reload(&self, model_id: &str) -> Self::Fut {
        let window = self.window.clone();
        let model_id = model_id.to_owned();
        Box::pin(async move { load_field_map(&window, &model_id).await })
    }
}

/// 异步换模型。调用方不得在持有 `SharedState` 借用时调用（函数内部会 borrow）。
pub(super) fn spawn_swap_model(state: SharedState, url: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(window) = web_sys::window() else {
            status("切换失败: 无 window 对象");
            return;
        };
        let mut host = WasmHost {
            state: state.clone(),
        };
        let loader = WasmModelLoader {
            window: window.clone(),
        };
        let field_map = WasmFieldMapReloader { window };
        swap_core::run_swap(&mut host, url, &loader, &field_map).await;
    });
}
