//! model smoke 运行统计（并入 [`crate::backend::RunReport`]；全部来自真实运行观测）。

use std::time::Duration;

/// model smoke 运行统计（并入 RunReport；全部来自真实运行观测）。
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSmokeStats {
    /// 实际使用的 model3.json 路径。
    pub model3_path: String,
    /// 兼容报告摘要行（notes + issues + v0 判定）。
    pub compat_lines: Vec<String>,
    /// 兼容判定：v0 是否可播放。
    pub compat_supported_v0: bool,
    /// 成功渲染并 present 的模型帧数。
    pub frames_rendered: u64,
    /// 执行的模拟步数（60 Hz 步）。
    pub sim_steps: u64,
    /// 平均单帧耗时（tick+渲染+present；None = 无成功帧）。
    pub avg_frame_time: Option<Duration>,
    /// 最慢单帧耗时。
    pub max_frame_time: Option<Duration>,
    /// 已完成动作次数。
    pub actions_completed: u32,
    /// 口型通道写入次数（含 0 电平帧）。
    pub mouth_writes: u64,
    /// 口型电平峰值（观测通道确实在动）。
    pub mouth_peak: f32,
    /// 模型缺失而降级的参数 ID（空 = 全参数可用）。
    pub missing_params: Vec<String>,
    /// 最终视口 `(width, height)`。
    pub viewport: (u32, u32),
}

impl ModelSmokeStats {
    /// 逐行摘要（CLI 打印用）。
    pub fn summarize_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("model         : {}", self.model3_path),
            format!(
                "compat        : v0={} （{} 条备注/问题，见启动日志）",
                self.compat_supported_v0,
                self.compat_lines.len()
            ),
            format!(
                "frames        : rendered={} sim_steps={}",
                self.frames_rendered, self.sim_steps
            ),
            format!(
                "frame_time    : avg={:?} max={:?}",
                self.avg_frame_time.unwrap_or_default(),
                self.max_frame_time.unwrap_or_default()
            ),
            format!(
                "actions       : completed={}（六动作固定顺序循环，Medium）",
                self.actions_completed
            ),
            format!(
                "mouth_channel : writes={} peak={:.3}",
                self.mouth_writes, self.mouth_peak
            ),
            format!("viewport      : {}x{}", self.viewport.0, self.viewport.1),
        ];
        if self.missing_params.is_empty() {
            lines.push("degraded      : (无缺失参数)".to_owned());
        } else {
            lines.push(format!(
                "degraded      : 缺失参数 {:?}",
                self.missing_params
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_summary_covers_key_channels() {
        let stats = ModelSmokeStats {
            model3_path: "/tmp/bai.model3.json".into(),
            compat_lines: vec!["note".into()],
            compat_supported_v0: true,
            frames_rendered: 150,
            sim_steps: 160,
            avg_frame_time: Some(Duration::from_millis(12)),
            max_frame_time: Some(Duration::from_millis(40)),
            actions_completed: 3,
            mouth_writes: 150,
            mouth_peak: 0.98,
            missing_params: vec![],
            viewport: (480, 640),
        };
        let text = stats.summarize_lines().join("\n");
        for needle in [
            "bai.model3.json",
            "rendered=150",
            "sim_steps=160",
            "completed=3",
            "writes=150",
            "peak=0.980",
            "480x640",
            "(无缺失参数)",
        ] {
            assert!(text.contains(needle), "摘要缺少 {needle}:\n{text}");
        }
    }
}
