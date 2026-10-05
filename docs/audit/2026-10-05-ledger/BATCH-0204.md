# BATCH-0204 · ⭐ P21 的三条约束**全部成立**，且「声明集」是**推导出来的**（0 条新发现）

Phase 1 · 域覆盖 · `design_tokens_test.dart`（台账两条自洁约束的实现）

## 跑的命令（全部只读）
```
grep -n "过期条目" -A 16 test/design_tokens_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**P21 从「用例名」核到「实现」**
### ① 过期条目检查：判据 + **可执行的失败文案**
```dart
final List<String> stale = kNotYetWired.where((String n) => (refs[n] ?? 0) > 0).toList();   // :302-304
expect(stale, isEmpty, reason: '以下令牌已经接线了，请从 kNotYetWired 里删掉：$stale');       // :305
```
⇒ 判据是「**引用次数 > 0**」⇒ **已接线的令牌必须从台账删掉** ⇒ **台账不能「只增不减」**
⇒ ⭐ `reason` **把违规的名字插进失败文案** ⇒ **报错直接告诉你删哪个**
（⇒ 正面模式 **P1**「理由要写『用户/维护者会看到什么』」**用在失败文案上**）

### ② 名字存在性检查：**声明集是推导的**，不是第二份手抄清单
```dart
final Set<String> declared = <String>{                       // :309-312
  for (final MapEntry<String, Set<String>> e in families.entries)
    for (final String n in e.value) '${e.key}.$n',            // ← **从 families 推导**
};
final List<String> bogus = kNotYetWired.where((n) => !declared.contains(n)).toList();   // :313-315
expect(bogus, isEmpty, reason: '台账里有不存在的令牌名：$bogus');                          // :316
```
⇒ ⭐ **没有第二份清单需要同步** ⇒ **这个检查本身不会漂移**
⇒ ⇒ 正面模式 **P2（私有汇合）** 用在**测试**上：真源只有 `families` 一处。

### ③ 顺带：`refs` 是**引用计数**而非布尔
`refs[n] > 0` ⇒ 同一遍扫描里**既知道「有没有被引用」、也知道「被引用了几次」**
⇒ **计数比标志多给信息**，且让两条约束共用一次遍历。

⇒ ⇒ **P21 三条约束全部核实**：名字必须真实存在（本批）· 不得含过期条目（本批）·
未被引用的必须显式登记（B0202：`要么被引用，要么在「未接线台账」里`）。

## 未核实项
1. `design_tokens_test.dart` 余约 36 条断言体未读
2. `tokens.dart` 本体（928 行）未读（含 `kNotYetWired` 与 `families` 的定义）
3. `dev_tools_section.dart` 余面未读（1874 行）
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 的采集点/轮转未核
5. 新露出的 208 条自我设防名只看了 12 条
6. Mod crates 逐文件覆盖率（9/61）
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
