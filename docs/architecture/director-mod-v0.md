# 导演 Mod v0（已注册第 5 个）：决策**一等面板** + **零投递**，不做 apply-to-TTS

> **状态**：2026-09-14 **产品级加强波次**（director 轨道）。
> 本 crate 已在 `AVAILABLE_MOD_FACTORIES` 中**注册**（产品级加强波次后注册面共 **5 个**：
> `external-input` / `persona` / `voice-input` / `memory` / `director`），**缺省停用**
> （不进 `cli_entry::default_mods_manifest`）。`wallpaper` / `pet-desktop` 已 **ARCHIVED**
> （移出注册表、不再编译进 binary），见 [ARCHIVED-mods.md](ARCHIVED-mods.md)。
>
> 版本保持 `0.2.0-rc.3`：**不改版本号、不 bump、不打 tag**。
>
> 契约来源：[director-rfc.md](director-rfc.md)（Wave 2 草案）。本文档是它的**实现面**，
> 与 `crates/live2d-ai-mod-director/` **逐条一致**；冲突时以代码 + 回归为准并回改本文档。
> 集成清单：[REGISTER-director-v0.md](../plans/parallel-mods/REGISTER-director-v0.md)。

## 0. 一句话

每轮输入正文经**纯函数**推导成 `{emotion, intent, suggested_tts:{speed,pitch}}`，
写进日志与 `state_json`；**没有任何下行通道**——`action_tx` **零调用**、
`apply_settings` **零调用**、`live2d-ai.toml` **零写入**。

产品级加强波次把「看得到」这条做实：`state_json` 新增 **`latest`**（最近一条决策，
前端直接消费，结构仍与 `recent_decisions` 的单条**逐字同形**），Flutter 的
`DirectorPanel` 把 `latest` 与最近 N 条渲染成**一等面板**（带标签的行 + 旧到新列表），
并提供 `command("clear")` 清空账本。**零投递语义一行未改**。

## 1. 范围与闭环定义（产品级加强波次）

| 项 | 内容 |
| --- | --- |
| **闭环定义** | 启停可用（唯一真源 = manifest `enabled`）→ 订阅两个主题 → `state_json` 可观察 `latest` + 最近 N 条 → **一等面板可读** → `clear` 可清空 → **不投递** |
| **输入** | `TurnPrompt`（本轮正文）+ `TurnEnded`（turn id），**正好两个主题** |
| **输出** | tracing 日志（**不含正文原文**）+ `GET /api/v1/mods/director/state`（新增 `latest`）+ `POST /api/v1/mods/director/command`（`clear`） |
| **前端** | `shell/flutter/lib/settings/mods/director_panel.dart`（**只改这一个面板文件**） |
| **缺省** | **停用**（不进 `cli_entry::default_mods_manifest`） |
| **crate** | `crates/live2d-ai-mod-director/`：`lib.rs`（Mod 集成 + `command`）/ `decision.rs`（纯函数）/ `ledger.rs`（账本）/ `tests.rs`（**33 条回归**） |

```text
用户输入 ──► supervisor 发 TurnStarted(turn id)
             └► supervisor 发 TurnPrompt(本轮正文)   ◄── director 在这里推导
                  └─ derive(正文, lexicon) → Decision
                  └─ 写账本（seq/emotion/intent/suggested_tts），**零副作用**
             └► TurnEnded(turn id)                    ◄── director 在这里结项
                  └─ 给最近一条未结项决策写上 turn id + closed=true
```

两条时序事实（读 `supervisor.rs` 得出，不是设计愿望）：

1. `TurnPrompt` 只带**正文**、不带 turn id；`TurnEnded` 只带 **turn id**、不带正文。
   两者由 host 在**同一个 Mod worker 上顺序投递**，且 supervisor 一轮内不并发
   （提交 → 跑完 → 发 `TurnEnded`），所以「最近一条未结项决策」就是本轮。
2. 本 Mod **不**在 `TurnPrompt` 里做任何配置写回，所以 RFC §1 那条
   「只对下一轮生效」的时序代价在这里**不成立**（本 Mod 根本不写配置）。

## 2. 输入（订阅面）

| 主题 | payload | director 怎么用 | 限制 / 坑 |
| --- | --- | --- | --- |
| `ModEventTopic::TurnPrompt` | 本轮输入**正文** | **权威输入**：清洗 → 纯函数推导 → 记一条决策 | 空 / 纯空白 → **静默轮**（不计决策、不计错误） |
| `ModEventTopic::TurnEnded` | turn id（与 `TurnStarted` 同序号） | 给最近一条未结项决策**结项**（写 turn id、置 `closed`） | 无在飞轮的 `TurnEnded`（乱序 / 重复）→ `errors += 1`，**不是**主链失败 |

其余主题（`TextDelta` / `VoiceStarted` / `VoiceEnded` / `ModelActivated` /
`ActionFinished` / `TurnStarted`）**不订阅**；收到未订阅主题时只记一条 info 日志、
零状态变化（回归 `tests::unsubscribed_events_are_ignored_without_side_effects`）。

## 3. 决策 = 纯函数（`decision.rs`）

```rust
pub fn derive(text: &str, lexicon: Lexicon) -> Decision
// Decision { emotion, intent, suggested_tts: TtsSuggestion { speed, pitch } }
```

无 IO / 无网络 / 无时钟 / 无随机；同一输入恒等输出（回归 `derive_is_deterministic`）。

### 3.1 情绪（`EmotionHint`）

`neutral` / `happy` / `sad` / `angry` / `surprised` / `anxious` / `affectionate`。

- **词表打分**：固定关键词表 × 权重（弱 = 1、强 = 2），六种情绪各自求和；
- **否定闸**：命中点前一个字是 `不 / 没 / 未 / 别 / 无 / 莫` 时该次命中作废
  （`我不开心` **不是** Happy）；
- **ASCII 词边界**：ASCII 关键词必须两侧不是 `[A-Za-z0-9]`（`unhappy` **不是** Happy）；
- **平局裁决**：分数相同取优先级靠前者——
  `Happy > Sad > Angry > Surprised > Anxious > Affectionate`（`EMOTION_PRIORITY`，
  与 `score_emotions` 的返回顺序同源，回归钉住）；
- **fail-safe**：全 0 → `neutral`（未知词 / 纯标点 / 无命中都不是故障）。

### 3.2 意图（`IntentHint`）

`chat` / `question` / `greeting` / `farewell` / `request` / `complaint` / `silence`。

**首个匹配者胜**，顺序固定：`Question > Greeting > Farewell > Request > Complaint > Chat`。

- Question：含 `？` / `?` 或 `吗/呢/什么/怎么/为什么/如何/哪`；
- Greeting / Farewell / Request / Complaint：各自关键词表（含少量中英词）；
- `Silence` 只在**清洗后正文为空**时出现（`derive` 走 `Decision::silent()`）。

### 3.3 词表档位（`Lexicon`）

| 值 | 语义 |
| --- | --- |
| `builtin`（缺省） | 弱（权重 1）+ 强（权重 2）关键词都算命中 |
| `strict` | **只算强关键词**（权重 ≥ 2）——保守档，误报更少 |

### 3.4 TTS **建议**参数（`suggested_tts`）

`speed` / `pitch`，缺省 1.0，都钳在 `0.5..=1.5`，保留两位小数（数值稳定可断言）。

| 情绪 | speed 增量 | pitch 增量 |
| --- | --- | --- |
| neutral | 0 | 0 |
| happy | +0.08 | +0.10 |
| sad | −0.10 | −0.08 |
| angry | +0.12 | +0.06 |
| surprised | +0.05 | +0.15 |
| anxious | +0.06 | +0.04 |
| affectionate | −0.05 | +0.08 |

**强调只抬音高**：`!` / `！` 的个数（**封顶 2**）× 0.05 加到 pitch，**不改 speed**。
例：`开心` → pitch 1.10；`开心！` → 1.15；`开心！！！！` → 1.20。

### 3.5 长度闸门

`clean_text` 先 `trim`，再按**字符**（不是字节）截断到 2000 字符；
清洗后为空 → `Decision::silent()`（`Neutral` + `Silence` + 1.0/1.0），
账本**不记决策**、只 `silent += 1`。

> **该建议参数不投递给任何人**：不进 `[tts]`、不进请求体、不进 WS 帧。
> 它只出现在日志与 `state_json` 里（为什么不做 apply-to-TTS 见 §8）。

## 4. 状态面（`ModRuntime::state_json`）

```json
{
  "delivered": false,
  "channel": "none",
  "turns_seen": 2,
  "turns_ended": 2,
  "decisions": 2,
  "silent": 0,
  "errors": 0,
  "log_capacity": 20,
  "emotion_lexicon": "builtin",
  "latest": {
    "seq": 2, "turn": "2",
    "emotion": "happy", "intent": "greeting",
    "suggested_tts": { "speed": 1.08, "pitch": 1.20 },
    "closed": true, "delivered": false
  },
  "recent_decisions": [
    { "seq": 1, "turn": "1", "emotion": "sad", "intent": "complaint",
      "suggested_tts": { "speed": 0.90, "pitch": 0.92 },
      "closed": true, "delivered": false },
    { "seq": 2, "turn": "2", "emotion": "happy", "intent": "greeting",
      "suggested_tts": { "speed": 1.08, "pitch": 1.20 },
      "closed": true, "delivered": false }
  ]
}
```

| 字段 | 语义 |
| --- | --- |
| `delivered` | **恒 `false`**：本 Mod 从不投递 |
| `channel` | **恒 `"none"`**：没有下行通道（与 RFC §3.2 的槽位 `channel:"none"` 同口径） |
| `turns_seen` | 见过的 `TurnPrompt` 数（含静默轮） |
| `turns_ended` | 正常结项的 `TurnEnded` 数 |
| `decisions` | 产生的决策数（不含静默轮、**不受容量影响**） |
| `silent` | 静默轮数（正文清洗后为空） |
| `errors` | 无在飞轮的 `TurnEnded` 数（乱序 / 重复） |
| `log_capacity` | 最近决策条数上限（1..=200，已钳位） |
| `emotion_lexicon` | 当前词表档位 |
| **`latest`** | **最近一条决策**（= `recent_decisions` 的最后一条，**同一个 `LedgerEntry::to_json` 形状**）；空账本 → `null`。**产品级加强波次新增**，前端不必再从数组尾部自己取 |
| `recent_decisions` | 最近 `log_capacity` 条（旧 → 新）；单条键：`seq/turn/emotion/intent/suggested_tts/closed/delivered`。**形状一行未改**（向后兼容，回归 `recent_decisions_shape_is_unchanged_with_latest_added`） |

- **只读、不写盘、不阻塞**：只读内存字段，无 IO、无锁等待、无网络
  （契约见 `ModRuntime::state_json` 头注）。经 host 暴露为
  `GET /api/v1/mods/director/state`（在册但暂不可读 = 503 `state_unavailable`）。
- **每完成一轮对话都应能看到 `decisions` 增长**：`TurnPrompt` 记一条 → `latest` 立刻
  更新（未结项 `closed=false`、`turn=null`）；`TurnEnded` 到达时写上 turn id 并置
  `closed=true`（回归 `state_json_latest_tracks_the_newest_decision`）。
- **不含 `slots` / `emitted`**：RFC §3.2 的槽位词汇**尚未评审**（RFC §9 缺口 5），
  本 Mod **不实现**槽位，也不向状态面塞占位。
- **容量**：`log_capacity` 走 `DecisionLedger::new` 的钳位；超出时丢最旧的，
  计数不丢（回归 `state_json_capacity_keeps_last_n`）。

## 5. 一次性命令（`ModRuntime::command`，产品级加强波次）

通道：`POST /api/v1/mods/director/command`，body `{"command":"clear","args":{}}`
（`args` 可省）。host 回 `200 {"ok":true,"result":{…}}`；其它失败见下表。

| 命令 | 语义 | 返回 |
| --- | --- | --- |
| `clear` | 清空决策账本（条目 + 计数 + 在飞标记），**只动内存** | `{"cleared": <清掉的条目数>, "counts": {清空前的 turns_seen/turns_ended/decisions/silent/errors}}` |
| 其它 | 不认识 | `Err(ModError::UnsupportedCommand)` → host 回 `409 unsupported_command` |

- 回执给的是**清空前的 counts**：用户想知道自己刚清掉了什么（回归
  `clear_command_empties_ledger_and_returns_before_counts`）；
- 清空**不是**把 Mod 关掉：之后照常记账（同一回归覆盖）；
- 命令路径**同样零投递**：`action_tx` / `apply_settings` 零调用（回归
  `command_paths_never_touch_action_or_settings`，见 §7）；
- `clear` 之后 `latest` 回到 `null`、`recent_decisions` 为空——面板随之回到空态指引。

## 6. 一等决策面板（`director_panel.dart`）

面板注册在 `settings/mods/mod_panels.dart`（共享只读），实现只住
`shell/flutter/lib/settings/mods/director_panel.dart`（本轨道独占）。
它渲染在 Mod 卡片的「运行态（只读）」块**之后**，**自己不做网络**——只消费
`ModPanelContext` 的 `state` / `onRefreshState` / `onCommand`。

| 区块 | 内容 |
| --- | --- |
| 顶部说明 | **文字**写清「仅建议：不驱动动作、不影响 TTS 输出（delivered=false、channel=none）。」——不靠颜色、不藏在 tooltip 里 |
| 本轮决策 | 把 `latest` 拆成带标签的行：**本轮情绪 / 意图 / 建议语速 / 建议音高 / 是否已结项 / 轮次**；情绪与意图给「中文（稳定码）」双写 |
| 最近决策 | `recent_decisions`（**旧到新**）逐条一行：`#seq · 轮 turn · 情绪 · 意图 · 语速 · 音高 · 已结项/未结项` |
| 空态 | **未启用** / **已启用但 decisions==0** → 都给「**先启用并聊一轮**」指引；`state` 未取到 → 可处置文案；`stateError` 原样带错误码上屏 |
| 清空 | 「清空决策账本」按钮 → `onCommand("clear")` → 显示「已清空 N 条决策（清空前 decisions=M）」并重取运行态；失败显示带错误码的文案（`command_unavailable` / `unsupported_command` / `not_found`） |

纪律与项目其余 UI 一致：文案走**文字**表意、错误带**错误码**、间距取 `Space` 档、
颜色走 `AppColors` 令牌、中文字符必须在自托管字体子集内
（`test/font_subset_test.dart` / `test/design_tokens_lint_test.dart` 守着）。

回归：`shell/flutter/test/director_panel_test.dart`（一等面板各字段、空态指引、
零投递说明、clear 的调用与失败带码，以及「ModsSection 展开导演卡片 → 面板渲染」
的注册面接线）。

## 7. 与休眠 `action_tx` 的关系（零投递，红线）

- `ModServices::action_tx` **保持休眠**：host 注入的固定 sender 只留一行 debug 日志并
  返回 `false`（`mod_registry.rs`）。
- 本 crate **从不调用** `action_tx`——不是「调用了但被拒」，是**零调用**；`apply_settings`
  同样**零调用**：`action_tx` / `apply_settings` 在本 crate 只出现在文档与测试替身里。
- 回归 `tests::action_tx_and_apply_settings_are_never_called` 用间谍 sender 断言
  `action_calls == 0` **且** `apply_calls == 0`；产品级加强波次**加强**了它——
  新加的**命令通道**（`clear` 成功 / `clear` 带多余 args / 未知命令报错）也走一遍，
  由 `tests::command_paths_never_touch_action_or_settings` 单独再钉一遍。
- 因此「没有任何动作被投递」是**结构性的**：没有代码路径能投递。
- RFC §4.2 的六条唤醒前提**一条都没动**；本轨**不**恢复 `RootEvent::Action` 注入、
  **不**碰 core `action/` / `performance/`、**不**新增 WS 帧、**不**碰 wasm 协议。

## 8. 为什么**不做**「应用到当前 TTS」（本波明确不做）

结论：`suggested_tts` 保持**只建议**。原因是**主链没有 per-request TTS 参数通道**，
而不是「懒得接」：

1. **合成请求体里没有 speed / pitch**。`live2d-ai-runtime/src/tts.rs` 的
   `SpeechRequestBody` 只有 `input` / `model` / `voice` / `response_format`；
   端点与这些字段的唯一权威来源是 `live2d-ai.toml` 的 `[tts]` 段
   （TTS 是核心链路，不是 Mod，见 [tts-is-core.md](tts-is-core.md)）。
2. **`apply_settings` 写的是持久配置**。用它做「这一轮语速」= 改**全局默认**，
   会泄漏到后续所有轮，还要热重载 / 重启才生效——语义是错的（本轮决策变成下一轮的默认）。
3. **要做就先进 core 开一条 per-request TTS 参数通道**（请求级、不落盘、不重启）。
   那属于**主链皮肤**（LLM/TTS/口型/Live2D），本波**冻结**、明令禁止改一行。

因此本面板上写的是「建议语速 / 建议音高」，不是「应用」。RFC §3.1 本来就写着
「当轮改音色做不到」；本波把这条边界**说到前端**，而不是留一个点了没反应的按钮。

## 9. 配置（`settings_spec`，静态）

| key | kind | 语义 | 缺省 / 范围 |
| --- | --- | --- | --- |
| `log_capacity` | Number | 决策日志容量 | 20；钳在 1..=200 |
| `emotion_lexicon` | Select | 情绪词表档位（`builtin` / `strict`） | `builtin` |

- **没有 `enabled`**：启停唯一真源是 Mod manifest 的 `enabled`
  （与 external-input / memory 同口径，`mod-product-chain.md` §4；`pet-desktop` 已 ARCHIVED）；
- 工厂的 `settings_spec()` 与 `start` 注册的是**同一份**（回归
  `settings_spec_has_no_enabled_and_matches_static_spec`）；
- 坏值只回落 / 钳位，绝不失败（回归 `log_capacity_is_clamped_and_bad_values_fall_back`）。

## 10. 边界（写入者 × 字段：director 一列全是「禁止」）

| 字段 / 落点 | director | 说明 |
| --- | --- | --- |
| `persona.system_prompt` | **禁止** | 归 persona / memory（last-writer-wins）；director 不当第三个写者 |
| `persona.max_history_pairs` | **禁止** | — |
| `[tts].base_url` / `api_key_env` | **禁止**（红线） | 端点唯一权威（`tts-is-core.md`） |
| `[tts].voice` / `[tts].model` / speed / pitch | **禁止** | §8：没有 per-request 通道；`apply_settings` 零调用 |
| `[tts].sample_rate` / `channels` / `response_format` | **禁止** | 被 `verify_core_chain.py` 不变量锁死 / 不是表演参数 |
| `[llm].*` | **禁止** | — |
| 壁纸偏好（`DisplayPrefs` / `stagePlaylist`） | **禁止** | 归 Flutter（`DisplayPrefs`）；wallpaper Mod 已 ARCHIVED |
| core 动作 / 表演状态 | **禁止** | §7；动作在产品路径上不存在 |
| 自己的 `mods.json` config | 只读（`log_capacity` / `emotion_lexicon`） | 本轮不写回 |
| 自己的决策账本（内存） | **可写**（`command("clear")`） | 唯一允许的「写」——只动内存，不落盘、不投递 |

## 11. 非目标（明文，防止当成 backlog）

- **不投递任何动作 / TTS 参数**（本 Mod 的**定义**，不是暂缺）；
- **不做「应用到当前 TTS」**（§8：无 per-request 参数通道，且本波主链冻结）；
- 不实现动作库 / 编舞 / 表情编排；不复活 `RootEvent::Action`；
- 不起第二个 LLM 调用、不做 embedding / 向量库 / 云端情绪分类；
- 不新增主链字段 / 事件主题 / WS 帧 / wasm 协议；不引入动态加载；
- 不做「第二个 TTS 端点」；
- 不把面板做成可编辑表单（决策是**只读**的展示面）。

## 12. 晋升门槛（对照 RFC §5.1，产品级加强波次后的进度）

| # | RFC §5.1 条件 | 当前状态 |
| --- | --- | --- |
| 0 | `[tts].voice` 持久化写入**被授权** | ✗ **未授权**——本 Mod 因此选择零写入 |
| 1 | ≥3 条用户可见演示（开 / 关有差异） | ✗ 不满足：本 Mod **故意无差异**（不投递）；但「开 / 关」在面板与 `state` 上**看得见**（一等面板 + `latest`） |
| 2 | 推导纯函数单测 ≥20 条 | ✓ 共 **33 条**回归（纯函数 14 + 账本/状态/命令/零投递） |
| 3 | 失败隔离可证明 | ✓ 坏 `TurnEnded` → `errors` 不 panic；未知命令 → `UnsupportedCommand` 且状态不变；`state_json` / `command` 不阻塞、无网络 |
| 4 | 端点红线可测（patch 键 ⊆ `{tts.voice, tts.model}`） | ✓ **更强**：`apply_calls == 0`（没有 patch，含命令通道） |
| 5 | 动作零接触可测 | ✓ `action_calls == 0`（含命令通道）+ 不 `use` core 动作类型 + `channel:"none"` |
| 6 | 不碰基座独占文件 | ✓ diff 只含本 crate + 本面板 + 本测试 + 本文档 |
| 7 | 全量门禁绿 | 本轨跑定向门禁（见 §13）；全量由主 agent 收束时跑 |
| 8 | 具名维护者 | ✓ REGISTER §5 点名 |
| 9 | `state` 200 且脱敏；错误码 `director_*` | ✓ `state_json` / `command` 无密钥字段；orphan 用 `director_orphan_turn_ended` 前缀；路由由 desktop host 统一回 `unsupported_command` / `command_unavailable` |

**结论**：本 Mod 仍**不是 v1**——第 0/1 条是**产品授权**问题，不是实现问题。在它们被满足前
**不得**接上任何下行通道，也**不得**被描述成「已经能让 TTS 变调」。

## 13. 验证（产品级加强波次实跑）

```text
CARGO_TARGET_DIR=… cargo test -p live2d-ai-mod-director --all-targets   # 33 passed; 0 failed
CARGO_TARGET_DIR=… cargo test -p live2d-ai-desktop mod_count           # 1 passed
CARGO_TARGET_DIR=… cargo test -p live2d-ai-desktop default_mods_manifest  # 1 passed
CARGO_TARGET_DIR=… cargo clippy -p live2d-ai-mod-director -p live2d-ai-desktop --all-targets -- -D warnings  # 0 warning
cargo fmt -p live2d-ai-mod-director -p live2d-ai-desktop               # clean
cd shell/flutter && flutter analyze                                    # No issues found
cd shell/flutter && flutter test                                       # 851 passed（本轨不跑 --workspace）
```

证据面清单（哪条需求由哪条回归守住）：

| 需求 | 回归 |
| --- | --- |
| 纯函数（命中 / 缺省 / 词表优先 / 空输入） | `emotion_keyword_hits_happy` / `strict_lexicon_drops_weak_keywords` / `empty_and_whitespace_are_silent_neutral` / `negation_suppresses_keyword` / `emotion_tie_broken_by_priority` |
| 订阅主题正确 | `start_subscribes_prompt_and_ended_only` |
| `state_json` 形状与容量上限 | `state_json_shape_and_delivered_false` / `state_json_capacity_keeps_last_n` / `suggested_tts_exposes_only_speed_and_pitch` |
| **`latest` 每轮更新、空账本为 null** | `state_json_latest_tracks_the_newest_decision` |
| **`recent_decisions` 向后兼容** | `recent_decisions_shape_is_unchanged_with_latest_added` |
| **`clear` 命令返回清空前 counts** | `clear_command_empties_ledger_and_returns_before_counts` |
| 未知命令 → `unsupported_command` | `unknown_command_is_unsupported_and_leaves_state_untouched` |
| `action_tx` / `apply_settings` 零调用（含命令通道） | `action_tx_and_apply_settings_are_never_called` / `command_paths_never_touch_action_or_settings` |
| 轮末结项（`TurnEnded`） | `turn_ended_closes_the_decision_and_writes_turn_id` / `orphan_turn_ended_counts_error_and_does_not_panic` |
| **一等面板 / 空态 / 零投递说明 / clear 文案** | `shell/flutter/test/director_panel_test.dart` |

## 14. 已知缺口（产品级加强波次收工时）

1. **回复侧证据缺失仍在**：本 Mod 只订阅用户侧 `TurnPrompt`，不订阅 `TextDelta`
   （RFC §2.1/§9 缺口 2 的口径不变）：失败轮可能一句回复都没有。
2. **零投递仍是有意的**：`suggested_tts` 目前**没有消费者**；要做「应用」须先按 §8 开
   per-request 通道并解冻主链。
3. **槽位未实现**：RFC §3.2 的 `slots` / `emitted` 不在状态面里（词汇未评审）。
4. **情绪词表是启发式**：中文子串匹配（`申请` 含 `请` → Request）与固定小词表，
   不是分类器；定位就是「确定性、可单测」。
5. **面板只在有 `settings_spec` 的卡片里渲染**：这是共享的 `ModsSection` 结构
   （无 spec 的旧 Mod 不展开卡片），不是 director 的选择；director 有 spec，不受影响。
6. **活服务验收未做**：本轨只到 `cargo test` + `flutter test` 级别；路由的活服务验证
   留待主 agent 收束波次。
