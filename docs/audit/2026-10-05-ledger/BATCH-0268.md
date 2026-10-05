# BATCH-0268 · ⭐ **③ 加重成立**：`catch_unwind` 在 `mod_registry.rs` 里**零命中** ⇒ panic **同时**杀 worker 并毒化它持有的锁

Phase 4 · **证伪 / 自相矛盾** —— 结清 B0267 留的敞口

## 跑的命令（全部只读）
```
grep -n "catch_unwind" crates/live2d-ai-desktop/src/mod_registry.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0006-03 的 ③ 加重确认**
```
$ grep -n "catch_unwind" crates/live2d-ai-desktop/src/mod_registry.rs
（**零命中**）
```
⇒ ⇒ **Mod worker 循环没有 `catch_unwind`**（**整个文件都没有**）
⇒ ⇒ ⇒ **一次 panic 同时做两件事**：**杀死那个 worker** + **毒化它当时持有的那把槽位锁**
⇒ ⇒ **两半同时发生** ⇒ ⇒ HTTP 侧那三处裸 `.lock().unwrap()`（`:334` / `:434` / `:617`）**没有兜底**
⇒ ⇒ ⭐ 而这也把 **B0117** 记的「hook 侧没有 `catch_unwind`」**扩展到整个文件**
⇒ ⇒ **B0267 收窄的前提（必须是「该 Mod 的槽位锁」）仍然成立** ⇒ 两者合起来是本条的**完整形态**

### ⇒ 而 Phase 4 在这条上的三样产出**凑齐了**
| 批 | 产出 |
|---|---|
| B0267 | **② 收窄**：前提从「任何地方 panic」收窄到「**该 Mod 的槽位锁**」 |
| B0267 | **③ 加重（待证）**：若无 `catch_unwind` ⇒ 毒化与死亡同时发生 |
| **B0268** | ✅ **③ 加重（已证）**：`catch_unwind` **零命中** |

⇒ ⭐ 而本批同时示范了**Phase 4 的节奏**：**上一批留的敞口，下一批只查那一个**，不做别的 ⇒ 成本可控、结论可追。

## 未核实项
1. Phase 4 待攻 P1：F-0001-01/02 · F-0013-01 · F-0020-01 · F-0046-01 · F-0049-01
2. ⚠ **哪些 Mod 的 worker 真的会在持锁期间 panic**（**能否点名**）⇒ 未核；
   ⇒ 即便点不出名，**兜底缺失本身**已成立 ⇒ **建议不变**（把三处 `.unwrap()` 换成
   `unwrap_or_else(|e| e.into_inner())` 之类，**毒化后仍可取回数据**）
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
