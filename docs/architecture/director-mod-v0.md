# 导演 Mod v0（已注册第 5 个）：按情绪/意图选动作包 → `latest.preset_id`（状态面）+ 按句 `action_cue`（驱动舞台）

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
>
> **产品口径（2026-09-21）**：产品 = 「**酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子**」，
> 各功能由现有 mod 矩阵承担（导演也在矩阵里）；**导演是一个 AI**，不是给用户操控皮套的辅助。
> 本 Mod 的**实际职责**：按情绪 / 意图选动作包 → 产出
> **`latest.preset_id`**（状态面，供面板展示）与**按句 `action_cue`**（驱动舞台）。
>
> **2026-09-22 并存关系**：主链 `[performance]`（见 [performance-layer-v0.md](performance-layer-v0.md)）
> 与本 Mod `staging_*` 是**两个可选提供者，都默认关、职责有重叠**——
> 前者产出 `speak`（本轮 TTS / 上屏）与 `cues`，后者只产出按句 cue。
> **谁的 `speak` 能力该保留未定**（[RESEARCH-actions-director-audit-2026-09-21.md](../plans/RESEARCH-actions-director-audit-2026-09-21.md) §3.7 Q1），
> 本文档**不裁决**。
>
> **当前实现事实（同轮只有一个 cue 产者）**：`[performance]` 开着时 host **不**把
> `SentenceReady` 转给本 Mod（`forward_sentence_ready_to_mods = !engine.performance_enabled()`），
> 避免两份 `action_cue` 在前端互相整份覆盖。规则层的单一真源仍是本 crate 的
> `presets::rule_cues_for_text`（本地纯函数，常驻）。
>
> | 提供者 | 当前产出 | 默认 |
> | --- | --- | --- |
> | **主链 `[performance]`**（runtime 主链） | `speak`（本轮 TTS / 上屏真源）+ `cues` → `action_cue` | `enabled=false` |
> | **本 Mod 规则层 + `staging_*`**（Mod） | `latest.preset_id`（状态面，供面板展示）+ 按句 `action_cue` → 驱动舞台 | 规则层常驻；`staging_enabled=false` |
>
> 界面侧一致：LLM 分区写「主模型不负责表演；表演层每轮 JSON」+ 三态说明（只说 / 只动 / noop）；
> 本 Mod 面板对两者关系另有一句兼容说明。

## 0. 一句话：按情绪/意图选动作包 → 状态面 `latest.preset_id` + 按句 `action_cue`

每轮输入正文经**纯函数**推导成 `{emotion, intent, suggested_tts:{speed,pitch}}`，
再经一张**映射表**选出一条**动作预设** `preset_id`（表情 / 短动作），写进日志与
`state_json.latest.preset_id`（**状态面，供面板展示**）。`action_tx` **零调用**、
`apply_settings` **零调用**、`live2d-ai.toml` **零写入**——这三点成立；
**但下行确实存在**：规则层 / 二路按 `SentenceReady` 锚出的 cue 经
`ModServices.cues`（ModCueSender）→ host 广播 WS **`action_cue`** 帧 →
前端在该句音频开始播放时 `applyPreset` **驱动舞台**。另一条通道是**只读状态面**
（`GET /api/v1/mods/director/state`）+ 前端拉取 `latest.preset_id`，供面板展示
（单一驱动者收口见 W9）。

> **产品口径（2026-09-21）**：产品 = 「酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子」，
> 各功能由现有 mod 矩阵承担（导演也在矩阵里）；**导演是一个 AI**，不是给用户操控皮套的辅助。
> 范围仍然严格：**只做** Expression 与短 motion 两条通道，不做手臂 / 手指 / 特效；
> 第二路 LLM 见第 0.1 节（默认关）。变更有专门回归（`presets` 模块 6 条 /
> `state_json_shape_and_preset_delivery`），且 `action_tx` / `apply_settings` 的
> **零调用**断言一条没放松。

产品级加强波次把「看得到」这条做实：`state_json` 新增 **`latest`**（最近一条决策，
前端直接消费，结构仍与 `recent_decisions` 的单条**逐字同形**），Flutter 的
`DirectorPanel` 把 `latest` 与最近 N 条渲染成**一等面板**（带标签的行 + 旧到新列表），
并提供 `command("clear")` 清空账本。

## 0.1 P1-3（2026-09-16）：SentenceReady + 异步第二路 LLM（**默认关**）

> 这是对本文档旧口径的**又一次明确反转**，必须与代码同步。

- **新事件**：宿主新增 EngineEvent::SentenceReady（发在该句 push_job **之前**）
  → ModEventTopic::SentenceReady（payload = JSON {epoch, ts_ms, sentence_seq, text}）。
  本 Mod 多订阅这一个主题；投递是**非阻塞**的（主链绝不等待导演）。
- **规则层常开兜底**（priority 10）：每轮 TurnPrompt 的规则决策选出的 preset_id
  在第一句锚成一条 cue。规则仍是本地纯函数（无网络 / 无时钟 / 无随机）。
- **异步第二路 LLM**（priority 40，**默认关**）：节流 = 首句触发 + 间隔 >= 1200ms +
  每轮 <= 3；失败 / 超时 / JSON 坏一律静默回退规则。输出契约是 plan
  {epoch, covers_upto_seq, cues:[{sentence_seq, preset_id, intensity, ttl_ms}]}；
  priority 由应用层写死，LLM 不得自报。
- **内联标签**（priority 80）**只预留解析器、默认关**。
- **下行**：产出经新的 host 能力 ModServices.cues（ModCueSender）→ host 广播
  WS **action_cue** 帧（缺省忽略 = 兼容）；前端按 sentence_seq 在该句音频
  **开始播放**时 applyPreset（渲染面按用户幅度倍率乘）。
- **已接线**（P1-4，2026-09-19）：真实 OpenAI 兼容 HTTP 客户端
  `staging_http::OpenAiStagingClient`（**非流式** `POST {base_url}/chat/completions`），
  由 `DirectorFactory::create` 在 host 装配时按 namespaced config 构造并注入。
  **要真开二路必须配齐** `staging_enabled=true` + `staging_base_url` +
  `staging_model`（密钥变量名 `staging_api_key_env` 可空=不鉴权；见 §9.1）。
  缺端点 / 关闸 → 仍 `DisabledStaging`，行为与「默认关」**逐字一致**；开闸没配齐
  在 `state_json.staging.degraded=true` + `note` 里如实标出（仅规则）。
  失败 / 超时 / 非 2xx / 坏 JSON → **静默回退规则**（`async_failures` 计数）。
  action_tx / apply_settings 的**零调用红线一条没放松**
  （回归 `director_never_writes_config_or_actions`）。
- **思考不外发**：请求体只有 system + user 两条消息，**不带** `reasoning_content`；
  响应侧只取 `choices[0].message.content`，思考字段忽略
  （回归 `staging_http::tests::openai_client_posts_and_parses_content` 直接抓原始请求文本）。
- 离线回归：① 导演 100% 失败 == 无导演
  （`staging_failure_is_identical_to_no_staging`）；
  ② **Disabled ≡ 未注入**（`disabled_staging_is_identical_to_no_injection`）；
  ③ epoch 不匹配零副作用
  （`arbiter_priority_and_epoch_mismatch_zero_side_effect` /
  `staging_epoch_mismatch_is_rule_only_and_leaves_arbiter_clean`）；
  ④ mock 成功 plan → cue（priority 40 覆盖规则 10，
  `staging_async_plan_reaches_cue_sink_with_async_priority`）；
  ⑤ 送 TTS 的文本不含任何导演产物（引擎在 push_job 前发锚点、导演只读不改）。
- **活服务实测抓到的缺陷（2026-09-19，已修 + 已回归）**：core 的 epoch **只在 stop
  时推进**（`live2d-ai-core/src/reducer.rs` 的 `stop()`），所以**普通轮次的
  SentenceReady epoch 恒为 0**（活服务日志实测）。旧守卫「`epoch == 0` 就丢」于是
  让规则层与二路在**所有普通轮次**都不工作——表现为「接线完成、什么也不发生」。
  两处修法：
  1. `handle_sentence_ready` **不再拒绝 epoch 0**（epoch 是不透明代号，0 合法）；
  2. **换轮的信号是 `TurnPrompt` 而不是 epoch 变化**——`TurnPrompt` 到达时
     `arbiter.reset(current_epoch)`，否则上一轮的 cue 会残留（同优先级 `upsert`
     保留先到的 → 旧 cue 赢）。
  回归：`epoch_zero_is_accepted_and_turn_prompt_resets_the_arbiter`。
- **真端点已冒烟（2026-09-19 活服务实测）**：staging 指向 `https://api.deepseek.com/v1`
  + `deepseek-flash`，`POST /api/v1/external/chat` 发一句后
  `state_json.staging = {client:"openai", enabled:true, async_plans:1, async_failures:0}`，
  `plan.cues[0] = {preset_id:"smile", priority:40, ttl_ms:1800}`——
  **异步 plan 真的覆盖了规则 cue（`nod`，priority 10）**。
  日志锚点：`director 二路 LLM 已接线（...）` 与每句一条
  `director action_cue：epoch=..., covers_upto_seq=..., cues=...`。

## 1. 范围与闭环定义（产品级加强波次）

| 项 | 内容 |
| --- | --- |
| **闭环定义** | 启停可用（唯一真源 = manifest `enabled`）→ 订阅两个主题 → `state_json` 可观察 `latest` + 最近 N 条 → **一等面板可读** → `clear` 可清空 → **产出一条动作预设**（`latest.preset_id`，状态面供面板展示）+ **按句 `action_cue`（`ModServices.cues` → WS `action_cue`，驱动舞台）**；`action_tx` / `apply_settings` 仍零调用 |
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
   「只对下一轮生效」的时序代价在这里**不成立**（本 Mod 不写配置；预设走状态面，
   前端在**本轮**回复开始时拉取——见 §0 的 2026-09-15 变更）。

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
> **2026-09-15 例外**：`preset_id`（动作预设）走 `state_json.latest.preset_id` +
> 前端拉取交给渲染面；`suggested_tts` 仍然**不投递**。

## 4. 状态面（`ModRuntime::state_json`）

```json
{
  "delivered": true,
  "channel": "preset",
  "presets_chosen": 2,
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
    "preset_id": "smile",
    "closed": true, "delivered": true
  },
  "recent_decisions": [
    { "seq": 1, "turn": "1", "emotion": "sad", "intent": "complaint",
      "suggested_tts": { "speed": 0.90, "pitch": 0.92 },
      "preset_id": "unhappy",
      "closed": true, "delivered": true },
    { "seq": 2, "turn": "2", "emotion": "happy", "intent": "greeting",
      "suggested_tts": { "speed": 1.08, "pitch": 1.20 },
      "preset_id": "smile",
      "closed": true, "delivered": true }
  ]
}
```

| 字段 | 语义 |
| --- | --- |
| `delivered` | 最近一轮**是否选出了一条预设**（`latest.preset_id != null`）；空账本 → `false`。**不是** host 通道的投递回执 |
| `channel` | 恒 `"preset"`：状态面这条通道是「只读状态面 + 前端拉取」；另有 host 广播 WS `action_cue` 的按句 cue 通道（`ModServices.cues`，不是 host 回调 `action_tx`） |
| `turns_seen` | 见过的 `TurnPrompt` 数（含静默轮） |
| `turns_ended` | 正常结项的 `TurnEnded` 数 |
| `decisions` | 产生的决策数（不含静默轮、**不受容量影响**） |
| `silent` | 静默轮数（正文清洗后为空） |
| `errors` | 无在飞轮的 `TurnEnded` 数（乱序 / 重复） |
| `presets_chosen` | 累计「选出了一条预设」的决策数（**不是**投递成功数） |
| `presets` | 当前生效的映射表（面板据此显示「现在映射到哪条」） |
| `log_capacity` | 最近决策条数上限（1..=200，已钳位） |
| `emotion_lexicon` | 当前词表档位 |
| **`latest`** | **最近一条决策**（= `recent_decisions` 的最后一条，**同一个 `LedgerEntry::to_json` 形状**）；空账本 → `null`。**产品级加强波次新增**，前端不必再从数组尾部自己取 |
| `recent_decisions` | 最近 `log_capacity` 条（旧 → 新）；单条键：`seq/turn/emotion/intent/suggested_tts/preset_id/closed/delivered`（2026-09-15 只新增 `preset_id`，其余一行未改） |
| **`staging`** | P1-4 新增：异步二路的可观察面——`enabled/client/degraded/note/base_url/model/api_key_env/api_key_set/timeout_ms/min_interval_ms/max_per_turn/fires_this_turn/async_plans/async_failures`；`degraded=true` = 开了闸但没配端点（仅规则）。密钥**只回 `api_key_set` 布尔**。详见 §9.1 |
| **`plan`** | P1-3 新增：当前仲裁表——`epoch/covers_upto_seq/cues/cues_emitted/rule_cues/sentences_seen/cue_sink_enabled` |

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
- 命令路径**同样不触碰下行通道**：`action_tx` / `apply_settings` 零调用（回归
  `command_paths_never_touch_action_or_settings`，见 §7）；
- `clear` 之后 `latest` 回到 `null`、`recent_decisions` 为空——面板随之回到空态指引。

## 6. 一等决策面板（`director_panel.dart`）

面板注册在 `settings/mods/mod_panels.dart`（共享只读），实现只住
`shell/flutter/lib/settings/mods/director_panel.dart`（本轨道独占）。
它渲染在 Mod 卡片的「运行态（只读）」块**之后**，**自己不做网络**——只消费
`ModPanelContext` 的 `state` / `onRefreshState` / `onCommand`。

| 区块 | 内容 |
| --- | --- |
| 顶部说明 | **文字**写清启用后的效果（按情绪触发表情 / 短动作；不改 TTS 输出、不碰手臂/特效）与和表演层的并存关系——不靠颜色、不藏在 tooltip 里 |
| 本轮决策 | 把 `latest` 拆成带标签的行：**本轮情绪 / 意图 / 建议语速 / 建议音高 / 是否已结项 / 轮次**；情绪与意图给「中文（稳定码）」双写 |
| 最近决策 | `recent_decisions`（**旧到新**）逐条一行：`#seq · 轮 turn · 情绪 · 意图 · 语速 · 音高 · 已结项/未结项` |
| 空态 | **未启用** / **已启用但 decisions==0** → 都给「**先启用并聊一轮**」指引；`state` 未取到 → 可处置文案；`stateError` 原样带错误码上屏 |
| 清空 | 「清空决策账本」按钮 → `onCommand("clear")` → 显示「已清空 N 条决策（清空前 decisions=M）」并重取运行态；失败显示带错误码的文案（`command_unavailable` / `unsupported_command` / `not_found`） |

纪律与项目其余 UI 一致：文案走**文字**表意、错误带**错误码**、间距取 `Space` 档、
颜色走 `AppColors` 令牌、中文字符必须在自托管字体子集内
（`test/font_subset_test.dart` / `test/design_tokens_lint_test.dart` 守着）。

回归：`shell/flutter/test/director_panel_test.dart`（一等面板各字段、空态指引、
并存说明、clear 的调用与失败带码，以及「ModsSection 展开导演卡片 → 面板渲染」
的注册面接线）。

## 7. 与休眠 `action_tx` 的关系（**host 通道零调用**，红线未变）

- `ModServices::action_tx` **保持休眠**：host 注入的固定 sender 只留一行 debug 日志并
  返回 `false`（`mod_registry.rs`）。
- 本 crate **从不调用** `action_tx`——不是「调用了但被拒」，是**零调用**；`apply_settings`
  同样**零调用**：`action_tx` / `apply_settings` 在本 crate 只出现在文档与测试替身里。
- 回归 `tests::action_tx_and_apply_settings_are_never_called` 用间谍 sender 断言
  `action_calls == 0` **且** `apply_calls == 0`；产品级加强波次**加强**了它——
  新加的**命令通道**（`clear` 成功 / `clear` 带多余 args / 未知命令报错）也走一遍，
  由 `tests::command_paths_never_touch_action_or_settings` 单独再钉一遍。
- 因此「**没有任何动作经 `action_tx` 投递**」是**结构性的**：没有代码路径能经它投递；
  下行只走 `ModServices.cues` → WS `action_cue`（§0.1）。
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

## 9. 配置（`settings_spec` v2，静态）

**2026-09-15 用户裁决：可操控项过多，8 个预设 Select 收成 3 个常用档。**

| key | kind | 语义 | 缺省 |
| --- | --- | --- | --- |
| `preset_happy` | Select | 开心 / 亲昵 → 动作预设 | `smile`（= 内置映射表缺省，**不是**第一个选项 `none`） |
| `preset_sad` | Select | 难过 → 动作预设 | `unhappy` |
| `preset_greeting` | Select | 打招呼 → 动作预设 | `nod` |

- 其余 5 档（生气 / 惊讶 / 焦虑 / 告别 / 亲昵的独立键）与 `log_capacity`、
  `emotion_lexicon` **退出表单**，仍按缺省生效；配置里显式写了仍然被读取
  （`PresetTable::from_value` / `DirectorConfig::from_value`）。
- 每个 Select 都带 **`default`**：前端表单据此回填、保存时按它写入——
  否则「点一次保存」会把缺省映射写成 `none`，表情/短动作**再也不会触发**。
  回归 `settings_spec_has_no_enabled_and_matches_static_spec`（断言缺省 = 内置映射表）。
- **没有 `enabled`**：启停唯一真源是 Mod manifest 的 `enabled`
  （与 external-input / memory 同口径，`mod-product-chain.md` §4；`pet-desktop` 已 ARCHIVED）；
- 工厂的 `settings_spec()` 与 `start` 注册的是**同一份**；
- 坏值只回落 / 钳位，绝不失败（回归 `log_capacity_is_clamped_and_bad_values_fall_back`）。
- **预设 id 集合的单一真源 = `presets::PRESET_IDS`**（**2026-09-23 v3 精简到 10 个**：
  `none` + 3 表情包（`smile` / `unhappy` / `surprised`）+ 6 手势包（`nod` / `shake` /
  `look_left` / `look_right` / `tilt_left` / `tilt_right`）。这一个常量同时决定三件事，
  **不得在别处再抄一份**）：① 规则层 allowlist（`PresetTable::from_value` 未知 id
  ——含已删除的旧 id——回落缺省）；② 面板 Select 选项（`preset_field`，
  **只列主 allowlist**）；③ **二路 LLM 的能力集**（`staging::build_user_prompt` 用它拼
  user 提示；`plan::parse_plan(raw, &PRESET_IDS)` 做第二道闸：不在能力集 → 丢该条）。

  v2 的 18 条已**合并**（sad+angry → `unhappy` 按 intensity morph 1..3；happy 并
  bounce、surprised 并 recoil、nod 并强弱档、shake 并否认档……），旧 id 已在
  2026-09-23 **整体删除**（配置回落缺省、表演层校验拒绝）。合并语义、intensity
  语义、与 N.E.K.O 五情对照见 `docs/architecture/action-packs-v0.md`。

  实际参数写在外置 `assets/actions/presets.json`（内建 `PRESETS` 只是 fallback，
  与主 allowlist **同集合**；JSON 加载失败时未知 id 静默 `Ignore`）。
  幅值 / 波形红线见 `crates/l2d-wasm-demo/src/preset/`（`table.rs` 校验 + `mod.rs` 头注）：
  表情包头 ≤12 / 身 ≤4，手势包头 ≤30 / 身 ≤10，`oscillate` **只用于左右类**。

### 9.1 二路 LLM 要哪些键才能**真开**（P1-4，2026-09-19）

规则层**永远常开**；二路是叠加，默认关。**只打开 `staging_enabled` 不够**——
必须同时给出端点与模型，否则仍然只走规则（面板一句话：「未配端点则仅规则」）。

| key | kind | 语义 | 缺省 | 真开二路是否必填 |
| --- | --- | --- | --- | --- |
| `staging_enabled` | Bool | 二路总闸 | `false` | **是**（必须 true） |
| `staging_base_url` | String | OpenAI 兼容端点（独立于 `[llm]`） | 空 | **是** |
| `staging_model` | String | 二路模型名 | 空 | **是** |
| `staging_api_key_env` | String | 密钥的**变量名**（值只住 `.env`） | 空 = 不鉴权 | 否（端点免鉴权可不填） |
| `staging_timeout_ms` | Number | 单次调用超时（100..=5000） | `1500` | 否 |
| `staging_min_interval_ms` | （不在表单） | 两次异步触发最小间隔 | `1200` | 否 |
| `staging_max_per_turn` | （不在表单） | 每轮异步触发上限 | `3` | 否 |

`mods.json`（director 的 namespaced config；**启停唯一真源仍是 manifest 的
`enabled`**，与 `staging_enabled` 是两件事）：

```json
{
  "mods": {
    "director": {
      "enabled": true,
      "config": {
        "staging_enabled": true,
        "staging_base_url": "http://127.0.0.1:11434/v1",
        "staging_model": "qwen2.5:7b",
        "staging_api_key_env": "DEEPSEEK_API_KEY",
        "staging_timeout_ms": 1500
      }
    }
  }
}
```

密钥值写 `.env`（唯一真源，查找优先级 `.env` 快照 > 进程环境）：

```dotenv
DEEPSEEK_API_KEY=sk-...
```

开成什么样、有没有真的接上，看
`GET /api/v1/mods/director/state` 的 `staging` 块：

| 字段 | 含义 |
| --- | --- |
| `enabled` | `staging_enabled && client 可用`——**这一位才是「会发 HTTP」** |
| `client` | `disabled` / `openai` / `injected`（测试替身） |
| `degraded` | 开了闸但没接上（缺端点/缺模型/URL 非法）→ 仍仅规则 |
| `note` | degraded 的原因（不含密钥）；正常接线时为 `null` |
| `api_key_set` | 是否从 `.env`/环境解析出密钥（**只回布尔，永不回值**） |
| `async_plans` / `async_failures` | 成功 plan 数 / 失败（含超时、坏 JSON、非 2xx）数 |

行为红线（回归见 §13）：失败 / 超时 / 坏 JSON **静默回退规则**，`state_json` 上
只看得到计数变化；送 TTS 的文本**一个字符都不改**。

## 10. 边界（写入者 × 字段：director 一列全是「禁止」）

| 字段 / 落点 | director | 说明 |
| --- | --- | --- |
| `persona.system_prompt` | **禁止** | 归 persona / memory（last-writer-wins）；director 不当第三个写者 |
| 动作预设（`preset_id`） | **允许**（2026-09-15）：写入自己的 `state_json.latest.preset_id`，由前端拉取后投给渲染面 |
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

- **不投递任何 TTS 参数**（`suggested_tts` 只进日志 / 状态面）；
  **动作预设**（`preset_id`）是 2026-09-15 起允许的唯一可执行产出，范围严格限定
  在表情 / 短动作；除「只读状态面 + 前端拉取」外，**按句 `action_cue` 经
  `ModServices.cues` → WS `action_cue` 下行驱动舞台**（不是 `action_tx`）；
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
| 1 | ≥3 条用户可见演示（开 / 关有差异） | ✓（2026-09-15 起）：启用后按情绪触发表情 / 短动作（`preset_id` 经前端投给渲染面），停用即无动作；面板与 `state` 同时可见（`latest.preset_id`） |
| 2 | 推导纯函数单测 ≥20 条 | ✓ 共 **33 条**回归（纯函数 14 + 账本/状态/命令/零调用） |
| 3 | 失败隔离可证明 | ✓ 坏 `TurnEnded` → `errors` 不 panic；未知命令 → `UnsupportedCommand` 且状态不变；`state_json` / `command` 不阻塞、无网络 |
| 4 | 端点红线可测（patch 键 ⊆ `{tts.voice, tts.model}`） | ✓ **更强**：`apply_calls == 0`（没有 patch，含命令通道） |
| 5 | 动作零接触可测 | ✓ `action_calls == 0`（含命令通道）+ 不 `use` core 动作类型 + `channel:"preset"` |
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
| **一等面板 / 空态 / 并存说明 / clear 文案** | `shell/flutter/test/director_panel_test.dart` |

## 14. 已知缺口（产品级加强波次收工时）

> **2026-09-19（P1-4）已闭合**：真实 HTTP 客户端不再是缺口——见 §0.1 / §9.1。
> 仍公开的两条新缺口：① **真端点未冒烟**（本波环境无可用二路端点，接线由
> loopback mock HTTP 回归证明）；② `complete` 是同步调用，会占用 Mod worker
> 至多一个 `staging_timeout_ms`（supervisor 侧投递非阻塞，主链不等；但同 worker
> 上排队的其它 Mod 事件会被推迟——P1-3 同步契约的既有取舍）。

1. **回复侧证据缺失仍在**：本 Mod 只订阅用户侧 `TurnPrompt`，不订阅 `TextDelta`
   （RFC §2.1/§9 缺口 2 的口径不变）：失败轮可能一句回复都没有。
2. **`suggested_tts` 不投递仍是有意的**：它目前**没有消费者**；要做「应用」须先按 §8 开
   per-request 通道并解冻主链。
3. **槽位未实现**：RFC §3.2 的 `slots` / `emitted` 不在状态面里（词汇未评审）。
4. **情绪词表是启发式**：中文子串匹配（`申请` 含 `请` → Request）与固定小词表，
   不是分类器；定位就是「确定性、可单测」。
5. **面板只在有 `settings_spec` 的卡片里渲染**：这是共享的 `ModsSection` 结构
   （无 spec 的旧 Mod 不展开卡片），不是 director 的选择；director 有 spec，不受影响。
6. **活服务验收未做**：本轨只到 `cargo test` + `flutter test` 级别；路由的活服务验证
   留待主 agent 收束波次。
