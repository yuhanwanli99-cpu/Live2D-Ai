# BATCH-0181 · ⭐ 三条结构型断言：**分工是「行为测纯模块 + 结构测调用」**，且断言**自带它要防的失败模式**

Phase 1 · 域覆盖 · `shell/flutter/test/chat_notice_test.dart`（结构型测试核验 2/2）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/chat_notice_test.dart` — （定点 150-172：三条结构型断言 + 一条 `testWidgets`）

## 跑的命令（全部只读）
```
grep -rn "走 \`settleTurn\`" -A 22 test/
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ 一处**分工**被看清 + 一条新正面模式
### ① 三条结构型断言，**每条都自带它要防的失败模式**（`reason:` 字段）
```
:150-156  controller.contains('settleTurn(')
         reason: 判据写在**纯模块**里才测得到；控制器自己写分支就[测不到]
:158-164  controller.contains('kWordlessTurnNotice')
         reason: 控制器另写字面量 → **改文案时漏改一处，测试与界面不一致**
:166-168  controller.contains('mustReleaseTurnOnWsLoss(')
```
⇒ ⭐ **断言 ① 的 reason 是本批最有价值的一句**：它说明了这套分工的**方向** ——
**决策逻辑必须住在纯模块里**（那样才测得到行为），控制器只负责**调用**。
⇒ 断言 ② 的 reason 则点名了它防的**具体危害**：**两份字符串 ⇒ 改一处漏一处**。

### ② ⭐ 于是分工是完整的，不是「用 grep 代替测试」
| 层 | 谁测 | 形式 | 核验 |
|---|---|---|---|
| **决策逻辑**（`settleTurn` 的七种结局） | `turn_liveness_test.dart` | **行为**（7 条真断言） | **B0139 我读过全文** |
| **控制器确实调用它** | `chat_notice_test.dart:150` | **结构**（`contains`） | **本批** |
| **调用真的存在** | —— | 我逐行核过 `:429` 签名 · `:460` 调用 | **B0140** |
⇒ ⇒ **turn 收口这条路径有三处独立确认**（行为测试 · 结构守卫 · 我本人读两端）⇒ **0 缺陷**。

### ③ ⚠ 但**诚实标注**这三条的固有弱点
`controller` 是 `chat_controller.dart` 的**源码文本** ⇒ `contains` **可能在注释或无关处命中**。
⇒ 这不构成发现（断言**能失败**，且 reason 说清了意图），但**它是这一类测试的共性弱点**：
> **正面模式 P18（新）**：**结构型断言要（1）自带 `reason` 说明它防什么（2）在同文件或邻文件里
> 有一条**行为**测试覆盖那个被引用的纯逻辑** —— 两者合起来才叫「测过」；
> 只有结构断言、找不到对应行为测试的，才降级为「仅防止架构被改回去」。**
⇒ 本文件三条**都满足**（① 的纯逻辑 `settleTurn` 在 `turn_liveness_test.dart` 有 7 条行为测试；
② 的 `kWordlessTurnNotice` 是**常量**、其值在同文件别处被断；③ 的 `mustReleaseTurnOnWsLoss`
在 `turn_liveness_test.dart` 也有行为测试——**待核**，见未核实项）。

## 未核实项
1. ⭐ `mustReleaseTurnOnWsLoss` **是否有行为测试**（P18 的三个条件之一，**下一批第一件事**）
2. `F-0002-2`（`_store` 一开始就是真库）断言体**未核** —— B0179 留的第三条
3. 上面三条 `contains` 断言**未逐条核**是否存在对应的行为测试（我只核了 ①）
4. 未审 `.dart` 仍 187 个（`dev_tools_section.dart` 1874 · `tokens.dart` 928 · `live2d_stage.dart` 666 …）
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
