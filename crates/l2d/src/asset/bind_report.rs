//! `loaded.param_bind` 可观测块（0.5.0 rc.3）：来源 / 缺失通道 / 冲突记录。
//!
//! 为什么单独成文件：它同时消费 [`ParamAdapter`]（来源）与 [`ParamBinder`]
//! （缺失 / 冲突），放 adapter 或 param_bind 任一文件都会突破「源码 ≤500 行」
//! 棘轮；协议形状（`{source,missing,conflicts}`）在这里单独可测。

use serde_json::{Value, json};

use super::adapter::{AdapterSource, ParamAdapter};
use super::param_bind::{BindConflict, ParamBinder, STANDARD_PARAM_IDS};

/// `loaded.param_bind` 的可观测块：来源 / 缺失通道 / 冲突记录（0.5.0 rc.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamBindReport {
    source: AdapterSource,
    missing: Vec<&'static str>,
    conflicts: Vec<BindConflict>,
}

impl ParamBindReport {
    /// 从「已按 adapter 合成好的 binder + adapter」出报告。
    pub fn new(binder: &ParamBinder, adapter: &ParamAdapter) -> Self {
        Self {
            source: adapter.source(),
            missing: STANDARD_PARAM_IDS
                .iter()
                .copied()
                .filter(|id| binder.resolve(id).is_none())
                .collect(),
            conflicts: binder.conflicts().to_vec(),
        }
    }

    /// 来源（`"binder"` / `"adapter"` / `"degraded"`）。
    pub fn source(&self) -> AdapterSource {
        self.source
    }

    /// 13 条标准语义名里最终**不可写**的（模型没有 / 被 adapter 禁用 / alias 失效）。
    pub fn missing(&self) -> &[&'static str] {
        &self.missing
    }

    /// 自动绑定期的冲突记录（同一 canonical 多个实际 id 的落选者）。
    pub fn conflicts(&self) -> &[BindConflict] {
        &self.conflicts
    }

    /// 协议里 `loaded.param_bind` 的 JSON 形状：`{source,missing,conflicts}`。
    pub fn to_json(&self) -> Value {
        json!({
            "source": self.source.as_str(),
            "missing": self.missing,
            "conflicts": self
                .conflicts
                .iter()
                .map(|c| json!({
                    "canonical": c.canonical,
                    "kept": c.kept,
                    "dropped": c.dropped,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::ParamAdapter;

    fn all_standard_ids() -> Vec<String> {
        STANDARD_PARAM_IDS
            .iter()
            .map(|id| (*id).to_owned())
            .collect()
    }

    #[test]
    fn report_json_carries_conflicts_and_missing() {
        // 模型同时有标准名与大写 snake：自动绑定记一条冲突。
        let mut ids = all_standard_ids();
        ids.push("PARAM_ANGLE_X".to_owned());
        let adapter = ParamAdapter::from_json(Some(br#"{"version":1}"#.as_slice()));
        let binder = adapter.bind(ids);
        let report = ParamBindReport::new(&binder, &adapter);
        assert_eq!(report.source(), AdapterSource::External);
        assert!(report.missing().is_empty());
        assert_eq!(report.conflicts().len(), 1);
        assert_eq!(report.conflicts()[0].canonical, "anglex");
        assert_eq!(report.conflicts()[0].kept, "ParamAngleX");
        let json = report.to_json();
        assert_eq!(json["source"], "adapter");
        assert!(
            json["missing"].as_array().is_some_and(Vec::is_empty),
            "全部可绑时 missing 必须是空数组：{json}"
        );
        assert_eq!(json["conflicts"][0]["canonical"], "anglex");
        assert_eq!(json["conflicts"][0]["kept"], "ParamAngleX");
        assert_eq!(json["conflicts"][0]["dropped"][0], "PARAM_ANGLE_X");
    }

    #[test]
    fn report_source_strings_are_stable() {
        let adapter = ParamAdapter::from_json(None);
        let binder = adapter.bind(all_standard_ids());
        assert_eq!(
            ParamBindReport::new(&binder, &adapter).to_json()["source"],
            "binder"
        );
        let adapter = ParamAdapter::from_json(Some(b"not-json".as_slice()));
        let binder = adapter.bind(all_standard_ids());
        assert_eq!(
            ParamBindReport::new(&binder, &adapter).to_json()["source"],
            "degraded"
        );
    }
}
