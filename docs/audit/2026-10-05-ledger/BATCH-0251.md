# BATCH-0251 · ⭐ 快照**只由 ack 生成**，而「清空」是**一等类型**（`clear`）⇒ 缺口若存在也只是**一次调用**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `preset_status.dart`(106) 全读（B0250 留的核验）

## 跑的命令（全部只读）
```
grep -nE "reset|clear|rebuild|retry|ready|onReady" preset_status.dart
wc -l preset_status.dart ; sed -n '60,82p' preset_status.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**B0250 留的问题**得到**精确界定**（而非「核到了/没核到」）
```dart
enum PresetStatusAction { **set, clear, ignore** }                        // :60
…
/// 渲染面 ack → 快照更新。
/// - preset-applied → 生成一条（id 取 ack 的 id，缺 id 时回落 field；时长取渲染面给的 ttl_ms）；
/// - preset-replaced / preset-expired / preset-dropped → **清空**（该动画段结束）；
/// - segment-ended → **不动**（它是音频事件，与预设显示无关）。            // :68-76
```
四点可核：
1. ⭐ **「清空」是一等类型**（`PresetStatusAction.clear`），**不是「没有值」**
   ⇒ ⇒ **清除是一个可以被要求发生的状态**，而不是靠「超时自然消失」
2. ⭐ **四种 ack 的映射被逐条写明**，且「不动」的理由也写了（`segment-ended` 是**音频事件**，与预设显示无关）
3. ⭐ `presetStatusUpdateFor(RenderEvent, DateTime, {source})` 是**纯函数**
   ⇒ ⇒ **同一个事件必得同一个更新** ⇒ ⇒ **不需要渲染面就能测**
   ⇒ ⇒ 与 B0232 核的「它**只由**渲染面 ack 生成，**真源在渲染面**」**接得上**
4. ⇒ ⇒ **B0250 的问题被精确界定**：快照**不由本地计时器清**、**不由重建路径清**，
   **只由渲染面自己的 `preset-replaced/expired/dropped` 清**

### ⚠ 仍**未核**的那一半（不夸大）
iframe 重建后，Flutter 侧**手上仍留着上一条快照**，直到新 ack 到达
⇒ ⇒ **「重建后是否主动发一次 clear」我没核到**（那是宿主侧的接线，不在本文件）
⇒ ⇒ 但 ⭐ **好消息是缺口的形状很轻**：`clear` 已经是**一等动作**，
⇒ ⇒ **若真存在这个缺口，修法是「重建后调一次 clear」，而不是「造一套新的机制」**
⇒ ⇒ 这正是把清理**建模成一个动作**（而非依赖「值自然消失」）买到的東西。

## 未核实项
1. ⭐ **iframe 重建后宿主是否主动发 `clear`**（本批**明确未核**，且这是宿主侧接线）
2. `dev_tools_section.dart` 余 ~1850 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
3. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
