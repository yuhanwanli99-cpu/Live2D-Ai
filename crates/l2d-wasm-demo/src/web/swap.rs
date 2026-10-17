//! 舞台热换模型（wasm-only，0.3.5）。
//!
//! `load-model` 与 `sync.payload.model` 共用这一条异步路径；不变量：
//! - 成功后按**新 model id** 重载 `field_map.json`，先清旧贡献 / pending
//!   （idle 在 core 栈里，不归 fields 管）；
//! - `core.load_model` 半失败 → 重新加载旧 URL 回滚，绝不留下半状态；
//! - generation 守卫：加载期间又来了新请求，旧结果直接丢弃；
//! - 失败时**保留旧模型**（wasm 侧回滚），只回 error 帧。

use super::{emit_event, load_field_map, load_model_from_url, status};
use crate::web::surface::{self, SharedState};

/// 异步换模型。调用方不得在持有 `SharedState` 借用时调用（函数内部会 borrow）。
pub(super) fn spawn_swap_model(state: SharedState, url: String) {
    wasm_bindgen_futures::spawn_local(async move {
        status("切换模型中…");
        let Some(win) = web_sys::window() else {
            status("切换失败: 无 window 对象");
            return;
        };
        let (old_url, generation) = {
            let mut st = state.borrow_mut();
            st.model_generation += 1;
            (st.current_model_url.clone(), st.model_generation)
        };
        let loaded = match load_model_from_url(&win, &url).await {
            Ok(l) => l,
            Err(e) => {
                if state.borrow().model_generation == generation {
                    let msg = format!("切换失败: {e}");
                    status(&msg);
                    emit_event("error", serde_json::json!({ "message": msg, "url": url }));
                }
                return;
            }
        };
        // 加载期间又来了新请求：旧结果丢弃（新请求会自己装 core）。
        if state.borrow().model_generation != generation {
            return;
        }
        let load_err = { state.borrow_mut().core.load_model(&loaded).err() };
        if let Some(e) = load_err {
            let msg = format!("渲染核心换模型失败：{e}");
            // core.load_model 可能已经换掉 renderer 的一半 —— 用旧 URL 回滚。
            let rollback = match load_model_from_url(&win, &old_url).await {
                Ok(old) => {
                    let mut st = state.borrow_mut();
                    if st.model_generation != generation {
                        "换模已被更新的请求取代".to_string()
                    } else {
                        match st.core.load_model(&old) {
                            Ok(()) => "已回滚旧模型".to_string(),
                            Err(re) => format!("回滚旧模型也失败：{re}"),
                        }
                    }
                }
                Err(re) => format!("回滚旧模型加载失败：{re}"),
            };
            if state.borrow().model_generation == generation {
                status(&format!("{msg}；{rollback}"));
                emit_event(
                    "error",
                    serde_json::json!({
                        "message": msg,
                        "url": url,
                        "rollback": rollback,
                    }),
                );
            }
            return;
        }
        // 成功：清旧贡献 / pending（idle 不受影响），记住新 URL。
        {
            let mut st = state.borrow_mut();
            let surface::FrameState { bridge, core, .. } = &mut *st;
            bridge.fields.revoke_all(core);
        }
        state.borrow_mut().current_model_url = url.clone();
        // 按新 id 重载映射（取不到时 load_field_map 会落回新 id 的内建表）。
        let note = load_field_map(&state, &win, &crate::preset::model_id_from_url(&url)).await;
        status(&format!("模型已切换（{note}）"));
        if state.borrow().model_generation == generation {
            emit_event("loaded", serde_json::json!({ "url": url }));
        }
    });
}
