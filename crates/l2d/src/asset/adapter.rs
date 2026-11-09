//! 每模型参数适配层（0.5.0 rc.3）：`assets/models/<id>/adapter.json`。
//!
//! # 为什么
//!
//! rc.1/rc.2 的 [`ParamBinder`] 能按 canonical 自动把 13 条标准语义名对到多数
//! 模型；但总有个别模型（私有命名 / 特殊嘴参数）对不上，且用户可能想让某个
//! 通道**静默**（例如模型自带夸张嘴形，不想被 TTS 口型驱动）。本模块给一份
//! **用户自持、不入库**的每模型适配文件，只做三件事，边界写死：
//!
//! - `param_aliases`：只允许「13 条标准语义名 → 模型实际参数 id」的显式覆盖，
//!   **禁止新增语义通道**（key 不在 13 条里直接丢弃 + warning）；
//! - `disable_channels`：也只认这 13 条语义名；禁用后该通道一律不写
//!   （例外：model3 `Groups` 里**模型自带**的口型 / 眨眼名单仍按模型声明写——
//!   那是模型显式绑定，不是 13 条语义通道）；
//! - `expressions`：只存「应用层 preset id → 模型 Expression 名」的映射，
//!   **只存不播**（exp3 播放不在本版范围）。
//!
//! # 回退与来源
//!
//! - 文件缺失（正常出厂）→ 只用 moc3 binder，`source = "binder"`；
//! - 文件合法并生效 → `source = "adapter"`；
//! - 坏 JSON / 顶层不是对象 / version 不支持 → **整份弃用**回退 binder，
//!   `source = "degraded"`；绝不「半用」一份坏文件；
//! - 单个 section / 单条 entry 非法 → 只丢那一条 + warning，其余照用
//!   （宽容但不撒谎）。
//!
//! # 形状
//!
//! ```json
//! {
//!   "version": 1,
//!   "param_aliases": { "ParamAngleX": "PARAM_ANGLE_X" },
//!   "disable_channels": ["ParamMouthForm"],
//!   "expressions": { "smile": "exp_smile" }
//! }
//! ```
//!
//! alias 指向的 id 必须**真实存在于该模型 moc3 参数表**（装载时校验）；不存在
//! 时该通道视为不可用（在 `loaded.param_bind.missing` 里如实出现），不会退回去
//! 写自动绑定的旧 id。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::param_bind::{ParamBinder, STANDARD_PARAM_IDS, canonical};

/// 本模块唯一支持的 adapter 格式版本。
const ADAPTER_VERSION: u64 = 1;

/// adapter 的加载来源（`loaded.param_bind.source` 的来源）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AdapterSource {
    /// 没有 adapter.json（正常出厂：只用 moc3 参数表建 binder）。
    #[default]
    Missing,
    /// adapter.json 合法并已生效。
    External,
    /// adapter.json 存在但不可用（坏 JSON / 结构非法 / version 不支持）→ 回退 binder。
    Degraded,
}

impl AdapterSource {
    /// 写进 `loaded.param_bind.source` 的稳定字符串（协议只加可选字段）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "binder",
            Self::External => "adapter",
            Self::Degraded => "degraded",
        }
    }
}

/// 解析后的每模型参数适配（缺省 = 空 = 只用 binder）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParamAdapter {
    source: AdapterSource,
    /// 13 条标准语义名（`&'static str` 来自 [`STANDARD_PARAM_IDS`]）→ 实际 id。
    aliases: BTreeMap<&'static str, String>,
    /// 被显式禁用的 13 条语义名。
    disabled: BTreeSet<&'static str>,
    /// 应用层 preset id → 模型 Expression 名（**只存不播**）。
    expressions: BTreeMap<String, String>,
    /// 解析期告警（被丢弃的条目 / 整份回退原因）。
    warnings: Vec<String>,
}

impl ParamAdapter {
    /// 从可选字节解析；**永不失败**：缺失 / 坏 JSON 都回退成空适配并由
    /// [`Self::source`] / [`Self::warnings`] 如实报告。
    pub fn from_json(bytes: Option<&[u8]>) -> Self {
        let Some(bytes) = bytes else {
            return Self::default();
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(e) => return Self::degraded(format!("adapter.json 不是 UTF-8：{e}")),
        };
        let value: Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(e) => return Self::degraded(format!("adapter.json 不是合法 JSON：{e}")),
        };
        let Some(obj) = value.as_object() else {
            return Self::degraded("adapter.json 顶层必须是对象".to_owned());
        };
        if let Some(version) = obj.get("version")
            && version.as_u64() != Some(ADAPTER_VERSION)
        {
            return Self::degraded(format!(
                "adapter.json version 只支持 {ADAPTER_VERSION}（实得 {version}）"
            ));
        }

        let mut out = Self {
            source: AdapterSource::External,
            ..Self::default()
        };
        match obj.get("param_aliases") {
            Some(Value::Object(map)) => {
                for (raw_semantic, raw_actual) in map {
                    let Some(semantic) = standard_id(raw_semantic) else {
                        out.warn(format!(
                            "param_aliases 的键 {raw_semantic:?} 不是 13 条标准语义名，已丢弃"
                        ));
                        continue;
                    };
                    let Some(actual) = raw_actual.as_str().map(str::trim).filter(|s| !s.is_empty())
                    else {
                        out.warn(format!(
                            "param_aliases[{raw_semantic}] 的值不是非空字符串，已丢弃"
                        ));
                        continue;
                    };
                    out.aliases.insert(semantic, actual.to_owned());
                }
            }
            Some(other) => out.warn(format!(
                "param_aliases 必须是对象（实得 {other}），整节省略"
            )),
            None => {}
        }
        match obj.get("disable_channels") {
            Some(Value::Array(list)) => {
                for raw in list {
                    let Some(raw) = raw.as_str() else {
                        out.warn("disable_channels 里的非字符串项已丢弃".to_owned());
                        continue;
                    };
                    let Some(semantic) = standard_id(raw) else {
                        out.warn(format!(
                            "disable_channels 的 {raw:?} 不是 13 条标准语义名，已丢弃"
                        ));
                        continue;
                    };
                    out.disabled.insert(semantic);
                }
            }
            Some(other) => out.warn(format!(
                "disable_channels 必须是数组（实得 {other}），整节省略"
            )),
            None => {}
        }
        match obj.get("expressions") {
            Some(Value::Object(map)) => {
                for (preset_id, raw_name) in map {
                    let Some(name) = raw_name.as_str().map(str::trim).filter(|s| !s.is_empty())
                    else {
                        out.warn(format!(
                            "expressions[{preset_id}] 的值不是非空字符串，已丢弃"
                        ));
                        continue;
                    };
                    out.expressions.insert(preset_id.clone(), name.to_owned());
                }
            }
            Some(other) => out.warn(format!("expressions 必须是对象（实得 {other}），整节省略")),
            None => {}
        }
        // 未知顶层键只提醒、不影响使用（向前兼容）。
        for key in obj.keys() {
            if !matches!(
                key.as_str(),
                "version" | "param_aliases" | "disable_channels" | "expressions"
            ) {
                out.warn(format!("未知顶层键 {key:?} 已忽略"));
            }
        }
        out
    }

    /// 加载来源。
    pub fn source(&self) -> AdapterSource {
        self.source
    }

    /// 解析期告警（被丢弃的条目 / 整份回退原因；装载期可写日志）。
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `param_aliases` 里生效的「语义名 → 实际 id」（只读诊断）。
    pub fn aliases(&self) -> &BTreeMap<&'static str, String> {
        &self.aliases
    }

    /// 被显式禁用的 13 条语义名。
    pub fn disabled(&self) -> &BTreeSet<&'static str> {
        &self.disabled
    }

    /// `expressions` 的「preset id → 模型 Expression 名」映射（**只存不播**）。
    pub fn expressions(&self) -> &BTreeMap<String, String> {
        &self.expressions
    }

    /// 用**本模型**的 moc3 参数 id 表 + 本 adapter 合成绑定表。
    ///
    /// 合成顺序：先按参数表自动绑定（[`ParamBinder::from_ids`]），再应用
    /// `param_aliases`（只接受指向模型真实存在的 id；指向不存在的 id = 该通道
    /// 不可用），最后应用 `disable_channels`（从绑定表移除并记禁用）。
    pub fn bind<I, S>(&self, ids: I) -> ParamBinder
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let ids: Vec<String> = ids.into_iter().map(|id| id.as_ref().to_owned()).collect();
        let mut binder = ParamBinder::from_ids(&ids);
        if self.aliases.is_empty() && self.disabled.is_empty() {
            return binder;
        }
        let present: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
        for (semantic, actual) in &self.aliases {
            let target = present.contains(actual.as_str()).then(|| actual.clone());
            binder.rebind(canonical(semantic), target);
        }
        for semantic in &self.disabled {
            binder.disable(canonical(semantic));
        }
        binder
    }

    fn degraded(reason: String) -> Self {
        Self {
            source: AdapterSource::Degraded,
            warnings: vec![reason],
            ..Self::default()
        }
    }

    fn warn(&mut self, msg: String) {
        self.warnings.push(msg);
    }
}

/// 由 model3 所在目录推 `adapter.json` 的磁盘路径（0.5.0 rc.3 资产约定）。
///
/// 标准布局是 `assets/models/<id>/runtime/<name>.model3.json`，adapter 在
/// `assets/models/<id>/adapter.json`：model3 目录名是 `runtime` 时取上一级，
/// 否则取 model3 同目录（测试 / 自定义布局）。
pub fn adapter_path(model3_dir: &Path) -> PathBuf {
    let base = match model3_dir.file_name().and_then(|name| name.to_str()) {
        Some("runtime") => model3_dir.parent().unwrap_or(model3_dir),
        _ => model3_dir,
    };
    base.join("adapter.json")
}

/// 13 条标准语义名之一 → 规范化成 `&'static str`；否则 `None`。
fn standard_id(raw: &str) -> Option<&'static str> {
    STANDARD_PARAM_IDS.iter().copied().find(|id| *id == raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::asset::bind_report::ParamBindReport;

    /// 13 条标准名（`param_aliases` / `disable_channels` 的白名单来源）打底。
    fn bai_like_ids() -> Vec<String> {
        let mut ids: Vec<String> = STANDARD_PARAM_IDS
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
        ids.push("ParamMouthOpenY".to_owned());
        ids.push("ParamBreath".to_owned());
        ids
    }

    fn json_of(raw: &str) -> ParamAdapter {
        ParamAdapter::from_json(Some(raw.as_bytes()))
    }

    #[test]
    fn missing_file_is_plain_binder_source() {
        let adapter = ParamAdapter::from_json(None);
        assert_eq!(adapter.source(), AdapterSource::Missing);
        assert_eq!(adapter.source().as_str(), "binder");
        assert!(adapter.warnings().is_empty());
        let binder = adapter.bind(bai_like_ids());
        assert_eq!(binder.resolve("ParamAngleX"), Some("ParamAngleX"));
        assert_eq!(
            ParamBindReport::new(&binder, &adapter).to_json()["source"],
            "binder"
        );
    }

    #[test]
    fn bad_json_degrades_to_binder_instead_of_half_applying() {
        for (raw, why) in [
            ("{oops", "非法 JSON"),
            ("[1,2,3]", "顶层不是对象"),
            (
                r#"{"version":2,"param_aliases":{"ParamAngleX":"X"}}"#,
                "version 不支持",
            ),
            (r#"{"version":"1"}"#, "version 类型非法"),
            ("", "空文件"),
        ] {
            let adapter = ParamAdapter::from_json(Some(raw.as_bytes()));
            assert_eq!(adapter.source(), AdapterSource::Degraded, "{why}");
            assert_eq!(adapter.source().as_str(), "degraded");
            assert!(!adapter.warnings().is_empty(), "{why} 必须留下可读原因");
            // 整份弃用：alias 一条都不能生效。
            assert!(adapter.aliases().is_empty(), "{why} 不得半用 alias");
            let binder = adapter.bind(bai_like_ids());
            assert_eq!(
                binder.resolve("ParamAngleX"),
                Some("ParamAngleX"),
                "{why} 必须回退自动绑定"
            );
            assert_eq!(
                ParamBindReport::new(&binder, &adapter).to_json()["source"],
                "degraded"
            );
        }
    }

    #[test]
    fn partial_override_changes_only_named_channel() {
        let adapter =
            json_of(r#"{"version":1,"param_aliases":{"ParamAngleX":"ParamAngleX_private"}}"#);
        assert_eq!(adapter.source(), AdapterSource::External);
        assert_eq!(adapter.source().as_str(), "adapter");
        let mut ids = bai_like_ids();
        ids.push("ParamAngleX_private".to_owned());
        let binder = adapter.bind(ids);
        assert_eq!(binder.resolve("ParamAngleX"), Some("ParamAngleX_private"));
        // 其它通道保持自动绑定。
        assert_eq!(binder.resolve("ParamAngleY"), Some("ParamAngleY"));
        assert_eq!(binder.resolve("ParamMouthOpenY"), Some("ParamMouthOpenY"));
        let report = ParamBindReport::new(&binder, &adapter);
        assert_eq!(report.source(), AdapterSource::External);
        assert!(report.missing().is_empty());
        assert!(report.conflicts().is_empty());
    }

    #[test]
    fn full_override_replaces_all_thirteen_channels() {
        let aliases = STANDARD_PARAM_IDS
            .iter()
            .enumerate()
            .map(|(i, id)| format!(r#""{id}":"custom_{i}""#))
            .collect::<Vec<_>>()
            .join(",");
        let adapter = json_of(&format!(r#"{{"version":1,"param_aliases":{{{aliases}}}}}"#));
        let mut ids: Vec<String> = (0..STANDARD_PARAM_IDS.len())
            .map(|i| format!("custom_{i}"))
            .collect();
        ids.push("unrelated".to_owned());
        let binder = adapter.bind(&ids);
        for (i, id) in STANDARD_PARAM_IDS.iter().enumerate() {
            assert_eq!(
                binder.resolve(id),
                Some(format!("custom_{i}").as_str()),
                "{id} 必须按 adapter 覆盖"
            );
        }
        let report = ParamBindReport::new(&binder, &adapter);
        assert!(report.missing().is_empty(), "全部覆盖后没有缺失通道");
    }

    #[test]
    fn disabled_channels_stop_writes_and_show_up_as_missing() {
        let adapter =
            json_of(r#"{"version":1,"disable_channels":["ParamMouthForm","ParamAngleZ"]}"#);
        let binder = adapter.bind(bai_like_ids());
        assert_eq!(binder.resolve("ParamMouthForm"), None);
        assert_eq!(binder.resolve("ParamAngleZ"), None);
        assert!(binder.is_disabled("ParamMouthForm"));
        assert!(!binder.is_disabled("ParamAngleX"));
        let report = ParamBindReport::new(&binder, &adapter);
        assert!(report.missing().contains(&"ParamMouthForm"));
        assert!(report.missing().contains(&"ParamAngleZ"));
        assert!(!report.missing().contains(&"ParamAngleX"));
    }

    #[test]
    fn invalid_entries_are_dropped_individually_with_warnings() {
        let adapter = json_of(
            r#"{"version":1,
                "param_aliases":{"ParamNope":"X","ParamAngleX":"","ParamEyeLSmile":42},
                "disable_channels":["ParamNope",7],
                "expressions":{"smile":"exp_smile","bad":null},
                "future_key":true}"#,
        );
        assert_eq!(
            adapter.source(),
            AdapterSource::External,
            "条目级错误不整份回退"
        );
        assert!(
            adapter.aliases().is_empty(),
            "非法语义名 / 空实际 id 都要丢弃"
        );
        assert!(adapter.disabled().is_empty(), "非标准语义名不得进禁用集");
        assert_eq!(adapter.expressions().len(), 1);
        assert_eq!(
            adapter.expressions().get("smile").map(String::as_str),
            Some("exp_smile")
        );
        assert!(
            adapter.warnings().len() >= 5,
            "每条丢弃都要有 warning：{:?}",
            adapter.warnings()
        );
    }

    #[test]
    fn alias_to_id_absent_on_model_leaves_channel_missing() {
        let adapter = json_of(r#"{"version":1,"param_aliases":{"ParamAngleX":"Nope"}}"#);
        let binder = adapter.bind(bai_like_ids());
        assert_eq!(
            binder.resolve("ParamAngleX"),
            None,
            "alias 指向不存在的 id ⇒ 通道不可用"
        );
        let report = ParamBindReport::new(&binder, &adapter);
        assert!(report.missing().contains(&"ParamAngleX"));
        // 不得退回去写自动绑定的 ParamAngleX（否则等于没生效）。
        assert_ne!(binder.resolve("ParamAngleX"), Some("ParamAngleX"));
    }

    #[test]
    fn expressions_are_stored_but_do_not_affect_binding() {
        let adapter = json_of(r#"{"version":1,"expressions":{"smile":"exp_smile"}}"#);
        assert_eq!(
            adapter.expressions().get("smile").map(String::as_str),
            Some("exp_smile")
        );
        let binder = adapter.bind(bai_like_ids());
        assert_eq!(binder.resolve("ParamMouthForm"), Some("ParamMouthForm"));
    }

    #[test]
    fn adapter_path_follows_asset_layout() {
        assert_eq!(
            adapter_path(Path::new("/repo/assets/models/bai/runtime")),
            PathBuf::from("/repo/assets/models/bai/adapter.json")
        );
        // 非 runtime 布局：取 model3 同目录（测试 / 自定义）。
        assert_eq!(
            adapter_path(Path::new("/tmp/model")),
            PathBuf::from("/tmp/model/adapter.json")
        );
    }
}
