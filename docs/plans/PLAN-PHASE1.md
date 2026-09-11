# Live2D-Ai Phase 1 结构化架构规划

> 产出时间: 2026-08-08 | 架构师: software-architect (SPOQ Phase: architecting)
> 范围: Part A (架构分层重构) + Part B (三层推进级3) + Part C (UI/UX矫正)
> 输入: Phase 0 全量侦察结论 + Phase 0 级3差距矩阵 + UX全量对标报告
> 核心约束: ChatService/EmotionController/VoiceIoController/Live2DRenderer/PluginManager **不动**（91个单测全过）

---

## 免责声明

本文档为**架构设计建议**，供 Human 审阅决策。所有文件路径引用自 Phase 0 侦察实况，行数估算基于现有代码量与级3算法复杂度推算（±30%）。实施方应优先验证框架 API 假设（GLSurfaceView.setRenderer 单次调用限制、Gradle rootProject 路径解析等已知坑）。

---

# Part A: 架构分层重构 — 技术债清偿

## A.1 问题诊断

| 问题 | 位置 | 严重度 |
|------|------|--------|
| **God Class 767 行** | `MainActivity.kt` 全文件 | 🔴 致命 |
| **MainChatScreen 505 行单体** | `MainActivity.kt:162-667` | 🔴 致命 |
| **5 个共享对象贯穿所有代码块** | voiceIo/pluginManager/chatService/scope/confirmationGate | 🔴 致命 |
| **无 ViewModel** | 所有状态用 `remember { mutableStateOf() }` | 🟠 严重 |
| **插件装载在主 Composable** | LaunchedEffect 中加载 4+ 插件 | 🟠 严重 |

**核心判断**: MainChatScreen **不能整理、必须重写**。5 个共享对象（voiceIo/pluginManager/chatService/scope/confirmationGate）贯穿所有代码块，提取任何一块都必须同时拖出其他 4 个。全量重写比逐块重构更快、更安全。

**不动区**（Phase 0 明确保留）:
- `ChatService.kt` — 91 单测全过，架构合理
- `EmotionController.kt` — 91 单测全过，架构合理
- `VoiceIoController.kt` — 91 单测全过，架构合理
- `Live2DRenderer.kt` — 核心渲染，稳定
- `PluginManager.kt` — 接口已完备
- 所有 `TtsProvider`/`AsrProvider` 实现 — V1.2 已落地
- `ChatHistoryStore.kt` / `SettingsRepository.kt` — 数据层稳定

## A.2 新分层架构

```
┌─────────────────────────────────────────────────────────┐
│  Presentation Layer  (Compose UI, 纯视图)               │
│  · MainChatScreen.kt       ≈200行  Compose 纯 UI        │
│  · Live2DStage.kt          ≈80行   Live2D 模型层        │
│  · ChatBubbleRow.kt        ≈60行   聊天气泡组件         │
│  · InputBar.kt             ≈100行  输入栏 + 快捷操作    │
│  · StatusIndicator.kt      ≈80行   思考/说话状态指示    │
│  · Dialogs/                                       │
│     ├── ClearConfirmDialog.kt    ≈40行                  │
│     ├── WelcomeGuide.kt          ≈120行                 │
│     └── ConfirmationDialog.kt    ≈40行                  │
├─────────────────────────────────────────────────────────┤
│  Domain Layer  (ViewModel + 业务编排)                   │
│  · MainChatViewModel.kt   ≈250行  ViewModel(状态管理)   │
│  · PluginBootstrap.kt     ≈100行  插件装载编排          │
│  · ChatOrchestrator.kt    ≈150行  聊天流程编排*         │
│          (* 可选: 拆分 sendMsg→receive→speak 流程)      │
├─────────────────────────────────────────────────────────┤
│  Data Layer  (不变, 已有)                               │
│  · ChatService.kt                  SSE 流式聊天         │
│  · ChatHistoryStore.kt             对话持久化           │
│  · SettingsRepository.kt           配置持久化           │
│  · ModelRegistry.kt                模型注册表           │
│  · EmotionController.kt            表情控制             │
│  · VoiceIoController.kt            TTS/ASR 协调器       │
│  · PluginManager.kt                插件管理             │
│  · LLMProviderManager.kt           LLM Provider 管理    │
└─────────────────────────────────────────────────────────┘
```

## A.3 MainChatScreen 重写方案

### A.3.1 重写前（现状）

```
MainChatScreen (505 行, 5 个共享对象)
 ├── displayMessages State (mutableStateListOf)
 ├── inputText State
 ├── isLoading State
 ├── voiceIo ref
 ├── pluginManager ref
 ├── chatService ref
 ├── scope ref
 ├── confirmationGate ref
 ├── ChatBubble composable (内联, ~100行)
 ├── InputBar (内联, ~80行)
 ├── ClearConfirmDialog (内联 AlertDialog)
 ├── WelcomeGuide (内联 LaunchedEffect)
 ├── TopAppBar (内联 设置/清除按钮)
 ├── Live2D AndroidView (内联)
 └── LaunchedEffect(plugins) { ... 插件装载 }
```

### A.3.2 重写后

```
MainChatScreen (≈200 行, 纯 Compose UI)
 ├── 持有 MainChatViewModel (remember)
 ├── 持有 Live2DView ref (onLive2DViewCreated 回调)
 ├── Live2DStage (Live2D AndroidView + 覆盖层)
 ├── ChatBubbleRow (LazyColumn items)
 ├── InputBar (发送/语音/快捷操作)
 ├── StatusIndicator (思考中/说话中 动画)
 ├── TopAppBar (角色名 + 形象切换 + 设置)
 ├── ClearConfirmDialog (独立文件)
 ├── WelcomeGuide (独立文件, 品牌化)
 └── ConfirmationDialog (独立文件, 工具确认)

MainChatViewModel (≈250 行, ViewModel)
 ├── uiState: StateFlow<MainChatUiState>
 ├── displayMessages: MutableStateFlow<List<DisplayMessage>>
 ├── isLoading: StateFlow<Boolean>
 ├── fun sendMessage(text: String)
 ├── fun clearConversation()
 ├── fun toggleVoiceInput()
 ├── fun onMicClick()
 ├── 持有 ChatService + VoiceIoController + PluginManager (通过 DI)
 └── 协程 scope: viewModelScope (自动跟随 ViewModel 生命周期)

PluginBootstrap (≈100 行, 独立编排)
 ├── fun loadCorePlugins(pluginManager, context, proactiveEnabled)
 ├── fun loadProactivePlugin(pluginManager, context)
 └── 在 ViewModel.init 或 Activity.onCreate 调用
```

### A.3.3 数据流

```
用户输入文本 → InputBar.onSend(text)
  → MainChatViewModel.sendMessage(text)
    → 追加 DisplayMessage(user) 到 displayMessages
    → isLoading = true
    → EmotionController.stripEmotionTags(text) → 只取纯文本
    → ChatService.send(pureText, systemPrompt, history)  // SSE 流式
      → 每个 delta → 追加到 DisplayMessage(assistant, 流式拼接)
      → 收到完成信号 → EmotionController.parseAndApply(fullText)
        → 提取结构化情感标签 → Live2D 表情切换
        → 剥离标签后纯文本 → VoiceIoController.speak(cleanText)
          → TTS 播报 + LipSync 驱动
          → onSpeakingComplete → isLoading = false
```

## A.4 文件拆分清单

### 新增文件

| # | 文件 | 行数 | 说明 |
|---|------|------|------|
| 1 | `MainChatViewModel.kt` | ~250 | ViewModel，状态集中管理 |
| 2 | `PluginBootstrap.kt` | ~100 | 插件装载编排，从 LaunchedEffect 抽出 |
| 3 | `Live2DStage.kt` | ~80 | Live2D 视图 + 情绪状态覆盖层 |
| 4 | `ChatBubbleRow.kt` | ~60 | 聊天气泡行组件（从 MainChatScreen 内联抽出） |
| 5 | `InputBar.kt` | ~100 | 输入栏组件（从 MainChatScreen 内联抽出） |
| 6 | `StatusIndicator.kt` | ~80 | 思考中/说话中状态指示器 |
| 7 | `ClearConfirmDialog.kt` | ~40 | 清空对话确认弹窗 |
| 8 | `WelcomeGuide.kt` | ~120 | 首次引导（品牌化，替代纯文字弹窗） |
| 9 | `ConfirmationDialog.kt` | ~40 | 工具调用确认弹窗 |

### 修改文件

| # | 文件 | 行数变化 | 改动 |
|---|------|---------|------|
| 10 | `MainActivity.kt` | 767→~200 | 移除 MainChatScreen 定义，仅保留 Activity 壳 + App 级导航 |
| — | `ChatService.kt` | 不变 | — |
| — | `EmotionController.kt` | 不变 | — |
| — | `VoiceIoController.kt` | 不变 | — |
| — | `PluginManager.kt` | 不变 | — |

### 文件关系

```
MainActivity.kt (壳, ~200行)
  └── Live2DAiApp() Composable (导航)
        ├── MainChatScreen(onOpenSettings, onLive2DViewCreated)
        │     ├── MainChatViewModel (viewModel)
        │     ├── Live2DStage(live2DView, emotionState)
        │     ├── StatusIndicator(isLoading, isSpeaking)
        │     ├── ChatBubbleRow(messages, lazyListState)
        │     ├── InputBar(inputText, onSend, onMicClick)
        │     ├── TopAppBar(personaName, onClear, onSettings)
        │     └── 各 Dialog (条件渲染)
        │
        └── SettingsScreen (覆盖层)
              └── EngineSelectorUi 等
```

---

# Part B: 三层推进级3

## 概述

| 层 | 当前等级 | 目标等级 | 核心升级 |
|----|---------|---------|---------|
| L1: LLM→表情 | 级1 (关键词→单索引) | **级3** (结构化JSON, expressionMix+parameterOverrides) |
| L2: TTS→口型 | 级2-B (随机波动) | **级3** (元音viseme分析, a/i/u/e/o区分) |
| L3: 动作联动 | 级1 (单idle循环) | **级3** (文本位置触发motion, 语义动作映射) |

**级3总策略**: "结构化JSON优先 → 降级到关键词匹配"（每层都定义降级路径）

---

## B.1 统一结构化情感协议（L1 基础 — 共享 Schema）

### B.1.1 协议设计

当前 `[joy]` / `[anger]` 关键词标签 → 升级为结构化 JSON。这是 L1→级3 的**跨平台共享基础**。

```json
{
  "$schema": "https://live2d-ai.dev/emotion-protocol/v1",
  "expressionMix": {
    "joy": 0.8,
    "surprise": 0.2
  },
  "parameterOverrides": {
    "ParamEyeLOpen": 1.2,
    "ParamEyeROpen": 1.2,
    "ParamMouthOpenY": 0.9,
    "ParamBrowY": -0.3
  },
  "transitionMs": 500,
  "holdMs": 3000,
  "motionLabel": "wave_hand"
}
```

**字段说明**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `expressionMix` | `Map<String, Float>` | ✅ | 表情名→权重 (0.0~1.0)，支持多表情混合。空 map = 回 neutral |
| `parameterOverrides` | `Map<String, Float>` | ❌ | Live2D 参数直接覆盖（如眼开合度、眉毛高度），高级用法 |
| `transitionMs` | `int` | ❌ | 表情过渡动画时长 (ms)，默认 300ms |
| `holdMs` | `int` | ❌ | 表情保持时长 (ms)，0 = 不自动回退，默认 3000ms |
| `motionLabel` | `string` | ❌ | 可选: 指定播放的动作标签（用于 L3 动作联动） |

### B.1.2 降级策略

```
级3 (最佳): LLM 输出结构化 JSON → 解析 expressionMix + parameterOverrides
  ├─ JSON 解析失败 ↓
级2 (降级): 关键词匹配 (现有 [joy] 机制) → 单表情索引 + 默认强度
  ├─ 关键词也未匹配 ↓
级1 (兜底): 回退 neutral (表情索引 0)
```

### B.1.3 LLM Prompt 升级方案

**当前 prompt** (persona.yaml 摘录):
```
可用表情关键词: [neutral], [joy], [anger], [sadness], [fear], [surprise], [smirk]
```

**升级后 prompt** (新):
```
## 表情控制 (结构化)
你可以在回复中用 JSON 表情块来控制 Live2D 表情。JSON 块独立一行，不会在语音中读出。

可用表情名: neutral, joy, anger, sadness, fear, surprise, smirk, disgust

格式:
```emotion
{"expressionMix":{"joy":0.8},"transitionMs":500}
```

多表情混合示例:
```emotion
{"expressionMix":{"joy":0.6,"surprise":0.4},"parameterOverrides":{"ParamEyeLOpen":1.2,"ParamEyeROpen":1.2},"transitionMs":400}
```

带动作联动示例:
```emotion
{"expressionMix":{"joy":0.7},"motionLabel":"wave_hand","transitionMs":300}
```

注意:
- 表情块必须用 ```emotion 代码块包裹，独立成行
- expressionMix 为必填，至少一个表情
- 如果无法确定表情，用 {"expressionMix":{"neutral":1.0}}
- 强度 0.0~1.0，越高越强
```

### B.1.4 双端改动

| 平台 | 文件 | 改动 | 行数 |
|------|------|------|------|
| **Android** | `EmotionController.kt` | 新增 `parseExpressionMix()` JSON 解析方法 | ~120 |
| **Android** | `AnimationSystem.kt` | 新增 `applyExpressionMix(mix, params, transitionMs)` | ~150 |
| **PC** | `live2d_model.py` | `extract_emotion()` → 新增 `parse_expression_mix()` JSON 解析 | ~120 |
| **PC** | `agent/transformers.py` | 接线: 传递 expressionMix 到前端 | ~80 |
| **共享** | `shared/emotion-protocol.md` | 协议文档（新增） | ~80 |

### B.1.5 Android EmotionController 改造

```kotlin
// EmotionController.kt 新增方法

/**
 * 从 LLM 回复中提取结构化情感 JSON。
 * 
 * 协议见 shared/emotion-protocol.md v1。
 * 级3 优先 → 级2 降级 (关键词) → 级1 兜底 (neutral)。
 */
fun parseAndApply(replyText: String): EmotionResult {
    // 1. 尝试提取 ```emotion JSON 块
    val emotionBlock = EMOTION_BLOCK_REGEX.find(replyText)?.groupValues?.get(1)
    if (emotionBlock != null) {
        return tryParseExpressionMix(emotionBlock)  // 级3
    }
    // 2. 降级: 现有关键词匹配
    return tryParseKeywordTags(replyText)  // 级2 → 级1 兜底
}

data class EmotionResult(
    val expressionMix: Map<String, Float>,
    val parameterOverrides: Map<String, Float> = emptyMap(),
    val transitionMs: Int = 300,
    val motionLabel: String? = null,
    val cleanText: String  // 剥离标签后的纯文本
)
```

---

## B.2 L1: LLM→表情 级3 详细设计

### B.2.1 Android 端: AnimationSystem 表情混合

**目标**: 支持 `expressionMix` 多表情权重混合 + `parameterOverrides` 直接参数控制 + 平滑过渡。

**改动文件**: `AnimationSystem.kt`（当前 1440 行，已有 ExpressionManager / MotionManager / PhysicsState）

```kotlin
// AnimationSystem.kt 新增

class ExpressionManager(private val live2dModel: PurismModel) {
    
    /** 当前活跃的表情混合（表情名 → 权重） */
    private var currentMix: Map<String, Float> = mapOf("neutral" to 1.0f)
    
    /** 目标表情混合（动画的终点） */
    private var targetMix: Map<String, Float> = mapOf("neutral" to 1.0f)
    
    /** 过渡剩余时间 (ms) */
    private var transitionRemainingMs: Long = 0L
    private var transitionTotalMs: Long = 300L
    
    /**
     * 设置目标表情混合，启动过渡动画。
     * @param mix 表情名→权重 (0.0~1.0)
     * @param transitionMs 过渡时长，默认 300ms
     */
    fun setExpressionMix(mix: Map<String, Float>, transitionMs: Int = 300) { ... }
    
    /**
     * 每帧调用: 线性插值推进过渡，计算当前帧参数值并应用到模型。
     */
    fun update(deltaTimeMs: Long) {
        if (transitionRemainingMs <= 0) return
        
        val progress = 1.0f - (transitionRemainingMs.toFloat() / transitionTotalMs)
        // 线性插值: currentMix + (targetMix - currentMix) * progress
        val interpolated = interpolateMix(currentMix, targetMix, progress)
        applyToModel(interpolated)
        
        transitionRemainingMs -= deltaTimeMs
        if (transitionRemainingMs <= 0) {
            currentMix = targetMix
        }
    }
    
    /** 直接覆盖 Live2D 参数（ParamEyeLOpen 等） */
    fun setParameters(params: Map<String, Float>) { ... }
}
```

**预估行数**: ~150 行新增（AnimationSystem.kt 已 1440 行，改动在 ExpressionManager 类内）

### B.2.2 PC 端: live2d_model.py 改造

**改动文件**: `live2d_model.py:188` (extract_emotion 方法，当前 ~30 行)

```python
# live2d_model.py extract_emotion 增强

def extract_emotion(self, str_to_check: str) -> list:
    """
    级3 优先: 解析 ```emotion JSON 块 → expressionMix
    级2 降级: 关键词匹配 [joy]/[anger]
    级1 兜底: 返回空列表
    """
    # 级3: 尝试提取结构化 JSON
    emotion_json = self._extract_emotion_json(str_to_check)
    if emotion_json:
        return self._emotion_json_to_expression_list(emotion_json)
    
    # 级2: 关键词匹配（现有逻辑保留）
    return self._keyword_extract(str_to_check)

def parse_expression_mix(self, str_to_check: str) -> dict:
    """解析结构化情感 JSON，返回前端可消费的 expression payload。"""
    ...
```

**预估行数**: ~120 行新增

---

## B.3 L2: TTS→口型 级3 详细设计

### B.3.1 现状 vs 目标

| 平台 | 现状 | 目标 |
|------|------|------|
| **Android** | `LipSyncMath.kt` — 随机波动嘴型 (级2-B)，不跟语音 | **级3**: FFT 频谱分析 → 元音 viseme (a/i/u/e/o) |
| **PC** | `pymouth_viseme.py` **级3 已实现** (pymouth DTW 元音)，但前端预编译 React **不消费 visemes 字段** | **级3 前端消费**: 前端接收 visemes → ParamMouthOpenY/Form |

### B.3.2 Android: VisemeAnalyzer 元音口型

**核心算法**: TTS 音频 WAV 数据 → FFT 短时频谱 → 元音分类 (a/i/u/e/o/SIL)。

**参考**: Android 端无现成轮子，自行实现。PC 端 `pymouth_viseme.py` + `VOWEL_MAP` 已验证方案可行。

**关键设计**: 纯 Kotlin 实现，无 JNI 依赖，使用 `kotlin.math` + `java.io.ByteArrayInputStream` + `javax.sound.sampled` 音频解码。Android 不自带 `javax.sound.sampled`，需要自写 WAV 头解析或使用轻量 WAV decoder。

**替代方案**: 使用 Android `AudioTrack` 或 `MediaCodec` 获取 PCM 数据，或直接使用 TTS Provider 返回的 `audioBytes: ByteArray`。

```kotlin
// VisemeAnalyzer.kt (新增，~300行)

/**
 * 元音嘴型分析器。
 * 
 * 算法: WAV PCM → 分帧 (20ms) → 每帧做 FFT → 频谱特征提取 
 * → 元音分类 (A/I/U/E/O/SIL) → 映射 mouthOpen/mouthForm 值。
 * 
 * 参考: PC 端 pymouth_viseme.py 的 VOWEL_MAP 方案，
 * 但用纯 Kotlin 实现（无 Python/pymouth 依赖）。
 */
class VisemeAnalyzer(
    private val sampleRate: Int = 22050,
    private val frameMs: Int = 20,       // 每帧 20ms
    private val fftSize: Int = 1024,
) {
    // 元音 → (mouthOpen, mouthForm) 与 PC 端 VOWEL_MAP 对齐
    companion object {
        val VOWEL_MAP = mapOf(
            "A"   to (0.8f to 0.2f),   // 张嘴，不撮
            "I"   to (0.3f to 0.8f),   // 微张，撮
            "U"   to (0.2f to 0.0f),   // 闭嘴，不撮
            "E"   to (0.5f to 0.5f),   // 中张，中撮
            "O"   to (0.6f to 0.7f),   // 中张，撮
            "SIL" to (0.0f to 0.0f),   // 闭嘴
        )
    }
    
    /**
     * 分析 PCM 音频数据，返回逐帧 viseme 序列。
     * 
     * @param pcmData PCM 16-bit mono 音频数据
     * @return List<VisemeFrame> 按时间戳排序的 viseme 帧
     */
    fun analyze(pcmData: ShortArray): List<VisemeFrame> { ... }
    
    /**
     * 简化的元音分类（无 FFT 时的降级方案）:
     * 基于短时能量 + 过零率做粗粒度分类。
     */
    fun analyzeSimple(pcmData: ShortArray): List<VisemeFrame> { ... }
}

data class VisemeFrame(
    val timestampMs: Long,
    val vowelLabel: String,    // "A"/"I"/"U"/"E"/"O"/"SIL"
    val mouthOpen: Float,      // 0.0~1.0
    val mouthForm: Float,      // 0.0~1.0
)
```

**降级策略**:
```
级3 (完整): FFT 频谱元音分类 (VisemeAnalyzer.analyze)
  ├─ TTS Provider 不返回 PCM (如 Edge TTS 只有 MP3) ↓
级2 (简化): 基于短时能量+过零率的粗分类 (analyzeSimple)
  ├─ 音频数据不可用 ↓
级1 (兜底): 现有 LipSyncMath.randomizedMouth (随机波动)
```

### B.3.3 Android: VisemeTimeline 时间线驱动

```kotlin
// VisemeTimeline.kt (新增，~150行)

/**
 * Viseme 时间线管理器。
 * 
 * 职责:
 * 1. 接收 analyze() 输出的逐帧 viseme 序列
 * 2. 按音频播放进度驱动当前帧嘴型
 * 3. 线性插值平滑相邻帧
 * 4. 播放结束时归零闭嘴
 * 
 * 与现有 LipSyncMath 并存（作为级3路径，现有路径保留降级）
 */
class VisemeTimeline {
    private var frames: List<VisemeFrame> = emptyList()
    private var currentIdx: Int = 0
    private var startTimeMs: Long = 0L
    private var active: Boolean = false
    
    fun load(frames: List<VisemeFrame>) { ... }
    
    /**
     * 每帧调用（100ms间隔对齐现有 LIP_SYNC_INTERVAL_MS）。
     * @param elapsedMs 自播放开始经过的毫秒数
     * @return (mouthOpen, mouthForm) 当前帧的嘴型值
     */
    fun tick(elapsedMs: Long): Pair<Float, Float> { ... }
    
    fun reset() { ... }
}
```

### B.3.4 Android: TtsProvider 接口扩展

```kotlin
// TtsProvider.kt 新增方法 (不影响现有接口)

interface TtsProvider {
    // ... 现有方法不变 ...
    
    /**
     * 级3 扩展: 返回最近一次 speak() 的音频数据。
     * null = 此 Provider 不支持 viseme 分析。
     * 
     * 实现者: 
     * - Edge TTS: 返回 WebSocket 收到的 MP3 解码后 PCM
     * - sherpa-onnx: 直接返回生成的 WAV PCM
     * - 系统 TTS: 返回 null（不提供音频数据）
     */
    val lastAudioBytes: ByteArray? get() = null
}
```

### B.3.5 Android: VoiceIoController 集成

**改动**: `VoiceIoController.kt` 约 150 行修改。

```
speak(text) 流程升级:
  1. 选择 TtsProvider
  2. visemeAnalyzer = VisemeAnalyzer()
  3. provider.speak(text)
  4. provider.lastAudioBytes → 
       visemeAnalyzer.analyze(pcm) → 
       visemeTimeline.load(frames)
  5. 播放期间: 每 100ms → visemeTimeline.tick(elapsedMs) → 
       live2dRenderer.mouthOpen/mouthForm
  6. 播放结束: mouthOpen=0, mouthForm=0
```

### B.3.6 PC 端: 前端消费 visemes

**现状**: `pymouth_viseme.py` 已生成 `visemes: [{timestamp_ms, vowel_label, mouth_open, mouth_form}]`，但预编译 React 前端**不消费此字段**。

**改动**:
- 前端 `Live2DModel.tsx` (或等效): 接收 WebSocket `visemes` 数组 → 按音频播放时间戳驱动 `ParamMouthOpenY` / `ParamMouthForm`
- 前端 `stream_audio.js`: 在音频播放 loop 中逐帧消费 visemes 数组

**预估行数**: ~200 行前端改动

### B.3.7 文件清单

| # | 平台 | 文件 | 改动类型 | 行数 |
|---|------|------|---------|------|
| 1 | Android | `VisemeAnalyzer.kt` | **新增** | ~300 |
| 2 | Android | `VisemeTimeline.kt` | **新增** | ~150 |
| 3 | Android | `TtsProvider.kt` | 修改 | ~30 |
| 4 | Android | `VoiceIoController.kt` | 修改 | ~150 |
| 5 | Android | `LipSyncMath.kt` | 保留 | 不变（降级路径） |
| 6 | PC | 前端 React 文件 | 修改 | ~200 |

---

## B.4 L3: 动作联动 级3 详细设计

### B.4.1 目标

TTS 文本位置触发 motion。当 TTS 播报到特定位置时（如"挥手"），对应 Live2D 动作自动播放。

### B.4.2 方案: 两层触发机制

**层1: LLM 语义标签**（与 L1 协议融合）:
LLM 在 `emotion` JSON 块的 `motionLabel` 字段指定动作，在情感表达同时触发动作。

```json
{"expressionMix":{"joy":0.7},"motionLabel":"wave_hand","transitionMs":300}
```

**层2: TTS 文本关键词触发**（降级路径）:
TTS 播报到包含"挥手"/"鼓掌"/"鞠躬"等关键词时自动触发对应 motion。

```kotlin
// MotionTagMatcher (AnimationSystem.kt 新增)
val MOTION_KEYWORDS = mapOf(
    "wave_hand" to listOf("挥手", "拜拜", "再见", "你好"),
    "clap"      to listOf("鼓掌", "拍拍手", "好棒"),
    "bow"       to listOf("鞠躬", "谢谢", "对不起"),
    "nod"       to listOf("嗯嗯", "是的", "没错"),
)
```

### B.4.3 Android: AnimationSystem MotionManager 标签触发

```kotlin
// AnimationSystem.kt MotionManager 新增

class MotionManager(private val live2dModel: PurismModel) {
    
    /** 根据标签触发动作 */
    fun triggerMotionByLabel(label: String, priority: Int = 0): Boolean {
        val motionIndex = motionLabelMap[label] ?: return false
        return startMotion(motionIndex, priority)
    }
    
    /** TTS 文本位置回调 → 检查关键词 → 触发动作 */
    fun onTtsPosition(textPosition: Int, fullText: String) {
        // 在当前位置附近查找关键词
        val context = fullText.substring(maxOf(0, textPosition - 5), 
                                         minOf(fullText.length, textPosition + 5))
        for ((label, keywords) in MOTION_KEYWORDS) {
            if (keywords.any { it in context }) {
                triggerMotionByLabel(label)
                break
            }
        }
    }
}
```

**预估行数**: ~150 行修改

### B.4.4 Android: TtsProvider 播放进度回调

```kotlin
// TtsProvider.kt 扩展 (可选，不强制所有 Provider 实现)

interface TtsProvider {
    // ... 现有方法不变 ...
    
    /**
     * 播放进度回调 (级3 扩展)。
     * 可选实现: 仅支持粒度足够的 Provider (Edge TTS/sherpa-onnx)。
     * 系统 TTS 不支持 → 返回 null。
     */
    val onPlaybackProgress: Flow<PlaybackProgress>? get() = null
}

data class PlaybackProgress(
    val textPosition: Int,   // 当前播报到文本的第几个字符
    val totalLength: Int,    // 文本总长
)
```

### B.4.5 PC 端动作联动

**方案**: TTS 字级时间戳已在 `stream_audio.py` 中可用（viseme 时间戳 ≡ 字级时间对齐），只需在合适的 timestamp 注入 motion 触发事件到 WebSocket。

**改动**: `agent/transformers.py` 或 `utils/stream_audio.py` 约 ~300 行

### B.4.6 文件清单

| # | 平台 | 文件 | 改动类型 | 行数 |
|---|------|------|---------|------|
| 1 | Android | `AnimationSystem.kt` | 修改 | ~150 |
| 2 | Android | `TtsProvider.kt` | 修改 | ~20 |
| 3 | Android | 各 TtsProvider 实现 | 修改 | ~80 (EdgeTtsProvider + SherpaOnnxTtsProvider 新增进度回调) |
| 4 | PC | `agent/transformers.py` | 修改 | ~80 |
| 5 | PC | `utils/stream_audio.py` | 修改 | ~120 |
| 6 | PC | 前端 motion 消费 | 修改 | ~100 |

---

# Part C: UI/UX 整改

## C.1 现状问题

| 优先级 | 问题 | 说明 |
|--------|------|------|
| **P0** | Live2D 与聊天分层 | 猫娘完全被气泡遮挡，"桌宠"名存实亡 |
| **P0** | 思考中/说话中状态 | 无"AI思考中"/"正在说话"指示 |
| **P0** | 表情硬切无过渡 | 表情突变，嘴型随机不等同语音 |
| P1 | Idle 单循环 | 动作重复机械 |
| P1 | 首次引导纯文字 | 弹窗生硬 |
| P1 | TopAppBar 无角色名 | 只显示 "Live2D-Ai" |
| P2 | 输入栏占满 | 无快捷表情/话题建议 |
| P2 | 设置页开发者面板 | API Key 堆砌 |

**注意**: P0 表情过渡已在 Part B L1 (AnimationSystem 过渡动画) 覆盖。本节仅覆盖 UI 层面的视觉反馈。

## C.2 Live2D 与聊天分层设计

### C.2.1 视觉布局

```
┌──────────────────────────────────────┐
│  TopAppBar: "小喵 (Xiao Miao)"  [⚙] │
├──────────────────────────────────────┤
│                                      │
│                                      │
│          ┌──────────────┐           │
│          │              │           │
│          │   Live2D     │  ← 猫娘始终可见
│          │   模型       │     半透明叠加
│          │              │     在聊天上方
│          │  (状态指示器) │
│          └──────────────┘           │
│                                      │
│  ┌────────────────────────────────┐  │
│  │ 💬 聊天气泡 (半透明背景)       │  │  ← 气泡在下层
│  │                                │  │     半透明不遮挡
│  │  [用户] 你好喵~               │  │
│  │  [小喵] 主人好呀喵~！=^･ω･^=  │  │
│  │  ... (滚动)                   │  │
│  └────────────────────────────────┘  │
├──────────────────────────────────────┤
│  [😊] [📎] 输入...         [🎤] [➤] │
└──────────────────────────────────────┘
```

**实现方案**: 
- `Box` 布局: Live2D (`AndroidView`) + 状态指示器 (顶层) + 聊天气泡 LazyColumn (底层，半透明)
- 气泡背景 `alpha = 0.75f`，选中/最新气泡 `alpha = 0.9f`
- 气泡最大高度限制为屏幕的 35%

## C.3 状态指示器设计

### C.3.1 三种状态

| 状态 | 触发条件 | 视觉 |
|------|---------|------|
| **思考中** | `isLoading = true` (LLM 生成中) | 猫娘头顶三个跳动点 `...` 动画 + 气泡内 "小喵正在思考..." 占位 |
| **说话中** | `isSpeaking = true` (TTS 播报中) | 猫娘嘴部高亮 + 当前播报气泡边框高亮 (主题色) |
| **空闲** | 两者皆 false | 无特殊指示，仅 idle 动画 |

### C.3.2 实现

```kotlin
// StatusIndicator.kt (新增)

@Composable
fun StatusIndicator(
    isLoading: Boolean,
    isSpeaking: Boolean,
    modifier: Modifier = Modifier,
) {
    when {
        isLoading -> ThinkingIndicator(modifier)  // 三个点跳动动画
        isSpeaking -> SpeakingIndicator(modifier)  // 嘴型波形动画 (背景)
        else -> { /* 不渲染 */ }
    }
}

@Composable
fun ThinkingIndicator(modifier: Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.Center) {
        repeat(3) { index ->
            Text(".", modifier = Modifier.alpha(
                animateFloatAsState(if (dotVisible) 1f else 0.2f).value
            ))
        }
    }
}
```

## C.4 表情过渡动画方案

已由 Part B L1 `AnimationSystem.ExpressionManager` 覆盖。UI 层面补充:
- 表情切换时猫娘周围短暂光晕（0.5s 扩散消失）
- 表情标签在气泡内小字提示（可选关闭）

## C.5 其他 UX 整改项

| 项 | 说明 | 预估行数 |
|----|------|---------|
| **Idle 多样化** | AnimationSystem 随机选择 3-5 个子动作，带不重复抑制 | ~80行 |
| **首次引导** | WelcomeGuide.kt: 猫娘角色图 + 自我介绍 + 3 步引导（替代纯文字弹窗） | ~120行 |
| **TopAppBar 角色名** | 从 `PersonaConfig.getName()` 读取，显示"小喵 (Xiao Miao)" | ~10行 |
| **输入栏优化** | 快捷表情按钮 (😊😢😡) + 话题建议 chips | ~80行 |

---

# Part D: 实施顺序与依赖关系

## D.1 Wave 分组

```
Wave 0 (P0, 地基 — 第1周):
  ├─ task-phase1-mainchat-refactor    Part A: MainChatScreen 重写
  │     MainChatViewModel + MainChatScreen + 3 Dialog + PluginBootstrap
  │
  └─ task-phase1-emotion-protocol     Part B L1: 统一结构化情感协议
        shared/emotion-protocol.md + EmotionController parseExpressionMix
        (先建协议，再双端实现)

Wave 1 (P0, 级3核心 — 第2周):
  ├─ task-phase1-expression-mix       Part B L1: AnimationSystem 表情混合
  │     Android AnimationSystem 级3 + PC live2d_model.py 级3
  │
  ├─ task-phase1-viseme-android       Part B L2: Android VisemeAnalyzer
  │     VisemeAnalyzer + VisemeTimeline + VoiceIoController 改造
  │
  └─ task-phase1-ux-layout           Part C: UI/UX 整改 (P0项)
        Live2D/聊天分层 + 状态指示器 + 表情过渡 UI

Wave 2 (P1, 增强 — 第3周):
  ├─ task-phase1-viseme-pc           Part B L2: PC 前端消费 visemes
  │     React 前端 + WebSocket visemes 接线
  │
  ├─ task-phase1-motion-linkage       Part B L3: 动作联动
  │     双端 TTS 位置触发 motion
  │
  └─ task-phase1-ux-polish           Part C: UI/UX 整改 (P1/P2项)
        Idle 多样化 + 首次引导 + TopAppBar + 输入栏优化
```

## D.2 依赖图

```
Wave 0:
  MainChatRefactor ──────────────────────────── (独立，仅依赖现有模块不变)
  EmotionProtocol ───────────────────────────── (独立，纯粹文档 + 单文件)

Wave 1:
  ExpressionMix ─── 依赖 EmotionProtocol ────── (先有协议, 后改 AnimationSystem)
  VisemeAndroid ─── 独立 ───────────────────── (不依赖其他 Wave 1 任务)
  UxLayout ──────── 依赖 MainChatRefactor ──── (UI 改在重写后的代码上)

Wave 2:
  VisemePC ──────── 独立 ───────────────────── (纯 PC 端改动)
  MotionLinkage ─── 依赖 ExpressionMix ─────── (motionLabel 字段在 emotion JSON 中)
  UxPolish ──────── 独立 ───────────────────── (增量 UI 改进)
```

## D.3 文件改动汇总

| Wave | 新增文件 | 修改文件 | 不动文件 | 预估总行数 |
|------|---------|---------|---------|-----------|
| Wave 0 | 6 | 2 | 7+ | ~920 |
| Wave 1 | 2 | 5 | 6+ | ~970 |
| Wave 2 | 0 | 8 | 8+ | ~830 |
| **合计** | **8** | **15** | **~30** | **~2,720** |

---

# Part E: 风险点与验收标准

## E.1 技术风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| **Android FFT 元音分类精度不足** | 中 | 中 | 降级路径已定义（analyzeSimple → LipSyncMath 随机波动）；参考 PC 端 pymouth_viseme.py 已验证方案 |
| **MainChatViewModel 生命周期** | 低 | 高 | ViewModel 使用 `viewModelScope`；Compose 使用 `collectAsStateWithLifecycle()` |
| **AnimationSystem 表情混合破坏现有渲染** | 中 | 高 | ExpressionManager 在 `update()` 中覆盖参数，不改变 MOC3 基础参数；回退: 注释 expressionMix 调用即可 |
| **前端 visemes 消费破坏 React bundle** | 低 | 高 | 预编译 React 不能改源码 → 需 fork 前端或改用 CDN 动态导入；评估是否值得 |
| **LLM 不遵守 JSON emotion 格式** | 中 | 低 | 降级到关键词匹配已内置；prompt 示例充分降低概率 |

## E.2 验收标准

### Part A 验收

| # | 标准 | 验证方式 |
|---|------|---------|
| A1 | MainChatScreen 重写后功能不退化（发送/接收/语音/插件/历史/设置） | 手工 E2E: 文本来回 5 轮 + 语音识别 + 插件触发 |
| A2 | MainChatViewModel 行数 ≤ 300 | `wc -l` |
| A3 | MainChatScreen Composable 行数 ≤ 250 | `wc -l` |
| A4 | 91 个现有单测全过 | `./gradlew test` |
| A5 | MainActivity.kt 行数 ≤ 250 | `wc -l` |

### Part B 验收

| # | 标准 | 验证方式 |
|---|------|---------|
| B1 | LLM 输出结构化 emotion JSON → 正确解析 expressionMix → Live2D 表情混合可见 | 输入 "[joy] 喵喵!" → 期望 LLM 输出 `{"expressionMix":{"joy":0.8}}` → 猫娘露出笑脸（渐进过渡） |
| B2 | JSON 格式错误时 → 降级到关键词匹配 → 功能不中断 | 手工注入畸形 JSON → 验证回退到 `[joy]` 关键词 |
| B3 | TTS 播报时嘴型按元音 (a/i/u/e/o) 变化，非随机 | 录音对比: 播报含 "啊" 句 vs "衣" 句 → 嘴型明显不同 |
| B4 | TTS Provider 不提供 PCM 时 → 降级到 analyzeSimple → 再降级到 LipSyncMath | 系统 TTS 播报 → 嘴型随机波动（不崩溃） |
| B5 | emotion JSON 含 motionLabel 时 → 对应动作在 TTS 播报期间触发 | 输入触发 "挥手" → 手臂动画播放 |
| B6 | PC 端 visemes 被前端消费 → 口型与音频同步 | 录制 PC 端屏幕 → 逐帧对比嘴型与音频 |

### Part C 验收

| # | 标准 | 验证方式 |
|---|------|---------|
| C1 | Live2D 猫娘始终可见（气泡半透明不遮挡） | 截图: 气泡底层可见猫娘轮廓 |
| C2 | LLM 思考时显示 "..." 动画 | 发送消息 → 等待 → 看到三个点跳动 |
| C3 | TTS 说话时当前气泡边框高亮 | 播报期间截图 → 高亮气泡明显 |
| C4 | 表情切换平滑过渡（不再硬切） | 慢动作录屏 → 表情在 300ms 内渐变 |

## E.3 回退策略

| 变更 | 回退方式 |
|------|---------|
| MainChatScreen 重写 | 保留原 MainActivity.kt 为 `MainActivity.kt.bak`（Git 历史可恢复） |
| EmotionController JSON 解析 | 关键词匹配逻辑保留不删，JSON 解析失败自动降级 |
| VisemeAnalyzer | 通过特性开关控制（`SettingsRepository.visemeEnabled`），关闭后走 LipSyncMath |
| AnimationSystem 表情混合 | `currentMix` 置空时回退到单表情模式 |
| PC 前端 visemes 消费 | `visemes` 字段为空时前端无操作（与现状一致），零破坏 |

---

# Part F: 与现有文档的一致性说明

本规划与以下现有文档保持一致或显式引用:

| 文档 | 关系 |
|------|------|
| `docs/architecture/ARCHITECTURE.md` | V1 架构基线，本规划在此基础上增量 |
| `docs/plans/Phase-0-notes-level3.md` | 级3 差距矩阵，本规划 Part B 据此设计 |
| `docs/plans/ux-overhaul-plan.md` | UX 对标方案，本规划 Part C 降级落地（Wave 1 只做 P0） |
| `docs/research/NeuroSama-architecture-research.md` | 结构化情感输出、流式+分句、VTube Studio 口型思路，已融入 Part B |
| `PLAN.md` | V1 架构规划，本规划为 Phase 1 增量 |
| `docs/plans/plan-v12-plan.md` | V1.2 语音重塑，TtsProvider 接口已落地，本规划在此基础上扩展级3能力 |
| `shared/persona.yaml` | 人设单源真理，LLM prompt 升级不改变人格设定，只改表情输出格式 |

---

# Part G: 教训候选

- [架构设计] 教训：God Class 重构不能逐块"摘出"——当 5+ 个共享对象贯穿所有代码块时，逐块提取的边际成本超过全量重写。标准：共享对象 ≥ 4 且行数 ≥ 400 的 Composable 函数应重写而非重构。建议：MainChatScreen 重写后，在 Compose 开发规范中加入"Composable 函数最大 200 行 + 最多 3 个外部依赖（通过参数传入）"的 lint 规则。

- [级3实现] 教训：级3 方案的"最好+降级"策略是防回归的关键——不应把旧代码删除换新代码，而是新增级3路径后保留级2/级1代码作为 fallback。标准：每个级3 模块必须至少有 2 层降级（级3 → 级2 → 级1），且降级触发条件可测试。建议：EmotionController.parseAndApply 的三个分支（JSON → 关键词 → neutral）在单测中分别覆盖。

- [跨平台协议] 教训：双端共享的结构化协议（emotion JSON schema）必须在代码改动前先写成文档——否则 PC/Android 各自理解产生语义漂移。标准：任何跨平台协议必须有独立 markdown 文档 + JSON schema，且在双端实现前经过 Human 审阅。建议：`shared/emotion-protocol.md` 作为单源真理，双端实现时引用此文档的 commit hash。
