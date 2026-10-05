# BATCH-0194 · ⚠ **t0 确认，但 B0193 的「自然触发」被我高估了**；`_ensureSettingsLoaded` 是一次性守卫

Phase 1 · 域覆盖 · `shell_settings.dart:12-23`（t0 核实）+ `api_client.dart` 的 `_guard` 调用面

## 跑的命令（全部只读）
```
sed -n '12,24p' lib/app/shell_settings.dart
grep -n "_guard" -A 18 lib/api/api_client.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**修正我 B0193 的一处高估**
### ① t0 **确认存在**，但它是**一次性**的 ⇒ B0193 的时间线**窗口很窄**
```dart
/// 进设置面时**懒加载一次**。**不放在 `initState`**：这些数据多数时候用不到，
/// 启动就打三个请求是白花钱（也拖慢首屏）。                    // :12-14
Future<void> _ensureSettingsLoaded() async {
  if (_settingsLoadedOnce) return;   // ← ⭐ **一次性守卫**
  _settingsLoadedOnce = true;
  await Future.wait<void>([ _settings.load(), _loadAdmin(), _loadPresetLabels() ]);
}
```
⇒ **`_settingsLoadedOnce` 使 init `load()` 只发生一次**（首次进入设置面）
⇒ ⇒ **它不能**作为**后续任意** `:108` 回读的搭子，**除非**用户的**第一个动作**就是保存覆盖
（且 init GET 仍在飞）⇒ **窗口窄于 B0193 说的「打开设置后立刻改东西」所暗示的程度**
⇒ ⚠ **我 B0193 说「重叠不需要连点两次、是正常操作」——这句**高估了**，如实修正。

### ② 修正后**最现实的触发路径**（按现实性排序）
| # | 路径 | 需要的用户行为 | 现实性 |
|---|---|---|---|
| **1** | ⭐ **连续保存两次覆盖** ⇒ 每次都触发 `:108` 的整份回读 ⇒ **两个 GET 无按钮参与** | 改两次覆盖 | **最高**（不需要任何异常操作） |
| 2 | init GET 仍在飞时**立刻**保存覆盖 | 极快操作 | 中（窗口窄） |
| 3 | `:224`「重试」**连点两次**（B0192） | 连点 | 中（且该分支只在失败时可见） |
⇒ ⇒ **发现本身不变**（无守卫、可乱序、可丢草稿），**但主路径应从「打开设置就改」改为「连续保存两次覆盖」**。

### ③ 顺带两处正面
- **`:12-14` 的性能决策带理由**：「**不放在 `initState`**：这些数据多数时候用不到，**启动就打三个请求是白花钱（也拖慢首屏）**」
  ⇒ **正面模式 P1（理由要写「用户会看到什么」）** —— 而这次的理由是**首屏延迟**，同样具体
- **三个请求用 `Future.wait` 并发** ⇒ 三个各自的失败被**分别**处理（其中之一注释「失败静默」）
  ⇒ 并发而不串行、且失败不互相污染 ⇒ 与 `:108` 那个「PATCH 完再回读」的**串行**形成对照
  ⇒ **两种时序都有明确理由**（并发为快、串行为「读到的必须是 PATCH 之后的状态」）

## 未核实项
1. `_guard()` **本体未读**（我只看到 3 处**调用**）—— 是否含超时/重试/去重
2. `_loadAdmin()` / `_loadPresetLabels()` 的失败处理未读
3. `settings_controller.dart:120-222` / `:296-409` 其余未读；`:327` 那处置 `_draft` 未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
