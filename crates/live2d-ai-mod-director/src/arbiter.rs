//! 三级仲裁（P1-3，纯函数）：规则 / 异步 / 标签 按优先级合并，按 sentence_seq 持有。
//!
//! 优先级（见 crate::plan）：规则 10 < 异步 40 < 标签 80。同一句被多条 cue 命中时
//! **高优先级覆盖低优先级**；同级保留先到的（稳定）。
//!
//! epoch 是**硬闸**：不匹配的 plan 整份丢弃、**零状态变更**（离线回归 2）。

use std::collections::BTreeMap;

use crate::plan::{Cue, DirectorPlan};

/// 按句持有的仲裁表。
#[derive(Debug, Default, Clone)]
pub struct Arbiter {
    epoch: u64,
    covers_upto_seq: u64,
    cues: BTreeMap<u64, Cue>,
}

impl Arbiter {
    /// 以某轮 epoch 建表。
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            covers_upto_seq: 0,
            cues: BTreeMap::new(),
        }
    }

    /// 当前 epoch。
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 已覆盖到第几句（plan 的效力终点）。
    pub fn covers_upto_seq(&self) -> u64 {
        self.covers_upto_seq
    }

    /// 应用一份 plan：epoch 不匹配 -> false 且**零副作用**。
    pub fn apply(&mut self, plan: &DirectorPlan) -> bool {
        if plan.epoch != self.epoch {
            return false;
        }
        for cue in &plan.cues {
            self.upsert(cue.clone());
        }
        self.covers_upto_seq = self.covers_upto_seq.max(plan.covers_upto_seq);
        true
    }

    /// 写入一条 cue：同句高优先级覆盖；同级保留先到的。
    pub fn upsert(&mut self, cue: Cue) {
        match self.cues.get(&cue.sentence_seq) {
            Some(old) if old.priority >= cue.priority => {}
            _ => {
                self.cues.insert(cue.sentence_seq, cue);
            }
        }
    }

    /// 查某句的 cue。
    pub fn cue_for(&self, seq: u64) -> Option<&Cue> {
        self.cues.get(&seq)
    }

    /// 全部 cue（按句号升序）。
    pub fn cues(&self) -> impl Iterator<Item = &Cue> {
        self.cues.values()
    }

    /// 当前持有的 cue 数。
    pub fn len(&self) -> usize {
        self.cues.len()
    }

    /// 是否没有 cue。
    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }

    /// 换轮：清空并指向新 epoch。
    pub fn reset(&mut self, epoch: u64) {
        self.epoch = epoch;
        self.covers_upto_seq = 0;
        self.cues.clear();
    }
}
