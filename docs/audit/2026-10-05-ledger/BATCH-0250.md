# BATCH-0250 · `live2d_stage.dart`：**「空 = 不动」与「`none` = 显式撤销」两态分明**，下发**恰好一次**（0 条新发现）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `live2d_stage.dart` 的 preset 下发语义

## 跑的命令（全部只读）
```
grep -nE "presetStatus|reset|清空|retry|_reset" live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 核到一处**极易混淆的两态区分**
| 行 | 内容 |
|---|---|
| `:53-54` | 「`applyForSeq`：**取到 cue 才下发**；`preset_id` **为空串 = 「本轮不动」（不调用下发）**；`preset_id == 'none'` = **显式撤销，照常下发一次**」 |
| `:77` | 「取用 + 下发：**空 `presetId` = 本轮不动**；其余（含 `'none'`）**恰好一次**」 |
| `:27` / `:31` | `kDefaultPresetIntensity = 1.2`，且与**手势**的同名常量**刻意不同**（表情调试的目的不同）⇒ P22 |

三个可核点：
1. ⭐⭐ **「空串」与「`none`」是两态**：前者 = **不动**（连下发都不发生），
   后者 = **显式撤销**（**照常下发一次**）⇒ ⇒ 这两态在**协议层也是两个值**
   （B0015 核过 WS `action_cue`；B0128 核过 schema 里 `expression_ids` 含 `"none"` 哨兵）
   ⇒ ⇒ **两个值、两种语义、且在函数文档上分开写**
2. ⭐ **「恰好一次」**：其余情况（含 `'none'`）**恰好下发一次**
   ⇒ ⇒ 与 SSE 的 `Done` 恰一次（B0156）、音频块的 `first/final_chunk`（B0132）**同纪律**
3. ⭐ 两个同名常量**刻意不同**且注明理由 ⇒ ⇒ **P22**（规则要写明「它不适用于谁」）

### ⚠ 我**没有**核到的东西（如实标注）
本批我**想**核的是「iframe 重建（`:120` 说 `error` 后会 `retry`）时，`presetStatus`
那份 ack 驱动的快照会不会留着旧值」——
⇒ **未核**（我这次的 grep 打在 preset 语义上，没落在 reset 上）
⇒ ⚠ 而这条**值得核**：B0232 已确认 `presetStatus` 是「**ack 驱动的显示快照**」，
⇒ 若重建后**旧快照不清**，就会**短暂显示一个已经不生效的表情状态** ⇒ 属「看起来没事」那一类
⇒ **列为下一批第一件事。**

## 未核实项
1. ⭐ **iframe 重建后 `presetStatus` 是否被清**（本批列的下一件事）
2. `dev_tools_section.dart` 余 ~1850 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
3. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
