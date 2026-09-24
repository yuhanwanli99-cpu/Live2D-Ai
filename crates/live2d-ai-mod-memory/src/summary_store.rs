//! 摘要**旁车文件**（sidecar）：一个桶一份 JSON，可回滚、带版本（P1-5 真摘要）。
//!
//! # 为什么是独立文件而不是写进 memory JSONL
//!
//! memory JSONL 的形状是「一行一条记忆、只追加」，而摘要是**覆盖式状态**
//! （新版本替换旧版本、可回滚）。把摘要塞进 JSONL 会让「逐行读」多出一种行
//! 类型，也让物理淘汰（截断最旧 N 条）可能把摘要一起删掉。旁车文件把两种
//! 生命周期分开：
//!
//! ```text
//! <桶路径同目录>/
//!   memory.jsonl                  ← 原文（永远在，回滚就是回到它）
//!   memory.summary.json           ← 旁车：摘要版本栈 + 覆盖位点
//!   sessions/<id>.memory.jsonl    ← 会话桶原文
//!   sessions/<id>.summary.json    ← 会话桶旁车
//! ```
//!
//! 命名规则：[JsonlStore::path] 去掉 `.jsonl` 后缀再加 `.summary.json`，
//! 所以自定义 `store_path` 时旁车**跟着那个库走**，不会写去别处。
//!
//! # 可回滚（口径，别改）
//!
//! - `versions` **最新在前**（`versions[0]` = 当前生效版）；
//! - 每个版本自带 `covers_upto`（**它把前多少条原文压缩进了摘要**）——
//!   回滚 = 丢掉 `versions[0]`，新头的 `covers_upto` 一般更小，
//!   于是被那版覆盖的原文**重新变回「未被摘要覆盖」**、又能进 top-k 检索；
//! - 回滚**不删原文**：原文从头到尾都在 JSONL 里，这就是「回滚 = 用回原文」；
//! - 版本栈截到 [MAX_VERSIONS] 条：摘要不是用户内容的主体，栈的意义是
//!   「上一版还能回来」，不是无限历史。
//!
//! # 失败纪律
//!
//! 读不到 / 坏 JSON / 字段类型不对 → 视作**没有摘要**（`SummaryFile::default()`）
//! ＋调用方 warn。**绝不**因为旁车坏了就把这一轮对话打挂，也绝不 panic。
//! 写盘走「同目录临时文件 + rename」原子替换（与 [crate::store] 同一条纪律）。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::JsonlStore;

/// 旁车文件扩展名（替换 `.jsonl` 后缀）。
pub const SUMMARY_SUFFIX: &str = ".summary.json";
/// 版本栈上限（最新在前；超出丢最旧）。
pub const MAX_VERSIONS: usize = 10;

/// 一版摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryVersion {
    /// 单调递增的版本号（1 起；回滚后新写的版本继续 +1，**不重号**）。
    pub version: u64,
    /// 本版覆盖到的原文条数（该桶 JSONL 的**前 N 条**已进摘要）。
    ///
    /// 语义是「条数位点」，不是字节偏移：原文被物理淘汰时前若干条会消失，
    /// 读取方用 `covers_upto - 已淘汰条数` 才是当前有效位点（见
    /// [crate::summary::uncovered_start]）。
    pub covers_upto: usize,
    /// 覆盖到的最后一条原文的 turn（可读的「到哪一轮」，非权威）。
    #[serde(default)]
    pub covers_turn: u64,
    /// 生成时刻（Unix 秒）。
    pub created_at: i64,
    /// 摘要正文（已 sanitize；注入时直接用）。
    pub text: String,
}

/// 旁车文件的完整内容（一个桶一份）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryFile {
    /// 版本栈，**最新在前**（空 = 从未成功摘要过）。
    #[serde(default)]
    pub versions: Vec<SummaryVersion>,
    /// 累计成功写盘的版本数（含已被回滚掉的）。
    #[serde(default)]
    pub generated: u64,
    /// 上次失败的**可读原因**（成功后清空；失败≡无摘要，不改 versions）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// 上次尝试的时刻（成功或失败都更新）。
    #[serde(default)]
    pub last_attempt_at: i64,
}

impl SummaryFile {
    /// 当前生效版（`versions[0]`）。
    pub fn current(&self) -> Option<&SummaryVersion> {
        self.versions.first()
    }

    /// 当前覆盖位点（无摘要 = 0：什么都还没被压缩进摘要）。
    pub fn covers_upto(&self) -> usize {
        self.current().map(|v| v.covers_upto).unwrap_or(0)
    }

    /// 当前版本号（无摘要 = 0）。
    pub fn version(&self) -> u64 {
        self.current().map(|v| v.version).unwrap_or(0)
    }

    /// 追加一版（最新在前），并把栈截到 [MAX_VERSIONS]。
    pub fn push(&mut self, version: SummaryVersion) {
        self.generated += 1;
        self.last_error = None;
        self.versions.insert(0, version);
        self.versions.truncate(MAX_VERSIONS);
    }

    /// **回滚上一版**：丢掉当前生效版并返回被丢的那版
    /// （没有可回滚的 → `None`）。
    ///
    /// 原文一条不动——回滚之后被那版覆盖的原文重新可被检索。
    pub fn rollback(&mut self) -> Option<SummaryVersion> {
        if self.versions.is_empty() {
            return None;
        }
        self.last_error = None;
        Some(self.versions.remove(0))
    }

    /// 记录一次失败（**不动 versions**）。
    pub fn note_failure(&mut self, reason: impl Into<String>, now: i64) {
        self.last_error = Some(reason.into());
        self.last_attempt_at = now;
    }
}

/// 旁车文件句柄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryStore {
    path: PathBuf,
}

impl SummaryStore {
    /// 由**记忆库路径**派生旁车路径（`x.memory.jsonl` → `x.memory.summary.json`）。
    ///
    /// 没有 `.jsonl` 后缀时直接追加 [SUMMARY_SUFFIX]（不猜、不改原路径）。
    pub fn for_memory_store(store: &JsonlStore) -> Self {
        Self::from_memory_path(store.path())
    }

    /// [Self::for_memory_store] 的路径版（单测用）。
    pub fn from_memory_path(memory: &Path) -> Self {
        let stem = memory
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".jsonl"))
            .map(str::to_string);
        let file = match stem {
            Some(stem) => format!("{stem}{SUMMARY_SUFFIX}"),
            None => format!(
                "{}{SUMMARY_SUFFIX}",
                memory
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("memory")
            ),
        };
        let dir = memory.parent().unwrap_or_else(|| Path::new(""));
        Self {
            path: dir.join(file),
        }
    }

    /// 旁车文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 读取旁车；**文件不存在 = 空文件**（首次启用本来就没有）。
    ///
    /// 坏 JSON / 坏字段 → `Err`（调用方视作「无摘要」＋ warn）。
    pub fn load(&self) -> std::io::Result<SummaryFile> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SummaryFile::default()),
            Err(e) => Err(e),
        }
    }

    /// 原子写盘（同目录 `.tmp` + `rename`；缺父目录则创建）。
    pub fn save(&self, file: &SummaryFile) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            let raw = serde_json::to_string_pretty(file)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            let mut handle = File::create(&tmp)?;
            handle.write_all(raw.as_bytes())?;
            handle.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
    }

    /// 删除旁车（**回滚也不用它**：回滚只丢版本，不删文件）。
    ///
    /// 给「清空该桶」用：桶被清空了，摘要留着就会描述一段不存在的历史。
    pub fn remove(&self) -> std::io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir()
            .join(format!("l2d-summary-{tag}-{}-{nanos}", std::process::id()))
            .join("memory.jsonl")
    }

    #[test]
    fn sidecar_path_is_derived_next_to_the_bucket() {
        let store = JsonlStore::new("/tmp/a/sessions/x-1.memory.jsonl");
        let side = SummaryStore::for_memory_store(&store);
        assert_eq!(
            side.path(),
            Path::new("/tmp/a/sessions/x-1.memory.summary.json")
        );
        // 没有 .jsonl 后缀也不 panic，只是追加后缀。
        let odd = SummaryStore::from_memory_path(Path::new("/tmp/a/weird"));
        assert_eq!(odd.path(), Path::new("/tmp/a/weird.summary.json"));
    }

    #[test]
    fn missing_sidecar_is_empty_and_bad_json_is_an_error() {
        let path = temp_path("missing");
        let store = SummaryStore::from_memory_path(&path);
        assert_eq!(
            store.load().expect("missing = empty"),
            SummaryFile::default()
        );

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(store.path(), "{ not json").unwrap();
        assert!(store.load().is_err(), "坏 JSON 必须报错，而不是冒充空摘要");
    }

    #[test]
    fn save_load_round_trip_and_versions_are_newest_first() {
        let path = temp_path("roundtrip");
        let store = SummaryStore::from_memory_path(&path);
        let mut file = SummaryFile::default();
        file.push(SummaryVersion {
            version: 1,
            covers_upto: 4,
            covers_turn: 4,
            created_at: 100,
            text: "第一版摘要".to_string(),
        });
        file.push(SummaryVersion {
            version: 2,
            covers_upto: 9,
            covers_turn: 9,
            created_at: 200,
            text: "第二版摘要".to_string(),
        });
        store.save(&file).expect("写盘");
        let loaded = store.load().expect("读回");
        assert_eq!(loaded, file);
        assert_eq!(loaded.version(), 2);
        assert_eq!(loaded.covers_upto(), 9);
        assert_eq!(loaded.generated, 2);
        assert_eq!(loaded.versions.len(), 2);
        assert_eq!(loaded.versions[1].text, "第一版摘要");
        // 临时文件不残留。
        assert!(!store.path().with_extension("json.tmp").exists());
    }

    fn two_versions() -> SummaryFile {
        let mut file = SummaryFile::default();
        file.push(SummaryVersion {
            version: 1,
            covers_upto: 3,
            covers_turn: 3,
            created_at: 1,
            text: "旧".to_string(),
        });
        file.push(SummaryVersion {
            version: 2,
            covers_upto: 8,
            covers_turn: 8,
            created_at: 2,
            text: "新".to_string(),
        });
        file
    }

    #[test]
    fn rollback_drops_the_newest_version_and_restores_the_older_one() {
        let mut file = two_versions();
        assert_eq!(file.version(), 2);
        let dropped = file.rollback().expect("有可回滚的版本");
        assert_eq!(dropped.version, 2);
        assert_eq!(dropped.covers_upto, 8);
        assert_eq!(file.version(), 1, "回滚后生效版是上一版");
        assert_eq!(file.covers_upto(), 3, "覆盖位点退回上一版");
        assert_eq!(file.versions.len(), 1);
        let last = file.rollback();
        assert_eq!(last.map(|v| v.version), Some(1));
        assert!(file.versions.is_empty());
        assert_eq!(file.version(), 0);
        assert_eq!(file.covers_upto(), 0);
        assert_eq!(file.rollback(), None, "空栈再回滚仍是 None（不 panic）");
    }

    #[test]
    fn rollback_round_trips_through_disk() {
        let path = temp_path("rollback");
        let store = SummaryStore::from_memory_path(&path);
        let mut file = two_versions();
        store.save(&file).expect("写盘");
        file.rollback();
        store.save(&file).expect("回滚后写盘");
        let loaded = store.load().expect("读回");
        assert_eq!(loaded.version(), 1);
        assert_eq!(
            loaded.generated, 2,
            "generated 记的是历史写入次数，不随回滚减小"
        );
    }

    #[test]
    fn versions_are_capped_and_failures_never_touch_them() {
        let mut file = SummaryFile::default();
        for i in 1..=(MAX_VERSIONS as u64 + 3) {
            file.push(SummaryVersion {
                version: i,
                covers_upto: i as usize,
                covers_turn: i,
                created_at: i as i64,
                text: format!("v{i}"),
            });
        }
        assert_eq!(file.versions.len(), MAX_VERSIONS);
        assert_eq!(file.version(), MAX_VERSIONS as u64 + 3);
        assert_eq!(file.generated, MAX_VERSIONS as u64 + 3);

        file.note_failure("上游 500", 42);
        assert_eq!(file.last_error.as_deref(), Some("上游 500"));
        assert_eq!(file.last_attempt_at, 42);
        assert_eq!(
            file.versions.len(),
            MAX_VERSIONS,
            "失败不得改版本栈（失败 ≡ 无摘要）"
        );
        // 成功后清掉 last_error。
        file.push(SummaryVersion {
            version: 99,
            covers_upto: 1,
            covers_turn: 1,
            created_at: 43,
            text: "ok".to_string(),
        });
        assert!(file.last_error.is_none());
    }
}
