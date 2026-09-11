//! 表演参数曲线层：把收敛后的 [`SemanticAction`] 用纯函数采样为项目自有的
//! [`ParameterFrame`]，供未来 l2d adapter 做字段 → 参数 ID 映射。
//!
//! 设计契约：
//! - **纯 Rust、零依赖、零堆分配**：曲线是 smoothstep/sine 纯函数；
//!   采样结果写入调用方提供的 `&mut ParameterFrame`，全程无 Vec/String/Box；
//! - **不依赖任何 Live2D 库**：[`ParameterFrame`] 是本项目自有字段
//!   （角度制 / 归一化），参数 ID 映射留给渲染适配层；
//! - **单 active**：同一时刻至多一个动作在采样；[`player::PerformancePlayer::play`]
//!   即抢占——时间轴重置，旧动作不参与叠加。与调度层的单一显式换演效果
//!   [`crate::ActionEffect::Transition`] 对应：shell 收到该效果后对新动作调用一次
//!   `play()` 即可；[`player::PerformancePlayer::interrupt`] 对应 `ActionEffect::End`，
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
//!
//! 子模块：
//! - [`player`]：单活动动作的时间轴播放器（[`player::PerformancePlayer`]）。
//!   本模块本体提供参数掩码 / 参数帧 / 采样纯函数与无状态入口 [`sample_at`]。

mod player;

pub use player::PerformancePlayer;

use crate::action::{ActionId, Strength};

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

// ---------------------------------------------------------------- 参数所有权

/// 帧级「动作所有权」位图（P0-5 裁决）：置位的通道表示**本帧由动作主动驱动**，
/// adapter 才允许把它写进姿态栈的 input 层；未置位的通道一律不写——
/// 「中性」意味着释放所有权，而不是持续写零压住 idle/物理层。
///
/// 位与字段的对应关系固定如下（见 [`ParameterFrame`] 字段文档）：
/// `HEAD_ANGLE_X..BODY_ANGLE_Z` 共 11 位；adapter 层再把 `EYE_OPEN_SCALE`
/// 双写到左右眼、`BROW_Y` 双写到左右眉。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParameterMask(u16);

impl ParameterMask {
    /// 空掩码：无任何通道被动作持有。
    pub const EMPTY: Self = Self(0);
    /// 头部水平转。
    pub const HEAD_ANGLE_X: Self = Self(1 << 0);
    /// 头部俯仰。
    pub const HEAD_ANGLE_Y: Self = Self(1 << 1);
    /// 头部侧倾。
    pub const HEAD_ANGLE_Z: Self = Self(1 << 2);
    /// 眼球水平注视。
    pub const EYE_X: Self = Self(1 << 3);
    /// 眼球垂直注视。
    pub const EYE_Y: Self = Self(1 << 4);
    /// 睁眼倍率（adapter 双写左右眼）。
    pub const EYE_OPEN_SCALE: Self = Self(1 << 5);
    /// 眉毛上下偏移（adapter 双写左右眉）。
    pub const BROW_Y: Self = Self(1 << 6);
    /// 嘴形偏移（永不包含嘴开合——MouthOpenY 归口型通道专属）。
    pub const MOUTH_FORM: Self = Self(1 << 7);
    /// 身体水平转。
    pub const BODY_ANGLE_X: Self = Self(1 << 8);
    /// 身体俯仰。
    pub const BODY_ANGLE_Y: Self = Self(1 << 9);
    /// 身体侧倾。
    pub const BODY_ANGLE_Z: Self = Self(1 << 10);

    /// 合并另一个掩码。
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// 是否持有某通道（对 [`Self::EMPTY`] 恒为真——空集被任何集合包含）。
    pub const fn contains(self, ch: Self) -> bool {
        self.0 & ch.0 == ch.0
    }
}

// ---------------------------------------------------------------- ParameterFrame

/// 一帧表演参数（本项目自有字段，非 Live2D 参数 ID）。
///
/// neutral 语义：角度/归一化通道 `0` = 回中/无偏移；`eye_open_scale` 为
/// **睁眼倍率**，`1.0` = 不干预眨眼基线。l2d adapter 只需做
/// 字段 → 参数 ID 映射（如 `head_angle_z` → ParamAngleZ、`eye_x` → ParamEyeBallX）。
///
/// **稀疏所有权**（P0-5）：[`Self::owned`] 列出本帧由动作**主动驱动**的通道；
/// 只有这些通道会被写入 input 层。动作结束/被打断后帧回到
/// `neutral() + EMPTY` 掩码，adapter 据此对上一帧仍持有的通道执行
/// `clear_parameter`——让 idle/物理立即接管，而不是被持续写入的中性值冻住。
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
    ///
    /// v0 注意：本项目当前**没有**自动眨眼驱动源，不声称「模型自带眨眼
    /// 会自动运行」；blink 未来应作为独立参数源接入并与本倍率合成。
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
    /// 本帧被动作持有的通道位图；采样器负责填写，空闲/终点帧为空。
    pub owned: ParameterMask,
}

impl ParameterFrame {
    /// 全中性帧（回中、不干预眨眼、无任何所有权）。
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
            owned: ParameterMask::EMPTY,
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
pub fn sample_at(action: crate::action::SemanticAction, progress: f32, out: &mut ParameterFrame) {
    let p = saturate01(progress);
    if p >= 1.0 {
        out.set_neutral();
        return;
    }
    sample_curve(action.action, strength_gain(action.strength), p, out);
}

/// 六动作曲线本体：`g` 为强度增益，`p` ∈ [0,1)。逐通道对 `g` 线性。
///
/// 每个分支同时声明本动作**持有的通道位图**（P0-5）：曲线驱动的字段必须置位，
/// 未置位的字段绝不定值——adapter 只写持有通道，其余交给 idle/物理。
fn sample_curve(action: ActionId, g: f32, p: f32, out: &mut ParameterFrame) {
    out.set_neutral();
    if p <= 0.0 {
        // 起点帧精确中性（值与所有权都为空）：动作尚未驱动任何通道——
        // envelope 在 0 处本就为 0，这里只额外保证掩码同样为空。
        return;
    }
    match action {
        // 点头：AngleY 下探-保持-回中；身体以 ~1/12 幅度跟随（≈1°）。
        ActionId::Nod => {
            out.owned = ParameterMask::HEAD_ANGLE_Y.union(ParameterMask::BODY_ANGLE_Y);
            let e = envelope(p, 0.30, 0.65);
            out.head_angle_y = -12.0 * g * e;
            out.body_angle_y = -(g * e);
        }
        // 摇头：AngleX 左右两个来回（2 周期正弦）× 边缘渐入渐出窗。
        ActionId::ShakeNo => {
            out.owned = ParameterMask::HEAD_ANGLE_X.union(ParameterMask::BODY_ANGLE_X);
            let w = envelope(p, 0.15, 0.85);
            let osc = (TAU * 2.0 * p).sin();
            out.head_angle_x = 14.0 * g * w * osc;
            out.body_angle_x = 1.2 * g * w * osc;
        }
        // 歪头：AngleZ 平滑侧倾-保持-回中；眼球反向微移 + 眉微抬（好奇感）。
        ActionId::Tilt => {
            out.owned = ParameterMask::HEAD_ANGLE_Z
                .union(ParameterMask::BODY_ANGLE_Z)
                .union(ParameterMask::EYE_X)
                .union(ParameterMask::BROW_Y);
            let e = envelope(p, 0.30, 0.70);
            out.head_angle_z = 10.0 * g * e;
            out.body_angle_z = 0.8 * g * e;
            out.eye_x = -0.15 * g * e;
            out.brow_y = 0.15 * g * e;
        }
        // 环视：眼球大幅左右扫视（2 周期），头部小幅同向跟随，双双回中。
        ActionId::LookAround => {
            out.owned = ParameterMask::EYE_X.union(ParameterMask::HEAD_ANGLE_X);
            let w = envelope(p, 0.12, 0.88);
            let osc = (TAU * 2.0 * p).sin();
            out.eye_x = 0.65 * g * w * osc;
            out.head_angle_x = 8.0 * g * w * osc;
        }
        // 倾听：轻前倾（标准参数无前后倾通道，用身体微倾 + 头微低 + 视线上抬近似）、
        // 歪头、眉微抬；整体带缓慢呼吸式起伏。
        ActionId::Listen => {
            out.owned = ParameterMask::HEAD_ANGLE_Z
                .union(ParameterMask::HEAD_ANGLE_Y)
                .union(ParameterMask::EYE_Y)
                .union(ParameterMask::BROW_Y)
                .union(ParameterMask::BODY_ANGLE_Z)
                .union(ParameterMask::BODY_ANGLE_Y);
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
            out.owned = ParameterMask::EYE_OPEN_SCALE
                .union(ParameterMask::BROW_Y)
                .union(ParameterMask::HEAD_ANGLE_Y)
                .union(ParameterMask::HEAD_ANGLE_Z)
                .union(ParameterMask::MOUTH_FORM);
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

// ---------------------------------------------------------------- 采样状态

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

#[cfg(test)]
mod tests;
