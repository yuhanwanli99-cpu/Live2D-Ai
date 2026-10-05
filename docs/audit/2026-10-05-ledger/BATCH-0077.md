# BATCH-0077 · 结 B0076 尾巴 + **离开 `display_prefs.dart`**

Phase 1 · 域覆盖 · 前端（`settings/` 收尾）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（定点 644-675：
   `_readBackgroundSource` / `effectiveBackground`）

## 跑过的命令（全部只读）
```
grep -n "readBackgrounds|_readBackgroundSource" -A 18 display_prefs.dart
sed -n '644,675p' display_prefs.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（两个正面样本，见 FINDINGS）
1. **带根因诊断的迁移修复**（:644-659）：未知枚举值回落、不生效；迁移走「读旧键」而非
   「改默认值」，理由是**改默认值会让「显式设过」与「从没设过」不可区分**；旧键读取带长度上限
2. **判据单一化 + 点名它防的那类症状**（:661-675）：「判据散到两处就会出现『设置说用背景库、
   画的不是背景库』这类不一致」；同处记了**每帧路径用固定 id 而非内容哈希**的性能决策
   ⇒ 与 B0071 的 dbj2 指纹构成一对，两处都写了理由

## 换根决定（按停止规则）
`display_prefs.dart` / `background_item.dart` 这一片已**连续 5 批零发现**（B0073–B0077）
且期间发生 **2 次自我更正** ⇒ 判定为「已被验透的区域」⇒ **离开**。
教训已固化为**模式 L 执行规则 3**（「没看到 ≠ 没有」，含 2 个补充：工具解析出空、辅助函数内的读取）。

## 未核实项（本区遗留，**不再追**，已记 STATE）
1. `readBackgrounds`（:744-748）**本体**未读（B0076 只确认它被调用；本批读的是 `_readBackgroundSource`）
2. `background_hydration.dart:140-255` 未读
3. `appearance_background.dart`(1551) / `appearance_section.dart`(709) 未读
4. `background_logic_test.dart` / `background_copy_test.dart` 未读

## 下一批换到
`settings_controller.dart`(409) + `app/shell_admin.dart` 余段 —— **另一个「单一漏斗」**所在
（`_applyPrefs` 是偏好生效的唯一漏斗，模型 G 的前端侧也在这条链上）

## 本批新增
**0 条**
