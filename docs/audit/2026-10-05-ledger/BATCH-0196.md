# BATCH-0196 · `dev_tools_section.dart`(1874)：**头注的「不拆」理由成立**；我两次猜名都错（0 条新发现）

Phase 1 · 域覆盖 · `shell/flutter/lib/settings/sections/dev_tools_section.dart`（前端最大未审文件）

## 跑的命令（全部只读）
```
grep -rln "dev_tools_section|DevToolsSection" test/ ; sed -n '1,14p' <file>
grep -nE "^class |^  Widget _" <file>
grep -c "_SectionScaffold|_ListWithActions|class _.*Scaffold|_InlineResult" <file>   # → 0
grep -nE "^class _" <file>
```
未跑任何 cargo / flutter / pnpm 命令。

## ① 头注给出了**不拆分的结构性理由**（在有人问之前就写了）
> 「合在一个文件里：它们**共享同一套『列表 + 动作 + 内联结果』的骨架**，
> **拆成四个文件只会让同一段列表渲染代码出现四遍**。每个类都短、职责单一，找起来**靠类名**即可。」

### 核验结果：**成立**（但我第一遍核错了方向）
- **类确实短、职责单一**：`AdminRow` · `AdminEmpty` · `ModelsSection` · `_ImportModelField(State)` ·
  `ModsSection` · `_ModConfigTile(State)` · `DiagnosticsSnapshot` · `DiagnosticsSection` …
  ⇒ 与「每个类都短」**一致**
- **私有 widget 只有 3 个**且**各自绑定一个分区**（`_ImportModelField` / `_ModConfigTile` / `_DebugPanelsState`）
  ⇒ **文件内没有「共用骨架」基类** ⇒ 但**头注也从未这么声称** ——
  它说的是「共享同一套**骨架**」（指**形状/约定**），并明说「找起来**靠类名**」
- **真正共享的逻辑在文件级**：`String modStateLabel(String key, {String? modId})`（:378 顶层函数）
  ⇒ **状态标签集中在一处**（正面模式 **P2** 的文件内形态）
- **24 处列表渲染点** ⇒ 「列表 + 动作 + 内联结果」这个**形状**在各分区**确实重复**
  ⇒ 而头注说的正是「**避免**让它在**四个文件**里各出现一遍」⇒ **取舍自洽**：
  **文件合一（收敛）** ＋ **形状在文件内重复（可见）**，好过**四个文件各抄一遍（不可见）**。

## ② ⚠ 诚实记：**我这一批两次猜名都错**（规则 3 变体 6 用在我自己身上）
1. 第一次：按「四个分区应有共用基类」的**预设**去 grep `_SectionScaffold|_ListWithActions|…`
   ⇒ **零命中** ⇒ 差点记成「头注声称的共享骨架**不存在** ⇒ 头注不实」
2. 第二次：把 `class AdminRow` 之类当成「共用骨架」⇒ 其实它们是**各自的行组件**
⇒ ⇒ **教训**（与 B0157 的「去掉另一半会怎样」同族）：
> **核一条「它说了 X」的注释时，要先确认它说的是不是「我以为的 X」。**
> 头注说的是**形状**（pattern），我核的是**基类**（base class）⇒ **两者不同 ⇒ 我的核验无效，而不是它不实。**
⇒ **可执行**：核「注释声称」时，**先把注释里的词换成代码里真实存在的名词**再 grep。

## ③ 顺带：它有 **5 个测试文件**覆盖（不同角度）
`restart_notice_test` · `models_section_test` · `pet_desktop_state_test` · `admin_api_test` · `mods_section_test`
⇒ 四个分区**各有对应测试文件** ⇒ 覆盖率不是问题。

## 未核实项
1. 该文件**实现本体未读**（1874 行；本批只核了头注理由 + 类结构 + 测试面）
2. `DiagnosticsSnapshot`(:1039) 的字段与刷新时机未读
3. 新露出的 208 条自我设防名只看了 11 条；`tokens.dart`(928) 未读
4. `settings_controller.dart:120-222` / `:296-409` 其余未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
