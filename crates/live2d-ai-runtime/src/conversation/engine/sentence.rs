//! 一句原文 → 上屏文本 + 送 TTS 文本 + 这一句的 cue（按句第二路的**唯一**消费点）。
//!
//! 0.4.4 rc.1：选完 cue 之后按**动作频率**（`crate::motion_rate`）过滤——
//! **顺序锁死：先倾向、再频率**（主动档空数组回退规则表仍在上面的 match 里）。
//!
//! rc4（2026-10-17）：动作恒开，每句都产 cue——
//! - cleaner 可用 → 模型 cues（空数组只有主动档回退规则表）；
//! - 失败 / 超时 / 坏 JSON / 未解析 → **这一句原文**的规则表；
//! - cleaner 不可用 → clean_for_tts + 规则表。
//!
//! 规则表单一真源 = [`crate::performance::rules::rule_cues_for_sentence`]（已从
//! director Mod 迁入核心）——实参只能是这一句一路原文，不得是用户输入。
//!
//! 三条口径（2026-10-08，见 `crate::cleaning` 与 `conversation/mod.rs` 的契约）：
//! 1. 不跑二路 / 二路 disabled → 上屏与送 TTS 都是 `clean_for_tts(原文)`；
//! 2. 二路可用 → 交给它；失败 / 超时 / 坏 JSON / `display` 不是原文切片由
//!    [SentenceCleaner::clean] 收敛成「两份都用原文」；
//! 3. 清洗是**逐句、同步**的：这一句拿到结果之前不读下一条 token——不是延迟
//!    优化，而是「这一句的提交必须发生在下一句原文出现之前」的可观测顺序。

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::cleaning::SentenceCleaner;
use crate::motion_rate::filter_cues;
use crate::performance::PerformanceCue;
use crate::performance::rules::rule_cues_for_sentence;

/// 一句原文 → **两份文本 + 这一句的 cue**（`(display, speech, cues)`）；`None` = 期间被取消。
///
/// `cues` 恒为「这一句」的 cue（可能为空 = 中性句不动）：
/// 模型给了有效 cues 就用模型的；失败 / 未解析 / 二路不可用则用
/// [`rule_cues_for_sentence`]（**实参只能是这一句一路原文**）。
pub(super) async fn sentence_texts(
    cleaner: Option<&Arc<SentenceCleaner>>,
    raw: &str,
    sentence_seq: u64,
    cancel: &CancellationToken,
) -> Option<(String, String, Vec<PerformanceCue>)> {
    // 规则回退只用这一句一路原文（不看用户句 / 上下文）。
    let rule = || rule_cues_for_sentence(raw, sentence_seq);
    let Some(cleaner) = cleaner.filter(|c| c.enabled()) else {
        // 句首/句尾空白由分句器保证已 trim；`clean_for_tts` 仍做幂等兜底。
        // **早退路径保持原样**（0.4.4 rc.1 明示）：没有二路 = 没有倾向选择，
        // 因此**也不做动作频率过滤**——这一句的规则表 cue 原样返回。
        let cleaned = crate::dialogue::clean_for_tts(raw);
        return Some((cleaned.clone(), cleaned, rule()));
    };
    let cleaned = tokio::select! {
        _ = cancel.cancelled() => return None,
        cleaned = cleaner.clean(raw) => cleaned,
    };
    let cues = match &cleaned.cue_ids {
        // 主动档：模型交回空数组、且表演倾向允许空数组回退时，改用规则表
        // （实参仍是这一句一路原文）。克制 / 平常的空数组仍是成功、不回退。
        Some(ids) if ids.is_empty() && cleaner.empty_cues_fall_back() => {
            let cues = rule();
            tracing::info!(
                target: "cleaning",
                sentence_seq,
                cues = cues.len(),
                "主动档：模型交回空数组，本句改用规则表"
            );
            cues
        }
        Some(ids) => crate::cleaning::cues_from_ids(ids, sentence_seq),
        // 未请求 cues / 失败 / 超时 / 坏 JSON：回退这一句原文的规则表。
        None => rule(),
    };
    // **顺序锁死：先倾向、再频率**。上面的 match 已完成倾向选择（主动档空数组
    // 回退整张规则表也发生在那里）；这里才按动作频率过滤：稀疏无问号清空 /
    // 有问号只留第一条，密集在本句空表且非克制档时补规则表第一条。
    let cues = filter_cues(
        cleaner.rate(),
        cleaner.bias_is_restrained(),
        raw,
        sentence_seq,
        cues,
    );
    Some((cleaned.display, cleaned.speech, cues))
}
