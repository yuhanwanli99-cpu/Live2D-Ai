//! 表演倾向（2026-10-13 执行单 T3）：导演卡片上的三档旋钮——**克制 / 平常 / 主动**。
//!
//! # 三档语义（口径锁死，改动需先拿到维护者新裁决）
//!
//! | 档位 | 提示词（`cue_clause`） | 模型交回空数组 |
//! | --- | --- | --- |
//! | `restrained` 克制 | 今天的提示词（逐字不变）：选不出就给空数组 | 成功不动，**不**回退规则表 |
//! | `normal` 平常（**缺省**） | 只把提示词写得更愿意选一个（问句 / 明显情绪必须选 1 个） | 同上，**不**回退规则表 |
//! | `eager` 主动 | 1~2 个，平铺直叙才允许空数组 | 规则表对**这一句原文**有动作时，改用规则表 |
//!
//! # 依赖方向
//!
//! runtime（`live2d_ai_runtime::cleaning`）不能反向依赖本 crate，所以
//! [`SentenceCleaner`](../..) 侧只收两条派生值（经 [`MotionBias::to_runtime_bias`]
//! 投影）；三档文案的**单一真源在本文件**，不要在 runtime 里再抄一份。

use live2d_ai_mod_system::{ModSettingField, SelectOption};

/// 表演倾向三档。未知或空 = [`MotionBias::Normal`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionBias {
    /// 克制 = 今天的提示词；空数组不回退。
    Restrained,
    /// 平常（**缺省**）：提示词写得更愿意选一个；空数组仍不回退。
    #[default]
    Normal,
    /// 主动：模型交回空数组且规则表对这一句有动作时，改用规则表。
    Eager,
}

impl MotionBias {
    /// 解析配置值：未知或空 = [`MotionBias::Normal`]（**绝不失败**，与其余字段同口径）。
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("restrained") => Self::Restrained,
            Some("eager") => Self::Eager,
            _ => Self::Normal,
        }
    }

    /// 稳定码（settings_spec 选项值 / 日志字段）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Restrained => "restrained",
            Self::Normal => "normal",
            Self::Eager => "eager",
        }
    }

    /// `motion_system_prompt` 里 `cues：` 那一句的正文（三档文案**锁死**，
    /// `{allowed}` = 能力集 id 列表；句号由提示词模板补）。
    pub fn cue_clause(&self, allowed: &str) -> String {
        match self {
            Self::Restrained => format!(
                "只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，\
                 不要根据别的句子或上下文，选不出就给空数组 []：{allowed}"
            ),
            Self::Normal => format!(
                "只根据这一句原文，从下面这组 id 里选 0~2 个。这一句是问句（含？或?），\
                 或能看出高兴、难过、惊讶、摇头、点头、朝左看、朝右看时，必须选 1 个最贴切的 id。\
                 看不出就给空数组 []，不要为了有动作而硬选：{allowed}"
            ),
            Self::Eager => format!(
                "只根据这一句原文，从下面这组 id 里选 1~2 个。\
                 只有整句是平铺直叙、没有情绪也没有问号时才允许空数组 []：{allowed}"
            ),
        }
    }

    /// 模型交回空数组时是否回退按句规则表（**只有主动档**为 true）。
    pub fn fallback_on_empty(&self) -> bool {
        matches!(self, Self::Eager)
    }

    /// runtime 侧投影：`SentenceCleaner` 只收两条派生值（依赖方向 director → runtime，
    /// 三档文案的单一真源仍在 [`cue_clause`](Self::cue_clause)）。
    pub fn to_runtime_bias(&self, allowed: &str) -> live2d_ai_runtime::cleaning::MotionBias {
        live2d_ai_runtime::cleaning::MotionBias::new(
            self.cue_clause(allowed),
            self.fallback_on_empty(),
        )
    }

    /// 设置面字段（Select）：键 `motion_bias`，label 提明「保存并重载后生效」
    /// （2026-10-13 执行单 T2：改档后点卡片里的「保存并应用」，或设置底部的
    /// 「保存并重载」——后者会调 `supervisor.reload()` 重建按句清洗器——
    /// 都立刻生效；**不再需要重新点火**）。
    /// 本体住本文件，`lib.rs` 只 push 一次。
    pub fn setting_field() -> ModSettingField {
        ModSettingField::Select {
            key: "motion_bias".to_string(),
            label: "表演倾向（保存并重载后生效）".to_string(),
            options: vec![
                SelectOption {
                    value: "restrained".to_string(),
                    label: "克制".to_string(),
                },
                SelectOption {
                    value: "normal".to_string(),
                    label: "平常".to_string(),
                },
                SelectOption {
                    value: "eager".to_string(),
                    label: "主动".to_string(),
                },
            ],
            default: Some("normal".to_string()),
        }
    }
}

#[cfg(test)]
mod motion_bias_tests {
    use super::*;

    /// 缺键 / 空 / 垃圾字符串 = 平常（normal）。
    #[test]
    fn motion_bias_parse_defaults_to_normal() {
        assert_eq!(MotionBias::parse(None), MotionBias::Normal, "缺键 = 平常");
        assert_eq!(
            MotionBias::parse(Some("")),
            MotionBias::Normal,
            "空串 = 平常"
        );
        for junk in ["垃圾", "NORMAL", "eagerr", "0", "true"] {
            assert_eq!(
                MotionBias::parse(Some(junk)),
                MotionBias::Normal,
                "垃圾字符串 {junk:?} = 平常"
            );
        }
        assert_eq!(
            MotionBias::parse(Some("restrained")),
            MotionBias::Restrained
        );
        assert_eq!(MotionBias::parse(Some("normal")), MotionBias::Normal);
        assert_eq!(MotionBias::parse(Some("eager")), MotionBias::Eager);
    }

    /// 三档 cue 子句逐字锁死（文案改动 = 本测试红）。
    #[test]
    fn motion_bias_cue_clauses_are_locked_verbatim() {
        let allowed = "nod, smile";
        assert_eq!(
            MotionBias::Restrained.cue_clause(allowed),
            "只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，\
             不要根据别的句子或上下文，选不出就给空数组 []：nod, smile"
        );
        assert_eq!(
            MotionBias::Normal.cue_clause(allowed),
            "只根据这一句原文，从下面这组 id 里选 0~2 个。这一句是问句（含？或?），\
             或能看出高兴、难过、惊讶、摇头、点头、朝左看、朝右看时，必须选 1 个最贴切的 id。\
             看不出就给空数组 []，不要为了有动作而硬选：nod, smile"
        );
        assert_eq!(
            MotionBias::Eager.cue_clause(allowed),
            "只根据这一句原文，从下面这组 id 里选 1~2 个。\
             只有整句是平铺直叙、没有情绪也没有问号时才允许空数组 []：nod, smile"
        );
    }

    /// 只有主动档在空数组时回退规则表。
    #[test]
    fn motion_bias_only_eager_falls_back_on_empty() {
        assert!(!MotionBias::Restrained.fallback_on_empty());
        assert!(!MotionBias::Normal.fallback_on_empty());
        assert!(MotionBias::Eager.fallback_on_empty());
    }

    /// 设置面字段：键 / label / 三选项 / 缺省 normal。
    #[test]
    fn motion_bias_setting_field_shape() {
        let field = MotionBias::setting_field();
        assert_eq!(field.key(), "motion_bias");
        let ModSettingField::Select {
            label,
            options,
            default,
            ..
        } = field
        else {
            panic!("必须是 Select");
        };
        assert_eq!(label, "表演倾向（保存并重载后生效）");
        assert_eq!(
            options
                .iter()
                .map(|o| (o.value.as_str(), o.label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("restrained", "克制"),
                ("normal", "平常"),
                ("eager", "主动")
            ]
        );
        assert_eq!(default.as_deref(), Some("normal"));
    }
}
