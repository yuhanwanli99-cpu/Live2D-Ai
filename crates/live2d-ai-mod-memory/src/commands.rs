//! `ModRuntime::command` 的实现：**主动管理面**（面板的查看 / 导入 / 编辑 /
//! 删除 / 清空）。L1 产品级波次新增 list / import / update / delete。
//!
//! # 命令契约（host 原样把 `args` 交给这里，返回体的键由本 Mod 自定）
//!
//! | command | args | 返回（`result`） |
//! | --- | --- | --- |
//! | `list` | `{limit?, session_id?}` | `{ok, records:[{id,text,ts,turn}], total, bucket, session_id, limit}`，**最新在前** |
//! | `import` | `{text, session_id?}` | `{ok, id, records, total, evicted, bucket, session_id}` |
//! | `update` | `{id, text, session_id?}` | `{ok, id, records, total, bucket, session_id}` |
//! | `delete` | `{id, session_id?}` | `{ok, removed, records, total, bucket, session_id}` |
//! | `clear` | `{session_id?}` | `{ok, records, cleared, removed, residue, bucket, session_id}` |
//! | `summary` | `{session_id?}` | `{ok, summary, bucket, session_id}`，**只读状态快照**（键同 `state_json.summary`） |
//! | `summary_rollback` | `{session_id?}` | `{ok, rolled_back:{version,covers_upto,text}, version, covers_upto, text, bucket, session_id}` |
//!
//! # 失败纪律（可读、可处置）
//!
//! - 参数缺失/空白/找不到 id → [`ModError::Other`]（host 409 `command_failed`，
//!   文案直接告诉用户「先 list 拿 id」）；
//! - 不认识命令 → [`ModError::UnsupportedCommand`]（host 409 `unsupported_command`）；
//! - 未启用 / worker 正持锁由 host 的 runtime 锁挡下（503 `command_unavailable`，
//!   可重试）——本文件不重复实现那层。
//!
//! # 为什么每条命令都重新解析桶
//!
//! 面板的 `session_id` 可能随时变（用户切了会话）。把「当前是哪个桶」钉在
//! runtime 字段上会在切会话的瞬间错位；每条命令按自己的 args 解析，才不会
//! 「刚给 A 导入，却写进了 B」。

use std::collections::BTreeSet;

use live2d_ai_mod_system::ModError;
use serde_json::{Value, json};

use crate::MemoryRuntime;
use crate::store::{JsonlStore, LoadOutcome};
use crate::strategy::{self, MemoryRecord};

/// `list` 缺省返回条数（面板一次拉一屏）。
const DEFAULT_LIST_LIMIT: usize = 50;
/// `list` 上限——面板不该一次把所有记忆糊到界面上。
const MAX_LIST_LIMIT: usize = 200;

impl MemoryRuntime {
    /// 命令分派（`lib.rs` 的 `command` 只做转达，规则集中在这里）。
    pub(crate) fn dispatch_command(
        &mut self,
        command: &str,
        args: &Value,
    ) -> Result<Value, ModError> {
        match command {
            "list" => self.cmd_list(args),
            "import" => self.cmd_import(args),
            "update" => self.cmd_update(args),
            "delete" => self.cmd_delete(args),
            "clear" => self.cmd_clear(args),
            "summary" => self.cmd_summary(args),
            "summary_rollback" => self.cmd_summary_rollback(args),
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }

    /// `list`：返回该桶的记录，**最新在前**（JSONL 追加序的逆序）。
    fn cmd_list(&mut self, args: &Value) -> Result<Value, ModError> {
        let limit = list_limit(args);
        let session = self.command_session(args)?;
        let store = self.require_store(session.as_deref())?;
        let outcome = self.load_records(&store, "列出")?;
        let total = outcome.records.len();
        let records: Vec<Value> = outcome
            .records
            .iter()
            .rev()
            .take(limit)
            .map(record_json)
            .collect();
        Ok(json!({
            "ok": true,
            "records": records,
            "total": total,
            "bad_lines": outcome.bad_lines,
            "limit": limit,
            "bucket": store.path().display().to_string(),
            "session_id": session,
        }))
    }

    /// `import`：面板手动加一条（走 `append_capped`，同样受 `max_records` 上限）。
    fn cmd_import(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        let text = require_text(args, "导入")?;
        let store = self.require_store(session.as_deref())?;
        let (id, ts, turn) = self.next_import_identity(&store, text)?;
        let record = MemoryRecord {
            id: id.clone(),
            text: text.to_string(),
            ts,
            turn,
            // 面板手动导入 = 用户侧（与 TurnPrompt 写入同角色）。
            role: strategy::MemoryRole::User,
        };
        let outcome = match store.append_capped(&record, self.config.max_records) {
            Ok(outcome) => outcome,
            Err(e) => {
                self.errors += 1;
                return Err(ModError::Other(format!(
                    "写入记忆失败（{}）: {e}",
                    store.path().display()
                )));
            }
        };
        self.writes += 1;
        self.note_eviction(outcome.evicted, outcome.kept);
        self.services.logger.info(&format!(
            "memory 面板导入 1 条（现存 {} 条，bucket {}；下一条命中它的用户话本轮即可用上）",
            outcome.kept,
            store.path().display()
        ));
        Ok(json!({
            "ok": true,
            "id": id,
            "records": outcome.kept,
            "total": outcome.kept,
            "evicted": outcome.evicted,
            "bucket": store.path().display().to_string(),
            "session_id": session,
        }))
    }

    /// `update`：按 id 原子重写正文；**id 保持不变**（见 `JsonlStore::update_text`）。
    fn cmd_update(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        let id = require_id(args, "更新")?;
        let text = require_text(args, "更新")?;
        let store = self.require_store(session.as_deref())?;
        let mut outcome = self.load_records(&store, "更新前读取")?;
        let Some(record) = outcome.records.iter_mut().find(|r| r.id == id) else {
            return Err(ModError::Other(format!(
                "更新失败：找不到 id={id} 的记录（可能已被淘汰或删除），请先 list 拿最新 id"
            )));
        };
        record.text = text.to_string();
        if let Err(e) = store.write_all(&outcome.records) {
            self.errors += 1;
            return Err(ModError::Other(format!(
                "更新记忆失败（{}）: {e}",
                store.path().display()
            )));
        }
        let total = outcome.records.len();
        self.services.logger.info(&format!(
            "memory 面板更新 1 条（id {id}，现存 {total} 条；下一条命中它的用户话本轮即可用上）"
        ));
        Ok(json!({
            "ok": true,
            "id": id,
            "records": total,
            "total": total,
            "bucket": store.path().display().to_string(),
            "session_id": session,
        }))
    }

    /// `delete`：按 id 原子删除一条；找不到 → 可读错误（不谎报 removed=0）。
    fn cmd_delete(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        let id = require_id(args, "删除")?;
        let store = self.require_store(session.as_deref())?;
        let mut outcome = self.load_records(&store, "删除前读取")?;
        let before = outcome.records.len();
        outcome.records.retain(|r| r.id != id);
        if outcome.records.len() == before {
            return Err(ModError::Other(format!(
                "删除失败：找不到 id={id} 的记录（可能已被淘汰或删除），请先 list 拿最新 id"
            )));
        }
        if let Err(e) = store.write_all(&outcome.records) {
            self.errors += 1;
            return Err(ModError::Other(format!(
                "删除记忆失败（{}）: {e}",
                store.path().display()
            )));
        }
        let total = outcome.records.len();
        self.services
            .logger
            .info(&format!("memory 面板删除 1 条（id {id}，现存 {total} 条）"));
        Ok(json!({
            "ok": true,
            "id": id,
            "removed": 1,
            "records": total,
            "total": total,
            "bucket": store.path().display().to_string(),
            "session_id": session,
        }))
    }

    /// `clear`：**原子重写该桶为空**，并报告可观察结果（产品级加强波次语义）。
    ///
    /// 只清 JSONL，**不写** `persona.system_prompt`（残留按既有 `strip_residue`
    /// 生命周期处理）；有会话时只清该会话的桶，其他会话不受影响。
    fn cmd_clear(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        let store = self.require_store(session.as_deref())?;
        let removed = self.load_records(&store, "清空前读取")?.records.len();
        if let Err(e) = store.clear() {
            self.errors += 1;
            return Err(ModError::Other(format!(
                "清空记忆库失败（{}）: {e}",
                store.path().display()
            )));
        }
        let residue = self.prompt_has_residue(session.as_deref());
        // P1-5：桶被清空后摘要**必须一起删**——留着它会描述一段不存在的历史，
        // 而且 `covers_upto` 会立刻指向错误位点（原文没了，位点还在）。
        // 回滚不删文件，但「清空」是另一回事：这是**删除语义**。
        let summary_removed =
            match crate::summary_store::SummaryStore::for_memory_store(&store).remove() {
                Ok(()) => true,
                Err(e) => {
                    self.services.logger.warn(&format!(
                        "memory 清空时删除摘要旁车失败（{}）: {e}",
                        store.path().display()
                    ));
                    false
                }
            };
        self.refresh_summary_for(session.as_deref());
        self.services.logger.info(&format!(
            "memory 已清空记忆库（清掉 {removed} 条，bucket {}；摘要旁车{}；提示词注入块{}）",
            store.path().display(),
            if summary_removed {
                "已一并删除"
            } else {
                "删除失败 / 本就不存在"
            },
            if residue {
                "仍在（按既有 strip_residue 语义处理）"
            } else {
                "本就不存在"
            }
        ));
        Ok(json!({
            "ok": true,
            "records": 0,
            "cleared": true,
            "removed": removed,
            "summary_removed": summary_removed,
            "residue": residue,
            "bucket": store.path().display().to_string(),
            "session_id": session,
        }))
    }

    /// `summary`：只读的摘要状态快照（**不触发**摘要、不写盘）。
    ///
    /// 与 `state_json.summary` 同源（[MemoryRuntime::summary_state]），所以面板
    /// 与运行态快照永远不会说两套话。切会话后按 `args.session_id` 重读该桶旁车
    /// ——不能拿「最近一轮」的缓存冒充。
    fn cmd_summary(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        self.refresh_summary_for(session.as_deref());
        let state = self.summary_state();
        Ok(json!({
            "ok": true,
            "summary": state,
            "bucket": self.bucket_path_string(session.as_deref()),
            "session_id": session,
        }))
    }

    /// `summary_rollback`：**回滚上一版摘要**（丢 `versions[0]`）。
    ///
    /// 语义（口径见 `summary_store.rs`）：原文一条不动，被那版覆盖的原文重新
    /// 可被 top-k 检索；没有可回滚的版本 → 可读错误（**不谎报成功**）。
    /// 回滚后**清掉该会话的记忆注入槽**——下一轮按新版本重拼，否则用户会看到
    /// 「回滚了但提示词里还是旧摘要」。
    fn cmd_summary_rollback(&mut self, args: &Value) -> Result<Value, ModError> {
        let session = self.command_session(args)?;
        self.refresh_summary_for(session.as_deref());
        let dropped = self.rollback_summary().map_err(ModError::Other)?;
        if let Some(id) = session.as_deref() {
            self.clear_session_injection(id);
        }
        Ok(json!({
            "ok": true,
            "rolled_back": {
                "version": dropped.version,
                "covers_upto": dropped.covers_upto,
                "text": dropped.text,
            },
            "version": self.summary_version_value(),
            "covers_upto": self.summary_covers_value(),
            "text": self.summary_text_value(),
            "bucket": self.bucket_path_string(session.as_deref()),
            "session_id": session,
        }))
    }

    /// 读全部记录 + 坏行可见化；读失败 → 可读错误 + `errors`。
    fn load_records(&mut self, store: &JsonlStore, what: &str) -> Result<LoadOutcome, ModError> {
        match store.load(0) {
            Ok(outcome) => {
                if outcome.bad_lines > 0 {
                    self.services.logger.warn(&format!(
                        "memory {what}时跳过 {} 条坏记录（{}）",
                        outcome.bad_lines,
                        store.path().display()
                    ));
                }
                Ok(outcome)
            }
            Err(e) => {
                self.errors += 1;
                Err(ModError::Other(format!(
                    "{what}记忆库失败（{}）: {e}",
                    store.path().display()
                )))
            }
        }
    }

    /// import 的 id 与 turn：同一秒导入同一句话时**递增 turn 直到不冲突**。
    ///
    /// 公式里的 `turn` 本是为了区分不同轮次；面板导入没有轮次，用它当去重序，
    /// 保证「连点两次导入同一句」得到两条**不同 id** 的记录（否则 update/delete
    /// 会一次命中两条）。
    fn next_import_identity(
        &mut self,
        store: &JsonlStore,
        text: &str,
    ) -> Result<(String, i64, u64), ModError> {
        let ts = strategy::unix_now();
        let existing: BTreeSet<String> = self
            .load_records(store, "检查 id 冲突")?
            .records
            .into_iter()
            .map(|r| r.id)
            .collect();
        let mut turn = self.turn_seq;
        loop {
            let id = MemoryRecord::make_id(ts, turn, text);
            if !existing.contains(&id) {
                return Ok((id, ts, turn));
            }
            turn += 1;
        }
    }

    /// 物理淘汰的可观察收尾（与被动路径同一条日志口径）。
    pub(crate) fn note_eviction(&mut self, evicted: usize, kept: usize) {
        if evicted == 0 {
            return;
        }
        self.evicted += evicted as u64;
        self.services.logger.info(&format!(
            "memory 超过条数上限 {}，物理淘汰最旧 {} 条（现保留 {} 条）",
            self.config.max_records, evicted, kept
        ));
    }
}

/// 一条记录的对外 JSON（面板用它定位/展示；Wave 3 起多一个 role）。
fn record_json(record: &MemoryRecord) -> Value {
    json!({
        "id": record.id,
        "text": record.text,
        "ts": record.ts,
        "turn": record.turn,
        "role": record.role.as_str(),
    })
}

/// `limit`：缺省 50、上限 200、非法（非数 / 0 / 负数 / NaN）回落缺省。
fn list_limit(args: &Value) -> usize {
    match args.get("limit").and_then(Value::as_f64) {
        Some(n) if n.is_finite() && n >= 1.0 => {
            let rounded = n.round();
            if rounded >= MAX_LIST_LIMIT as f64 {
                MAX_LIST_LIMIT
            } else {
                rounded as usize
            }
        }
        _ => DEFAULT_LIST_LIMIT,
    }
}

/// 取必填的 `text`（去首尾空白后非空），否则可读错误。
fn require_text<'a>(args: &'a Value, action: &str) -> Result<&'a str, ModError> {
    args.get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ModError::Other(format!("{action}失败：text 不能为空——请先输入要记住的内容"))
        })
}

/// 取必填的 `id`（非空），否则可读错误并提示先 list。
fn require_id<'a>(args: &'a Value, action: &str) -> Result<&'a str, ModError> {
    args.get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ModError::Other(format!(
                "{action}失败：缺少 id——请先 list 拿到要操作那条的 id"
            ))
        })
}
