//! 本地 JSONL 存储（**唯一**持久化面）：追加一行 = 记住一条，逐行读 = 载入。
//!
//! # 为什么是 JSONL
//!
//! 追加写、逐行读、坏行局部化——没有索引、没有事务、没有依赖。记忆以**追加**
//! 为主（Wave 3 起唯一例外是超过条数上限时的物理重写，见 `append_capped`），
//! JSONL 与它的形状天然匹配；换成 SQLite 会带来一个数据库依赖，而 v0 的检索
//! 规模（`max_records` ≤ 10000）用不上。
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

/// 一次「带上限的追加」的结果（Wave 3 物理淘汰的可观察面）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppendOutcome {
    /// 追加后文件里保留的有效记录条数（≤ 上限）。
    pub kept: usize,
    /// 因超过上限被**物理删除**的条数（0 = 本轮没淘汰）。
    pub evicted: usize,
    /// 本次载入跳过的坏行数（重写会顺带清掉坏行）。
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

    /// 追加一条，并在超过 `max_records` 时**物理淘汰**最旧记录（Wave 3）。
    ///
    /// 语义（取舍见 `docs/architecture/memory-mod-v0.md` §6.1）：
    /// - `max_records == 0` → 只追加、不淘汰（与 [`Self::load`] 的 0 对称）；
    /// - 追加后有效记录 ≤ 上限 → 文件一字不改（返回 `evicted: 0`）；
    /// - 超过上限 → 保留**最新** `max_records` 条，同目录临时文件 + `rename`
    ///   原子替换；坏行一并清掉（重写只输出可解析的记录）。
    ///
    /// **数据丢失风险**：没有备份语义——被淘汰的记录不可恢复。换取的是文件
    /// 有界：检索 / 注入不会随历史无限变慢，`bad_lines` 也有机会被清走。
    pub fn append_capped(
        &self,
        record: &MemoryRecord,
        max_records: usize,
    ) -> std::io::Result<AppendOutcome> {
        self.append(record)?;
        let loaded = self.load(0)?;
        if max_records == 0 || loaded.records.len() <= max_records {
            return Ok(AppendOutcome {
                kept: loaded.records.len(),
                evicted: 0,
                bad_lines: loaded.bad_lines,
            });
        }
        let cut = loaded.records.len() - max_records;
        self.rewrite(&loaded.records[cut..])?;
        Ok(AppendOutcome {
            kept: max_records,
            evicted: cut,
            bad_lines: loaded.bad_lines,
        })
    }

    /// 用给定记录**原子重写**整个文件：写同目录临时文件 → `rename`。
    ///
    /// `rename` 在同一文件系统内是原子的，因此崩溃/断电最多留下一个无关的
    /// `.jsonl.tmp`，不会把正式文件截成半截。
    fn rewrite(&self, records: &[MemoryRecord]) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        {
            let mut file = File::create(&tmp)?;
            for record in records {
                let value = serde_json::json!({
                    "text": record.text,
                    "ts": record.ts,
                    "turn": record.turn,
                });
                let line = serde_json::to_string(&value)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                writeln!(file, "{line}")?;
            }
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
    }

    /// **原子清空**记忆库：把文件重写成**空**（同目录 `.tmp` + `rename`）。
    ///
    /// 为什么不是 `remove_file`：与 [`Self::rewrite`] 同一条原子纪律。
    /// 直接删文件会在「删除」与「下一次追加」之间留下一个**不存在的库**
    /// （`load` 会把 `NotFound` 当空库，看似等价），但一旦删除后进程崩在
    /// 「刚删、还没重建」之间，读者就分不清「清空成功」与「库丢了」；
    /// `rename` 是**原子替换**：读者要么看到旧内容、要么看到空文件。
    ///
    /// 文件本来不存在也会创建一个**空文件**——「空库」与「没有库」都算 0 条，
    /// 但前者让「清空」这件事在文件系统上可见（大小 0、存在）。
    pub fn clear(&self) -> std::io::Result<()> {
        self.rewrite(&[])
    }

    /// 载入记录，**只保留最新的 `max_records` 条**。
    ///
    /// 本方法**只缩载入窗口、不动文件**；真正的物理删除在
    /// [`Self::append_capped`]（Wave 3 起运行时唯一写入路径）。
    /// 因此坏行 / 外部手改进来的多余记录不会因为 `load` 而消失。
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
    fn append_capped_truncates_file_to_cap_keeping_newest() {
        let (dir, store) = temp_store("cap");
        let cap = 3;
        let mut evicted_total = 0;
        for i in 1..=5 {
            let outcome = store
                .append_capped(&rec(&format!("第{i}条"), i as i64, i as u64), cap)
                .expect("append_capped 应成功");
            assert!(outcome.kept <= cap, "返回的保留数不得超过上限");
            evicted_total += outcome.evicted;
        }
        assert_eq!(evicted_total, 2, "第 4、5 条各挤掉 1 条最旧");
        let all = store.load(0).unwrap();
        assert_eq!(all.records.len(), cap, "物理文件里只剩 N 条");
        let texts: Vec<&str> = all.records.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["第3条", "第4条", "第5条"], "保留最新的 N 条");
        // 裸读文件行数也必须是 N（不是靠 load 窗口装出来的）。
        let raw = fs::read_to_string(store.path()).unwrap();
        assert_eq!(raw.lines().count(), cap, "文件行数 = N，物理淘汰");
        assert!(!raw.contains("第1条") && !raw.contains("第2条"));
        assert!(
            !dir.join("memory.jsonl.tmp").exists(),
            "rename 之后不得残留临时文件"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_capped_under_cap_does_not_evict() {
        let (dir, store) = temp_store("cap-under");
        let first = store.append_capped(&rec("一", 1, 1), 10).unwrap();
        assert_eq!((first.kept, first.evicted), (1, 0));
        let second = store.append_capped(&rec("二", 2, 2), 10).unwrap();
        assert_eq!((second.kept, second.evicted), (2, 0));
        assert_eq!(store.load(0).unwrap().records.len(), 2);
        assert!(!dir.join("memory.jsonl.tmp").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_capped_zero_cap_means_no_eviction() {
        let (dir, store) = temp_store("cap-zero");
        for i in 1..=4 {
            let outcome = store
                .append_capped(&rec(&format!("第{i}条"), i, i as u64), 0)
                .unwrap();
            assert_eq!(outcome.evicted, 0, "0 = 不淘汰（与 load(0) 对称）");
        }
        assert_eq!(store.load(0).unwrap().records.len(), 4);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_capped_rewrite_purges_bad_lines() {
        let (dir, store) = temp_store("cap-bad");
        store.append(&rec("好一", 1, 1)).unwrap();
        {
            let mut f = OpenOptions::new().append(true).open(store.path()).unwrap();
            writeln!(f, "这不是 JSON").unwrap();
        }
        store.append(&rec("好二", 2, 2)).unwrap();
        // cap=2：追加第三条好行后超限 → 重写只输出可解析记录，坏行被清掉。
        let outcome = store.append_capped(&rec("好三", 3, 3), 2).unwrap();
        assert_eq!(outcome.evicted, 1);
        assert_eq!(outcome.bad_lines, 1, "重写前看到 1 条坏行");
        let after = store.load(0).unwrap();
        assert_eq!(after.bad_lines, 0, "重写后坏行已被清掉");
        let texts: Vec<&str> = after.records.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["好二", "好三"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn append_capped_cap_one_keeps_only_the_last_write() {
        let (dir, store) = temp_store("cap-one");
        for i in 1..=3 {
            store
                .append_capped(&rec(&format!("第{i}条"), i, i as u64), 1)
                .unwrap();
        }
        let all = store.load(0).unwrap();
        assert_eq!(all.records.len(), 1);
        assert_eq!(all.records[0].text, "第3条");
        assert_eq!(fs::read_to_string(store.path()).unwrap().lines().count(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_rewrites_file_to_empty_atomically() {
        let (dir, store) = temp_store("clear");
        store.append(&rec("一", 1, 1)).unwrap();
        store.append(&rec("二", 2, 2)).unwrap();
        store.clear().expect("clear 应成功");
        assert!(
            store.path().exists(),
            "clear 是重写不是删文件：文件必须还在"
        );
        assert_eq!(
            fs::read_to_string(store.path()).unwrap(),
            "",
            "清空后文件内容必须为空"
        );
        assert!(store.load(0).unwrap().records.is_empty());
        assert!(
            !dir.join("memory.jsonl.tmp").exists(),
            "rename 之后不得残留临时文件"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clear_creates_empty_file_when_store_absent() {
        let (dir, store) = temp_store("clear-missing");
        assert!(!store.path().exists(), "前置：库文件本来不存在");
        store.clear().expect("clear 应成功");
        assert!(
            store.path().exists(),
            "清空后留下空文件（「空库」在盘上可见）"
        );
        assert_eq!(fs::read_to_string(store.path()).unwrap(), "");
        assert!(store.load(0).unwrap().records.is_empty());
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
