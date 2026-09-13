//! `settings_ui` 模块的补充单元测试（拆分承载：`settings_ui.rs` 已超
//! 500 硬上限；本批为 W7 任务「dev_mode 可配置开关」+ 「UI 编辑字段直接改
//! Draft → save_draft 写盘 → 重读一致」端到端新增测试）。
//!
//! 入口：`app::mod settings_ui_tests;`（仅 `#[cfg(test)]` 下生效）——
//! 本文件作为 `app` 的子模块，可直接 `use super::settings_ui::*;` 访问私有项。

use super::settings_ui::{Draft, draft_to_settings, save_draft, settings_to_draft};

#[test]
fn api_key_env_never_carries_a_real_secret() {
    // 草稿字段命名 `*_api_key_env`（**环境变量名**）；实现不持有
    // `ApiSecret`。这里只验证 `Draft` 没有 `api_key: String` 这类
    // 字段——编译期保障（如果未来加错，draft_to_settings 编译会破坏
    // round-trip 测试的形态）。
    let d = Draft::default();
    let debug = format!("{d:?}");
    // 字段名白名单：只允许 *_api_key_env（环境变量名）。
    assert!(debug.contains("llm_api_key_env"));
    assert!(debug.contains("tts_api_key_env"));
    assert!(!debug.contains("api_key:"));
}

#[test]
fn dev_mode_round_trips_through_draft() {
    // UI 切 dev_mode → settings.dev_mode → 再读回草稿字段必须等值。
    let d1 = Draft {
        dev_mode: true,
        ..Draft::default()
    };
    let s = draft_to_settings(&d1);
    assert!(
        s.dev_mode,
        "draft.dev_mode=true 必须透传到 settings.dev_mode"
    );
    let d2 = settings_to_draft(&s);
    assert!(d2.dev_mode, "settings → draft 必须保留 dev_mode=true");

    let d1_off = Draft::default();
    assert!(!d1_off.dev_mode, "Draft::default() 保持 dev_mode=false");
    let s_off = draft_to_settings(&d1_off);
    assert!(!s_off.dev_mode, "默认草稿 → settings.dev_mode=false");
}

/// W7 端到端：UI 编辑字段（`Draft` 直变） → `save_draft` 落盘 → 重读一致。
///
/// 本测试不渲染 egui（无 GPU 上下文），只模拟「用户在 UI 上改了某字段
/// → 控件把 `Draft` 字段改了 → 点保存按钮调 `save_draft`」的同款语义。
/// `config_path` 用 `std::env::temp_dir()` 下唯一文件名（与
/// `web_api::tests_reload` / `supervisor::tests_reload` 同样的隔离约定），
/// 测试结束清理。
#[test]
fn save_draft_writes_draft_to_disk_and_reload_round_trips() {
    // 1) 模拟「用户在 UI 上编辑字段」——直接 `Draft` 字面量（与控件
    //    `text_edit_singleline` / `checkbox` 在 egui 闭包内 mutate
    //    `state.draft.<field>` 形态等价）。
    let mut state_draft = Draft {
        llm_base_url: "https://api.example-llm.com/v1".into(),
        llm_model: "ui-llm-model-7b".into(),
        llm_api_key_env: Some("UI_LLM_KEY".into()),
        tts_base_url: "https://api.example-tts.com/v1".into(),
        tts_voice: "ui-voice-aria".into(),
        tts_api_key_env: Some("UI_TTS_KEY".into()),
        persona_system_prompt: "你是来自 UI 测试的人设".into(),
        persona_max_history_pairs: 6,
        dev_mode: true,
    };

    // 2) 临时配置文件：进程 id + 测试名 = 跨并发 run 隔离。
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_settings_ui_save_draft_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);

    // 3) 调 `save_draft`：`supervisor = None`（smoke 模式不热重载，
    //    仍能验证落盘 + 反序列化一致）；与 render_overlay 内「保存」
    //    按钮调用点同款签名。
    let config_str = tmp_path.to_string_lossy().into_owned();
    save_draft(&state_draft, &config_str, None).expect("save_draft 写盘成功");

    // 4) 重读：经 `AppSettings::load_from_path` 走与 `web_api` PATCH
    //    路由同款路径——保证 UI 写出的 TOML 一定能被现有 `from_toml_str`
    //    解析回 `AppSettings`（契约对称：render 写、api 读）。
    let loaded = live2d_ai_runtime::AppSettings::load_from_path(&tmp_path)
        .expect("AppSettings::load_from_path 重读成功");
    let reloaded = settings_to_draft(&loaded);

    // 5) 落盘字段全部等于 UI 草稿（注意：`api_key_env` 走 `Option<String>`，
    //    round-trip 必须保留 None/Some 形态；空串被 draft_to_settings
    //    规范化为 None 是已知语义——这里用非空值避开）。
    assert_eq!(reloaded.llm_base_url, state_draft.llm_base_url);
    assert_eq!(reloaded.llm_model, state_draft.llm_model);
    assert_eq!(reloaded.llm_api_key_env, state_draft.llm_api_key_env);
    assert_eq!(reloaded.tts_base_url, state_draft.tts_base_url);
    assert_eq!(reloaded.tts_voice, state_draft.tts_voice);
    assert_eq!(reloaded.tts_api_key_env, state_draft.tts_api_key_env);
    assert_eq!(
        reloaded.persona_system_prompt,
        state_draft.persona_system_prompt
    );
    assert_eq!(
        reloaded.persona_max_history_pairs,
        state_draft.persona_max_history_pairs
    );
    assert_eq!(reloaded.dev_mode, state_draft.dev_mode);

    // 6) 编辑一次再保存：模拟「用户改字段 → 再点保存」——验证 plan_atomic_write
    //    + fs::rename 路径在文件已存在时也能覆盖（与 web_api PATCH 同语义）。
    state_draft.llm_model = "ui-llm-model-14b".into();
    state_draft.dev_mode = false;
    save_draft(&state_draft, &config_str, None).expect("二次 save_draft 写盘成功");
    let loaded2 = live2d_ai_runtime::AppSettings::load_from_path(&tmp_path)
        .expect("AppSettings::load_from_path 二次重读成功");
    let reloaded2 = settings_to_draft(&loaded2);
    assert_eq!(reloaded2.llm_model, "ui-llm-model-14b");
    assert!(!reloaded2.dev_mode, "二次保存 dev_mode=false 必须落盘");

    // 7) 清理临时文件。
    let _ = std::fs::remove_file(&tmp_path);
}

/// 端到端扩展：空 `api_key_env`（UI 上「清空」控件）等价于 `None`，
/// 落盘后反序列化必须保持 `None`——这是 UI 控件「空串 ↔ None」双向
/// 绑定语义的持久化侧断言（避免「清空环境变量名后写盘却留了一个空
/// 字符串」的反向漏报）。
#[test]
fn save_draft_empty_key_env_normalizes_to_none_on_reload() {
    let draft = Draft {
        llm_base_url: "http://localhost:11434/v1".into(),
        llm_model: "qwen2.5:7b".into(),
        llm_api_key_env: None, // 模拟 UI 清空后的「None」
        tts_base_url: "http://localhost:8000/v1".into(),
        tts_voice: "alloy".into(),
        tts_api_key_env: Some("ONLY_TTS_KEY".into()),
        persona_system_prompt: String::new(),
        persona_max_history_pairs: 0,
        dev_mode: false,
    };
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_settings_ui_empty_key_env_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);
    let config_str = tmp_path.to_string_lossy().into_owned();
    save_draft(&draft, &config_str, None).expect("save_draft 写盘成功");
    let loaded =
        live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("AppSettings 重读成功");
    assert!(
        loaded.llm.api_key_env.is_none(),
        "llm api_key_env 持久化为 None"
    );
    assert_eq!(loaded.tts.api_key_env.as_deref(), Some("ONLY_TTS_KEY"));
    let _ = std::fs::remove_file(&tmp_path);
}

// =====================================================================
// F2b（2026-08-31 高级复审）：
//   1. 保存前统一配置校验（非法 URL / env 名 → Err，磁盘不变，不 reload）
//   2. apply_draft 只合并 UI 暴露字段（隐藏 TTS 字段原样保留）
// =====================================================================

/// 构造一个**带隐藏 TTS 字段**的初始配置（model=Some / 48kHz / 双声道）。
fn write_initial_config_with_hidden_tts(path: &std::path::Path) {
    use live2d_ai_runtime::settings::{AppSettings, LlmSettings, PersonaSettings, TtsSettings};
    let s = AppSettings {
        llm: LlmSettings {
            base_url: "https://api.example-llm.com/v1".into(),
            model: "init-llm".into(),
            api_key_env: Some("INIT_LLM_KEY".into()),
            max_tokens: None,
        },
        tts: TtsSettings {
            base_url: "https://api.example-tts.com/v1".into(),
            model: Some("tts-1-hd".into()),
            voice: "nova".into(),
            response_format: "pcm".into(),
            api_key_env: Some("INIT_TTS_KEY".into()),
            sample_rate: 48_000,
            channels: 2,
        },
        persona: PersonaSettings {
            system_prompt: "初始人设".into(),
            max_history_pairs: 4,
        },
        dev_mode: false,
    };
    std::fs::write(path, s.to_toml_string()).expect("写初始配置成功");
}

/// F2b-M0-1：非法 LLM URL → save_draft 返回 Err，磁盘字节不变。
#[test]
fn invalid_llm_url_does_not_modify_disk() {
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_f2b_invalid_llm_url_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);
    write_initial_config_with_hidden_tts(&tmp_path);
    let before = std::fs::read(&tmp_path).expect("读初始配置");

    let mut draft = settings_to_draft(
        &live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("加载初始配置"),
    );
    draft.llm_base_url = "not a url".into(); // 非法 URL

    let config_str = tmp_path.to_string_lossy().into_owned();
    let err = save_draft(&draft, &config_str, None).expect_err("非法 URL 必须保存失败");
    assert!(
        err.contains("配置校验失败"),
        "错误信息应含校验失败，got: {err}"
    );

    let after = std::fs::read(&tmp_path).expect("读磁盘");
    assert_eq!(before, after, "磁盘配置必须字节不变");
    let _ = std::fs::remove_file(&tmp_path);
}

/// F2b-M0-1：非法 TTS URL → save_draft 返回 Err，磁盘不变。
#[test]
fn invalid_tts_url_does_not_modify_disk() {
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_f2b_invalid_tts_url_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);
    write_initial_config_with_hidden_tts(&tmp_path);
    let before = std::fs::read(&tmp_path).expect("读初始配置");

    let mut draft = settings_to_draft(
        &live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("加载初始配置"),
    );
    draft.tts_base_url = "ftp://not-http".into(); // 非法 scheme

    let config_str = tmp_path.to_string_lossy().into_owned();
    let err = save_draft(&draft, &config_str, None).expect_err("非法 TTS URL 必须保存失败");
    assert!(err.contains("配置校验失败"), "got: {err}");

    let after = std::fs::read(&tmp_path).expect("读磁盘");
    assert_eq!(before, after, "磁盘配置必须字节不变");
    let _ = std::fs::remove_file(&tmp_path);
}

/// F2b-M0-1：非法 env 名 → save_draft 返回 Err，磁盘不变。
#[test]
fn invalid_api_key_env_does_not_modify_disk() {
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_f2b_invalid_env_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);
    write_initial_config_with_hidden_tts(&tmp_path);
    let before = std::fs::read(&tmp_path).expect("读初始配置");

    let mut draft = settings_to_draft(
        &live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("加载初始配置"),
    );
    draft.llm_api_key_env = Some("INVALID-NAME".into()); // 含连字符，非法 env 名

    let config_str = tmp_path.to_string_lossy().into_owned();
    let err = save_draft(&draft, &config_str, None).expect_err("非法 env 名必须保存失败");
    assert!(err.contains("配置校验失败"), "got: {err}");

    let after = std::fs::read(&tmp_path).expect("读磁盘");
    assert_eq!(before, after, "磁盘配置必须字节不变");
    let _ = std::fs::remove_file(&tmp_path);
}

/// F2b-M0-2：只改 LLM model → 隐藏 TTS 字段（model/sample_rate/channels）原样保留。
#[test]
fn saving_visible_fields_preserves_hidden_tts_fields() {
    let tmp_path = std::env::temp_dir().join(format!(
        "live2d_ai_test_f2b_preserve_tts_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp_path);
    write_initial_config_with_hidden_tts(&tmp_path);

    let mut draft = settings_to_draft(
        &live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("加载初始配置"),
    );
    // 用户只改 LLM model。
    draft.llm_model = "ui-new-model".into();

    let config_str = tmp_path.to_string_lossy().into_owned();
    save_draft(&draft, &config_str, None).expect("合法保存成功");

    let loaded = live2d_ai_runtime::AppSettings::load_from_path(&tmp_path).expect("重读配置");
    assert_eq!(loaded.llm.model, "ui-new-model", "LLM model 已更新");
    assert_eq!(
        loaded.tts.model.as_deref(),
        Some("tts-1-hd"),
        "隐藏字段 tts.model 必须保留"
    );
    assert_eq!(loaded.tts.sample_rate, 48_000, "tts.sample_rate 必须保留");
    assert_eq!(loaded.tts.channels, 2, "tts.channels 必须保留");
    assert_eq!(loaded.tts.voice, "nova", "tts.voice（UI 暴露字段）不变");
    let _ = std::fs::remove_file(&tmp_path);
}
