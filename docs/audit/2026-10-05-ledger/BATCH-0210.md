# BATCH-0210 · 🔻 **我自己推翻了上一条发现的核心论据**

Phase 1 · 域覆盖 · 文档新鲜度（核实 B0209 留的问题：孤例还是系统性）

## 跑的命令（全部只读）
```
grep -rhoE "2026-09-(1[5-9]|2[0-9]|30)" crates/ shell/flutter/lib shell/flutter/test scripts/ .github/ xtask/ | sort | uniq -c
grep -oE "2026-09-[0-9]{2}" AGENTS.md | sort -u | tail -3
grep -n "syncShellStageBg|backgroundSource|壳跟随舞台" AGENTS.md
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**撤回 F-0209-01 的一条核心论据**
### ① 机械核验的结果：**AGENTS.md 正文并没有落后**
```
$ grep -oE "2026-09-[0-9]{2}" AGENTS.md | sort -u | tail -3
2026-09-26
2026-09-27
2026-09-28                      ← ⭐ AGENTS.md **正文**已到 09-28
$ （代码侧最晚的日期标记）
2026-09-28                      ← 代码也只到 09-28 ⇒ **两者同步**
```
⇒ ⇒ **我 B0209 说的「AGENTS.md 落后代码 13 天」是错的。**
**错的只是「变更历史」那一节**（最后一条 2026-09-14）⇒ **该节落后 14 天，正文并未落后**。

### ② 而**发现本身仍成立**（收缩后）
AGENTS.md **正文** `:123` 与 `:641` **仍把 `syncShellStageBg` 写成现行机制**（默认 `true`＝壳与舞台共用一张图）
⇒ ⇒ **受影响的是「正文里的机制描述」**，不是「整个文件的新鲜度」。

### ③ ⭐ 而这暴露了我**同一形态上的第 4 次犯错**
| 批 | 我做错什么 |
|---|---|
| B0104 | 凭局部印象断言「**唯一**」 |
| B0186 | grep 零命中 ⇒ 差点断言「文件里没有测试」 |
| B0196 | 按预设找「共用基类」⇒ 差点断言「头注不实」 |
| **B0209** | **读了一节（「变更历史」）、把它推广到全文** ⇒ 差点断言「整份文件落后 13 天」 |
⇒ ⇒ **共同形态**：**用一个局部的观察支撑一个全局的结论**。
⇒ **教训升级**：**「某节落后」不能推出「全文落后」** ⇒ 要推广任何结论，
**必须先机械核一遍全文**（本批就是靠 `grep -oE` 核出来的，**成本一次 grep**）。

## 未核实项
1. AGENTS.md 正文里还有**哪些**描述已被代码取代（我只核了 `syncShellStageBg` 一处）
   ⇒ **这是 B0209 那条发现真正该扩大/收缩的地方**
2. `mod-wallpaper/src/strategy.rs`(655) 未读；`mod-template` 未读（Mod crates 逐文件 10/61）
3. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
4. `dev_tools_section.dart` 余面未读（1874 行）
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
