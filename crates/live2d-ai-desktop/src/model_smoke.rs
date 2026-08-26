//! model smoke（Bai 实时渲染冒烟）的纯逻辑层：类型化错误 / 动作序列 /
//! 低频合成口型电平 / 运行统计。本模块不触碰 GPU、窗口与事件循环，
//! 全部可无显示单测；接线在 [`crate::app`]。
//!
//! 冒烟语义（RFC §4 批次 5 推进，D10）：
//! - 自动按固定顺序循环播放六动作（每动作 Medium 强度；上一动作结束
//!   才开始下一动作），同时以低频合成口型电平验证 `ParamMouthOpenY`
//!   通道——**不访问 LLM/TTS/声卡**；
//! - 失败类型化：模型代码错误/资产问题 → 退出码 1；GPU 环境不满足 → 退出码 3。

use std::time::Duration;

use l2d::renderer::RenderError;
use live2d_ai_core::{
    ActionId, ActionSource, ParameterFrame, PerformancePlayer, SampleStatus, SemanticAction,
    Strength,
};

// ---------------------------------------------------------------- 固定步长

/// 模拟固定步长：60 Hz（l2d bakeoff 验证值；物理引擎按此步进最稳定）。
pub const SIM_DT: f32 = l2d::renderer::FIXED_DT_60HZ;

/// 单帧墙钟 dt 的上限（秒）：超过按该值截断（拖尾帧不放大模拟步数）。
///
/// 合理范围口径：正常 60 Hz present 下 dt ≈ 16.7 ms；前台切换/断点等异常长帧
/// 按 250 ms 截断，避免一次追帧把模拟时间快进数秒。
pub const MAX_FRAME_DT_SECS: f32 = 0.25;

/// 累加器水位上限：最多预存 4 个模拟步（约一帧的量），超出部分丢弃——
/// 渲染跟不上时宁可放慢模拟时间也不累积爆炸（螺旋死亡保护）。
pub const MAX_ACCUMULATOR_STEPS: u32 = 4;

/// 单帧最多执行的模拟步数（防御性上限；正常 ≤ [`MAX_ACCUMULATOR_STEPS`]）。
pub const MAX_TICKS_PER_FRAME: u32 = 8;

// ---------------------------------------------------------------- 错误分类

/// model smoke 的类型化失败。类别决定退出码：
/// [`ModelSmokeError::Asset`] / [`ModelSmokeError::ModelCode`] → 1（代码/资产问题），
/// [`ModelSmokeError::GpuEnvironment`] → 3（运行环境不满足）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelSmokeError {
    /// 资产问题：model3.json/moc3/纹理/physics 文件缺失或清单非法、
    /// 兼容报告否决 v0 播放。退出码 1。
    Asset(String),
    /// 模型代码错误：moc3 解析失败、渲染核心加载模型失败（模型内容无法构建）。
    /// 退出码 1。
    ModelCode(String),
    /// GPU 环境不满足：adapter/device/pipeline 创建失败、渲染中 poll 失败等。
    /// 与代码缺陷区分，退出码 3。
    GpuEnvironment(String),
}

impl ModelSmokeError {
    /// 类别名（日志/报告用）。
    ///
    /// 退出码映射（在 [`crate::app`] 落地）：Asset / ModelCode →
    /// `BackendError::Failed` → 进程退出码 **1**；GpuEnvironment →
    /// `BackendError::Environment` → 进程退出码 **3**。
    pub fn category(&self) -> &'static str {
        match self {
            Self::Asset(_) => "asset",
            Self::ModelCode(_) => "model-code",
            Self::GpuEnvironment(_) => "gpu-environment",
        }
    }
}

impl std::fmt::Display for ModelSmokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asset(msg) => write!(f, "皮套资产问题: {msg}"),
            Self::ModelCode(msg) => write!(f, "模型代码错误: {msg}"),
            Self::GpuEnvironment(msg) => write!(f, "GPU 环境不满足: {msg}"),
        }
    }
}

impl std::error::Error for ModelSmokeError {}

impl From<l2d::asset::LoadError> for ModelSmokeError {
    fn from(e: l2d::asset::LoadError) -> Self {
        Self::Asset(e.to_string())
    }
}

impl From<l2d::model::ModelError> for ModelSmokeError {
    fn from(e: l2d::model::ModelError) -> Self {
        Self::ModelCode(e.to_string())
    }
}

/// 渲染核心创建失败（`ModelRendererCore::new`：在既有 device 上建管线）。
///
/// 管线创建失败通常是设备/驱动能力不足 → GPU 环境（退出码 3）。
pub fn classify_core_init_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::Renderer(_) => ModelSmokeError::GpuEnvironment(format!("渲染核心初始化: {e}")),
        _ => ModelSmokeError::ModelCode(format!("渲染核心初始化: {e}")),
    }
}

/// 模型加载进渲染核心失败（`ModelRendererCore::load_model`）。
///
/// physics3.json 属资产文件（解析失败 → 资产问题）；其余是模型内容无法
/// 构建为可绘制对象 → 模型代码错误。均退出码 1。
pub fn classify_load_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::PhysicsJson(msg) => ModelSmokeError::Asset(format!("physics3.json: {msg}")),
        other => ModelSmokeError::ModelCode(format!("load_model: {other}")),
    }
}

/// 逐帧渲染阶段失败（`render_to_view`）。
///
/// poll 失败/回调通道关闭属设备异常 → GPU 环境；dt 非法等参数错误属代码缺陷。
pub fn classify_frame_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::Poll(_) | RenderError::ReadbackClosed => {
            ModelSmokeError::GpuEnvironment(format!("渲染帧提交/等待: {e}"))
        }
        other => ModelSmokeError::ModelCode(format!("渲染帧: {other}")),
    }
}

// ---------------------------------------------------------------- 动作序列

/// 构造脚本动作：固定 Medium 强度、来源标记为规则 fallback（确定性脚本，
/// 不是 LLM tool 输出——诚实标注来源便于观测区分）。
pub fn scripted_action(action: ActionId) -> SemanticAction {
    SemanticAction::new(action, Strength::Medium, ActionSource::RuleFallback)
}

/// 六动作固定顺序播放器：`nod → shake_no → tilt → look_around → listen → surprise`
/// （即 [`ActionId::ALL`] 顺序），循环回绕。每动作 [`Strength::Medium`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSequence {
    index: usize,
}

impl Default for ActionSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionSequence {
    /// 新序列：从第一个动作开始。
    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// 当前应播放的动作（不推进）。
    pub fn current(&self) -> SemanticAction {
        scripted_action(ActionId::ALL[self.index])
    }

    /// 推进到下一动作（动作结束/被打断时调用）；末尾回绕到第一个。
    pub fn advance(&mut self) -> SemanticAction {
        self.index = (self.index + 1) % ActionId::ALL.len();
        self.current()
    }

    /// 完整顺序的动作名表（测试与文档对照用）。
    pub fn names() -> Vec<&'static str> {
        ActionId::ALL.iter().map(|a| a.name()).collect()
    }
}

// ---------------------------------------------------------------- 口型合成

/// 合成口型的「说话/停顿」周期（秒）：低频门控，模拟一段一段的话语节奏。
pub const MOUTH_CYCLE_SECS: f32 = 3.0;
/// 周期内处于「说话」状态的时间占比（前段说话、后段静默）。
pub const MOUTH_DUTY: f32 = 0.6;
/// 音节级开合频率（Hz）：说话段内嘴一张一合的节奏。
pub const SYLLABLE_HZ: f32 = 2.5;

/// 低频合成口型电平 ∈ [0,1]（纯函数、确定性与模拟时钟绑定）。
///
/// 波形 = 「说话门控」（周期 [`MOUTH_CYCLE_SECS`]、占空比 [`MOUTH_DUTY`]）
/// × 音节正弦平方（频率 [`SYLLABLE_HZ`]）。用于验证 `ParamMouthOpenY`
/// 参数通道端到端可达，**不访问 LLM/TTS/声卡**。
///
/// 非法输入（非有限/负时间）一律输出 0（闭嘴）。
pub fn synthesized_mouth_level(sim_time: f32) -> f32 {
    if !sim_time.is_finite() || sim_time < 0.0 || MOUTH_CYCLE_SECS <= 0.0 {
        return 0.0;
    }
    let phase = sim_time % MOUTH_CYCLE_SECS;
    if phase >= MOUTH_CYCLE_SECS * MOUTH_DUTY {
        return 0.0; // 静默段：明确闭嘴。
    }
    let syllable = (std::f32::consts::TAU * SYLLABLE_HZ * phase).sin() * 0.5 + 0.5;
    let level = syllable * syllable; // 平滑、峰值 1、谷值 0。
    if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

// ---------------------------------------------------------------- 冒烟驱动

/// 把 core 表演播放器与固定动作序列绑定的冒烟驱动（纯逻辑，无时钟依赖）。
#[derive(Debug)]
pub struct SmokeDriver {
    player: PerformancePlayer,
    sequence: ActionSequence,
    completed: u32,
}

/// 一次模拟 tick 的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TickOutcome {
    /// 当前动作表演中。
    Playing,
    /// 本 tick 恰好完成了一个动作并已切换到下一动作（首帧中性）。
    Advanced {
        /// 刚完成的动作名。
        finished: &'static str,
        /// 新开始的动作名。
        started: &'static str,
        /// 已完成动作总数。
        completed: u32,
    },
}

impl SmokeDriver {
    /// 新驱动：立即开始第一个动作（时间轴归零）。
    pub fn new() -> Self {
        let mut driver = Self {
            player: PerformancePlayer::new(),
            sequence: ActionSequence::new(),
            completed: 0,
        };
        driver.player.play(driver.sequence.current());
        driver
    }

    /// 当前活动动作（若有；完成瞬间到下一次 play 之间可能为 None）。
    pub fn active(&self) -> Option<SemanticAction> {
        self.player.active()
    }

    /// 已完成的动作总数。
    pub const fn completed(&self) -> u32 {
        self.completed
    }

    /// 推进一个固定模拟步并把表演帧写入 `out`（零分配缓冲由调用方持有）。
    ///
    /// 动作到达终点时：记录完成 → 序列推进 → 立即开始下一动作（本步输出
    /// 中性帧，下一动作从下一步起采样）——“动作结束下一动作”的固定顺序语义。
    pub fn tick(&mut self, out: &mut ParameterFrame) -> TickOutcome {
        // 序列只在完成时推进，因此当前序列位置即正在播放的动作。
        let playing = self.sequence.current().action.name();
        match self.player.update(SIM_DT, out) {
            SampleStatus::Finished => {
                self.completed += 1;
                let started = self.sequence.advance();
                self.player.play(started);
                TickOutcome::Advanced {
                    finished: playing,
                    started: started.action.name(),
                    completed: self.completed,
                }
            }
            // Playing / Idle（Idle 只出现在极端越界 dt 后，输出已是中性帧）。
            _ => TickOutcome::Playing,
        }
    }
}

// ---------------------------------------------------------------- 帧时钟

/// 固定步长累加器：真实墙钟 dt（截断到合理范围）→ 60 Hz 模拟步。
///
/// - 输入 dt 先夹到 `[0, MAX_FRAME_DT_SECS]`（非有限按 0）；
/// - 累加水位封顶 [`MAX_ACCUMULATOR_STEPS`] 步，超出丢弃（防螺旋死亡）；
/// - [`Self::advance`] 返回本帧应执行的模拟步数并在内部等量扣减。
#[derive(Debug, Clone, PartialEq)]
pub struct FrameClock {
    accumulator: f32,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock {
    /// 新时钟（零累计）。
    pub fn new() -> Self {
        Self { accumulator: 0.0 }
    }

    /// 当前未消费的累计量（秒；诊断/测试观测用）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn accumulator(&self) -> f32 {
        self.accumulator
    }

    /// 累计一段墙钟时间，返回本帧应执行的模拟步数。
    pub fn advance(&mut self, wall_dt: f32) -> u32 {
        let dt = if wall_dt.is_finite() && wall_dt > 0.0 {
            wall_dt.min(MAX_FRAME_DT_SECS)
        } else {
            0.0
        };
        self.accumulator += dt;
        let cap = MAX_ACCUMULATOR_STEPS as f32 * SIM_DT;
        if self.accumulator > cap {
            self.accumulator = cap;
        }
        let mut ticks = (self.accumulator / SIM_DT).floor() as u32;
        ticks = ticks.min(MAX_TICKS_PER_FRAME);
        self.accumulator -= ticks as f32 * SIM_DT;
        ticks
    }
}
// ---------------------------------------------------------------- 统计

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
    fn error_categories_map_to_exit_code_contract() {
        assert_eq!(ModelSmokeError::Asset("x".into()).category(), "asset");
        assert_eq!(
            ModelSmokeError::GpuEnvironment("x".into()).category(),
            "gpu-environment"
        );
        // Display 文案可区分三类问题。
        assert!(
            ModelSmokeError::Asset("缺文件".into())
                .to_string()
                .contains("资产")
        );
        assert!(
            ModelSmokeError::ModelCode("解析失败".into())
                .to_string()
                .contains("模型代码")
        );
        assert!(
            ModelSmokeError::GpuEnvironment("无 adapter".into())
                .to_string()
                .contains("GPU 环境")
        );
    }

    #[test]
    fn render_error_classification_matches_stage_semantics() {
        // load 阶段：physics JSON 是资产问题，其余是模型代码错误。
        assert_eq!(
            classify_load_error(RenderError::PhysicsJson("bad json".into())),
            ModelSmokeError::Asset("physics3.json: bad json".into())
        );
        assert!(matches!(
            classify_load_error(RenderError::Renderer("build fail".into())),
            ModelSmokeError::ModelCode(_)
        ));
        // 核心初始化阶段：管线建不起来是 GPU 环境问题。
        assert!(matches!(
            classify_core_init_error(RenderError::Renderer("pipeline".into())),
            ModelSmokeError::GpuEnvironment(_)
        ));
        // 帧阶段：poll/回调通道异常是 GPU 环境；dt 非法是代码缺陷。
        assert!(matches!(
            classify_frame_error(RenderError::ReadbackClosed),
            ModelSmokeError::GpuEnvironment(_)
        ));
        assert!(matches!(
            classify_frame_error(RenderError::Poll("lost".into())),
            ModelSmokeError::GpuEnvironment(_)
        ));
        assert!(matches!(
            classify_frame_error(RenderError::InvalidDt(-1.0)),
            ModelSmokeError::ModelCode(_)
        ));
    }

    #[test]
    fn action_sequence_plays_fixed_order_then_wraps() {
        assert_eq!(
            ActionSequence::names(),
            vec![
                "nod",
                "shake_no",
                "tilt",
                "look_around",
                "listen",
                "surprise"
            ]
        );
        let mut seq = ActionSequence::new();
        let mut seen = Vec::new();
        for _ in 0..(ActionId::ALL.len() + 2) {
            seen.push(seq.current().action.name());
            seq.advance();
        }
        // 固定顺序循环：第 7、8 个动作回到序列开头；当前位置与名字一致。
        assert_eq!(
            &seen[..6],
            &[
                "nod",
                "shake_no",
                "tilt",
                "look_around",
                "listen",
                "surprise"
            ][..]
        );
        assert_eq!(seen[6], "nod");
        assert_eq!(seen[7], "shake_no");
        assert_eq!(
            seq.current().action.name(),
            "tilt",
            "推进 8 次后应位于序列第 3 位"
        );
    }

    #[test]
    fn scripted_actions_are_always_medium_rule_fallback() {
        for id in ActionId::ALL {
            let a = scripted_action(id);
            assert_eq!(a.strength, Strength::Medium);
            assert_eq!(a.source, ActionSource::RuleFallback);
            assert_eq!(a.action, id);
        }
    }

    #[test]
    fn smoke_driver_finishes_each_action_before_the_next() {
        let mut driver = SmokeDriver::new();
        // 构造即开始第一个动作。
        assert_eq!(
            driver.active().map(|a| a.action.name()),
            Some(ActionId::ALL[0].name())
        );
        let mut frame = live2d_ai_core::ParameterFrame::neutral();
        let mut transitions: Vec<(&'static str, &'static str)> = Vec::new();
        let names = ActionSequence::names();
        // 两轮完整序列的步数预算（最长动作 look_around 2.4s ≈ 144 步）。
        for _ in 0..(names.len() * 2 * 200) {
            match driver.tick(&mut frame) {
                TickOutcome::Playing => {}
                TickOutcome::Advanced {
                    finished,
                    started,
                    completed,
                } => {
                    // 完成瞬间输出精确中性帧。
                    assert!(frame.is_neutral());
                    transitions.push((finished, started));
                    assert_eq!(completed, transitions.len() as u32);
                }
            }
            if driver.completed() > names.len() as u32 {
                break;
            }
        }
        let started: Vec<&'static str> = transitions.iter().map(|(_, started)| *started).collect();
        // 第一个动作在构造时已开始，故 started 是名字表左旋一位的循环序列
        // （nod 完成后起 shake_no … surprise 完成后回绕到 nod）。
        let mut rotated = names.clone();
        rotated.rotate_left(1);
        assert_eq!(&started[..6], &rotated[..]);
        // 第 7 次完成：surprise 已回绕到 nod，故此时开始的动作是第二轮的
        // 第二个动作（shake_no）。
        assert_eq!(started[6], names[1]);
        // finished 与 started 首尾相接。
        for pair in transitions.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        assert_eq!(transitions[0].0, "nod");
    }

    #[test]
    fn synthesized_mouth_level_is_bounded_gated_and_deterministic() {
        // 全周期采样：值域 [0,1]、静默段恒 0、峰值足够高。
        let steps = 600;
        let mut peak = 0.0_f32;
        let mut silent_seen = false;
        let mut speaking_seen = false;
        for i in 0..steps {
            let t = i as f32 * MOUTH_CYCLE_SECS / steps as f32;
            let level = synthesized_mouth_level(t);
            assert!(
                level.is_finite() && (0.0..=1.0).contains(&level),
                "t={t} level={level}"
            );
            peak = peak.max(level);
            if level == 0.0 {
                silent_seen = true;
            } else {
                speaking_seen = true;
            }
        }
        assert!(peak > 0.9, "合成电平应有接近满幅的峰值，实际 {peak}");
        assert!(silent_seen && speaking_seen, "必须有说话段与静默段");
        // 静默段（占空比之外）恒为 0。
        let rest_start = MOUTH_CYCLE_SECS * MOUTH_DUTY + 0.01;
        assert_eq!(synthesized_mouth_level(rest_start), 0.0);
        assert_eq!(synthesized_mouth_level(MOUTH_CYCLE_SECS * MOUTH_DUTY), 0.0);
        // 同周期内重复调用位级一致；跨周期相位相同者允许模运算的浮点微差。
        assert_eq!(synthesized_mouth_level(0.37), synthesized_mouth_level(0.37));
        let a = synthesized_mouth_level(0.37);
        let b = synthesized_mouth_level(0.37 + MOUTH_CYCLE_SECS * 7.0);
        assert!(
            (a - b).abs() < 1e-3,
            "跨周期同相位电平应近似一致: {a} vs {b}"
        );
        for bad in [f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(synthesized_mouth_level(bad), 0.0);
        }
    }

    #[test]
    fn frame_clock_accumulates_fixed_steps_with_caps() {
        let mut clock = FrameClock::new();
        assert_eq!(clock.advance(0.0), 0);
        // 一个标准步触发一个 tick。
        assert_eq!(clock.advance(SIM_DT), 1);
        // 小于一步的时间不触发，累计后触发。
        assert_eq!(clock.advance(SIM_DT / 2.0), 0);
        assert_eq!(clock.advance(SIM_DT / 2.0), 1);
        // 超长帧被截断并封顶：最多 MAX_ACCUMULATOR_STEPS 个 tick。
        let burst = clock.advance(MAX_FRAME_DT_SECS);
        assert_eq!(burst, MAX_ACCUMULATOR_STEPS);
        assert!(clock.accumulator() < SIM_DT);
        // 非法输入按 0 处理。
        assert_eq!(clock.advance(f32::NAN), 0);
        assert_eq!(clock.advance(-1.0), 0);
        // 契约：非有限输入按 0 处理（不触发追帧）。
        assert_eq!(clock.advance(f32::INFINITY), 0);
        // 连续正常帧：60Hz 下每帧恰好一步。
        let mut clock = FrameClock::new();
        for _ in 0..10 {
            assert_eq!(clock.advance(SIM_DT), 1);
        }
    }

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
