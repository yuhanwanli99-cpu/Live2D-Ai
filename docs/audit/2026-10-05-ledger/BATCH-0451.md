# BATCH-0451 · ⭐⭐⭐ **F-0387-01 的前提被证成** —— 而性质因此**收紧**为「**功能在、索引缺**」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— **唤醒闸 / 手动闸的输入框**在不在（B0387 那条 P2 的前提）

## 跑的命令（全部只读）
```
grep -rn "wake_phrase|manual_enabled" lib/ --include=.dart | head -5
grep -n "ModFieldKind.bool|ModFieldKind.string" lib/settings/sections/dev_tools_section.dart | head -3
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0387-01 的前提成立、性质收紧**（0 条新发现）
| 证据 | 含义 |
|---|---|
| `lib/settings/mods/**voice_input_panel.dart**:77` `'wake_phrase_set': '**唤醒词已自定义**'` | 面板里**有一个专门的状态展示** |
| `dev_tools_section.dart:706/712/715` `ModFieldKind.string` / `.bool` **都在 spec 驱动的表单里** | `wake_phrase`(string) 与 `manual_enabled`(bool) **各有一个输入框** |
| `main.dart:976-986` 「读当前生效的唤醒词：voice-input Mod 的 `config.wake_phrase`」 | B0384 核的那条链在 **D 侧有一份镜像** |

### 三个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 「面板里有、文档表里没有」被证成**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 而这让 F-0387-01 的性质更准了**：
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **它不是「功能缺失」、是「**索引**与实现不同步」**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「功能在、索引缺」比「功能缺」轻得多**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ ⇒ 而 B0387 的修法（**补两行表格**）**正好对应这个性质** ⇒⇒⇒⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐ **⇒ 而 `voice_input_panel.dart:77` 那张状态表**是 B0426 核的那种**「值归服务端、文案归客户端」**的又一实例
   ⇒⇒ **⇒⇒ 「`wake_phrase_set` 这个**码**由 Mod 侧给、这句中文由前端写」** ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒ 与 B0426 那条**完全同型**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ 「同一个模式在第五次出现」**
3. ⭐⭐⭐⭐⭐ **⇒ 而这一批是「反证条款用上、结果为真」的第二例**（第一例是 B0412）
   ⇒⇒⭐⭐ **⇒⇒ ⇒⇒ B0412：反证不成立、发现被**加强**；本批：前提被**证成**、性质被**收紧**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 两种结果都算「条款被用了」**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ ⇒ 而「被用了」比「结论对我有利」更重要** ⇒⇒⇒⭐⭐⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~440 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
