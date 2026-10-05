# BATCH-0392 · ⭐⭐ **「读回成功 / 超时兜底都算结束」** —— 而这条路的缺陷记着**两个**后果

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— IndexedDB **水合**的完成与失败（B0391 留）

## 跑的命令（全部只读）
```
grep -rn "hydrating" lib/ --include=.dart | head -6
grep -rn "backgroundHydrating" lib/ --include=.dart | head -5
sed -n '195,210p' lib/main.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0391 留的问题结清**（0 条新发现）
```dart
if (!mounted) return;
// **只回填背景域**（F-0013-1 / F-0001-3）：水合窗口里用户改的主题 / 音量 / 透明度、
// 以及背景库的增 / 删 / 重排，全部保留**当前最新值**。                          // :196-198
// 旧实现是 `_prefs = hydrated.prefs`——把水合**开始时刻的快照**整体盖回去，
// **窗口期的改动全部回滚**；`migrated` 时**还会把这份旧对象写回盘**。            // :199-201
setState(() {
  _prefs = **applyBackgroundHydration(_prefs, hydrated)**;                  // :203  ⭐ 传当前值
  // **水合这一次读结束了（读回成功 / 超时兜底都算结束）。**                   // :204
  _backgroundHydrating = false;                                             // :205
});
if (hydrated.migrated) **saveDisplayPrefs(_prefs)**;                        // :209  只搬过才写回
```
四个可核点：
1. ⭐⭐⭐ **「读回成功 / **超时兜底**都算结束」** ⇒⇒ **正面回答 B0391 的问题**：
   **`hydrating` 不会卡在 true** ⇒⇒ 而兜底是「**超时**」不是「错误」⇒⇒
   **用户看到的是「读完了、但库是空的」** ⇒⇒ **这是一个取舍，而取舍被一句注释写下了**
   ⇒ ⇒⇒ **不记发现的三个理由**：① 兜底存在 ② 代价被写明 ③ 用户看到「空」而不是「卡住」
   ⇒ ⇒⇒ **B0362 的判据（文档缺口不算发现）在此再次适用**
2. ⭐⭐⭐ **那条缺陷记着编号与**两个**后果**（`F-0013-1` / `F-0001-3`）：
   旧实现 `_prefs = hydrated.prefs` **把水合开始时刻的快照整体盖回去**
   ⇒ **① 窗口期的改动全部回滚** **② `migrated` 时还会把这份旧对象写回盘**
   ⇒ ⇒⇒ **两个后果都被写明** ⇒⇒ **而第二个更持久（下次启动再来一次）所以被点名**
   ⇒ ⇒⇒ **修法是「只回填背景域 + 传入当前值」**（不是「换个合并顺序」）
3. ⭐⭐ **写回盘的判据是 `hydrated.migrated`** ⇒⇒ **只搬过才写回** ⇒⇒ **不制造需要清理的状态**
   ⇒⇒ **同 B0371「换对象而不是逐个清」的思路**
4. ⭐⭐ `if (!mounted) return;` 在最前 ⇒⇒ **异步统一守这一条**（B0342 第四处的同族）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **超时兜底的具体时长与「库空」的呈现**（用户看到的是空图库、还是别的）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
