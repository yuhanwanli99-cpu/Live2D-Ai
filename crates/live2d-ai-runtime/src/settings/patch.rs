//! 设置面板的**补丁**语义层：
//!
//! - [`SettingsPatch`] + [`apply_patch`]：把 HTTP/egui 提交的部分更新三态
//!   （「不修改 / 显式清除 / 设置新值」）合并进 [`AppSettings`]，返回
//!   [`PatchOutcome`] 用于上层判断「是否需要写回磁盘」。
//! - [`plan_atomic_write`]：完整原子写回——写 tmp → `fdatasync` → 返回 tmp
//!   路径，由调用方 `rename` 到目标位置（写一半 / 崩溃不会污染原文件）。
//!
//! 这些函数**纯内存**（除了 `plan_atomic_write` 的 tmp 文件 IO），不读
//! `live2d-ai.toml`、不查环境变量、不发起 HTTP。
//!
//! # 三态 JSON 端可区分性（修复点 W6-A）
//!
//! 段级 / 字段级统一用 `Option<Option<T>>`：
//!
//! - **字段级三态**（`Option<Option<T>>`）：
//!   - JSON 缺字段 / `null` → 行为与 serde 默认不同：缺字段 = `None`（不修改），
//!     显式 `null` = `Some(None)`（显式清除）。这里靠
//!     [`::serde_with::rust::double_option`] 把 serde 的默认「缺省/null 都塌缩
//!     成 None」拆开，HTTP 端才能表达「无意识清空 vs 主动清除」。
//!   - JSON 显式值 → `Some(Some(v))`（覆盖）。
//!
//! - **段级三态**（同样是 `Option<Option<LlmPatch>>` 等）：
//!   - 段缺省 → `None`（不修改）。
//!   - 段显式 `null` → `Some(None)`（整段清空）。
//!   - 段是对象（即使 `{}` 空对象） → `Some(Some(p))`：走字段级三态合并；
//!     空对象会走「所有字段都 None」分支，等价于「段级 no-op」（不修改任何字段）。
//!
//! 字段级三态在 W6-A 后即可在 JSON 端真实可区分；段级用「null vs 对象」靠
//! 外层 `Option<Option<…>>` 的 `double_option` 自动区分。`apply_patch` 按
//! `Some(None)` → 段清空 / `Some(Some(p))` → 字段级合并的两条路径处理，详见
//! 单元测试 `patch_tests::json_three_state_*`。
//!
//! # `api_key_env`：显式 `null` 就是清除（P5 单一规则）
//!
//! 2026-09 起契约收敛为**单一规则**：`{"llm":{"api_key_env":null}}`（`tts` 段
//! 同款）**就是**清除绑定——即字段级三态的 `Some(None)`，与 `tts.model` /
//! `max_tokens` 等字段同一形状，不再是特例。
//!
//! 历史（已废除，勿复活）：D1 §1.2 P0-2 曾要求「不带 `clear_api_key` 的
//! `api_key_env: null` 视为保持原值」，由桌面层 `inject_clear_key_flag` 在
//! `apply_patch` **前**改写实现。那让一个字段有了两种「清除」语法（段级标志
//! vs 字段级 null），前端与服务端各发一套，语义漂移时没人知道该信哪个；而
//! 字段级三态本身已经能区分「缺省（不改）」与 `null`（清除），改写层是多余的。
//! **不要**再加回任何形式的 `clear_api_key`。
//!
//! # 行数豁免（AGENTS.md「源码 ≤500 行，豁免 ≤1000 需头注理由」）
//!
//! 本文件 **780 行**（2026-09-26 追加 `[action.models]` 覆盖后），超过 500 行
//! 默认上限。理由：
//!
//! 1. 测试**已经**按约定外提到 `patch_tests.rs`（约 990 行），这里的行数
//!    全部是生产代码，不是靠「把测试留在文件里」堆出来的。
//! 2. 余下内容是一个**不变量密集**的整体：`Option<Option<T>>` 三态在 JSON /
//!    TOML / HTTP / egui 四个入口的语义必须逐条对齐（`double_option` 的
//!    deserialize/serialize、段级 vs 字段级两条路径、密钥绑定的清除语义、
//!    原子写回）。拆成两个文件会让「改一半忘了另一半」变成默认风险——
//!    而这正是 W6-A 那类缺陷的成因。
//! 3. 仍在 1000 行豁免上限内；再增长时优先拆 `plan_atomic_write`（它与三态
//!    补丁语义正交，是最自然的第一刀）。

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    ActionModelOverride, ActionSettings, AppSettings, clamp_action_scale, is_valid_model_id,
};

// `serde_with::rust::double_option` 的 deserialize/serialize 函数；
// 作用在 `Option<Option<T>>` 字段上，让 JSON 端 `缺省 / null / 显式值`
// 三态真实可区分（serde 默认会把缺省 + null 都映射成 `None`）。

/// 设置面板提交的整份补丁（每段三态）。
///
/// - 段缺省 → `None`（不修改整段）；
/// - 段显式 `null` → `Some(None)`（整段显式清空）；
/// - 段是对象（含 `{}`）→ `Some(Some(p))`（字段级三态逐字段合并；空对象 = no-op）。
// 含 f32（action 倍率）→ 只能 PartialEq。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SettingsPatch {
    /// LLM 段补丁。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub llm: Option<Option<LlmPatch>>,
    /// TTS 段补丁。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub tts: Option<Option<TtsPatch>>,
    /// persona 段补丁。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub persona: Option<Option<PersonaPatch>>,
    /// action 段补丁（2026-09-16：三项动作幅度倍率）。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub action: Option<Option<ActionPatch>>,
    /// 顶层 `dev_mode` 三态（W7 任务：可配置运行时开关）。
    /// - 缺省 = `None`（不修改；与已有段级三态同构）。
    /// - `Some(None)` = 显式关闭（写回 `dev_mode = false`）。
    /// - `Some(Some(true|false))` = 显式开启 / 显式覆盖。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub dev_mode: Option<Option<bool>>,
}

/// LLM 段字段级三态补丁。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub base_url: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub model: Option<Option<String>>,
    /// 特别注意：这里用 `Option<Option<String>>` 是**故意**的——
    /// 前端可以表达「不修改 / 清除 env 绑定 / 设置新的 env 变量名」；
    /// 显式 `null`（`Some(None)`）**就是**清除——P5 单一规则，没有
    /// `clear_api_key` 标志（见模块头「`api_key_env`：显式 `null` 就是清除」）。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub api_key_env: Option<Option<String>>,
    /// 输出 token 上限（三态）：缺省=不改 / `null`=清除（回落默认） / 数字=设为该值。
    ///
    /// `0` 是**合法的显式值**，表示「不限制」，与 `null`（回落默认 512）不同。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub max_tokens: Option<Option<u32>>,
    /// 展示思考总闸（三态）：缺省=不改 / `null`=清除（回落缺省 false） /
    /// `true|false`=显式设定。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub show_reasoning: Option<Option<bool>>,
}

/// TTS 段字段级三态补丁。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub base_url: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub model: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub voice: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub api_key_env: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub sample_rate: Option<Option<u32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub channels: Option<Option<u16>>,
}

/// persona 段字段级三态补丁。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub system_prompt: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub max_history_pairs: Option<Option<usize>>,
}

/// action 段字段级三态补丁（2026-09-16）。
///
/// 三项倍率都是 f32：`Some(None)`（显式 `null`）= **回落出厂默认**，
/// `Some(Some(v))` = 设为 v（写入前钳进 `[0.2, 2.2]`）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub head_scale: Option<Option<f32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub body_scale: Option<Option<f32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub expression_scale: Option<Option<f32>>,
    /// **每模型覆盖**（阶段5 D40）：`None` = 不动整张表；`Some(map)` 逐模型处理。
    ///
    /// map 的值本身是三态：`null` = **删除该模型覆盖**；对象 = 逐键三态合并
    /// （键 `null` = 删除该键；值 = 钳后写入）。合并后三键全 `None` 的条目
    /// **从表里删掉**（不留空表）。键（模型 id）非法 → `apply_patch` 直接 `Err`，
    /// **不静默落表**。注意：map 自身序列化时用 `Option::is_none` skip——
    /// `"models": null` 与「不写 models」都是「不动」；要清空整表得逐个置 `null`
    /// 或段级 `action: null`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<BTreeMap<String, Option<ActionModelOverridePatch>>>,
}

/// 单个模型覆盖的字段级三态补丁（`[action.models.<id>]` 的一行）。
///
/// 与 [`ActionPatch`] 的三键同款 `Option<Option<f32>>` + `double_option`：
/// 缺省 = 不改该键 / `null` = 删除该键（回落全局）/ 值 = 设为钳后的值。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionModelOverridePatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub head_scale: Option<Option<f32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub body_scale: Option<Option<f32>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "::serde_with::rust::double_option::deserialize",
        serialize_with = "::serde_with::rust::double_option::serialize"
    )]
    pub expression_scale: Option<Option<f32>>,
}

/// `apply_patch` 的结果——上层据此决定要不要触发磁盘写回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutcome {
    /// 补丁与当前值完全一致，无需写回。
    NoChange,
    /// 至少一个字段被修改，需要写回。
    Updated,
}

// ===== 工具：env 名校验（与 settings.rs 里的私有实现同步）=====

fn validate_env_name_strict(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(format!(
            "api_key_env 非法: {name:?}（仅允许 ASCII 字母/数字/下划线，且不以数字开头）"
        ))
    }
}

// ===== [action.models] 每模型覆盖（阶段5 D40）=====

/// 把一个模型覆盖的**单键**三态补丁写进 `slot`。
///
/// - `None` = 不改；
/// - `Some(None)` = 删除该键（回落全局）；
/// - `Some(Some(v))` = `clamp_action_scale(v)`；`v` 非有限（NaN / 无穷）按
///   既有 `set_scale` 口径回落 `fallback`（出厂默认）。
fn apply_model_key(slot: &mut Option<f32>, fallback: f32, v: &Option<Option<f32>>) {
    match v {
        None => {}
        Some(None) => *slot = None,
        Some(Some(raw)) => {
            *slot = Some(if raw.is_finite() {
                clamp_action_scale(*raw)
            } else {
                fallback
            });
        }
    }
}

/// 合并出某模型覆盖的**最终形态**；三键全 `None` → `None`（条目该被删掉）。
fn merge_model_override(
    current: Option<&ActionModelOverride>,
    patch: &ActionModelOverridePatch,
) -> Option<ActionModelOverride> {
    let defaults = ActionSettings::default();
    let mut merged = current.cloned().unwrap_or_default();
    apply_model_key(
        &mut merged.head_scale,
        defaults.head_scale,
        &patch.head_scale,
    );
    apply_model_key(
        &mut merged.body_scale,
        defaults.body_scale,
        &patch.body_scale,
    );
    apply_model_key(
        &mut merged.expression_scale,
        defaults.expression_scale,
        &patch.expression_scale,
    );
    if merged.head_scale.is_none()
        && merged.body_scale.is_none()
        && merged.expression_scale.is_none()
    {
        None
    } else {
        Some(merged)
    }
}

// ===== apply_patch =====

/// 把 `patch` 合并进 `current` 的（拷贝）版本上。
///
/// 语义：
/// - `patch.llm == None` → 整段不动；
/// - `patch.llm == Some(None)` → 整段显式清空（base_url/model 清成 `""`，
///   `api_key_env` 清成 `None`）；
/// - `patch.llm == Some(Some(field))` → 仅修改该字段；字段内部三态同段级。
///
/// 非法值（如 `base_url = Some("")` 但不是显式清除场景）返回 `Err`。
/// 返回的 `PatchOutcome` 报告「实际是否有字段真的改了」。
pub fn apply_patch(
    current: &AppSettings,
    patch: &SettingsPatch,
) -> Result<(AppSettings, PatchOutcome), String> {
    let mut next = current.clone();
    let mut changed = false;

    // 段级三态拆解：外层 Option 决定「整段是否参与」，
    // 内层 Option 决定「清空 / 改字段」。
    if let Some(llm_outer) = &patch.llm {
        match llm_outer {
            None => {
                // 整段显式清空：base_url/model 清成 ""，api_key_env 清成 None。
                if !next.llm.base_url.is_empty() {
                    next.llm.base_url = String::new();
                    changed = true;
                }
                if !next.llm.model.is_empty() {
                    next.llm.model = String::new();
                    changed = true;
                }
                if next.llm.api_key_env.is_some() {
                    next.llm.api_key_env = None;
                    changed = true;
                }
                if next.llm.max_tokens.is_some() {
                    next.llm.max_tokens = None; // 回落默认
                    changed = true;
                }
                if next.llm.show_reasoning.is_some() {
                    next.llm.show_reasoning = None; // 回落默认（不展示）
                    changed = true;
                }
            }
            Some(fields) => {
                // base_url：None=Keep / Some(None)=清成 "" / Some(Some(v))=赋值。
                if let Some(v) = &fields.base_url {
                    let new = v.clone().unwrap_or_default();
                    if next.llm.base_url != new {
                        next.llm.base_url = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.model {
                    let new = v.clone().unwrap_or_default();
                    if next.llm.model != new {
                        next.llm.model = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.api_key_env {
                    let new = v.clone();
                    if let Some(ref name) = new {
                        validate_env_name_strict(name)?;
                    }
                    if next.llm.api_key_env != new {
                        next.llm.api_key_env = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.max_tokens {
                    let new = *v;
                    if next.llm.max_tokens != new {
                        next.llm.max_tokens = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.show_reasoning {
                    let new = *v;
                    if next.llm.show_reasoning != new {
                        next.llm.show_reasoning = new;
                        changed = true;
                    }
                }
            }
        }
    }

    if let Some(tts_outer) = &patch.tts {
        match tts_outer {
            None => {
                if !next.tts.base_url.is_empty() {
                    next.tts.base_url = String::new();
                    changed = true;
                }
                if next.tts.model.is_some() {
                    next.tts.model = None;
                    changed = true;
                }
                if next.tts.voice != "alloy" {
                    next.tts.voice = "alloy".to_string();
                    changed = true;
                }
                if next.tts.api_key_env.is_some() {
                    next.tts.api_key_env = None;
                    changed = true;
                }
                let spec = crate::audio::AudioSpec::default();
                if next.tts.sample_rate != spec.sample_rate() {
                    next.tts.sample_rate = spec.sample_rate();
                    changed = true;
                }
                if next.tts.channels != spec.channels() {
                    next.tts.channels = spec.channels();
                    changed = true;
                }
            }
            Some(fields) => {
                if let Some(v) = &fields.base_url {
                    let new = v.clone().unwrap_or_default();
                    if next.tts.base_url != new {
                        next.tts.base_url = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.model {
                    let new = v.clone();
                    if next.tts.model != new {
                        next.tts.model = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.voice {
                    let new = v.clone().unwrap_or_default();
                    if next.tts.voice != new {
                        next.tts.voice = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.api_key_env {
                    let new = v.clone();
                    if let Some(ref name) = new {
                        validate_env_name_strict(name)?;
                    }
                    if next.tts.api_key_env != new {
                        next.tts.api_key_env = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.sample_rate {
                    let new = v.unwrap_or(0);
                    if new == 0 {
                        return Err("tts.sample_rate 不能为 0".to_string());
                    }
                    if next.tts.sample_rate != new {
                        next.tts.sample_rate = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.channels {
                    let new = v.unwrap_or(0);
                    if new == 0 {
                        return Err("tts.channels 不能为 0".to_string());
                    }
                    if next.tts.channels != new {
                        next.tts.channels = new;
                        changed = true;
                    }
                }
            }
        }
    }

    if let Some(persona_outer) = &patch.persona {
        match persona_outer {
            None => {
                if !next.persona.system_prompt.is_empty() {
                    next.persona.system_prompt = String::new();
                    changed = true;
                }
                if next.persona.max_history_pairs != 0 {
                    next.persona.max_history_pairs = 0;
                    changed = true;
                }
            }
            Some(fields) => {
                if let Some(v) = &fields.system_prompt {
                    let new = v.clone().unwrap_or_default();
                    if next.persona.system_prompt != new {
                        next.persona.system_prompt = new;
                        changed = true;
                    }
                }
                if let Some(v) = &fields.max_history_pairs {
                    let new = v.unwrap_or(0);
                    if next.persona.max_history_pairs != new {
                        next.persona.max_history_pairs = new;
                        changed = true;
                    }
                }
            }
        }
    }

    // action 段（2026-09-16）：三项倍率，缺省=不改 / null=回落出厂默认 /
    // 数值=设为该值（钳进 [0.2, 2.2]）。NaN / 无穷按「非法」处理→回落默认，
    // 与渲染面 clamp_action_scale 同一条口径。
    if let Some(action_outer) = &patch.action {
        let defaults = ActionSettings::default();
        match action_outer {
            None => {
                if next.action != defaults {
                    next.action = defaults;
                    changed = true;
                }
            }
            Some(fields) => {
                let set_scale = |slot: &mut f32, fallback: f32, v: &Option<f32>| -> bool {
                    let raw = v.unwrap_or(fallback);
                    let new = if raw.is_finite() {
                        clamp_action_scale(raw)
                    } else {
                        fallback
                    };
                    if (*slot - new).abs() > f32::EPSILON {
                        *slot = new;
                        true
                    } else {
                        false
                    }
                };
                if let Some(v) = &fields.head_scale
                    && set_scale(&mut next.action.head_scale, defaults.head_scale, v)
                {
                    changed = true;
                }
                if let Some(v) = &fields.body_scale
                    && set_scale(&mut next.action.body_scale, defaults.body_scale, v)
                {
                    changed = true;
                }
                if let Some(v) = &fields.expression_scale
                    && set_scale(
                        &mut next.action.expression_scale,
                        defaults.expression_scale,
                        v,
                    )
                {
                    changed = true;
                }
                // 每模型覆盖（阶段5 D40）：map 缺省 = 不动；null 值 = 删除该模型；
                // 对象 = 逐键三态合并。非法 id **直接 Err**（上层映射 400
                // invalid_payload），绝不静默落表——否则错 id 会永久留在 toml 里。
                if let Some(models) = &fields.models {
                    for (id, entry) in models {
                        if !is_valid_model_id(id) {
                            return Err(format!(
                                "action.models 非法模型 id: {id:?}（非空、长度≤64、仅 ASCII 字母/数字与 . _ -，且不含 ..）"
                            ));
                        }
                        match entry {
                            None => {
                                if next.action.models.remove(id).is_some() {
                                    changed = true;
                                }
                            }
                            Some(ov_patch) => {
                                match merge_model_override(next.action.models.get(id), ov_patch) {
                                    Some(merged) => {
                                        if next.action.models.get(id) != Some(&merged) {
                                            next.action.models.insert(id.clone(), merged);
                                            changed = true;
                                        }
                                    }
                                    None => {
                                        if next.action.models.remove(id).is_some() {
                                            changed = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 顶层 dev_mode 三态：与段级同构，None=不修改 / Some(None)=显式关 /
    // Some(Some(b))=覆盖。bool 没有「非法值」概念。
    if let Some(v) = &patch.dev_mode {
        let new = v.unwrap_or(false);
        if next.dev_mode != new {
            next.dev_mode = new;
            changed = true;
        }
    }

    // 合并后再做一次 base_url 合法性总校验（非空就是非法的硬约束；空串
    // 视为「未配置」，会由 resolve 阶段在启动时报，这里允许通过以便
    // 用户在 UI 上分两步：先清空再填新值）。
    if !next.llm.base_url.is_empty() {
        validate_base_url_strict("llm", &next.llm.base_url)?;
    }
    if !next.tts.base_url.is_empty() {
        validate_base_url_strict("tts", &next.tts.base_url)?;
    }

    let outcome = if changed {
        PatchOutcome::Updated
    } else {
        PatchOutcome::NoChange
    };
    Ok((next, outcome))
}

fn validate_base_url_strict(section: &'static str, raw: &str) -> Result<(), String> {
    match url::Url::parse(raw) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => Ok(()),
        _ => Err(format!(
            "{section} base_url 非法: {raw:?}（需要绝对 http/https URL）"
        )),
    }
}

// ===== 原子写回规划 =====
/// 完整原子写回规划（修复点 W6-B：满足 D1 §3.2 原子写回语义）。
///
/// 流程：
/// 1. 拒绝空 `content`（空配置盘 = 静默丢失，必须由调用方显式删文件）→ `Err`；
/// 2. 计算 tmp 路径 `{原完整文件名}.tmp.{pid}`（**保留原扩展名**，不替换）；
/// 3. `create` + `write_all` 把 `content` 写入 tmp（写失败自动 `remove_file` 清理半写 tmp）；
/// 4. `File::sync_data`（POSIX `fdatasync`，仅刷数据不刷元数据，崩溃时不丢落盘内容）；
/// 5. 返回 tmp 路径；调用方在自己合适的时候 `fs::rename(tmp, path)` 完成原子替换。
///
/// tmp 用**当前进程 PID** 区分，并发写不会撞；**失败保证不留半写文件**——
/// `Err` 路径下已写好的 tmp 会被 `remove_file` 清理（`NotFound` 静默忽略）。
///
/// 与旧实现的差异：
/// - tmp 路径形状从 `with_extension("tmp.pid")` 修正为 `<原文件名>.tmp.<pid>`（不再吞掉 `.toml`）；
/// - 写失败自动清理 tmp（不留垃圾）；
/// - 增加 `fdatasync`（崩溃安全）；
/// - 拒绝空内容（空 `live2d-ai.toml` 写盘 = 静默丢失配置）。
pub fn plan_atomic_write(path: &Path, content: &str) -> Result<PathBuf, String> {
    if content.is_empty() {
        return Err(format!(
            "拒绝空内容写盘（{}）：写空 live2d-ai.toml 会静默丢失配置",
            path.display()
        ));
    }

    // 保留完整文件名，仅在末尾追加 `.tmp.<pid>`（不替换扩展名）。
    let pid = process::id();
    let tmp = append_tmp_suffix(path, pid);

    // 写 tmp：失败时尝试清理半写文件，旧文件保留。
    let write_result = (|| -> std::io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        // fdatasync：仅刷数据（不刷 mtime 等元数据），比 fsync 快且崩溃语义足够。
        // 关键保证——崩溃时 tmp 已落盘到磁盘，rename 后 `live2d-ai.toml`
        // 不会是「半写」状态。
        f.sync_data()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        // 清理可能存在的半写 tmp 文件（NotFound 静默忽略）。
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "原子写回失败（{}，tmp={}）: {e}",
            path.display(),
            tmp.display()
        ));
    }
    Ok(tmp)
}

/// 把 `<path>.tmp.<pid>` 拼到 `path` 的完整文件名后；不替换扩展名。
///
/// 例子：`/x/live2d-ai.toml` + `pid=1234` → `/x/live2d-ai.toml.tmp.1234`。
fn append_tmp_suffix(path: &Path, pid: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".tmp.{pid}"));
    PathBuf::from(s)
}
