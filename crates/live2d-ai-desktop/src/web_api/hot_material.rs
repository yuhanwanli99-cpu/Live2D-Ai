//! 外挂资料的**读盘半边**（2026-11-16 热槽记忆 M5）。
//!
//! # 边界
//!
//! - **只读 UTF-8 文本**：不是数据库、不是索引、不写回；
//! - **只在「查」时读**（POST /api/v1/chat/draft）：说话路径不碰磁盘——
//!   热卡是草稿时代算好的，说的时候只读内存；
//! - **路径为空 = 不查资料**（计划 §6 M5「缺省空 = 不查资料」）。
//!
//! # 失败怎么办
//!
//! 读不到 / 不是 UTF-8 / 太大 → **当作没有资料**，返回 None，不报错、不重试。
//! 资料是「锦上添花」的能力：它坏了不该让草稿端点失败，更不该让说话失败。
//!
//! # 路径解析
//!
//! 相对路径按**配置文件所在目录**解析（与 mods.json 同一条锚点纪律）。绝对
//! 路径原样用。这条规则写在这里而不是散在调用点——「cwd 不同就读不到」那类缺陷
//! 在本仓已经出现过一次。

use std::path::{Path, PathBuf};

/// 资料文件的大小上限（字节）。
///
/// 取值理由：外挂资料是**给人写的笔记**，不是语料库。1 MiB 足够几千字，又能挡住
/// 「误把整本书/整个日志目录指过来」——那会让每次草稿都读一个大文件。
pub const MAX_MATERIAL_BYTES: u64 = 1024 * 1024;

/// 解析后的资料路径（None = 没配 / 配了个空串）。
pub fn resolve_material_path(config_path: &str, configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if configured.is_empty() {
        return None;
    }
    let path = Path::new(configured);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    match Path::new(config_path).parent() {
        Some(dir) if !dir.as_os_str().is_empty() => Some(dir.join(path)),
        _ => Some(path.to_path_buf()),
    }
}

/// 读资料文件并切块（失败 = None，**不报错**）。
///
/// 返回的块已经过 split_material：按空行分段、超长段硬切。
pub fn load_material(config_path: &str, configured: &str) -> Option<Vec<String>> {
    let path = resolve_material_path(config_path, configured)?;
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_MATERIAL_BYTES {
        tracing::debug!(
            path = %path.display(),
            bytes = meta.len(),
            "外挂资料不可用（不是文件或超过上限），本轮不查资料"
        );
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let chunks = live2d_ai_runtime::hotmem::split_material(&text);
    if chunks.is_empty() {
        return None;
    }
    Some(chunks)
}
