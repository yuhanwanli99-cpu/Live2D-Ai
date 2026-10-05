# BATCH-0216 · `tick()`：**「一次 tick 至多换一张，长挂起也不补播」** —— 计时器补播风暴被挡且写明

Phase 1 · 域覆盖 · `mod-wallpaper/src/strategy.rs`（决策主体；定点 `WallpaperDecision` + `tick`）

## 跑的命令（全部只读）
```
grep -nE "^pub enum WallpaperDecision|^pub struct WallpaperStrategy|^    pub fn |^    fn " strategy.rs
sed -n '142,168p' strategy.rs ; sed -n '305,342p' strategy.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**两处刻意设计 + 一处策略层兜底**
### ① ⭐ `Interval` 的**补播策略**被写明（:326-332）
```rust
self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);      // :326  饱和加 ⇒ 巨大 delta 不溢出
if self.elapsed_ms >= self.config.interval_ms() {
    // **换后重新计时：一次 tick 至多换一张，长挂起也不补播。**            // :330  ⭐
    self.elapsed_ms = 0;
    self.cursor = (self.cursor + 1) % self.playlist_len;
```
⇒ ⭐⭐ 这是**计时器补播风暴**的经典形状：若实现写成
`while (elapsed >= interval) { advance(); elapsed -= interval; }`
⇒ 进程挂起一小时后恢复，**会一次性重放几百张图**（并把 CPU/WS 打满）
⇒ ⇒ 这里**一次至多换一张** + **重置累加器** ⇒ **不补播**，而且**理由写在代码里**
⇒ 与 B0132/P5「不得静默截断」同族但方向相反：这里**故意少做**（不补播）而不是多做
⇒ ⭐ **「刻意少做」也需要写明**，否则后来者会以为这是 bug 而「修好」它。

### ② `FollowStage` 是**一次性闩**（:308-316）
```rust
if self.synced { None } else { self.synced = true; SyncStage }
```
⇒ **只发一次**，不会每个 tick 重复写 `DisplayPrefs`；而 `reconfigure`（:275）**复位同步标志**
（B0110 已读 `lib.rs:190`「计时 / 同步标志复位」）⇒ ⇒ **改配置会重新断言一次** ⇒ **行为闭合**。
⇒ 另：`!self.started` 的首次 tick **立刻换第一张**（:317-322）⇒ 首屏不等一个间隔。

### ③ ⭐ 而「列表为空」被挡在**策略层**，投影层因此产不出悬空下标
`WallpaperDecision` 的文档（:165-168）：
> 「列表为空的情形**不在这里**出现：策略层已经用 `None` 表达了『没有可切的图』，
> 于是**不会产出「指向不存在那张图」的 patch**。」

⇒ `tick` 里 `if self.playlist_len == 0 { return None; }`（:314-316）**在策略层就返回 `None`**
⇒ ⇒ 投影层（`to_prefs_patch`）**因此不可能**拿到越界的 `index`
⇒ ⭐ **这是 P25 的正面实例**：**不变式在上游强制，下游就不可能违反**（与「钳位在入口」同形）

### ④ 顺带：`None` 的含义被**反向定义**（:160）—— P22 又一例
> 「| `None` | `None` | 什么都不做（**不是**「假装成功」）|」

⇒ **点名了它「不是」什么** ⇒ 免得后来者把「无决策」当成「已成功执行」

## 未核实项
1. `strategy.rs` 其余 ~300 行未读（`to_prefs_patch` 全文、`reconfigure`/`set_playlist_len` 细节、自测）
2. `mod-template` 未读；Mod crates 逐文件覆盖率（12/61）
3. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
4. `dev_tools_section.dart` 余面未读（1874 行）
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
