//! F-0062-01（2026-10-05）：**保存 Mod 配置不抹 secret** 的专项回归。
//!
//! 单列文件的理由：
//! 1. `mod_registry.rs` 本体已超 code-stats 的 `>1000` 棘轮档（1547 行），不该再被
//!    测试撑大——本仓既有同款做法（如 `supervisor/tests_stall.rs`）；
//! 2. 本文件 <800 行，满足「测试文件 ≤800」。
//!
//! 背景：`GET /api/v1/mods` 对 `secret=true` 的字段**脱敏**，前端提交里没有密钥；
//! 旧的整份替换（`e.config = config`）会在保存任意普通字段时把它一并抹掉，注入
//! 端点的鉴权随之**静默失效**。修复在 `super::merge_mod_config`（规则）与
//! `super::reload_config`（入口）。

use super::*;
use live2d_ai_mod_system::ModSettingField;

/// 纯函数：合并是**按键**的——提交里没有的键保留（secret 保护的地基）。
#[test]
fn merge_mod_config_is_key_wise() {
    let old = serde_json::json!({"a": 1, "b": 2});
    let merged = merge_mod_config(&old, &serde_json::json!({"b": 3}), &[]);
    assert_eq!(
        merged,
        serde_json::json!({"a": 1, "b": 3}),
        "未提交的键必须保留"
    );
    // 提交不是对象 → 保守处理：一个键都不吸收（也不清空）。
    let merged = merge_mod_config(&old, &serde_json::Value::Null, &[]);
    assert_eq!(merged, old);
}

/// 纯函数：只有**声明的 secret 字段**的显式空串才是「清除」。
///
/// 非 secret 的空串照存（它不是删除语法）——与 `PUT /api/v1/env` 的
/// 「空值 = 清除该键」刻意区分。
#[test]
fn merge_mod_config_blank_secret_clears_only_secret_keys() {
    let old = serde_json::json!({"token": "s", "prefix": "p"});
    let secret = vec!["token".to_string()];
    // 1) 非 secret 空串照存，未提交的 secret 保留。
    let merged = merge_mod_config(&old, &serde_json::json!({"prefix": ""}), &secret);
    assert_eq!(merged["prefix"], serde_json::json!(""));
    assert_eq!(
        merged["token"],
        serde_json::json!("s"),
        "未提交的 secret 必须保留"
    );
    // 2) secret 显式空串（含纯空白）→ 删键。
    let merged = merge_mod_config(&old, &serde_json::json!({"token": "   "}), &secret);
    assert!(merged.get("token").is_none(), "显式空串应删键：{merged}");
    assert_eq!(merged["prefix"], serde_json::json!("p"));
    // 3) secret 非空串 → 覆盖。
    let merged = merge_mod_config(&old, &serde_json::json!({"token": "new"}), &secret);
    assert_eq!(merged["token"], serde_json::json!("new"));
}

/// **覆盖全部在册 Mod**：保存一个非 secret 字段（前端从 GET 的**脱敏**配置出发，
/// 提交里本来就没有密钥）后，settings_spec 声明的每个 secret 都必须仍在。
///
/// 循环遍历 `crate::AVAILABLE_MOD_FACTORIES`：将来新增带 secret 的 Mod **自动**
/// 纳入本回归，不需要有人记得来补一行。
#[test]
fn every_registered_mod_keeps_its_secret_on_config_save() {
    let mut mods = serde_json::Map::new();
    let mut covered: Vec<(&'static str, String)> = Vec::new();
    for factory in crate::AVAILABLE_MOD_FACTORIES {
        let id = factory.descriptor().id;
        let mut cfg = serde_json::Map::new();
        if let Some(spec) = factory.settings_spec() {
            for field in &spec.fields {
                if let ModSettingField::String {
                    key, secret: true, ..
                } = field
                {
                    cfg.insert(key.clone(), serde_json::json!(format!("secret-of-{id}")));
                    covered.push((id, key.clone()));
                }
            }
        }
        // 本次保存只动这个普通字段。
        cfg.insert("probe_non_secret".to_string(), serde_json::json!("v1"));
        mods.insert(
            id.to_string(),
            serde_json::json!({"enabled": false, "config": cfg}),
        );
    }
    assert!(
        covered.len() >= 2,
        "在册 Mod 至少要有两个 secret 字段（external-input / voice-input），否则本测试空转；当前 {covered:?}"
    );
    let mut reg = ModRegistry::new(
        crate::AVAILABLE_MOD_FACTORIES,
        &serde_json::json!({"mods": mods}),
    );
    for (id, key) in &covered {
        reg.reload_config(id, serde_json::json!({"probe_non_secret": "v2"}))
            .expect("在册 Mod 的配置保存必须成功");
        let cfg = reg.config(id).expect("在册");
        let expected = format!("secret-of-{id}");
        assert_eq!(
            cfg.get(key.as_str()).and_then(|v| v.as_str()),
            Some(expected.as_str()),
            "Mod {id} 的 secret `{key}` 被一次配置保存抹掉了：{cfg}"
        );
        assert_eq!(cfg["probe_non_secret"], serde_json::json!("v2"));
    }
}

/// 显式空串（工具客户端 / curl 的清除语法）真的删掉密钥；清除后
/// `token_from_config` 回 `None`（= 空 = 没配）。
#[test]
fn explicit_blank_secret_clears_stored_token() {
    let mut reg = ModRegistry::new(
        crate::AVAILABLE_MOD_FACTORIES,
        &serde_json::json!({"mods": {"external-input": {
            "enabled": false,
            "config": {"token": "cfg-secret", "text_template": "[T]{text}"}
        }}}),
    );
    reg.reload_config("external-input", serde_json::json!({"token": ""}))
        .unwrap();
    let cfg = reg.config("external-input").expect("在册").clone();
    assert!(cfg.get("token").is_none(), "显式空串应删键：{cfg}");
    assert_eq!(
        live2d_ai_mod_external_input::token_from_config(&cfg),
        None,
        "清除后不得再有 config token：{cfg}"
    );
    assert_eq!(
        cfg["text_template"],
        serde_json::json!("[T]{text}"),
        "未提交的键保留"
    );
}
