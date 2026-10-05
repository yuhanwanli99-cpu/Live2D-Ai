# BATCH-0184 · ⭐ **F-0184-01（P2）**：`stripCommentsAndStrings` 8 份副本 · 1 份已漂移（且漂移的那份更强）

Phase 1 · 域覆盖 · `shell/flutter/test/`（P19 的**承重实现**核验）

## 跑的命令（全部只读）
```
grep -rn "String stripCommentsAndStrings" -A 8 test/ lib/
grep -rln "String stripCommentsAndStrings" test/ lib/ | wc -l
for f in <8 份>; do awk '/String stripCommentsAndStrings/,/^}$/' "$f" | md5sum; done
diff <多数版> <漂移版>
sed -n '/String stripCommentsAndStrings/,/^}$/p' test/glass_rim_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0184-01（P2）**
- **8 份副本**（`grep -rln`）· **7 份 md5 相同** · **1 份不同**
- 差异**只多一行**：`out.write('S');`（`design_tokens_test.dart`）
- ⇒ **多数版把字符串「删空」**（剥完 `continue;` 不写）⇒ **两侧代码会粘连**
- ⇒ **漂移版保留一个分隔符 `'S'`** ⇒ **更强**（粘连不会造成假匹配）
- ⇒ 且**零共享 helper**（`grep` 只见 8 份定义，无 import 共享版）

**P2 的根据**：不是「某条守卫已被骗过」（我**无法演示**），
而是「**同目录 8 份承重辅助函数、零共享、且已漂移一份**」这个**可核事实**。
**修法**：收敛成一份（`test/support/source_scan.dart`），**以漂移那份为准**；
并把「保留分隔符」这个关键性质**写进函数名**（P3/P19 的应用）。

## 未核实项
1. 8 条现有守卫的 needle 是否**都**为长而具体（我核了几条，未穷举 8 条全量）
2. 各副本的**调用点**是否有其它差异（只比了函数体）
3. `stripCommentsAndStrings` 对 **raw string / 三引号 / 转义** 的处理我只读到分支骨架，**未逐条核边界**
4. 未审 `.dart` 仍 187 个（`dev_tools_section.dart` 1874 · `tokens.dart` 928 …）
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
