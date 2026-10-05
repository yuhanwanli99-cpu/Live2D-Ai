# BATCH-0012 · Mod registry 装配面 —— 收口 F-0006-03 的未核实项

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`（收尾批）

## 读过的文件（来自 `git ls-files`）

1. `crates/live2d-ai-desktop/src/mod_registry.rs` — 1362 行（**读 306-509 生产段**，
   + 全文 grep sweep；未逐行全读，见「未核实项」）
2. `crates/live2d-ai-desktop/src/web_api/tests_reload.rs` — 211（**枚举全部 4 条测试名**）
3. `crates/live2d-ai-desktop/src/web_api/tests_p0c.rs` — 416（**枚举全部 10 条测试名**）

`tests_dev_mode.rs`(328) / `tests_mod.rs`(266) 本批**未读**，顺延 BATCH-0013。

## 跑过的命令（全部只读）
```
grep -n "pub fn (enable|disable|restart|enable_with_config|reload_config)" mod_registry.rs
grep -n "runtimes:" mod_registry.rs
grep -n "lock()\.unwrap()|lock()\.expect(" mod_registry.rs     # → 11 命中，3 处在生产路径
grep -n "fn start_one" -A 20 mod_registry.rs
grep -n "^fn |#\[test\]" -A 1 tests_reload.rs tests_p0c.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批最重要的一步：F-0006-03 的未核实项**闭合**，且发现**升级为 P1**

BATCH-0006 记的未核实项是「`enable/disable/restart` 的实现是否可能 panic」。
本批读 `mod_registry.rs:421-468` 得到答案：**是，且有三处，全在 web 请求路径上。**

```rust
// mod_registry.rs:432-439  disable()
pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {
    let Some(e) = self.entries.get_mut(id) else { ... };
    if let Some(mut rt) = self.runtimes.get(id).and_then(|s| s.lock().unwrap().take()) { let _ = rt.shutdown(); }   // ← :434
    ...
}
```
同款 `.lock().unwrap()` 另两处：`:334`（`start_one` 内，`enable` 走它）与 `:617`（shutdown 路径）。
`runtimes: BTreeMap<&'static str, SharedRuntime>`（:171），而 `SharedRuntime` 是被
**Mod worker 线程**持有的锁——本文件自己的 `runtime_state` 注释就写着
「3. Mod worker 正持着 runtime 锁（`try_lock` 失败）」（:497）。

⇒ **完整链条**（F-0006-03 的最后一环补齐）：
1. 某个 Mod worker 线程 panic（持着 runtime 锁）→ 锁被毒化；
2. 用户点「停用」或「重启」→ `POST /api/v1/mods/{id}/disable|restart`；
3. `mods_routes.rs:163` 拿 `mod_registry` 互斥锁，**并持有到 :322**；
4. → `mod_registry.rs:434` `s.lock().unwrap()` → **panic**；
5. panic 毒化 `mod_registry` 互斥锁，并从 `run_request_loop` 的裸 `for` 循环
   （mod.rs:446，**无 `catch_unwind`**）里展开出去 → **整个 HTTP 服务停摆**。

**对照组（同文件内的正确做法）**：`runtime_state`（:502-506）刻意用
`try_lock().ok()?`，注释写「web_api 线程**绝不**为一个可选 Mod 的状态等锁——
那会让『看一眼桌宠状态』把整个 HTTP 请求循环卡在 Mod worker 上」（:500-501）。
**这条纪律是已知的、写下来的，只是没有覆盖到 enable/disable/shutdown 三处。**

→ **F-0006-03 由 P2 升级为 P1**（严重性依据：一次 Mod 侧 panic + 一次用户点击 =
服务永久停摆，无自愈、无日志；且触发面是「Mod 管理」这个用户会主动点、Mod 又确有
失败面的功能——登记在案的 S4「Mod 启动失败无日志与 last_error」正说明该路径会出错）。

## 另一条回填：F-0002-01 的测试覆盖检查（§12.1 要求）
枚举 `tests_reload.rs` 与 `tests_p0c.rs` 的全部测试名后确认：
- `tests_reload.rs:160 refresh_from_disk_syncs_snapshot_and_dev_mode`、
  `:192 refresh_from_disk_keeps_snapshot_on_broken_toml`
  ——它们**直接调 `StatusContext::refresh_from_disk`**，测的是**那个函数**；
- `tests_p0c.rs:153 first_time_full_config_patch_dynamic_assembles_supervisor`、
  `:343 e2e_first_time_no_config_patch_completes_loop`
  ——它们测的是「无 supervisor 时 PATCH 能动态装配」，也测的是**函数**；
- **全仓没有任何测试**断言「`FileWatcher` 在什么条件下被创建」——
  `watch_config`（file_watcher.rs:46）需要真实 `Arc<SupervisorHandle>` + notify，一处测试都没有。

⇒ F-0002-01（「首次配置路径上 file_watcher 永久不装」）**零测试覆盖**，
而它旁边有 4 条「看起来覆盖了配置一致性纪律」的邻居测试
——这正是任务书 §7-J 说的「只测纯函数不测接线」的接缝。
已在 F-0002-01 补记该结果（不改变其 P1 定级，只补上「为什么门禁没抓到」）。

## 维度覆盖（本批）
- 红线 R（Mod 边界与秘密）：✔ Mod 一律经 `ModRegistry` 仲裁；`say` 走 `SaySender`（:395）、
  动作走**休眠 sender** 并逐条 `tracing::debug!`（:385-393，与 AGENTS.md 的休眠台账一致，
  **没有**私自接回真通道）；`apply_settings` 是一等公民（:404），事件走私已删（:396-397 注释）
- 维度 B：`start_one`（:306-321）对 `api_version` 不兼容的处理是**标记 Failed + 记
  `last_error` + `tracing::warn!` 后 return**，不 panic、不崩主链（:309-320）——
  **这条与登记在案的 S4「Mod 启动失败无日志与 last_error」不符**：
  本分支有日志也有 `last_error`。**S4 可能已部分修复，留 Phase 2 维度 B 横扫时逐条复核。**
- 维度 J：模式 B 复查扩到这 14 条测试名，**无一条自相矛盾**（名字承诺的行为都对应真实场景）

## 未核实项
1. `mod_registry.rs` 1362 行只读了 306-509 + grep；`persist_manifest`（mods.json 原子写回，
   **红线 R 明列「不得丢未知 id」**）的实现在 510 之后，**本批未读**——留 BATCH-0013。
2. `tests_dev_mode.rs` / `tests_mod.rs` 未读。
3. 「Mod worker 是否真的会 panic」未验证（需要构造一个会 panic 的 Mod）——
   但 `.lock().unwrap()` 的毒化风险**不需要** worker panic 才会发生这一条论证不成立：
   风险来自「worker 持锁期间任何 panic」，而 worker 跑的是第三方/社区 Mod 代码
   （见 `docs/architecture/mod-community-license.md`），其 panic 是可预期的。

## 本批新增
P0 0 · **P1 1**（F-0006-03 由 P2 **升级**）· P2 0 · P3 0
另：F-0002-01 补记「零测试覆盖 + 4 条邻居测试造成覆盖错觉」
