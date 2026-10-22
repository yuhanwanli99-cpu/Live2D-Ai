//! `mods_routes` 场景回归（自内联 `mod tests` 拆出，2026-10-06：本文件原 1009 行，
//! 触到 code-stats 的 `>1000` 棘轮）。脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;
use crate::web_api::security::SecurityContext;
use std::io::Read;

/// M2：`GET /api/v1/mods` 带出 `settings_spec` + `config`，且 secret 字段脱敏。
#[test]
fn mods_list_includes_spec_and_redacts_secret() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    assert!(body.contains("\"settings_spec\""), "应带 schema: {body}");
    assert!(
        body.contains("\"kind\":\"string\""),
        "字段种类应可见: {body}"
    );
    assert!(body.contains("\"kind\":\"bool\""), "字段种类应可见: {body}");
    assert!(
        body.contains("\"secret\":true"),
        "secret 标记应在 schema 里: {body}"
    );
    assert!(!body.contains("SECRET"), "secret 字段值不得进 GET: {body}");
    assert!(body.contains("\"/x\""), "非 secret 字段应回值: {body}");
}

/// **rc4（2026-10-17）**：本机 `mods.json` 可能仍有已退役的 `director` 条目
///（该文件是本机配置，运维不动它）。未注册的 id 必须被**忽略并告警**，而不是让
/// 启动失败；`GET /api/v1/mods` 里也**不得**出现 director（它已移出
/// `AVAILABLE_MOD_FACTORIES`，见 main.rs `mod_count_is_five`）。
#[test]
fn retired_director_entry_in_manifest_is_ignored_by_the_registry() {
    let manifest = serde_json::json!({
        "mods": {
            "specmod": {"enabled": true, "config": {"path": "/x"}},
            "director": {"enabled": true, "config": {"motion_bias": "eager"}}
        }
    });
    // 不 panic / 不报错：未注册 id 只走「warn + 忽略」，不动其它 Mod 的注册。
    let mut registry = crate::mod_registry::ModRegistry::new(SPEC_FACTORIES, &manifest);
    registry.start_all();
    assert!(registry.contains("specmod"), "已注册 Mod 照常进表");
    assert!(
        !registry.contains("director"),
        "未注册的 director 条目必须被忽略"
    );
    assert!(
        registry.config("director").is_none(),
        "未注册 id 不得出现在配置面"
    );
    assert_eq!(registry.ids(), vec!["specmod"], "注册表只含编译期工厂");

    let ctx = dummy_ctx(SecurityContext::new(18099, true));
    *ctx.mod_registry.lock().unwrap() = registry;
    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    assert!(body.contains("\"id\":\"specmod\""), "body = {body}");
    assert!(
        !body.contains("director"),
        "GET /api/v1/mods 不得出现已退役的 director: {body}"
    );
}

/// M2：`GET /api/v1/mods/{id}/config` 只读返回；未知 id → 404；secret 不回值。
#[test]
fn mod_config_get_returns_redacted_config() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let ok = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/specmod/config",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(ok.status_code(), StatusCode(200));
    let body = body_string(ok);
    assert!(body.contains("\"/x\""), "body = {body}");
    assert!(!body.contains("SECRET"), "secret 不得回值: {body}");

    let missing = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/nope/config",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(missing.status_code(), StatusCode(404));
}

/// **F-0062-01（路由层）**：POST `…/config` 省略声明的 secret 字段（= 前端从 GET
/// 的**脱敏**配置出发的形状）时**保留**旧密钥。显式清除（空串）见
/// `mod_registry::tests_secret`（用测试专用 ModFactory 覆盖两条语义）。
/// 2026-10-19：原先指向 `external_routes` 的端到端「密钥还在 ⇒ 401」用例已随
/// `external-input` Mod 移出主仓删除；保存不抹 secret 的规则本文件这条仍在守。
#[test]
fn config_save_preserves_omitted_secret() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/config",
        r#"{"config":{"path":"/y"}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    // 2026-10-09：config 的**写配置**仍然同步（纯内存 + 原子写），只有随后的
    // restart 异步 —— 所以这里立刻读 registry 就能看到新配置，状态码是 202。
    assert_eq!(resp.status_code(), StatusCode(202));
    let reg = ctx.mod_registry.lock().unwrap();
    let cfg = reg.config("specmod").unwrap();
    assert_eq!(
        cfg["token"],
        serde_json::json!("SECRET"),
        "省略 secret 必须保留旧值：{cfg}"
    );
    assert_eq!(cfg["path"], serde_json::json!("/y"));
}

/// 非本族路径 → None。
#[test]
fn non_mods_path_returns_none() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    assert!(
        handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/chat",
            "",
            None,
            Some("application/json"),
        )
        .is_none()
    );
}

/// 空 registry，enable 未知 id → 404。
#[test]
fn enable_unknown_id_returns_404() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/enable",
        "{}",
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(404));
}

/// **回归（stabilize）**：无 body 的 enable/disable **不要求 Content-Type**。
///
/// 触发场景（无头浏览器实测）：Flutter `ModsApi.setEnabled` 发的是
/// `fetch(POST)` **不带 body、不带 Content-Type**；旧实现在此处无条件要
/// `application/json` → 恒 415，前端「Mod 管理」的启停整个点不动。
/// 口径与 `security::check_mutating_request` 一致：只有带 body 才校验 CT。
#[test]
fn bodyless_enable_disable_skip_content_type_check() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {}}}
    }));
    let origin = Some("http://127.0.0.1:18099");
    for action in ["disable", "enable"] {
        let path = format!("/api/v1/mods/specmod/{action}");
        let resp = handle_mods_route(&ctx, &Method::Post, &path, "", origin, None)
            .expect("mods 路由应命中");
        // 2026-10-09：生命周期动作改成**异步**（不堵接受循环），所以回 202 +
        // pending；本条的判据仍是「**不能是 415**」——旧实现在这里无条件要
        // Content-Type。
        assert_eq!(
            resp.status_code(),
            StatusCode(202),
            "无 body 的 {action} 必须 202（旧实现会 415）"
        );
        assert!(
            body_string(resp).contains("\"pending\":true"),
            "异步受理必须如实回 pending=true（不把「还在启动」写成成功）"
        );
    }
}

/// 带 body 的 mutating 仍必须 `application/json`（防表单 CSRF 不回退）。
#[test]
fn body_with_non_json_content_type_is_415() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {}}}
    }));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/enable",
        r#"{"config":{"on":true}}"#,
        Some("http://127.0.0.1:18099"),
        Some("text/plain"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(415));
    assert!(body_string(resp).contains("unsupported_media_type"));
}

/// 无 body + 缺 Origin + 不允许无 Origin → 403（**不是** 415）：
/// Origin 才是闸门，错误面要给对，工具客户端才知道去开 allow_no_origin。
#[test]
fn bodyless_without_origin_is_403_not_415() {
    let sec = SecurityContext::new(18099, false);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/enable",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(403),
        "缺 Origin 应回 403（origin_required），不能是 415"
    );
}

/// GET /api/v1/mods 在空 registry → 200 + `{"mods":[]}`。
#[test]
fn get_mods_list_empty() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    assert!(s.contains("\"mods\""), "body = {s}");
    assert!(s.contains("[]"), "body = {s}");
}

/// POST /api/v1/mods/{id}/config 路由匹配测试。
/// - 空 registry 时 reload_config 返回 Other → 404。
#[test]
fn config_route_match_unknown_id_returns_404() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"{"config":{"a":1}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(404));
}

/// POST /api/v1/mods/{id}/config：无效 JSON → 400。
#[test]
fn config_route_bad_json_returns_400() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"not json"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
}

/// POST /api/v1/mods/{id}/config：缺少 config 字段 → 400。
#[test]
fn config_route_missing_config_returns_400() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"{"other":"value"}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
}

/// 启用中的 Mod：200 + `{id, enabled, state}`。
#[test]
fn state_route_returns_runtime_snapshot() {
    let ctx = ctx_with_state_mod(true);
    let resp = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/stateful/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    assert!(body.contains("\"id\":\"stateful\""), "{body}");
    assert!(body.contains("\"enabled\":true"), "{body}");
    assert!(body.contains("\"counter\":7"), "应带出运行态: {body}");
}

/// **404 vs 503 的分界**：不在注册表 = 404；在册但没在跑 = 503。
#[test]
fn state_route_distinguishes_unknown_from_unavailable() {
    let ctx = ctx_with_state_mod(true);
    let unknown = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/nope/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        unknown.status_code(),
        StatusCode(404),
        "不存在的 Mod 必须 404（前端显示「没有这个 Mod」）"
    );
    let body = body_string(unknown);
    assert!(body.contains("not_found"), "{body}");

    // 同一个 Mod，停用 → 有 runtime 槽位为空 → 503。
    let mut registry = ctx.mod_registry.lock().unwrap();
    registry.disable("stateful").unwrap();
    drop(registry);
    let unavailable = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/stateful/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        unavailable.status_code(),
        StatusCode(503),
        "在册但取不到快照必须 503（前端显示「暂时读不到」）"
    );
    assert!(body_string(unavailable).contains("state_unavailable"));
}

/// 在册、在跑、但**没实现** `state_json` → 也是 503（不是 500）。
#[test]
fn state_route_without_state_json_is_503() {
    let ctx = ctx_with_mod(serde_json::json!({"mods": {"specmod": {"enabled": true}}}));
    let resp = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/specmod/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(503));
    assert!(body_string(resp).contains("state_unavailable"));
}

/// **F-0006-03 回归**：`mod_registry` 锁被毒化后，Mod 路由不得 panic。
///
/// 生产路径上那次 panic 落在 run_request_loop 的 accept 线程上（无
/// catch_unwind）⇒ 一次 Mod 侧 panic 会永久打死 HTTP 面。这里把**同一把**
/// 锁真的毒化，再打列表路由：把 lock_registry 改回 .expect(...) 就变红。
#[test]
fn poisoned_registry_lock_degrades_instead_of_panicking() {
    let ctx = ctx_with_mod(serde_json::json!({}));
    let reg = std::sync::Arc::clone(&ctx.mod_registry);
    let _ = std::thread::spawn(move || {
        let _guard = reg.lock().expect("fresh lock");
        panic!("故意 panic：毒化 mod_registry");
    })
    .join();
    assert!(ctx.mod_registry.is_poisoned(), "前提：这把锁真的被毒化了");

    let resp = handle_mods_list(&ctx);
    assert!(
        resp.status_code().0 == 200,
        "毒化后仍应给出 200 快照；panic 或 5xx 都说明降级失效"
    );
}

/// **2026-10-09（计划书 §4）**：enable 立刻回 202 + pending=true，真正的启动
/// 在后台线程里跑完 → 状态面自己变成 running。
///
/// 把 spawn_mod_op 改回同步调用、或把回执写成 200「已启用」，这条就变红。
#[test]
fn enable_is_accepted_then_settles_in_background() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": false, "config": {}}}
    }));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/enable",
        "",
        Some("http://127.0.0.1:18099"),
        None,
    )
    .expect("mods 路由应命中");
    assert_eq!(
        resp.status_code(),
        StatusCode(202),
        "受理态必须是 202（不是 200「已启用」）"
    );
    assert!(body_string(resp).contains("pending"));

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let running = {
            let reg = ctx.mod_registry.lock().unwrap();
            reg.list()
                .iter()
                .any(|(d, st, en)| d.id == "specmod" && *en && st.as_str() == "running")
        };
        if running {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "enable 没有在 5s 内落到 running（后台线程没跑完？）"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// 列表带 last_error：失败原因（含脚本输出）必须能到界面上。
#[test]
fn mods_list_carries_last_error() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {}}}
    }));
    {
        let mut reg = ctx.mod_registry.lock().unwrap();
        // 直接制造一次失败态（与「立刻退出 = Failed」同一条落点）。
        reg.mark_failed_for_tests("specmod", "脚本说：权重缺失");
    }
    let body = body_string(handle_mods_list(&ctx));
    assert!(body.contains("last_error"), "列表应带 last_error: {body}");
    assert!(
        body.contains("脚本说：权重缺失"),
        "失败原因必须在列表里: {body}"
    );
}

/// **T2（2026-10-13 执行单）**：POST /api/v1/mods/{id}/config（这里用测试 Mod 的
/// 同一个 config 分支）落盘成功后必须触发 supervisor.reload()——核心第二路
/// 按句清洗器**只在重载时**重读 mods.json 的 `motion_bias`，不重载的话
/// 「表演倾向」改了也不生效（rc4 起导演 Mod 已退役，本测试只测 config→reload
/// 这条通用路径）。
#[test]
fn config_post_reloads_supervisor_for_motion_bias() {
    let (supervisor, tmp) = crate::web_api::tests_reload::spawn_test_supervisor("mods_bias");
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"path": "/x"}}}
    }))
    .with_supervisor(std::sync::Arc::clone(&supervisor));

    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/config",
        r#"{"config":{"motion_bias":"eager"}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(202));

    // 落内存（reload_config 的同步部分）。
    {
        let reg = ctx.mod_registry.lock().unwrap();
        assert_eq!(
            reg.config("specmod").unwrap()["motion_bias"],
            serde_json::json!("eager")
        );
    }

    // 关键断言：reload_pending 被消费 = 配置保存路径真的调了 reload()。
    assert!(
        crate::web_api::tests_reload::wait_until_reload_consumed(
            &supervisor,
            std::time::Duration::from_secs(5)
        ),
        "config POST 落盘后必须 supervisor.reload()（否则 motion_bias 改了不生效）"
    );

    let _ = std::fs::remove_file(&tmp);
    drop(supervisor);
}

/// 0.3.11 启动器「进入」序列（执行单 §7）：夹具注册 local-tts-melo（拉起类替身）。
///
/// 页面在「进入」时只发现有接口的 POST：
/// - 预启动关 → 对已启用的拉起类 id 发 disable（不得 spawn）；
/// - 预启动开且勾选 → 发 enable（spawn 一次）。
///
/// 本测试直接调用同一条路由，钉住服务端语义；页面 JS 的判定规则与之对应。
/// manifest 是临时构造的，ModRegistry 的 manifest 路径未设，不碰仓库根 mods.json。
#[test]
fn launcher_enter_flow_controls_the_launch_class_mod() {
    use std::sync::atomic::Ordering;

    fn wait_status(ctx: &crate::web_api::ServerContext, id: &str, want: &str) {
        for _ in 0..200 {
            let status = {
                let registry = ctx.mod_registry.lock().unwrap();
                registry
                    .list()
                    .into_iter()
                    .find(|(d, _, _)| d.id == id)
                    .map(|(_, s, _)| s.as_str().to_string())
            };
            if status.as_deref() == Some(want) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("Mod {id} 未在 2s 内进入 {want}");
    }

    // 阶段 1：预启动关。进入前该 Mod 已启用（= 缺省启用，start_all 已拉起一次）。
    MELO_STARTS.store(0, Ordering::SeqCst);
    let ctx = ctx_with_launch_mod(serde_json::json!({
        "mods": {"local-tts-melo": {"enabled": true}}
    }));
    let boot = MELO_STARTS.load(Ordering::SeqCst);
    assert!(
        boot >= 1,
        "缺省启用应在 start_all 时启动一次（实际 {boot}）"
    );
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/local-tts-melo/disable",
        "",
        Some("http://127.0.0.1:18099"),
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(202));
    wait_status(&ctx, "local-tts-melo", "disabled");
    assert_eq!(
        MELO_STARTS.load(Ordering::SeqCst),
        boot,
        "预启动关着：进入只 disable，不得再 spawn"
    );

    // 阶段 2：预启动开且勾选。进入前该 Mod 停用（manifest enabled=false）。
    MELO_STARTS.store(0, Ordering::SeqCst);
    let ctx = ctx_with_launch_mod(serde_json::json!({
        "mods": {"local-tts-melo": {"enabled": false}}
    }));
    assert_eq!(MELO_STARTS.load(Ordering::SeqCst), 0);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/local-tts-melo/enable",
        "",
        Some("http://127.0.0.1:18099"),
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(202));
    wait_status(&ctx, "local-tts-melo", "running");
    assert_eq!(
        MELO_STARTS.load(Ordering::SeqCst),
        1,
        "预启动开且勾选：走现有 enable 生进程一次"
    );
}
// ---------------------------------------------------------------- 0.3.12 文件夹清单

/// `GET /api/v1/mods/discovered`：只读清单接口，且文件夹**不混进**工厂列表。
///
/// 测试进程的当前目录是 crate 目录（没有 `mods/`）→ 已发现 0；这里钉住的是
/// **响应形状**与「两份列表分开」这条执行单 §4 的硬要求。
#[test]
fn discovered_route_is_read_only_and_separate_from_the_factory_list() {
    let ctx = dummy_ctx(SecurityContext::new(18099, true));
    let resp = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/discovered",
        "",
        None,
        None,
    )
    .expect("本族路由");
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert!(
        json["discovered"].is_array(),
        "必须回 discovered 数组：{body}"
    );

    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    let body = body_string(resp);
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(
        json["mods"],
        serde_json::json!([]),
        "工厂列表仍只代表工厂（缺省 0 条）：{body}"
    );
}

/// `POST /api/v1/mods/discovered` 是 mutating：Origin + JSON Content-Type 都要过闸门；
/// 合法 body 在「没有可写文件夹」时是 no-op（changed 0），绝不写仓库根 `mods.json`。
#[test]
fn discovered_select_requires_json_and_origin_and_is_a_noop_when_nothing_is_selectable() {
    let path = "/api/v1/mods/discovered";
    // 严格模式（allow_no_origin=false）：缺 Origin → 403。
    let strict = dummy_ctx(SecurityContext::new(18099, false));
    let resp = handle_mods_route(
        &strict,
        &Method::Post,
        path,
        r#"{"enabled":[]}"#,
        None,
        Some("application/json"),
    )
    .expect("本族路由");
    assert_eq!(
        resp.status_code(),
        StatusCode(403),
        "mutating 必须过 Origin 闸门"
    );

    // 缺 JSON Content-Type → 415。
    let ctx = dummy_ctx(SecurityContext::new(18099, true));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        path,
        r#"{"enabled":[]}"#,
        Some("http://127.0.0.1:18099"),
        Some("text/plain"),
    )
    .expect("本族路由");
    assert_eq!(resp.status_code(), StatusCode(415));

    // 合法：当前目录没有 `mods/` → 没有可写 id，changed 0（不写盘）。
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        path,
        r#"{"enabled":[]}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .expect("本族路由");
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(json["changed"], serde_json::json!(0), "body = {body}");
    assert_eq!(json["selectable"], serde_json::json!(0), "body = {body}");
    // 0.3.13：没有可启动的文件夹 → 三个新字段都是空数组。
    assert_eq!(json["started"], serde_json::json!([]), "body = {body}");
    assert_eq!(json["stopped"], serde_json::json!([]), "body = {body}");
    assert_eq!(json["failed"], serde_json::json!([]), "body = {body}");
}

/// 坏 body（不是字符串数组）→ 400（不猜、不写盘）。
#[test]
fn discovered_select_rejects_bad_body() {
    let ctx = dummy_ctx(SecurityContext::new(18099, true));
    for body in [r#"{"enabled":"x"}"#, "{}", "not json"] {
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/discovered",
            body,
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .expect("本族路由");
        assert_eq!(resp.status_code(), StatusCode(400), "body = {body}");
    }
}

// ---------------------------------------------------------------- 0.3.13 文件夹启动

/// 建一棵临时 `mods/` 树；返回（root, mods.json 路径）。
#[cfg(unix)]
fn spawn_tree(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "l2d-0313-routes-{}-{}-{tag}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("mods")).expect("临时 mods/");
    (root.clone(), root.join("mods.json"))
}

#[cfg(unix)]
fn write_exec(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().expect("父目录")).expect("父目录");
    std::fs::write(path, body).expect("写脚本");
    let mut perms = std::fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod");
}

#[cfg(unix)]
fn ctx_with_manifest_path(path: &std::path::Path) -> ServerContext {
    let ctx = dummy_ctx(SecurityContext::new(18099, true));
    let registry = crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({}))
        .with_manifest_path(path.to_path_buf());
    *ctx.mod_registry.lock().unwrap() = registry;
    ctx
}

#[cfg(unix)]
fn wait_for(path: &std::path::Path) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

#[cfg(unix)]
fn post_select(
    ctx: &ServerContext,
    root: &std::path::Path,
    body: &str,
) -> (u16, serde_json::Value) {
    let resp = super::discovered_select_with_root(
        ctx,
        &[root.to_path_buf()],
        body,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    );
    let status = resp.status_code().0;
    (
        status,
        serde_json::from_str(&body_string(resp)).expect("JSON"),
    )
}

#[cfg(unix)]
#[test]
fn discovered_select_spawns_checked_process_and_stops_when_unchecked() {
    let (root, manifest) = spawn_tree("spawn");
    let mods_root = root.join("mods");
    let mod_dir = mods_root.join("demo-mod");
    std::fs::create_dir_all(&mod_dir).expect("mod 目录");
    std::fs::write(
        mod_dir.join("mod.toml"),
        "schema = 1\nid = \"demo-mod\"\nname = \"演示\"\nversion = \"0.1.0\"\n\
         api_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n",
    )
    .expect("清单");
    write_exec(
        &mod_dir.join("run.sh"),
        "#!/bin/sh\ntouch marker\nsleep 30\n",
    );
    let ctx = ctx_with_manifest_path(&manifest);

    let (status, json) = post_select(&ctx, &mods_root, r#"{"enabled":["demo-mod"]}"#);
    assert_eq!(status, 200, "{json}");
    assert_eq!(json["started"], serde_json::json!(["demo-mod"]), "{json}");
    assert!(
        wait_for(&mod_dir.join("marker")),
        "POST 必须真的启动勾选的 process"
    );
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).expect("mods.json"))
            .expect("JSON");
    assert_eq!(doc["mods"]["demo-mod"]["enabled"], serde_json::json!(true));

    // GET 是只读的：连刷两次也不会起第二个。
    for _ in 0..2 {
        let resp = super::handle_discovered_at(&ctx, std::slice::from_ref(&mods_root));
        assert_eq!(resp.status_code().0, 200);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        ctx.folder_processes.running_ids(),
        vec!["demo-mod".to_string()],
        "GET / 刷新不得启动进程"
    );

    let (status, json) = post_select(&ctx, &mods_root, r#"{"enabled":[]}"#);
    assert_eq!(status, 200, "{json}");
    assert_eq!(json["stopped"], serde_json::json!(["demo-mod"]), "{json}");
    assert!(
        ctx.folder_processes.running_ids().is_empty(),
        "取消勾选后必须结束"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn discovered_select_skips_on_demand_and_reports_start_failure() {
    let (root, manifest) = spawn_tree("mixed");
    let mods_root = root.join("mods");
    let lazy = mods_root.join("lazy-mod");
    std::fs::create_dir_all(lazy.join("bin")).expect("lazy 目录");
    write_exec(&lazy.join("bin/start"), "#!/bin/sh\ntouch marker\n");
    std::fs::write(
        lazy.join("mod.toml"),
        "schema = 1\nid = \"lazy-mod\"\nname = \"lazy\"\nversion = \"0.1.0\"\n\
         api_version = 1\nkind = \"launcher\"\n[launch]\nprogram = \"bin/start\"\n\
         mode = \"on_demand\"\n",
    )
    .expect("lazy 清单");
    let bad = mods_root.join("bad-mod");
    std::fs::create_dir_all(&bad).expect("bad 目录");
    std::fs::write(bad.join("run.sh"), "#!/bin/sh\ntouch marker\n").expect("无执行位");
    std::fs::write(
        bad.join("mod.toml"),
        "schema = 1\nid = \"bad-mod\"\nname = \"bad\"\nversion = \"0.1.0\"\n\
         api_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n",
    )
    .expect("bad 清单");
    let ctx = ctx_with_manifest_path(&manifest);

    let (status, json) = post_select(&ctx, &mods_root, r#"{"enabled":["lazy-mod","bad-mod"]}"#);
    assert_eq!(status, 200, "{json}");
    assert_eq!(
        json["started"],
        serde_json::json!([]),
        "on_demand 不启动：{json}"
    );
    assert_eq!(json["failed"][0]["id"], "bad-mod", "{json}");
    assert!(!lazy.join("marker").exists(), "on_demand 不得被 POST 启动");
    assert!(!bad.join("marker").exists());
    assert!(ctx.folder_processes.running_ids().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
