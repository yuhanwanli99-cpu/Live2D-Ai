//! 语音来源（本地 / 云端）对本地引擎的去留 + 自重启前的收尾（2026-10-09）。
//!
//! 从 `registry.rs` 拆出（该文件已贴着 code-stats 的 `crates/*/src > 500 行`
//! 棘轮上限 44——塞进这一族会直接把 `src-rs-500` 判红）。
//!
//! 断言/口径与 `registry.rs` 的那一族**同源**：都是 `ModRegistry` 的方法，
//! 都由宿主的语音模式（`[tts].mode`）或自重启路径调用。

use super::*;

impl ModRegistry {
    /// **幂等启用**（2026-10-09，语音模式=本地用）：已启用 → 什么都不做并回
    /// `false`。
    ///
    /// 为什么不直接再调一次 [`Self::enable`]：那会重新 `start_one`，而
    /// `local-tts` 的运行时**已有子进程时拒绝再拉一个**（返回 Err）⇒ 界面会把
    /// 一个本来好好跑着的 Mod 标成 Failed。「已有子进程就不要再起一个」在这里
    /// 落地成一句 `if enabled { return Ok(false) }`。
    ///
    /// 返回 `true` = 这一次真的从停用变成启用。
    pub fn ensure_enabled(&mut self, id: &'static str) -> Result<bool, ModError> {
        if self.entries.get(id).is_some_and(|e| e.enabled) {
            return Ok(false);
        }
        self.enable(id)?;
        Ok(true)
    }

    /// **幂等停用**（2026-10-09，语音模式=云端用）：本来就停用 → no-op。
    ///
    /// 停不掉（`shutdown` 失败）→ `Err`，且**什么都不改**（沿用
    /// [`Self::disable`] 的口径：不写 `enabled=false`、不写 `mods.json`）。
    pub fn ensure_disabled(&mut self, id: &'static str) -> Result<bool, ModError> {
        if !self.entries.get(id).is_some_and(|e| e.enabled) {
            return Ok(false);
        }
        self.disable(id)?;
        Ok(true)
    }

    /// **自重启前的收尾**（2026-10-09）：停掉所有已启用 Mod 的 runtime。
    ///
    /// 为什么必须做：服务端「必须重启」路径用 `std::process::exit` 退出，
    /// 而它**不跑析构** ⇒ 子进程会被留成孤儿；新进程再拉一个就会撞端口
    /// （「已有子进程」只在本进程内有效）。这里先停干净，新进程开机按
    /// `with_app` 再拉**一个**。
    ///
    /// 停止失败**不阻断**退出（旧进程本来就要死，卡在这里更坏）：记一行日志。
    pub fn shutdown_all(&mut self) {
        let ids: Vec<&'static str> = self
            .entries
            .iter()
            .filter(|(_, e)| e.enabled)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let taken = self
                .runtimes
                .get(id)
                .and_then(|slot| slot.lock().ok().and_then(|mut g| g.take()));
            if let Some(mut rt) = taken
                && let Err(e) = rt.shutdown()
            {
                tracing::warn!(target: "mod", mod_id = id, "自重启前停掉 Mod 失败（照旧退出）：{e}");
            }
            if let Some(entry) = self.entries.get_mut(id) {
                entry.status = ModStatus::Disabled;
            }
        }
    }

    /// 最近一次失败的**原始文案**（含脚本 stderr 的尾巴）；没有就是 `None`。
    ///
    /// 列表路由把它带出去：不带的话「启用立刻退出 = Failed」只在 `status` 里
    /// 显示成 `failed`，用户看不到脚本到底说了什么。
    pub fn last_error(&self, id: &str) -> Option<&str> {
        self.entries
            .get(id)
            .and_then(|e| e.last_error.as_deref())
            .filter(|s| !s.is_empty())
    }

    /// **测试专用**：把某个 Mod 直接标成 Failed（`last_error` 一起写上）。
    ///
    /// 生产路径的 Failed 由 `start_one` / `create` 写；这里只为了让
    /// 「列表带出失败原因」这条路由回归不必真的跑一个立刻退出的子进程。
    /// 生产代码不得使用。
    #[cfg(test)]
    pub fn mark_failed_for_tests(&mut self, id: &str, message: &str) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.status = ModStatus::Failed {
                message: message.to_string(),
            };
            entry.last_error = Some(message.to_string());
        }
    }
}
