# BATCH-0411 · ✅ **+1 条 P2：「对齐」不成立，两侧差四个字符** —— 而它**修正了我 B0350 的说法**

Phase 4 · **证伪**（逐字比两侧的句末标点集合 · B0410 留）

## 跑的命令（全部只读）
```
grep -nA 3 "kSentenceEnders = " shell/flutter/lib/state/live_region.dart
grep -rn "。！？" crates/live2d-ai-runtime/src/ --include=*.rs | head -4
sed -n '10,16p' crates/live2d-ai-runtime/src/dialogue/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0411-01 (P2)**
> Dart（`live_region.dart:19-22`）：「句末标点（**与 `live2d-ai-runtime` 的分句标点对齐**）」
> `const String kSentenceEnders = **'。！？…；\n'**;`
> Rust（`dialogue/mod.rs:11-12`，对 `SentenceAssembler` 的记载）：「中英文 **`.?!。！？…\n`** 均为句界」

**逐字比对**：
| 字符 | Dart | Rust |
|---|---|---|
| `。` `！` `？` `…` `\n` | ✅ | ✅ |
| **`.`（ASCII 句点）** | ❌ **缺** | ✅ |
| **`?`（ASCII 问号）** | ❌ **缺** | ✅ |
| **`!`（ASCII 感叹号）** | ❌ **缺** | ✅ |
| **`；`（全角分号）** | ✅ **多** | ❌ |

⇒ ⇒ **两个方向都不同** ⇒⇒⇒ **「对齐」这个断言不成立**

### 影响
- ① **英文句尾不触发「立刻播报」** ⇒ 退化为 1.5 s 限速兜底（**体验降级、不丢信息**）
- ② **全角分号处提前播报**（**提前、不丢内容**）
- ⇒⇒ **两个方向都不丢内容 ⇒ 故 P2 非 P1**（按 P1 判据「可察觉的故障 / 数据丢失」）

### ⇒ ⇒⭐⭐ 而本批**修正了我自己 B0350 的一条说法**
> 我 B0350 记：「**读屏播报与切句共用『整句到达』判据**」⇒⇒ **当时的依据是**注释****
> ⇒⇒⇒ **逐字比之后：那条说法只对「判据的**形状**」成立、对「标点**集合**」不成立**
> ⇒⇒⇒⭐ **⇒ 这与 B0410 那条是同一个病**（核了注释、没核另一处）⇒⇒ **两批连续两次犯它**

### ⇒ ⇒⭐⭐ 可提炼
> **「跨语言对齐」这类断言必须被一条跨语言断言守住** ——
> 否则它只是一句**会过期的注释** ⇒⇒ **同 B0344「门禁防的是过期合规」的应用**：
> **那一次防的是覆盖表，这一���防的是「两份枚举」**

⇒ ⇒ **修法（一行）**：让集合逐字相同 + 注释改准；**或**把注释改成「只认全角（英文句尾走 1.5 s 兜底）」。
⇒ ⇒ **反证已写**（若 Rust 侧 `mod.rs:11-12` 过时，则本条至少是 P3 级的**注释不准确**、不整体失效）。

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `SentenceAssembler` 的**实际判据出处**（`mod.rs:11-12` 是不是唯一依据）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
