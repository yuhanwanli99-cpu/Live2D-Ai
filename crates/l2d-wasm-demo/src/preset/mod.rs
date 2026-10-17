//! 舞台**动作包**表 + 运行状态机（2026-09-15，原生可测；2026-09-23 v3 包重构）。
//!
//! **2026-10-17 再拆分**：按行为边界拆为 `data.rs`（类型 / 内建表）、`command.rs`（协议解析）、
//! `params.rs`（包络 / 强度 morph / 增益）；本文件只留文档头、模块装配与双槽状态机，
//! 每个文件均 ≤500 行（原先的 ≤1000 行头注豁免随之失效）。
//!
//! # 为什么要一个原生可测的模块
//!
//! 本仓库踩过两次同款坑（`mouth.rs`、`stage_bg.rs`）：纯逻辑写在 `wasm32` 才编译的
//! 文件里，`cargo test` 根本编译不到 → **没有回归**。所以预设的「id → 参数」
//! 映射、包络、时长、协议解析、强度 morph 与到点撤销全部放在这里，`wasm` 侧只负责
//! 按帧调用。
//!
//! # v3：包 = 五官 + 小幅头身；intensity 是一等公民
//!
//! - **表情包**（`smile` / `unhappy` / `surprised`，`kind="expression"`）：五官通道
//!   **加上**小幅 Angle/Body 位移（头为主、身约头的 1/3，头 ≤12、身 ≤4）。表情不再
//!   是「只有嘴和眉」——但也**不是**手势包那种大姿态（红线见 `EXPRESSION_HEAD_LIMIT`）。
//! - **手势包**（`nod` / `shake` / `look_*` / `tilt_*`，`kind="motion"`）：头 + 半身，
//!   头角可用到 ±30、身 ±10；左右方向与 intensity 正交，故左右各自保留一条。
//! - **intensity ∈ [0,3]**：普通包 = 表值 × intensity（五官 + 头身一起缩放）；
//!   **morph 包**（`PresetMorph`，目前只有 `unhappy`）= 在 low/high 两极之间按
//!   intensity 插值（1=难过相、3=生气相），intensity 不再二次相乘。
//! - **双槽**：`PresetSlot::Face` 与 `PresetSlot::Gesture` 可**同轮并存**
//!   （微笑 + 点头）。同槽换包**从换下去那一帧接上**（`handoff`），`id:"none"`
//!   按槽**放回**（`release`）到点才清；同帧写入顺序 Face → Gesture，共享头/身由手势赢。
//!
//! # P1-1：表外置（2026-09-16）
//!
//! 内建 PRESETS 仍是 **fallback**；运行时优先加载 assets/actions/presets.json
//! （经桌面端 /actions/* 静态路由取到），由 PresetTable::from_json 解析 + 校验
//! （通道白名单 / 幅值分档 / 未知通道丢通道 + warn）。改 JSON 的幅值 / 周期 / 时长
//! **不需要重编 Rust**，重开页面即生效；加载失败回退内建表并在状态栏说清。
//!
//! # 旧 id 已删除：未知 → Ignore（2026-09-23）
//!
//! v2 的旧 id（含资产 `deprecated`、内建别名表、导演旧 id 表）已**整体删除**。
//! 收到旧 id 与收到任意未知名等价：`PresetCommand::Ignore`——**不动任何参数**、不回执。
//! 产品里只认 `PRESET_IDS`（主 allowlist）与 `assets/actions/presets.json`。
//!
//! # 文件划分（AGENTS.md 行数纪律：源码 ≤500 行）
//!
//! - `mod.rs`（本文件）：模块装配 + 双槽到点撤销状态机；
//! - `data.rs`：类型 / 内建 fallback 表 / 强度 morph 描述；
//! - `command.rs`：`preset` 协议解析；
//! - `params.rs`：包络 / 强度 morph / 逐通道增益；
//! - `table.rs`：外置 JSON → 表 + 校验；
//! - `handoff.rs`：同槽换动作时的接力插值（2026-10-15）；
//! - `release.rs`：`Revoke` 的放回（2026-10-15）；
//! - `scales.rs`：通道红线、用户可调倍率、最终下发；
//! - `tests/`：协议（`mod.rs` 内联）+ 双槽 / morph（`packs.rs`）+ 资产（`assets.rs`）。
//!
//! # 通道（严格按任务书）
//!
//! 只做标准皮套的两条常见通道：
//! - **表情**（expression）：进 / 保持 / 出与手势同一条起落曲线；
//! - **短动作**（motion）：带包络的短动作（`nod` / `shake` / `look_*` / `tilt_*`）。
//!
//! **不做**手臂 / 手指 / 特效 / 第二路 LLM / motion3 / exp3。缺参数的皮套由
//! 渲染层静默降级（未知参数 `override_parameter` 返回 `false`，不报错）。
//!
//! # 半身随动 + 起落曲线（2026-09-16 L1 / 2026-10-14 裁决）
//!
//! 手势包一律头 + 身同向，主轴**表内身/头 = 0.325**（设计口径 [0.30, 0.50]；乘出厂
//! 倍率 0.80/0.75 后 ≈ 0.347）；摇头用 `MotionWave::Oscillate`（`sin(2*pi*cycles*p)`，
//! >=2 个完整来回）叠加同一个起落曲线，**首末仍为 0**。起落不再用单峰 `sin(pi*p)`：
//! 改成 `motion_curve` 的**攻击（0 → 0.22）→ 保持（→ 0.62）→ 放回（→ 1）**三段
//! smootherstep，保持段恰好 1、两端值与斜率都是 0——**表情与手势同一条曲线**；
//! `Single` 的幅值乘子就是它（不再等于 `envelope`）。**身体滞后**：`ParamBodyAngle*`
//! 走 `body_p = (p - 0.12) / 0.88`，身比头晚起步、同一帧归零。幅值 / 通道钳位不动，
//! `envelope()` 留着（「两端恰好 0」的旧回归继续测它）。
//!
//! # 幅值标定（2026-09-24，W2）
//!
//! 手势主轴表值头 12.0 / 身 3.9（峰值系数：2026-10-14 起起落曲线带保持段，
//! Single 与 shake 的 Oscillate `cycles=2` **都是 1.0**；旧的 `sin(pi*p)` 时代
//! shake 是 0.9285），使「出厂组合」「scale 走满」「intensity 走满」三条各自
//! 都留在上限 ×0.95 以内；数值账与「两个旋钮同时拉满会钳位的组合」见
//! `scales.rs` 顶部注释（那里是标定真源，本表是它的逐值副本）。
//!
//! # P0-2：预设写 `final_override` 层，不再写 input
//!
//! 旧实现把预设写进 `input` 层，而待机微表情（`idle.rs`）在**同一帧更晚**写同一个
//! input 层的 `ParamMouthForm` / `ParamBrowLY` / `ParamBrowRY` → 表情被 idle 盖掉。
//! 现在预设经 `PresetSink` 写 `final_override`（最高优先级），到点用
//! `clear_override_parameter` **整批撤销**。口型 `ParamMouthOpenY` 仍走 input。
//!
//! # P0-1：`preset` 协议
//!
//! 旧形状 `{"id": "smile"}` 继续可用（缺省 = 该预设时长、强度 1、来源
//! `unknown`）。新增：
//! - `id:"none"` = 撤销：每个还有姿势的槽**从屏幕值放回** 0（见 `release`）；
//! - 可选 `intensity`（钳 `[0, 3]`）、`ttl_ms`（钳 `(0, 5000]`）、
//!   `source`（`debug` / `director` / 其它 → `unknown`）。
//!
//! 未知 id **仍静默忽略**（不报错、不改任何参数）。解析逻辑是纯函数
//! `parse_command`，原生 `cargo test` 直接回归。
mod command;
mod data;
mod params;

use command::parse_command_with;
pub use command::*;
pub use data::*;
pub use params::*;

mod handoff;
mod motion_curve;
mod release;
mod scales;
pub use handoff::Handoff;
pub use release::Release;
// 这些名字大多给 table.rs / 测试用（cfg(test) 下才被「用到」），故放宽 unused。
#[allow(unused_imports)]
pub use scales::{
    BODY_LIMIT, EXPRESSION_BODY_LIMIT, EXPRESSION_HEAD_LIMIT, EXPRESSION_LIMIT, HEAD_LIMIT,
    MAX_SCALE, MIN_SCALE, PresetScales, ScaleClass, clamp_scale, clamp_to_channel,
    frame_params_scaled_by, scale_class,
};

// 阶段4c：三字段（body / head / expression）的映射表 + 运行状态机 + 音频时钟 + ack。
// 与旧的 `preset_id` 双槽（`PresetRuntime`）**并存**：旧路径一字不删（V11）。
mod field_clock;
mod field_map;
mod field_runtime;
#[allow(unused_imports)]
pub use field_clock::{ClockDomain, ClockState, StageClock};
#[allow(unused_imports)]
pub use field_map::{FACIAL_PARAMS, Field, FieldMap, field_allows, model_id_from_url};
#[allow(unused_imports)]
pub use field_runtime::{
    AckEvent, AckKind, Anchor, DEFAULT_TTL_EXPRESSION_MS, FieldCue, FieldRuntime, FrameOutcome,
    FrameWrite, default_ttl_ms, dropped_for_bad_cue,
};

/// 一条预设用到的**全部通道**（低 / 高两极并集；去重、稳定顺序）。
pub fn spec_channels(spec: &PresetSpec) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (id, _) in spec.params {
        if !out.contains(id) {
            out.push(id);
        }
    }
    if let Some(m) = spec.morph {
        for (id, _) in m.high_params {
            if !out.contains(id) {
                out.push(id);
            }
        }
    }
    out
}

mod table;
#[allow(unused_imports)]
pub use table::{ALLOWED_PARAMS, PresetTable, amplitude_limit, pack_limit};

/// 预设的**唯一写入目标**：只暴露 `final_override` 层的写 / 清（P0-2）。
pub trait PresetSink {
    /// 写 `final_override`；模型缺该参数返回 `false`（静默降级）。
    fn override_param(&mut self, id: &str, value: f32) -> bool;
    /// 清 `final_override`。
    fn clear_override_param(&mut self, id: &str) -> bool;
}

/// 清掉**一条**预设用到的 override 参数（到点按槽撤销；换包改走 `handoff` 接力）。
pub fn clear_spec_overrides(spec: &PresetSpec, sink: &mut impl PresetSink) {
    for id in spec_channels(spec) {
        sink.clear_override_param(id);
    }
}

/// 当前活动的预设（跨帧状态）。
#[derive(Debug, Clone, PartialEq)]
pub struct ActivePreset {
    pub spec: &'static PresetSpec,
    /// 起始时刻（`performance.now()` 毫秒，与 rAF 的 dt 同源）。
    pub started_ms: f64,
    /// 生效时长（毫秒，已钳位）。
    pub ttl_ms: f64,
    /// 强度（已钳位）。
    pub intensity: f32,
    pub source: PresetSource,
    /// 同槽换包时的「换下去那一帧」快照（见 `handoff`）；无接力 = `None`。
    pub handoff: Option<Handoff>,
    /// `Revoke` 的放回状态（见 `release`）；非放回 = `None`。
    pub release: Option<Release>,
}

/// 预设运行状态机（**纯逻辑**，原生可测）：表情槽 + 手势槽。
#[derive(Debug, Clone)]
pub struct PresetRuntime {
    /// 预设表（内建或外置 JSON；见 PresetTable）。
    table: PresetTable,
    /// 表情槽（kind=expression）。
    face: Option<ActivePreset>,
    /// 手势槽（kind=motion）。
    gesture: Option<ActivePreset>,
    /// 用户可调的幅度倍率（初始 = 出厂默认 0.75/1.4/1.0）。
    scales: PresetScales,
}

impl Default for PresetRuntime {
    fn default() -> Self {
        Self {
            table: PresetTable::builtin(),
            face: None,
            gesture: None,
            scales: PresetScales::PRODUCT_DEFAULT,
        }
    }
}

impl PresetRuntime {
    /// 最近触发的活动预设（兼容旧 API）：优先表情槽，其次手势槽。
    ///
    /// HUD / 调试面板要看**两个槽**时用 `Self::active_face` / `Self::active_gesture`。
    pub fn active(&self) -> Option<&ActivePreset> {
        self.face.as_ref().or(self.gesture.as_ref())
    }

    /// 表情槽的当前预设。
    pub fn active_face(&self) -> Option<&ActivePreset> {
        self.face.as_ref()
    }

    /// 手势槽的当前预设。
    pub fn active_gesture(&self) -> Option<&ActivePreset> {
        self.gesture.as_ref()
    }

    /// 手势槽里是否**有活动预设**（表情槽不算）——待机摆头让开的判据。
    ///
    /// 到点撤销在 `apply_frame` 里先行，所以每帧 `apply_frame` 之后取这个布尔，
    /// 拿到的是「本帧还活着的手势」。表情不占手势槽（脸不该压住摆头）。
    pub fn gesture_occupied(&self) -> bool {
        self.gesture.is_some()
    }

    /// 某个槽的当前预设。
    pub fn active_in(&self, slot: PresetSlot) -> Option<&ActivePreset> {
        self.slot_ref(slot)
    }

    /// 当前生效的幅度倍率。
    pub fn scales(&self) -> PresetScales {
        self.scales
    }

    /// 设置幅度倍率（消息入口调用；内部再钳一次，防非法值）。
    pub fn set_scales(&mut self, scales: PresetScales) {
        self.scales = scales.clamped();
    }

    /// 替换预设表（启动时加载外置 JSON 后调用；失败回退内建则不调用）。
    pub fn set_table(&mut self, table: PresetTable) {
        self.table = table;
    }

    /// 当前表（诊断 / 启动日志用）。
    #[allow(dead_code)]
    pub fn table(&self) -> &PresetTable {
        &self.table
    }

    fn slot_ref(&self, slot: PresetSlot) -> Option<&ActivePreset> {
        match slot {
            PresetSlot::Face => self.face.as_ref(),
            PresetSlot::Gesture => self.gesture.as_ref(),
        }
    }

    fn slot_mut(&mut self, slot: PresetSlot) -> &mut Option<ActivePreset> {
        match slot {
            PresetSlot::Face => &mut self.face,
            PresetSlot::Gesture => &mut self.gesture,
        }
    }

    /// 用**本运行时的表**解析一条 preset 指令（未知 id → `Ignore`）。
    pub fn resolve(
        &self,
        id: Option<&str>,
        intensity: Option<f64>,
        ttl_ms: Option<f64>,
        source: Option<&str>,
    ) -> PresetCommand {
        parse_command_with(
            |id| self.table.get(id).map(|spec| (spec, None)),
            id,
            intensity,
            ttl_ms,
            source,
        )
    }

    /// 剩余毫秒（两个槽里**最晚结束**的那个；都没有 → `None`）。
    pub fn remaining_ms(&self, now_ms: f64) -> Option<f64> {
        [self.face.as_ref(), self.gesture.as_ref()]
            .into_iter()
            .flatten()
            .map(|a| (a.ttl_ms - (now_ms - a.started_ms)).max(0.0))
            .reduce(f64::max)
    }

    /// 处理一条指令；返回值 = **是否值得回执**（`applied`）。
    ///
    /// - `PresetCommand::Revoke`：每个还有姿势的槽从**屏幕值**放回 0（见 `release`）；
    /// - `PresetCommand::Apply`：空槽 / 同一 id 照旧；同一槽换**不同 id** 时不先
    ///   归零，把「换下去那一帧」记在新预设上，由下一帧 `Self::apply_frame` 接力；
    /// - `PresetCommand::Ignore`：什么都不做、不回执（未知 id 静默降级）。
    pub fn handle(&mut self, cmd: PresetCommand, now_ms: f64, _sink: &mut impl PresetSink) -> bool {
        match cmd {
            PresetCommand::Ignore => false,
            PresetCommand::Revoke => {
                // **不从当帧清参数**：每个还有姿势的槽从屏幕上这一帧的值收到 0，到点清槽。
                let scales = self.scales;
                for slot in [PresetSlot::Face, PresetSlot::Gesture] {
                    if let Some(active) = self.slot_mut(slot) {
                        let from = handoff::on_screen(active, now_ms, scales);
                        active.release = Some(Release::new(from, active.ttl_ms, now_ms));
                    }
                }
                true
            }
            PresetCommand::Apply {
                spec,
                intensity,
                ttl_ms,
                source,
            } => {
                let slot = slot_of(spec.kind);
                // 同一槽换下一个动作：不先归零。把「换下去那一帧」记在新预设上，
                // 由 `apply_frame` 在攻击段内插值接上（空槽 / 同一 id 不接力）。
                let scales = self.scales;
                let handoff = self
                    .slot_ref(slot)
                    .filter(|old| old.spec.id != spec.id)
                    .map(|old| Handoff::capture(old, now_ms, scales));
                *self.slot_mut(slot) = Some(ActivePreset {
                    spec,
                    started_ms: now_ms,
                    ttl_ms,
                    intensity,
                    source,
                    handoff,
                    release: None,
                });
                true
            }
        }
    }

    /// 每帧把两个槽写进 `final_override`；到点 / 放回到点按槽撤销并清空该槽。
    ///
    /// 同帧写入顺序 Face → Gesture：共享的头 / 身通道由手势包赢（否则点头会被
    /// 表情包的小幅抬头盖掉）。
    pub fn apply_frame(&mut self, now_ms: f64, sink: &mut impl PresetSink) {
        for slot in [PresetSlot::Face, PresetSlot::Gesture] {
            let expired = self.slot_ref(slot).is_some_and(|a| {
                a.release.is_none() && (now_ms - a.started_ms).max(0.0) >= a.ttl_ms
            });
            if expired {
                if let Some(old) = self.slot_ref(slot) {
                    clear_spec_overrides(old.spec, sink);
                }
                *self.slot_mut(slot) = None;
            }
        }
        let scales = self.scales;
        for slot in [PresetSlot::Face, PresetSlot::Gesture] {
            let Some(active) = self.slot_ref(slot).cloned() else {
                continue;
            };
            if let Some(release) = &active.release {
                // 放回：每帧从这一帧屏幕上的值按 smootherstep 收到 0，到点清槽。
                if release.write(now_ms, sink) {
                    *self.slot_mut(slot) = None;
                }
                continue;
            }
            let elapsed = (now_ms - active.started_ms).max(0.0);
            let progress = progress_at(elapsed, active.ttl_ms);
            let finished = match active.handoff.as_ref() {
                Some(handoff) => {
                    handoff.write(active.spec, progress, active.intensity, scales, sink)
                }
                None => {
                    for (id, value) in
                        frame_params_scaled_by(active.spec, progress, active.intensity, scales)
                    {
                        sink.override_param(id, value);
                    }
                    true
                }
            };
            // 攻击段结束：丢掉旧帧快照，此后帧值与不接旧值时逐值相同。
            if finished && let Some(cur) = self.slot_mut(slot) {
                cur.handoff = None;
            }
        }
    }
}
#[cfg(test)]
mod tests;
