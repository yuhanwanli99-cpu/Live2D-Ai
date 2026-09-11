# Live2D-Ai PC v4 计划（对话体验优先 + 全链路插件登记）

> 范围：只谈 PC。Android 不进入本计划。
> 版本：v4，取代 PLAN-PC-V3.md。
> 原则：精简、信任本机、不做防呆；不做静默崩溃。

---

## 0. 本轮范围修正

1. 不做主播相关：删除 Streamer Director、plugin-streamer、主动搭话、观众互动、礼物策略、直播人格。
2. 只暴露通用端口链路：第三方想接入就自己往接口发事件，本项目不接任何具体平台。
3. 长程人设与记忆插件继续做，作为 v1 必做项。
4. 不做具体领域分化：不建游戏、教育、编程等垂直专家。
5. 不做个人陪伴：不建主动陪伴、不建关系养成、不做日程提醒式陪伴。
6. 保留“提升对话体验”的能力：记忆、上下文压缩、知识检索、联网查询、身体/表情、心情/TTS 等。
7. 暴露端口链路本身也必须注册为插件，未注册不得使用。

---

## 1. 核心与插件边界

### 1.1 核心（不注册为插件）

- WebSocket 服务与连接管理。
- 对话状态机与 TurnConductor。
- 核心对话脑 LLM-A。
- TTS 引擎抽象与播放队列。
- Live2D 渲染器与参数仲裁器。
- 统一 EventBus / 健康状态 / 错误可见通道。
- PluginHost 与 PluginManager。

### 1.2 插件（除核心外全部注册）

v1 必做插件：

| 插件 | 职责 |
| --- | --- |
| plugin-input-port | 暴露外部对话输入端口（HTTP/WS/文件管道），本身是插件 |
| plugin-live-ingress | 通用直播/弹幕事件输入接口，只收通用事件，不接平台 |
| plugin-memory | 短期历史、长期事实、偏好、人物关系、约定 |
| plugin-persona | 长程人设维护、设定书、角色卡导入导出 |
| plugin-context-compactor | 上下文压缩、分层摘要、token 预算 |
| plugin-knowledge | 本地知识库、文档检索、来源引用 |
| plugin-web-search | 联网查询 |
| plugin-body-director | 身体/表情/动作导演 |
| plugin-mood-tts-director | 心情与 TTS 风格导演 |
| plugin-asr | 语音识别 |
| plugin-voice-assets | 参考音频与克隆音色管理 |
| plugin-model-assets | 模型 zip 上传、删除、默认模型 |
| plugin-config-pack | 配置导入/导出/重置 |

P2 或可选，不进入 v1：

- plugin-background：背景图更换。
- plugin-desktop-pet：Live2D 悬浮窗/桌宠壳。
- plugin-vision：屏幕感知。
- 任何具体直播平台插件。
- CLI、roleplay 等。

---

## 2. 插件系统

### 2.1 注册制

- 每个插件必须有 manifest：id、name、version、description、origin（native | external）、hook_points、capabilities、config_schema、default_enabled。
- origin=native 表示主仓库自带原生插件；origin=external 表示来自插件仓库或第三方。
- 插件管理页必须明确标出每个插件是原生还是外部。
- 只有注册成功且 enabled=true 的插件才参与运行。
- 未注册插件不得运行。
- 禁用插件立即从钩子链移除；需要重启的插件明确提示。

### 2.2 插件管理 UI

设置中心新增“插件管理”：

- 已注册插件列表。
- 启用/禁用开关。
- 状态：ok / disabled / error / degraded。
- 显示插件注册的钩子与能力。
- 显示最近错误与降级原因。
- 插件配置表单由 manifest 的 config_schema 动态生成。

### 2.3 端口链路作为插件

- plugin-input-port：注册外部文本输入端口。
- plugin-live-ingress：注册通用直播事件输入端口。
- 两个端口都必须在插件管理页显示为已注册插件。
- 不接 B 站、不接抖音，只保留 HTTP/WS 通用协议。

---

## 3. 多脑编排（只保留对话体验相关）

### 3.1 专家清单

| 专家 | 是否核心 | 职责 | 输出 |
| --- | --- | --- | --- |
| Dialogue Brain LLM-A | 核心 | 生成可朗读、有人设的回复 | 流式文本 + emo token |
| Memory Brain | 插件 | 记忆写入、事实抽取、检索注入 | 记忆更新、注入上下文 |
| Body/Expression Director | 插件 | 半身动作、表情混合、motion 选择 | BodyFrame / MotionFrame |
| Mood/TTS Director | 插件 | 心情分层、TTS 风格、instruction、语速/音高 | MoodFrame / TtsFrame |
| Context Compactor | 插件 | 长上下文压缩、分层摘要 | 摘要层、token 预算报告 |

不做：Streamer Director、Personal Companion Director、垂直领域专家。

### 3.2 调度原则

- A 是热路径，流式返回，绝不被其他专家阻塞。
- Memory、Body、Mood 并行运行，设置超时；超时走确定性规则，并显示 fallback。
- 每个专家有独立状态：ok、running、timeout、fallback、disabled、error。
- 所有专家输出统一进 TurnBundle，再分发到 TTS 与 Live2D。

### 3.3 数据契约

TurnBundle：

- text：A 的干净回复。
- emotion_events：流式情绪事件。
- body_frames：B 输出，按 sentence_id 对齐。
- mood：C 输出，当前 mood tier 与持久 mood 更新。
- tts_style：C 输出，instruction/rate/pitch/volume。
- memory_updates：Memory 输出，待写入记忆的事实。
- knowledge_hits：Memory/Knowledge 检索命中。
- expert_status：每个专家的状态与耗时。

---

## 4. 长程人设与记忆

### 4.1 记忆插件

- 短期历史：最近 N 轮原文。
- 长期事实：用户称呼、偏好、习惯、约定、重要事件。
- 人物关系：可扩展字段，v1 只做简单事实。
- 写入触发：每 N 轮或用户说“记住/忘掉”。
- 检索注入：每次请求 top-k 注入。
- 管理 UI：记忆列表、删除、清空、关闭注入。

### 4.2 长程人设

- plugin-persona 维护 persona 锚定字段与设定书。
- persona 锚定始终注入，避免长上下文漂移。
- 支持 SillyTavern 角色卡导入导出，但不做酒馆 UI 复刻。
- 长程人设与即时情绪分开存储，互不污染。

### 4.3 上下文压缩

- 分层：turn_summary → topic_summary → long_term_fact。
- 原始 turn 永远落盘，压缩不丢原文。
- 动作/情绪信号单独结构化保存，不随文本摘要丢失。
- 超过 token 阈值触发压缩。
- 前端状态页显示当前上下文使用率。

---

## 5. 明确不做

- 不做主播相关。
- 不做具体平台接入。
- 不做个人陪伴。
- 不做具体领域分化。
- 不做主动陪伴。
- 不做多角色/群聊。
- 不做插件市场/在线安装。
- 不做 VRM/PNGTuber/MMD。
- 不做 AI 唱歌。
- 不做跨设备云同步。

---

## 6. 许可证

- 方案 A（推荐）：AGPL-3.0-only + 商业授权双许可。
- 方案 B（严格非商用）：PolyForm Noncommercial 1.0.0 或 BSL 1.1。

v1 上传 GitHub 前补齐 LICENSE、NOTICE、CREDITS、SPDX 头。

---

## 7. 路线图

### P0：静默崩溃清零 + 基线绿门

- 修 BiliBiliLivePlatform 缺失，只做删除或隔离，不接平台。
- 修 pytest 与前端测试。
- 统一 EventBus，所有异常可见。
- 清零空 except 与只 logger.warning 的吞错路径。

### P1：插件宿主 + 插件管理

- 实现 PluginHost 与 PluginManager。
- 设置中心新增“插件管理”页。
- 将 input-port 与 live-ingress 注册为插件。
- 将 memory、persona、context-compactor、body-director、mood-tts-director 登记为插件。
- 未注册插件不得运行。

### P2：对话体验插件落地

- 实现 TurnConductor 与 TurnBundle。
- A 流式保持；Body/Mood/Memory 并行，超时 fallback 可见。
- 实现长程人设与记忆插件。
- 实现上下文压缩器。
- 实现身体/表情导演与心情/TTS 导演。
- 实现知识库与联网查询。
- 实现 ASR 插件。

### P3：v1 发布收口

- 模型/配置/声音资产插件可用。
- 许可证、文档、测试、e2e。
- 主仓库上传 GitHub，打 tag v1.0.0。
- v1 边界：只做通用端口与对话体验链路，不接任何具体平台。

---

## 8. 立即开工项

1. 统一错误通道，修静默崩溃。
2. 搭 PluginHost 与插件管理页骨架。
3. 把 input-port 和 live-ingress 先登记为插件。
4. 设计 TurnConductor 与 TurnBundle，跑通“A 流式 + Body/Mood/Memory 并行 + 超时 fallback”。
5. 先做 Context Compactor 的 recent_turns 阈值压缩。
