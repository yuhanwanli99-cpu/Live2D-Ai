//! web_api 热重载测试（拆分承载：mod.rs 主体 + 本文件 + 既有 tests
//! 接近 600 行撞 500 上限；本批为 Reload 接线新增 2 条测试）。
//!
//! 入口：`web_api::mod tests_reload;`（仅 `#[cfg(test)]` 下生效）——
//! 本文件作为 `web_api` 的子模块，可直接 `use super::*;` 访问私有项。

use super::*;
use std::io::Read;
use std::sync::Arc;
use tiny_http::{Method, Response};

/// 把 Response body 拉成 String（与 `super::tests` 模块同实现）。
fn body_to_string(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

/// 轮询等待 `reload_pending` 被 supervisor 消费（`apply_reload` swap 清零）。
///
/// 替代「固定 sleep(100ms) 后断言已消费」：重负载（3× flutter 并发）下
/// 100ms 不保证 supervisor 已经跑过 `apply_reload`；这里以 10ms 步长轮询
/// 到期限为止，命中即返回 true。
pub(super) fn wait_until_reload_consumed(
    supervisor: &Arc<crate::supervisor::SupervisorHandle>,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let pending = supervisor
            .reload_pending_for_test()
            .load(std::sync::atomic::Ordering::SeqCst);
        if !pending {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// 起一个最小 supervisor 指向 tmp 配置（URL 不要求端点可达，只要求
/// reload 路径走通）。返回 `SupervisorHandle + tmp path`。
pub(super) fn spawn_test_supervisor(
    key: &str,
) -> (Arc<crate::supervisor::SupervisorHandle>, std::path::PathBuf) {
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_webapi_reload_{}_{}.toml",
        key,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let initial = AppSettings::default();
    std::fs::write(&tmp, initial.to_toml_string()).expect("write initial");

    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let supervisor = Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        |_ev| {},
    ));
    (supervisor, tmp)
}

/// 热重载：PATCH 写盘成功后必须通知 supervisor 重建。
///
/// 测试用真实 supervisor（指向不存在的端点 reload 会失败，但标志
/// 会被置位，断言只看「handle 是否收到 Reload」不看重建结果）。
/// 配置路径用临时文件 + 写一个最小有效 LLM+TTS TOML（reload 不要求
/// 端点可达，只要求 URL 合法 + 解析通过）。
#[test]
fn dispatch_settings_patch_triggers_supervisor_reload() {
    let (supervisor, tmp) = spawn_test_supervisor("trigger");

    // 构造 ServerContext 并注入 supervisor。
    let ctx = ServerContext::new(
        AppSettings::default(),
        tmp.to_string_lossy().into_owned(),
        None,
    )
    .with_supervisor(Arc::clone(&supervisor));

    // 写一个最小 patch body（改 LLM model）。
    let body = r#"{"llm":{"model":"patched-model"}}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(
        resp.status_code().0,
        200,
        "PATCH 必须 200；body={}",
        body_to_string(resp)
    );

    // 验证：settings 文件已被写盘（PATCH 写盘成功）。
    let after = std::fs::read_to_string(&tmp).expect("read after");
    assert!(
        after.contains("patched-model"),
        "PATCH 应把新 model 写盘: {after}"
    );

    // 验证：supervisor 的 reload_pending 已被消费（说明 Reload 抵达）。
    // 这是「handle 收到 Reload」的可观察代理：标志由 handle.reload() 置位、
    // apply_reload swap 清零；清零即证明「Reload 已经被处理过」。
    //
    // 同步：轮询等待清零（上限 5s），而不是固定 sleep(100ms)——重负载下
    // 100ms 不保证 supervisor 已完成 apply_reload（/tmp/flaky-A-r2-4.log
    // 那一批假红里就有这一类固定窗口假设）。
    assert!(
        wait_until_reload_consumed(&supervisor, std::time::Duration::from_secs(5)),
        "PATCH 后 reload_pending 必须被消费（idle 或 turn 收口）"
    );

    let _ = std::fs::remove_file(&tmp);
    drop(supervisor);
}

/// PATCH 失败时不触发 reload（不写盘 = 不 reload）。
#[test]
fn dispatch_settings_patch_failure_does_not_reload() {
    let (supervisor, tmp) = spawn_test_supervisor("fail");

    let ctx = ServerContext::new(
        AppSettings::default(),
        tmp.to_string_lossy().into_owned(),
        None,
    )
    .with_supervisor(Arc::clone(&supervisor));

    // 无效 body（非 JSON） → 400，不应触发 reload。
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", "not json");
    assert_eq!(resp.status_code().0, 400);
    // 标志位应仍为 false（reload_pending 永远只在 reload() 时被置位）。
    let pending = supervisor
        .reload_pending_for_test()
        .load(std::sync::atomic::Ordering::SeqCst);
    assert!(!pending, "400 路径不应置位 reload_pending");

    let _ = std::fs::remove_file(&tmp);
    drop(supervisor);
}

/// 外部手改配置 → 快照必须跟着变（2026-09-11 修）。
///
/// 复现的真实症状：用户在编辑器里改了 `live2d-ai.toml` 的提示词，文件监听器
/// 只 `supervisor.reload()` 重建了 client，`GET /api/v1/settings` 仍回旧值；
/// 更糟的是下一次界面「保存」会拿**旧快照**做合并后整份写回，把手改的内容
/// 覆盖掉——看起来就是「改了提示词就被吃掉」。
#[test]
fn refresh_from_disk_syncs_snapshot_and_dev_mode() {
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_refresh_{}.toml",
        std::process::id()
    ));
    let mut on_disk = AppSettings::default();
    on_disk.persona.system_prompt = "磁盘上的新提示词".to_string();
    on_disk.dev_mode = true;
    std::fs::write(&tmp, on_disk.to_toml_string()).expect("write");

    // 快照是**旧**的（模拟「启动后用户才在编辑器里改」）。
    let mut stale = AppSettings::default();
    stale.persona.system_prompt = "启动时的旧提示词".to_string();
    stale.dev_mode = false;
    let ctx = ServerContext::new(stale, tmp.to_string_lossy().into_owned(), None);
    assert!(!ctx.status_ctx.dev_mode());

    assert!(
        ctx.status_ctx
            .refresh_from_disk(&tmp.to_string_lossy())
            .is_ok(),
        "合法 toml 必须刷新成功"
    );
    assert_eq!(
        ctx.status_ctx.settings_snapshot().persona.system_prompt,
        "磁盘上的新提示词"
    );
    assert!(
        ctx.status_ctx.dev_mode(),
        "dev_mode 必须跟着磁盘走（否则日志端点门控与配置分叉）"
    );

    let _ = std::fs::remove_file(&tmp);
}

/// 配置写坏（语法错误）时**保留旧快照**并返回 Err——界面不该跟着一起坏，
/// 但调用方要拿到失败原文（S-007：无 supervisor 时它变成 error 帧的 message）。
#[test]
fn refresh_from_disk_keeps_snapshot_on_broken_toml() {
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_refresh_broken_{}.toml",
        std::process::id()
    ));
    std::fs::write(&tmp, "这不是 = = 合法 TOML [").expect("write");

    let mut old = AppSettings::default();
    old.persona.system_prompt = "旧值".to_string();
    let ctx = ServerContext::new(old, tmp.to_string_lossy().into_owned(), None);

    let err = ctx
        .status_ctx
        .refresh_from_disk(&tmp.to_string_lossy())
        .expect_err("坏 toml 必须返回 Err（S-007：调用方靠它发 error 帧）");
    assert!(
        err.contains("配置文件解析失败"),
        "Err 必须带解析错误原文（error 帧 message 的来源）: {err}"
    );
    assert_eq!(
        ctx.status_ctx.settings_snapshot().persona.system_prompt,
        "旧值",
        "解析失败时必须保留旧快照"
    );

    let _ = std::fs::remove_file(&tmp);
}

// ---------------------------------------------------------------- S-007：无 supervisor
// 时手改坏 toml 必须有界面提示

/// 构造一个 broadcaster + 订阅端，用来读**实际广播的帧原文**。
///
/// `chat_routes_tests_support` 里有一份同语义脚手架，但它挂在 `chat_routes`
/// 模块下、`pub(super)` 只对 `chat_routes` 可见，本模块用不到，所以这里重建
/// 最小版（与那份实现逐字同形）。
fn broadcaster_with_rx() -> (ws::Broadcaster, std::sync::mpsc::Receiver<String>) {
    let bc = ws::Broadcaster::new();
    let (tx, rx) = std::sync::mpsc::sync_channel(16);
    bc.try_subscribe(tx).expect("订阅 broadcaster");
    (bc, rx)
}

/// 写一个测试用临时配置文件；`tag` 保证并行测试互不踩。
fn write_tmp_config(tag: &str, contents: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "live2d_ai_test_watch_{tag}_{}.toml",
        std::process::id()
    ));
    std::fs::write(&path, contents).expect("write tmp config");
    path
}

/// S-007 的**无 supervisor 分支**：手改坏 `live2d-ai.toml` 必须**恰好**广播
/// 一条既有 `error` 帧（前端既有错误横幅因此出现），且保留旧快照。
///
/// 守门人：把 `config_watch_post_reload` 里的广播删掉（回到「只 warn」），
/// 本测试变红——那正是修复前的缺陷行为。
#[test]
fn config_watch_without_supervisor_broadcasts_one_error_frame() {
    let tmp = write_tmp_config("broken_no_supervisor", "这不是 = = 合法 TOML [");
    let mut stale = AppSettings::default();
    stale.persona.system_prompt = "旧提示词".to_string();
    let ctx = ServerContext::new(stale, tmp.to_string_lossy().into_owned(), None);
    let (bc, rx) = broadcaster_with_rx();

    let hook = super::cli_entry::config_watch_post_reload(
        Arc::clone(&ctx.status_ctx),
        tmp.to_string_lossy().into_owned(),
        bc,
    );
    // false = 本次事件时槽位里没有 supervisor（首跑窗口 / 动态装配之前）。
    hook(false);

    let raw = rx
        .try_recv()
        .expect("坏 toml + 无 supervisor：必须广播 error 帧（S-007 的修复点）");
    let frame: serde_json::Value = serde_json::from_str(&raw).expect("帧必须是合法 JSON");
    assert_eq!(frame["type"], "error", "帧类型必须是既有 error: {raw}");
    let data = &frame["data"];
    assert_eq!(data["code"], "config_reload_failed");
    assert_eq!(data["stage"], "config");
    assert_eq!(data["epoch"], 0, "不是某一轮对话的错误，epoch 恒 0");
    assert_eq!(data["fatal"], false, "旧配置仍在跑，不是本轮失败");
    assert_eq!(
        data["hint"], "修好 live2d-ai.toml 后再保存一次，或重启服务",
        "hint 必须与 supervisor 分支逐字一致（前端只有一条横幅样式）"
    );
    let message = data["message"].as_str().expect("message 必须是字符串");
    assert!(
        message.contains("配置文件解析失败"),
        "message 必须带解析错误原文: {message}"
    );
    assert!(
        rx.try_recv().is_err(),
        "一次事件只许一条 error 帧（两条横幅是 bug）"
    );
    assert_eq!(
        ctx.status_ctx.settings_snapshot().persona.system_prompt,
        "旧提示词",
        "解析失败时必须保留旧快照"
    );

    let _ = std::fs::remove_file(&tmp);
}

/// **有** supervisor 时钩子**不**广播：那条失败由 `apply_reload` 统一发帧，
/// 两处都发会让界面出现两条重复横幅。
#[test]
fn config_watch_with_supervisor_does_not_broadcast() {
    let tmp = write_tmp_config("broken_with_supervisor", "这不是 = = 合法 TOML [");
    let ctx = ServerContext::new(
        AppSettings::default(),
        tmp.to_string_lossy().into_owned(),
        None,
    );
    let (bc, rx) = broadcaster_with_rx();
    let hook = super::cli_entry::config_watch_post_reload(
        Arc::clone(&ctx.status_ctx),
        tmp.to_string_lossy().into_owned(),
        bc,
    );
    // true = 本次事件时槽位里有 supervisor（apply_reload 那条出口存在）。
    hook(true);
    assert!(
        rx.try_recv().is_err(),
        "有 supervisor 时钩子不得广播（否则与 apply_reload 的帧重复）"
    );

    let _ = std::fs::remove_file(&tmp);
}

/// 合法 toml → 只刷新快照，**不**广播任何帧（提示只在解析失败时出现）。
#[test]
fn config_watch_with_valid_toml_broadcasts_nothing() {
    let mut on_disk = AppSettings::default();
    on_disk.persona.system_prompt = "磁盘上的新提示词".to_string();
    on_disk.dev_mode = true;
    let tmp = write_tmp_config("valid", &on_disk.to_toml_string());
    let ctx = ServerContext::new(
        AppSettings::default(),
        tmp.to_string_lossy().into_owned(),
        None,
    );
    assert!(!ctx.status_ctx.dev_mode(), "前提：快照是旧的");

    let (bc, rx) = broadcaster_with_rx();
    let hook = super::cli_entry::config_watch_post_reload(
        Arc::clone(&ctx.status_ctx),
        tmp.to_string_lossy().into_owned(),
        bc,
    );
    hook(false);

    assert!(rx.try_recv().is_err(), "合法 toml 不得广播任何帧");
    assert_eq!(
        ctx.status_ctx.settings_snapshot().persona.system_prompt,
        "磁盘上的新提示词"
    );
    assert!(ctx.status_ctx.dev_mode(), "dev_mode 必须跟着磁盘走");

    let _ = std::fs::remove_file(&tmp);
}
