//! 真摘要的**运行时编排**（P1-5）：触发 / 后台跑 / 收结果 / 回滚 / 状态。
//!
//! 拆出来的理由：`lib.rs` 只留「一轮发生了什么」的生命周期与注入落点；
//! 摘要这条链路有自己的状态（旁车缓存、后台线程回执、版本、失败原因），
//! 塞回去会把「读时序」和「压缩策略」搅在一起。
//!
//! # 非阻塞（硬要求）
//!
//! memory 的 `TurnPrompt` 走**同步投递**（supervisor 等 Mod worker 回执），
//! 所以**绝不能**在这里同步发 HTTP：一次 8s 超时会把每一轮都拖住。
//! 做法是 `std::thread::spawn` 一个后台线程跑
//! [crate::summary::Summarizer::summarize]，主链只 `try_recv` 收**已经跑完**的
//! 结果；拿不到就继续用「无摘要 + top-k 原文」的形态。
//!
//! # 状态从哪来
//!
//! 权威是**旁车文件**（[crate::summary_store]）：内存里的 `summary_*` 字段只是
//! 它的只读缓存（面板 / 注入 / state_json 共用）。缓存按**桶路径**失效——
//! 用户切会话就换桶，缓存必须跟着换，否则 B 会话会读到 A 的摘要。

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::summary::{self, SummaryRequest};
use crate::summary_store::{SummaryStore, SummaryVersion};
use crate::{MemoryRuntime, strategy};

/// 后台摘要线程的回执。
#[derive(Debug)]
pub(crate) enum SummaryOutcome {
    /// 成功：摘要正文 + 覆盖位点（**位点是发起时的快照**，落盘时不再重算）。
    Ok(SummaryRequest, String),
    /// 失败：可读原因（超时 / 非 2xx / 空正文）。**旁车一字不改**。
    Failed(String),
}

impl MemoryRuntime {
    /// 旁车句柄（当前生效桶）；路径不可解析 → `None`。
    pub(crate) fn summary_store(&self) -> Option<SummaryStore> {
        self.effective_store()
            .map(|store| SummaryStore::for_memory_store(&store))
    }

    /// 重读旁车，刷新内存缓存（按桶路径失效）。
    ///
    /// 坏 JSON / 读失败 → 视作**无摘要** + warn（绝不因为旁车坏了就把这一轮
    /// 打挂）。没有旁车文件（首次摘要之前）→ 无摘要。
    pub(crate) fn refresh_summary_state(&mut self) {
        let Some(store) = self.summary_store() else {
            self.clear_summary_cache();
            return;
        };
        let key = store.path().display().to_string();
        if self.summary_cache_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.summary_cache_key = Some(key.clone());
        self.summary_evicted = 0;
        match store.load() {
            Ok(file) => {
                self.summary_version = file.version();
                self.summary_covers_upto = file.covers_upto();
                self.summary_text = file.current().map(|v| v.text.clone());
                self.summary_last_error = file.last_error.clone();
            }
            Err(e) => {
                self.clear_summary_cache();
                self.summary_last_error = Some(format!("读取摘要旁车失败: {e}"));
                self.services.logger.warn(&format!(
                    "memory 摘要旁车不可读（{}）: {e}；本轮按「无摘要」处理（只注入原文命中）",
                    key
                ));
            }
        }
    }

    /// 把摘要缓存切到**指定会话的桶**重读一次（命令面用）。
    ///
    /// 命令带 `session_id` 时不能拿「最近一轮」的缓存冒充：面板切了会话而
    /// 用户还没发话时，最近一轮仍是旧会话。做法是临时把 `current_session`
    /// 指到命令会话 → 走同一条 [Self::refresh_summary_state] → 还原，
    /// 保证「读哪个桶」只有一处实现。
    pub(crate) fn refresh_summary_for(&mut self, session: Option<&str>) {
        let previous = self.current_session.clone();
        self.current_session = session.map(str::to_string);
        self.invalidate_summary_cache();
        self.refresh_summary_state();
        self.current_session = previous;
    }

    /// 当前（或指定会话的）桶路径字符串；不可解析 → `null`。
    pub(crate) fn bucket_path_string(&self, session: Option<&str>) -> serde_json::Value {
        match self
            .store_for(session)
            .map(|store| store.path().display().to_string())
        {
            Some(path) => serde_json::Value::String(path),
            None => serde_json::Value::Null,
        }
    }

    /// 当前生效摘要版本号（0 = 无摘要）。
    pub(crate) fn summary_version_value(&self) -> u64 {
        self.summary_version
    }

    /// 当前生效摘要覆盖位点。
    pub(crate) fn summary_covers_value(&self) -> usize {
        self.summary_covers_upto
    }

    /// 当前生效摘要正文（`null` = 无摘要）。
    pub(crate) fn summary_text_value(&self) -> serde_json::Value {
        match self.summary_text.clone() {
            Some(text) => serde_json::Value::String(text),
            None => serde_json::Value::Null,
        }
    }

    /// 清空内存缓存（无桶 / 旁车坏 / 回滚后强制重读都走它）。
    pub(crate) fn clear_summary_cache(&mut self) {
        self.summary_cache_key = None;
        self.summary_version = 0;
        self.summary_covers_upto = 0;
        self.summary_text = None;
        self.summary_evicted = 0;
    }

    /// 强制下次 [Self::refresh_summary_state] 重读（回滚 / 后台写盘后调用）。
    pub(crate) fn invalidate_summary_cache(&mut self) {
        self.summary_cache_key = None;
    }

    /// 收后台线程的**已完成**结果（非阻塞；每轮开头调一次）。
    ///
    /// 成功 → 写旁车（新版本）并刷新缓存；失败 → 只记 `last_error`，
    /// **旁车一字不改**（失败 ≡ 无摘要）。
    pub(crate) fn drain_summary_results(&mut self) {
        let Some(rx) = self.summary_rx.take() else {
            return;
        };
        let mut drop_rx = false;
        loop {
            match rx.try_recv() {
                Ok(outcome) => self.apply_summary_outcome(outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // 还没跑完：把接收端放回去，下一轮再收。
                    self.summary_rx = Some(rx);
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    drop_rx = true;
                    break;
                }
            }
        }
        if drop_rx {
            self.summary_pending = false;
        }
    }

    /// 把一条回执落到盘上 / 记进状态。
    fn apply_summary_outcome(&mut self, outcome: SummaryOutcome) {
        self.summary_pending = false;
        let Some(store) = self.summary_store() else {
            self.summary_last_error = Some("摘要已生成但桶路径不可解析，未落盘".to_string());
            return;
        };
        let now = strategy::unix_now();
        match outcome {
            SummaryOutcome::Ok(request, text) => {
                let mut file = match store.load() {
                    Ok(file) => file,
                    Err(e) => {
                        self.services.logger.warn(&format!(
                            "memory 摘要旁车不可读（{}）: {e}；本轮不覆盖旧摘要",
                            store.path().display()
                        ));
                        self.summary_last_error = Some(format!("读取摘要旁车失败: {e}"));
                        return;
                    }
                };
                let version = file.generated + 1;
                file.push(SummaryVersion {
                    version,
                    covers_upto: request.covers_upto,
                    covers_turn: request.covers_turn,
                    created_at: now,
                    text: text.clone(),
                });
                if let Err(e) = store.save(&file) {
                    self.errors += 1;
                    self.summary_last_error = Some(format!("写摘要旁车失败: {e}"));
                    self.services.logger.warn(&format!(
                        "memory 摘要写盘失败（{}）: {e}；本轮仍按无摘要处理",
                        store.path().display()
                    ));
                    return;
                }
                self.summary_version = version;
                self.summary_covers_upto = request.covers_upto;
                self.summary_text = Some(text);
                self.summary_last_error = None;
                self.invalidate_summary_cache();
                self.services.logger.info(&format!(
                    "memory 摘要 v{version} 已落盘（覆盖前 {} 条原文，保留最近 {} 轮原文；下一轮起进注入块）",
                    request.covers_upto, self.config.summary_keep_recent_turns
                ));
            }
            SummaryOutcome::Failed(reason) => {
                // 失败 ≡ 无摘要：只记原因（+ 尝试时刻），versions 一个字不动。
                let mut file = store.load().unwrap_or_default();
                file.note_failure(reason.clone(), now);
                if let Err(e) = store.save(&file) {
                    self.services.logger.warn(&format!(
                        "memory 摘要失败状态未能写盘（{}）: {e}",
                        store.path().display()
                    ));
                }
                self.summary_last_error = Some(reason.clone());
                self.invalidate_summary_cache();
                self.services.logger.warn(&format!(
                    "memory 摘要失败（{reason}）；本轮按无摘要处理（规则裁 top-k 仍生效）"
                ));
            }
        }
    }

    /// 触发判定 + **起后台线程**（不阻塞主链）。
    ///
    /// 触发条件（四条全中）：总闸开 + 客户端可用 + 过冷却 + 桶内总字符越阈值
    /// + 待压条数够（在 [crate::summary::build_request] 里判）。
    pub(crate) fn maybe_spawn_summary(&mut self) {
        // **无 conversation 不摘要**：摘要是「本 conversation 的注入段」，
        // 没有会话就没有注入面（P1-5 删掉了全局降级）；而老桶是跨会话共享的，
        // 给它压一份摘要会变成第二个全局真相。这条与「无会话不注入」同源。
        if self.current_session.is_none() {
            return;
        }
        if self.summary_pending || self.turn_seq < self.summary_ready_at {
            return;
        }
        if !self.summarizer.enabled() {
            return;
        }
        let budget = self.config.injection_budget_chars;
        if budget == 0 {
            return;
        }
        let ratio = self.bucket_ratio;
        if ratio <= self.config.summary_ratio {
            return;
        }
        // 「待压条数够不够」由 [crate::summary::build_request] **一处**判定
        // （它还要看保留最近 K 轮与字符预算），这里不重复一份口径。
        let Some((records, offset)) = self.load_bucket() else {
            return;
        };
        let start = self.effective_summary_start(records.len(), offset);
        let Some(request) =
            summary::build_request(&records, start, self.config.summary_keep_recent_turns)
        else {
            return;
        };
        let Some(store) = self.summary_store() else {
            return;
        };
        // 过冷却只有**真的发出请求**才推进：没发就没有冷却可谈。
        self.summary_pending = true;
        self.summary_marks += 1;
        self.summary_ready_at = self
            .turn_seq
            .saturating_add(self.config.summary_cooldown_turns);
        let client = Arc::clone(&self.summarizer);
        let timeout_ms = self.config.summary_timeout_ms;
        let covers = request.covers_upto;
        let (tx, rx): (Sender<SummaryOutcome>, Receiver<SummaryOutcome>) = channel();
        self.summary_rx = Some(rx);
        let key = store.path().display().to_string();
        let pct = (ratio * 100.0).round() as i64;
        self.services.logger.info(&format!(
            "memory 桶内原文 {pct}% 越过摘要阈值：后台起摘要（覆盖前 {covers} 条，保留最近 {} 轮；不阻塞本轮）",
            self.config.summary_keep_recent_turns
        ));
        // 线程名带桶路径尾段，排障时能看出是哪个会话在压。
        std::thread::Builder::new()
            .name(format!("memory-summary:{}", tail(&key)))
            .spawn(move || {
                let outcome = match client.summarize(&request.system, &request.user, timeout_ms) {
                    Some(text) => SummaryOutcome::Ok(request, text),
                    None => SummaryOutcome::Failed(format!(
                        "摘要请求失败（client={}, timeout_ms={}）",
                        client.kind(),
                        summary::clamp_timeout_ms(timeout_ms)
                    )),
                };
                let _ = tx.send(outcome);
            })
            .map_or_else(
                |e| {
                    self.summary_pending = false;
                    self.summary_last_error = Some(format!("起摘要线程失败: {e}"));
                    self.services
                        .logger
                        .warn(&format!("memory 起摘要线程失败: {e}"));
                },
                |_| {},
            );
    }

    /// **回滚上一版摘要**（命令面）：丢 `versions[0]`，原文一条不动。
    ///
    /// 返回被丢的那版（没有可回滚的 → `Err(可读原因)`）。
    pub(crate) fn rollback_summary(&mut self) -> Result<SummaryVersion, String> {
        let store = self.summary_store().ok_or_else(|| {
            "回滚摘要失败：桶路径不可解析（store_path 空且无 config_path）".to_string()
        })?;
        let mut file = store.load().map_err(|e| {
            format!(
                "回滚摘要失败：旁车不可读（{}）: {e}",
                store.path().display()
            )
        })?;
        let dropped = file
            .rollback()
            .ok_or_else(|| "回滚摘要失败：没有可回滚的版本（从未成功摘要过）".to_string())?;
        store
            .save(&file)
            .map_err(|e| format!("回滚摘要失败：写盘失败（{}）: {e}", store.path().display()))?;
        self.invalidate_summary_cache();
        self.refresh_summary_state();
        self.summary_last_error = None;
        self.services.logger.info(&format!(
            "memory 摘要已回滚：丢弃 v{}（覆盖前 {} 条），生效版 {}；被它覆盖的原文重新可检索（原文从未删除）",
            dropped.version,
            dropped.covers_upto,
            if self.summary_version == 0 {
                "无（已回到全原文）".to_string()
            } else {
                format!("v{}", self.summary_version)
            }
        ));
        Ok(dropped)
    }

    /// 摘要状态快照（`state_json.summary` 的来源；只读内存，不碰盘）。
    pub(crate) fn summary_state(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.summarizer.enabled(),
            "client": self.summarizer.kind(),
            "has_api_key": self.summarizer.has_api_key(),
            "endpoint": self.summarizer.endpoint(),
            "note": self.summary_note,
            "idle": self.summary_rx.is_none() && !self.summary_pending,
            "pending": self.summary_pending,
            "version": self.summary_version,
            "covers_upto": self.summary_covers_upto,
            "evicted": self.summary_evicted,
            "pending_records": self.summary_pending_records,
            "kept_recent": self.config.summary_keep_recent_turns,
            "cooldown_left": self.summary_ready_at.saturating_sub(self.turn_seq),
            "marks": self.summary_marks,
            "bucket_chars": self.bucket_chars,
            "bucket_ratio": self.bucket_ratio,
            "last_error": self.summary_last_error,
            "text": self.summary_text,
            "sidecar": self
                .summary_store()
                .map(|s| s.path().display().to_string()),
        })
    }
}

/// 路径尾段（线程名用；没有分隔符就整串）。
fn tail(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}
