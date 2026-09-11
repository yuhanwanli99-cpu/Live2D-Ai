# Phase 0 侦察笔记 — 深度重构 + 推进级3

> 2026-08-07 | Phase 0 自由探索

---

## 一、项目现状总览

### 1.1 规模

| 端 | 语言 | 核心文件数 | 核心代码行 | 框架 |
|---|---|---|---|---|
| Android | Kotlin | ~50 `.kt` | ~13,000 | Jetpack Compose + Purism Core |
| PC | Python | ~90 `.py` | ~15,000 | Open-LLM-VTuber (FastAPI + React) |
| 共享 | JSON/YAML | 5 | — | persona.yaml + model_registry.json |

### 1.2 三层管线现状

```
文本输入 → [LLM] → 回复文本(含[emotion]标签) ──┬──→ [层1] 表情 → Live2D 表情
                                              ├──→ [层2] TTS→口型 → Live2D 嘴型
                                              └──→ [层3] 动作 → Live2D 身体动作
```

| 层 | Android 现状 | PC 现状 | 级3 目标 |
|---|---|---|---|
| **层1** LLM→表情 | 级1：`[joy]`标签 → 索引直切，无过渡 | 级1：关键词匹配 `live2d_model.py:188` | 结构化JSON协议，表情混合+参数覆盖 |
| **层2** TTS→口型 | 级2-B：RMS驱动+随机波动 | **级3后端已实现** (`pymouth_viseme.py`) 但前端不消费 visemes | 元音viseme (A/I/U/E/O) → ParamMouthOpenY/Form |
| **层3** 文本→动作 | 级1：随机 idle 单循环 | 级1：startRandomMotion | 文本位置触发motion，情绪联动 |

---

## 二、架构腐化诊断（"一塌糊涂"的根因）

### 2.1 Android 端

| 问题 | 严重度 | 证据 |
|---|---|---|
| **God Class** | 🔴 | `MainActivity.kt` 767行：UI+业务+插件+聊天+TTS+ASR+权限+历史+引导+确认 |
| **无 ViewModel** | 🔴 | 所有状态 `remember { mutableStateOf() }` 写在 Composable 里 |
| **插件装载在主 Composable** | 🟠 | `LaunchedEffect` 加载 4+ 插件，启动慢、耦合紧 |
| **无集中状态管理** | 🟠 | ChatService/VoiceIoController/PluginManager/SettingsRepository 各自散落 |
| **代码分布极不均衡** | 🟡 | 核心3文件 (AnimationSystem+L2DRenderer+Main) = 3,456行，占总量 26% |

### 2.2 PC 端

| 问题 | 严重度 | 证据 |
|---|---|---|
| **前端预编译不透明** | 🟠 | React 前端为预编译bundle，无法直接修改消费visemes |
| **表达式协议陈旧** | 🟠 | 仍用 `[keyword]` 标签，无法表达混合情绪 |
| **Open-LLM-VTuber 上游依赖** | 🟡 | 作为 fork 使用，需谨慎维护与上游的 diff |

### 2.3 UI/UX（Android 端实测问题）

| 优先级 | 问题 |
|---|---|
| P0 | 聊天气泡**完全遮挡** Live2D 模型 |
| P0 | 无"AI 思考中"/"正在说话"状态指示 |
| P0 | TTS 播报与气泡文本不同步 |
| P0 | 表情切换生硬（级1硬切，无 fade 过渡） |
| P1 | Idle 动作机械重复 |
| P1 | 设置页像开发者面板，非消费产品 |

---

## 三、参考项目深度分析

### 3.1 my-neuro（morettt/my-neuro，⭐1K+，Python）

> "打造逼近真人的AI伙伴 — 1秒延迟、长期记忆、视觉识别、声音克隆"

**核心架构亮点：**

| 模块 | 实现 | 可复用性 |
|---|---|---|
| **TTS 双队列** | 一段文本转音频时，上一段已在播放——流水线重叠 | ⭐⭐⭐⭐⭐ Android 端急需 |
| **实时管道** | ASR → Input Routing → LLM → TTS → Playback，5 阶段流式 | ⭐⭐⭐⭐ 参考管道设计 |
| **插件架构** | 统一的 plugin hooks 系统 | ⭐⭐⭐⭐ 对齐现有 ChatHook |
| **Live2D 管理** | 多角色切换、表情动画同步 | ⭐⭐⭐ 借鉴管理层设计 |
| **长期记忆** | 社区确认有持久化记忆能力 | ⭐⭐⭐ 对齐现有 proactive |

**关键参数/经验：**
- 目标延迟：<1 秒（端到端）
- TTS 双队列确保"边生成边播放"不卡顿
- 插件在前端（Electron/JS），不在后端

### 3.2 Open-LLM-VTuber（⭐13K+，已用）

> 我们的 PC 端主轮子

**关键架构经验：**

| 经验 | 详情 |
|---|---|
| **LLM Provider 工厂** | 所有后端都是 `openai_compatible_llm` 换皮，统一接口 |
| **TTS 多引擎** | TTSFactory 支持 17+ 引擎，Edge TTS 免费可用 |
| **口型同步** | 默认用 `volumes`（RMS 分帧），通过 WebSocket 下发数组 |
| **表情提取** | `live2d_model.py` 关键词匹配，`[joy]`→索引 |
| **输出 Transformer** | 装饰器模式把 LLM token 流转换为结构化数据 |
| **WebSocket 协议** | `{type, audio(base64), volumes[], visemes[], display_text, actions}` |

**已知坑（GitHub Issue #412）：** TTS 引擎输出 `pcm_f32le` 编码时 lip-sync 计算失效。选 TTS 引擎必须验证音频编码兼容性。

### 3.3 N.E.K.O.（Project-N-E-K-O，⭐2.1K+）

> "全栈 AI 伴侣平台 — 多形态 + 多服务 + 插件商城"

| 亮点 | 架构决策 | 是否适合我们 |
|---|---|---|
| **三层服务解耦** | Main Server + Memory Server + Agent Server | ⚠️ Docker 太重，Android 不适用 |
| **多 Avatar** | Live2D/VRM/MMD/PNGTuber/桌宠 5 种 | ✅ PC 端可参考 |
| **5 层记忆** | 事实/反思/人设/短期/长期 | ❌ 过度设计，不在级3范围内 |
| **TTS 双路径** | OmniRealtimeClient (native audio) + External TTS runtime | ⭐⭐⭐ 架构思路参考 |
| **角色卡导出** | 一键分享人设 | ⭐⭐⭐⭐ 低成本高价值 |
| **主动陪伴** | topic 推荐 + activity 跟踪 | ⭐⭐⭐ 对齐现有 proactive 插件 |

### 3.4 pymouth（organics2016/pymouth，Apache 2.0）

> 我们 PC 端已用的元音口型库

| 特性 | 详情 |
|---|---|
| **算法** | DTW（动态时间规整）匹配元音，非 AI 模型 |
| **元音集** | A/I/U/E/O/SIL，输出 softmax 置信度 |
| **性能** | 移动端 CPU 绰绰有余（README 原话） |
| **问题** | Python 库，Android 端需移植或找 Java 等价实现 |

**Android 端移植选项：**
1. **手动移植 DTW 算法到 Kotlin** — 最可控，~300行
2. **Chaquopy** (Python on Android) — 引入额外依赖
3. **使用 Java 音频 MFCC 库** (如 TarsosDSP) + 自写元音分类器 — 中等工作量

### 3.5 参考项目总结：什么该抄、什么不抄

| 抄（高ROI） | 不抄（过度设计/不符合MC模型） |
|---|---|
| my-neuro 的 TTS 双队列流式播放 | N.E.K.O. 的 Docker 多服务架构 |
| Open-LLM-VTuber 的 Provider 工厂模式 | N.E.K.O. 的 5 层记忆系统 |
| pymouth 的 DTW 元音识别算法 | Neuro-sama 的直播/游戏AI系统 |
| N.E.K.O. 的角色卡导出思路 | Neuro-sama 的"翻车"争议人设 |
| Open-LLM-VTuber 的 WebSocket 协议 | my-neuro 的 Electron 前端（Android 不适用） |

---

## 四、"级3"精确定义

基于已有文档 `Phase-0-notes-level3.md` 和代码现状，明确三层级定义：

### 层1：LLM → 表情（Emotion → Expression）

| 级别 | 能力 | 当前状态 |
|---|---|---|
| **级1** | 关键词→索引直切。LLM 输出 `[joy]` → 查表得 index=5 → `setExpression(5)` | ✅ 双端均在此级 |
| **级2** | 表情交叉淡变（crossfade）。两个表情之间有过渡，非硬切 | 🔶 Android 有 ExpressionManager crossfade 基础设施 |
| **级3** | **结构化 JSON 协议 + 多表情混合 + 参数覆盖**。LLM 输出 `{"expressions": [{"name":"joy","weight":0.7}, {"name":"surprise","weight":0.3}], "params": {"ParamEyeLOpen": 1.2}}` → 混合渲染 | ❌ 双端均未实现 |

### 层2：TTS → 口型（Audio → Lip-sync）

| 级别 | 能力 | 当前状态 |
|---|---|---|
| **级1** | 固定正弦波/无口型 | — |
| **级2** | RMS 音量驱动（随音量大小张嘴）| ✅ Android: VoiceIoController 随机波动 + RMS |
| **级2-B** | RMS + 随机波动 + lerp 平滑 | ✅ Android: LipSyncMath + VoiceIoController |
| **级3** | **元音 viseme**（A/I/U/E/O → ParamMouthOpenY/ParamMouthForm，口型随音素变化） | 🔶 PC 后端已实现，前端不消费；Android 需新建 |

### 层3：文本 → 动作（Text → Motion）

| 级别 | 能力 | 当前状态 |
|---|---|---|
| **级1** | 随机 idle 循环 | ✅ 双端均在此级 |
| **级2** | 多 idle 随机 + 不重复抑制 | ❌ |
| **级3** | **文本关键词/情绪触发特定 motion**。如 LLM 说"挥手"→ 触发 wave motion；每句说完眨眼 | ❌ 双端均未实现 |

---

## 五、重构两大任务

### 任务A：老代码重构与架构分层

**目标：解决"一塌糊涂"的技术债，建立可持续架构。**

| 子项 | 说明 | 工作量 |
|---|---|---|
| **A1. ViewModel 引入** | 提取 MainActivity 状态到 `ChatViewModel`，使用 `StateFlow` 管理 | ~200行 |
| **A2. MainActivity 拆分** | 拆为 ChatScreen / Live2DHost / InputBar / OnboardingGuide 等独立 Composable | ~400行重构 |
| **A3. 统一状态管理** | ChatService/VoiceIoController/PluginManager 汇总到 ChatViewModel | ~150行 |
| **A4. 插件懒加载** | 插件不再阻塞首屏，改为按需加载+悬浮初始化 | ~100行 |
| **A5. UI 分层** | Live2D 视图层 / 气泡浮层 / 输入栏层 明确 z-order | ~150行 |
| **A6. 设置页消费化** | 非开发者面板；Key 细节进"高级"折叠区 | ~200行 |

### 任务B：推进三层到级3

| 子项 | 平台 | 说明 | 工作量 |
|---|---|---|---|
| **B1. 统一情感协议** | 共享 | 定义 JSON schema：`ExpressionMix` + `ParameterOverride` | schema 文档 |
| **B2. LLM Prompt 升级** | 双端 | 从 `[joy]` 标签升级为要求输出结构化 JSON | prompt 重写 |
| **B3. 表情混合引擎** | Android | `ExpressionManager` 扩展：多表情加权混合 + 参数覆盖 | ~200行 |
| **B4. 表情混合后端** | PC | `live2d_model.py` extract_emotion → 解析 JSON → expressionMix | ~150行 |
| **B5. 元音 viseme (Android)** | Android | 移植 DTW 算法 → Kotlin `VisemeAnalyzer` + `VisemeTimeline` | ~400行 |
| **B6. PC 前端消费 visemes** | PC | React 前端读取 visemes 字段 → 驱动 ParamMouthOpenY/Form | ~200行 |
| **B7. TTS 双队列重疊** | Android | TTS 边合成边播放（当前是等整个音频生成完再播放） | ~250行 |
| **B8. 动作联动** | 双端 | 文本→motion 映射表 + TTS 播放进度回调触发 | ~500行 |

---

## 六、关键架构决策点（Phase 1 待确认）

1. **pymouth Android 移植方案**：手动移植 DTW 到 Kotlin vs Chaquopy vs TarsosDSP？
2. **统一情感协议格式**：JSON 嵌套程度？是否参考 Open-LLM-VTuber 的 Actions 结构？
3. **PC 前端修改方案**：修改预编译 bundle vs 重新构建 React 前端？
4. **层3 动作联动优先级**：是否在级3第一阶段做，还是放到后续迭代？
5. **重构与级3的关系**：先重构再推级3？还是并行？
6. **TTS 双队列**：是否需要扩展 TtsProvider 接口（当前 `speak(text)` 是同步等待）？

---

## 七、网搜开源轮子参考清单

| 项目 | Stars | 许可证 | 可复用内容 | 集成难度 |
|---|---|---|---|---|
| **my-neuro** | 1K+ | Apache 2.0 | TTS 双队列架构理念、插件系统设计 | 低（理念级） |
| **Open-LLM-VTuber** | 13K+ | Apache 2.0 | LLM/TTS Provider 工厂、WS 协议、pymouth 集成 | 已集成（PC端） |
| **N.E.K.O.** | 2.1K+ | Apache 2.0 | 角色卡导出、多 Avatar 思路 | 低（理念级） |
| **pymouth** | — | Apache 2.0 | DTW 元音识别算法（当前 PC 端已用） | 需移植到 Kotlin |
| **TarsosDSP** | — | GPL | Java 音频处理（MFCC/FFT/pitch） | 中（GPL 传染性需注意） |
| **pixi-live2d-display** | 1.5K+ | MIT | Web Live2D 渲染（PC 前端已在用） | 已集成 |

---

*Phase 0 结束。准备进入 Phase 1 结构化规划。*
