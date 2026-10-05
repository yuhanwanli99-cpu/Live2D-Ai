# BATCH-0183 · ⭐⭐ **先剥注释与字符串再匹配** ⇒ `contains` 从「grep」升级为「调用形状断言」

Phase 1 · 域覆盖 · `shell/flutter/test/background_hydration_window_test.dart`（结构型测试核验 3/3）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/background_hydration_window_test.dart` — （定点 229-262：
   `F-0002-2 / F-0002-3` 结构性守卫组）

## 跑的命令（全部只读）
```
grep -n "F-0002-2" -A 26 test/background_hydration_window_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐⭐ 立**正面模式 P19**，并**修正 B0181 的诚实标注**
### ① ⭐⭐ 关键的一行（:232-233）
> 「扫的是**调用形状**（不是某一行字），而且先**剥掉注释与字符串** ——
> 否则**注释里提一句旧实现就会把守卫自己判红**。改坏接线断言就红。」

```dart
final String src = stripCommentsAndStrings(File('lib/main.dart').readAsStringSync());   // :234-236
```
⇒ ⭐ **它正好消掉了我在 B0181 标注的那个弱点**：
我那批写「`contains` **可能在注释或无关处命中**」⇒ **这里先把注释与字符串剥掉**
⇒ ⇒ `contains` **不再是 grep，而是「调用形状」断言**。

### ② 而 `reason` 给的是**因果链**，不是口号
```dart
reason: 'createBackgroundStore 是**同步**构造（open 惰性在实现内部），**根本不必等水合** ——   // :242-243
        占位内存库一旦暴露给可写路径，**窗口内导入的字节就进了这个即将被丢弃的实例**'      // :243-245
…
reason: '主入口里不该再有内存占位库（**代码路径，不是注释里提一句**）'                    // :249
```
⇒ 第一条把**危害的机制**写成一句话（同步构造 ⇒ 不必等水合 ⇒ 占位库暴露 ⇒ 窗口内导入的字节进了将被丢弃的实例）
⇒ 第二条**主动承认了「代码路径 vs 注释」这个区分** ⇒ 正是我提的那个关注点，**他们自己先提了**。

### ③ 而且是**正反两条**（不止「包含」）
`src.contains('_store = createBackgroundStore()')` **为真** ＋ `src.contains('MemoryBackgroundStore(')` **为假**（:246-250）
⇒ **重引入占位库会被第二条抓住** ⇒ 这是 P7（配反向对照）用在**否定式**守卫上。
⇒ 组名也诚实标注：`F-0002-2 / F-0002-3：接线顺序与「假兜底」（**结构性守卫**）`（:229）· 并写明
「这几条**只能扫源码**：`main.dart` 在 VM 里加载不了（依赖 `package:web`）」⇒ **P17 的两个条件齐**（B0180 核过同类）。

### ④ ⭐ 因此**修正 B0181 的诚实标注**
B0181 我写「`contains` **可能在注释或无关处命中**」⇒ 现在要区分两种情况：
> **不剥注释的 `contains` ⇒ 只算「防止架构被改回去」（B0181 的 `chat_notice_test.dart` 属此类）；
> 剥了注释与字符串的 `contains` ⇒ 才算「断言调用形状」（本文件属此类）。**
⇒ **P18 与 P19 合起来才是完整的判据。**

## 未核实项
1. `stripCommentsAndStrings` **本体未读**（它自己正确吗？**这是 P19 的承重实现** ⇒ 值得核）
2. 本文件其余断言（`F-0002-3` / `F-0001-3` 之后的）未逐条核
3. 未审 `.dart` 仍 187 个（`dev_tools_section.dart` 1874 · `tokens.dart` 928 · `live2d_stage.dart` 666 …）
4. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
