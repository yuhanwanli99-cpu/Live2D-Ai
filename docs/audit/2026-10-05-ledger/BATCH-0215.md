# BATCH-0215 · ⭐ 「若有两份就会漂移」—— **核出来只有一份**（`settings_spec` 引用常量）

Phase 1 · 域覆盖 · `mod-wallpaper/src/lib.rs`（`settings_spec` 的边界声明）

## 跑的命令（全部只读）
```
grep -n "interval_secs|MIN_INTERVAL|min:|max:" mod-wallpaper/src/lib.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**B0214 留的防漂移问题关闭（答案是「不会漂移」）**
```rust
key: "interval_secs".to_string(),
…
min: MIN_INTERVAL_SECS as f64,      // :115  ⭐ **引用常量，不是另写一个 5**
max: MAX_INTERVAL_SECS as f64,      // :116  ⭐
```
⇒ ⇒ 改 `MIN_INTERVAL_SECS` **一处** ⇒ **钳位**（`strategy.rs:119`）与 **UI 声明的边界**（`lib.rs:115`）
**同时变** ⇒ **不可能漂移**
⇒ `:62-63` 的 import 列表也确认这两个常量**来自 `strategy`** ⇒ **单一真源成立**

### ④ 而同一个事实在**四处**被表述，且**四处都由一个源推导**
| 处 | 表述 | 形式 |
|---|---|---|
| `strategy.rs:27/30` | `MIN=5` / `MAX=86_400` | **常量定义（真源）** |
| `strategy.rs:119` | `.clamp(MIN, MAX)` | **引用** |
| `lib.rs:115-116` | `min: MIN as f64` / `max: MAX as f64` | **引用**（给前端的声明） |
| `lib.rs:24` | 「钳在 **5..=86400**，永不 0」 | **散文**（唯一一处手写数字） |

⇒ ⭐ ⇒ **正面模式 P13 / P21 的第四例**：**同一条约束在提示 / 规则 / 测试 / 文档里各出现一次，
而其中三处是「引用」、只有一处（散文）是「手写」** ⇒ ⇒ **手写的那一处才是唯一的漂移点**
⇒ **可提炼为 P25**：**当同一约束出现在多处时，尽量让「除文档外」的全部都是引用** ——
**文档可以写数字，但代码里的每一处都应当是引用**，这样改一处即可全对。

### ⑤ ⭐ 而这也让「看起来会漂移」与「真的会漂移」**成对**出现在我的账本里
| 案例 | 核验结果 |
|---|---|
| B0152 `join_endpoint`（runtime / director 各一份） | ❌ **真的两份**（md5 不同）· 有架构理由但**无同步机制** |
| **本批** `MIN/MAX_INTERVAL_SECS`（`strategy` / `settings_spec`） | ✅ **只有一份**（`settings_spec` 引用常量） |
⇒ ⇒ **「看起来会漂移」与「真的会漂移」必须分别核** —— 而我现在**各有一个样本**。

## 未核实项
1. `lib.rs:24` 那处**散文里的「5..=86400」**是否会与常量脱节（**唯一的漂移点**）
   ⇒ ⭐ **这正是 P25 的代价**：文档里的数字**没有任何机制能保证它跟着常量变**
   ⇒ **本仓对令牌有 `kNotYetWired` 双向对账**（B0212），**但对「文档里的数字」没有**
2. `strategy.rs` 决策主体余约 520 行未读；`mod-template` 未读；Mod crates 逐文件 12/61
3. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
4. `dev_tools_section.dart` 余面未读（1874 行）
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
