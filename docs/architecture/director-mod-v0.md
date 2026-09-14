# 导演 Mod v0（最小骨架）：正文 → 情绪/意图/TTS 建议，**不投递**

> **状态**：2026-09-14 **Wave 3 G 轨**（分支 `mod/w3-director`，基线 `118bd435`）。
> 本 crate 是**可启用的最小骨架**，不是 v1：工厂注册（**缺省停用**）由**主 agent 收束时**
> 完成——本轨**不碰** `main.rs` / `mod_count_*` / `default_mods_manifest` / 版本号。
>
> **本波后现状**：director 已注册（缺省停用）；`AVAILABLE_MOD_FACTORIES` 共 **5 个**
> （`external-input` / `persona` / `voice-input` / `memory` / `director`）——`wallpaper` /
> `pet-desktop` 已 **ARCHIVED**（移出注册表、不再编译进 binary），见
> [ARCHIVED-mods.md](ARCHIVED-mods.md)。
>
> 契约来源：[director-rfc.md](director-rfc.md)（Wave 2 草案）。本文档是它的**实现面**，
> 与 `crates/live2d-ai-mod-director/` **逐条一致**；冲突时以代码 + 回归为准并回改本文档。
> 集成清单：[REGISTER-director-v0.md](../plans/parallel-mods/REGISTER-director-v0.md)。

## 0. 一句话

把每轮输入正文经**纯函数**推导成 `{emotion, intent, suggested_tts:{speed,pitch}}`，
写进日志与 `state_json`；**没有任何下行通道**——`action_tx` **从不调用**、
`apply_settings` **从不调用**、`live2d-ai.toml` **从不写**。

## 1. 范围与闭环定义（Wave 3 §3G）

| 项 | 内容 |
| --- | --- |
| **闭环定义** | 启停可用（唯一真源 = manifest `enabled`）→ 订阅两个主题 → `state_json` 可观察最近 N 条决策 → **不投递** |
| **输入** | `TurnPrompt`（本轮正文）+ `TurnEnded`（turn id），**正好两个主题** |
| **输出** | tracing 日志（**不含正文原文**）+ `GET /api/v1/mods/director/state` |
| **缺省** | **停用**（不进 `cli_entry::default_mods_manifest`） |
| **crate** | `crates/live2d-ai-mod-director/`：`lib.rs`（Mod 集成）/ `decision.rs`（纯函数）/ `ledger.rs`（账本）/ `tests.rs`（28 条回归） |

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
   「只对下一轮生效」的时序代价在骨架里**不成立**（骨架根本不写）。

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
> 它只出现在日志与 `state_json` 里。

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
  "recent_decisions": [
    {
      "seq": 1, "turn": "1",
      "emotion": "happy", "intent": "greeting",
      "suggested_tts": { "speed": 1.08, "pitch": 1.2 },
      "closed": true, "delivered": false
    }
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
| `recent_decisions` | 最近 `log_capacity` 条（旧 → 新）；单条键：`seq/turn/emotion/intent/suggested_tts/closed/delivered` |

- **只读、不写盘、不阻塞**：只读内存字段，无 IO、无锁等待、无网络
  （契约见 `ModRuntime::state_json` 头注）。经 host 暴露为
  `GET /api/v1/mods/director/state`（在册但暂不可读 = 503）。
- **不含 `slots` / `emitted`**：RFC §3.2 的槽位词汇**尚未评审**（RFC §9 缺口 5），
  骨架**不实现**槽位，也不向状态面塞占位。
- **容量**：`log_capacity` 走 `DecisionLedger::new` 的钳位；超出时丢最旧的，
  计数不丢（回归 `state_json_capacity_keeps_last_n`）。

## 5. 与休眠 `action_tx` 的关系

- `ModServices::action_tx` **保持休眠**：host 注入的固定 sender 只留一行 debug 日志并
  返回 `false`（`mod_registry.rs`）。
- 本 crate **从不调用** `action_tx`——不是「调用了但被拒」，是**零调用**。回归
  `tests::action_tx_and_apply_settings_are_never_called` 用间谍 sender 断言
  `action_calls == 0` **且** `apply_calls == 0`。
- 因此「没有任何动作被投递」在骨架里是**结构性的**：没有代码路径能投递。
- RFC §4.2 的六条唤醒前提**一条都没动**；本轨**不**恢复 `RootEvent::Action` 注入、
  **不**碰 core `action/` / `performance/`、**不**新增 WS 帧、**不**碰 wasm 协议。

## 6. 配置（`settings_spec`，静态）

| key | kind | 语义 | 缺省 / 范围 |
| --- | --- | --- | --- |
| `log_capacity` | Number | 决策日志容量 | 20；钳在 1..=200 |
| `emotion_lexicon` | Select | 情绪词表档位（`builtin` / `strict`） | `builtin` |

- **没有 `enabled`**：启停唯一真源是 Mod manifest 的 `enabled`
  （与 external-input / memory 同口径，`mod-product-chain.md` §4；`pet-desktop` 已 ARCHIVED）；
- 工厂的 `settings_spec()` 与 `start` 注册的是**同一份**（回归
  `settings_spec_has_no_enabled_and_matches_static_spec`）；
- 坏值只回落 / 钳位，绝不失败（回归 `log_capacity_is_clamped_and_bad_values_fall_back`）。

## 7. 边界（写入者 × 字段：director 一列全是「禁止」）

| 字段 / 落点 | director | 说明 |
| --- | --- | --- |
| `persona.system_prompt` | **禁止** | 归 persona / memory（last-writer-wins）；director 不当第三个写者 |
| `persona.max_history_pairs` | **禁止** | — |
| `[tts].base_url` / `api_key_env` | **禁止**（红线） | 端点唯一权威（`tts-is-core.md`） |
| `[tts].voice` / `[tts].model` | **禁止**（骨架） | RFC 允许「未来仅可提议」，但骨架**不写**——无 `apply_settings` 调用 |
| `[tts].sample_rate` / `channels` / `response_format` | **禁止** | 被 `verify_core_chain.py` 不变量锁死 / 不是表演参数 |
| `[llm].*` | **禁止** | — |
| 壁纸偏好（`DisplayPrefs` / `stagePlaylist`） | **禁止** | 归 Flutter（`DisplayPrefs`）；wallpaper Mod 已 ARCHIVED |
| core 动作 / 表演状态 | **禁止** | §5；动作在产品路径上不存在 |
| 自己的 `mods.json` config | 只读（`log_capacity` / `emotion_lexicon`） | 本轮不写回 |

## 8. 非目标（明文，防止当成 backlog）

- **不投递任何动作 / TTS 参数**（本骨架的**定义**，不是暂缺）；
- 不实现动作库 / 编舞 / 表情编排；不复活 `RootEvent::Action`；
- 不起第二个 LLM 调用、不做 embedding / 向量库 / 云端情绪分类；
- 不新增主链字段 / 事件主题 / WS 帧 / wasm 协议；
- 不做「当轮改音色」「改这一句的音色」（RFC §3.1 明确做不到）；
- 不做导演 UI / 不做「第二个 TTS 端点」。

## 9. 晋升门槛（对照 RFC §5.1，Wave 3 骨架的进度）

| # | RFC §5.1 条件 | 骨架状态 |
| --- | --- | --- |
| 0 | `[tts].voice` 持久化写入**被授权** | ✗ **未授权**——骨架因此选择零写入 |
| 1 | ≥3 条用户可见演示（开 / 关有差异） | ✗ 不满足：骨架**故意无差异**（不投递） |
| 2 | 推导纯函数单测 ≥20 条 | ✓ **28 条**（含纯函数 14 条） |
| 3 | 失败隔离可证明（`on_event` Err / `apply_settings` false / `state_json` 不阻塞） | 部分：坏 `TurnEnded` → `errors` 不 panic；`apply_settings` 不适用（从不调用） |
| 4 | 端点红线可测（patch 键 ⊆ `{tts.voice, tts.model}`） | ✓ **更强**：`apply_calls == 0`（没有 patch） |
| 5 | 动作零接触可测 | ✓ `action_calls == 0` + 不 `use` core 动作类型 + `channel:"none"` |
| 6 | 不碰基座独占文件 | ✓ diff 只含新 crate + 根 `Cargo.toml` members + 文档 |
| 7 | 全量门禁绿 | 本轨只跑定向门禁（见 §10）；全量由主 agent 收束时跑 |
| 8 | 具名维护者 | ✓ REGISTER §5 点名 |
| 9 | `state` 200 且脱敏；错误码 `director_*` | 部分：`state_json` 无密钥字段；orphan 用 `director_orphan_turn_ended` 前缀；路由回归待收束后跑 |

**结论**：骨架**只满足第 2/4/5/6/8 条**（外加第 3 条的一部分）。第 0/1 条是
**产品授权**问题，不是实现问题——在它们被满足前，本 Mod 保持「最小骨架 / 缺省停用」，
**不得**被描述成 v1，**不得**接上第 7 条以外的任何下行通道。

## 10. 验证（Wave 3 G 轨实跑）

```text
cargo test -p live2d-ai-mod-director                                  # 28 passed; 0 failed
cargo fmt -p live2d-ai-mod-director -- --check                        # clean
cargo clippy -p live2d-ai-mod-director --all-targets -- -D warnings   # 0 warning
```

（全量 `cargo test --workspace --all-targets` / doc / rust-ratio 由主 agent 收束时跑；
见 WG3 协议 §2 的门禁分工。）

证据面清单（哪条需求由哪条回归守住）：

| 需求（Wave 3 §3G） | 回归 |
| --- | --- |
| 纯函数（命中 / 缺省 / 词表优先 / 空输入） | `emotion_keyword_hits_happy` / `strict_lexicon_drops_weak_keywords` / `empty_and_whitespace_are_silent_neutral` / `negation_suppresses_keyword` / `emotion_tie_broken_by_priority` |
| 订阅主题正确 | `start_subscribes_prompt_and_ended_only` |
| `state_json` 形状与容量上限 | `state_json_shape_and_delivered_false` / `state_json_capacity_keeps_last_n` / `suggested_tts_exposes_only_speed_and_pitch` |
| `action_tx` 未被调用 | `action_tx_and_apply_settings_are_never_called` |
| 轮末结项（`TurnEnded`） | `turn_ended_closes_the_decision_and_writes_turn_id` / `orphan_turn_ended_counts_error_and_does_not_panic` |

## 11. 已知缺口（Wave 3 G 轨收工时）

1. **已注册（本波后）**：director 已在 `AVAILABLE_MOD_FACTORIES` 中（缺省停用），
   `GET /api/v1/mods/director/state` 可读；计数断言 `mod_count_is_five` 守住当前 5 个
   （原「未注册 / 503」是 Wave 3 G 轨收工时的状态，REGISTER §2 已由主 agent 收束）。
2. **零投递是有意的**：`suggested_tts` 目前**没有消费者**。要让它生效必须先过
   RFC §5.1 第 0/1 条（授权 + 用户可见演示）；本骨架刻意不提供通道。
3. **槽位未实现**：RFC §3.2 的 `slots` / `emitted` 不在状态面里（词汇未评审）。
4. **回复侧证据缺失仍在**：本 Mod 只订阅用户侧 `TurnPrompt`，不订阅 `TextDelta`
   （RFC §2.1/§9 缺口 2 的口径不变）：失败轮可能一句回复都没有。
5. **情绪词表是启发式**：中文子串匹配（`申请` 含 `请` → Request）与固定小词表，
   不是分类器；本骨架的定位就是「确定性、可单测」。
6. **未做活服务验收**：本轨只到 `cargo test` 级别；路由的活服务验证留待注册后的
   主 agent 收束波次补（本波起已在 `AVAILABLE_MOD_FACTORIES` 中）。
