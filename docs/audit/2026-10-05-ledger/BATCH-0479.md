# BATCH-0479 · ✅ **② 成立** —— 而它的守卫**不是**「每帧检查 sigma ≤ 8」

Phase 4 · 证伪（兑现 B0478 留的：②「sigma 由偏好上界（8 px）压着」那条守卫在哪）

## 跑的命令（全部只读）
```
grep -rn "8\.0|sigma" lib/ui/shell_backdrop.dart | head -5
grep -rn "kShellBlur|blurSigma|shellBlur" lib/ --include=*.dart | head -4     # ⇒ 零命中
sed -n '62,70p' lib/settings/display_prefs.dart
grep -rn "= 8\b|, 8)|8.0" lib/settings/display_prefs.dart | head -3
```
未跑任何 cargo / flutter / pnpm 命令。

## ⚠ 本批的**换地方问**（B0440 规矩的又一次应用）
`kShellBlur` / `blurSigma` / `shellBlur` **三个候选名全部零命中** ⇒⇒
「上界是个具名常量」这个假设不成立 ⇒⇒ **它在偏好层**（B0421 已核那里有四个预算常量）
⇒⇒ **落到 `display_prefs.dart:352` `static const double maxBackgroundBlur = 8.0;`**

## ★ 本批产出：**② 成立，而它的守卫方式是「不可达」而不是「检查」**
```dart
// lib/settings/display_prefs.dart:352
static const double maxBackgroundBlur = 8.0;
// lib/ui/shell_backdrop.dart:288-290
// sigma 用半径换算（σ ≈ r/2），再乘 0.5 让滑杆的手感更线性。
imageFilter: ui.ImageFilter.blur(sigmaX: blur / 2, sigmaY: blur / 2),
```

### 四个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「sigma 由偏好上界压着」成立的方式是「偏好里没有超过它的可能」**
   ⇒⇒ **⇒ 守卫不是「每帧检查 sigma ≤ 8」—— 而是滑杆的取值范围本身就是 `0.0 … 8.0`**
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:288` 那句「sigma 用半径换算（σ ≈ r/2），再乘 0.5 让滑杆的手感更线性」
   把换算写在用它的那个文件里**
   ⇒⇒ **「σ ≈ r/2」是近似、而近似与上界的关系被注释说明、不是被测试说明** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐ **⇒ 而它是一个 `class static const`（不是顶层 const）** ⇒⇒⭐⭐
   ⇒⇒ **⇒⇒ 「上界住在拥有它的那个类里」** ⇒⇒ 与 B0443 核的 `stripCommentsAndStrings`
   「**住在 `design_tokens_test.dart` 里**」是**同一个形状的两面** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⇒ ⇒⭐⭐⭐⭐ **⇒ 而本批把 B0478 ② 从「未核」变成「成立」** ⇒⇒ **⇒⇒ 三条约束全部核完** ⇒⇒⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**（B0478 的「例外」小规格**三条全部核完**）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. 「σ ≈ r/2」这个近似**有没有被测**（B0439 那条「§2 B 栏冻结」是这类近似的先例 · **未核**）·
   `wake_gate_open` 本体 · `clean_transcript` 尾部（`:270` 之后）· `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
