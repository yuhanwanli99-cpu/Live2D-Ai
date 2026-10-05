# BATCH-0337 · ⭐ 六个文案**都带分类信息** —— 而中文的**语气**本身承担了一部分区分

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `state_pill` 的六个 `UiPhaseView`

## 跑的命令（全部只读）
```
sed -n '52,96p' lib/ui/state_pill.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **四个可核点，第 2 个我没预期**
| 相位 | 文案 | 图标 | 色 |
|---|---|---|---|
| `offline` | **「后端未连接」** | `cloud_off` | **danger** |
| `idle` | 「空闲」 | `circle_outlined` | contentFaint |
| `thinking` | 「**思考中**」 | `psychology_outlined` | warning |
| `speaking` | 「**说话中**」 | `graphic_eq` | primary |
| `interrupted` | 「**已打断**」 | `stop_circle_outlined` | **contentMuted** |
| `error` | 「出错」 | `error_outline` | **danger** |

1. ⭐⭐ **`offline` 的文案是「后端未连接」而不是「离线」** ⇒ **它点名了是「后端」**
   ⇒ ⇒ 而 AGENTS v0.5.1 ⑩ 记过「状态胶囊**冒充**『后端未连接』」（B0000 ⑩：那条派生被删掉了）
   ⇒ ⇒⇒ **修完之后文案反而更准了** —— 因为它现在**真的只由 `wsConnected` 决定**（B0333 核过链的第 ② 位）
2. ⭐⭐ **两个「非模型」相位用完成/陈述语气**（「**已**打断」「出错」），
   而两个「模型」相位用**进行时**（「思考**中**」「说话**中**」）
   ⇒ ⇒⇒ ⭐ **中文的语气本身承担了分类** ⇒ 而 B0332 那句「6 个项目共用 1 种视觉，**只有文字变**」
   —— 这里的文字**不止是变，而是带着分类信息**
3. ⭐⭐ **`interrupted`（muted）与 `error`（danger）用了不同的语气与色**
   ⇒ ⇒ 而这两个是**最容易被混为一谈**的 ⇒ ⇒ **`danger` 留给 `error`、`interrupted` 降为 muted**
   ⇒ ⇒⇒ **「出错了」与「被叫停了」不该看起来一样重**
4. ⭐ 而 `offline` 与 `error` **同为 danger**、而 `idle`/`interrupted` 都是 muted/faint
   ⇒ ⇒ **色分三档，档位与「要不要紧」对齐** ⇒ ⇒ **色不是装饰、是分档**

⇒ ⇒ **0 findings**；⇒ ⭐ 而本批最值得记的是 ②：**六个「只有文字变」的状态，文字里带了语法信息**
⇒ ⇒ **这是 B0245「状态用文字、不靠颜色」家族里最深的一次** ——
**不是「不靠颜色」，是「文字比颜色携带更多信息」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~455）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
