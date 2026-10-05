# BATCH-0075 · `DisplayPrefs` 三处清单机械对账（按规则 3）

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（`copyWith` 签名 :478-496+ 全段；
   `==` :941-973 / `hashCode` :975-1005 B0074 已核）

## 跑过的命令（全部只读）
```
grep -n "DisplayPrefs copyWith" -A 18 display_prefs.dart
python3 - <<'PY'   # 机械集合差：构造函数字段 / == 的 other.X != / copyWith 形参
  … 正则三扫 + 差集 …
PY
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**未执行任何 Dart 代码**（只做文本集合运算）。

## 本批产出：**0 条新发现** + 一条**风险陈述**（非缺陷）
- `==`(22 标量) ⊆ `copyWith`(25 形参) ⇒ **差集 ∅**；
  `copyWith` 多的 `backgrounds` / `stagePlaylist` 确实在 `==` 里（走 `sameAs` 循环与 `_sameList`）
- ⇒ **`==` / `copyWith` / `hashCode` 三处手维护清单当前完全一致**
- ⚠ 过程中我犯了一次「**解析失败当成结论**」：第一次正则用了旧 Dart 风格，
  解析出 0 个 copyWith 参数、差集显示「缺 22 个」——**若未警觉就会留下一条假发现**
  ⇒ **规则 3 补充**：「我的工具解析出空/异常」也要按「没看到 ≠ 没有」处理
- 与 F-0074-01 合起来的**风险陈述**（非缺陷）：三处清单今天是对的，
  **且没有任何测试会在它们开始漂的那天告诉你** ⇒ 建议**参数化**测试（枚举字段名）而非手写断言

## 未核实项
1. `toJson` / `fromJson` 的字段集**未与前三者对账**（B0075 只对了 `==` / `copyWith` / `hashCode`）
   —— 序列化面若有第 4 处清单，同样是风险面，**下批第一件事**
2. `background_hydration.dart:140-255` 未读
3. `settings_controller.dart`(409) / `app_shell.dart` 余段未读
4. `appearance_background.dart`(1551) 未读（只核过 :1264）
5. 其余 Mod 面板（4 个 / 2284 行）未读
6. `background_logic_test.dart` / `background_copy_test.dart` 未读

## 本批新增
**0 条**
