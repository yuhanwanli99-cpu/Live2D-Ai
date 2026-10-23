//! 表演层**装配面**（T8，2026-10-07；2026-10-17 rc4 去导演 Mod）。
//!
//! # 两个独立装配点
//!
//! 1. **核心第二路按句清洗**（产品路径，**常驻**）：[`SentenceCleaner`] 恒从
//!    `[llm]` 同一份端点/模型/密钥构造（[`build_sentence_cleaner`]）；动作
//!    能力集与按句规则表来自 [`live2d_ai_runtime::performance::rules`]——**单一
//!    cue 写者**（导演 Mod 已于 rc4 退役，不再并行投递）。表演倾向 / 动作频率
//!    只读 `mods.json` 的两个既有键 `mods.director.config.{motion_bias,motion_rate}`
//!    （缺省平常 / 不增不减）；**不读** `enabled` / staging 端点配置，不新增键。
//! 2. **整段表演层**（显式 opt-in）：`[performance].enabled`（缺省 false）→
//!    [`PerformanceRuntime`]，每轮一份 JSON 负责切段 / speak。**动作仍恒开**
//!    （2026-10-17 返工裁决）：阶段 1.5 对**每一段原文**用同一张核心本地规则表
//!    产 cue（非空才发 ActionCue，纯函数、不新增 LLM 调用）；表演层自己的
//!    `resolution.cues` **不投递**，避免两份 cue 互相整份覆盖。
//!
//! # 何时读 / 何时写
//!
//! 只在启动装配与热重载（`ControlCommand::Reload`）时读一次：改
//! `motion_bias` / `motion_rate` / `[performance]` 都要重载才生效。
//! **读路径只读不写**（一行不改）；写路径只有
//! [`merge_director_motion_config`]（0.4.4 rc.1 的 POST director config
//! 旁路：只合并两个白名单键，原子写、不写 `enabled`、不碰其它键）。
//!
//! # 行数
//!
//! 从 `supervisor.rs` 拆出（那份已 971 行、贴近 `code_stats` 的 >1000 棘轮）：
//! 本文件仍是 ≤500 行的小文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use live2d_ai_mod_director::motion_bias::MotionBias;
use live2d_ai_runtime::motion_rate::MotionRate;
use live2d_ai_runtime::performance::{PerformanceRuntime, RuleFallback, assemble, rules};
use live2d_ai_runtime::settings::PerformanceSettings;

/// `mods.json` 路径 = `live2d-ai.toml` 同目录（与 `cli_entry::mods_path_for_web` 同口径）。
pub fn mods_manifest_path(config_path: &str) -> PathBuf {
    Path::new(config_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mods.json")
}

/// 读 `mods.json` 里 `mods.director.config.<key>` 的字符串值（只读）。
///
/// 返回 `(manifest 路径, 值)`：路径只用于日志；缺文件 / 坏 JSON / 缺段 /
/// 非字符串一律 `None`（调用方按各字段缺省值处理，**读取绝不失败**）。
fn director_config_string(config_path: &str, key: &str) -> (PathBuf, Option<String>) {
    let path = mods_manifest_path(config_path);
    let raw = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|manifest| {
            manifest
                .get("mods")
                .and_then(|mods| mods.get("director"))
                .and_then(|entry| entry.get("config"))
                .and_then(|config| config.get(key))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    (path, raw)
}

/// 读 `mods.json` 里**仅**的表演倾向（`mods.director.config.motion_bias`）。
///
/// 只读这一个**既有**键：缺文件 / 坏 JSON / 缺段 / 未知值 → 平常
/// （[`MotionBias::default`]）。**不读** `enabled` / 导演自己的 staging 端点
/// 配置——导演 Mod 已退役，它的启用状态不再是任何开关；端点也已删除。
pub fn load_motion_bias(config_path: &str) -> MotionBias {
    let (path, raw) = director_config_string(config_path, "motion_bias");
    let bias = MotionBias::parse(raw.as_deref());
    tracing::debug!(
        bias = bias.as_str(),
        path = %path.display(),
        "表演倾向已读（只读 mods.director.config.motion_bias）"
    );
    bias
}

/// 读 `mods.json` 里**仅**的导演动作频率（`mods.director.config.motion_rate`）。
///
/// 只读这一个键：缺文件 / 坏 JSON / 缺键 / 未知值 → 平常
/// （[`MotionRate::default`] = 不增不减）。**不读** `enabled` / staging 端点；
/// 与 [`load_motion_bias`] 同一套防御读口径。
pub fn load_motion_rate(config_path: &str) -> MotionRate {
    let (path, raw) = director_config_string(config_path, "motion_rate");
    let rate = MotionRate::parse(raw.as_deref());
    tracing::debug!(
        rate = rate.as_str(),
        path = %path.display(),
        "动作频率已读（只读 mods.director.config.motion_rate）"
    );
    rate
}

/// `motion_bias` 白名单（值 = `live2d_ai_mod_director::motion_bias::MotionBias::as_str()`）。
const MOTION_BIAS_VALUES: [&str; 3] = ["restrained", "normal", "eager"];
/// `motion_rate` 白名单（值 = [`MotionRate::as_str`]）。
const MOTION_RATE_VALUES: [&str; 3] = ["sparse", "normal", "dense"];

/// `merge_director_motion_config` 的失败分类：HTTP 层据此判 400 / 409。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// 提交值非法（非字符串 / 未知白名单值 / `config` 不是对象）→ 可判 400。
    InvalidValue(String),
    /// 磁盘坏 JSON / 结构不可合并 / 写盘失败 → 可判 409。
    Storage(String),
}

/// 只把 `motion_bias` / `motion_rate` 合并进 `mods.json` 的
/// `mods.director.config`（0.4.4 rc.1；**不写 enabled、不碰其它键**）。
///
/// 口径（逐条）：
/// - 只认这两个键；其余键**忽略且不写**（前端整份提交也不会丢东西）；
/// - 值必须字符串且属于白名单（[`MOTION_BIAS_VALUES`] / [`MOTION_RATE_VALUES`]），
///   否则 [`MergeError::InvalidValue`]（HTTP 400）——**先全量校验，再动磁盘**；
/// - 以**磁盘当前内容**为基底：其它顶层键、其它 `mods` 条目、director 的其它
///   `config` 键与 `enabled` 全部原样保留；`mods.director` 不存在时创建
///   `{"config":{...}}`（**不写 `enabled`**）；
/// - 坏 JSON / 结构不可合并 / 写盘失败 → [`MergeError::Storage`]（HTTP 409），
///   绝不覆盖读不懂的磁盘内容；
/// - 原子写（`plan_atomic_write` + rename，pretty JSON），与 `mods.json`
///   既有写回同一条纪律。
pub fn merge_director_motion_config(
    config_path: &str,
    submitted: &serde_json::Value,
) -> Result<(), MergeError> {
    // 1) 先全量校验，避免「改一半才报错」留下半份更新。
    let submitted = submitted
        .as_object()
        .ok_or_else(|| MergeError::InvalidValue("config 必须是 JSON 对象".to_string()))?;
    let mut updates: Vec<(&'static str, String)> = Vec::new();
    for (key, allowed) in [
        ("motion_bias", &MOTION_BIAS_VALUES[..]),
        ("motion_rate", &MOTION_RATE_VALUES[..]),
    ] {
        let Some(value) = submitted.get(key) else {
            continue;
        };
        let Some(text) = value.as_str() else {
            return Err(MergeError::InvalidValue(format!(
                "{key} 必须是字符串（{} 之一）",
                allowed.join(" / ")
            )));
        };
        if !allowed.contains(&text) {
            return Err(MergeError::InvalidValue(format!(
                "{key} 的未知值 {text:?}（允许：{}）",
                allowed.join(" / ")
            )));
        }
        updates.push((key, text.to_string()));
    }
    // 全是无关键 = 不写盘（幂等 no-op）。
    if updates.is_empty() {
        return Ok(());
    }
    // 2) 以磁盘当前内容为基底：读不到 = 新建；坏 JSON = 409，绝不覆盖用户数据。
    let path = mods_manifest_path(config_path);
    let mut root = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text).map_err(|err| {
            MergeError::Storage(format!("{} 不是合法 JSON：{err}", path.display()))
        })?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            serde_json::Value::Object(serde_json::Map::new())
        }
        Err(err) => {
            return Err(MergeError::Storage(format!(
                "读取 {} 失败：{err}",
                path.display()
            )));
        }
    };
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| MergeError::Storage(format!("{} 顶层不是对象", path.display())))?;
    let mods = root_object
        .entry("mods")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let mods = mods
        .as_object_mut()
        .ok_or_else(|| MergeError::Storage(format!("{} 的 mods 不是对象", path.display())))?;
    let director = mods
        .entry("director")
        .or_insert_with(|| serde_json::json!({ "config": {} }));
    let director = director.as_object_mut().ok_or_else(|| {
        MergeError::Storage(format!("{} 的 mods.director 不是对象", path.display()))
    })?;
    let config = director
        .entry("config")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let config = config.as_object_mut().ok_or_else(|| {
        MergeError::Storage(format!(
            "{} 的 mods.director.config 不是对象",
            path.display()
        ))
    })?;
    for (key, value) in updates {
        config.insert(key.to_string(), serde_json::Value::String(value));
    }
    // 3) 原子写：tmp + fdatasync + rename（失败不留半写文件）。
    let text = serde_json::to_string_pretty(&root)
        .map_err(|err| MergeError::Storage(format!("mods.json 序列化失败：{err}")))?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        return Err(MergeError::Storage(format!(
            "创建 {} 的目录失败：{err}",
            path.display()
        )));
    }
    let tmp = live2d_ai_runtime::settings::patch::plan_atomic_write(&path, &text)
        .map_err(MergeError::Storage)?;
    if let Err(err) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(MergeError::Storage(format!(
            "{} rename 失败：{err}",
            path.display()
        )));
    }
    Ok(())
}

/// 按 `[performance]` 装配整段表演层运行时（**唯一装配点**；启动与热重载共用）。
///
/// - `[performance].enabled == false`（**缺省**）→ `None`：主链走**常驻的
///   二路按句清洗**（产品路径）；
/// - 显式打开后只读 `[performance]` 自己的 `base_url/model/api_key_env`；
///   端点缺 / URL 非法时仍装配 degraded 运行时（`enabled()==false`）→ 每轮
///   确定性回退，原文一个字不丢；
/// - **能力集 = runtime 规则表的 `accepted_preset_ids()`**、**规则回退 =
///   `rule_cues_for_text`**（单一真源在 runtime，不在这里再抄一份）。
pub fn build_performance_runtime(
    performance: &PerformanceSettings,
) -> Option<Arc<PerformanceRuntime>> {
    // 2026-10-08：整段表演层是显式 opt-in（见文件头注）。缺省关 = 二路按句清洗。
    if !performance.enabled {
        return None;
    }
    // 只读 [performance]：端点两项必须齐（缺 → None，保持主链行为，不装配）。
    if performance.base_url.trim().is_empty() || performance.model.trim().is_empty() {
        return None;
    }
    let allow: Vec<String> = rules::accepted_preset_ids();
    let rule: RuleFallback = Arc::new(rules::rule_cues_for_text);
    let setup = assemble(performance, allow, Some(rule));
    if let Some(note) = setup.note.as_deref() {
        tracing::warn!(note, "表演层接管了但没接上：每轮回退到确定性规则层");
    }
    if let Some(rt) = setup.runtime.as_ref() {
        tracing::info!(
            client = rt.client_kind(),
            mode = rt.mode().as_str(),
            model = rt.model(),
            allow_len = rt.allow().len(),
            "整段表演层已装配（[performance].enabled=true；本轮 0 条按句 action_cue）"
        );
    }
    setup.runtime
}

#[cfg(test)]
#[path = "performance_wiring_tests.rs"]
mod tests;
