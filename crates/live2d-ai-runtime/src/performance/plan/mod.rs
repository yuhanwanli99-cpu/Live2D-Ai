//! **表演层唯一输出 schema** + 严格校验（本文件是单一真源）。
//!
//! # 契约（v1，2026-09-26 契约冻结；文档见 docs/architecture/performance-protocol-v1.md）
//!
//! 每一轮表演层必须交回**恰好一份** JSON：
//!
//! ```text
//! {
//!   "segments": ["嗯……", "我想到了。"],     // 只能切分主模型原文，逐字不变（V1）
//!   "cues": [
//!     { "field": "head", "x": 0.0, "y": 0.3, "z": 0.2, "intensity": 1, "at": "now", "hold": true },
//!     { "field": "expression", "id": "thinking", "intensity": 1, "at": "seg:2", "hold": false }
//!   ]
//! }
//! ```
//!
//! - **segments**：原文的**切分方案**（不是上屏字符串）。核心不变量：
//!   segments.concat() == 主模型原文（逐码点）——不等 = **整份失败**
//!   （performance_plan_segments_not_partition）。空原文 → segments: []。
//! - **cues**：两族表演字段 head / expression（**body 已停用**，T8：交上来就丢该条
//!   并 warn）；同类按 add 合成、立即生效不排队。[] = 本轮不动（不是撤销）。
//!   头的 x/y/z 落在现有头部角（ParamAngle*），颈随之转动——**没有新参数**。
//! - **speak**：v0 旧字段，**保留可解析（弃用，V11）**；只有 segments 缺席时走
//!   旧路径。与 segments 同现 → **segments 优先、speak 忽略 + warn**（O1）。
//! - **未知顶层键一律丢弃**（宽容）。
//!
//! # 校验纪律（整份失败，不做局部抢救）
//!
//! - 坏 JSON / 顶层不是对象 / 缺 segments（且无 speak）/ 类型不对 / 词表外 field /
//!   at 非法 / 越界锚点 / 超条数 / 超长 / **拼接不等于原文** → **整份失败** → 引擎回退；
//! - 轴值越界 / intensity 越界 / ttl_ms 越界 → **钳位**（不是失败）；
//! - expression 给 z/x/y、id 给非 expression → **丢该键 + warn**；
//! - **field=body → 丢该条 cue + warn**（T8 起只演头与表情；整份失败会连带丢语音，
//!   代价不对等，所以是丢条而不是失败）；
//! - 未知表情 id → **丢该条 cue + warn**（§2.4 #16；整份失败会连带丢语音）；
//! - **expression 的 id 只认表情槽**（[EXPRESSION_PRESET_IDS] = smile / unhappy /
//!   surprised / thinking，外加撤销哨兵 none）：手势 id（nod / tilt_left / look_up …）
//!   填进 expression → **丢该条 cue + warn**（T9；整份仍成功——表情表只有脸，
//!   手势 id 交上来在渲染面本来也会被丢掉）。
//!
//! # 问句补丁（T9，2026-10-07）
//!
//! 校验**成功之后**，[apply_question_patch](super::question::apply_question_patch)
//! 用本地词典给「第一处问句所在的那一段」补一次歪头
//! （head x=0 y=0.12 z=0.45 intensity=2 1800ms）与思考
//! （expression id=thinking intensity=2 2600ms），两样都锚在该段音频开始。
//! 该段上模型交来的 head / expression **换成词典值**；别的段原样保留；
//! segments 一个字不改、不加标签、不随机、不跨轮常驻。没有问号（只认 ？ / ?）
//! 就什么都不补。legacy speak 路径不补。
//!
//! # 为什么 schema 写在代码里
//!
//! [json_schema_strict] **由 `types.rs` 的常量拼出**，`parse.rs` 里的 [parse_plan]
//! 逐条实现同样的边界；回归 schema_and_validator_share_the_same_bounds 钉住
//! 「文档里的 schema == 发出去的 schema == 校验器认的东西」。不要在别处再抄一份。
//!
//! # 文件划分（2026-10-17）
//!
//! `mod.rs` 只做模块装配与 `pub use` 重导出（保持 `crate::performance::plan::*` 路径）；
//! 类型 / 常量在 `types.rs`，严格校验 / schema / wire 投影在 `parse.rs`——两处**同源**，
//! 仍由 schema_and_validator_share_the_same_bounds 钉住同一组边界。
//!
//! # v1 cue 的 wire 投影（B9 / V11 / D27）
//!
//! v1 的新键（`field` / `x` / `y` / `z` / `id` / `at` / `hold` /
//! `seq`）是 [PerformanceCue] 上的**显式可选字段**（`Option`，缺省 `None`）——
//! `preset_id` **只承载预设 id 本身**，绝不承载 JSON（Gate 4 D27：控制字符
//! 信封退场）。[PerformanceCue::to_json] 把它们摊平成**既有 action_cue 帧**上的
//! 可选键；既有键 sentence_seq/preset_id/intensity/ttl_ms/priority 逐字保留；
//! `field == None` = 旧 preset_id 路径，语义逐字不变（V11）。
//!
//! `epoch` **不在 cue 上**：它是 `action_cue` 帧的 payload 级字段
//!（[action_cue_payload]），前端→渲染面的 `preset` 消息按 D30 单独透传。

mod parse;
mod types;

pub use parse::*;
pub use types::*;
