# BATCH-0033 · 前端持久化真源（browser_io + display_prefs.fromJson）

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/app/browser_io.dart` — 157（读 1-110）
2. `shell/flutter/lib/settings/display_prefs.dart` — 1157（结构图 + 读 570-632 `fromJson` +
   定点 865-869 `clampSlideInterval` + 300-423 常量面）
3. `shell/flutter/lib/main.dart` — （定点 151 / 209 / 240：落盘失败的接住与两处调用）

## 跑过的命令（全部只读）
```
grep -n "^class |^enum |^  static const |localStorage|window\." display_prefs.dart
grep -n "saveDisplayPrefs|loadDisplayPrefs" display_prefs.dart data/*.dart
grep -rn "saveDisplayPrefs" shell/flutter/lib/ | grep -v "onPrefsChanged|//"
find shell/flutter/lib -name "browser_io*"
grep -n "factory DisplayPrefs.fromJson" -A 45 display_prefs.dart
grep -n "clampSlideInterval" -A 22 display_prefs.dart
sed -n '1,110p' app/browser_io.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A（真源唯一性）：localStorage **一条**记录存整份偏好（:32-35）+ 逐字段设防的 `fromJson`
- 维度 C（失败传播）：`bool`/`void` 的不对称是**按「丢了用户会不会心疼」**取的，两侧理由都写明
- 维度 G：偏好里不含任何密钥面（无 token/key 字段）
- 维度 B：`pickImageDataUrl` 三态、无未释放资源

## 未核实项
1. `display_prefs.dart` 只读了 `fromJson` 与常量面（1157 行中约 130 行）。**未读**：
   `toJson`（序列化方向——**与 fromJson 不对称就是丢数据**，是本文件最该核的一半）、
   `readBackgrounds` / `readStagePlaylist` 的预算逐项把关实现、`copyWith` 家族、
   `BackgroundItem` 的 `toJson/fromJson`（B0028 已知它「只写 id」）
2. `browser_io.dart:110-157` 未读（`pickFileDataUrl` 实现 + 剩余两个函数）
3. `display_prefs_test.dart`(892) 未读——按模式 H 的执行建议，该问的是
   「**难形态**有测试吗」：坏值夹持、哨兵值、预算边界这三类是否有能失败的断言

## 本批新增
**0 条**（净产出：三处「可能静默丢用户数据」的地方全部证伪 + 一条取舍判据）
