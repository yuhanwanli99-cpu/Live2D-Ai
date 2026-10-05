# BATCH-0207 · ⭐ `copyWith` 逐字段断言**锚定在 `fieldCount` 上** ⇒ 「加字段」本身是一次必然失败

Phase 1 · 域覆盖 · `design_tokens_test.dart:350-374`

## 跑的命令（全部只读）
```
grep -n "每个字段都被 copyWith" -A 24 test/design_tokens_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**Flutter 最经典的两个 `copyWith` bug 各被一层覆盖**
```dart
final List<AppColors> singles = <AppColors>[            // :351-364  **12 个「只改一个字段」的副本**
  base.copyWith(hairline: …), base.copyWith(hoverWash: …), … base.copyWith(radiusScale: 0.6),
];
expect(singles.length, AppColors.fieldCount);            // :365  ⭐ **与 fieldCount 对账**
for (final c in singles) { expect(c, isNot(base), reason: '有字段被 copyWith 漏掉了'); }   // :366-368
```
⇒ **两个经典 bug，各由一层覆盖**：
| bug | 会被谁抓住 |
|---|---|
| **漏**：给 `AppColors` 加了第 13 个字段，但 `copyWith` 没处理 | `:365` `singles.length`(12) ≠ `fieldCount`(13) ⇒ **红** |
| **错**：字段进了 `copyWith` 但**接错到别的字段**（`hairline: hairline ?? this.hoverWash` 这类） | 该字段的副本会 **等于 base** ⇒ `:367` `isNot(base)` ⇒ **红** |
⇒ 而另一条「ThemeExtension **结构枚举**」覆盖**枚举面** ⇒ 三层合起来：**枚举 + 计数 + 逐字段效果**。

### ⭐ 由此提炼一条更一般的模式（**正面模式 P24**）
> **把「新增一个成员」变成一次必然失败。**
> 做法：**把测试里的成员数与代码里的 `fieldCount`/`registry.length` 对账**，
> 于是「加了字段却忘了在某处处理」**必然变红**，而**不需要任何人记得去加断言**。
⇒ 与 P21 第 ① 条（登记表长度与预期一致）**同源**，也与 B0194 的「新增一个 load 的守卫」**同族**
（都是**让「扩展」这个动作自带绊线**）⇒ 归入正面模式 **P24**。

### 顺带：`:371-374` 的第三条也锚定计数
「`toValuesMap` 的**字段数**与 `fieldCount` 一致」+ 注释「令牌面 + 标量清单 = 字段总数，
**一个都不能少、也不能多**」⇒ **第三处**计数对账。

## 未核实项
1. `design_tokens_test.dart` 余约 32 条断言体未读
2. `tokens.dart` 本体（928 行）未读（`AppColors` / `fieldCount` / `kStructuralScalarNames` / 各 `registry`）
3. `dev_tools_section.dart` 余面未读（1874 行）
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. 新露出的 208 条自我设防名只看了 12 条
6. Mod crates 逐文件覆盖率（9/61）
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
