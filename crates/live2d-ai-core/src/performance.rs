//! 表演参数曲线层：把收敛后的 [`SemanticAction`] 用纯函数采样为项目自有的
//! [`ParameterFrame`]，供未来 l2d adapter 做字段 → 参数 ID 映射。
//!
//! 设计契约：
//! - **纯 Rust、零依赖、零堆分配**：曲线是 smoothstep/sine 纯函数；
//!   采样结果写入调用方提供的 `&mut ParameterFrame`，全程无 Vec/String/Box；
//! - **不依赖任何 Live2D 库**：[`ParameterFrame`] 是本项目自有字段
//!   （角度制 / 归一化），参数 ID 映射留给渲染适配层；
//! - **单 active**：同一时刻至多一个动作在采样；[`PerformancePlayer::play`]
//!   即抢占——时间轴重置，旧动作不参与叠加。与调度层的单一显式换演效果
//!   [`crate::ActionEffect::Transition`] 对应：shell 收到该效果后对新动作调用一次
//!   `play()` 即可；[`PerformancePlayer::interrupt`] 对应 `ActionEffect::End`，
//!   立刻回到精确 neutral；
//! - **确定性**：相同 (动作, 强度, dt 序列) ⇒ 位级相同的帧序列（同平台构建内）；
//! - **口型所有权**：本层永不驱动嘴开合（MouthOpenY 归 TTS RMS 通道），
//!   至多写 `mouth_form` 微调嘴形；
//! - **安全范围**：所有通道限幅在 Bai 皮套常规范围（头 ±30°、身 ±10°、
//!   眼球/眉/嘴形 ±1、睁眼倍率 [0.5,1.3]）。曲线振幅本身已留余量，限幅只是防线。
//!
//! 时间模型：播放器内部以固定 dt 累计 `elapsed`，`play()` 即「开始时间 = 现在」
//! （elapsed 归零）。进度 p = elapsed/duration ∈ [0,1]；p≥1 输出精确 neutral 并报
//! [`SampleStatus::Finished`]（恰好一次），随后回到 [`SampleStatus::Idle`]。

use crate::action::{ActionId, SemanticAction, Strength};

// ---------------------------------------------------------------- 安全范围

/// 头部角度安全上限（度）：Bai 标准 ParamAngleX/Y/Z 范围 ±30。
pub const HEAD_ANGLE_LIMIT: f32 = 30.0;
/// 身体角度安全上限（度）：标准 ParamBodyAngleX/Y/Z 范围 ±10。
pub const BODY_ANGLE_LIMIT: f32 = 10.0;
/// 归一化通道（眼球 XY / 眉 Y / 嘴形）安全上限。
pub const NORMALIZED_LIMIT: f32 = 1.0;
/// 睁眼倍率下限（防御值；本层只放大睁眼，不主动闭眼——眨眼归自动化层）。
pub const EYE_OPEN_MIN: f32 = 0.5;
/// 睁眼倍率上限（1.3 = 明显瞪眼，不触碰模型开闭极限的激进段）。
pub const EYE_OPEN_MAX: f32 = 1.3;

const TAU: f32 = std::f32::consts::TAU;

// ---------------------------------------------------------------- 曲线元数据

/// 动作时长元数据（秒，固定值；与强度无关），全部为正。
pub const fn duration(action: ActionId) -> f32 {
    match action {
        ActionId::Nod => 0.9,
        ActionId::ShakeNo => 1.1,
        ActionId::Tilt => 1.6,
        ActionId::LookAround => 2.4,
        ActionId::Listen => 2.2,
        ActionId::Surprise => 0.8,
    }
}

/// 强度 → 振幅增益（Low 0.6 / Medium 1.0 / High 1.35）。
///
/// 曲线形状与强度无关（逐通道对增益线性缩放），强度单调性因此按构造成立。
const fn strength_gain(strength: Strength) -> f32 {
    match strength {
        Strength::Low => 0.6,
        Strength::Medium => 1.0,
        Strength::High => 1.35,
    }
}

// ---------------------------------------------------------------- 纯函数基元

/// 截断到 [0,1]。
fn saturate01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// smoothstep(0,1,x)：端点导数为 0 的 S 型过渡；x≤0→0、x≥1→1（均精确）。
fn smoothstep01(x: f32) -> f32 {
    let t = saturate01(x);
    t * t * (3.0 - 2.0 * t)
}

/// 攻击-保持-释放包络：[0,attack] 平滑起、[release_from,1] 平滑落。
/// 两端点精确为 0（smoothstep 在 0/1 处取精确值），保证动作首尾帧中性。
fn envelope(p: f32, attack: f32, release_from: f32) -> f32 {
    smoothstep01(p / attack) * (1.0 - smoothstep01((p - release_from) / (1.0 - release_from)))
}

// ---------------------------------------------------------------- ParameterFrame

/// 一帧表演参数（本项目自有字段，非 Live2D 参数 ID）。
///
/// neutral 语义：角度/归一化通道 `0` = 回中/无偏移；`eye_open_scale` 为
/// **睁眼倍率**，`1.0` = 不干预眨眼基线。未来 l2d adapter 只需做
/// 字段 → 参数 ID 映射（如 `head_angle_z` → ParamAngleZ、`eye_x` → ParamEyeBallX）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterFrame {
    /// 头部水平转（度，±30，>0 向角色左）。
    pub head_angle_x: f32,
    /// 头部俯仰（度，±30，<0 低头）。
    pub head_angle_y: f32,
    /// 头部侧倾（度，±30）。
    pub head_angle_z: f32,
    /// 眼球水平注视（归一化 ±1，>0 向角色左）。
    pub eye_x: f32,
    /// 眼球垂直注视（归一化 ±1，>0 向上）。
    pub eye_y: f32,
    /// 睁眼倍率（乘在眨眼基线上；1.0 = 不干预）。
    pub eye_open_scale: f32,
    /// 眉毛上下偏移（归一化 ±1，>0 上抬/挑眉）。
    pub brow_y: f32,
    /// 嘴形偏移（归一化 ±1）；本层永不写嘴开合（MouthOpenY 归 TTS）。
    pub mouth_form: f32,
    /// 身体水平转（度，±10）。
    pub body_angle_x: f32,
    /// 身体俯仰（度，±10）。
    pub body_angle_y: f32,
    /// 身体侧倾（度，±10）。
    pub body_angle_z: f32,
}

impl ParameterFrame {
    /// 全中性帧（回中、不干预眨眼）。
    pub const fn neutral() -> Self {
        Self {
            head_angle_x: 0.0,
            head_angle_y: 0.0,
            head_angle_z: 0.0,
            eye_x: 0.0,
            eye_y: 0.0,
            eye_open_scale: 1.0,
            brow_y: 0.0,
            mouth_form: 0.0,
            body_angle_x: 0.0,
            body_angle_y: 0.0,
            body_angle_z: 0.0,
        }
    }

    /// 就地写为全中性帧。
    pub fn set_neutral(&mut self) {
        *self = Self::neutral();
    }

    /// 是否与全中性帧逐字段相等（帧由本模块构造，端点值精确）。
    pub fn is_neutral(&self) -> bool {
        *self == Self::neutral()
    }

    /// 就地限幅到 Bai 常规安全范围（防线；曲线振幅本身已留余量）。
    pub fn clamp_to_safe(&mut self) {
        self.head_angle_x = self.head_angle_x.clamp(-HEAD_ANGLE_LIMIT, HEAD_ANGLE_LIMIT);
        self.head_angle_y = self.head_angle_y.clamp(-HEAD_ANGLE_LIMIT, HEAD_ANGLE_LIMIT);
        self.head_angle_z = self.head_angle_z.clamp(-HEAD_ANGLE_LIMIT, HEAD_ANGLE_LIMIT);
        self.body_angle_x = self.body_angle_x.clamp(-BODY_ANGLE_LIMIT, BODY_ANGLE_LIMIT);
        self.body_angle_y = self.body_angle_y.clamp(-BODY_ANGLE_LIMIT, BODY_ANGLE_LIMIT);
        self.body_angle_z = self.body_angle_z.clamp(-BODY_ANGLE_LIMIT, BODY_ANGLE_LIMIT);
        self.eye_x = self.eye_x.clamp(-NORMALIZED_LIMIT, NORMALIZED_LIMIT);
        self.eye_y = self.eye_y.clamp(-NORMALIZED_LIMIT, NORMALIZED_LIMIT);
        self.brow_y = self.brow_y.clamp(-NORMALIZED_LIMIT, NORMALIZED_LIMIT);
        self.mouth_form = self.mouth_form.clamp(-NORMALIZED_LIMIT, NORMALIZED_LIMIT);
        self.eye_open_scale = self.eye_open_scale.clamp(EYE_OPEN_MIN, EYE_OPEN_MAX);
    }
}

impl Default for ParameterFrame {
    fn default() -> Self {
        Self::neutral()
    }
}

// ---------------------------------------------------------------- 采样

/// 把 `(action, strength)` 在 `progress` ∈ [0,1] 处的帧写入 `out`。
///
/// 无状态纯函数、零分配；`progress` 被截断到 [0,1]，≥1 输出精确 neutral
/// （供调试/拖动进度条直接采样，正常播放走 [`PerformancePlayer`]）。
pub fn sample_at(action: SemanticAction, progress: f32, out: &mut ParameterFrame) {
    let p = saturate01(progress);
    if p >= 1.0 {
        out.set_neutral();
        return;
    }
    sample_curve(action.action, strength_gain(action.strength), p, out);
}

/// 六动作曲线本体：`g` 为强度增益，`p` ∈ [0,1)。逐通道对 `g` 线性。
fn sample_curve(action: ActionId, g: f32, p: f32, out: &mut ParameterFrame) {
    out.set_neutral();
    match action {
        // 点头：AngleY 下探-保持-回中；身体以 ~1/12 幅度跟随（≈1°）。
        ActionId::Nod => {
            let e = envelope(p, 0.30, 0.65);
            out.head_angle_y = -12.0 * g * e;
            out.body_angle_y = -(g * e);
        }
        // 摇头：AngleX 左右两个来回（2 周期正弦）× 边缘渐入渐出窗。
        ActionId::ShakeNo => {
            let w = envelope(p, 0.15, 0.85);
            let osc = (TAU * 2.0 * p).sin();
            out.head_angle_x = 14.0 * g * w * osc;
            out.body_angle_x = 1.2 * g * w * osc;
        }
        // 歪头：AngleZ 平滑侧倾-保持-回中；眼球反向微移 + 眉微抬（好奇感）。
        ActionId::Tilt => {
            let e = envelope(p, 0.30, 0.70);
            out.head_angle_z = 10.0 * g * e;
            out.body_angle_z = 0.8 * g * e;
            out.eye_x = -0.15 * g * e;
            out.brow_y = 0.15 * g * e;
        }
        // 环视：眼球大幅左右扫视（2 周期），头部小幅同向跟随，双双回中。
        ActionId::LookAround => {
            let w = envelope(p, 0.12, 0.88);
            let osc = (TAU * 2.0 * p).sin();
            out.eye_x = 0.65 * g * w * osc;
            out.head_angle_x = 8.0 * g * w * osc;
        }
        // 倾听：轻前倾（标准参数无前后倾通道，用身体微倾 + 头微低 + 视线上抬近似）、
        // 歪头、眉微抬；整体带缓慢呼吸式起伏。
        ActionId::Listen => {
            let breathe = envelope(p, 0.25, 0.75) * (0.85 + 0.15 * (TAU * p).sin());
            out.head_angle_z = 6.0 * g * breathe;
            out.head_angle_y = -3.0 * g * breathe;
            out.eye_y = 0.3 * g * breathe;
            out.brow_y = 0.2 * g * breathe;
            out.body_angle_z = 1.5 * g * breathe;
            out.body_angle_y = 2.0 * g * breathe;
        }
        // 惊讶：快起缓落的脉冲——瞪眼（倍率）、挑眉、头部后仰微收。
        // 只写 mouth_form 微调嘴形，绝不驱动 MouthOpenY（TTS 口型专属通道）。
        ActionId::Surprise => {
            let e = envelope(p, 0.15, 0.40);
            out.eye_open_scale = 1.0 + 0.20 * g * e;
            out.brow_y = 0.70 * g * e;
            out.head_angle_y = 4.0 * g * e;
            out.head_angle_z = -2.0 * g * e;
            out.mouth_form = 0.30 * g * e;
        }
    }
    out.clamp_to_safe();
}

// ---------------------------------------------------------------- 播放器

/// 单次采样的结果状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleStatus {
    /// 空闲：无活动动作，`out` 已写 neutral。
    Idle,
    /// 表演中：`out` 为当前帧，`progress` ∈ [0,1)。
    Playing { progress: f32 },
    /// 本帧动作到达终点：`out` 已写精确 neutral，active 已清除。
    Finished,
}

/// 表演播放器：单 active，把 [`SemanticAction`] 按固定 dt 推进为
/// [`ParameterFrame`] 序列。无内部时钟依赖（由调用方喂 dt）、无堆分配。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PerformancePlayer {
    active: Option<ActivePerformance>,
}

/// 活动动作及其已推进的时间（私有）。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActivePerformance {
    action: SemanticAction,
    elapsed: f32,
}

impl PerformancePlayer {
    /// 新建空闲播放器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前活动动作（若有）。
    pub fn active(&self) -> Option<SemanticAction> {
        self.active.map(|a| a.action)
    }

    /// 开始/抢占：无条件重置时间轴（开始时间 = 现在），旧动作不做叠加。
    /// 对应调度层 [`crate::ActionEffect::Transition`]（幂等去重由上游 reducer 负责）。
    pub fn play(&mut self, action: SemanticAction) {
        self.active = Some(ActivePerformance {
            action,
            elapsed: 0.0,
        });
    }

    /// 打断：立即清空活动动作；后续帧为精确 neutral。对应 `ActionEffect::End`。
    pub fn interrupt(&mut self) {
        self.active = None;
    }

    /// 推进一帧并把结果写入 `out`（零分配），返回 [`SampleStatus`]。
    ///
    /// dt 安全规则：非有限或 ≤ 0 的 dt 一律按 0 处理（帧照常输出、时间不前进），
    /// NaN/负 dt 因此不会污染时间轴；超大的有限 dt 则把动作一步推到终点。
    pub fn update(&mut self, dt: f32, out: &mut ParameterFrame) -> SampleStatus {
        let Some(active) = self.active.as_mut() else {
            out.set_neutral();
            return SampleStatus::Idle;
        };
        if dt.is_finite() && dt > 0.0 {
            active.elapsed += dt;
        }
        let dur = duration(active.action.action);
        if active.elapsed < dur {
            let progress = saturate01(active.elapsed / dur);
            let (id, strength) = (active.action.action, active.action.strength);
            sample_curve(id, strength_gain(strength), progress, out);
            SampleStatus::Playing { progress }
        } else {
            self.active = None;
            out.set_neutral();
            SampleStatus::Finished
        }
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    /// 固定测试步长（60 Hz）。
    const DT: f32 = 1.0 / 60.0;
    /// 步数预算：覆盖最长动作（look_around 2.4s ≈ 144 帧）并留余量。
    const MAX_STEPS: usize = 200;

    fn act(id: ActionId, s: Strength) -> SemanticAction {
        SemanticAction::new(id, s, crate::ActionSource::LlmTool)
    }

    /// 从全新播放器采一段帧序列（含结束后的空闲帧）。
    fn sample_frames(id: ActionId, s: Strength, steps: usize) -> Vec<ParameterFrame> {
        let mut player = PerformancePlayer::new();
        player.play(act(id, s));
        let mut f = ParameterFrame::neutral();
        let mut frames = Vec::with_capacity(steps);
        for _ in 0..steps {
            player.update(DT, &mut f);
            frames.push(f);
        }
        frames
    }

    /// 各通道相对中性的偏离幅度（顺序固定）。
    fn abs_channels(f: &ParameterFrame) -> [f32; 11] {
        [
            f.head_angle_x.abs(),
            f.head_angle_y.abs(),
            f.head_angle_z.abs(),
            f.eye_x.abs(),
            f.eye_y.abs(),
            (f.eye_open_scale - 1.0).abs(),
            f.brow_y.abs(),
            f.mouth_form.abs(),
            f.body_angle_x.abs(),
            f.body_angle_y.abs(),
            f.body_angle_z.abs(),
        ]
    }

    fn assert_in_bounds(f: &ParameterFrame) {
        for v in [f.head_angle_x, f.head_angle_y, f.head_angle_z] {
            assert!(
                v.is_finite() && v.abs() <= HEAD_ANGLE_LIMIT,
                "head 越界: {v}"
            );
        }
        for v in [f.body_angle_x, f.body_angle_y, f.body_angle_z] {
            assert!(
                v.is_finite() && v.abs() <= BODY_ANGLE_LIMIT,
                "body 越界: {v}"
            );
        }
        for v in [f.eye_x, f.eye_y, f.brow_y, f.mouth_form] {
            assert!(
                v.is_finite() && v.abs() <= NORMALIZED_LIMIT,
                "归一化通道越界: {v}"
            );
        }
        assert!(
            f.eye_open_scale.is_finite()
                && (EYE_OPEN_MIN..=EYE_OPEN_MAX).contains(&f.eye_open_scale),
            "eye_open_scale 越界: {}",
            f.eye_open_scale
        );
    }

    /// 要求：t=0 与 t=end 都是精确中性帧；结束只报一次 Finished，之后恒 Idle+中性。
    #[test]
    fn t0_and_end_are_exactly_neutral() {
        for id in ActionId::ALL {
            for s in Strength::ALL {
                let mut player = PerformancePlayer::new();
                player.play(act(id, s));
                let mut f = ParameterFrame::neutral();

                let status = player.update(0.0, &mut f);
                assert_eq!(status, SampleStatus::Playing { progress: 0.0 });
                assert!(f.is_neutral(), "{id:?} t=0 必须中性");

                let mut finished = false;
                for _ in 0..MAX_STEPS {
                    match player.update(DT, &mut f) {
                        SampleStatus::Finished => {
                            finished = true;
                            assert!(f.is_neutral(), "{id:?} 结束帧必须精确中性");
                            break;
                        }
                        SampleStatus::Playing { .. } => {}
                        SampleStatus::Idle => unreachable!("表演中不应回到 Idle"),
                    }
                }
                assert!(finished, "{id:?} 应在步数预算内结束");
                assert_eq!(player.active(), None);

                for _ in 0..3 {
                    assert_eq!(player.update(DT, &mut f), SampleStatus::Idle);
                    assert!(f.is_neutral(), "{id:?} 结束后必须保持中性");
                }
            }
        }
    }

    /// 要求：六动作 × 三强度的每一采样帧都有限且落在 Bai 安全范围内。
    #[test]
    fn every_sampled_frame_is_within_safe_bounds() {
        for id in ActionId::ALL {
            for s in Strength::ALL {
                for f in sample_frames(id, s, MAX_STEPS) {
                    assert_in_bounds(&f);
                }
            }
        }
    }

    /// 要求：Low/Medium/High 逐帧、逐通道幅度单调不减（曲线对增益线性）。
    #[test]
    fn strength_scales_every_channel_monotonically() {
        const STEPS: usize = 90;
        for id in ActionId::ALL {
            let low = sample_frames(id, Strength::Low, STEPS);
            let mid = sample_frames(id, Strength::Medium, STEPS);
            let high = sample_frames(id, Strength::High, STEPS);
            for ((l, m), h) in low.iter().zip(&mid).zip(&high) {
                let (lv, mv, hv) = (abs_channels(l), abs_channels(m), abs_channels(h));
                for i in 0..lv.len() {
                    assert!(
                        hv[i] >= mv[i] && mv[i] >= lv[i],
                        "{id:?} 通道#{i} 非单调: low={} mid={} high={}",
                        lv[i],
                        mv[i],
                        hv[i]
                    );
                }
            }
        }
    }

    /// 要求：相同输入序列 ⇒ 位级相同的帧序列（含同一播放器重放）。
    #[test]
    fn sampling_is_deterministic() {
        let a = sample_frames(ActionId::Surprise, Strength::High, 100);
        let b = sample_frames(ActionId::Surprise, Strength::High, 100);
        assert_eq!(a, b);

        let mut player = PerformancePlayer::new();
        player.play(act(ActionId::Surprise, Strength::High));
        let mut f = ParameterFrame::neutral();
        let mut replay = Vec::with_capacity(100);
        for _ in 0..100 {
            player.update(DT, &mut f);
            replay.push(f);
        }
        assert_eq!(replay, a);
    }

    /// 要求：look_around 双向扫视、视线穿过中心、头部小幅跟随，且结束精确回中。
    #[test]
    fn look_around_scans_both_sides_then_recenters() {
        let frames = sample_frames(ActionId::LookAround, Strength::Medium, MAX_STEPS);
        assert!(frames.iter().any(|f| f.eye_x > 0.4), "应看向一侧");
        assert!(frames.iter().any(|f| f.eye_x < -0.4), "应看向另一侧");
        // 正弦网格采样允许的小残差（理论峰值 ~0.03）。
        assert!(
            frames.iter().any(|f| f.eye_x.abs() < 0.05),
            "扫视应穿过中心"
        );
        assert!(
            frames.iter().any(|f| f.head_angle_x.abs() > 2.0),
            "头部应小幅跟随"
        );
        assert!(
            frames.last().is_some_and(|f| f.is_neutral()),
            "结束必须回中"
        );
    }

    /// 要求：interrupt 立即回到精确 neutral，空闲态 interrupt 幂等无害。
    #[test]
    fn interrupt_snaps_to_neutral_immediately() {
        let mut player = PerformancePlayer::new();
        player.play(act(ActionId::Nod, Strength::Medium));
        let mut f = ParameterFrame::neutral();
        for _ in 0..10 {
            player.update(DT, &mut f);
        }
        assert!(!f.is_neutral(), "前置：打断前应为非中性帧");

        player.interrupt();
        assert_eq!(player.active(), None);
        for _ in 0..5 {
            assert_eq!(player.update(DT, &mut f), SampleStatus::Idle);
            assert!(f.is_neutral());
        }

        player.interrupt(); // 空闲态再打断：无害。
        player.play(act(ActionId::Tilt, Strength::Low));
        assert_eq!(
            player.update(0.0, &mut f),
            SampleStatus::Playing { progress: 0.0 }
        );
        assert!(f.is_neutral());
    }

    /// 要求：抢占（对应 Transition）重置时间轴，旧动作完全不叠加。
    #[test]
    fn preempt_resets_timeline_without_blending() {
        let mut player = PerformancePlayer::new();
        player.play(act(ActionId::Nod, Strength::Medium));
        let mut f = ParameterFrame::neutral();
        for _ in 0..15 {
            player.update(DT, &mut f);
        }
        assert!(!f.is_neutral());

        // 抢占换演后与全新播放器的 shake_no 逐位一致。
        player.play(act(ActionId::ShakeNo, Strength::Low));
        let mut reference = PerformancePlayer::new();
        reference.play(act(ActionId::ShakeNo, Strength::Low));
        let mut rf = ParameterFrame::neutral();
        for _ in 0..40 {
            player.update(DT, &mut f);
            reference.update(DT, &mut rf);
            assert_eq!(f, rf, "抢占后不得残留旧动作分量");
        }
    }

    /// 要求：NaN / 负 dt / 零 dt 不推进时间轴；巨大有限 dt 安全终止；NaN 不入帧。
    #[test]
    fn nan_negative_and_huge_dt_are_safe() {
        let mut player = PerformancePlayer::new();
        player.play(act(ActionId::ShakeNo, Strength::Medium));
        let mut f = ParameterFrame::neutral();
        player.update(DT, &mut f);
        let baseline = f;

        for bad in [f32::NAN, -DT, 0.0, f32::NEG_INFINITY] {
            let status = player.update(bad, &mut f);
            assert!(matches!(status, SampleStatus::Playing { .. }));
            assert_eq!(f, baseline, "非法 dt({bad}) 不得改变输出帧");
        }

        assert_eq!(player.update(1.0e9, &mut f), SampleStatus::Finished);
        assert!(f.is_neutral());

        assert_eq!(player.update(f32::NAN, &mut f), SampleStatus::Idle);
        assert!(f.is_neutral());
    }

    /// 要求：无状态采样截断 progress；neutral 语义与时长元数据成立。
    #[test]
    fn stateless_sample_at_and_metadata() {
        let n = ParameterFrame::neutral();
        assert!(n.is_neutral());
        assert_eq!(n.eye_open_scale, 1.0);
        let mut m = n;
        m.head_angle_z = 5.0;
        assert!(!m.is_neutral());

        let a = act(ActionId::Tilt, Strength::High);
        let mut out = ParameterFrame::neutral();
        sample_at(a, 0.5, &mut out);
        assert!(out.head_angle_z > 0.0);
        for over in [1.0, 1.5, 42.0] {
            sample_at(a, over, &mut out);
            assert!(out.is_neutral());
        }
        sample_at(a, -3.0, &mut out); // 截断到 0 → 中性。
        assert!(out.is_neutral());

        for id in ActionId::ALL {
            let d = duration(id);
            assert!(d.is_finite() && d > 0.0, "{id:?} 时长必须为正");
        }
        assert_eq!(duration(ActionId::Nod), 0.9);
        assert_eq!(duration(ActionId::LookAround), 2.4);
    }
}
