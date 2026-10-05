//! 会话 baseline 的读写与广播（V10 / O8；自 `chat_routes.rs` 拆出）。
//!
//! 停止 / 新消息事务的**第四步**：取当前活动会话的 baseline → 封成既有
//! `action_cue` 帧 → 广播（不排队、不重放）。清动作/表情/TTS 待播仍在调用方
//! 的 supervisor stop 事务里。父文件保有路由与 stop 事务；两个私有 helper 按
//! `pub(super)` 开放回父模块，两个 `pub fn` 由父模块 `pub use` 保持原路径。

use super::*;

// ------------------------------------------------- 会话 baseline（V10 / O8）

/// 停止 / 新消息的**第四步**：回该会话 baseline（V10 §9.3）。
///
/// 这里**只**做第四步——清动作 / 表情 / TTS 待播由调用方先走 supervisor 的 stop
/// 事务（[cancel_and_return_to_session_baseline] 是它的打包）。本函数：
///
/// 1. 读宿主**活动会话**（`session_scopes.active()`）；
/// 2. 取该会话的 baseline（同一道 `sanitize_session_id` 闸；`None` = 待机）；
/// 3. 把它封成一条**既有 `action_cue` 帧**（V11：不新增帧型）——
///    `cues` 为 baseline 的 cue 列表；待机时 `cues: []`（= 本轮不动，停在
///    三样清空后的状态）；
/// 4. 有 broadcaster 就广播；**不排队、不重放**（`backfilled` 恒 false）。
///
/// 为什么复用 `action_cue`：前端 `ActionCueEvent` 已经在消费它（按句替换 cue
/// 计划），这是「取消旧计划」与「给回基准」共用的**唯一**既有下行通道；
/// 新增帧型需要前端同批改动，越出本轨授权。
pub fn return_to_session_baseline(
    handle: &SupervisorHandle,
    broadcaster: Option<&crate::web_api::ws::Broadcaster>,
    reason: &str,
) -> BaselineRevocation {
    let store = handle.session_scopes();
    let session = store.active();
    let baseline = store.baseline_for(session.as_deref());
    let cues = baseline_cues(baseline.as_deref());
    let frame = serde_json::to_string(&serde_json::json!({
        "type": "action_cue",
        "data": {
            "epoch": handle.current_epoch(),
            "covers_upto_seq": 0,
            "cues": cues,
            // 附加标记（帧结构**只增不改**，V11）：让消费方能与导演 cue 区分。
            "baseline": true,
            "reason": reason,
        }
    }))
    .ok();
    if let (Some(bc), Some(raw)) = (broadcaster, frame.as_deref()) {
        bc.broadcast(raw);
    }
    BaselineRevocation {
        session,
        baseline,
        frame,
        // host 侧没有任何「待补帧」队列：取消就是取消。
        backfilled: false,
    }
}

/// 停止路径的打包：清三样（supervisor 的 stop 事务）+ 回该会话 baseline（V10 §9.3）。
///
/// `handle.stop()` 非阻塞、幂等；supervisor 在轮边界推进 epoch、取消在飞轮、
/// 清 pending PCM 并 StopPlayback（不在本 crate 的授权内重复实现）。
pub fn cancel_and_return_to_session_baseline(
    handle: &SupervisorHandle,
    broadcaster: Option<&crate::web_api::ws::Broadcaster>,
    reason: &str,
) -> BaselineRevocation {
    handle.stop();
    return_to_session_baseline(handle, broadcaster, reason)
}

/// baseline 原文 → `action_cue.data.cues` 列表。
///
/// 合法形状（写入方 = 宿主 API，见 session 路由）：
/// - 一个 cue 对象 → `[obj]`；
/// - 一个 cue 对象数组 → 原样（数组里的非对象元素丢弃）；
/// - 非法 JSON / 其它形状 → `[]`（= 待机，**绝不**把坏串塞上 wire）。
///
/// 注意：host **不解释** cue 内容（与 prompts「宿主只存字符串」同一条纪律）——
/// 「field 是不是 body/head/expression」由写入方与渲染面负责。
pub(super) fn baseline_cues(raw: Option<&str>) -> Vec<serde_json::Value> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter(serde_json::Value::is_object)
            .collect(),
        Ok(value @ serde_json::Value::Object(_)) => vec![value],
        _ => Vec::new(),
    }
}

/// 存储里 baseline 的字符串 → HTTP 响应里的 JSON（`null` = 待机）。
pub(super) fn baseline_value(raw: Option<&str>) -> serde_json::Value {
    let Some(raw) = raw else {
        return serde_json::Value::Null;
    };
    serde_json::from_str::<serde_json::Value>(raw).unwrap_or(serde_json::Value::Null)
}
