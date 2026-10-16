//! 一句原文 → 上屏文本 + 送 TTS 文本 + 这一句的 cue（二路清洗的**唯一**消费点）。
//!
//! 从 engine.rs 拆出（2026-10-16，D5 同款「先只搬家」）。行为一行未改。

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::cleaning::SentenceCleaner;

/// 一句原文 → **两份文本 + 这一句的 cue**（`(display, speech, cues)`）；`None` = 期间被取消。
///
/// `cues` 只在动作门（`motion`）开着时非空：模型给了 `cues` 就用模型的（可空 = 这一句不动；
/// **只有「主动」档**例外，空数组且规则表有动作时改用规则表——2026-10-13 执行单 T3），
/// 模型失败 / 未解析才回退 [`rule`]（**实参只能是这一句一路原文**）。
///
/// 三条口径（2026-10-08，见 `crate::cleaning` 与 `conversation/mod.rs` 的契约）：
/// 1. 不跑二路 / 二路 disabled → 上屏与送 TTS 都是 `clean_for_tts(原文)`
///    （既有行为逐字不变）；
/// 2. 二路可用 → 交给它；失败 / 超时 / 坏 JSON / `display` 不是原文切片由
///    [SentenceCleaner::clean] 收敛成「两份都用原文」；
/// 3. 清洗是**逐句、同步**的：这一句拿到结果之前不读下一条 token——不是延迟
///    优化，而是「这一句的提交必须发生在下一句原文出现之前」的可观测顺序。
pub(super) async fn sentence_texts(
    cleaner: Option<&Arc<SentenceCleaner>>,
    raw: &str,
    sentence_seq: u64,
    motion: bool,
    rule: Option<&crate::cleaning::SentenceRuleFallback>,
    cancel: &CancellationToken,
) -> Option<(String, String, Vec<crate::performance::PerformanceCue>)> {
    // 动作门开着时的按句规则回退：**只用这一句一路原文**（`raw`），不看用户句。
    let fallback = |raw: &str| -> Vec<crate::performance::PerformanceCue> {
        if motion {
            rule.map(|r| r.call(raw, sentence_seq)).unwrap_or_default()
        } else {
            Vec::new()
        }
    };
    // 句首/句尾空白由分句器保证已 trim；`clean_for_tts` 仍做幂等兜底。
    let Some(cleaner) = cleaner.filter(|c| c.enabled()) else {
        let cleaned = crate::dialogue::clean_for_tts(raw);
        let cues = fallback(raw);
        return Some((cleaned.clone(), cleaned, cues));
    };
    let cleaned = tokio::select! {
        _ = cancel.cancelled() => return None,
        cleaned = cleaner.clean(raw) => cleaned,
    };
    // 成功解析（含空 `cues` = 这一句不动）用模型 cues；失败 / 未解析才回退规则。
    // 2026-10-13 执行单 T3：**只有「主动」档**的空数组例外——模型交回空数组、
    // 且表演倾向允许空数组回退（`empty_cues_fall_back`）时，改用已有的按句
    // 规则表（实参仍是这一句一路原文）。克制 / 平常的空数组仍是成功、不回退。
    let cues = if motion {
        match &cleaned.cue_ids {
            Some(ids) if ids.is_empty() && cleaner.empty_cues_fall_back() => {
                let cues = fallback(raw);
                tracing::info!(
                    target: "cleaning",
                    sentence_seq,
                    cues = cues.len(),
                    "主动档：模型交回空数组，本句改用规则表"
                );
                cues
            }
            Some(ids) => crate::cleaning::cues_from_ids(ids, sentence_seq),
            None => fallback(raw),
        }
    } else {
        Vec::new()
    };
    Some((cleaned.display, cleaned.speech, cues))
}
