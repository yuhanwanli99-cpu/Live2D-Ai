//! 三族表演字段的**运行状态机** + 音频时钟 + 事件级 ack（阶段4c；协议 §4 / §6 / §7）。
//!
//! **行数豁免（≤1000 头注）**：本文件当前 953 行，超过 AGENTS「源码 ≤500」的常规线。
//! 豁免理由是**语义不可拆**：三段核心状态（add/replace/hold/ttl 的贡献累加、跨批
//! replace、ack 的 clamped/degraded 归因）共享同一份「贡献清单」，其中
//! 「**先加后钳**」（C3）要求累加与钳位在同一函数内可读——拆到两个文件会把这条
//! 不变量的两半隔开，正是 D14 禁止的「为凑字面而拆」。音频时钟已单独拆到
//! `field_clock.rs`；本文件保持在 1000 行以内（AGENTS 的豁免上限）。
//!
//! # 为什么是纯逻辑
//!
//! 本仓反复踩过同一个坑：把纯逻辑写在 wasm 门控后面 = 原生 `cargo test` 编译不到 =
//! 没有回归（`mouth.rs` / `stage_bg.rs`）。字段的 add / replace / hold / ttl /
//! 锚点 / 时钟 / ack 全部住在这里，wasm 侧只做「收消息 → 调这里 → 发事件」。
//!
//! # 语义（协议冻结）
//!
//! - **同类 add**：同一批（同一 `epoch`）内、同一 field 的多条 cue 相加，
//!   最后**统一钳位**（先加后钳，§4.3 / C3）。
//! - **跨批 replace（D26）**：新 `epoch` 到达 → 结束旧动画段（发 `preset-replaced`）
//!   并以新值为当前值。
//! - **hold**：`hold=true` 保持到下次指令，**不**到点回基准、**不**产生
//!   `preset-expired`（V5）；`hold=false` 按 `ttl_ms` 到点移除自己的贡献。
//! - **clock**：`stage-clock {seg,pos_ms,playing}` 到位后用音频时间轴当 `now_ms`；
//!   没有 clock（段 A）回落墙钟（§6.1 / §6.2 / O6）。
//! - **ack**：`preset-applied` / `preset-replaced` / `preset-expired` /
//!   `preset-dropped` + `segment-ended`，wire 名由 O13 冻结，不得改名。
//! - **缺参数不得静默**：`PresetSink::override_param` 返回 `false` → 发
//!   `preset-dropped`（或 `preset-applied` + `degraded=true`，§7.1 / §10 #9）。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::field_clock::{ClockDomain, ClockState};
use super::field_map::{Field, FieldMap, field_allows};
use super::{PresetScales, PresetSink, REVOKE_ID, clamp_to_channel};

/// `body` / `head` 的默认 ttl（= 现有手势包时长，O3 / §2.5）。
pub const DEFAULT_TTL_BODY_MS: f64 = 900.0;
/// `head` 默认 ttl（O3）。
pub const DEFAULT_TTL_HEAD_MS: f64 = 900.0;
/// `expression` 默认 ttl（= 现有表情包时长，O3 / §2.5）。
pub const DEFAULT_TTL_EXPRESSION_MS: f64 = 2_600.0;
/// cue 强度上限（协议 §2.2：1..=3，越界钳位）。
pub const MAX_FIELD_INTENSITY: f32 = 3.0;
/// cue 强度下限（§2.2：越界钳位到 1..=3）。
pub const MIN_FIELD_INTENSITY: f32 = 1.0;
/// `ttl_ms` 上限（沿用 P0-1）。
pub const MAX_FIELD_TTL_MS: f64 = 5_000.0;

/// 某字段省略 `ttl_ms` 时的默认时长（O3）。
pub const fn default_ttl_ms(field: Field) -> f64 {
    match field {
        Field::Body => DEFAULT_TTL_BODY_MS,
        Field::Head => DEFAULT_TTL_HEAD_MS,
        Field::Expression => DEFAULT_TTL_EXPRESSION_MS,
    }
}

/// cue 的生效锚点（V6 / §6.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// 立即生效（段 A = 墙钟；段 B = 音频时钟当前位置）。
    Now,
    /// 第 N 段音频开始播放（1-based）。
    Seg(u32),
    /// 上一条 cue 的动作做完（事件式）；上一条是 hold → 退化为 `Now`（O4）。
    AfterPrev,
}

impl Anchor {
    /// 解析 `now` / `seg:N` / `after_prev`；非法 → `None`（调用方丢弃 + 发 dropped）。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "now" => Some(Anchor::Now),
            "after_prev" => Some(Anchor::AfterPrev),
            other => other
                .strip_prefix("seg:")
                .and_then(|n| n.trim().parse::<u32>().ok())
                .filter(|n| *n >= 1)
                .map(Anchor::Seg),
        }
    }
}

/// 一条字段 cue（协议 §2.2 + 编排者冻结的 `preset` payload 只增键）。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldCue {
    /// cue 序号（同一 plan 内 1 起；缺省 0）。
    pub seq: u32,
    /// 批次代号（= 新一轮 / 新 plan；同 epoch 内 add，跨 epoch replace）。
    pub epoch: i64,
    /// 是否显式给了 `epoch`。
    pub has_epoch: bool,
    pub field: Field,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    /// `expression` 的面板 id（`none` = 撤销哨兵）。
    pub id: Option<String>,
    /// 已钳到 `1..=3`。
    pub intensity: f32,
    pub at: Anchor,
    pub hold: bool,
    /// 已钳到 `(0, 5000]`（hold 时被忽略）。
    pub ttl_ms: f64,
}

impl FieldCue {
    /// 从 `preset` payload（或 `action_cue` 单条）解析。
    ///
    /// 宽容处（不失败）：`intensity` 越界钳位、轴越界钳位、`body`/`expression` 的
    /// `z` 丢弃、`hold` 缺省 false、`ttl_ms` 缺省按字段、`at` 缺省 `now`。
    /// 失败处（`Err`，由调用方转 `preset-dropped`，**不静默**）：缺 / 词表外 `field`、
    /// `at` 非法（含 `seg:0`）。
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let field_raw = v.get("field").and_then(Value::as_str).unwrap_or("");
        let field = Field::parse(field_raw).ok_or_else(|| format!("bad_field:{field_raw}"))?;
        let at = match v.get("at").and_then(Value::as_str) {
            Some(raw) => Anchor::parse(raw).ok_or_else(|| format!("bad_anchor:{raw}"))?,
            None => match v.get("sentence_seq").and_then(Value::as_u64) {
                // V11：sentence_seq（1-based）与 at:"seg:N" 同义。
                Some(n) if n >= 1 => Anchor::Seg(n as u32),
                _ => Anchor::Now,
            },
        };
        let axis = |key: &str| -> Option<f32> {
            v.get(key)
                .and_then(Value::as_f64)
                .filter(|x| x.is_finite())
                .map(|x| (x as f32).clamp(-1.0, 1.0))
        };
        // §2.4 #11：body / expression 给 z → 丢该键（宽容，不整份失败）。
        let z = if field == Field::Head {
            axis("z")
        } else {
            None
        };
        let intensity = v
            .get("intensity")
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite())
            .map_or(1.0, |x| {
                (x as f32).clamp(MIN_FIELD_INTENSITY, MAX_FIELD_INTENSITY)
            });
        let hold = v.get("hold").and_then(Value::as_bool).unwrap_or(false);
        let ttl_ms = v
            .get("ttl_ms")
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite() && *x > 0.0)
            .map_or_else(|| default_ttl_ms(field), |x| x.min(MAX_FIELD_TTL_MS));
        let epoch = v.get("epoch").and_then(Value::as_i64);
        Ok(Self {
            seq: v.get("seq").and_then(Value::as_u64).unwrap_or(0) as u32,
            epoch: epoch.unwrap_or(0),
            has_epoch: epoch.is_some(),
            field,
            x: axis("x"),
            y: axis("y"),
            z,
            id: v
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            intensity,
            at,
            hold,
            ttl_ms,
        })
    }
}

/// ack 事件类型（O13 冻结 wire 名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckKind {
    /// cue 生效。
    Applied,
    /// 同场被新 cue 顶掉（跨批 replace）。
    Replaced,
    /// 非 hold cue 到点。
    Expired,
    /// 皮套缺该参数（或 cue 无法落地）。
    Dropped,
    /// 某段音频播完。
    SegmentEnded,
}

impl AckKind {
    /// 冻结的 wire 名（O13，**不得改名**）。
    pub const fn wire_type(self) -> &'static str {
        match self {
            AckKind::Applied => "preset-applied",
            AckKind::Replaced => "preset-replaced",
            AckKind::Expired => "preset-expired",
            AckKind::Dropped => "preset-dropped",
            AckKind::SegmentEnded => "segment-ended",
        }
    }
}

/// 一条事件级 ack（协议 §7.2 payload）。
#[derive(Debug, Clone, PartialEq)]
pub struct AckEvent {
    pub kind: AckKind,
    pub epoch: i64,
    pub ts_ms: f64,
    pub seq: u32,
    pub field: Option<Field>,
    pub id: Option<String>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub intensity: f32,
    pub clamped: bool,
    pub degraded: bool,
    pub reason: Option<String>,
    pub seg: Option<u32>,
}

/// 构造「来自生效贡献」的 ack 的上下文（kind + 轮次 + 时间 + 表 + 倍率）。
///
/// 收成一个结构体而不是九个参数：字段通道判据（表）与最终轴值换算（倍率）
/// 是同一件事的两半，打包后调用点也更难把 epoch / ts_ms 传反。
#[derive(Clone, Copy)]
struct AckCtx<'a> {
    kind: AckKind,
    epoch: i64,
    ts_ms: f64,
    map: &'a FieldMap,
    scales: PresetScales,
}

impl AckEvent {
    fn base(kind: AckKind, epoch: i64, ts_ms: f64) -> Self {
        Self {
            kind,
            epoch,
            ts_ms,
            seq: 0,
            field: None,
            id: None,
            x: None,
            y: None,
            z: None,
            intensity: 1.0,
            clamped: false,
            degraded: false,
            reason: None,
            seg: None,
        }
    }

    /// `segment-ended`（§7.2 只有 `seg`）。
    pub fn segment_ended(epoch: i64, ts_ms: f64, seg: u32) -> Self {
        let mut e = Self::base(AckKind::SegmentEnded, epoch, ts_ms);
        e.seg = Some(seg);
        e
    }

    fn from_contribution(
        ctx: AckCtx<'_>,
        c: &Contribution,
        clamped: bool,
        degraded: bool,
        reason: Option<String>,
    ) -> Self {
        let (x, y, z) = report_axes(c, ctx.map, ctx.scales);
        Self {
            kind: ctx.kind,
            epoch: ctx.epoch,
            ts_ms: ctx.ts_ms,
            seq: c.seq,
            field: Some(c.field),
            id: c.id.clone(),
            x,
            y,
            z,
            intensity: c.intensity,
            clamped,
            degraded,
            reason,
            seg: None,
        }
    }

    fn dropped_from_cue(epoch: i64, ts_ms: f64, cue: &FieldCue, reason: &str) -> Self {
        Self {
            kind: AckKind::Dropped,
            epoch,
            ts_ms,
            seq: cue.seq,
            field: Some(cue.field),
            id: cue.id.clone(),
            x: cue.x,
            y: cue.y,
            z: cue.z,
            intensity: cue.intensity,
            clamped: false,
            degraded: false,
            reason: Some(reason.to_string()),
            seg: None,
        }
    }

    /// 投影成协议 §7.2 的 payload（`{version,type,payload}` 由 `emit_event` 加壳）。
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("type".to_string(), Value::from(self.kind.wire_type()));
        obj.insert("epoch".to_string(), Value::from(self.epoch));
        obj.insert("ts_ms".to_string(), Value::from(self.ts_ms));
        match self.seg {
            Some(seg) => {
                obj.insert("seg".to_string(), Value::from(seg));
            }
            None => {
                obj.insert("seq".to_string(), Value::from(self.seq));
                if let Some(field) = self.field {
                    obj.insert("field".to_string(), Value::from(field.as_str()));
                }
                if let Some(id) = &self.id {
                    obj.insert("id".to_string(), Value::from(id.clone()));
                }
                if let Some(x) = self.x {
                    obj.insert("x".to_string(), Value::from(x));
                }
                if let Some(y) = self.y {
                    obj.insert("y".to_string(), Value::from(y));
                }
                if let Some(z) = self.z {
                    obj.insert("z".to_string(), Value::from(z));
                }
                obj.insert("intensity".to_string(), Value::from(self.intensity));
                obj.insert("clamped".to_string(), Value::from(self.clamped));
                obj.insert("degraded".to_string(), Value::from(self.degraded));
                if let Some(reason) = &self.reason {
                    obj.insert("reason".to_string(), Value::from(reason.clone()));
                }
            }
        }
        Value::Object(obj)
    }
}

/// 非法 cue（`field` / `at` 解析失败）也要回一条可读的 `preset-dropped`——**不得静默**。
pub fn dropped_for_bad_cue(epoch: i64, ts_ms: f64, payload: &Value, reason: &str) -> AckEvent {
    let mut e = AckEvent::base(AckKind::Dropped, epoch, ts_ms);
    e.seq = payload.get("seq").and_then(Value::as_u64).unwrap_or(0) as u32;
    e.field = payload
        .get("field")
        .and_then(Value::as_str)
        .and_then(Field::parse);
    e.id = payload.get("id").and_then(Value::as_str).map(str::to_owned);
    e.reason = Some(reason.to_string());
    e
}

/// 本帧对一个参数的写入（`raw` = 累加后的未钳位值；`value` = 钳位后真正下发的值）。
#[derive(Debug, Clone, PartialEq)]
pub struct FrameWrite {
    pub field: Field,
    pub param: String,
    pub raw: f32,
    pub value: f32,
    pub clamped: bool,
}

/// 一帧的结果：本帧写出的参数 + 本帧产生的 ack。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameOutcome {
    pub events: Vec<AckEvent>,
    pub writes: Vec<FrameWrite>,
}

/// 一条正在生效的贡献（同字段的多条贡献相加）。
#[derive(Debug, Clone)]
struct Contribution {
    seq: u32,
    field: Field,
    id: Option<String>,
    intensity: f32,
    axes: [f32; 3],
    provided: [bool; 3],
    /// param → `amount × 轴值 × intensity`（未乘倍率、未钳位）。
    targets: Vec<(String, f32)>,
    elapsed_ms: f64,
    ttl_ms: f64,
    hold: bool,
    /// `preset-applied` / `preset-dropped` 尚未发出（首帧写入后才知道缺不缺参数）。
    ack_pending: bool,
}

/// 推迟到锚点 / `after_prev` 的 cue。
#[derive(Debug, Clone)]
struct Pending {
    cue: FieldCue,
    /// `after_prev` 等待的上一 cue 序号（`None` = 段锚点）。
    release_after: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct FieldState {
    contributions: Vec<Contribution>,
    pending: Vec<Pending>,
    /// 上一帧写过的参数（用于「全无贡献 → 回基准」时清 override）。
    last_written: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy)]
struct PrevCue {
    seq: u32,
    hold: bool,
}

/// 字段运行状态机（纯逻辑）。
#[derive(Debug, Clone)]
pub struct FieldRuntime {
    map: FieldMap,
    scales: PresetScales,
    states: [FieldState; 3],
    epoch: i64,
    has_epoch: bool,
    prev: Option<PrevCue>,
    clock: ClockState,
    last_now_ms: Option<f64>,
    last_domain: ClockDomain,
}

impl Default for FieldRuntime {
    fn default() -> Self {
        Self::new(FieldMap::builtin())
    }
}

impl FieldRuntime {
    /// 用给定映射表建状态机。
    pub fn new(map: FieldMap) -> Self {
        Self {
            map,
            scales: PresetScales::PRODUCT_DEFAULT,
            states: std::array::from_fn(|_| FieldState::default()),
            epoch: 0,
            has_epoch: false,
            prev: None,
            clock: ClockState::default(),
            last_now_ms: None,
            last_domain: ClockDomain::Wall,
        }
    }

    /// 当前映射表。
    pub fn map(&self) -> &FieldMap {
        &self.map
    }

    /// 更换映射表（加载外置覆盖后调用）；不重置状态。
    pub fn set_map(&mut self, map: FieldMap) {
        self.map = map;
    }

    /// 当前倍率。
    pub fn scales(&self) -> PresetScales {
        self.scales
    }

    /// 更新倍率（与旧 preset 路径共用同一份 `[action]` 三倍率）。
    pub fn set_scales(&mut self, scales: PresetScales) {
        self.scales = scales.clamped();
    }

    /// 当前批次代号。
    pub fn epoch(&self) -> i64 {
        self.epoch
    }

    /// 有效 `now_ms`（HUD / 诊断）。
    pub fn effective_now_ms(&self, wall_ms: f64) -> f64 {
        self.clock.effective_now_ms(wall_ms)
    }

    /// 当前时钟域。
    pub fn domain(&self) -> ClockDomain {
        self.clock.domain()
    }

    /// 收一条 stage-clock；返回 `segment-ended` ack（每段恰好一次）。
    pub fn on_clock(
        &mut self,
        seg: u32,
        pos_ms: f64,
        playing: bool,
        wall_ms: f64,
    ) -> Vec<AckEvent> {
        let ended = self.clock.on_clock(seg, pos_ms, playing);
        self.sync(wall_ms);
        let now = self.current_now();
        ended
            .into_iter()
            .map(|seg| AckEvent::segment_ended(self.epoch, now, seg))
            .collect()
    }

    /// 停止 / 新消息 / 换模型：清三字段贡献 + 清 override（不补帧）。
    pub fn revoke_all(&mut self, sink: &mut impl PresetSink) {
        for st in &mut self.states {
            for c in st.contributions.drain(..) {
                for (p, _) in &c.targets {
                    sink.clear_override_param(p);
                }
            }
            for p in std::mem::take(&mut st.last_written) {
                sink.clear_override_param(&p);
            }
            st.pending.clear();
        }
        self.prev = None;
        self.clock.reset();
    }

    /// 收一条字段 cue（`preset` payload；缺 `field` 时调用方走旧的 `preset_id` 路径）。
    ///
    /// 返回**立即**产生的 ack（`preset-replaced` / `preset-dropped`）；
    /// `preset-applied` / `preset-expired` 在随后的 [`FieldRuntime::frame`] 里发。
    pub fn accept_cue(&mut self, cue: FieldCue, wall_ms: f64) -> Vec<AckEvent> {
        self.sync(wall_ms);
        let mut events = Vec::new();
        let now = self.current_now();
        // D26：新一批（epoch 变化）→ 结束旧动画段。
        if cue.has_epoch && (!self.has_epoch || cue.epoch != self.epoch) {
            for field in Field::ALL {
                self.clear_field(field, now, &mut events);
            }
            self.epoch = cue.epoch;
            self.has_epoch = true;
            self.prev = None;
            self.clock.reset_closed();
        }
        let epoch = self.epoch;
        // expression 的 none 撤销哨兵（V11：语义不变）。
        if cue.field == Field::Expression && cue.id.as_deref() == Some(REVOKE_ID) {
            self.clear_field(Field::Expression, now, &mut events);
            self.prev = Some(PrevCue {
                seq: cue.seq,
                hold: cue.hold,
            });
            return events;
        }
        // 未知 / 缺失 expression id：丢该条 + 发 dropped（不得静默）。
        if cue.field == Field::Expression {
            let known = cue
                .id
                .as_deref()
                .is_some_and(|id| self.map.expression_targets(id).is_some());
            if !known {
                let reason = if cue.id.is_none() {
                    "expression_id_missing"
                } else {
                    "expression_unknown_id"
                };
                events.push(AckEvent::dropped_from_cue(epoch, now, &cue, reason));
                return events;
            }
        }
        // 锚点。
        match cue.at {
            Anchor::Now => self.activate(cue.clone(), now, &mut events),
            Anchor::Seg(_) => {
                self.states[cue.field.index()].pending.push(Pending {
                    cue: cue.clone(),
                    release_after: None,
                });
            }
            Anchor::AfterPrev => {
                match (self.prev.map(|p| p.seq), self.prev.map(|p| p.hold)) {
                    // O4：上一条是 hold（永远没有「做完」点）→ 退化为 now。
                    (Some(seq), Some(false)) => {
                        self.states[cue.field.index()].pending.push(Pending {
                            cue: cue.clone(),
                            release_after: Some(seq),
                        });
                    }
                    _ => self.activate(cue.clone(), now, &mut events),
                }
            }
        }
        self.prev = Some(PrevCue {
            seq: cue.seq,
            hold: cue.hold,
        });
        events
    }

    /// 每帧：推进 ttl、兑现段锚点、写参数、发 `preset-applied` / `preset-expired`。
    pub fn frame(&mut self, wall_ms: f64, sink: &mut impl PresetSink) -> FrameOutcome {
        self.sync(wall_ms);
        let now = self.current_now();
        let seg = self.clock.active_seg();
        let mut out = FrameOutcome::default();

        // 1. 段锚点兑现。
        for field in Field::ALL {
            let idx = field.index();
            let mut i = 0;
            while i < self.states[idx].pending.len() {
                let fire = match self.states[idx].pending[i].cue.at {
                    Anchor::Seg(n) => seg.is_some_and(|s| s >= n),
                    _ => false,
                };
                if fire {
                    let p = self.states[idx].pending.remove(i);
                    self.activate(p.cue, now, &mut out.events);
                } else {
                    i += 1;
                }
            }
        }

        // 2. 非 hold 到点 → 移除自己的贡献 + `preset-expired`。
        let mut expired: Vec<u32> = Vec::new();
        for field in Field::ALL {
            let idx = field.index();
            let mut i = 0;
            while i < self.states[idx].contributions.len() {
                let due = {
                    let c = &self.states[idx].contributions[i];
                    !c.hold && c.elapsed_ms >= c.ttl_ms
                };
                if due {
                    let c = self.states[idx].contributions.remove(i);
                    out.events.push(AckEvent::from_contribution(
                        AckCtx {
                            kind: AckKind::Expired,
                            epoch: self.epoch,
                            ts_ms: now,
                            map: &self.map,
                            scales: self.scales,
                        },
                        &c,
                        false,
                        false,
                        None,
                    ));
                    expired.push(c.seq);
                } else {
                    i += 1;
                }
            }
        }
        for seq in expired {
            self.release_waiters(seq, now, &mut out.events);
        }

        // 3. 写参数（同类 add → 统一钳位）+ 发出首帧 ack。
        for field in Field::ALL {
            let idx = field.index();
            let mut sums: Vec<(String, f32)> = Vec::new();
            for c in &self.states[idx].contributions {
                for (p, v) in &c.targets {
                    let scaled = v * self.scales.for_param(p);
                    add_into(&mut sums, p, scaled);
                }
            }
            let mut accepted: BTreeMap<String, bool> = BTreeMap::new();
            let mut clamped_by: BTreeMap<String, bool> = BTreeMap::new();
            let mut live: BTreeSet<String> = BTreeSet::new();
            for (param, raw) in &sums {
                let value = clamp_to_channel(param, *raw);
                let clamped = (value - *raw).abs() > 1e-6;
                let ok = sink.override_param(param, value);
                accepted.insert(param.clone(), ok);
                clamped_by.insert(param.clone(), clamped);
                out.writes.push(FrameWrite {
                    field,
                    param: param.clone(),
                    raw: *raw,
                    value,
                    clamped,
                });
                if ok {
                    live.insert(param.clone());
                }
            }
            // 全无贡献（或参数消失）→ 回基准：清掉上一帧写过、这一帧不写的 override。
            let stale: Vec<String> = self.states[idx]
                .last_written
                .difference(&live)
                .cloned()
                .collect();
            for param in stale {
                sink.clear_override_param(&param);
            }
            self.states[idx].last_written = live;

            let map = &self.map;
            let scales = self.scales;
            let epoch = self.epoch;
            let st = &mut self.states[idx];
            for c in &mut st.contributions {
                if !c.ack_pending {
                    continue;
                }
                c.ack_pending = false;
                let missing: Vec<&str> = c
                    .targets
                    .iter()
                    .filter(|(p, _)| accepted.get(p) == Some(&false))
                    .map(|(p, _)| p.as_str())
                    .collect();
                let any_ok = c
                    .targets
                    .iter()
                    .any(|(p, _)| accepted.get(p) == Some(&true));
                let clamped = c
                    .targets
                    .iter()
                    .any(|(p, _)| clamped_by.get(p) == Some(&true));
                if !any_ok {
                    // §7.1：皮套缺参数 = 最该暴露的事实（不得静默无反应）。
                    out.events.push(AckEvent::from_contribution(
                        AckCtx {
                            kind: AckKind::Dropped,
                            epoch,
                            ts_ms: now,
                            map,
                            scales,
                        },
                        c,
                        false,
                        true,
                        Some(missing.join(",")),
                    ));
                } else {
                    let degraded = !missing.is_empty();
                    let reason = degraded.then(|| missing.join(","));
                    out.events.push(AckEvent::from_contribution(
                        AckCtx {
                            kind: AckKind::Applied,
                            epoch,
                            ts_ms: now,
                            map,
                            scales,
                        },
                        c,
                        clamped,
                        degraded,
                        reason,
                    ));
                }
            }
        }
        out
    }

    /// HUD 一行（`preset:` 行的一部分）。
    pub fn diag(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for field in Field::ALL {
            let st = &self.states[field.index()];
            let n = st.contributions.len();
            if n == 0 {
                parts.push(format!("{}: -", field.as_str()));
                continue;
            }
            if field == Field::Expression {
                let ids: Vec<&str> = st
                    .contributions
                    .iter()
                    .filter_map(|c| c.id.as_deref())
                    .collect();
                parts.push(format!("expr:{}[{n}]", ids.join("+")));
            } else {
                let hold = st.contributions.iter().filter(|c| c.hold).count();
                let mut s = format!("{}[{n}", field.as_str());
                if hold > 0 {
                    s.push_str(&format!(",hold{hold}"));
                }
                s.push(']');
                parts.push(s);
            }
        }
        parts.join(" ")
    }

    // ── 内部 ──────────────────────────────────────────────────────────

    fn sync(&mut self, wall_ms: f64) {
        let now = self.clock.effective_now_ms(wall_ms);
        let domain = self.clock.domain();
        let dt = match self.last_now_ms {
            Some(prev) if self.last_domain == domain => (now - prev).max(0.0),
            _ => 0.0,
        };
        if dt > 0.0 {
            for st in &mut self.states {
                for c in &mut st.contributions {
                    c.elapsed_ms += dt;
                }
            }
        }
        self.last_now_ms = Some(now);
        self.last_domain = domain;
    }

    fn current_now(&self) -> f64 {
        self.last_now_ms.unwrap_or(0.0)
    }

    /// 结束某字段当前动画段（发 `preset-replaced`）并清空贡献 + 待兑现 cue。
    fn clear_field(&mut self, field: Field, now: f64, events: &mut Vec<AckEvent>) {
        let idx = field.index();
        let old = std::mem::take(&mut self.states[idx].contributions);
        for c in &old {
            events.push(AckEvent::from_contribution(
                AckCtx {
                    kind: AckKind::Replaced,
                    epoch: self.epoch,
                    ts_ms: now,
                    map: &self.map,
                    scales: self.scales,
                },
                c,
                false,
                false,
                None,
            ));
        }
        self.states[idx].pending.clear();
    }

    fn activate(&mut self, cue: FieldCue, now: f64, events: &mut Vec<AckEvent>) {
        let targets = self.materialize(&cue);
        if targets.is_empty() {
            events.push(AckEvent::dropped_from_cue(
                self.epoch,
                now,
                &cue,
                "no_channel",
            ));
            return;
        }
        let mut axes = [0.0_f32; 3];
        let mut provided = [false; 3];
        if cue.field != Field::Expression {
            for (idx, v) in [(0, cue.x), (1, cue.y), (2, cue.z)] {
                if let Some(v) = v {
                    axes[idx] = v;
                    provided[idx] = true;
                }
            }
        }
        self.states[cue.field.index()]
            .contributions
            .push(Contribution {
                seq: cue.seq,
                field: cue.field,
                id: cue.id.clone(),
                intensity: cue.intensity,
                axes,
                provided,
                targets,
                elapsed_ms: 0.0,
                ttl_ms: cue.ttl_ms,
                hold: cue.hold,
                ack_pending: true,
            });
    }

    fn materialize(&self, cue: &FieldCue) -> Vec<(String, f32)> {
        let mut out: Vec<(String, f32)> = Vec::new();
        match cue.field {
            Field::Body | Field::Head => {
                for (axis, value) in [(0, cue.x), (1, cue.y), (2, cue.z)] {
                    let Some(value) = value else { continue };
                    for t in self.map.axis_targets(cue.field, axis) {
                        if !field_allows(cue.field, &t.param) {
                            continue;
                        }
                        add_into(&mut out, &t.param, t.amount * value * cue.intensity);
                    }
                }
            }
            Field::Expression => {
                let Some(id) = cue.id.as_deref() else {
                    return out;
                };
                let Some(targets) = self.map.expression_targets(id) else {
                    return out;
                };
                for t in targets {
                    // 唯一的通道红线判据（与建表同一函数）——C2 负对照就改这里。
                    if !field_allows(Field::Expression, &t.param) {
                        continue;
                    }
                    add_into(&mut out, &t.param, t.amount * cue.intensity);
                }
            }
        }
        out
    }

    /// `after_prev`：等到 `seq` 的贡献被移除（到点）时兑现。
    fn release_waiters(&mut self, seq: u32, now: f64, events: &mut Vec<AckEvent>) {
        let mut ready: Vec<FieldCue> = Vec::new();
        for field in Field::ALL {
            let idx = field.index();
            let mut i = 0;
            while i < self.states[idx].pending.len() {
                if self.states[idx].pending[i].release_after == Some(seq) {
                    ready.push(self.states[idx].pending.remove(i).cue);
                } else {
                    i += 1;
                }
            }
        }
        for cue in ready {
            self.activate(cue, now, events);
        }
    }
}

/// 同 param 累加（同类 add）。
fn add_into(list: &mut Vec<(String, f32)>, param: &str, value: f32) {
    if let Some(entry) = list.iter_mut().find(|(p, _)| p == param) {
        entry.1 += value;
    } else {
        list.push((param.to_string(), value));
    }
}

/// ack 里的 `x/y/z`：**最终生效**的轴值 = 轴值 × intensity × 通道倍率，钳到 `[-1,1]`。
fn report_axes(
    c: &Contribution,
    map: &FieldMap,
    scales: PresetScales,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    if c.field == Field::Expression {
        return (None, None, None);
    }
    let mut out: [Option<f32>; 3] = [None; 3];
    for (axis, slot) in out.iter_mut().enumerate() {
        if !c.provided[axis] {
            continue;
        }
        let scale = map
            .axis_targets(c.field, axis)
            .first()
            .map_or(1.0, |t| scales.for_param(&t.param));
        *slot = Some((c.axes[axis] * c.intensity * scale).clamp(-1.0, 1.0));
    }
    (out[0], out[1], out[2])
}
