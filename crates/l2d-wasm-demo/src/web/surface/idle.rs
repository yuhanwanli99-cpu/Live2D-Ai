//! 待机生命体征（RM6，仅 `target_arch = "wasm32"` 下编译）：呼吸 / 眨眼 / 微表情。
//!
//! 与动作系统是**两套机制**：动作层已整体删除（rc.2），本模块是产品路径上一直在跑的
//! 待机体征，**必须保留**。每帧由 [`super::input::apply_bridge_effects`] 调用
//! [`apply_idle_life`]，经 `set_parameter` 写 input 层。

use super::render::FrameState;

/// RM6 待机生命体征层（idle-breath / idle-blink / idle-micro-expr）。
///
/// 照抄 py 版 `idle-breath.ts` / `idle-blink.ts` / `idle-micro-expr.ts`（MIT），
/// 写入 input 层 `set_parameter`。2026-09-12（rc.2）起其上不再有动作
/// `final_override` 层（动作已整体删除），breath/blink 参数
/// （ParamBreath / ParamEyeLOpen / ParamEyeROpen）仍然只由这里驱动。
///
/// 状态机与 py 版同构：
/// - **呼吸**：周期 [3.1, 5]s 随机锁定，正弦 `0.5 + 0.15*sin(2π·t/period)`。
/// - **眨眼**：间隔 [3000, 7500]ms 随机触发；闭眼 75ms 线性 1→0；睁眼
///   [150,300]ms 随机线性 0→1。状态机：Open → Closing → Opening → Open。
/// - **微表情**：间隔 [3,6]s 随机触发；随机选一个参数 ±0.05，400ms fade。
///
/// 与 py 版差异（v1 简化）：
/// - py 版会在动作播放期间**挂起** idle（surprise 用 override 层写 EyeLOpen/ROpen
///   时，idle blink 被挂起避免冲突）；本版**不挂起**——动作 override 层盖住同名
///   参数即可，呼吸/眨眼参数（ParamBreath/EyeL/R）动作不动它们，天然不打架。
///   详见 `apply_idle_life`。
///
/// dt 来源：`apply_bridge_effects(state, dt_millis)` 已有 dt（R4 加的），
/// idle 采样用它，帧率无关。
///
/// RNG：wasm 环境无 `rand` crate（体积与 Send 约束），手写 xorshift64。
/// 种子用启动时 `performance.now() as u64`——每次加载不同，待机天然随机。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlinkPhase {
    /// 睁眼待机：不写参数（保持 1.0），等待随机间隔。
    Open,
    /// 闭眼中：线性 1→0，固定 75ms。
    Closing,
    /// 睁眼中：线性 0→1，随机 [150,300]ms。
    Opening,
}

/// idle 生命体征采样器状态（储存在 FrameState，跨帧持续）。
///
/// xorshift64 状态 `rng_state` 永不为 0（见 `rng_next` 种子保证）。
pub(crate) struct IdleState {
    /// 呼吸周期 ms（随机锁定 [3100, 5000]）。
    breath_period_ms: f64,
    /// 呼吸相位起点（performance.now() 毫秒）。
    breath_t0_ms: f64,
    /// 下次眨眼触发时刻（now + rand[3000,7500]）。
    blink_next_ms: f64,
    /// 眨眼当前相位。
    blink_phase: BlinkPhase,
    /// 眨眼当前相位已推进 ms。
    blink_phase_ms: f64,
    /// 单次闭眼固定时长 ms（py 版 75ms 定步长）。
    blink_close_ms: f64,
    /// 本次睁眼总时长 ms（随机 [150,300]）。
    blink_open_ms: f64,
    /// 下次微表情触发时刻（now + rand[3000,6000]）。
    micro_next_ms: f64,
    /// 进行中的微表情：(参数 ID, 目标 delta, 开始时刻 ms)。
    /// fade 进 1/3 → 停 1/3 → fade 出 1/3，共 400ms。
    micro_active: Option<(String, f32, f64)>,
    /// xorshift64 状态（永不为 0）。
    rng_state: u64,
}

impl IdleState {
    /// 用当前时间作为种子，随机初始化呼吸周期 / 眨眼间隔 / 微表情间隔。
    pub(crate) fn new(now_ms: f64) -> Self {
        // xorshift64 种子：performance.now() 微秒部分 + 恒定盐，保证非零。
        let mut s = (now_ms as u64).wrapping_mul(0x9E3779B97F4A6295) | 1;
        let breath_period_ms = rng_range_f64(&mut s, 3100.0, 5000.0);
        let blink_next_ms = now_ms + rng_range_f64(&mut s, 3000.0, 7500.0);
        let micro_next_ms = now_ms + rng_range_f64(&mut s, 3000.0, 6000.0);
        let blink_open_ms = rng_range_f64(&mut s, 150.0, 300.0);
        Self {
            breath_period_ms,
            breath_t0_ms: now_ms,
            blink_next_ms,
            blink_phase: BlinkPhase::Open,
            blink_phase_ms: 0.0,
            blink_close_ms: 75.0,
            blink_open_ms,
            micro_next_ms,
            micro_active: None,
            rng_state: s,
        }
    }
}

/// xorshift64 伪随机（wasm 无 rand crate，手写）。
/// 三次异或移位，状态永不为 0（`new` 用 `| 1` 保证）。
#[inline]
fn rng_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// 返回 `[min, max)` 区间内的随机 f64。
#[inline]
fn rng_range_f64(state: &mut u64, min: f64, max: f64) -> f64 {
    let span = max - min;
    // 取 state 高 53 位（u64 → [0,1)）以保证精度。
    let r = (rng_next(state) >> 11) as f64 / ((1u64 << 53) as f64);
    min + r * span
}

/// 眨眼闭眼/睁眼线性插值因子（0..=1）。
#[inline]
fn blink_factor(phase_ms: f64, duration_ms: f64) -> f64 {
    (phase_ms / duration_ms).clamp(0.0, 1.0)
}

/// RM6 待机生命体征采样：呼吸 / 眨眼 / 微表情，每帧调用一次。
///
/// 写入 input 层 `set_parameter`。
/// 公式与 py 版 `idle-breath.ts` / `idle-blink.ts` / `idle-micro-expr.ts`（MIT）同构。
///
/// - **呼吸**：`0.5 + 0.15*sin(2π*(now-t0)/period)` → ParamBreath，永不打断。
/// - **眨眼**：状态机推进（dt 归一化）；Closing 线性 1→0（75ms）；
///   Opening 线性 0→1（随机 150-300ms）。写入 ParamEyeLOpen/ROpen。
/// - **微表情**：到触发时刻随机选参 ±0.05，400ms fade（进 1/3 → 停 1/3 → 出 1/3）。
///   `base` 为对应参数中性值（嘴形 0.5，眉/眼球 0.0）。
///
/// 与 py 版差异：py 版微表情触发时对**所有** MICRO_PARAMS 同时写入一个 batch；
/// 本版**随机选一个**参数做一次性小偏移（更自然、减轻写入），fade 仍 400ms。
/// 详见 IdleState 文档注释。
pub(crate) fn apply_idle_life(st: &mut FrameState, now_ms: f64, dt_ms: f64) {
    // ---- 呼吸：正弦波，永不打断 ----
    // value = 0.5 + 0.15 * sin(2π * (now - t0) / period) ∈ [0.35, 0.65]
    let breath = {
        let phase = (now_ms - st.idle.breath_t0_ms) / st.idle.breath_period_ms;
        0.5 + 0.15 * (2.0 * std::f64::consts::PI * phase).sin()
    };
    let _ = st.core.set_parameter("ParamBreath", breath as f32);

    // ---- 眨眼：状态机 ----
    // 推进当前相位计时；到时切换。Closing 线性 1→0；Opening 线性 0→1。
    st.idle.blink_phase_ms += dt_ms;
    match st.idle.blink_phase {
        BlinkPhase::Open => {
            // 睁眼待机：不写参数（保持 1.0），等待随机间隔。
            if now_ms >= st.idle.blink_next_ms {
                st.idle.blink_phase = BlinkPhase::Closing;
                st.idle.blink_phase_ms = 0.0;
                // 首帧写入 1.0（开始下降）。
                let _ = st.core.set_parameter("ParamEyeLOpen", 1.0);
                let _ = st.core.set_parameter("ParamEyeROpen", 1.0);
            }
        }
        BlinkPhase::Closing => {
            // 线性 1→0，固定 75ms。
            let f = (1.0 - blink_factor(st.idle.blink_phase_ms, st.idle.blink_close_ms)) as f32;
            let _ = st.core.set_parameter("ParamEyeLOpen", f);
            let _ = st.core.set_parameter("ParamEyeROpen", f);
            if st.idle.blink_phase_ms >= st.idle.blink_close_ms {
                // 闭眼到位 → 进入睁眼，随机 [150,300]ms。
                st.idle.blink_phase = BlinkPhase::Opening;
                st.idle.blink_phase_ms = 0.0;
                st.idle.blink_open_ms = rng_range_f64(&mut st.idle.rng_state, 150.0, 300.0);
            }
        }
        BlinkPhase::Opening => {
            // 线性 0→1，随机时长。
            let f = blink_factor(st.idle.blink_phase_ms, st.idle.blink_open_ms) as f32;
            let _ = st.core.set_parameter("ParamEyeLOpen", f);
            let _ = st.core.set_parameter("ParamEyeROpen", f);
            if st.idle.blink_phase_ms >= st.idle.blink_open_ms {
                // 睁眼完成 → 回到 Open，单次写回 1.0。
                let _ = st.core.set_parameter("ParamEyeLOpen", 1.0);
                let _ = st.core.set_parameter("ParamEyeROpen", 1.0);
                st.idle.blink_phase = BlinkPhase::Open;
                st.idle.blink_phase_ms = 0.0;
                // 下一次间隔：[3000, 7500]ms。
                st.idle.blink_next_ms =
                    now_ms + rng_range_f64(&mut st.idle.rng_state, 3000.0, 7500.0);
            }
        }
    }

    // ---- 微表情：间隔触发，随机参数 ±0.05，400ms fade ----
    // fade 三段：推进 1/3（0→delta）→ 停 1/3（持 delta）→ 缩退 1/3（delta→0）。
    // end_t = start_ms + 400ms；过时清除（恢复 base）。
    const MICRO_FADE_MS: f64 = 400.0;
    let micro_done = if let Some((param, delta, start_ms)) = &st.idle.micro_active {
        let elapsed = now_ms - *start_ms;
        if elapsed >= MICRO_FADE_MS {
            // 结束：清参数回 base（input 层 set base 值），清活跃。
            let base = micro_param_base(param);
            let _ = st.core.set_parameter(param, base as f32);
            st.idle.micro_active = None;
            // 排下一次：[3000, 6000]ms。
            st.idle.micro_next_ms = now_ms + rng_range_f64(&mut st.idle.rng_state, 3000.0, 6000.0);
            true
        } else if elapsed >= MICRO_FADE_MS * 2.0 / 3.0 {
            // 阶段 3：缩退 delta→0，线性。
            let s = (1.0 - (elapsed - MICRO_FADE_MS * 2.0 / 3.0) / (MICRO_FADE_MS / 3.0))
                .clamp(0.0, 1.0);
            let v = (micro_param_base(param) + *delta as f64 * s) as f32;
            let _ = st.core.set_parameter(param, v);
            true
        } else if elapsed >= MICRO_FADE_MS / 3.0 {
            // 阶段 2：停（持 delta 峰值）。
            let v = (micro_param_base(param) + *delta as f64) as f32;
            let _ = st.core.set_parameter(param, v);
            false
        } else {
            // 阶段 1：推进 0→delta。
            let s = (elapsed / (MICRO_FADE_MS / 3.0)).clamp(0.0, 1.0);
            let v = (micro_param_base(param) + *delta as f64 * s) as f32;
            let _ = st.core.set_parameter(param, v);
            false
        }
    } else {
        false
    };
    // 只有活跃微表情已结束（或从未启动），才检查是否触发新的。
    if !micro_done && now_ms >= st.idle.micro_next_ms {
        // 随机选一个参数 + 随机 ±0.05 delta。
        let params = [
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamMouthForm",
            "ParamEyeBallX",
            "ParamEyeBallY",
        ];
        let idx = (rng_next(&mut st.idle.rng_state) % params.len() as u64) as usize;
        let param = params[idx].to_string();
        let delta = (rng_range_f64(&mut st.idle.rng_state, 0.0, 1.0) - 0.5) * 0.1; // ±0.05
        let start_ms = now_ms;
        // 立即写初始值（base + 0），fade-in 逐渐推进。
        let _ = st
            .core
            .set_parameter(&param, micro_param_base(&param) as f32);
        st.idle.micro_active = Some((param, delta as f32, start_ms));
    }
}

/// 微表情参数的中性基线值（对应 M3 参数表 neutral）。
/// 嘴形 0.5，眉毛/眼球 0.0。
fn micro_param_base(param: &str) -> f64 {
    match param {
        "ParamMouthForm" => 0.5,
        _ => 0.0,
    }
}

/// HUD 用的待机生命体征快照：呼吸正弦值 + 眨眼相位回路。
///
/// 由 `render::write_hud_if_due` 每 500ms 取一次，字符串与 rc.2 内联版本逐字符一致。
pub(crate) fn diag_line(st: &IdleState, now_ms: f64) -> String {
    let breath = {
        let phase = (now_ms - st.breath_t0_ms) / st.breath_period_ms;
        0.5 + 0.15 * (2.0 * std::f64::consts::PI * phase).sin()
    };
    let blink_phase_str = match st.blink_phase {
        BlinkPhase::Open => "O",
        BlinkPhase::Closing => "C",
        BlinkPhase::Opening => "P",
    };
    format!(
        "idle: b{breath:.2} bl{blink_phase_str}{:.0}",
        st.blink_phase_ms
    )
}
