# BATCH-0211 · 🔻 **撤回一条旧发现** + **扩大另一条的范围**（0 条新发现）

Phase 1 · 域覆盖 · 文档一致性复核（AGENTS.md 正文 vs 代码）

## 跑的命令（全部只读）
```
grep -rn "effectiveShellImage" shell/flutter/lib/
grep -rn "get effectiveBackground" shell/flutter/lib/settings/display_prefs.dart
grep -n "mod_count_is" AGENTS.md
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**撤回 F-0040-01**；**F-0209-01 范围扩大**
### ① 🔻 **撤回 F-0040-01**（审计的**第二次撤回**）
```
$ grep -n "mod_count_is" AGENTS.md
45:  两者移出 `AVAILABLE_MOD_FACTORIES`，7 → **5**，`mod_count_is_seven` →
46:  **`mod_count_is_five`**；crate 暂留 workspace（可编译可测）并标 **ARCHIVED**，
73:  `ModEventTopic::TurnEnded`。`AVAILABLE_MOD_FACTORIES` 6 → **7**，`mod_count_is_six` →
```
⇒ **全文已无 `mod_count_is_three`** ⇒ AGENTS.md `:45-46` 写的就是 **`mod_count_is_five`**
⇒ **与代码一致**（`main.rs:471`，B0170 已核）⇒ **F-0040-01 所指的文档缺陷已被修复**
⇒ ⚠ **连带撤回我 B0170 写的「影响升级」**（「**文档指向了一个不存在的符号**」**已不成立**）
⇒ **教训**：**一条发现的「对象」可能被修好** ⇒ 复核时**必须先核对象是否还存在**，
再谈它是否仍成立 —— **这与复核代码缺陷同构**（代码改了，结论也要重算）。
（**第二次撤回**：第一次是 B0069 的 F-0030-01「150ms 防抖我漏了」。）

### ② ⭐ 而 F-0209-01 **不是「单字段漂移」，是「一处机制、三处描述」**
```
$ grep -rn "effectiveShellImage" shell/flutter/lib/        ⇒ **零命中**（已被 :665 的 get effectiveBackground 取代）
$ grep -n "syncShellStageBg" AGENTS.md                     ⇒ :123 / :641
```
⇒ **同一处机制（壳画哪一张图），AGENTS.md 用三种说法描述，而三种都已失效**：
| # | AGENTS.md 的说法 | 代码的现状 |
|---|---|---|
| ① | `syncShellStageBg`（**字段**） | 已换成 **`backgroundSource` 枚举** |
| ② | **默认 `true`**（跟随舞台） | 新默认是「**背景库**」 |
| ③ | `effectiveShellImage`（**判据的 getter**） | 已改名 **`effectiveBackground`**（`:665`） |
⇒ ⇒ **发现范围应扩大**：不是「一个字段名写错了」，而是「**一个机制在文档里整体停留在替换前**」
⇒ 而这与 B0210 的结论**并不矛盾**：**「变更历史」那节落后 ≠ 正文落后** ——
**真正落后的是「正文中对某个机制的三处描述」**（那三处写于 rc.5、而替换发生在 09-27）。

## 未核实项
1. AGENTS.md 正文里**还有哪些**机制描述已被代码取代（我只核了「壳画哪一张图」这一处）
   ⇒ 下一批继续，**用同样方式逐个机制核**，而不是全文泛泛地找
2. `mod-wallpaper/src/strategy.rs`(655) 未读；`mod-template` 未读（Mod crates 逐文件 10/61）
3. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
4. `dev_tools_section.dart` 余面未读（1874 行）
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
