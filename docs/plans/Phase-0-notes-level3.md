# Phase 0 最终侦察笔记 — 推进双端到级3

> 2026-08-07 | 侦察员: architect

---

## 一、现状诊断：为什么"一塌糊涂"

### 1.1 架构腐化

| 问题 | 严重度 | 证据 |
|---|---|---|
| **God Class** | 🔴 致命 | `MainActivity.kt` 767行，混入UI+业务+插件+聊天+TTS+ASR+权限+历史持久化+首次引导+清空确认 |
| **无ViewModel** | 🔴 致命 | 所有状态用 `remember { mutableStateOf() }` 直接写在 Composable 里，无生命周期管理 |
| **无状态集中管理** | 🟠 严重 | ChatService/VoiceIoController/PluginManager/SettingsRepository 各自散落，无单一真相源 |
| **插件装载在主Composable** | 🟠 严重 | LaunchedEffect 里加载 4+ 个插件，启动慢、耦合紧 |

### 1.2 UI/UX 问题清单

| 问题 | 影响 |
|---|---|
| 聊天气泡**完全遮挡** Live2D 模型 | 猫娘看不见，"桌宠"名存实亡 |
| 无"AI 思考中"状态指示 | 用户发送后不知道AI在工作 |
| 无"正在说话"气泡高亮 | 看不到哪句是当前播报 |
| TTS 播报和文本显示不同步 | AI说完3句了，气泡还没跟上 |
| 表情切换生硬（级1硬切） | 嘴和表情突然跳变 |
| 嘴型纯随机，不跟声音 | 嘴在乱动，对不上语音 |
| Idle 只有单循环 | 猫娘动作重复，机械感 |
| 输入栏占满底部 | 无快捷操作、无语义联想 |
| TopAppBar 只写"Live2D-Ai" | 无角色名、无形象感 |
| 首次引导是纯文字弹窗 | 不生动、无品牌感 |
| 设置页是开发者面板 | API Key输入框堆砌，不像消费产品 |

### 1.3 Android 代码量（全50文件 ~13,000行）

| 核心渲染 | 行数 |
|---|---|
| `Live2DRenderer.kt` | 1249 |
| `AnimationSystem.kt` | 1440 |
| `MainActivity.kt` | **767** ← 该拆分 |
| `PurismClippingManager.kt` | ~600 |
| `PurismModel.kt` | ~400 |
| 其余 45 文件 | ~8,500 |

---

## 二、PC 端现状速查

| 模块 | 状态 |
|---|---|
| 表情 | 级1 (关键词→索引, `live2d_model.py:188`) |
| 口型后端 | **★ 级3 已实现** (`pymouth_viseme.py` + `stream_audio.py`) |
| 口型前端 | ❌ 预编译 React 不消费 visemes 字段 |
| 动作 | 级1 (startRandomMotion, 前端bundle) |
| 提示词 | `live2d_expression_prompt.txt` → LLM嵌 `[keyword]` |

---

## 三、级3 差距矩阵

### 层1: LLM→表情（当前级1 → 目标级3: expressionMix+参数覆盖）

| 平台 | 需改动 | 改动量 |
|---|---|---|
| **PC** | `live2d_model.py` extract_emotion → 解析结构化JSON | ~150行 |
| **PC** | LLM prompt 升级 → 输出 expressionMix JSON | prompt重写 |
| **PC** | `agent/transformers.py` 接线 | ~80行 |
| **Android** | `EmotionController.kt` parseAndApply → 解析JSON | ~200行 |
| **Android** | `ChatService.kt` prompt 升级 | prompt重写 |
| **Android** | `AnimationSystem.kt` ExpressionManager → 支持混合 | ~150行 |
| **共享** | 统一结构化情感协议 | schema文档 |

### 层2: TTS→口型（Android级2-B → 级3: 元音viseme / PC级3→前端消费）

| 平台 | 需改动 | 改动量 |
|---|---|---|
| **PC** | 前端消费 visemes → ParamMouthOpenY/Form | 前端改动 |
| **Android** | 新建 `VisemeAnalyzer.kt`：WAV帧切+FFT+元音分类 | ~300行 |
| **Android** | 新建 `VisemeTimeline.kt`：时间线+插值 | ~150行 |
| **Android** | TtsProvider 接口扩展 `audioBytes: ByteArray?` | ~30行 |
| **Android** | `VoiceIoController.kt` 集成viseme驱动 | ~150行 |
| **Android** | `LipSyncMath.kt` 替换为 viseme 驱动（保留RMS降级） | ~100行 |

### 层3: 动作联动（当前级1 → 级3: 文本位置触发motion）

| 平台 | 需改动 | 改动量 |
|---|---|---|
| **PC** | TTS 字级时间戳 + emotion-motion-mapper | ~300行 |
| **Android** | TTS Provider 暴露播放进度 + motion触发 | ~250行 |
| **Android** | `AnimationSystem.kt` MotionManager → 支持标签触发 | ~150行 |

---

## 四、UI/UX 需要修复的关键项

| 优先级 | 项 | 说明 |
|---|---|---|
| P0 | Live2D与聊天分层 | 猫娘始终可见，气泡半透明浮在上层 |
| P0 | 思考中/说话中状态 | 加载动画 + 当前播报句高亮 |
| P0 | 表情 fade 过渡 | 修 fallback 硬切，句子间隙切换 |
| P1 | Idle 多样化 | ~3-5个随机子动作+不重复抑制 |
| P1 | 首次引导品牌化 | 猫娘自我介绍 + 品牌氛围 |
| P1 | TopAppBar 角色名 | 显示 persona name，非"Live2D-Ai" |
| P2 | 输入栏优化 | 快捷表情/话题建议/语音按钮视觉升级 |
| P2 | 设置页消费化 | 非开发者面板，隐藏Key细节到"高级" |

---

## 五、总改动量估算

| 模块 | 平台 | 行数 | 难度 |
|---|---|---|---|
| 层1 级3 (表情结构化) | 双端 | ~800行 | M |
| 层2 级3 (元音viseme) | Android | ~750行 | M-H |
| 层2 级3 (前端viseme) | PC | ~200行 | M |
| 层3 级3 (动作联动) | 双端 | ~700行 | M |
| UI/UX 整改 | Android | ~600行 | M |
| MainActivity 拆分 | Android | ~400行 (重构) | S |
| **合计** | | **~3,450行** | |

---

*Phase 0 结束。进入 Phase 1 结构化规划。*
