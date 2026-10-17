//! `settings::patch` 的三态 JSON 非法载荷回归（自 `patch_tests.rs` 拆出，
//! 保持 `patch_tests.rs` ≤500 行；测试计数不丢）。

use crate::settings::patch::{SettingsPatch, apply_patch};

use super::patch_tests::sample;

#[test]
fn json_three_state_invalid_payload_is_rejected() {
    // `base_url` 非法值走 `apply_patch` 错误路径——三态拆开后非法值的拒绝
    // 仍要在 deserializer + apply_patch 双层都生效。
    let json = r#"{"llm":{"base_url":"not a url"}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("JSON 形状合法");
    let s = sample();
    assert!(apply_patch(&s, &patch).is_err(), "非法 base_url 必须拒绝");

    // 非法 sample_rate（0）——数值类型可解，但 apply_patch 拒绝。
    let json2 = r#"{"tts":{"sample_rate":0}}"#;
    let patch2: SettingsPatch = serde_json::from_str(json2).expect("JSON 形状合法");
    assert!(apply_patch(&s, &patch2).is_err());

    // 真正的 JSON 语法错（缺右括号）—— deserializer 拒绝。
    let bad = r#"{"llm":{"base_url":"x""#;
    let parse_err = serde_json::from_str::<SettingsPatch>(bad);
    assert!(parse_err.is_err(), "JSON 语法错必须被 deserializer 拒绝");
}
