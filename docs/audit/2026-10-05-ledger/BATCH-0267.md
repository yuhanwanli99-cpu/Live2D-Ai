# BATCH-0267 · ⭐ **Phase 4 攻 F-0006-03**：最强的一击（锁的类型）**失败**，但前提被**收窄**

Phase 4 · **证伪 / 自相矛盾** —— 攻第二条 P1

## 跑的命令（全部只读）
```
grep -n "Mutex|RwLock" crates/live2d-ai-desktop/src/mod_registry.rs
sed -n '334p;434p;617p' mod_registry.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0006-03 的 Phase 4 结果** = **② 收窄**（+ 一半 ③ 待证）
### ① 攻法与结果
```
mod_registry.rs:34  use std::sync::{Arc, Mutex};
mod_registry.rs:85  type SharedRuntime = Arc<Mutex<Option<Box<dyn ModRuntime>>>>;
mod_registry.rs:27  //! …槽位共享。**worker 线程持这些**…
```
⇒ ⇒ **是 `std::sync::Mutex`，不是 `parking_lot`** ⇒ ⇒ **`parking_lot` 无毒化**这条路被排除
⇒ ⇒ **毒化是真的** ⇒ ⇒ **本批最强的一击失败** ⇒ **机制成立**

### ② ⭐ 而**前提被收窄**（这才是 Phase 4 的产出）
原措辞容易读成「进程里任何地方 panic ⇒ 整个 HTTP 服务死」
⇒ ⇒ **准确的前提是**：「**某个 Mod worker 在持有该 Mod 的槽位锁时 panic**」
（**不是** `blocking_mods` 那把锁 · **也不是**别的线程的 panic）
⇒ ⇒ **这比原来窄得多、也硬得多** —— 因为它把「什么 panic 会毒化这把锁」**指定了**

### ③ 三个站点全是裸 `.lock().unwrap()`，而 ③ 的另一半**本批未证**
`:334` 写 · `:434` take · `:617` take
⇒ ⭐ 若 worker 侧**没有** `catch_unwind`（B0117 已核 hook 侧没有）
⇒ ⇒ **毒化与 worker 死亡同时发生** ⇒ ⇒ 那会把本条**加重**
⇒ ⚠ **但「worker 循环处是否有 catch_unwind」本批未核** ⇒ **记为敞口，不并入本条结论**
⇒ ⇒ **这正是 Phase 4 判据的用法**：不把「可能」写成「已证」

## 未核实项
1. ⭐ **Mod worker 循环处是否有 `catch_unwind`**（③ 加重那一半的敞口，本批**明确未核**）
2. Phase 4 待攻 P1：F-0001-01/02 · F-0002-02 · F-0013-01 · F-0020-01 · F-0046-01 · F-0049-01
3. `refresh_from_disk` 除 PATCH 外是否还有文件变更路径（B0266 反证 (a) 的敞口）
4. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
5. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
6. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
7. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
8. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
9. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
10. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
11. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
