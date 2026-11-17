//! `[hotmem]` 段：主链热槽的外部资料（2026-11-16 热槽记忆 M5）。
//!
//! 从 `settings.rs` 拆出：那个文件贴着 code-stats 的 `>1000 行` 棘轮（上限 0），
//! 而这一段的文档比代码长得多——继续留在那里只会把整份配置模块推到限外。

use serde::{Deserialize, Serialize};

/// 见模块头注。
///
/// # 为什么只有路径一项
///
/// 热槽本身就是内存里的四块拼接，没有可调的旋钮；这个段存在的唯一理由是
/// **外挂资料要有一个「在哪里」的答案**。路径为空 = 不查资料。
///
/// # 只读文本，不是 Mod
///
/// 计划 §4：外挂知识库是配置目录里的文本文件，**不是 Mod**；查可以读块，写进
/// 热卡时每条压成一行 `[资料]`，原文留在文件里。资料条目没有强度 `S`，不参与
/// 遗忘——它每次都是现读现用。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HotmemSettings {
    /// 外挂资料的文本文件路径（UTF-8）。空串 = 不查资料（缺省）。
    ///
    /// 相对路径按**配置文件所在目录**解析（与 `mods.json` 同一条锚点纪律，
    /// 避免「cwd 不同就读不到」——那类缺陷在本仓已经出现过一次）。
    #[serde(default)]
    pub material_path: String,
}

impl HotmemSettings {
    /// 是否是出厂状态（整段省略）。
    pub fn is_default(&self) -> bool {
        self.material_path.trim().is_empty()
    }
}
