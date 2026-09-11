//! Mod 状态机（E0 ADR §3：enable/disable 生命周期可观察）。

/// Mod 运行时状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModStatus {
    /// 未启用（manifest `enabled=false`）。
    #[default]
    Disabled,
    /// 正在构造/注册。
    Starting,
    /// 运行中（`start` 已成功）。
    Running,
    /// 失败（含原因；核心继续运行，仅 disable 本 Mod）。
    Failed { message: String },
    /// 正在关闭。
    Stopping,
}

impl ModStatus {
    /// 是否已启用（Running / Starting）。
    pub const fn is_enabled(&self) -> bool {
        matches!(self, ModStatus::Starting | ModStatus::Running)
    }

    /// 稳定字符串 id（前端/日志）。
    pub const fn as_str(&self) -> &'static str {
        match self {
            ModStatus::Disabled => "disabled",
            ModStatus::Starting => "starting",
            ModStatus::Running => "running",
            ModStatus::Failed { .. } => "failed",
            ModStatus::Stopping => "stopping",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trip() {
        assert_eq!(ModStatus::default(), ModStatus::Disabled);
        assert!(ModStatus::default().as_str() == "disabled");
        assert!(ModStatus::Running.is_enabled());
        assert!(!ModStatus::Disabled.is_enabled());
        assert_eq!(
            ModStatus::Failed {
                message: "boom".into()
            }
            .as_str(),
            "failed"
        );
    }
}
