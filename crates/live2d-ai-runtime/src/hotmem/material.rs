//! 外挂资料：按块取候选、压成 `[资料]` 行（2026-11-16 热槽记忆 M5）。
//!
//! # 它是什么 / 不是什么
//!
//! - **是**：配置目录里的一个 UTF-8 文本文件。查的时候按块取候选，写进热卡时每条
//!   压成一行 `- [资料] …`，**原文留在文件里**；
//! - **不是** Mod、不是向量库、不是索引服务。没有嵌入、没有持久化、没有后台任务。
//!
//! # 与保留率无关
//!
//! 事实有三档（清楚 / 模糊 / 不进卡），资料**没有档**：它要么进 `[资料]` 行，
//! 要么不进卡。也没有强度 `S`、不参与遗忘——每次都是现读现用。
//!
//! # 预算
//!
//! 资料与事实**共用**热卡那 200 token：先放事实行，再用剩余预算放资料行。
//! 计划 §4「受热卡剩余预算限制」即此意。
//!
//! # 没有向量时热卡只剩资料
//!
//! 事实行只能来自本轮检索命中；本机没有 embedding 时检索给不出命中，于是
//! 热卡要么只有 `[资料]` 行、要么是空卡。**不要**为此退回「按 `R` 铺事实库」
//! ——那会把不相关的旧事每轮都带进请求（2026-11-16 维护者裁决）。

use super::card::{CardLine, CardTier, HotCard};

/// 资料切块的最大字符数（一块 = 一个候选）。
///
/// 取值理由：**单块必须能独自进得去 200 token 的热卡**（一整行收，不截半行），
/// 所以上限取 120 —— 120 个 CJK 字符 + `- [资料] ` 前缀仍远低于 200。
/// 300 字的块看着更像「一段知识」，但它单独就超预算，会让整卡为空。
pub const MATERIAL_CHUNK_CHARS: usize = 120;

/// 一次最多取多少块候选（防止 5000 字文件产出几十条把热卡挤满）。
pub const MATERIAL_MAX_CHUNKS: usize = 6;

/// 把一份文本切成块（按空行分段，再对超长段做硬切）。
///
/// 规则：
/// - 空行分段（`\n\n`）是一等公民——外挂资料几乎总是「一段一条」；
/// - 单段超过 [MATERIAL_CHUNK_CHARS] 就**硬切**（保证预算可控，也保证一段里的
///   特征句不会因为整段太长而永远进不了卡）；
/// - 空白块丢弃。
pub fn split_material(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for para in text.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if para.chars().count() <= MATERIAL_CHUNK_CHARS {
            out.push(para.replace('\n', " "));
            continue;
        }
        let chars: Vec<char> = para.chars().collect();
        for chunk in chars.chunks(MATERIAL_CHUNK_CHARS) {
            let piece: String = chunk.iter().collect();
            let piece = piece.trim();
            if !piece.is_empty() {
                out.push(piece.replace('\n', " "));
            }
        }
    }
    out
}

/// 把候选块压成 `[资料]` 行，并**整块**与调用方给的事实行一起装进热卡。
///
/// 参数：
/// - `facts`：**本轮检索命中**的事实行（顺序即优先级）。没有向量检索时
///   调用方传空表——保留率不是相关性（见 [super::card] 头注），
///   本模块**不会**自己去事实库里按 `R` 挑；
/// - `material`：外挂资料切出来的块（调用方已按 [split_material] 切好）；
/// - `recent`：最近几轮原文（与热卡同一套去重口径）。
///
/// 返回装好的热卡：事实行在前，资料行吃剩余预算（超了就丢，**不截半行**）。
pub fn compose_card(facts: Vec<CardLine>, material: &[String], recent: &[String]) -> HotCard {
    let mut candidates = facts;
    for chunk in material.iter().take(MATERIAL_MAX_CHUNKS) {
        let text = chunk.trim();
        if text.is_empty() {
            continue;
        }
        candidates.push(CardLine::new(CardTier::Material, text));
    }
    HotCard::from_lines(candidates, recent)
}
