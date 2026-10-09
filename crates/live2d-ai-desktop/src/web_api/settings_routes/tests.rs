//! `settings_routes` 模块的单元测试（拆分承载：保持 mod.rs ≤ 500）。
//!
//! 入口：`settings_routes::mod tests;`（仅 `#[cfg(test)]` 下生效）。
//!
//! P5（2026-09）新增：**单一三态规则**——`{"llm":{"api_key_env":null}}`
//! 直接就是清除（磁盘上该键整行被移除），`{"llm":{"api_key_env":"NEW"}}`
//! 设为新值；不再有 `clear_api_key` 标志与 inject 改写层。另含写盘路径
//! 安全（父目录创建 + rename 失败 tmp 清理）。

use super::*;

fn sample() -> AppSettings {
    AppSettings {
        // 2026-09-16：action 段默认（幅度倍率）。
        action: Default::default(),
        // 2026-09-22：表演层默认关（[performance] 段，客户端在 host 侧构造）。
        performance: Default::default(),
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen2.5:7b".into(),
            api_key_env: Some("LIVE2D_AI_LLM_API_KEY".into()),
            max_tokens: None,
            show_reasoning: None,
        },
        tts: live2d_ai_runtime::settings::TtsSettings {
            mode: live2d_ai_runtime::settings::TtsMode::Cloud,
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: Some("tts-1".into()),
            voice: "alloy".into(),
            response_format: "pcm".into(),
            api_key_env: Some("LIVE2D_AI_TTS_API_KEY".into()),
            sample_rate: 24_000,
            channels: 1,
        },
        persona: live2d_ai_runtime::settings::PersonaSettings {
            system_prompt: "你是桌宠".into(),
            max_history_pairs: 2,
        },
        dev_mode: false,
    }
}

/// 注入 lookup：**任何键都读不到值**（= 未配置密钥）。
///
/// 测试一律用注入的 fake lookup，绝不 `std::env::set_var`——那会污染
/// 同进程其它测试，也绕过了「读密钥只走 secrets::lookup」的纪律。
fn no_keys(_name: &str) -> Option<String> {
    None
}

/// 注入 lookup：`sample()` 声明的两个键都读得到**非空**值。
fn keys_present(name: &str) -> Option<String> {
    match name {
        "LIVE2D_AI_LLM_API_KEY" | "LIVE2D_AI_TTS_API_KEY" => Some("sk-test".to_string()),
        _ => None,
    }
}

/// 读响应体（tiny_http Cursor 后端）。
fn body(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    use std::io::Read;
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

fn tempdir_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("live2d_ai_test_{name}_{}.toml", std::process::id()));
    p
}

fn unique_tmp(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "live2d_ai_test_{name}_{}_{}.toml",
        std::process::id(),
        n,
    ));
    p
}

fn init_cfg_with(path: &std::path::Path, settings: &AppSettings) {
    let _ = std::fs::remove_file(path);
    std::fs::write(path, settings.to_toml_string()).unwrap();
}

#[test]
fn get_response_never_leaks_key() {
    let resp = handle_get(&sample(), &no_keys, "");
    assert_eq!(resp.status_code().0, 200);
}

/// P2：`GET /api/v1/settings` 的 `has_api_key` = 「值真的读得到」。
///
/// 同一份配置在两种注入 lookup 下必须给出相反结果——这就是 P2 要守的
/// 单一语义；响应里仍**不得**出现环境变量名。
#[test]
fn get_response_has_api_key_reflects_injected_lookup() {
    let s = sample();
    let with = body(handle_get(&s, &keys_present, ""));
    let without = body(handle_get(&s, &no_keys, ""));
    assert!(with.contains("\"has_api_key\":true"), "{with}");
    assert!(without.contains("\"has_api_key\":false"), "{without}");
    assert!(
        !with.contains("LIVE2D_AI_LLM_API_KEY") && !with.contains("LIVE2D_AI_TTS_API_KEY"),
        "GET 不得回环境变量名：{with}"
    );
    assert!(!with.contains("sk-test"), "GET 不得回密钥值：{with}");
}

/// P2：PATCH 响应与 GET **同口径**（都走 `settings_to_view_with_keys`）。
#[test]
fn patch_response_has_api_key_reflects_injected_lookup() {
    let tmp = tempdir_path("p2_patch_has_key");
    let current = sample();
    init_cfg_with(&tmp, &current);
    // 与现值相同的补丁 → NoChange 分支；视图仍必须反映注入 lookup。
    let resp = handle_patch(
        &current,
        r#"{"llm":{"model":"qwen2.5:7b"}}"#,
        &tmp.to_string_lossy(),
        None,
        &keys_present,
        "",
    );
    assert_eq!(resp.status_code().0, 200);
    let text = body(resp);
    assert!(text.contains("\"has_api_key\":true"), "{text}");
    assert!(
        !text.contains("LIVE2D_AI_LLM_API_KEY"),
        "PATCH 不得回环境变量名：{text}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ===== 段级三态解析：PatchBody 直接持有 runtime patch =====

#[test]
fn patch_body_section_parsing() {
    // 段内字段**直接**落进 runtime LlmPatch（P5 删掉了 flatten 包装层）。
    let body = PatchBody::parse(r#"{"llm":{"base_url":"http://x/"}}"#).unwrap();
    let llm = body.llm.unwrap().unwrap();
    assert_eq!(llm.base_url, Some(Some("http://x/".into())));
    // tts 段独立
    let body = PatchBody::parse(r#"{"tts":{"voice":"nova"}}"#).unwrap();
    let tts = body.tts.unwrap().unwrap();
    assert_eq!(tts.voice, Some(Some("nova".into())));
    // 段级 null = Some(None)（整段清空）
    let body = PatchBody::parse(r#"{"llm":null}"#).unwrap();
    assert!(body.llm.unwrap().is_none());
}

/// 段级三态在「D4 把 `serde_with` 换成 crate 内同语义助手」后仍必须成立：
/// 段缺省 → `None`；段 `null` → `Some(None)`；段对象（含 `{}`）→ `Some(Some(..))`；
/// 顶层 `dev_mode` 同款三态。
#[test]
fn patch_body_absent_section_is_none_and_null_is_some_none() {
    // 段缺省：整份 body 里什么都没有。
    let body = PatchBody::parse("{}").unwrap();
    assert!(body.llm.is_none(), "段缺省不得被当成「清空」");
    assert!(body.tts.is_none());
    assert!(body.persona.is_none());
    assert!(body.action.is_none());
    assert!(body.dev_mode.is_none(), "dev_mode 缺省 = 不修改");

    // 段对象（`{}`）= Some(Some(..))：空对象是「段级 no-op」，不是清空。
    let body = PatchBody::parse(r#"{"tts":{}}"#).unwrap();
    assert!(
        body.tts.expect("tts 段在场").is_some(),
        "`{{}}` 必须解析成 Some(Some(..))"
    );
    assert!(body.llm.is_none(), "只写了 tts，llm 仍必须是缺省");

    // `dev_mode` 三态：值 / null / 缺省 三者互不相同。
    assert_eq!(
        PatchBody::parse(r#"{"dev_mode":false}"#).unwrap().dev_mode,
        Some(Some(false))
    );
    assert_eq!(
        PatchBody::parse(r#"{"dev_mode":null}"#).unwrap().dev_mode,
        Some(None),
        "显式 null = 显式关闭，不能与缺省塌缩成同一个值"
    );
    assert_eq!(
        PatchBody::parse(r#"{"dev_mode":true}"#).unwrap().dev_mode,
        Some(Some(true))
    );
}

/// 「段在场但没写 `api_key_env`」**不是**清除——字段缺省 = 不修改。
///
/// 这正是 P0-2 当年用 `clear_api_key` 想守的东西；P5 后由字段级三态本身
/// 保证（`None` ≠ `Some(None)`），不再需要任何改写层。
#[test]
fn absent_api_key_env_field_is_not_a_clear() {
    let body = PatchBody::parse(r#"{"llm":{"model":"m"}}"#).unwrap();
    let llm = body.llm.unwrap().unwrap();
    assert_eq!(llm.model, Some(Some("m".into())));
    assert_eq!(llm.api_key_env, None, "没写的字段不得变成「清除」");
    assert_eq!(llm.base_url, None);
}

/// 旧契约的 `clear_api_key` 现在是**未知键**：serde 缺省忽略——
/// 既不触发清除也不让请求失败。清除只能靠 `api_key_env: null`。
#[test]
fn legacy_clear_api_key_key_no_longer_clears() {
    let body = PatchBody::parse(r#"{"llm":{"clear_api_key":true}}"#).unwrap();
    let llm = body.llm.unwrap().unwrap();
    assert_eq!(llm.api_key_env, None, "旧标志不得再表达清除");
}

// ===== 端到端：单一三态规则（P5）+ 段级独立 =====

/// 跑一条 PATCH + 校验磁盘状态。
fn patch_and_check(current: &AppSettings, name: &str, body: &str, assert: impl Fn(&AppSettings)) {
    let tmp = tempdir_path(name);
    init_cfg_with(&tmp, current);
    let resp = handle_patch(current, body, &tmp.to_string_lossy(), None, &no_keys, "");
    assert_eq!(resp.status_code().0, 200);
    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert(&after);
    let _ = std::fs::remove_file(&tmp);
}

/// **单一规则**：`{"llm":{"api_key_env":null}}` → 磁盘上该键整行被移除。
#[test]
fn e2e_api_key_env_null_clears_binding_on_disk() {
    let current = sample();
    let tmp = tempdir_path("p5_clear_llm_key");
    init_cfg_with(&tmp, &current);
    let before = std::fs::read_to_string(&tmp).expect("read before");
    assert!(
        before.contains("LIVE2D_AI_LLM_API_KEY"),
        "起点应有绑定：\n{before}"
    );

    let resp = handle_patch(
        &current,
        r#"{"llm":{"api_key_env":null}}"#,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "",
    );
    assert_eq!(resp.status_code().0, 200);

    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert_eq!(after.llm.api_key_env, None, "内存值应被清除");
    let raw = std::fs::read_to_string(&tmp).expect("read after");
    assert!(
        !raw.contains("LIVE2D_AI_LLM_API_KEY"),
        "llm 的 api_key_env 整行应被移除：\n{raw}"
    );
    assert!(
        raw.contains("LIVE2D_AI_TTS_API_KEY"),
        "tts 绑定不得被误清：\n{raw}"
    );
    let _ = std::fs::remove_file(&tmp);
}

/// `{"llm":{"api_key_env":"NEW"}}` → 设为新值（llm / tts 两段同规则）。
#[test]
fn e2e_api_key_env_set_new_value_persists() {
    let current = sample();
    patch_and_check(
        &current,
        "p5_set_llm_key",
        r#"{"llm":{"api_key_env":"NEW_LLM_KEY"}}"#,
        |after| assert_eq!(after.llm.api_key_env, Some("NEW_LLM_KEY".into())),
    );
    patch_and_check(
        &current,
        "p5_set_tts_key",
        r#"{"tts":{"api_key_env":"NEW_TTS_KEY"}}"#,
        |after| {
            assert_eq!(after.tts.api_key_env, Some("NEW_TTS_KEY".into()));
            assert_eq!(after.llm.api_key_env, current.llm.api_key_env);
        },
    );
}

/// 段级独立：只清 LLM + 改 TTS voice → TTS 绑定保留（无需任何标志）。
#[test]
fn e2e_clear_one_provider_does_not_touch_the_other() {
    let current = sample();
    patch_and_check(
        &current,
        "p5_clear_llm_only",
        r#"{"llm":{"api_key_env":null},"tts":{"voice":"nova"}}"#,
        |after| {
            assert_eq!(after.llm.api_key_env, None);
            assert_eq!(after.tts.api_key_env, current.tts.api_key_env);
            assert_eq!(after.tts.voice, "nova");
        },
    );
    patch_and_check(
        &current,
        "p5_clear_tts_only",
        r#"{"llm":{"model":"qwen2.5:14b"},"tts":{"api_key_env":null}}"#,
        |after| {
            assert_eq!(after.tts.api_key_env, None);
            assert_eq!(after.llm.api_key_env, current.llm.api_key_env);
            assert_eq!(after.llm.model, "qwen2.5:14b");
        },
    );
}

/// 只改同段其它字段（llm 段在场但没给 `api_key_env`）→ 绑定保持原值。
///
/// 这是「只改一个字段不得清空同段其它字段」那条最危险的回归；P5 后由
/// 字段级三态本身保证，不再依赖 inject 改写。
#[test]
fn e2e_partial_section_patch_keeps_untouched_api_key_env() {
    let current = sample();
    patch_and_check(
        &current,
        "p5_partial_llm",
        r#"{"llm":{"model":"qwen2.5:14b"}}"#,
        |after| {
            assert_eq!(after.llm.api_key_env, current.llm.api_key_env);
            assert_eq!(after.llm.model, "qwen2.5:14b");
        },
    );
}

// ===== 写盘路径安全：父目录创建 + tmp 清理 =====

#[test]
fn apply_and_write_creates_parent_dir_when_missing() {
    // 父目录不存在 → 第一次写盘应创建并成功。
    let mut base = std::env::temp_dir();
    base.push(format!("live2d_ai_test_p0a_mkdir_{}", std::process::id()));
    let target = base.join("nested").join("config.toml");
    let _ = std::fs::remove_dir_all(&base);

    let current = sample();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("first-write".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let resp = apply_and_write(
        &current,
        &patch,
        &target.to_string_lossy(),
        None,
        &no_keys,
        "",
    )
    .expect("apply+write should succeed with parent dir creation");
    assert!(resp.persisted);
    let after = AppSettings::load_from_path(&target).expect("reload");
    assert_eq!(after.llm.model, "first-write");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn apply_and_write_cleans_tmp_on_rename_failure() {
    // 模拟 rename / write tmp 失败：让 target 父路径是**文件**。
    // tmp 路径 = `<target>.tmp.<pid>` 必须写到 target 的父目录。
    // 所以"父路径是文件"指 target.parent() 必须是个文件。
    let base = unique_tmp("p0a_rename_fail");
    let _ = std::fs::remove_file(&base);
    std::fs::write(&base, "I am a file, not a dir").unwrap();
    // target 路径 = `<base>/<file>.toml` → parent = base（已存在的文件）。
    let target = base.join("config.toml");
    let current = sample();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("new".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let result = apply_and_write(
        &current,
        &patch,
        &target.to_string_lossy(),
        None,
        &no_keys,
        "",
    );
    // 父路径是文件 → create_dir_all(base) 失败 → apply_and_write 返回 Err。
    assert!(
        result.is_err(),
        "父路径是文件时写盘应失败: {:?}",
        result.as_ref().err()
    );
    // 验证无本次目标对应的 tmp.* 残留（pid + 唯一 nanos）。
    let pid = std::process::id();
    let base_name = base.file_name().unwrap().to_string_lossy().to_string();
    let leftover = std::fs::read_dir(std::env::temp_dir())
        .expect("readdir /tmp")
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.contains(&base_name) && n.contains(&format!(".tmp.{pid}"))
        });
    assert!(!leftover, "rename 失败后 tmp 应被清理");
    let _ = std::fs::remove_file(&base);
}

// ===== 旧覆盖 =====

#[test]
fn apply_patch_then_persists_round_trip() {
    let tmp = tempdir_path("patch_roundtrip");
    init_cfg_with(&tmp, &AppSettings::default());
    let current = AppSettings::load_from_path(&tmp).expect("load");
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("new-model".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let resp = apply_and_write(&current, &patch, &tmp.to_string_lossy(), None, &no_keys, "")
        .expect("apply+write");
    assert!(resp.persisted);
    assert_eq!(resp.settings.llm.model, "new-model");
    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert_eq!(after.llm.model, "new-model");
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn no_change_patch_does_not_touch_disk() {
    let tmp = tempdir_path("patch_no_change");
    init_cfg_with(&tmp, &AppSettings::default());
    let first = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("first".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let _ = apply_and_write(
        &AppSettings::load_from_path(&tmp).expect("load"),
        &first,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "",
    )
    .expect("first");
    let disk = AppSettings::load_from_path(&tmp).expect("load");
    let resp =
        apply_and_write(&disk, &first, &tmp.to_string_lossy(), None, &no_keys, "").expect("second");
    assert!(!resp.persisted, "NoChange 时不应写盘");
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn url_invalid_patch_returns_400() {
    let mut current = sample();
    current.llm.base_url = "not a url".into();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            base_url: Some(Some("not a url".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let tmp = tempdir_path("url_invalid");
    let _ = std::fs::remove_file(&tmp);
    let err =
        apply_and_write(&current, &patch, &tmp.to_string_lossy(), None, &no_keys, "").unwrap_err();
    assert_eq!(err.error.code, "url_invalid");
}
#[test]
fn test_outcome_serializes_with_ok_tag() {
    let s = TestOutcome::success(42, "qwen2.5:7b");
    let j = serde_json::to_value(&s).unwrap();
    assert_eq!(j["ok"], true);
    assert_eq!(j["latency_ms"], 42);
    assert_eq!(j["model_echo"], "qwen2.5:7b");
}
#[test]
fn test_outcome_failure_serializes_with_ok_false() {
    let f = TestOutcome::failure("timeout", "请求超时");
    let j = serde_json::to_value(&f).unwrap();
    assert_eq!(j["ok"], false);
    assert_eq!(j["error"]["code"], "timeout");
}
#[test]
fn timeout_to_duration_clamps_to_bounds() {
    use std::time::Duration;
    assert_eq!(timeout_to_duration(None), Duration::from_millis(3000));
    assert_eq!(timeout_to_duration(Some(0)), Duration::from_millis(1));
    assert_eq!(timeout_to_duration(Some(8000)), Duration::from_millis(8000));
    assert_eq!(
        timeout_to_duration(Some(99_999)),
        Duration::from_millis(8000)
    );
}
#[test]
fn run_llm_test_fails_on_unconfigured_url() {
    let mut s = sample();
    s.llm.base_url.clear();
    let out = run_llm_test(&s, Some(100));
    assert!(!out.ok);
    assert_eq!(out.error.unwrap().code, "llm_unreachable");
}
#[test]
fn handle_patch_invalid_json_returns_400() {
    let tmp = tempdir_path("bad_json");
    init_cfg_with(&tmp, &AppSettings::default());
    let current = AppSettings::load_from_path(&tmp).expect("load");
    let resp = handle_patch(
        &current,
        "not json",
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "",
    );
    assert_eq!(resp.status_code().0, 400);
    let _ = std::fs::remove_file(&tmp);
}

/// `llm.max_tokens` 走完整 HTTP PATCH → 写盘 → 重载 链路。
///
/// 这里是**唯一**能证明「字段级三态 + 落盘」两者真的一起工作的地方：
/// `PatchBody` 现在直接持有 `LlmPatch`（P5 删掉了 `#[serde(flatten)]` 包装），
/// `double_option` 是否仍然区分 `0` / `null` / 缺省，只有端到端跑一遍才算数。
#[test]
fn e2e_max_tokens_tri_state_through_http_patch() {
    let current = sample();
    assert_eq!(current.llm.max_tokens, None, "起点：省略 = 用默认");

    // 1) 显式 0 = 不限制（必须与 null 区分开）。
    patch_and_check(
        &current,
        "mt_zero",
        r#"{"llm":{"max_tokens":0}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(0)),
    );

    // 2) 显式 2048 = 硬上限。
    patch_and_check(
        &current,
        "mt_set",
        r#"{"llm":{"max_tokens":2048}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(2048)),
    );

    // 3) 缺省字段 = 不动（已有值保持）。
    let capped = AppSettings {
        // 2026-09-16：action 段默认（幅度倍率）。
        action: Default::default(),
        llm: live2d_ai_runtime::settings::LlmSettings {
            max_tokens: Some(333),
            ..current.llm.clone()
        },
        ..current.clone()
    };
    patch_and_check(
        &capped,
        "mt_keep",
        r#"{"llm":{"model":"qwen2.5:14b"}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(333)),
    );

    // 4) 显式 null = 清除 → 回落默认（视图报默认值，而不是「不限制」）。
    //
    // 断言**引用常量**而不是写死数字：默认值本身是策略（2026-09-13 因推理模型
    // 的思考占用输出预算，512 → 4096），钉死数字会让「改策略」看起来像
    // 「改坏了」，而这条测试真正要守的是「null 会回落到默认、不是不限制」。
    patch_and_check(
        &capped,
        "mt_clear",
        r#"{"llm":{"max_tokens":null}}"#,
        |after| {
            assert_eq!(after.llm.max_tokens, None);
            assert_eq!(
                live2d_ai_runtime::settings::view::settings_to_view(after)
                    .llm
                    .max_tokens,
                live2d_ai_runtime::settings::DEFAULT_MAX_TOKENS
            );
        },
    );
}

/// 2026-09-16（用户可调幅度）：action 三段倍率经 HTTP PATCH 落地并钳位。
#[test]
fn e2e_action_scales_patch_clamps_via_http() {
    let current = sample();
    patch_and_check(
        &current,
        "action_scales",
        r#"{"action":{"head_scale":1.25,"body_scale":9.0,"expression_scale":0.0}}"#,
        |after| {
            assert_eq!(after.action.head_scale, 1.25);
            assert_eq!(after.action.body_scale, 2.2, "9.0 应钳到上限");
            assert_eq!(after.action.expression_scale, 0.2, "0.0 应钳到下限");
        },
    );
    // 不写 action 段的补丁 → 出厂默认保持不动。
    patch_and_check(
        &current,
        "action_defaults",
        r#"{"llm":{"model":"m2"}}"#,
        |after| {
            assert_eq!(after.action.head_scale, 0.75);
            assert_eq!(after.action.body_scale, 0.80);
            assert_eq!(after.action.expression_scale, 1.0);
        },
    );
}
// ===== 阶段5 D40：active_model_id 透传 + [action.models] 就地写回 =====

/// GET / PATCH 响应都带当前模型 id（保存后前端不丢模型名）。
#[test]
fn settings_responses_carry_active_model_id() {
    let tmp = tempdir_path("d40_active_model_id");
    let current = sample();
    init_cfg_with(&tmp, &current);

    let get = body(handle_get(&current, &no_keys, "bai"));
    assert!(get.contains(r#""active_model_id":"bai""#), "{get}");

    let resp = handle_patch(
        &current,
        r#"{"llm":{"model":"qwen2.5:7b"}}"#,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "bai",
    );
    let status = resp.status_code().0;
    let text = body(resp);
    assert_eq!(status, 200, "{text}");
    assert!(text.contains(r#""active_model_id":"bai""#), "{text}");
    // 缺省（旧调用方）一律空串。
    assert!(body(handle_get(&current, &no_keys, "")).contains(r#""active_model_id":"""#));
    let _ = std::fs::remove_file(&tmp);
}

/// 判据③：PATCH 新增 / 修改 / 删除 `[action.models.<id>]` 后，
/// 注释逐字还在、无 `.tmp.<pid>` 残留、被删条目真的消失、toml 仍可解析。
#[test]
fn e2e_action_model_overrides_keep_comments_and_prune_tables() {
    let tmp = tempdir_path("d40_models_comments");
    let base = r#"# 顶部注释：必须逐字留住
[action]
# 全局幅度注释
head_scale = 0.75
body_scale = 0.80
expression_scale = 1.0

# bai 覆盖注释（改值时要留住）
[action.models.bai]
head_scale = 1.0

[action.models.follow]
body_scale = 1.1
"#;
    std::fs::write(&tmp, base).unwrap();
    let current = AppSettings::load_from_path(&tmp).expect("load");

    let resp = handle_patch(
        &current,
        r#"{"action":{"models":{"bai":{"head_scale":1.5},"hiyori":{"body_scale":0.9},"follow":null}}}"#,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "bai",
    );
    let status = resp.status_code().0;
    let text = body(resp);
    assert_eq!(status, 200, "{text}");

    let raw = std::fs::read_to_string(&tmp).expect("read after");
    // ① 注释逐字还在。
    for needle in [
        "# 顶部注释：必须逐字留住",
        "# 全局幅度注释",
        "# bai 覆盖注释（改值时要留住）",
    ] {
        assert!(raw.contains(needle), "注释丢了：{needle}\n---\n{raw}");
    }
    // ② 修改生效 + 新增子表 + 被删模型整块消失。
    assert!(raw.contains("[action.models.bai]"), "{raw}");
    assert!(raw.contains("head_scale = 1.5"), "{raw}");
    assert!(raw.contains("[action.models.hiyori]"), "{raw}");
    assert!(
        !raw.contains("[action.models.follow]"),
        "被删模型必须消失：\n{raw}"
    );
    assert!(
        !raw.contains("body_scale = 1.1"),
        "被删模型的值必须消失：\n{raw}"
    );
    // ③ 仍可解析且值正确（bai head=1.5，body/expression 逐键回落全局）。
    let after = AppSettings::load_from_path(&tmp).expect("reload");
    let bai = after.action.models.get("bai").expect("bai");
    assert_eq!(bai.head_scale, Some(1.5));
    assert_eq!(bai.body_scale, None);
    let e = after.action.effective_for("bai");
    assert_eq!(
        (e.head_scale, e.body_scale, e.expression_scale),
        (1.5, 0.80, 1.0)
    );
    assert_eq!(after.action.models.get("follow"), None);
    // ④ 无 .tmp.<pid> 残留。
    let pid = std::process::id();
    let parent = tmp.parent().unwrap();
    let leftover = std::fs::read_dir(parent)
        .expect("readdir")
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with("live2d_ai_test_d40_models_comments")
                && n.contains(&format!(".tmp.{pid}"))
        });
    assert!(!leftover, "写盘后不得留 .tmp.<pid> 残留");
    let _ = std::fs::remove_file(&tmp);
}
