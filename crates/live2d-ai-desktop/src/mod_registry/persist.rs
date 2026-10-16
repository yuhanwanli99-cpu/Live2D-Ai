//! `mods.json` 原子写回（rc.4 M1）与它的失败文案（S-003，2026-10-16）。
//!
//! 从 registry.rs 拆出：本文件只放「把当前注册表写回磁盘」这一件事，
//! 以及它失败时那句必须说清后果的文案。**行为一行未改**。

use super::*;

impl ModRegistry {
    /// 原子写回 `mods.json`（rc.4 M1）。
    ///
    /// 写入内容 = **当前全部注册 Mod** 的 `enabled` + `config`（含未启用者，
    /// 否则重新启用另一个 Mod 会把前一个的状态丢掉）。
    ///
    /// - 未注入路径（单测 / 纯内存）→ no-op；
    /// - 复用 `live2d-ai-runtime` 的 `plan_atomic_write`（tmp + fdatasync + rename，
    ///   与 `live2d-ai.toml` 同一条原子写纪律）；
    /// - **失败要上抛**（S-003，2026-10-16）：从前是 best-effort——失败只记
    ///   `error` 日志、函数返回 `()`，于是 HTTP 200 + 界面「已启用 / 已保存」，
    ///   而磁盘上什么都没变（重启后开关回到旧值）。内存状态确实已经变了
    ///   （这一进程里操作**是真的**），但「下次重启还在」没有保证——调用方必须
    ///   拿到这个 `Err`，才能不把它说成成功。
    pub(super) fn persist_manifest(&self) -> Result<(), ModError> {
        let Some(path) = self.manifest_path.as_deref() else {
            return Ok(());
        };
        // F-0013-01 / F-0644-01：以**磁盘当前内容**为基底（读不到 / 坏 JSON 时退回
        // 构造时那份原始文档），**只覆写**本进程真正拥有的 id。
        //
        // 从前的实现整份重建 {"mods": {...}}，于是用户按文档手写的「当前不存在的
        // Mod id」与其它顶层键，会在第一次写回时被静默抹掉——那是数据丢失，
        // 不是清理（docs/external-input.md 两处明确邀请用户改这个文件）。
        let mut root = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| self.raw_manifest.clone());
        if !root.is_object() {
            root = serde_json::Value::Object(serde_json::Map::new());
        }
        let root_obj = root.as_object_mut().expect("刚刚保证是对象");
        if !root_obj
            .get("mods")
            .is_some_and(serde_json::Value::is_object)
        {
            root_obj.insert(
                "mods".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        let mods = root_obj
            .get_mut("mods")
            .and_then(|v| v.as_object_mut())
            .expect("刚刚保证是对象");
        for (id, e) in &self.entries {
            mods.insert(
                (*id).to_string(),
                serde_json::json!({ "enabled": e.enabled, "config": e.config }),
            );
        }
        let doc = root;
        let text = match serde_json::to_string_pretty(&doc) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(target: "mod", "mods.json 序列化失败: {e}");
                return Err(persist_failed(&format!("序列化失败：{e}")));
            }
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::error!(target: "mod", path = %path.display(), "mods.json 目录创建失败: {e}");
            return Err(persist_failed(&format!("目录创建失败：{e}")));
        }
        match live2d_ai_runtime::settings::patch::plan_atomic_write(path, &text) {
            Ok(tmp) => {
                if let Err(e) = std::fs::rename(&tmp, path) {
                    let _ = std::fs::remove_file(&tmp);
                    tracing::error!(target: "mod", path = %path.display(), "mods.json rename 失败: {e}");
                    return Err(persist_failed(&format!("rename 失败：{e}")));
                }
            }
            Err(e) => {
                tracing::error!(target: "mod", path = %path.display(), "mods.json 写盘失败: {e}");
                return Err(persist_failed(&format!("写盘失败：{e}")));
            }
        }
        Ok(())
    }
}

/// `mods.json` 写盘失败的错误文案（S-003）：一句话说清「内存变了、磁盘没变」。
///
/// 措辞刻意带上前因后果——用户看到的 toast 只有这一句，重启后状态会回退是
/// 最难事后解释的一件事。
fn persist_failed(what: &str) -> ModError {
    ModError::Other(format!(
        "mods.json {what}（本次改动只留在内存里，重启后会回到磁盘上的旧值）"
    ))
}
