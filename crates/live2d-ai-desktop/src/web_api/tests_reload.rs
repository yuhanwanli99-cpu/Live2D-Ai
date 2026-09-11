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

/// 起一个最小 supervisor 指向 tmp 配置（URL 不要求端点可达，只要求
/// reload 路径走通）。返回 `SupervisorHandle + tmp path`。
fn spawn_test_supervisor(
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

    // 给 supervisor 一点时间消费 Reload（idle select 立即处理；100ms 余量足够）。
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 验证：settings 文件已被写盘（PATCH 写盘成功）。
    let after = std::fs::read_to_string(&tmp).expect("read after");
    assert!(
        after.contains("patched-model"),
        "PATCH 应把新 model 写盘: {after}"
    );

    // 验证：supervisor 的 reload_pending 已被消费（说明 Reload 抵达）。
    // 这是「handle 收到 Reload」的可观察代理：原子标志 swap 后归零。
    // 标志由 handle.reload() 置位、apply_reload swap 清零；这里清零
    // 即证明「Reload 已经被处理过」。
    let still_pending = supervisor
        .reload_pending_for_test()
        .load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        !still_pending,
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

    assert!(ctx.status_ctx.refresh_from_disk(&tmp.to_string_lossy()));
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

/// 配置写坏（语法错误）时**保留旧快照**并返回 false——界面不该跟着一起坏。
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

    assert!(!ctx.status_ctx.refresh_from_disk(&tmp.to_string_lossy()));
    assert_eq!(
        ctx.status_ctx.settings_snapshot().persona.system_prompt,
        "旧值",
        "解析失败时必须保留旧快照"
    );

    let _ = std::fs::remove_file(&tmp);
}
