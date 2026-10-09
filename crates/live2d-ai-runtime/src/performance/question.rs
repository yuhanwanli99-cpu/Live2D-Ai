//! **问句补丁**（T9，2026-10-07）：本地词典给第一处问句所在段补一次歪头 + 思考。
//!
//! 从 plan.rs 拆出（AGENTS「源码 ≤500 行；>500 需豁免头注、上限 1000」）：plan.rs
//! 只留 v1 schema 与严格校验，问句词典与它的常量住这里——「只认 ？/?」「补哪一段」
//! 「补什么值」三件事仍然并排可读，parse_plan 在校验**成功之后**调
//! [apply_question_patch]，legacy speak 路径不调。
//!
//! 纯函数（无 IO / 无网络 / 无时钟 / 无随机），逐条口径见 [apply_question_patch]。

use super::plan::{CueField, PerformanceCue, PerformancePlan};

/// 问句只认这两个标记（T9 口径，2026-10-07）：全角 ？ 与半角 ?。
///
/// **不**把「吗 / 呢 / 什么」当问句——那些词会误伤陈述句。director 的
/// derive_intent 另有一套询问式词表（意图推导），两者刻意不是同一件事：
/// 这里判的是「原文里有没有问号」。
pub const QUESTION_MARKS: [char; 2] = ['？', '?'];

/// 正文里有没有问句标记（问句补丁与 director 规则层**共用这一条判据**）。
pub fn has_question_mark(text: &str) -> bool {
    text.chars().any(|c| QUESTION_MARKS.contains(&c))
}

/// 问句补丁的歪头轴值 x：固定 0。
pub const QUESTION_HEAD_X: f64 = 0.0;
/// 问句补丁的歪头轴值 y：微抬。
pub const QUESTION_HEAD_Y: f64 = 0.12;
/// 问句补丁的歪头轴值 z：固定为正（与现成 tilt_left 同向）。
pub const QUESTION_HEAD_Z: f64 = 0.45;
/// 问句补丁的强度（歪头与思考同档）。
pub const QUESTION_INTENSITY: u8 = 2;
/// 问句补丁的歪头时长（毫秒）。
pub const QUESTION_HEAD_TTL_MS: u64 = 1_800;
/// 问句补丁的表情 id。
pub const QUESTION_EXPRESSION_ID: &str = "thinking";
/// 问句补丁的表情时长（毫秒）。
pub const QUESTION_EXPRESSION_TTL_MS: u64 = 2_600;

/// 一轮里**第一处**问句所在的段（1-based）；没有问号 → None。
pub fn first_question_segment(segments: &[String]) -> Option<u64> {
    segments
        .iter()
        .position(|segment| has_question_mark(segment))
        .map(|index| index as u64 + 1)
}

/// **问句补丁**（T9，2026-10-07）：本地词典给第一处问句所在段补一次歪头 + 思考。
///
/// 口径逐条：
/// - 只补**第一处**问句所在的那一段（[has_question_mark]；没有 → 原样返回）；
/// - 两样都锚在该段音频开始（seg:N），hold = false：不跨轮常驻、不随机、
///   不做「50% 重播」；
/// - 该段上模型交来的 head / expression **一律换成词典值**（这两项以词典为准），
///   别的段上的合法 cue 原样保留；
/// - **不改字**：segments 逐字不动（V1 拼接恒等仍成立），不加标签、不做内容审查；
/// - 空 cues + 合法分段仍是成功（这里只是**在成功结果上叠加**，绝不把这一轮
///   打成失败，也不退回直送）；
/// - 纯函数（无 IO / 无网络 / 无时钟 / 无随机）：同一份 plan 恒等输出。
///
/// **legacy 路径不补**（segments == None）：那条路径的文本来自 speak，
/// 与「问句所在的那一段」没有对应关系。
///
/// 为什么问句两项在这里而不是在第二路 LLM：通用大模型没有「填写 Live2D cue
/// 数组」的训练，第二路只负责切分原文（T9 口径：prompt.rs 一个字不改）。
pub fn apply_question_patch(mut plan: PerformancePlan) -> PerformancePlan {
    let Some(segments) = plan.segments.as_ref() else {
        return plan;
    };
    let Some(anchor) = first_question_segment(segments) else {
        return plan;
    };
    // 新 cue 的 seq 接着**原 plan 内序号**往下排（丢条不重排的既有口径）。
    let next_seq = plan
        .cues
        .iter()
        .filter_map(|cue| cue.seq)
        .max()
        .unwrap_or(0)
        + 1;
    plan.cues.retain(|cue| {
        cue.sentence_seq != anchor
            || !matches!(cue.field, Some(CueField::Head) | Some(CueField::Expression))
    });
    plan.cues.push(question_head_cue(anchor, next_seq));
    plan.cues
        .push(question_expression_cue(anchor, next_seq + 1));
    plan
}

/// 问句补丁的歪头（head 字段）：x=0 / y=0.12 / z=0.45 / intensity=2 / 1800ms。
///
/// 幅度是算过的：头通道满幅 30°，再乘出厂 head_scale 0.75，
/// 所以 0.45 × 2 × 30 × 0.75 ≈ 20°——比现成手势包（约 9°）明显，
/// 也不会顶到 30° 钳位。**不改滑条默认值**。
fn question_head_cue(anchor: u64, seq: u64) -> PerformanceCue {
    PerformanceCue {
        sentence_seq: anchor,
        preset_id: CueField::Head.as_str().to_string(),
        intensity: QUESTION_INTENSITY,
        ttl_ms: QUESTION_HEAD_TTL_MS,
        field: Some(CueField::Head),
        x: serde_json::Number::from_f64(QUESTION_HEAD_X),
        y: serde_json::Number::from_f64(QUESTION_HEAD_Y),
        z: serde_json::Number::from_f64(QUESTION_HEAD_Z),
        id: None,
        at: Some(format!("seg:{anchor}")),
        hold: Some(false),
        seq: Some(seq),
    }
}

/// 问句补丁的思考表情（expression 字段，id=thinking，2600ms）。
fn question_expression_cue(anchor: u64, seq: u64) -> PerformanceCue {
    PerformanceCue {
        sentence_seq: anchor,
        preset_id: QUESTION_EXPRESSION_ID.to_string(),
        intensity: QUESTION_INTENSITY,
        ttl_ms: QUESTION_EXPRESSION_TTL_MS,
        field: Some(CueField::Expression),
        x: None,
        y: None,
        z: None,
        id: Some(QUESTION_EXPRESSION_ID.to_string()),
        at: Some(format!("seg:{anchor}")),
        hold: Some(false),
        seq: Some(seq),
    }
}
