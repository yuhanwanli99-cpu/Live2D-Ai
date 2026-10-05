# BATCH-0461 · ⭐⭐⭐ **两个挂了几十批的未核实项都结了** —— 而 `clamp` 那一行**正是 B0437 那个问题的答案**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_currentBackground` + `_pane` 缓存（B0460 留）

## 跑的命令（全部只读）
```
grep -rn "_currentBackground" lib/ --include=.dart | head -4
sed -n '613,645p' lib/app/app_shell.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**两处都核完，而形状不同**（0 条新发现）
```dart
// lib/app/app_shell.dart:613-620
BackgroundItem? get _currentBackground {
  if (widget.prefs.backgroundSource != DisplayPrefs.backgroundSourceLibrary) {
    return widget.prefs.effectiveBackground;                       // ① 非库 ⇒ 直接用
  }
  final List<BackgroundItem> items = widget.prefs.backgrounds;
  if (items.isEmpty) **return null;**                              // ② 库空 ⇒ null
  return items[widget.backgroundIndex.**clamp(0, items.length - 1)**];  // ③ ⭐ clamp 索引
}
// :625-640  _pane 的缓存注释
// 「**为什么不每帧重算（F-0005-2）**：外壳会被聊天增量（毫秒级）高频重建而面板常驻树里
//   （折叠**不卸载**，见 `CollapsiblePanel` / `settings_panel_keepalive_test.dart`）。
//   **失效条件只有三条**，都真的与内容有关：宿主代际、设置数据代际、换分区。
//   **三种宿主**（内联 / 浮层 / 整页）**共用同一实例是安全的——同一时刻只有一个在树上**。」
// 「⚠️ `context` 必须是 `BackgroundRuntimeScope` **之上**那一层（`SettingsScaffold.child` 的实参位置）：
//   外观区靠一个 `Builder` 去读 scope，**约定与回归见 `test/app_shell_background_test.dart`**；
//   **别改成 tick 那一层的**。」
```

### 五个可核点
1. ⭐⭐⭐⭐⭐ **⇒ `_currentBackground` 的三个分支与 B0390 核的「按来源互斥」完全对上**
   （来源 = 舞台那张 ⇒ `effectiveBackground` · 来源 = 库 ⇒ 按 `backgroundIndex`）
2. ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `clamp(0, items.length - 1)` 正是我 B0437 之后自己提的那个问题的答案**
   ⇒⇒ **列表变短（旧文件被删）而 `backgroundIndex` 还指着旧位置** ⇒⇒ **那一行就是完整处理**
   ⇒⇒⭐⭐⭐⭐⭐⭐ **⇒ 「库空」与「索引越界」被同一个表达式覆盖** ⇒⇒⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐ **⇒ 而 `_pane` 的缓存注释把**失效条件逐条列出、并给每条一个理由**（都真的与内容有关）
   ⇒⇒ **⇒ 排除了「顺手也清一下」那类多余失效** ⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「别改成 tick 那一层的」是一句**禁令 + 它为什么必要**（`context` 的继承位置）
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「约定与回归见 `test/app_shell_background_test.dart`」** ⇒⇒ **又一次「指路」+ 回归文件名** ⇒⇒⭐⭐⭐⭐⭐
5. ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 而两处共同的一层是「**跨对象 / 跨帧的取值都要有边界**」** ⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒ 一处用 `clamp`（索引）、一处用三条失效条件（代际）⇒⇒⇒⭐⭐⭐⭐⭐**
   **⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 「边界」有两种：一个值的边界 · 一个缓存的边界**

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐ **⇒ 而本批最值钱的是第 2 点**
> **`clamp(0, items.length - 1)` 那一行就是我 B0437 提的「索引会变陈旧」问题的答案**
> **⇒⇒⇒ ⇒ 而它在**第一次读这个函数**时就该被看到 —— 而我当时是从**面板**那一侧读的**
> **⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ **我 B0437 记的是「面板上那三个失败面」，而 B0461 记的是「其中两个（互斥 · 判据集中）早已在宿主侧解决」——⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   **`app_shell_background_test.dart` 里对 `clamp` 那条边界的断言是否存在**（**B0461 留**）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
