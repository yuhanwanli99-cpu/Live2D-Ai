# BATCH-0078 · 「唯一漏斗」核实 + 模式 G 前端侧（换根后第 1 批）

Phase 1 · 域覆盖 · 前端 `app/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/app/shell_prefs.dart` — 500（定点 100-159 + :475-482，B0069 已核）
2. `shell/flutter/lib/main.dart` — （定点 482 / 496 / 682 / 1207：`onReady` 接线与作用域声明）
3. `shell/flutter/test/action_scales_wiring_test.dart` · `test/stage_color_motion_test.dart` — （定点 6-8 头注）

## 跑过的命令（全部只读）
```
grep -rn "_applyPrefs(" shell/flutter/lib/                      # ← 第一次：漏 tear-off
grep -rn "sendSync|applyActionScales|sendStageBg|_stageKey.currentState?|\.sync\(" shell/flutter/lib/
grep -rn "_applyPrefs" shell/flutter/lib/ shell/flutter/test/     # ← 第二次：全覆盖
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；「唯一漏斗」声明**核实为真**
- `_applyPrefs` 调用点**恰好三处**：定义 `shell_prefs.dart:76` · `_updatePrefs`→`:158` ·
  **`main.dart:1207` `onReady: _applyPrefs`**
- 声明**自己划了作用域**（`main.dart:496`「那四个字段**只有** `_applyPrefs` 会发」）——
  舞台组件**确实也直写桥**（`live2d_stage.dart` 9 处），所以「唯一」成立于**那四个字段**，
  且代码**明确列出是哪四个** ⇒ **精确的边界声明，不是虚假的全局唯一**
- 两个测试记这条链，其中 `stage_color_motion_test.dart:6` 是**真实缺陷的回归**
  （「过去 `_applyPrefs` 一按就下发终色」会**杀掉过渡动画**）⇒ 声明/实现/回归三处对齐

## ⚠ 第 5 次同类失误（新变体）：**grep 假设了调用形态**
`grep "_applyPrefs("`（带括号）漏掉 tear-off `onReady: _applyPrefs`。
若据此下「『首次靠 onReady』是假的」，会记下**方向相反**的假发现。
⇒ **规则 3 补一个变体**：查「谁调用它」的模式**必须覆盖「调用 / tear-off / 回调字段传递」三种形态**；
只匹配 `name(` 会**系统性**漏掉后两种 —— 而那**恰恰是接线最可能出现的地方**。

## 未核实项
1. `settings_controller.dart`(409) 未读（本批原定目标，被漏斗核验占满）
2. `main.dart` 的其余部分未读（~1000 行未读区）
3. `shell_admin.dart` 余段未读；`shell_prefs.dart` 其余段未读
4. `background_hydration.dart:140-255` 未读
5. `appearance_background.dart`(1551) / `appearance_section.dart`(709) 未读
6. 其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
