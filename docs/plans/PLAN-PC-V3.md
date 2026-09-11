# Live2D-Ai PC v3 计划（全链路插件化 + 多脑编排 + 上下文压缩）

> 范围：只谈 PC。Android 不进入本计划。
> 版本：v3，替代 PLAN-PC-V2.md 的 3-LLM 方案。
> 硬原则：精简、信任本机、不做防呆；但**不做静默崩溃**。

---

## 0. 本轮修正

1. B 站弹幕先不做，不维护，不迁移。只保留一个通用输入接口，第三方想接就自己往接口发事件。
2. 插件系统升级为**全链路注册制**：除核心外，任何能力必须先在插件系统注册并启用，才能被使用。
3. 设置中心加入“插件管理”页，显示已注册插件、启用/禁用状态、状态与错误。
4. 3 个 LLM 不够，改为**多脑编排**：对话脑 + 记忆/知识脑 + 身体/表情导演 + 心情/TTS 导演 + 主播/互动导演 + 上下文压缩器等专家。
5. 增加 LLM 上下文长度压缩策略，覆盖个人陪伴、长期人设、主播互动三个方向。

---

## 1. 核心与插件边界

### 1.1 核心（不注册为插件，代码最小化）

- WebSocket 服务与连接管理。
- 对话状态机与 TurnConductor（回合编排）。
- 核心对话脑 LLM-A。
- TTS 引擎抽象与播放队列。
- Live2D 渲染器与参数仲裁器。
- 统一 EventBus / 健康状态 / 错误可见通道。
- 插件宿主 PluginHost。

### 1.2 插件（除核心外全链路）

以下能力必须是插件，未注册不得调用：

| 插件 | 职责 |
| --- | --- |
| plugin-asr | 语音识别 |
| plugin-memory | 短期历史、长期事实、偏好、人物关系 |
| plugin-knowledge | 本地知识库、文档检索、来源引用 |
| plugin-context-compactor | 上下文压缩、分层摘要、token 预算 |
| plugin-body-director | 身体/表情/动作导演 |
| plugin-mood-tts-director | 心情与 TTS 风格导演 |
| plugin-streamer | 主播方向：主动搭话、观众互动、直播事件策略 |
| plugin-live-ingress | 通用直播/弹幕输入接口，不接具体平台 |
| plugin-web-search | 联网查询 |
| plugin-background | 背景图更换 |
| plugin-desktop-pet | Live2D 悬浮窗/桌宠壳 |
| plugin-settings-book | 设定书/角色卡管理 |
| plugin-input-port | 外部对话输入端口（HTTP/WS/文件管道） |
| plugin-model-assets | 模型 zip 上传、删除、默认模型 |
| plugin-config-pack | 配置导入/导出/重置 |
| plugin-voice-assets | 参考音频与克隆音色管理 |
| plugin-vision | 屏幕感知，可选 |
| plugin-proactive | 主动陪伴，可并入 plugin-streamer |

核心不直接 import 这些模块；只通过 PluginHost 的钩子与能力接口调用。

---

## 2. 插件系统

### 2.1 注册制

- 每个插件必须有 manifest：id、name、version、description、hook_points、capabilities、config_schema、default_enabled。
- PluginManager 启动时扫描 plugins/ 目录并加载 manifest。
- 只有注册成功且 enabled=true 的插件才参与运行。
- 禁用插件立即从钩子链移除；需要重启的插件明确提示。

### 2.2 插件管理 UI

设置中心新增“插件管理”：

- 已注册插件列表。
- 启用/禁用开关。
- 插件状态：ok / disabled / error / degraded。
- 显示插件注册的钩子与能力，避免“黑盒插件”。
- 显示最近错误与降级原因。
- 插件配置表单由 manifest 的 config_schema 动态生成。

### 2.3 权限与异常

- 插件默认权限大：可注入任意链路钩子，可阻塞任意链路。
- 但不做静默崩溃：
  - 插件异常必须有状态变化与错误事件。
  - 插件阻塞链路必须显示“被哪个插件阻塞”。
  - 插件失败不影响其他插件和核心，但一定要可见。

---

## 3. 多脑编排架构

### 3.1 专家清单

| 专家 | 是否核心 | 职责 | 输出 |
| --- | --- | --- | --- |
| Dialogue Brain LLM-A | 核心 | 生成可朗读、有人设的回复 | 流式文本 + emo token |
| Memory/Knowledge Brain | 插件 | 记忆写入、事实抽取、知识检索、上下文压缩 | 注入上下文、记忆更新、检索片段 |
| Body/Expression Director | 插件 | 半身动作、表情混合、motion 选择 | BodyFrame / MotionFrame |
| Mood/TTS Director | 插件 | 心情分层、TTS 风格、instruction、语速/音高 | MoodFrame / TtsFrame |
| Streamer Director | 插件 | 主动搭话、观众互动、礼物/弹幕策略、直播人格 | 主动消息、事件响应、优先级决策 |
| Context Compactor | 插件 | 长上下文压缩、分层摘要 | 摘要层、token 预算报告 |
| ASR / Vision / Search / Knowledge | 插件 | 输入、感知、检索 | 文本/上下文 |

这些专家不是每轮全部调用；TurnConductor 根据场景调度：

- 普通对话：A 必跑；B/C 并行尽力；M 负责检索注入。
- 直播互动：S 参与，决定哪些弹幕进 A、哪些只触发动作。
- 长期陪伴：M 与 Context Compactor 维护长期人设。
- 需要画面理解：Vision 作为插件可选参与。

### 3.2 调度原则

- A 是热路径，流式返回，绝不被其他专家阻塞。
- B/C/M/S 并行运行，设置超时；超时走确定性规则，并显示 fallback。
- 每个专家有独立状态：ok、running、timeout、fallback、disabled、error。
- 所有专家输出统一进 TurnBundle，再分发到 TTS 与 Live2D。

### 3.3 数据契约

TurnBundle：

- text：A 的干净回复。
- emotion_events：流式情绪事件。
- body_frames：B 输出，按 sentence_id 对齐。
- mood：C 输出，当前 mood tier 与持久 mood 更新。
- tts_style：C 输出，instruction/rate/pitch/volume。
- memory_updates：M 输出，待写入记忆的事实。
- knowledge_hits：M 输出，检索命中的知识片段。
- streamer_action：S 输出，直播互动决策。
- expert_status：每个专家的状态与耗时。

---

## 4. LLM 上下文压缩策略

### 4.1 预算模型

为每个专家定义 ContextBudget：

- persona_anchor：固定人设内核，不压缩。
- recent_turns：最近 N 轮原文。
- mid_summary：中期摘要，分层。
- long_term_facts：长期事实与偏好。
- knowledge_hits：知识库检索结果。
- live_state：直播/观众/礼物状态。
- motion_state：最近情绪与动作元数据，只保留结构化字段，不占正文 token。

### 4.2 分层压缩

- 原始 turn 永远落盘，不丢。
- 当 recent_turns 超过 token 阈值，触发 Context Compactor：
  1. 把最旧 K 轮压缩成 turn_summary。
  2. 多个 turn_summary 再合并成 topic_summary。
  3. topic_summary 再沉淀为 long_term_fact。
- 摘要保留：人物、时间、情绪、事件、用户偏好、约定、未完成事项。
- 动作/情绪信号单独存结构化元数据，不因文本摘要而丢失。

### 4.3 检索与注入

- 短期历史：直接进上下文。
- 长期事实：按相关度检索 top-k 注入。
- 知识库：FTS5/BM25 或 embedding 检索，v1 先 FTS5。
- 注入顺序：persona_anchor + live_state + long_term_facts + knowledge_hits + mid_summary + recent_turns。
- 每次压缩生成 token 预算报告，前端状态页可见“当前上下文使用率”。

### 4.4 触发条件

- 超过阈值：例如 recent_turns 超过 6000 token 时压缩。
- 会话切换：新会话自动从 last_summary 起步。
- 长期任务：定期把 topic_summary 写入长期记忆。
- 用户显式说“记住/忘掉”：触发记忆插件，不等周期压缩。

---

## 5. 个人 / 长期 / 主播方向

### 5.1 个人陪伴

- 记忆插件维护：用户称呼、偏好、习惯、关系状态、近况、约定。
- 对话脑每次请求都注入相关记忆。
- 长期陪伴靠“同一人设 + 持续记忆 + 主动回应”，不是靠更多表情。

### 5.2 长期人设

- 设定书插件管理 persona 与 SillyTavern 角色卡导入导出。
- 长期事实不回退、不轻易覆盖；与即时情绪分开存储。
- 人设锚定字段始终注入，避免长上下文把性格漂移掉。

### 5.3 主播方向

- plugin-streamer 负责：
  - 哪些弹幕进对话脑。
  - 哪些弹幕只触发动作或感谢。
  - 礼物优先级与大航海插队。
  - 主动搭话与冷场救场。
  - 直播中的 persona 模式切换。
- 只做通用事件策略，不接具体平台；具体平台以后作为 P2 插件。
- 通用输入接口保留：/api/live/event 与 /api/external/chat。

---

## 6. 许可证

维持 v2 建议：

- 方案 A（推荐）：AGPL-3.0-only + 商业授权双许可。
- 方案 B（严格非商用）：PolyForm Noncommercial 1.0.0 或 BSL 1.1。

v1 上传 GitHub 前补齐 LICENSE、NOTICE、CREDITS、SPDX 头。

---

## 7. 路线图

### P0：静默崩溃清零 + 基线绿门

- 修 BiliBiliLivePlatform 缺失，但只做“删除或隔离”，不接平台。
- 修 pytest 与前端测试。
- 统一 EventBus，所有异常可见。
- 清零空 except 与只 logger.warning 的吞错路径。

### P1：插件宿主 + 插件管理

- 实现 PluginHost 与 PluginManager。
- 把现有必要能力改造成插件 manifest 并注册。
- 设置中心新增“插件管理”页，显示启用/禁用/状态。
- 未注册插件不得运行。
- 通用 LiveEventIngress 插件接口上线，不接 B 站。

### P2：多脑编排 + 上下文压缩

- 实现 TurnConductor 与 TurnBundle。
- LLM-A 核心对话脑保持流式。
- 实现 Memory/Knowledge Brain、Body Director、Mood/TTS Director、Context Compactor 插件。
- 分层摘要与 token 预算报告落地。
- 所有专家状态可见，超时/失败 fallback 可见。

### P3：必要链路插件实现

- ASR、知识库、记忆、背景、桌宠、设定书、输入端口、联网查询、模型资产、配置包、声音资产。
- 插件管理页可管理全部必要链路。
- 配置导入/导出/重置可用。

### P4：主播方向与长期人设

- plugin-streamer 实现观众互动与主动搭话。
- 长期记忆与 persona 锚定闭环。
- 通用直播事件策略：哪些进脑、哪些触发动作、礼物优先级。
- 不做具体平台连接。

### P5：v1 发布

- 许可证、文档、测试、e2e。
- 主仓库上传 GitHub，打 tag v1.0.0。
- 明确 v1 边界：B 站/抖音/CLI/roleplay 均不做，只留接口。

---

## 8. 立即开工项

1. 统一错误通道，修静默崩溃。
2. 搭 PluginHost 与插件管理页骨架。
3. 把现有非核心能力逐个登记为插件，先登记后使用。
4. 设计 TurnConductor 与 TurnBundle，跑通“A 流式 + B/C 并行 + 超时 fallback”的最小 Demo。
5. 先做 Context Compactor 的 recent_turns 阈值压缩，证明长上下文不爆。
