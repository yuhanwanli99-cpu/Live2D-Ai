# BATCH-0481 · ✅ **相邻两条测试明说两种相反的策略** —— 而分界是「值域里有没有语义」

Phase 4 · 证伪（兑现 B0480 留的：`:645` 那条断言的形状）

## 跑的命令（全部只读）
```
sed -n '636,660p' test/display_prefs_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0480 的观察被确认、且更强**（0 条新发现）
```dart
// test/display_prefs_test.dart:641-654
test('越界 / 非有限数一律 **clamp 到区间**（模糊不许变负）', () {
  expect(DisplayPrefs.clampBackgroundBlur(double.nan), 0.0);
  expect(DisplayPrefs.clampBackgroundBlur(-5), 0.0);
  expect(DisplayPrefs.clampBackgroundBlur(99), **DisplayPrefs.maxBackgroundBlur**);   // 引用常量
  // 非有限数（含 ±Infinity）一律回落默认，与 clampVolume 等同一条纪律。
  expect(DisplayPrefs.clampBackgroundOpacity(double.infinity),
      DisplayPrefs.defaultBackgroundOpacity);
  expect(DisplayPrefs.clampBackgroundOpacity(-1), 0.0);
});

// :656
test('枚举字段越界**回落默认**而不是夹到端点', () {
  // 端点有语义（**0=auto / 1=无**），**把坏值夹到 1 会让背景不可读**。
  // **不含 `slideInterval`：DEC-1（2026-09-28）把它改成了端点夹持，
  // 回归在 `display_prefs_background_fit_test.dart` 的 DEC-1 组**。
```

### 四个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 两条相邻测试构成一对、而这对明说两种**相反**的策略**：
   | 测试 | 坏值怎么处理 | 为什么 |
   |---|---|---|
   | `:641` 「一律 **clamp 到区间**（模糊不许变负）」 | **夹到端点** | 连续量 · 端点无语义 |
   | `:656` 「枚举字段越界**回落默认**而不是夹到端点」 | **回落默认** | **端点有语义**（`0=auto` / `1=无`） |
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而分界正是「这个值域里有没有哪个值本身带着语义」**
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 这正是 B0453 那条判据**；
   而 B0473 核的 `manual_enabled` 是它的**反例**（值域里没有有意义的值 ⇒ 随便回落）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:656` 把那层语义说成了**用户可见的后果**：
   「**把坏值夹到 1 会让背景不可读**」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒ 「端点有语义」不只被断言、还被翻译成一个界面上的坏结果** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:656` 还做了**第三件事**：「**不含 `slideInterval`：DEC-1（2026-09-28）把它改成了端点夹持，回归在 `display_prefs_background_fit_test.dart` 的 DEC-1 组**」
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「这一条规则的**边界**」被写下来了 —— 而它**是一条被改过的规则**（DEC-1）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒ 「例外被点名 + 回归被指路」两个动作在这里同时发生** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐ **⇒ 而 `:641` 里那条注释「非有限数（含 ±Infinity）一律回落默认，**与 `clampVolume` 等同一条纪律**」** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒ 「同一条纪律在另一个字段上也这么用」**被写成了一句话** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 1 点**
> **相邻两条测试明说两种相反的策略，而分界是「这个值域里有没有哪个值本身带着语义」**
> **⇒⇒⇒ ⇒⇒⇒ ⇒ 模糊 / 不透明度（连续量、端点无语义）⇒ **夹**；枚举 / 滑杆间隔（端点有语义）⇒ **回落默认****
> **⇒⇒⇒ ⇒⇒⇒ ⇒ 而「把坏值夹到 1 会让背景不可读」**把那层语义说成了用户可见的后果** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `display_prefs_background_fit_test.dart` 的 **DEC-1 组**（`:656` 指名的那个 · **未读**）·
   `wake_gate_open` 本体 · `clean_transcript` 尾部（`:270` 之后）· `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
