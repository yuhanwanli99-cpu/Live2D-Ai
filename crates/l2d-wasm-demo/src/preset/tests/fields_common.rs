//! 阶段4c 字段回归的共享测试夹具（从 fields.rs 拆出，避免两个测试文件各抄一份）。
//!
//! 只放夹具（fake sink / cue 构造 / 状态机构造 / 断言辅助），不放 `#[test]`。

use super::*;

/// 可控「缺参数」的 fake sink：`missing` 里的参数 `override_param` 返回 false
/// （复刻皮套缺通道时 `ModelRendererCore::override_parameter` 的行为）。
#[derive(Default)]
pub(super) struct FieldSink {
    pub(super) values: HashMap<String, f32>,
    pub(super) missing: std::collections::HashSet<String>,
    pub(super) cleared: Vec<String>,
}

impl FieldSink {
    pub(super) fn missing(params: &[&str]) -> Self {
        Self {
            missing: params.iter().map(|p| (*p).to_string()).collect(),
            ..Self::default()
        }
    }

    pub(super) fn get(&self, param: &str) -> Option<f32> {
        self.values.get(param).copied()
    }
}

impl PresetSink for FieldSink {
    fn override_param(&mut self, id: &str, value: f32) -> bool {
        if self.missing.contains(id) {
            return false;
        }
        self.values.insert(id.to_string(), value);
        true
    }

    fn clear_override_param(&mut self, id: &str) -> bool {
        self.cleared.push(id.to_string());
        self.values.remove(id).is_some()
    }
}

/// 默认 cue（now / hold=false / intensity 1 / 该字段默认 ttl）。
pub(super) fn cue(field: Field, epoch: i64, seq: u32) -> FieldCue {
    FieldCue {
        seq,
        epoch,
        has_epoch: true,
        field,
        x: None,
        y: None,
        z: None,
        id: None,
        intensity: 1.0,
        at: Anchor::Now,
        hold: false,
        ttl_ms: default_ttl_ms(field),
    }
}

/// 倍率全 1.0 的状态机（账好算；出厂倍率另有回归）。
pub(super) fn runtime() -> FieldRuntime {
    let mut rt = FieldRuntime::new(FieldMap::builtin());
    rt.set_scales(PresetScales::default());
    rt
}

/// 取某个参数的本帧写入（不存在 → panic）。
pub(super) fn write_of<'a>(out: &'a FrameOutcome, param: &str) -> &'a FrameWrite {
    out.writes
        .iter()
        .find(|w| w.param == param)
        .unwrap_or_else(|| panic!("本帧没有写 {param}：{:?}", out.writes))
}

/// 事件类型序列。
pub(super) fn kinds(events: &[AckEvent]) -> Vec<AckKind> {
    events.iter().map(|e| e.kind).collect()
}
