//! Mod 设置数据协议（纯数据 schema，前端统一渲染）。
//!
//! **禁止 Mod 注入 HTML/JS/DOM**——前端通过 `fields` 生成控件。

/// 单个下拉选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// Mod 设置字段。
#[derive(Debug, Clone, PartialEq)]
pub enum ModSettingField {
    Bool {
        key: String,
        label: String,
        default: bool,
    },
    String {
        key: String,
        label: String,
        /// 是否密钥类（前端渲染为 password 输入；内容不落日志）。
        secret: bool,
        /// **缺省值**（可空）：config 里没有这个键时，前端表单回填它、
        /// 保存时按它写入——避免「面板显示空 / 服务端却按另一个默认跑」
        /// 以及「点一次保存就把缺省语义写成显式空值」。
        ///
        /// 为什么是可选：绝大多数 String 字段的缺省语义是「空 = 未设置」
        /// （locale 回落 zh-CN、sidecar 路径由 host 推导），不需要显式值；
        /// 只有少数（语音唤醒词「小可爱」）必须让用户看见缺省。
        default: Option<String>,
    },
    Number {
        key: String,
        label: String,
        min: f64,
        max: f64,
    },
    Select {
        key: String,
        label: String,
        options: Vec<SelectOption>,
        /// **缺省值**（可空）：config 里没有这个键时表单回填它，
        /// 保存时按它写入。用于「服务端缺省不是第一个选项」的字段——
        /// 导演的动作预设正是这种（缺省是 smile，不是 none）。
        default: Option<String>,
    },
}

impl ModSettingField {
    /// 字段 key（唯一；用于配置命名空间）。
    pub fn key(&self) -> &str {
        match self {
            ModSettingField::Bool { key, .. }
            | ModSettingField::String { key, .. }
            | ModSettingField::Number { key, .. }
            | ModSettingField::Select { key, .. } => key,
        }
    }
}

/// Mod 设置面板规格（一组字段）。
#[derive(Debug, Clone, PartialEq)]
pub struct ModSettingsSpec {
    pub mod_id: String,
    pub title: String,
    pub version: u32,
    pub fields: Vec<ModSettingField>,
}

impl ModSettingsSpec {
    /// 所有字段 key 去重校验（重复 → Err）。
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for f in &self.fields {
            if !seen.insert(f.key().to_string()) {
                return Err(format!("字段 key 重复: {}", f.key()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModSettingsSpec {
        ModSettingsSpec {
            mod_id: "external-input".into(),
            title: "外部接入".into(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "enabled".into(),
                    label: "启用".into(),
                    default: true,
                },
                ModSettingField::String {
                    key: "port".into(),
                    label: "端口".into(),
                    secret: false,
                    default: None,
                },
                ModSettingField::Number {
                    key: "timeout".into(),
                    label: "超时".into(),
                    min: 0.0,
                    max: 60.0,
                },
                ModSettingField::Select {
                    key: "mode".into(),
                    label: "模式".into(),
                    options: vec![SelectOption {
                        value: "chat".into(),
                        label: "对话".into(),
                    }],
                    default: None,
                },
            ],
        }
    }

    #[test]
    fn settings_spec_validates() {
        assert!(sample().validate().is_ok());
        let dup = ModSettingsSpec {
            mod_id: "x".into(),
            title: "x".into(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "a".into(),
                    label: "a".into(),
                    default: true,
                },
                ModSettingField::String {
                    key: "a".into(),
                    label: "b".into(),
                    secret: false,
                    default: None,
                },
            ],
        };
        assert!(dup.validate().is_err());
    }

    #[test]
    fn field_keys() {
        let s = sample();
        assert_eq!(s.fields[0].key(), "enabled");
        assert_eq!(s.fields[3].key(), "mode");
    }
}
