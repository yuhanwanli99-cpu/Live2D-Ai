# 导演（director）RFC：文本 → 情绪/意图 → TTS 参数 + 动作槽位

> **状态**：2026-09-14 **草案 / 未实现 / 未注册**。Wave 2 **D 轨**（分支
> `mod/director-rfc`，基座 `429609f2`）**唯一交付物就是本文档**。
> 范围真源：[`../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md)
> §3D；上层协议：[`../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md`](../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md)。
> 相关契约：[`core-chain-baseline.md`](core-chain-baseline.md) §3（动作在产品路径上不存在）、
> [`tts-is-core.md`](tts-is-core.md)（TTS 是核心链路，`[tts]` 是端点唯一权威）、
> [`mod-product-chain.md`](mod-product-chain.md) §5/§6/§7。
>
> **本文档不产生任何代码**：不新建 crate、不注册 `AVAILABLE_MOD_FACTORIES`、
> 不改任何 `.rs`、不动 `mod_count_*` / 缺省 manifest / 版本号。
> 它定义的是「将来若要做导演，契约长什么样、边界在哪、什么条件下才准晋升」。

## 0. 本轮决定摘要（先读这一节）

| 决定 | 内容 | 锚点 |
| --- | --- | --- |
| 只交文档 | 无 `crates/live2d-ai-mod-director/`（该 crate 已于 `0.1.0-rc.2` 删除，归档分支 `archive/action-layer-p6`） | [`directory.md`](directory.md)、[`core-chain-baseline.md`](core-chain-baseline.md) §3.2 |
| **不注册** | 不进 `AVAILABLE_MOD_FACTORIES`、不新增 id、不碰 `mod_count_is_five` / `mod_factory_ids_match_expected` | §8；Wave 2 §3D |
| 动作只是**槽位占位** | 槽位是本文档里的**命名契约**，不是通道、不是 crate、不是注册表项 | §3.2、[`core-chain-baseline.md`](core-chain-baseline.md) §3.1 |
| `action_tx` 保持休眠 | 不复活、不接线、不实现动作库 | §4 |
| TTS 端点唯一权威 | `director` **不得**写 `[tts].base_url` / `api_key_env`；未来最多只能**提议** `voice` / `model` | §3.1；[`tts-is-core.md`](tts-is-core.md) §2 |
| 情绪/意图只能 Mod 侧推导 | 主链**不新增**任何情绪字段/事件/第二 LLM 调用 | §2.2、§2.3 |

## 1. 状态机草图

导演是**一轮一次、只读主链、只对下一轮生效**的旁路决策器。它不参与主链仲裁，
不改变文本上屏、不改变音频路径，也**没有**任何能在当轮改变发声的通道（原因见 §3.1）。

```text
                 ┌──────────────────────────────────────────────┐
                 │ Idle（无输入 / 静默）                          │
                 │ 无在飞 turn：不推导、不写配置、不发槽位          │
                 └───────────────┬──────────────────────────────┘
                                 │ TurnStarted{turn_id}
                                 │ TurnPrompt{本轮输入正文}
                                 ▼
                 ┌──────────────────────────────────────────────┐
                 │ Observing（观察）                              │
                 │ 逐句收 TextDelta（已是「同拍正文」，见 §2.1）    │
                 │ 累积本轮情绪 / 意图证据；不发任何外部副作用      │
                 └───────┬───────────────────────┬──────────────┘
        VoiceEnded（可选节拍）│                       │ 下一轮 TurnPrompt（权威节拍）
                             │                       │
                             ▼                       ▼
              ┌──────────────────────────────────────────────┐
              │ Deriving（纯函数推导，无 IO / 无网络）           │
              │ text → EmotionHint + IntentHint               │
              └───┬───────────────────────────┬──────────────┘
   正文为空 / 纯空白 │                           │ 推导成功
   （等同「无输入」）▼                           ▼
       ┌───────────────────────┐   ┌────────────────────────────────────┐
       │ Silent（静默）          │   │ Decided（决策）                     │
       │ 无决策、零副作用        │   │ TtsParamPlan{voice?, model?}        │
       └───────────┬───────────┘   │ + SlotIntent[<契约槽位名>]           │
                   │               └────────────────┬───────────────────┘
                   │                                │ 本轮唯一合法落点：
                   │                                │ apply_settings → [tts].voice
                   │                                │ （只对下一轮生效，§3.1）
                   │                                ▼
                   │               ┌────────────────────────────────────┐
                   │               │ Applied / ContractOnly（已下发或仅记录）│
                   │               └────────────────┬───────────────────┘
                   └────────────────┬───────────────┘
                                    ▼
                                  Idle

失败态（Fail-Open，横切所有状态）：
  推导不可用 / 无证据 / apply_settings 返回 false / 事件队列满被丢弃
    → 本轮**零副作用**：不写配置、不记槽位为「已发」、不阻断主链
    → 只经 ModServices.logger 记一条 code=director_degraded 的 warn
      （不新增 println! 路径；与「链路错误必须可观测」同一条纪律）
```

状态语义与「谁来驱动迁移」：

| 状态 | 进入条件 | 退出条件 | 允许的副作用 |
| --- | --- | --- | --- |
| `Idle` | 进程启动 / 上一轮收口 | `TurnStarted` | **零**（尤其不写 `[tts]`） |
| `Observing` | `TurnPrompt` | 下一轮 `TurnPrompt`（权威）或 `VoiceEnded`（可选加速） | **零** |
| `Deriving` | 有观察到的正文 | 推导返回 | **零**（纯函数） |
| `Silent` | 正文为空 / 纯空白 | 下一轮 `TurnStarted` | **零**（不得把空输入硬判成 `Neutral` 以外的情绪） |
| `Decided` | 推导给出非中性结果 | 落点尝试完成 | 仅 §3.1 许可的 patch |
| `Applied` / `ContractOnly` | patch 成功 / 无通道可用 | 下一轮 `TurnStarted` | 无 |
| `Failed`（横切） | 任一环节不可用 | 下一轮 `TurnStarted` | **零**，且**绝不**把主链判失败 |

**关键时序后果**（与记忆 Mod 同一条，见 [`../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md) §1 基座表）：
`TurnPrompt` 发出时本轮请求体**马上**构建，因此导演在本轮事件里做的任何
`apply_settings` **只对下一轮生效**。「观察到第 N 轮 → 影响第 N+1 轮」不是缺陷，
是唯一可能的语义；任何「当轮改音色」的承诺都是空头支票（§3.1 会说明为什么连
「下一句」都做不到）。

## 2. 输入

### 2.1 可用事件（逐条：payload、发点、导演怎么用、限制）

来源：`crates/live2d-ai-mod-system/src/topics.rs`（**基座文件，导演不得改**；
下列「限制」全部是读实现得出的硬事实，不是设计愿望）。

| 主题 | payload | 实际发点 | 导演的用途 | 限制 / 坑 |
| --- | --- | --- | --- | --- |
| `TurnStarted` | turn id（纯序号字符串） | `supervisor.rs`，`say_rx` 收到文本后 | **轮次边界标记**：复位上一轮观察、把状态机从 `Silent`/`Applied` 拉回 `Observing` | **拿不到正文**。它只回答「开了一轮」，不能用来判情绪 |
| `TurnPrompt` | **本轮输入正文**（聊天框 / external-input / voice 转写渲染后的同一份字符串） | 紧随 `TurnStarted`，同一提交点 | **权威输入**：白/黑名单、长度闸门、情绪/意图的第一手证据 | ① 只代表**用户侧**输入；② 在此做的 `apply_settings` 只对下一轮生效；③ 正文为空时不得造决策（`Silent`） |
| `TextDelta` | 一句**完整正文**（`ConversationUiEvent::TextDelta`） | `supervisor/handlers.rs`，`EngineEvent::SentenceVoiced` 处 | 回复侧情绪/意图的**补充**证据（用户问「今天很累」vs 模型回「那早点休息」） | **不是 token 流**！它是「该句语音**已完整合成**」之后才发的**同拍正文**（[`core-chain-baseline.md`](core-chain-baseline.md) §2.1）。因此导演**永远比发声晚一句话**：看到第 k 句时第 k 句的音频已经合成完毕。原始 LLM 增量只做控制台回显，**不投影给 Mod** |
| `VoiceStarted` | epoch 字符串 | 首个非空 PCM 成功入环时（**轮级**，不是句级） | 粗粒度节拍：标记「本轮开始发声」 | 轮级；一轮只发一次。TTS 未配置/全静音句时可能**根本不发** |
| `VoiceEnded` | epoch 字符串 | 声卡侧静默收尾（排空 / 被清 / stop / 声卡故障兜底） | **可选**的「本轮差不多结束了」加速节拍，用于提前结算决策 | 轮级；失败轮可能不发。**不得**把它当唯一的轮收口信号 |
| `ModelActivated` | —（本树无发点） | **当前没有任何 host 发点** | 理论上「模型切换 = 表演语境变了」 | **休眠主题**：`topics.rs` 里声明了，但 desktop 侧零 emit（唯一订阅者 `local-llm` 已废除启动）。导演**不得**把逻辑挂在它上面 |
| （`ActionFinished`） | 动作协议名 | 动作自然播放完成的事实分支 | — | **无驱动方**：动作在产品路径上不存在（[`core-chain-baseline.md`](core-chain-baseline.md) §3.1），该分支永不触发。**不是**导演的输入 |

### 2.2 情绪 / 意图怎么从文本得到（**只能 Mod 侧推导**）

**红线**：主链**不新增**情绪字段、不新增情绪事件、不为导演改
`ConversationConfig` / `PersonaSettings` / 引擎；导演**不得**起第二个 LLM 客户端
（那会绕过统一端点权威并复制核心网络层，属 [`mod-product-chain.md`](mod-product-chain.md) §7
「不得绕过 core 仲裁」）。

因此推导是**本地、确定、可单测的纯函数**（与 memory 的「不外挂向量云、不引入
embedding 依赖」同一条纪律）：

```rust
// 契约草图（本轮不实现，仅定义形状）
pub enum EmotionHint { Neutral, Happy, Sad, Angry, Surprised, Anxious, Affectionate }
pub enum IntentHint  { Chat, Question, Greeting, Farewell, Request, Complaint, Silence }

pub fn derive(text: &str) -> (EmotionHint, IntentHint);
```

实现口径（写进未来的 crate 头注与单测）：

1. **词表打分**：固定关键词/短语表 + 程度副词权重；中文按字 bigram 与关键词双路，
   ASCII 按词（可复用 memory 的 token 化口径，但不共享代码）；
2. **标点/形状启发**：`？`/`?` → `Question`；`！` + 短句 → 情绪强度上调；
   连续重复字符、颜文字/emoji 白名单 → 强度上调；
3. **fail-safe 到中性**：未知词、空串、纯空白、纯标点、超长截断后无命中 →
   `Neutral` + `Silence`/`Chat`。**永不**返回「错误」——没有证据不是故障；
4. **确定性**：同一输入恒等输出；不读时钟、不读文件、不联网（单测不需要 sleep / mock）；
5. **长度闸门**：与 `voice-input::clean_transcript` 同口径，清洗后为空 → `Silent`，
   **不发决策**（空输入回合不得产生任何配置写入）。

### 2.3 明确「不做」的输入改造

| 想要的输入 | 为什么不在这里做 |
| --- | --- |
| 主链新增 `emotion` / `mood` 字段 | 违反本题红线；且会给「LLM 必须输出结构化情绪」的假承诺（DeepSeek API 无此能力，同 [`mod-product-chain.md`](mod-product-chain.md) §8） |
| 主链新增 `TurnEnded` / `TurnFailed` 主题 | `topics.rs` 是**基座独占文件**（Wave 2 §1 红线）。若确实需要，由主 agent 在基座补，**导演轨不得自己动**（见 §9 缺口 1） |
| 原始 LLM token 流（`EngineEvent::TextDelta`）投影给 Mod | 会破坏「正文与音频同拍」的语义分层，且思考/正文共用通道容易把 `reasoning_content` 混进来（[`AGENTS.md`](../../AGENTS.md) 推理模型一节）。**不做** |
| 第二 LLM 调用做情绪分类 | 复制核心网络层、多一份端点与密钥路径、一轮多一次延迟与费用。**不做** |

## 3. 输出

### 3.1 TTS 参数选择：**哪些参数、哪条通道、边界在哪**

先把今天的事实钉清楚（读实现得出，**不是**设计）：

- TTS 请求体只有四个字段：`input` / `model` / `voice` / `response_format`
  （`crates/live2d-ai-runtime/src/tts.rs`，`SpeechRequestBody::from_config`）。
  **前三个以外的任何参数都不在 wire 上**。
- `voice` / `model` / `response_format` 全部来自**配置快照**（`TtsConfig`），
  **没有任何 per-request 覆盖路径**：引擎从 `ctx.client.tts()` 读配置，
  客户端在 `reload` 时重建（`supervisor.rs`）。
- 唯一存在的「Mod 改 TTS 配置」通道是 `ModServices.apply_settings`
  （`crates/live2d-ai-mod-system/src/services.rs`）→ `live2d-ai.toml` 写盘 + `reload()`。
  它**写的是同一个 `[tts]` 段**、**持久化**、**全局**、**下一轮生效**。
- `SettingsPatch` 的 `TtsPatch` 只有六个键：`base_url` / `model` / `voice` /
  `api_key_env` / `sample_rate` / `channels`——**没有** `response_format`。

因此本 RFC 把参数分成三档：

| 档 | 参数 | 导演可否选 | 通道 | 理由 |
| --- | --- | --- | --- | --- |
| **A. 唯一候选** | `[tts].voice` | 未来**仅可提议**；**本轮不写**（无 crate、无写入者） | 若将来做：`ModServices.apply_settings` → `{"tts":{"voice":"…"}}` | 唯一既是「表演参数」又能经既有通道落地的键 |
| **B. 条件候选** | `[tts].model` | 同上（仅在多音色服务按 model 区分时有意义） | 同上 | 与 `voice` 同类；服务不支持时是 no-op，必须在文档里承认 |
| **C. 禁止** | `[tts].base_url`、`api_key_env` | **红线禁止** | — | 端点唯一权威在 `live2d-ai.toml`（[`tts-is-core.md`](tts-is-core.md) §1/§2）。导演改端点 = 造出第二个权威 |
| **C. 禁止** | `[tts].response_format` | **禁止** | — | 不是表演参数，是**解码契约**：`pcm`/`wav` 决定 [`core-chain-baseline.md`](core-chain-baseline.md) §4 的解析路径；且 patch 通道根本没有这个键 |
| **C. 禁止** | `[tts].sample_rate`、`channels` | **禁止** | — | 被不变量锁死：`verify_core_chain.py` 断言 WS `audio` 帧的 `sample_rate` 与 `[tts]` 一致（不一致会逐片重采样 → 咔哒声，[`core-chain-baseline.md`](core-chain-baseline.md) §6.1）。改它 = 破坏已钉死的验收不变量 |

**本轮的执行边界（比「未来能不能」更重要）**：

1. **本轮没有任何写入者**——本文档不含 crate，所以「导演写 TTS 参数」这件事
   在本轮**不可能发生**，也无从越界。
2. **未来若做**，只有 `apply_settings` 一条通道，且必须同时接受两个明示代价：
   a. **只对下一轮生效**（时序，§1）；
   b. **会改写用户的 `live2d-ai.toml`**——这是持久、全局的写入，不是「本轮临时参数」。
   正因如此，**「每轮换音色」这种用法在本 RFC 里被明确判为不可接受**：
   它会把用户文件当草稿纸每轮重写。真正干净的「每轮参数」需要主链新增
   **per-request 覆盖** 契约（不落盘、只影响本轮请求体），那是**核心链路改动**，
   必须先过 §5.2 的门槛。
3. **端点红线是可测的**：将来任何导演实现都须带一条断言——
   送往 `apply_settings` 的 patch 键集合 ⊆ `{tts.voice, tts.model}`；
   出现 `base_url` / `api_key_env` 即门禁红。

### 3.2 预置动作**槽位**（slots）：只有契约占位，不接真通道

**前提事实**：动作在产品路径上**不存在**（[`core-chain-baseline.md`](core-chain-baseline.md) §3.1
的裁决，不是「暂时没接」）。而且唤醒动作真通道有三重前提（§3.2 三条理由 +
`lib.rs` 两条不变量 + 显式恢复 `RootEvent::Action` 注入分支）。
所以导演**今天能定义的只有名字**。

槽位契约（本文档定义的唯一形态，**不是**代码）：

```jsonc
// 未来 director 的 state_json() 里可以出现的一段（本轮无实现）
{
  "slots": [
    { "id": "greet",   "intent": "Greeting",  "emotion": "Happy",   "channel": "none" },
    { "id": "comfort", "intent": "Complaint", "emotion": "Sad",     "channel": "none" },
    { "id": "agree",   "intent": "Chat",      "emotion": "Neutral", "channel": "none" }
  ],
  "emitted": []          // 本轮「想发」的槽位名；⚠ 只是记录，没有任何消费者
}
```

三条硬约束：

1. **`channel: "none"` 是唯一允许的值**。槽位是「命名 + 语义标注」，
   **不得**映射到 `action_tx`、**不得**映射到 core 的 `action/` / `performance/`、
   **不得**新增 WS 帧、**不得**写 wasm 协议。
2. **槽位不进任何注册表**：不是 `ModFactory` 注册项、不是能力声明、
   不是 `capabilities` 字段。它只活在本文档（以及未来 director 自己的
   `state_json` 只读快照里，供人看）。
3. **「emitted」不是承诺**：它是导演内部意图的**只读投影**。前端/日志看到
   `emitted: ["comfort"]` 时**不允许**把它解释成「动作发生了」——因为动作机制
   不存在。将来若真要接真通道，必须先走 §4 + §5 的门槛，并重新论证动作裁决。

**为什么不干脆定义一套完整动作库？** 因为那正是 rc.2 删掉的东西
（`archive/action-layer-p6`），且「实现动作库」是本题红线。槽位的价值在于
**先固定词汇、不让实现自行发明协议**；代价是它现在什么都不驱动——这一点明文写出来，
而不是留给读者猜。

## 4. 与休眠 `action_tx` 的关系

### 4.1 它现在是什么（读实现，2026-09-14）

`ModServices.action_tx`（`crates/live2d-ai-mod-system/src/services.rs`）仍是
**Mod API 契约**的一部分，但 host 注入的是一个**固定休眠 sender**
（`crates/live2d-ai-desktop/src/mod_registry.rs`，`make_services`）：

- 任何 `ActionRequest` → 只留一条 `tracing::debug!`（「动作子系统休眠中」）→
  **返回 `false`**（= 未被接受）；
- 它**不通向**任何地方：`HostChannels` 只有 `say` / `apply_settings` / `read_settings` /
  `config_path`；`SupervisorHandle::trigger_action` 与 supervisor 的 `action_rx`
  分支已整体删除——那是**唯一**能把 `RootEvent::Action` 送进 core reducer 的路径；
- 钉子两条：`mod_registry::tests::action_request_is_dormant_not_delivered`
  （`ActionRequest` 必须**不被接受**）与 `main.rs::mod_count_is_five`
  （工厂数不得因动作 Mod 增加）。

### 4.2 导演若将来真要动它，先满足什么

按顺序，**全部**满足才允许触碰（缺一条就是「顺手接回去」）：

1. 重新论证 [`core-chain-baseline.md`](core-chain-baseline.md) §3.2 的三条理由
   （`action/` 是自洽带单测的内部能力 / `lib.rs` 第 4、6 条不变量以它为前提 /
   本轮目标是链路干净而非删净未接线能力）——**在新的产品前提下**逐条回应，而不是复述；
2. 重新论证 `live2d-ai-core/src/lib.rs` 的两条不变量（单 active 动作、表演参数曲线）；
3. 产出并评审一份**显式恢复 host 通道**的设计（`RootEvent::Action` 注入分支由谁重建、
   epoch/审计怎么记、失败怎么回滚）；
4. 给出「动作不会挤掉对话」的**实测**证据——rc.1 的教训是
   「空 system prompt + 一个 `perform_action` 工具 ⇒ 模型只调工具、不说话，
   产出正常完成但一个字都没有的回合」（[`core-chain-baseline.md`](core-chain-baseline.md) §3.1
   当场复现过）。导演若把动作重新引入 LLM 路径，必须证明这条不回归；
5. 渲染面若同步改造，验收方式只能是 **wasm 重建 + 肉眼确认**（§3.3 的方式），且
   **待机生命体征 `IdleState` 一行不动**（§3.4）；
6. 触碰 `mod_registry.rs` / `supervisor.rs` / `main.rs` 属**基座独占文件**——
   导演轨不得自己改，须由主 agent 在基座提交里做（Wave 2 §1 红线）。

### 4.3 本轮明确不动的部分

- `action_tx` **保持休眠 sender**：不改实现、不改签名、不改那条回归；导演**不订阅**
  也不**调用**它（本文档没有 crate，所以是「零调用」而不是「调用了但被拒」）；
- **不实现动作库**、不定义可播放动作表、不在槽位里编码强度/时长；
- **不改** core 的 `action/` / `performance/`、不恢复 `RootEvent::Action` 注入；
- 导演的槽位输出**只**出现在「未来 `state_json` 的只读快照」这一种形态里
  （§3.2），**没有**第二条下行路径。

## 5. 晋升核心的门槛（可验证的准入条件）

「晋升」有两个层级，门槛不同，**不许合并成一次跳**。

### 5.1 第一级：晋升为「注册的 Mod」

| # | 准入条件（可验证） | 怎么验 |
| --- | --- | --- |
| 0 | **前置**：§3.1 的落点已被**明确授权**——即「允许经 `apply_settings` 写 `[tts].voice`」这一**持久化**写入被产品负责人接受为可付的代价；否则第 1 条的演示无从谈起，本项直接不成立 | 设计评审记录（谁授权的、授权到哪个键） |
| 1 | **≥3 条用户可见演示**：同一句文本，开 / 关 director 时 `voice`（或降级为「无差异但 state 可见」）呈现可观察差异 | Windows 浏览器 `/app/` 肉眼 + 截图/录屏；**不接受**单测替代（教训：「测试全绿 ≠ 界面是对的」） |
| 2 | 推导纯函数单测 **≥20 条**：空 / 纯空白 / 超长 / 纯标点 / 中英混排 / 未知词 → `Neutral` / 同一输入恒等 | `cargo test -p live2d-ai-mod-director`，数字写进 REGISTER |
| 3 | **失败隔离可证明**：`on_event` 返回 Err → 槽位清空且**当轮主链照常完成**；`apply_settings` 返回 false → 零副作用且主链照常；`state_json` 不阻塞（`try_lock` 失败 → 503） | 三条回归 + 一次手改配置注入错误的手动验证 |
| 4 | **端点红线可测**：一条断言「送往 `apply_settings` 的 patch 键集合 ⊆ `{tts.voice, tts.model}`」 | 单测；出现 `base_url` / `api_key_env` 即红 |
| 5 | **动作零接触可测**：一条断言「director 不 `use` core 动作类型、不调 `action_tx`、不产出 `channel != "none"`」 | 单测 + `grep` 级检查 |
| 6 | **不碰基座独占文件**：diff 中不含 `topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` / `supervisor.rs` | PR diff 逐文件核对；需要改 = 基座不足，交主 agent |
| 7 | 全量门禁绿：`cargo test --workspace --all-targets` / `--doc` / `fmt --check` / `clippy -D warnings` / `rust-ratio ≥ 95%`；`MOD_API_VERSION` 对齐 | 命令输出贴进 REGISTER |
| 8 | **具名维护者**：`REGISTER-director.md` 写清 owner（「谁来维护第二个 UI 壳」是同一条问题） | REGISTER 文件必须点名 |
| 9 | 观感与文案：`GET /api/v1/mods/director/state` 200 且**脱敏**（无 token/密钥明文，契约见 `ModRuntime::state_json` 头注）；错误码走 `director_*` 前缀进 tracing，不新增 `println!` | 路由回归 + 日志抽查 |

**注**：第 1 条要求「≥3 条演示」而不是「≥3 个单测」，是刻意的——
本仓的历史教训是异步时序 / 浮层时机 / 平台视图这类缺陷**只有真的在浏览器里点一遍才会露出来**。

### 5.2 第二级：晋升为**核心链路**（per-request TTS 参数覆盖）

只有当「全局、持久、下一轮生效的 `apply_settings`」被证明**不够用**（§3.1 的
代价 a/b 有一个真的挡住了产品需求）才启动，且必须**全部**满足：

- [ ] 现象证据：记录 ≥3 轮真实对话，说明「同一轮内需要不同 `voice`」或
      「不该改写用户 `live2d-ai.toml`」是真实需求，而不是想象；
- [ ] 契约设计：定义 per-request 覆盖字段（请求体从哪来、谁有权写、
      缺省/清空语义、与 `[tts]` 冲突时的优先级——**建议显式 per-request > `[tts]`，
      且 per-request 不落盘**）；
- [ ] 不变量重论证：该契约**不得**触碰 `sample_rate` / `channels` / `response_format`；
      必须写明它如何与 `verify_core_chain.py` 的
      「`audio.sample_rate` == `[tts].sample_rate`」断言共存
      （[`core-chain-baseline.md`](core-chain-baseline.md) §6.1）；
- [ ] 同拍契约不变：覆盖**不得**改变 `SentenceVoiced` 上屏闸门与逐句 TTS 队列语义；
- [ ] 端点权威不变：`base_url` / `api_key_env` 仍然只有 `[tts]` 一个来源；
- [ ] 核心评审：作为**核心链路改动**单独立项（[`tts-is-core.md`](tts-is-core.md)
      的立场是 TTS 在链路中间，不是在旁边），版本线由主 agent 决定；
- [ ] 回滚方案：一键关掉覆盖后行为与今天完全一致。

## 6. 与 persona / memory 的边界（写入者 × 字段）

这是导演**最容易越界**的地方，因此列成表。口径：**一列一写者，越界即缺陷**。

| 字段 / 落点 | 唯一写入者（今天的真源） | director | 备注 |
| --- | --- | --- | --- |
| `persona.system_prompt` | `persona` Mod（启用时合成角色卡）或 memory Mod（marker 块）；用户也可经 `PATCH /api/v1/settings` 写 | **禁止** | persona 与 memory 之间已是 **last-writer-wins**（memory 先剥 marker 再拼，见 Wave 2 §3C）。导演**不得**成为第三个写者，否则「谁写的」无解 |
| `persona.max_history_pairs` | 用户 / Flutter `PATCH /api/v1/settings` | **禁止** | 导演不得以「多轮语境」为名改它 |
| `[tts].base_url`、`[tts].api_key_env` | 用户 / Flutter（端点唯一权威） | **禁止**（红线） | [`tts-is-core.md`](tts-is-core.md) §2 |
| `[tts].voice`、`[tts].model` | 用户 / Flutter 是权威 | **未来仅可提议**；**本轮不写** | 唯一候选输出（§3.1 A/B 档）；不得持久化每轮改写 |
| `[tts].response_format` | 用户（配置文件；patch 通道无此键） | **禁止** | 解码契约，不是表演参数 |
| `[tts].sample_rate`、`[tts].channels` | 用户 / Flutter；同时被 `verify_core_chain.py` 不变量锁死 | **禁止** | 改了会破坏 WS 音频一致性断言 |
| `[llm].*` | 用户 / Flutter `PATCH /api/v1/settings` | **禁止** | 导演不选模型、不碰端点、不碰密钥 |
| 壁纸偏好 `DisplayPrefs.stageImage` / `shellImage` / `syncShellStageBg` / `stagePlaylist` | **Flutter**（localStorage，唯一写入者）；`wallpaper` Mod 只**出决策**，由 Flutter 消费后落盘 | **禁止** | 壁纸归属 B 轨（`wallpaper-mod-v0.md` §5）；导演不得直写前端偏好，也不得新增 wasm 路径 |
| 各自 Mod 的 `mods.json` config | 各 Mod 自己的 namespaced 段 + Flutter config API | 仅自己的 `director` 段（本轮无 crate = 无写入） | 不得借 `apply_settings` 写别人的 namespace |
| core 动作 / 表演状态 | **无驱动方**（休眠） | **禁止** | §4；[`core-chain-baseline.md`](core-chain-baseline.md) §3.2 |

两条总结纪律：

1. **导演是只读主链 + 只提议表演参数的旁路**。它不写人设（那是 persona/memory 的）、
   不写壁纸（那是 Flutter/wallpaper 的）、不写端点（那是 `[tts]` 的）。
2. 与 memory 的**节拍同源**（都挂在 `TurnPrompt`、都只对下一轮生效），
   但**输出面不重叠**：memory 动的是「说什么」（`system_prompt`），
   director 动的是「怎么说」（`voice`）。两者**不共用**通道，也不互相等待。

## 7. 非目标

### 7.1 本轮（Wave 2 D 轨）明文不做

- **不新建 crate**（无 `crates/live2d-ai-mod-director/`）、**不改任何 `.rs`**；
- **不注册** `AVAILABLE_MOD_FACTORIES`、不新增 id、不碰 `mod_count_*` /
  `mod_factory_ids_match_expected` / `cli_entry::default_mods_manifest` / 版本号；
- **不复活 Action 真通道**：`action_tx` 保持休眠、core `action/` / `performance/` 不碰；
- **不实现动作库**、不定义可播放动作、不写 wasm 协议、不新增 WS 帧；
- 不改 persona / memory / wallpaper / voice-input / pet-desktop 任何一轨的文档或代码；
- 不写需求里没提的「导演 UI」。

### 7.2 长期非目标（写进契约，防止以后被当成 backlog）

- LLM 工具 / function calling 的**任何**形式（rc.1 的整体拆除不再回退）；
- 由导演驱动的**动作序列 / 编舞 / 表情编排**——除非 §4.2 全部满足并重新裁决；
- 第二 LLM 调用、embedding、向量库、云端情绪分类；
- 把导演做成「第二个 TTS 端点/第二个 LLM 端点」的权威（端点权威永远只有 `[tts]` / `[llm]`）；
- 导演直写 Flutter 偏好 / localStorage / 前端 UI 状态；
- 「当轮改音色」「改这一句的音色」这类**做不到**的承诺（§1、§3.1）；
- 动态 `.so` / 热插拔市场（[`mod-product-chain.md`](mod-product-chain.md) §8）。

## 8. 「不注册」决定的论证

Wave 2 §3D 已把选择钉死：**本轮不新建 crate、不注册 `AVAILABLE_MOD_FACTORIES`**。
本节把理由写全，供以后翻案时对照。

### 8.1 被采纳：只交文档、默认不注册

1. **没有消费者就没有产品价值**。本文档不含实现；注册一个什么都不做的工厂，
   在 `GET /api/v1/mods` 里平白多一条列表项，却是唯一一个「启用后什么都不会发生」
   的条目。Wave 2 §0 的原话是「再交一个『只有 `settings_spec` + 空 tick』的半成品
   不算过关」——注册一个**连 `settings_spec` 都没有**的 doc-only 工厂只会更差。
2. **计数断言是安全护栏，不能被稀释**。`mod_count_is_five` /
   `mod_factory_ids_match_expected` 存在的意义是阻止**动作 Mod 悄悄挂回来**
   （[AGENTS.md](../../AGENTS.md)：director 是动作序列的唯一驱动方，而动作在产品路径上
   不存在，所以它被删除并由计数断言守住）。把导演加进去会**削弱**这条护栏：
   以后「工厂数对不对」不再能推出「没有动作驱动方混进来」。
3. **契约还不稳定，注册会冻结它**。注册意味着 `descriptor` / `api_version` /
   工厂身份成为公开面；而本文档的状态机、槽位词汇、TTS 参数档位都还在
   **草案**阶段。先冻结实现面，再让契约演进，方向是反的。
4. **本仓的既有流程是「有接线 + REGISTER 才注册」**。wallpaper / voice-input /
   persona 三条轨都是「crate + 落点 + REGISTER-<id>.md」之后由集成方注册
   （见 [`../plans/parallel-mods/REGISTER-wallpaper.md`](../plans/parallel-mods/REGISTER-wallpaper.md)）。
   导演没有落点（§3.1 本轮不写参数），所以它连 REGISTER 都不该有——
   Wave 2 §3 表格里 D 轨「REGISTER：不需要（不注册）」正是这个意思。
5. **越权风险**：本文档里唯二可能「动手」的地方是 `[tts].voice` 与槽位；前者需要
   §5.2 的核心门槛，后者需要 §4.2 的动作裁决。**在两个门槛都没过之前注册**，
   等于把一份未获授权的写入能力放进注册表。

### 8.2 被否决：注册但 `enable` 即 `Failed`

这个选项听起来「既占了位置又不骗人」，但逐条看：

1. **它把 `Failed` 语义用坏了**。[`mod-product-chain.md`](mod-product-chain.md) §7 里
   `Failed` 是**失败隔离**状态（`create`/`start` 返回 Err、配置错误、api_version 不匹配），
   用来区分「这个 Mod 坏了」和「这个 Mod 没启用」。用它表示「还没实现」，
   会让 `GET /api/v1/mods` 的读者**无法分辨**「导演配置错了」和「导演还没做」——
   两种情况的处置完全不同。
2. **它让计数断言同样失去意义**——与 8.1 第 2 条是同一个损失，只是换了个马甲：
   照样多一个 id、照样要改 `mod_count_*` 与 id 列表。
3. **它是永久的假广告**。Flutter 的 Mod 管理面会渲染出这个条目，用户点「启用」，
   看到它立刻变 Failed，并且 `GET /api/v1/mods/director/state` 永远 503
   （未启用 / 无 runtime / 未实现 `state_json` 三种成因本来就归同一个 503，
   见 `mods_routes.rs` 的路由注释）。它提供的信息量为零——本文档提供的信息比它多。
4. **它没有任何收益**。「预留位置」不是需求：注册是一行代码的工作，
   等 §5.1 的门槛过了再加，成本不变；而提前加要付上面三条代价。
5. **与红线直接冲突**：「不实现动作库」「不复活 Action 真通道」都意味着导演在
   可预见的将来没有可接线的东西；为它占一个 Failed 槽位，只会让下一轮读者
   误以为「导演已经动过工」。

**结论**：不注册。将来若要注册，走 §5.1 的 checklist + `REGISTER-director.md`，
并由**集成方/主 agent** 改基座独占文件，导演轨自己不碰。

## 9. 未决 / 已知缺口

1. **没有 `TurnEnded` / `TurnFailed` 主题**。Mod 看不到 turn 的收口，只能靠
   下一轮 `TurnPrompt`（权威）或 `VoiceEnded`（可选、失败轮可能不发）来结束观察。
   本轮**不动** `topics.rs`（基座独占）；若导演真要做，建议由主 agent 在基座补
   一个 `TurnEnded{turn_id, status}`——这是**基座需求**，不是导演的实现细节。
2. **Mod 侧拿不到「收口正文」**。主链有 `TurnReport::assistant_text`，但
   `TextFallback`（失败轮整段正文）**不投影**到 Mod 主题；Mod 只能把本轮
   `TextDelta` 逐句累积出近似值，且**失败轮可能一句 `TextDelta` 都没有**。
   导演的「回复侧情绪」证据因此是**不完整**的（这也是它必须 fail-safe 到中性的原因之一）。
3. **`ModelActivated` 是休眠主题**：声明存在、host 零发点。导演不得依赖它；
   若将来要「模型切换 → 表演语境变化」，先由基座补发点。
4. **`apply_settings` 的 TTS 写入是持久且全局的**（§3.1）。本 RFC 判「每轮换音色」
   不可接受，但**没有**解决「只影响一轮且不落盘」的通道——那需要 §5.2 的核心改动。
5. **槽位词汇尚未评审**：`greet` / `comfort` / `agree` 只是示例。评审前不实现、
   不注册、不写进任何代码。
6. **本轨无 cargo 改动**，因此不存在「导演的门禁数字」；§5 里的数字是
   **准入条件**，不是本轮实测值。

## 10. 参考

- [`core-chain-baseline.md`](core-chain-baseline.md) §3.1 / §3.2 / §3.3 / §3.6：
  动作在产品路径上不存在、core 子系统休眠台账、渲染面残件已删、原生第二壳非主线
- [`tts-is-core.md`](tts-is-core.md)：TTS 是核心链路，`[tts]` 是端点唯一权威
- [`mod-product-chain.md`](mod-product-chain.md) §2 / §5 / §6 / §7：Mod 契约面、
  现行 Mod 表、正式版 Rust/C 规则、失败隔离
- [`wallpaper-mod-v0.md`](wallpaper-mod-v0.md) §5 / §8：决策 → 显示层落点与「不做大轮播」
- [`directory.md`](directory.md)：`live2d-ai-mod-director` 已于 `0.1.0-rc.2` 删除
- [`mod-community-license.md`](mod-community-license.md)：注册与分发边界
- [`../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md)
  §1（基座 `TurnPrompt` / `state_json`）、§3C（memory 注入点与边界）、§3D（本轨范围）
- [`../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md`](../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md)
  §3（冲突红线：FACTORIES / count / manifest / 版本号）
- [`../releases/v0.2.0-rc.2.md`](../releases/v0.2.0-rc.2.md)：Wave 1 三轨合成现状
