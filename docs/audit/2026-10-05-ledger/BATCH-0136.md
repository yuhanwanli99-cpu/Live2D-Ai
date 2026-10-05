# BATCH-0136 · ⭐ `ConversationUiEvent` 7 变体 ↔ 7 个 WS 投影，**且 9→7 的分流是文档化的**

Phase 1 · 域覆盖 · 事件链的**最外层**（WS）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/app_event.rs` — （机械枚举 `ConversationUiEvent` 全部变体，:156）
2. `crates/live2d-ai-desktop/src/web_api/ws/events.rs` — （机械枚举 WS 投影臂）
3. `crates/live2d-ai-desktop/src/web_api/ws/{audio,broadcaster}.rs` — （定点 40-49 / 164-173：音频分流与历史缺陷）

## 跑的命令（全部只读）
```
awk '/enum ConversationUiEvent/,/^}$/' app_event.rs | grep -E "^    [A-Z][A-Za-z]+" | awk '{print $1}'
awk '/AppEvent::Conversation/,/^}$/' ws/events.rs | grep -oE "ConversationUiEvent::[A-Za-z]+" | sort | uniq -c
grep -rn "AudioChunk|SentenceVoiced" ws/*.rs
grep -n "EngineEvent::AudioChunk" -B 3 -A 6 ws/*.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**整条事件链已端到端画完**
### ① 7 变体 ↔ 7 投影，**零缺口**
```
ConversationUiEvent（app_event.rs:156，7 个）：
  NewEpoch · ReasoningDelta · VoiceStarted · VoiceEnded · TextDelta · TextFallback · ActionCue
WS 投影（ws/events.rs）：以上 7 个**各恰好 1 条**（ActionCue 3 次引用、ReasoningDelta 2 次，均在同一臂内）
```
⇒ **每一个 UI 事件都上了线**，无死变体。

### ② 9 → 7 的差额**不是丢失，是文档化的分流**
`AudioChunk` 走 **`ws/audio.rs` + `broadcaster.rs`** 这条**独立通道**（不进 `ConversationUiEvent`），
其契约写在 `audio.rs:40-49`，其中：
> `first_chunk` / `final_chunk`：**两者都由引擎给出**（:46）
> 「**一句音频会被切成十几块，只有引擎知道哪块**」

### ③ ⭐ 顺带核到**句界红线的历史缺陷记录**（`broadcaster.rs:169-173`）
> 「**2026-09-11 修**：这里过去用 `audio_epoch_s…` 把『**一轮语音的首片**』当成了句子边界，
> epoch 记账**永远推不出句界**——实测 WS 上……前端按 `start`→`end` 攒句会攒错、播出……
> **本方法不再做任何推断**，那份 epoch …」

⇒ 与我在早前批次核到的 AGENTS 规则（「句子边界必须由引擎给出……**不得在发射侧用记账/推断去猜**」）
**完全对应**，且代码自己记着「**曾经就是用 epoch 推断的，错了**」⇒ 该规则是**事故驱动的**，
不是凭空立规。这与 B0133 的 `TextFallback`、B0086 的 `stage_css` 属同一族。

### ④ 完整事件链（**本审计的最终地图**）
```
EngineEvent（9 种）                      ← 引擎内部
  └─ handle_engine_event（9 臂，**无兜底**）   ← supervisor（B0135）
       ├─ ConversationUiEvent（7 种）
       │    └─ ws/events.rs（**7 条投影，无死变体**）   ← WS 文本/控制帧
       └─ AudioChunk
            └─ ws/audio.rs build_audio_frames + broadcaster.rs（**独立通道，不做推断**）  ← WS 音频帧
```
⇒ **四个环节全部核过，每个环节都有「完整性」的机械证据**（9/9、9/9、7/7、7/7）。

## 未核实项
1. `ConversationUiEvent` **各变体的投影体**未逐条读（本批只核「有投影」与「变体↔投影对应」）
2. `conversation/mod.rs` 其余 ~500 行未读（`TurnReport` 字段 / 配置类型 / 测试）
3. `engine.rs` 余 390 行（Stage A/B/C 主体）· `turn.rs` 余 530 行未读
4. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
