//! **导演动作频率**（0.4.4 rc.1）：按句 cue 的第三个、也是最后一个旋钮——
//! `sparse` / `normal` / `dense`。
//!
//! # 它解决什么
//!
//! 「表演倾向」（`motion_bias`：克制 / 平常 / 主动）决定**模型被要求选几个**；
//! 「动作频率」决定**模型 / 规则表选出来的 cue 最终留几条**。两者是串联的两步，
//! 顺序锁死：**先倾向**（[sentence_texts](crate::conversation) 里主动档的空数组
//! 回退规则表仍在那一步），**再频率**（本模块）。反过来会让「主动档回退填表」
//! 绕过频率过滤。
//!
//! # 三档口径（**不随机、不新增预设**）
//!
//! | 档位 | 这一句没有问号（`？` / `?`） | 这一句有问号 |
//! | --- | --- | --- |
//! | `sparse` 稀疏 | **清空**（这一句不动） | 最多留**第一条** |
//! | `normal` 平常（**缺省**） | 原样 | 原样 |
//! | `dense` 密集 | 空表且**非克制**档时，从规则表（[rule_cues_for_sentence]）**只补第一条** | 同左 |
//!
//! `dense` 的补充只发生在「这一步之前已经产出的 cue 为空」时：主动档上一步
//! 已经把整张规则表填进来（非空）→ **不再补**；克制档**不补**（克制就是宁可少）。
//! 规则表对这句原文没有动作（空表）→ 也不补。
//!
//! **不改 ttl**：本模块只增 / 删 / 截断条目，不碰 `intensity` / `ttl_ms`
//! （规则表 `ttl_ms = 2000` 照旧）。**不随机**：同一输入恒等输出。
//! **不接 `action_tx`**：cue 仍只经引擎既有的 `ActionCue` 事件出口出去。
//!
//! [rule_cues_for_sentence]: crate::performance::rules::rule_cues_for_sentence

use crate::performance::PerformanceCue;
use crate::performance::question::has_question_mark;
use crate::performance::rules::rule_cues_for_sentence;

/// 动作频率三档。未知 / 缺省 = [MotionRate::Normal]（**绝不失败**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionRate {
    /// 稀疏：无问号整句不出动作；有问号最多留第一条。
    Sparse,
    /// 平常（**缺省**）：不增不减。
    #[default]
    Normal,
    /// 密集：本句空表且非克制档时，按规则表补第一条。
    Dense,
}

impl MotionRate {
    /// 解析配置值：`sparse` / `dense`；未知或空 = [MotionRate::Normal]。
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("sparse") => Self::Sparse,
            Some("dense") => Self::Dense,
            _ => Self::Normal,
        }
    }

    /// 稳定码（mods.json / 设置面 / 日志字段）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::Normal => "normal",
            Self::Dense => "dense",
        }
    }
}

/// 按频率过滤**已经选好**的这一句 cue（顺序锁死：倾向 → 频率）。
///
/// 参数 `bias_restrained` = 当前表演倾向是否「克制」档；`cues` 为空但**没有**
/// 倾向时按克制处理（**不可补**，见 `SentenceCleaner::bias_is_restrained`）。
/// `raw` 只能是**这一句一路原文**（规则表补充与问句判据都只看它）；
/// `sentence_seq` 是这一句的句号（补出来的 cue 必须锚在同一句）。
///
/// 纯函数：无 IO / 无网络 / 无时钟 / 无随机。
pub fn filter_cues(
    rate: MotionRate,
    bias_restrained: bool,
    raw: &str,
    sentence_seq: u64,
    cues: Vec<PerformanceCue>,
) -> Vec<PerformanceCue> {
    match rate {
        MotionRate::Normal => cues,
        MotionRate::Sparse => {
            if has_question_mark(raw) {
                // 问句已经是「这一句值得动一下」的最小信号：只留第一条。
                cues.into_iter().take(1).collect()
            } else {
                // 无问号 = 稀疏档这一句不动（不随机、不补规则表）。
                Vec::new()
            }
        }
        MotionRate::Dense => {
            // 非空 = 这一步之前已经产出了动作（模型给的 / 主动档回退的整张表）：
            // 密集档**不再加**——频率只调「有没有」，不在既有 cue 上叠层。
            if !cues.is_empty() {
                return cues;
            }
            // 克制档不补；没有倾向（bias_restrained=true）也不补。
            if bias_restrained {
                return cues;
            }
            // 规则表对**这一句原文**没有动作 = 不补（不硬造、不新增预设）。
            rule_cues_for_sentence(raw, sentence_seq)
                .into_iter()
                .take(1)
                .collect()
        }
    }
}

#[cfg(test)]
#[path = "motion_rate_tests.rs"]
mod tests;
