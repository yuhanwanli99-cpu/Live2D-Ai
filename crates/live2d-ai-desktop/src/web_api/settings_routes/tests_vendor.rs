//! 0.3.4：厂商快照错误映射的回归（独立文件，保持 `tests.rs` ≤800 行）。

use super::*;

use super::tests::{no_keys, sample};

/// LLM 厂商快照里的非法 `api_key_env` -> 400，`details.section` 必须是 llm
/// （此前误报 tts；仍 400、文件不动）。
#[test]
fn llm_vendor_invalid_env_name_reports_llm_section() {
    let mut current = sample();
    current.llm.vendors.insert(
        "bad".into(),
        live2d_ai_runtime::settings::LlmVendor {
            base_url: "https://api.example/v1".into(),
            model: "m".into(),
            api_key_env: Some("9 BAD".into()),
            ..Default::default()
        },
    );
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            vendor: Some(Some("bad".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let err = apply_and_write(
        &current,
        &patch,
        "/nonexistent/live2d-ai.toml",
        None,
        &no_keys,
        "",
    )
    .expect_err("非法 api_key_env 必须 400");
    assert_eq!(err.error.code, "invalid_env_name");
    let details = err.error.details.expect("details");
    assert_eq!(details["section"], "llm", "{details}");
}
