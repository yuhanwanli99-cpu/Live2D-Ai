//! `[tts]` 语音来源二选一（local / cloud）的回归（2026-10-09）。
//!
//! 单列文件：`patch_tests.rs` 已经贴着 code-stats 的 `crates/*/src > 1000 行`
//! 棘轮（上限 0）——把新一族断言塞进去会直接把 `src-rs-1000` 判红。
//! 断言本体与住在 `patch_tests.rs` 里的那些**同一口径**：都走 `apply_patch`。

use crate::config::TtsProvider;
use crate::settings::patch::{PatchOutcome, SettingsPatch, apply_patch};
use crate::settings::{AppSettings, TtsMode, TtsSettings};

/// 一份**云端形态**的样本（旧口径：8000/v1 + alloy + 24 kHz + 有密钥）。
///
/// 本文件不依赖 `patch_tests::sample`（那是那个文件的私有脚手架）。
fn cloud_sample() -> AppSettings {
    AppSettings {
        tts: TtsSettings {
            mode: TtsMode::Cloud,
            provider: crate::config::TtsProvider::OpenAi,
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: Some("tts-1".into()),
            voice: "alloy".into(),
            response_format: "pcm".into(),
            api_key_env: Some("TTS_KEY".into()),
            sample_rate: 24_000,
            channels: 1,
        },
        ..Default::default()
    }
}

// ───────────────────────── 语音来源二选一（2026-10-09）

/// 缺省是**本地**：出货那份 `[tts]` 是平台缺省（非 Windows = MeloTTS 8091，
/// Windows = CosyVoice3 8011；见 `DEFAULT_TTS_*` 的 cfg 分叉）。
#[test]
fn tts_mode_defaults_to_local() {
    let d = TtsSettings::default();
    assert_eq!(d.mode, TtsMode::Local);
    assert_eq!(d.mode.as_str(), "local");
    assert_eq!(d.base_url, crate::settings::DEFAULT_TTS_BASE_URL);
    assert_eq!(d.voice, crate::settings::DEFAULT_TTS_VOICE);
    assert_eq!(d.sample_rate, 44_100);
    assert_eq!(d.channels, 1);
    assert!(d.api_key_env.is_none());
    // 旧文件没有 mode 键 → 也解成本地（向后兼容）。
    let parsed: TtsSettings =
        serde_json::from_str(r#"{"base_url":"http://127.0.0.1:8091/v1"}"#).unwrap();
    assert_eq!(parsed.mode, TtsMode::Local);
}

/// 切到「本地」→ **覆盖**同一次补丁里的端口 / 采样率（普通层不给改端口），
/// 并把这一组冻结值写进 [tts]。
#[test]
fn patch_tts_local_overwrites_port_and_spec() {
    let s = cloud_sample();
    let patch: SettingsPatch = serde_json::from_str(
        r#"{"tts":{"mode":"local","base_url":"http://127.0.0.1:9999/v1","sample_rate":48000}}"#,
    )
    .unwrap();
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.tts.mode, TtsMode::Local);
    assert_eq!(
        next.tts.base_url,
        crate::settings::DEFAULT_TTS_BASE_URL,
        "本地端口/地址固定为平台缺省"
    );
    assert_eq!(next.tts.voice, crate::settings::DEFAULT_TTS_VOICE);
    assert_eq!(next.tts.response_format, "pcm");
    assert_eq!(next.tts.sample_rate, 44_100);
    assert_eq!(next.tts.channels, 1);
    assert!(next.tts.api_key_env.is_none(), "本地不带密钥");
    assert!(next.tts.model.is_none());
}

/// 切到「云端」但没填地址 → **Err（400）**，且**不静默改回另一个**。
#[test]
fn patch_tts_cloud_without_address_is_rejected() {
    let s = AppSettings {
        tts: TtsSettings::default(),
        ..Default::default()
    };
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"tts":{"mode":"cloud","base_url":""}}"#).unwrap();
    let err = apply_patch(&s, &patch).expect_err("空地址的云端必须被拒");
    assert!(err.contains("服务地址"), "{err}");
    // 原设置一个字没动。
    assert_eq!(s.tts.mode, TtsMode::Local);
    assert_eq!(s.tts.base_url, crate::settings::DEFAULT_TTS_BASE_URL);
}

/// 切到「云端」带地址与音色 → 用用户填的；密钥绑定也随同一次补丁写入。
#[test]
fn patch_tts_cloud_uses_user_values() {
    let s = AppSettings {
        tts: TtsSettings::default(),
        ..Default::default()
    };
    let patch: SettingsPatch = serde_json::from_str(
        r#"{"tts":{"mode":"cloud","base_url":"https://tts.example/v1","voice":"nova","model":"tts-1","api_key_env":"MY_TTS_KEY"}}"#,
    )
    .unwrap();
    let (next, _) = apply_patch(&s, &patch).unwrap();
    assert_eq!(next.tts.mode, TtsMode::Cloud);
    assert_eq!(next.tts.base_url, "https://tts.example/v1");
    assert_eq!(next.tts.voice, "nova");
    assert_eq!(next.tts.model.as_deref(), Some("tts-1"));
    assert_eq!(next.tts.api_key_env.as_deref(), Some("MY_TTS_KEY"));
}

/// 云端但音色为空 → 同样拒绝（不写一个发不出声的配置）。
#[test]
fn patch_tts_cloud_without_voice_is_rejected() {
    let s = AppSettings {
        tts: TtsSettings::default(),
        ..Default::default()
    };
    let patch: SettingsPatch = serde_json::from_str(
        r#"{"tts":{"mode":"cloud","base_url":"https://tts.example/v1","voice":""}}"#,
    )
    .unwrap();
    let err = apply_patch(&s, &patch).expect_err("空音色的云端必须被拒");
    assert!(err.contains("音色"), "{err}");
}
// ------------------------- 出声分档（2026-11-16）

/// 缺省 `provider = openai`：旧 toml 不写这个键，行为与今天一字不变。
#[test]
fn tts_provider_defaults_to_openai() {
    assert_eq!(TtsSettings::default().provider, TtsProvider::OpenAi);
    assert_eq!(TtsProvider::default().as_str(), "openai");
    // 旧 toml（无 provider 键）→ 仍是 openai，且 base_url / 规格一字不动。
    let parsed: TtsSettings = serde_json::from_str(
        r#"{"base_url":"http://127.0.0.1:8091/v1","voice":"ZH","sample_rate":44100}"#,
    )
    .unwrap();
    assert_eq!(parsed.provider, TtsProvider::OpenAi);
    assert_eq!(parsed.sample_rate, 44_100);
}

/// 切到 `deepseek` 且音色在**上游白名单**内 → 保存成功。
#[test]
fn patch_tts_deepseek_with_whitelisted_voice_is_accepted() {
    let s = AppSettings::default();
    for voice in ["stella", "tide"] {
        let json = format!(r#"{{"tts":{{"provider":"deepseek","voice":"{voice}"}}}}"#);
        let patch: SettingsPatch = serde_json::from_str(&json).unwrap();
        let (next, outcome) = apply_patch(&s, &patch).unwrap();
        assert_eq!(outcome, PatchOutcome::Updated);
        assert_eq!(next.tts.provider, TtsProvider::DeepSeek);
        assert_eq!(next.tts.voice, voice);
    }
}

/// 切到 `deepseek` 但音色不在白名单 → **保存失败**（原值一个字没动）。
///
/// 这是 V1 的硬要求：不静默落一个上游会拒的音色（那会让每一句都静默失败）。
#[test]
fn patch_tts_deepseek_with_other_voice_is_rejected() {
    let s = AppSettings::default();
    for voice in ["alloy", "nova", "Stella", "", " stella"] {
        let json = format!(
            r#"{{"tts":{{"provider":"deepseek","voice":{}}}}}"#,
            serde_json::to_string(voice).unwrap()
        );
        let patch: SettingsPatch = serde_json::from_str(&json).unwrap();
        let err = apply_patch(&s, &patch).expect_err("非白名单音色必须被拒");
        assert!(err.contains("stella") && err.contains("tide"), "{err}");
    }
    // 失败后原设置对象一个字没动（apply_patch 是纯函数）。
    assert_eq!(s.tts.provider, TtsProvider::OpenAi);
}

/// 已经在 deepseek 档时，把音色改成白名单外的值同样被拒（总校验，不只是切换时校验）。
#[test]
fn patch_tts_deepseek_voice_change_is_validated_too() {
    let s = AppSettings {
        tts: TtsSettings {
            provider: TtsProvider::DeepSeek,
            voice: "stella".into(),
            ..TtsSettings::default()
        },
        ..Default::default()
    };
    let patch: SettingsPatch = serde_json::from_str(r#"{"tts":{"voice":"alloy"}}"#).unwrap();
    let err = apply_patch(&s, &patch).expect_err("白名单外的音色必须被拒");
    assert!(err.contains("DeepSeek"), "{err}");
    // 白名单内的换音色放行。
    let ok: SettingsPatch = serde_json::from_str(r#"{"tts":{"voice":"tide"}}"#).unwrap();
    let (next, _) = apply_patch(&s, &ok).unwrap();
    assert_eq!(next.tts.voice, "tide");
}

/// 未知的 provider 取值由 serde 直接拒（不静默忽略）。
#[test]
fn patch_tts_unknown_provider_is_rejected_by_serde() {
    let bad = serde_json::from_str::<SettingsPatch>(r#"{"tts":{"provider":"azure"}}"#);
    assert!(bad.is_err(), "未知 provider 必须在反序列化时就失败");
}
