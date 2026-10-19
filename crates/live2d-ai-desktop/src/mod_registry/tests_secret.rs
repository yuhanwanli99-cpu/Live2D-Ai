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
//!
//! 2026-10-19（一 Mod 一本地仓库）：本文件**不再引用任何已移出主仓的 Mod crate**，
//! 也不依赖 `AVAILABLE_MOD_FACTORIES` 非空——用**测试专用 ModFactory** 覆盖两条
//! 语义：① 保存非 secret 字段不抹 secret；② 显式空串清 secret。
//! 原先那条「循环全部在册 Mod、断言 covered >= 2」的测试已删除：缺省工厂表为 0，
//! 那个循环必然空转（假绿灯），规则改由下面这两个用例直接钉住。

use super::*;
use live2d_ai_mod_system::ModSettingField;

/// 测试专用工厂：`settings_spec` 含一个 secret String（`token`）+ 一个普通
/// String（`path`）。**不进产品** `AVAILABLE_MOD_FACTORIES`。
struct SecretSpecMod;

impl ModFactory for SecretSpecMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "secretmod",
            name: "SecretMod",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }

    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(ModSettingsSpec {
            mod_id: "secretmod".to_string(),
            title: "密钥回归".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::String {
                    key: "token".to_string(),
                    label: "密钥".to_string(),
                    secret: true,
                    default: None,
                },
                ModSettingField::String {
                    key: "path".to_string(),
                    label: "路径".to_string(),
                    secret: false,
                    default: None,
                },
            ],
        })
    }

    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(NoopRuntime))
    }
}

/// 本回归不启用 Mod，runtime 只为满足工厂契约。
struct NoopRuntime;

impl ModRuntime for NoopRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
}

/// 测试工厂表（仅本文件用；缺省产品工厂表为空）。
static FACTORIES: &[&dyn ModFactory] = &[&SecretSpecMod];

fn registry_with(config: serde_json::Value) -> ModRegistry {
    ModRegistry::new(
        FACTORIES,
        &serde_json::json!({"mods": {"secretmod": {"enabled": false, "config": config}}}),
    )
}

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

/// **保存非 secret 字段不抹 secret**（F-0062-01 的行为级回归）。
///
/// 模拟前端：GET 的脱敏配置里没有密钥，用户只改普通字段后提交。旧实现整份替换
/// 会把 `token` 抹掉 ⇒ 注入端点鉴权**静默失效**（这正是 F-0062-01）。
#[test]
fn saving_non_secret_field_keeps_secret() {
    let mut reg = registry_with(serde_json::json!({"token": "s3cret", "path": "/old"}));
    reg.reload_config("secretmod", serde_json::json!({"path": "/new"}))
        .expect("在册 Mod 的配置保存必须成功");
    let cfg = reg.config("secretmod").expect("secretmod 在册");
    assert_eq!(
        cfg["token"],
        serde_json::json!("s3cret"),
        "secret 被一次普通保存抹掉了：{cfg}"
    );
    assert_eq!(cfg["path"], serde_json::json!("/new"), "普通字段应更新");
}

/// **显式空串清 secret**（工具客户端 / curl 的清除语法）。
///
/// 清除后键被删除（= 未配置）；未提交的普通字段保留；普通字段的空串只是值，
/// **不是**删除语法（与 `PUT /api/v1/env` 的「空 = 清除」刻意区分）。
#[test]
fn explicit_blank_secret_clears_stored_token() {
    let mut reg = registry_with(serde_json::json!({"token": "cfg-secret", "path": "/p"}));
    reg.reload_config("secretmod", serde_json::json!({"token": "   "}))
        .expect("显式空串清除必须成功");
    let cfg = reg.config("secretmod").expect("secretmod 在册").clone();
    assert!(cfg.get("token").is_none(), "显式空串应删键：{cfg}");
    assert_eq!(cfg["path"], serde_json::json!("/p"), "未提交的键保留");

    reg.reload_config("secretmod", serde_json::json!({"path": ""}))
        .expect("非 secret 空串保存必须成功");
    let cfg = reg.config("secretmod").expect("secretmod 在册").clone();
    assert_eq!(cfg["path"], serde_json::json!(""), "非 secret 空串照存");
    assert!(
        cfg.get("token").is_none(),
        "已清除的 secret 不该被一次空串保存复活：{cfg}"
    );
}
