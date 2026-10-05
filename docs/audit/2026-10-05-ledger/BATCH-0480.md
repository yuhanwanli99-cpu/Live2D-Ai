# BATCH-0480 · ✅ **上界在三个地方各出现一次** —— 而我 B0479 的「不可达」那句**要收窄**

Phase 4 · 证伪（兑现 B0479 留的：「σ ≈ r/2」这个近似有没有被测）

## 跑的命令（全部只读）
```
grep -rn "maxBackgroundBlur" lib/ test/ --include=*.dart | head -6
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**三处引用、一个常量、而我上一批的措辞被自己收窄**
```
lib/settings/display_prefs.dart:352   static const double maxBackgroundBlur = 8.0;      // ① 常量
lib/settings/display_prefs.dart:812   /// 背景模糊**夹到** `[0, maxBackgroundBlur]`；非有…
lib/settings/display_prefs.dart:815   return value.clamp(0.0, maxBackgroundBlur);      // ② 写入口夹一次
lib/settings/sections/appearance_background.dart:540
                                       max: DisplayPrefs.maxBackgroundBlur               // ③ 滑杆上界
test/display_prefs_test.dart:645      DisplayPrefs.maxBackgroundBlur                      // ④ 回归引用它
```

### 四个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 三处引用（常量 · 写入口 `clamp` · 滑杆 `max`）+ 一处测试**，
   **而它们读的都是同一个常量** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而我 B0479 说的「守卫不是检查、是**不可达**」要收窄** ——
   **它**同时**是一次检查**（`:815` 的 `clamp`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   **⇒⇒ 「守卫不是**每帧**检查」那句仍成立**（`shell_backdrop` 读到的 `blur` **一定**在界内、因为它是从偏好来的）
   **⇒⇒ 「不是不可达」那句要改** ⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ **这是「措辞被自己下一批收窄」的又一处**（另见 B0428 / B0412 / B0457）** ⇒⇒⇒⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐ **⇒ 而 `:812` 用的是「**夹到** `[0, maxBackgroundBlur]`」** ⇒⇒ **「夹」是一个动作、「区间」是一个集合** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 而注释**同时说了两者**（做什么 + 做到哪）⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐⭐ **⇒ 而 `test/display_prefs_test.dart:645` 引用了这个常量** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ **「回归引用它」**而不是**「回归写死 8.0」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ 这与 B0474 核的「`voice_wake_default_consistency_test` 读 Rust 源码」**是同一个原则的轻量版**：
   **⇒⇒ 测试引用常量、于是改常量时测试跟着变、而**不会因为两边都写死同一个数而假绿**** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 2 点**
> **我 B0479 说的「守卫不是检查、是不可达」要收窄 —— 它同时是一次 `clamp`（`:815`）**
> **⇒⇒ 「守卫不是**每帧**检查」仍成立**（读到的 `blur` 一定在界内）· **「不是不可达」要改**
> **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ **⇒ 这是「措辞被自己下一批收窄」的又一处**（B0428 / B0412 / B0457）**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐ `test/display_prefs_test.dart:645` 那条**断言的形状**（它断言 `clamp` 的结果、还是只引用常量）·
   `wake_gate_open` 本体 · `clean_transcript` 尾部（`:270` 之后）· `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
