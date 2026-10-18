//! 字段映射表的**重建**与来源报告（0.3.6 T2）。
//!
//! # 为什么单独一个模块
//!
//! 换模成功后必须按**新 model id** 重建字段表：旧皮套的外置覆盖表绝不能继续
//! 作用在新皮套上。判定逻辑住在原生可测的这里；wasm 侧（`web/swap.rs` /
//! `main.rs`）只做 fetch，把取回结果交给 [`rebuild_for_model`]。
//!
//! # 不变量
//!
//! - **任何**失败路径都返回新 model id 的内建表（[`FieldMap::for_model`]），
//!   **绝不沿用旧表**；
//! - `loaded` ack 的 `field_map` 载荷（source / degraded / reason）只由这里产出；
//! - **404（文件本来就没有）不是降级**：桌面端 `/actions/*` 未提供覆盖表是正常配置
//!   （O9「本轮不要求每皮套一份」），报 builtin + degraded=false，避免长期噪声；
//! - 外置 JSON 合法但没有本模型 / default 节同样是 builtin + degraded=false。

use serde_json::{Value, json};

use super::FieldMap;

/// 外置覆盖表的取回结果（`field_map` 载荷 source 判据的输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideFetch {
    /// 外置表存在并可读。
    Fetched(Vec<u8>),
    /// 外置表**本来就没有**（HTTP 404 / 未提供）：内建表即完整映射，不算降级。
    Missing(String),
    /// 读取失败（网络 / 非 404 状态码 / 其它）：降级并给可读原因。
    Failed(String),
}

impl OverrideFetch {
    /// wasm 侧 fetch 结果 → 取回结果（**唯一**的「文件缺失」判定处，原生可测）。
    ///
    /// `net::fetch_bytes` 把非 2xx 收敛成 `"HTTP {status}"`；404 = 文件本来就没有。
    pub fn from_fetch(result: Result<Vec<u8>, String>) -> Self {
        match result {
            Ok(bytes) => OverrideFetch::Fetched(bytes),
            Err(e) if e.trim() == "HTTP 404" => OverrideFetch::Missing(e),
            Err(e) => OverrideFetch::Failed(e),
        }
    }
}

/// 外置覆盖表的来源（wire：`field_map.source`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMapSource {
    /// 外置 JSON 读到且至少一项生效（有告警时 `degraded=true`）。
    External,
    /// 内建表就是完整映射：外置表没有 / 没有本模型或 default 节。
    Builtin,
    /// 外置表存在但不可用（读取失败 / 非 UTF-8 / JSON 非法）。
    Degraded,
}

impl FieldMapSource {
    /// wire 名（Flutter 逐字消费，**不得改名**）。
    pub const fn as_str(self) -> &'static str {
        match self {
            FieldMapSource::External => "external",
            FieldMapSource::Builtin => "builtin",
            FieldMapSource::Degraded => "degraded",
        }
    }
}

/// `loaded` ack 的 `field_map` 载荷（0.3.6 T2，与 Flutter 逐字一致）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMapReport {
    pub source: FieldMapSource,
    pub degraded: bool,
    pub reason: String,
}

impl FieldMapReport {
    fn new(source: FieldMapSource, degraded: bool, reason: String) -> Self {
        Self {
            source,
            degraded,
            reason,
        }
    }

    /// 投影成 wire payload：`{source, degraded, reason}`。
    pub fn to_json(&self) -> Value {
        json!({
            "source": self.source.as_str(),
            "degraded": self.degraded,
            "reason": self.reason,
        })
    }
}

/// 一次字段表重建的结果：装进运行时的表 + 上报给宿主的来源。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMapLoad {
    pub map: FieldMap,
    pub report: FieldMapReport,
}

/// 按 model id 重建字段表（**唯一**的「取不到外置表」判定处）。
///
/// 调用点：启动首装、换模成功（generation 已确认未变）各一次。
pub fn rebuild_for_model(model_id: &str, fetch: OverrideFetch) -> FieldMapLoad {
    let builtin = |reason: String| FieldMapLoad {
        map: FieldMap::for_model(model_id),
        report: FieldMapReport::new(FieldMapSource::Builtin, false, reason),
    };
    let degraded = |reason: String| FieldMapLoad {
        map: FieldMap::for_model(model_id),
        report: FieldMapReport::new(FieldMapSource::Degraded, true, reason),
    };
    let bytes = match fetch {
        OverrideFetch::Fetched(bytes) => bytes,
        OverrideFetch::Missing(e) => return builtin(format!("覆盖表未提供（{e}），用内建默认表")),
        OverrideFetch::Failed(e) => {
            return degraded(format!("覆盖表读取失败（{e}），用内建默认表"));
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => return degraded(format!("覆盖表不是 UTF-8（{e}），用内建默认表")),
    };
    let map = match FieldMap::with_override_json(model_id, &text) {
        Ok(map) => map,
        Err(e) => return degraded(format!("覆盖表校验失败（{e}），用内建默认表")),
    };
    let model = map.model().to_owned();
    let warnings = map.warnings().to_vec();
    // 合法 JSON 但没有本模型 / default 节：内建表即本轮完整映射（O9），不是降级。
    if !map.has_override() {
        let reason = if warnings.is_empty() {
            format!("外置覆盖表无 {model} 节，用内建表")
        } else {
            warnings.join("；")
        };
        return FieldMapLoad {
            map,
            report: FieldMapReport::new(FieldMapSource::Builtin, false, reason),
        };
    }
    let report = if warnings.is_empty() {
        FieldMapReport::new(
            FieldMapSource::External,
            false,
            format!("外置覆盖表已应用 model={model}"),
        )
    } else {
        FieldMapReport::new(
            FieldMapSource::External,
            true,
            format!(
                "外置覆盖表已应用（{} 条告警：{}）",
                warnings.len(),
                warnings.join("；")
            ),
        )
    };
    FieldMapLoad { map, report }
}
