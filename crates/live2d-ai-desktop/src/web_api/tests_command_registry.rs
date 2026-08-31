//! D4 命令注册表测试（D4 任务，2026-08-29）。
//!
//! 与 `command_registry.rs` 主体分离（≤500 行约束；本文件**仅**含
//! `#[cfg(test)]` 代码）。`command_registry` 主体对外暴露的
//! `COMMANDS` / `lookup` / `validate_params` / `ParsedParams` /
//! `CommandDescriptor` / `CommandKind` 全部为 `pub`，可在此直接调用。
//!
//! 覆盖：
//! 1. **清单完整性**：6 条基础动作；id 唯一 + snake_case；
//!    基础命令 id ↔ `live2d_ai_core::ActionId::ALL` 一一对应。
//! 2. **schema 形态**：基础 schema 含 strength（integer + 1..=3 +
//!    additionalProperties=false）。
//! 3. **allowed_sources**：每条命令至少含 `user_command`。
//! 4. **validate_params**：默认值 / 显式合法 / 非法 strength / 未知字段
//!    / 顶层非 object。
//! 5. **lookup**：找不到返回 None。

use crate::web_api::command_registry::{
    COMMANDS, CommandKind, ParsedParams, count, lookup, validate_params,
};
use serde_json::Value;
use serde_json::json;

/// 6 基础动作齐备。
#[test]
fn registry_has_six_commands() {
    assert_eq!(count(), 6, "D4：6 基础动作 = 6");
}

/// 6 个基础动作命令的 id ↔ `ActionId::ALL.name()` 一一对应。
#[test]
fn single_commands_align_with_action_id_names() {
    use live2d_ai_core::ActionId;
    let expected: Vec<&str> = ActionId::ALL.iter().map(|a| a.name()).collect();
    let got: Vec<&str> = COMMANDS
        .iter()
        .map(|c| match c.kind {
            CommandKind::Single { action } => action,
        })
        .collect();
    assert_eq!(got, expected, "基础命令 id ↔ ActionId 必须一一对应");
}

/// id 唯一 + snake_case。
#[test]
fn ids_are_unique_and_snake_case() {
    let mut seen = std::collections::HashSet::new();
    for c in COMMANDS.iter() {
        assert!(seen.insert(c.id), "id 重复：{}", c.id);
        assert!(
            c.id.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
            "id 非 snake_case：{}",
            c.id
        );
    }
}

/// 基础命令 schema 形态合法：type=object + properties + additionalProperties=false。
#[test]
fn single_schema_shape_is_valid() {
    for c in COMMANDS.iter() {
        let s = &c.params_schema;
        assert_eq!(s["type"], "object");
        assert!(s["properties"]["strength"].is_object());
        assert_eq!(s["additionalProperties"], false);
    }
}

/// 全部命令 allowed_sources 至少含 `user_command`。
#[test]
fn allowed_sources_include_user_command() {
    for c in COMMANDS.iter() {
        assert!(
            c.allowed_sources.contains(&"user_command"),
            "命令 {} 缺少 user_command 来源",
            c.id
        );
    }
}

/// validate_params 默认值：空 body = 缺省 strength=2。
#[test]
fn validate_single_defaults() {
    let c = lookup("live2d_perform_action_nod").unwrap();
    let p = validate_params(c, &Value::Null).unwrap();
    assert_eq!(p, ParsedParams::Single { strength: 2 });
}

/// validate_params 显式合法。
#[test]
fn validate_single_explicit_valid() {
    let c = lookup("live2d_perform_action_tilt").unwrap();
    let p = validate_params(c, &json!({"strength": 3})).unwrap();
    assert_eq!(p, ParsedParams::Single { strength: 3 });
}

/// validate_params 非法 strength → 拒绝。
#[test]
fn validate_single_rejects_illegal_strength() {
    let c = lookup("live2d_perform_action_nod").unwrap();
    let err = validate_params(c, &json!({"strength": 4})).unwrap_err();
    assert!(err.contains("strength"), "got: {err}");
    let err = validate_params(c, &json!({"strength": 0})).unwrap_err();
    assert!(err.contains("strength"), "got: {err}");
}

/// validate_params 拒绝附加字段（additionalProperties: false）。
#[test]
fn validate_single_rejects_unknown_field() {
    let c = lookup("live2d_perform_action_nod").unwrap();
    let err = validate_params(c, &json!({"strength": 2, "extra": true})).unwrap_err();
    assert!(err.contains("未知字段"), "got: {err}");
}

/// validate_params 顶层非 object → 拒绝。
#[test]
fn validate_rejects_non_object() {
    let c = lookup("live2d_perform_action_nod").unwrap();
    let err = validate_params(c, &json!("string")).unwrap_err();
    assert!(err.contains("object"), "got: {err}");
}

/// lookup 找不到返回 None。
#[test]
fn lookup_returns_none_for_missing() {
    assert!(lookup("nope").is_none());
}
