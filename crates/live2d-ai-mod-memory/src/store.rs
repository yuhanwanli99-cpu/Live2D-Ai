//! 本地 JSONL 存储（**唯一**持久化面）：追加一行 = 记住一条，逐行读 = 载入。
//!
//! # 为什么是 JSONL
//!
//! 追加写、逐行读、坏行局部化——没有索引、没有事务、没有依赖。记忆是
//! **只增不改**的数据，JSONL 与它的形状天然匹配；换成 SQLite 会带来一个
//! 数据库依赖，而 v0 的检索规模（`max_records` ≤ 10000）用不上。
//!
//! # 坏行纪律（**不许 panic、不许静默**）
//!
//! - 空行 → 跳过，**不计**坏行（正常文件尾常见）；
//! - 非 JSON / 缺 `text` / `text` 空白 → 计入 [`LoadOutcome::bad_lines`] 并跳过；
//! - 读 `File::open` 时 `NotFound` → 返回空结果（首次启用本来就没有文件）；
//! - 其它 IO 错误 → 由调用方决定（本 Mod 的选择是 warn + 本轮 no-op，见 `crate`）。
//!
//! `bad_lines` 会被上层写进日志——「跳过了几条」这件事必须可见，
//! 否则损坏的库表现为「记忆忽然变少」而没有任何线索。

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::strategy::MemoryRecord;

/// 一次载入的结果：可用记录 + 被跳过的坏行数。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadOutcome {
    pub records: Vec<MemoryRecord>,
    /// 被跳过并计数的坏行（不含空行）。
    pub bad_lines: usize,
}

/// 单个 JSONL 文件的读写句柄（无缓存、无锁；每次调用自开自关）。
///
/// 无内部状态是有意的：Mod worker 线程独占调用它，`state_json` 走内存计数、
/// 不碰这个结构，因此不存在跨线程共享可变状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlStore {
    path: PathBuf,
}

impl JsonlStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// 文件路径（日志/状态用）。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 追加一条记忆（缺父目录则创建）。返回 IO 结果，**不**在此处吞错。
    pub fn append(&self, record: &MemoryRecord) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let value = serde_json::json!({
            "text": record.text,
            "ts": record.ts,
            "turn": record.turn,
        });
        let line = serde_json::to_string(&value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")
    }

    /// 载入记录，**只保留最新的 `max_records` 条**（检索窗口，不物理删除历史文件）。
    ///
    /// `max_records == 0` 视为「不设窗口」（调用方一般已钳位 ≥ 1）。
    pub fn load(&self, max_records: usize) -> std::io::Result<LoadOutcome> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadOutcome::default());
            }
            Err(e) => return Err(e),
        };
        let mut outcome = LoadOutcome::default();
        for line in BufReader::new(file).lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => {
                    // 读一行就失败（非法 UTF-8 / IO 错）：计坏行并继续——
                    // 不能因为一行坏了就丢掉整个库。
                    outcome.bad_lines += 1;
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match parse_record_line(&line) {
                Some(record) => outcome.records.push(record),
                None => outcome.bad_lines += 1,
            }
        }
        if max_records > 0 && outcome.records.len() > max_records {
            let cut = outcome.records.len() - max_records;
            outcome.records.drain(..cut);
        }
        Ok(outcome)
    }
}

/// 解析一行 JSONL。任何形状不对的行都返回 `None`（由 [`JsonlStore::load`] 计数）。
///
/// `ts` / `turn` 缺失按 `0` 处理（旧文件向前兼容）；`text` 缺失或空白 → `None`。
pub fn parse_record_line(line: &str) -> Option<MemoryRecord> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let text = value.get("text")?.as_str()?.to_string();
    if text.trim().is_empty() {
        return None;
    }
    let ts = value
        .get("ts")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let turn = value
        .get("turn")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    Some(MemoryRecord { text, ts, turn })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// 每个测试一个独立临时目录（不引 tempfile 依赖）。
    fn temp_store(tag: &str) -> (PathBuf, JsonlStore) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "l2d-memory-store-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = JsonlStore::new(dir.join("memory.jsonl"));
        (dir, store)
    }

    fn rec(text: &str, ts: i64, turn: u64) -> MemoryRecord {
        MemoryRecord {
            text: text.to_string(),
            ts,
            turn,
        }
    }

    #[test]
    fn append_and_load_round_trip() {
        let (dir, store) = temp_store("roundtrip");
        store.append(&rec("第一条", 100, 1)).unwrap();
        store.append(&rec("第二条", 200, 2)).unwrap();
        let outcome = store.load(10).unwrap();
        assert_eq!(outcome.records.len(), 2);
        assert_eq!(outcome.bad_lines, 0);
        assert_eq!(outcome.records[0], rec("第一条", 100, 1));
        assert_eq!(outcome.records[1], rec("第二条", 200, 2));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_missing_file_is_empty_not_error() {
        let (dir, store) = temp_store("missing");
        let outcome = store.load(10).unwrap();
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.bad_lines, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_tolerates_bad_lines_and_counts_them() {
        let (dir, store) = temp_store("badlines");
        store.append(&rec("好行一", 1, 1)).unwrap();
        {
            let mut f = OpenOptions::new().append(true).open(store.path()).unwrap();
            writeln!(f, "这不是 JSON").unwrap();
            writeln!(f, "{{\"text\":\"\"}}").unwrap();
            writeln!(f).unwrap(); // 空行：不计坏行
            writeln!(f, "{{\"ts\":5}}").unwrap(); // 缺 text
        }
        store.append(&rec("好行二", 2, 2)).unwrap();
        let outcome = store.load(10).unwrap();
        assert_eq!(outcome.records.len(), 2, "两条好行都还在");
        assert_eq!(outcome.bad_lines, 3, "三条坏行被计数");
        assert_eq!(outcome.records[1].text, "好行二");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_keeps_only_newest_max_records_window() {
        let (dir, store) = temp_store("window");
        for i in 1..=5 {
            store
                .append(&rec(&format!("第{i}条"), i as i64, i as u64))
                .unwrap();
        }
        let outcome = store.load(2).unwrap();
        assert_eq!(outcome.records.len(), 2);
        assert_eq!(outcome.records[0].text, "第4条");
        assert_eq!(outcome.records[1].text, "第5条");
        // 窗口只影响载入，不物理删除文件。
        assert_eq!(store.load(0).unwrap().records.len(), 5);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_creates_missing_parent_directory() {
        let (dir, _) = temp_store("mkdir");
        let nested = JsonlStore::new(dir.join("a").join("b").join("memory.jsonl"));
        nested.append(&rec("嵌套", 1, 1)).unwrap();
        assert_eq!(nested.load(10).unwrap().records.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parse_record_line_shapes() {
        assert!(parse_record_line("not json").is_none());
        assert!(parse_record_line("[]").is_none());
        assert!(parse_record_line("{\"text\":\"  \"}").is_none());
        let ok = parse_record_line("{\"text\":\"x\",\"ts\":7,\"turn\":3}").unwrap();
        assert_eq!(ok, rec("x", 7, 3));
        // ts / turn 缺失 → 0（向前兼容）。
        assert_eq!(
            parse_record_line("{\"text\":\"x\"}").unwrap(),
            rec("x", 0, 0)
        );
    }
}
