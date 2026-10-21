//! `mods.json` 原子写回（rc.4 M1）与它的失败文案（S-003，2026-10-16）。
//!
//! 从 registry.rs 拆出：本文件只放「把注册表 / 文件夹勾选写回磁盘」这一件事，
//! 以及失败时那句必须说清后果的文案。0.3.12 起多一条**只覆写文件夹 id 的
//! `enabled`** 的写回（悬挂在 `persist_folder_selection`，不经过工厂表）。

use super::*;

impl ModRegistry {
    /// 读一份用于写回的根文档：**磁盘当前内容**（读不到 / 坏 JSON 时退回
    /// 构造时那份原始文档），并保证 `mods` 是对象。
    ///
    /// F-0013-01 / F-0644-01：以磁盘为基底、只覆写本进程拥有的 id——用户按
    /// 文档手写的「当前不存在的 Mod id」与其它顶层键必须在写回后原样保留。
    fn manifest_root_for_write(&self) -> serde_json::Value {
        let mut root = self
            .manifest_path
            .as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
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
        root
    }

    /// 原子写回一份根文档（tmp + fdatasync + rename，与 `live2d-ai.toml` 同一条
    /// 原子写纪律）。失败**上抛**（S-003：内存变了、磁盘没变必须让调用方知道）。
    fn write_manifest_root(&self, root: &serde_json::Value) -> Result<(), ModError> {
        let Some(path) = self.manifest_path.as_deref() else {
            return Ok(());
        };
        let text = match serde_json::to_string_pretty(root) {
            Ok(text) => text,
            Err(err) => {
                tracing::error!(target: "mod", "mods.json 序列化失败: {err}");
                return Err(persist_failed(&format!("序列化失败：{err}")));
            }
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            tracing::error!(target: "mod", path = %path.display(), "mods.json 目录创建失败: {err}");
            return Err(persist_failed(&format!("目录创建失败：{err}")));
        }
        match live2d_ai_runtime::settings::patch::plan_atomic_write(path, &text) {
            Ok(tmp) => {
                if let Err(err) = std::fs::rename(&tmp, path) {
                    let _ = std::fs::remove_file(&tmp);
                    tracing::error!(target: "mod", path = %path.display(), "mods.json rename 失败: {err}");
                    return Err(persist_failed(&format!("rename 失败：{err}")));
                }
            }
            Err(err) => {
                tracing::error!(target: "mod", path = %path.display(), "mods.json 写盘失败: {err}");
                return Err(persist_failed(&format!("写盘失败：{err}")));
            }
        }
        Ok(())
    }

    /// 原子写回 `mods.json`（rc.4 M1）。
    ///
    /// 写入内容 = **当前全部注册 Mod** 的 `enabled` + `config`（含未启用者，
    /// 否则重新启用另一个 Mod 会把前一个的状态丢掉）。
    ///
    /// - 未注入路径（单测 / 纯内存）→ no-op；
    /// - 以磁盘当前内容为基底，**只覆写**在册 id（见 `manifest_root_for_write`）；
    /// - **失败要上抛**（S-003，2026-10-16）：从前是 best-effort——失败只记
    ///   warn 日志、函数返回 `()`，于是 HTTP 200 + 界面「已启用 / 已保存」，
    ///   而磁盘上什么都没变（重启后开关回到旧值）。
    pub(super) fn persist_manifest(&self) -> Result<(), ModError> {
        if self.manifest_path.is_none() {
            return Ok(());
        }
        let mut root = self.manifest_root_for_write();
        let mods = root
            .get_mut("mods")
            .and_then(serde_json::Value::as_object_mut)
            .expect("刚刚保证是对象");
        for (id, entry) in &self.entries {
            mods.insert(
                (*id).to_string(),
                serde_json::json!({ "enabled": entry.enabled, "config": entry.config }),
            );
        }
        self.write_manifest_root(&root)
    }

    /// 当前 `mods.json` 文档（**只读**；未注入路径 / 坏 JSON 时回构造时那份）。
    ///
    /// 0.3.12 的启动器页用它决定文件夹行的**初始勾选**（执行单 `4：该 id 在
    /// `mods.json` 里 `enabled = true` 才勾）；写回走 `persist_folder_selection`。
    pub fn manifest_value(&self) -> serde_json::Value {
        self.manifest_path
            .as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| self.raw_manifest.clone())
    }

    /// 把**文件夹清单**的勾选写回 `mods.json`（0.3.12，执行单 `4 / `6）。
    ///
    /// - 只覆写 `ids` 这些 id 的 `enabled`（`ids` = 本次扫描里**可选**的文件夹
    ///   id，由扫描模块给出；**不碰** `entries` / 工厂表 / 其它键）；
    /// - `enabled` 里有的 id 写 true，其余写 false（勾选与磁盘一致）；
    /// - 已有 entry 的 `config` 等键原样保留；不认识的老 id 原样保留；
    /// - 没有任何变化 → 不重写文件；未注入路径 → no-op（单测）；
    /// - 返回**实际改动的 id 数**（0 = 没写盘）。
    ///
    /// 这是「点进入对文件夹只记下勾选」的落盘点：**不** enable / disable，
    /// **不** spawn（执行单 `4）。
    pub fn persist_folder_selection(
        &self,
        ids: &[String],
        enabled: &[String],
    ) -> Result<usize, ModError> {
        if ids.is_empty() || self.manifest_path.is_none() {
            return Ok(0);
        }
        let mut root = self.manifest_root_for_write();
        let mods = root
            .get_mut("mods")
            .and_then(serde_json::Value::as_object_mut)
            .expect("刚刚保证是对象");
        let mut changed = 0usize;
        for id in ids {
            let want = enabled.iter().any(|selected| selected == id);
            let slot = mods
                .entry(id.clone())
                .or_insert_with(|| serde_json::json!({}));
            if !slot.is_object() {
                // 手写坏值（如 "id": true）：这份 entry 本来就装不下 enabled，
                // 只能换成一个对象——只影响这一条 id。
                *slot = serde_json::json!({});
            }
            let current = slot.get("enabled").and_then(serde_json::Value::as_bool);
            if current != Some(want) {
                changed += 1;
            }
            slot["enabled"] = serde_json::json!(want);
        }
        if changed == 0 {
            return Ok(0);
        }
        self.write_manifest_root(&root)?;
        Ok(changed)
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
