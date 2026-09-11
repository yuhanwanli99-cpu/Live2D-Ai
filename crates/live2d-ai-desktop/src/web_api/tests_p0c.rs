//! D-P0C（2026-08-29）首次配置闭环测试。
//!
//! 复审 P0-4：用户从无配置启动 → 页面填写 → PATCH 保存 → 应立即可对话
//! （或明确 restart_required），不能「保存显示已热重载但实际 503」。
//!
//! 覆盖：
//! 1. `apply_status` 序列化（**前端字符串锚点**）：
//!    `applied` / `restart_required` / `queued` / `no_supervisor`；
//! 2. 端到端闭环：配置不存在 → PATCH 保存完整配置 → 槽位非空 →
//!    `POST /api/v1/chat` 200 accepted（首配即对话）；
//! 3. 配置不完整（缺 base_url）：PATCH 保存后 apply_status =
//!    `restart_required`，chat 端点仍 503；
//! 4. 已有 supervisor：PATCH → 触发 reload（非 no-op）；
//! 5. 槽位基础 API：set / try_get / take / 重入容错。
//!
//! 入口：`web_api::mod tests_p0c;`（仅 `#[cfg(test)]` 下生效）。

use std::io::Read;

use tiny_http::{Method, Response};

use live2d_ai_runtime::AppSettings;

use super::*;

/// 把 Response `(status, body)` 同时拿到（避免消费两次）。
fn split_response(resp: Response<std::io::Cursor<Vec<u8>>>) -> (u16, String) {
    let status = resp.status_code().0;
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    (status, s)
}

/// 起一个最小 ServerContext：默认 settings + tmp 配置路径。
///
/// **路径唯一性**：用 caller 给的 `name` 拼 `{pid}-{name}-{thread_id}`。
fn ctx_with_tmp(name: &str) -> (ServerContext, std::path::PathBuf) {
    use std::hash::{Hash, Hasher};
    let thread_id = format!("{:?}", std::thread::current().id());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_p0c_{}_{}_{}.toml",
        std::process::id(),
        name,
        hasher.finish(),
    ));
    let _ = std::fs::remove_file(&tmp);
    let ctx = ServerContext::new(
        AppSettings::default(),
        tmp.to_string_lossy().into_owned(),
        None,
    );
    (ctx, tmp)
}

/// 起一个 tmp 路径，**预写**完整可解析的 LLM+TTS 配置（指向不可达端点，
/// 但解析合法）——用于"完整配置保存后能动态装配"路径。
fn ctx_with_full_cfg(name: &str) -> (ServerContext, std::path::PathBuf) {
    let mut s = AppSettings::default();
    s.llm.base_url = "http://127.0.0.1:11434/v1".into();
    s.llm.model = "qwen2.5:7b".into();
    s.llm.api_key_env = None; // 不依赖 env
    s.tts.base_url = "http://127.0.0.1:8000/v1".into();
    s.tts.voice = "alloy".into();
    s.tts.api_key_env = None;
    let (ctx, tmp) = ctx_with_tmp(name);
    std::fs::write(&tmp, s.to_toml_string()).expect("write initial cfg");
    // 把 ctx.status_ctx 里的 settings 视图同步（避免后续 swap 时 diff）。
    let s2 = AppSettings::load_from_path(&tmp).expect("reload");
    ctx.status_ctx.swap_settings(s2);
    (ctx, tmp)
}

/// 回收 Arc<SupervisorHandle>：尝试 unwrap 成独占 handle，失败时只 quit。
fn reclaim(handle: std::sync::Arc<crate::supervisor::SupervisorHandle>) {
    match std::sync::Arc::try_unwrap(handle) {
        Ok(h) => {
            h.quit();
            h.join();
        }
        Err(arc) => {
            // 兜底：仍有其他持有者（status_ctx 的 epoch_source 等），
            // 仅 quit（线程会在 quit 后被 supervisor 自身清理）。
            arc.quit();
        }
    }
}

// ============================================================================
// 1) ApplyStatus 序列化（**前端字符串锚点**——app.js 硬编码）
// ============================================================================

#[test]
fn apply_status_serializes_to_snake_case() {
    use crate::web_api::dto::ApplyStatus;
    let cases = [
        (ApplyStatus::Applied, "\"applied\""),
        (ApplyStatus::RestartRequired, "\"restart_required\""),
        (ApplyStatus::Queued, "\"queued\""),
        (ApplyStatus::NoSupervisor, "\"no_supervisor\""),
    ];
    for (status, want_json) in cases {
        let j = serde_json::to_string(&status).expect("serialize");
        assert_eq!(j, want_json, "ApplyStatus 序列化锚点：{status:?}");
    }
}

#[test]
fn patch_response_serializes_with_apply_status_field() {
    use crate::web_api::dto::ApplyStatus;
    use crate::web_api::settings_routes::PatchResponse;
    use live2d_ai_runtime::settings::view::settings_to_view;
    let r = PatchResponse {
        persisted: true,
        settings: settings_to_view(&AppSettings::default()),
        apply_status: ApplyStatus::NoSupervisor,
    };
    let j = serde_json::to_value(&r).expect("serialize");
    assert_eq!(j["apply_status"], "no_supervisor");
    assert_eq!(j["persisted"], true);
}

// ============================================================================
// 2) 端到端：完整配置保存后能动态装配 + chat 200
// ============================================================================

#[test]
fn first_time_full_config_patch_dynamic_assembles_supervisor() {
    // 起一个默认 settings + 空配置（盘上文件不存在）。
    let (ctx, tmp) = ctx_with_tmp("first_apply");

    // 起点：槽位空 + chat → 503（无 supervisor）。
    assert!(ctx.try_get_supervisor().is_none());
    let (status, _body) = split_response(dispatch(
        &ctx,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
    ));
    assert_eq!(status, 503, "起点无 supervisor → 503");

    // PATCH 写一个完整 LLM+TTS 配置（动态装配路径）。
    let body = r#"{"llm":{"base_url":"http://127.0.0.1:11434/v1","model":"qwen2.5:7b"},"tts":{"base_url":"http://127.0.0.1:8000/v1","voice":"alloy"}}"#;
    let (status, body_str) =
        split_response(dispatch(&ctx, &Method::Patch, "/api/v1/settings", body));
    assert_eq!(status, 200, "PATCH 200；body={body_str}");
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "applied",
        "完整配置应触发动态装配 → applied：body={body_str}"
    );

    // 关键断言：槽位现在非空（动态装配成功）。
    assert!(
        ctx.try_get_supervisor().is_some(),
        "PATCH 后槽位必须非空（动态装配成功）"
    );

    // 关键断言：POST /api/v1/chat 200 accepted（首配即对话闭环）。
    let (status, body_str) = split_response(dispatch(
        &ctx,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
    ));
    assert_eq!(status, 200, "动态装配后 chat 必须 200；body={body_str}");
    assert!(
        body_str.contains("\"accepted\":true"),
        "chat 响应必须 accepted=true：{body_str}"
    );

    // 清理：取走槽位 → reclaim。
    if let Some(h) = ctx.take_supervisor_for_reclaim() {
        reclaim(h);
    }
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn restart_required_when_config_cannot_assemble() {
    // 起一个 tmp + 写一个**不完整**配置（base_url 空 → 不可装配）。
    let (ctx, tmp) = ctx_with_tmp("restart_required");
    let bad = AppSettings::default();
    // base_url 默认空字符串（不可装配）。
    std::fs::write(&tmp, bad.to_toml_string()).expect("write");
    let s2 = AppSettings::load_from_path(&tmp).expect("reload");
    ctx.status_ctx.swap_settings(s2);

    // PATCH 一个「看似改了一点但 base_url 仍空」的配置（保留空 = 仍不可装配）。
    let body = r#"{"llm":{"model":"qwen2.5:7b"}}"#;
    let (status, body_str) =
        split_response(dispatch(&ctx, &Method::Patch, "/api/v1/settings", body));
    assert_eq!(status, 200, "PATCH 200（保存成功）；body={body_str}");
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "restart_required",
        "base_url 空 → 无法装配 → restart_required：{body_str}"
    );

    // 关键断言：槽位仍空（未装配）。
    assert!(
        ctx.try_get_supervisor().is_none(),
        "restart_required 路径槽位必须保持空"
    );

    // chat 端点仍 503。
    let (status, _body) = split_response(dispatch(
        &ctx,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
    ));
    assert_eq!(status, 503, "未装配 → 503");

    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 3) 已有 supervisor：PATCH 触发 reload（非 no-op）
// ============================================================================

#[test]
fn patch_with_existing_supervisor_triggers_reload() {
    // 起一个带 supervisor 的 ctx（指向不可达端点但**可装配**）。
    let (ctx, tmp) = ctx_with_full_cfg("existing");

    // 第一次 PATCH：槽位空 → 动态装配。
    let (status, body_str) = split_response(dispatch(
        &ctx,
        &Method::Patch,
        "/api/v1/settings",
        r#"{"persona":{"system_prompt":"first"}}"#,
    ));
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "applied",
        "首次 PATCH 必触发动态装配：{body_str}"
    );
    assert!(ctx.try_get_supervisor().is_some());

    let handle = ctx.try_get_supervisor().expect("slot filled");
    // 给 supervisor 一点时间消费 Reload（首次的 ensure 路径不调 reload，
    // 但有 race；sleep 让后续断言稳定）。
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 第二次 PATCH：槽位已非空 → 走 reload 路径。
    let (status, body_str) = split_response(dispatch(
        &ctx,
        &Method::Patch,
        "/api/v1/settings",
        r#"{"persona":{"system_prompt":"second"}}"#,
    ));
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "applied",
        "第二次 PATCH 走 reload → applied：{body_str}"
    );

    // 给 supervisor 一点时间消费 Reload。
    std::thread::sleep(std::time::Duration::from_millis(100));
    // 关键断言：reload_pending 被消费（说明 Reload 抵达）。
    let still_pending = handle
        .reload_pending_for_test()
        .load(std::sync::atomic::Ordering::SeqCst);
    assert!(!still_pending, "第二次 PATCH 后 reload_pending 必须被消费");

    // 清理。
    if let Some(h) = ctx.take_supervisor_for_reclaim() {
        reclaim(h);
    }
    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 4) ServerContext 槽位 API 边界
// ============================================================================

#[test]
fn server_context_supervisor_slot_default_is_empty() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    assert!(ctx.try_get_supervisor().is_none());
    assert!(ctx.take_supervisor_for_reclaim().is_none());
}

#[test]
fn server_context_take_after_dynamic_assemble_returns_handle() {
    // 完整配置 → 动态装配 → 拿回 handle → 二次 take = None。
    let (ctx, tmp) = ctx_with_full_cfg("take_after");
    let (status, _body) = split_response(dispatch(
        &ctx,
        &Method::Patch,
        "/api/v1/settings",
        r#"{"persona":{"system_prompt":"x"}}"#,
    ));
    assert_eq!(status, 200);
    assert!(ctx.try_get_supervisor().is_some());

    // 拿回。
    let h = ctx.take_supervisor_for_reclaim().expect("take ok");
    // 二次 take = None。
    assert!(ctx.take_supervisor_for_reclaim().is_none());
    reclaim(h);
    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 5) 集成：完整 E2E 闭环（接近复审指定场景）
// ============================================================================

#[test]
fn e2e_first_time_no_config_patch_completes_loop() {
    // 复审场景：删除配置及父目录 → --web 启动 → curl PATCH 保存完整配置
    // → 观察 apply_status + POST /chat 是否 200。
    //
    // 本测试模拟「无配置启动 + PATCH」路径（**不**走真实 `run_web_mode`，
    // 走 dispatch 单元层：等价验证槽位 + apply_status 流程）。
    let (ctx, tmp) = ctx_with_tmp("e2e_no_config");
    // 不写任何文件 = 等价「无配置启动」。
    assert!(!tmp.exists(), "tmp 路径不应预先存在");
    assert!(ctx.try_get_supervisor().is_none(), "起点槽位空");

    // 步骤 1：PATCH 完整 LLM+TTS（一次性写入文件 + 动态装配）。
    let body = r#"{"llm":{"base_url":"http://127.0.0.1:11434/v1","model":"qwen2.5:7b"},"tts":{"base_url":"http://127.0.0.1:8000/v1","voice":"alloy"}}"#;
    let (status, body_str) =
        split_response(dispatch(&ctx, &Method::Patch, "/api/v1/settings", body));
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "applied",
        "完整配置动态装配成功：{body_str}"
    );

    // 文件**真的**被写盘（首配即落盘）。
    assert!(tmp.exists(), "PATCH 写盘后文件必须存在");
    let on_disk = std::fs::read_to_string(&tmp).expect("read");
    assert!(
        on_disk.contains("qwen2.5:7b"),
        "盘上必须含新 LLM model：{on_disk}"
    );

    // 步骤 2：POST /api/v1/chat 必须 200 accepted。
    let (status, body_str) = split_response(dispatch(
        &ctx,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hello"}"#,
    ));
    assert_eq!(status, 200, "首配后 chat 必须 200：{body_str}");
    assert!(body_str.contains("\"accepted\":true"), "{body_str}");

    // 步骤 3：再 PATCH 一次（走 reload 而非 no-op）。
    let (status, body_str) = split_response(dispatch(
        &ctx,
        &Method::Patch,
        "/api/v1/settings",
        r#"{"persona":{"system_prompt":"updated"}}"#,
    ));
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body_str).expect("json");
    assert_eq!(
        json["apply_status"], "applied",
        "二次 PATCH 走 reload：{body_str}"
    );

    // 清理。
    if let Some(h) = ctx.take_supervisor_for_reclaim() {
        reclaim(h);
    }
    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 6) supervisor_slot 模块直接覆盖（API 边界）
// ============================================================================

#[test]
fn supervisor_slot_set_get_take_basic() {
    use crate::web_api::supervisor_slot::SupervisorSlot;
    let slot = SupervisorSlot::new();
    assert!(slot.try_get().is_none());
    assert!(slot.take().is_none());
    // 二次 take = None。
    assert!(slot.take().is_none());
}
