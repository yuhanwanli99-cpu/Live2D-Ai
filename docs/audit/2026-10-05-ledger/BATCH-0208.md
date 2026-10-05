# BATCH-0208 · `tokens.dart`：`lerp` 按**类型分两种插值**，且 `toValuesMap` 是**专为测试建的生产 API**

Phase 1 · 域覆盖 · `lib/design/tokens.dart`（928 行；定点结构 + `AppColors.lerp` + `toValuesMap`）

## 跑的命令（全部只读）
```
grep -nE "^class |^  static const Map|^  static int get fieldCount|^  AppColors copyWith|^  AppColors lerp|…" tokens.dart
sed -n '572,600p' tokens.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；两处结构核验通过
### ① ⭐ `lerp`（:572-596）按**字段类型分成两种插值**，且这个分法是**语义必需**的
| 类型 | 写法 | 数量 |
|---|---|---|
| `Color`（10 个） | `Color.lerp(a, b, t)!` | 10 |
| `double`（`panelAlpha` / `radiusScale`） | `a + (b - a) * t` | 2 |
⇒ ⇒ **12 个字段一个不缺**（与 B0207 的 `fieldCount` 对账一致）
⇒ ⭐ 而这个分法**不是风格问题**：`Color.lerp` 的签名是 `static Color? lerp(Color? a, Color? b, double t)`
⇒ **对一个 `double` 调它根本编译不过** ⇒ ⇒ **类型系统在这里帮忙**：**两个家族的字段不可能被写错插值方式**。
⇒ 另注：那些 `!` **不是**在掩盖真实的 null —— 两个入参都是**非空类型**时 `Color.lerp` 不会返回 null
⇒ **10 个 `!` 是安全的**（**记录这点以免日后被误当成「危险的强制非空」**）。

### ② ⭐ `toValuesMap` 是**专为测试而建的生产 API**
```dart
/// **供测试做结构枚举**：字段名 → 值。          // :598
Map<String, Object?> toValuesMap() => <String, Object?>{ 'hairline': hairline, … };
```
⇒ ⇒ **测试的枚举源与字段声明在同一处维护** ⇒ **不会漂移**
⇒ ⇒ 正面模式 **P2（私有汇合）** 用在**生产 / 测试的边界**上 ——
不是「测试抄了一份清单」，而是「**生产把那份清单以 API 形式交出来**」。

### ③ ⇒ 与 B0207 合起来：`copyWith` 有**三个独立锚点**
| 锚点 | 位置 | 挡住什么 |
|---|---|---|
| **结构枚举** | 「ThemeExtension 结构枚举」用例 + `toValuesMap()` | 加字段忘了枚举 |
| **计数对账** | `:365` `singles.length == fieldCount` | 加字段忘了进 `copyWith` |
| **逐字段效果** | `:367` `isNot(base)` | 字段**接错**到别的字段 |

## 顺带记录：文件的整体形状（为下次抽读定位）
`AppPalette`(92) · `AppPalette.registry`(203) · **`AppColors`(406)** · `copyWith`(539) · `lerp`(572) ·
`toValuesMap`(598) · `AppMaterial`(662) · 四个 `registry`：`double`×2(759/810) + `Duration`×2(866/888)
⇒ **两个 `ThemeExtension`**（`AppPalette` / `AppColors`）—— 两者都受同一套三锚点保护。

## 未核实项
1. `tokens.dart` 其余 ~600 行未读（`AppPalette` 的字段/registry 内容、各 `registry` 的具体档位）
2. `design_tokens_test.dart` 余约 32 条断言体未读
3. `dev_tools_section.dart` 余面未读（1874 行）
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. 新露出的 208 条自我设防名只看了 12 条
6. Mod crates 逐文件覆盖率（9/61）—— **已是最大的结构性空白**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
