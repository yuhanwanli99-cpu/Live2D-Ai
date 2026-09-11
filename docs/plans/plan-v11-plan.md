# Live2D-Ai V1.1「核心补齐」架构设计文档

> 版本: v1.0 — 2026-08-05
> 架构师: software-architect (SPOQ Phase: architecting)
> 范围: Android 端 4 模块补齐（语音闭环接线 / 聊天历史持久化 / 长期记忆集成 / LipSync）

---

## 1. 需求概述

Live2D-Ai V1 文本→LLM→[emotion]→Live2D 核心链路已双端验收通过（Android 真机 + PC :12393 PASS）。
V1.1 补齐 4 个关键能力闭环：

| 模块 | 当前状态 | V1.1 目标 |
|---|---|---|
| 语音闭环 (TTS+ASR) | EdgeTtsService/VoiceInputService 存在但断链 | 接线→用户可语音输入/猫娘播报回复 |
| 聊天历史持久化 | displayMessages 纯内存态 | 重启恢复 + 清空对话 UI |
| 长期记忆 | 插件已实现未集成 | 集成 room 记忆插件 + 注入对话上下文 |
| LipSync | Live2D 参数接口已就绪 | TTS 播放驱动嘴型开合 |

**核心原则**: 优先复用现有代码（EdgeTtsService/VoiceInputService/PluginManager/memory plugin）；每模块 ≥3 候选方案对比。

---

## 2. 系统架构

### 2.1 整体数据流（V1.1 完整链路）

```
用户语音输入 ──→ VoiceInputService.listen()
    │
    ▼
文本输入框 ──→ ChatService.sendMessage(text)
    │               │
    │  beforeSend   │  hooks: memory recall (注入相关记忆), vision describe
    │    hooks      ▼
    │           LLM API (SSE 流式)
    │               │
    │    afterReceive hooks (emotion 已剥离)
    │               │
    ├──→ EmotionController.setEmotion() → Live2DView.setExpression()
    ├──→ EdgeTtsService.speak(strippedText) → [LipSync 驱动嘴型开合]
    └──→ MemoryRecallHook.afterReceive (周期性摘要写入 Room)
```

### 2.2 模块间依赖关系

```
                    ┌──────────────────┐
                    │  模块2: 聊天持久化 │ (纯数据层，无上游依赖)
                    └────────┬─────────┘
                             │ displayMessages 序列化/反序列化
                             │ 提供 ChatHistoryStore 供模块3 参考
                             ▼
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│ 模块1: 语音接线│◄──┤ ChatService 核心 │──►│ 模块3: 长期记忆  │
│ TTS + ASR    │   │ (Hook 调度器)    │   │ (memory plugin)  │
│              │   └──────────────────┘   │                  │
│ TTS→LipSync  │            │             │ 复用: Room/SQLite │
│   驱动       │            │             │ ChatHook 接口     │
└──────┬───────┘            │             └──────────────────┘
       │                    │
       ▼                    ▼
┌──────────────────────────────────────────┐
│           模块4: LipSync                  │
│  TTS 播放状态 → Live2DView.setLipSyncParam │
│  (TTS 播放期间周期性更新 mouth open 参数)   │
└──────────────────────────────────────────┘
```

**集成顺序**: 模块2(持久化) → 模块1(语音接线) → 模块4(LipSync) → 模块3(记忆集成)
> 原因: 持久化是数据基础设施；语音接线是交互入口；LipSync 依赖 TTS 接线完成；记忆集成是锦上添花且大部分已完成。

---

## 3. 模块划分

### 模块 1: Android 语音闭环接线 (TTS + ASR)

#### 3.1.1 职责

将已存在的 `EdgeTtsService.speak()` 和 `VoiceInputService.listen()` 接入聊天主循环。

#### 3.1.2 关键接线点

**TTS 接线**:
- **触发时机**: `ChatService.runAfterReceive()` 内部——该处已解析 emotion、已剥离标签（`EmotionController.stripEmotionTags`），拿到 `strippedText`
- **播放内容**: `strippedText`（纯文本，无 `[emotion]` 标签）
- **中断**: 新消息到来时调用 `EdgeTtsService.stop()`（停止 Edge TTS WebSocket 合成 + MediaPlayer 播放 + Android TTS 播放）

**ASR 接线**:
- **触发**: 输入栏右侧「麦克风按钮」
- **权限**: `RECORD_AUDIO` 运行时权限（按需申请，不过度）
- **识别结果**: 填入输入框（`inputText = recognized`），用户可编辑后发送；或长按直接发送
- **VAD（可选短期）**: 使用 Android SpeechRecognizer 自带的自动静音检测（`EXTRA_SPEECH_INPUT_COMPLETE_SILENCE_LENGTH_MILLIS`）

#### 3.1.3 数据流

```
ASR 路径:
  麦克风按钮 onClick → 检查 RECORD_AUDIO 权限
    ├─ 未授权 → requestPermission
    └─ 已授权 → VoiceInputService.listen()
                    │ (suspendCancellableCoroutine)
                    ▼
              recognizedText → inputText = it (或直接发送)

TTS 路径:
  ChatService.runAfterReceive(strippedText, emotion, chatContext)
    │
    ├─ EmotionController.setEmotion(emotion)  // 表情切换
    ├─ EdgeTtsService.speak(strippedText)     // TTS 播报（同一协程点）
    └─ MemoryRecallHook.afterReceive(...)      // 长期记忆写入
```

#### 3.1.4 新增/改动文件

| 文件 | 改动 | 说明 |
|---|---|---|
| `MainActivity.kt` | 修改 | 添加 EdgeTtsService/VoiceInputService 实例化；输入栏添加麦克风按钮；TTS 停播接线 |
| `ChatService.kt` | 修改（可选） | 暴露 `onAfterReceive` 回调或直接在 MainActivity 中使用 hook 方式接入 TTS |
| `EdgeTtsService.kt` | 无需改动 | 现有 speak/stop 接口已完备 |
| `VoiceInputService.kt` | 无需改动 | 现有 listen/stopListening 接口已完备 |
| `AndroidManifest.xml` | 修改 | 确认 RECORD_AUDIO 权限声明存在 |

#### 3.1.5 候选对比: TTS 引擎

| 方案 | 许可 | APK 体积增量 | 离线 | 中文自然度 | 延迟 | 依赖复杂度 | 推荐 |
|---|---|---|---|---|---|---|---|
| **Edge TTS (已有)** | 免费 (微软公共端点) | 0 (无本地模型) | ❌ 需网络 | ★★★★★ (XiaoxiaoNeural) | ~1-3s (网络) | 低 (OkHttp WebSocket) | ✅ **V1.1 推荐** |
| sherpa-onnx TTS | Apache 2.0 | +80-150MB (vits-melo-tts 中文模型) | ✅ 完全离线 | ★★★★ (vits-melo-tts-zh) | ~0.5-1s (本地推理) | 高 (需 JNI 编译 .so 文件) | 🔶 V1.2 离线增强 |
| MeloTTS (MyShell) | MIT | +50-100MB (PyTorch 依赖，不适合 Android 直接集成) | ✅ | ★★★☆ (中文女声) | ~1-3s | 极高 (Python 依赖，无法直接用于 Android) | ❌ 不适合移动端 |
| Android 系统 TTS | 系统内置 | 0 | ✅ | ★★☆☆ (取决于厂商 ROM) | ~0.5-1s | 低 (Android SDK) | ❌ 已作为 EdgeTTS 降级链第二层 |
| Kokoro TTS | MIT | +300MB+ (ONNX 模型) | ✅ | ★★★★★ | ~1-2s | 高 | 🔶 V1.2 候选 |

**结论**: **Edge TTS 零成本零体积，中文自然度顶级，V1.1 直接用**。sherpa-onnx 或 Kokoro 作为 V1.2 离线增强候选（APK 体积增量需用户显式下载模型）。

#### 3.1.6 候选对比: ASR 引擎

| 方案 | 许可 | APK 体积增量 | 离线 | 中文准确率 | 延迟 | 依赖复杂度 | 推荐 |
|---|---|---|---|---|---|---|---|
| **Android SpeechRecognizer (已有)** | 系统内置 | 0 | 🔶 部分离线 (Gboard 提供离线包) | ★★★☆ (依赖系统服务) | ~0.5-2s | 低 (Android SDK) | ✅ **V1.1 推荐** |
| Vosk (alphacep) | Apache 2.0 | +50MB (vosk-model-small-cn) | ✅ 完全离线 | ★★★☆ | ~0.5-1s | 中 (需 JNI .so + 模型文件) | 🔶 V1.2 离线增强 |
| whisper.cpp | MIT | +70-600MB (tiny→large) | ✅ | ★★★★ (small 及以上) | ~1-5s (取决于模型大小) | 高 (需编译 .so + 模型文件) | 🔶 V1.2 候选 |
| sherpa-onnx ASR | Apache 2.0 | +50-150MB (zipformer 模型) | ✅ | ★★★★★ | ~0.5-2s | 中高 (JNI .so + 模型) | 🔶 V1.2 候选 |

**结论**: **Android SpeechRecognizer 零成本零体积，V1.1 直接用**。离线 ASR 留作 V1.2（与 PC 端 faster-whisper 统一技术栈可考虑 whisper.cpp 或 sherpa-onnx）。

---

### 模块 2: 聊天历史持久化 + 清空对话 UI

#### 3.2.1 职责

- `displayMessages` 持久化到本地（重启恢复，规模限制最近 200 条）
- 清空对话: 调用 `ChatService.resetConversation()` + 清 `displayMessages` + 清持久化
- 恢复时同时恢复 `ChatService.messages`（通知 LLM 上下文连续性）

#### 3.2.2 设计决策

**持久化内容**: 只持久化 `DisplayMessage`（id, content, isUser, isError, showSettingsButton），不持久化 `ChatService.messages`（`ChatService` 的 messages 包含 system prompt 和 tool messages，恢复复杂度高且重启后重新注入 system prompt 即可）

> **安全考量**: 聊天历史可能包含敏感的个人对话内容。由于 Live2D-Ai V1.1 无账号系统、无后端服务器，所有数据仅存储在用户设备本地。后续版本如需导出/同步功能，需额外设计加密方案。

**存储位置**: `context.filesDir / "chat_history.json"`（与 memory plugin 的 Room 数据库 `live2dai_memory.db` 同目录）

**规模限制**: write 前 trim 到最近 200 条（`takeLast(200)`）

#### 3.2.3 数据流

```
保存:
  displayMessages 变化 (LaunchedEffect / snapshotFlow)
    → ChatHistoryStore.save(displayMessages.takeLast(200))
    → JSON 序列化 → filesDir/chat_history.json

恢复 (冷启动):
  MainChatScreen LaunchedEffect
    → ChatHistoryStore.load()
    → JSON 反序列化 → displayMessages.addAll(messages)
    → 滚动到最底部

清空:
  清空按钮 onClick
    → chatService.resetConversation()
    → displayMessages.clear()
    → ChatHistoryStore.clear()
    → 删除 chat_history.json
```

#### 3.2.4 新增/改动文件

| 文件 | 改动 | 说明 |
|---|---|---|
| **`ChatHistoryStore.kt`** | **新增** | 持久化工具类（JSON 文件读写） |
| `MainActivity.kt` | 修改 | 添加存储实例化、恢复逻辑、清空按钮、清空确认对话框 |
| `DisplayMessage` (MainActivity.kt) | 修改 | 添加 `kotlinx.serialization.Serializable` 注解 |

#### 3.2.5 候选对比: 持久化方案

| 方案 | 许可 | 体积/成本 | 离线 | 查询能力 | 复杂度 | 推荐 |
|---|---|---|---|---|---|---|
| **JSON 文件 (已有方案)** | — | 0 | ✅ | 无（全量读写） | 低 (kotlinx.serialization + File IO) | ✅ **V1.1 推荐** |
| SharedPreferences + JSON | — | 0 | ✅ | 无（全量读写） | 低 | 🔶 备选 |
| Jetpack DataStore (Preferences) | Apache 2.0 | +0.5MB (库) | ✅ | 无（全量读写） | 中 (迁移到 Proto/Preferences DataStore) | ❌ 过度设计 (200条消息场景) |
| Room / SQLite | Apache 2.0 | +2MB (库，已有 room 依赖) | ✅ | ✅ (分页/搜索) | 高 | ❌ 过度设计 (200条消息不需要 SQL) |
| 纯内存 (现状) | — | 0 | ❌ | 无 | 极低 | ❌ 重启即丢失 |

**结论**: **JSON 文件方案最简单、零依赖、200 条消息全量读写性能开销可忽略**。kotlinx.serialization 已在项目中作为依赖存在（ChatService 用 `kotlinx.serialization.json.Json`）。

---

### 模块 3: 长期记忆接口 + 最小实现

#### 3.3.1 现状盘点（重要！）

`live2d-ai-plugin-memory` 模块**已实现但未集成**。已实现的组件：

| 组件 | 文件 | 说明 |
|---|---|---|
| MemoryPlugin | `MemoryPlugin.kt` | 插件入口，onLoad 注册 MemoryRecallHook |
| MemoryRecallHook | `MemoryRecallHook.kt` | ChatHook: beforeSend 关键词检索注入，afterReceive 每 N 轮 LLM 摘要写入 |
| MemoryFact | `MemoryFact.kt` | Room Entity: content, keywords, createdAt, lastUsedAt |
| MemoryDao | `MemoryDao.kt` | Room DAO: insert/getAll/update |
| MemoryDatabase | `MemoryDatabase.kt` | Room 数据库 (live2dai_memory.db) |
| MemoryKeywordMatcher | `MemoryKeywordMatcher.kt` | 中文关键词提取 + 重叠打分检索 |
| SummaryLlmClient | `SummaryLlmClient.kt` | 函数式接口: summarize(recentTurns, personaName) → String? |
| MemoryPluginConfig | `MemoryPluginConfig.kt` | 构造参数: context, summaryLlmClient, triggerEveryNTurns |

**缺失**: 
1. `SummaryLlmClient` 的**具体实现**（调用 LLM API 生成事实摘要）
2. MainActivity 中**未加载 MemoryPlugin**（只加载了 VisionPlugin）
3. 记忆去重/冲突更新逻辑（现有 insert 前只检查 content 全等）

#### 3.3.2 V1.1 目标

- **集成现有 memory plugin**: MainActivity 中添加 `MemoryPlugin` 到插件列表
- **实现 SummaryLlmClient**: 复用 `LLMProvider` 做摘要生成（小 prompt → 精简回复）
- **微调 ChatHook 接口**: 无需改动（现有 `beforeSend`/`afterReceive` 已完备）
- **记忆上限**: 最多 500 条（`MemoryDao` 新增 `deleteOldest` 方法）

#### 3.3.3 记忆注入数据流

```
beforeSend:
  用户输入 "我记得我告诉过你我喜欢咖啡"
    → MemoryRecallHook.beforeSend()
       ├─ MemoryKeywordMatcher.rankTop(allFacts, "我记得我告诉过你我喜欢咖啡")
       │   → 检索到: ["用户喜欢喝咖啡 (2026-08-01)"]
       └─ 返回 "[系统: 相关记忆]\n- 用户喜欢喝咖啡"
    → ChatService 将附加上下文拼接进本轮请求
    → LLM 看到记忆信息并回复

afterReceive (每 10 轮):
  ChatService.runAfterReceive → MemoryRecallHook.afterReceive
    └─ turnCounter % 10 == 0:
       ├─ SummaryLlmClient.summarize(recentTurns, personaName)
       │   → 调用 LLM: "从以下对话中提取关键事实... 用户喜欢喝咖啡..."
       │   → 返回 "用户喜欢喝咖啡"
       ├─ MemoryKeywordMatcher.extractKeywords → "咖啡,喜欢,用户"
       ├─ 去重检查 → 不存在 → dao.insert(...)
       └─ 下次对话注入
```

#### 3.3.4 新增/改动文件

| 文件 | 改动 | 说明 |
|---|---|---|
| `MainActivity.kt` | 修改 | 添加 MemoryPlugin 到 pluginManager.loadPlugins(...) |
| **`LlmSummaryClient.kt`** | **新增** (core) | SummaryLlmClient 实现：复用 LLMProvider API 做事实提取 |
| `MemoryDao.kt` | 修改 | 新增 `deleteOldest(keepCount: Int)` 实现记忆上限 |
| `MemoryRecallHook.kt` | 修改（可选） | afterReceive 写入后 trim 到上限 |
| `plugin-sdk/ChatHook.kt` | **无需改动** | 现有接口已完备 |

#### 3.3.5 候选对比: 记忆架构

| 方案 | 许可 | 存储 | 检索 | 复杂度 | V1.1 适用性 | 推荐 |
|---|---|---|---|---|---|---|
| **Room + 关键词匹配 (当前)** | Apache 2.0 | SQLite | 中文关键词重叠打分 | 低 | ✅ 已实现 | ✅ **V1.1** |
| 简单 key-value JSON | — | JSON 文件 | 全量注入 | 极低 | ✅ 即插即用 | 🔶 过渡方案 (更简单但无检索) |
| 向量检索 (chromadb/sqlite-vec) | MIT/自定义 | 向量数据库 | embedding 相似度 | 极高 (需引入 embedding 模型) | ❌ 过度设计 | 🔶 V1.2+ |
| Miru 可验证 Markdown 记忆 | Apache 2.0 | Markdown 文件 | 全文检索 | 高 | ❌ 架构差异大 | ❌ |
| Soul of Waifu 四层认知 | GPLv3 | SQLite + 多层 | 语义/情景/关系/日记 | 极高 | ❌ 过度设计 | ❌ |

**结论**: **当前 Room + 关键词匹配方案已是最优 MVP 选择**。简单 key-value JSON 可作为降级备选。向量检索留作 V1.2+（需 embedding 模型 + 向量存储，目前 APK 体积/推理成本过高）。

#### 3.3.6 记忆召回注入方式: 全量 vs 相关性

**V1.1 选择**: **相关性检索**（已在 MemoryKeywordMatcher.rankTop 实现：提取用户消息关键词 → 按重叠打分排序 → 取 top-3 注入）

理由: 全量注入在记忆条目超过 50 条后会导致 token 浪费（每条 100 chars × 50 = 5000 chars ≈ 1250 tokens），且大量无关记忆会混淆 LLM。相关性检索保证每次只注入 3 条最相关记忆（~300 chars ≈ 75 tokens），不影响对话质量。

---

### 模块 4: LipSync（TTS 播放驱动嘴型）

#### 4.1.1 职责

TTS 播放期间驱动 Live2D 嘴型开合参数（`Live2DView.setLipSyncParam(value: Float)`，0.0=闭嘴, 1.0=最大张口），实现口型同步。

#### 4.1.2 已有基础设施

```
Live2DView.setLipSyncParam(value: Float)
  → Live2DRenderer.setLipSyncParam(value: Float)
    → AnimationPipeline.setLipSyncValue(value)
      → 写入匹配的参数 (来自 ModelMetaConfig.semantic.mouthOpenIdx)
    OR (fallback)
      → Live2DNative.setParameterValue(modelPtr, paramAIndex, value)
```

`ModelMetaConfig` 已解析 model3.json Groups[LipSync] 显式声明的嘴型参数。

#### 4.1.3 候选方案对比

| 方案 | 原理 | 实现复杂度 | 口型准确度 | 额外依赖 | 推荐 |
|---|---|---|---|---|---|
| **A: 播放状态驱动** | TTS 开始→mouth=0.8, TTS结束→mouth=0.0 (简单二元) | ★☆☆☆☆ 极低 | ★☆☆☆☆ | 无 | ✅ **V1.1 推荐** |
| B: 音频振幅驱动 | 实时采样 MediaPlayer 音频 RMS → 映射到 0.0-1.0 | ★★★☆☆ | ★★★☆☆ | AudioTrack/Visualizer API | 🔶 V1.2+ |
| C: 基于文本分割 | 按标点/字符分割文本，每个片段驱动 mouth 短暂张开 | ★★☆☆☆ | ★★☆☆☆ | 无（纯文本处理） | 🔶 备选（1 天可接） |
| D: Edge TTS word-boundary 事件 | Edge TTS 返回 word-boundary 时间戳 → 精准映射 | ★★★★☆ | ★★★★★ | Edge TTS SSML wordBoundaryEnabled | 🔶 V1.2+ |

#### 4.1.4 V1.1 方案: 播放状态驱动 (方案 A)

```
EdgeTtsService.speak(strippedText)
  │
  ├─ 播放开始: live2DView.setLipSyncParam(0.8f)
  │     └─ coroutineScope.launch { 
  │           while (mediaPlayer?.isPlaying == true) {
  │             delay(100ms)
  │             // 简单正弦波模拟嘴型变化 (0.4~0.9)
  │             val value = 0.65f + 0.25f * sin(frame * 0.5f)
  │             live2DView.setLipSyncParam(value)
  │             frame++
  │           }
  │           live2DView.setLipSyncParam(0.0f)  // 闭嘴
  │       }
  │
  └─ 播放完成/中断: live2DView.setLipSyncParam(0.0f)
```

#### 4.1.5 升级路径 (V1.2+)

- **音频振幅驱动**: 使用 `AudioTrack` 或 `Visualizer` API 实时获取音频帧振幅 → RMS 归一化 → `setLipSyncParam`
- **Edge TTS word-boundary**: 启用 SSML `wordBoundaryEnabled: true`，Edge TTS 返回每词时间戳 → 精确嘴型时序

#### 4.1.6 新增/改动文件

| 文件 | 改动 | 说明 |
|---|---|---|
| `MainActivity.kt` | 修改 | TTS speak 调用处添加 LipSync 协程控制 |
| `EdgeTtsService.kt` | 修改（可选） | 暴露播放状态回调 `onPlayingStateChanged: (Boolean) -> Unit` |

---

## 4. 数据模型

### 4.1 聊天历史条目 (JSON Schema)

```json
{
  "version": 1,
  "messages": [
    {
      "id": "uuid-string",
      "content": "消息内容",
      "isUser": true,
      "isError": false,
      "showSettingsButton": false
    }
  ]
}
```

### 4.2 记忆事实 (Room Entity — 已有，无变更)

```kotlin
@Entity(tableName = "memory_facts")
data class MemoryFact(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val content: String,       // "用户喜欢喝咖啡"
    val keywords: String,      // "咖啡,喜欢,用户"
    val createdAt: Long,       // epoch millis
    val lastUsedAt: Long       // epoch millis
)
```

### 4.3 LipSync 参数模型

```kotlin
data class LipSyncState(
    val mouthValue: Float,    // 0.0 (闭嘴) ~ 1.0 (最大张口)
    val isSpeaking: Boolean   // 是否正在播放 TTS
)
```

---

## 5. 接口定义

### 5.1 模块 1: VoiceIoController (新增)

```kotlin
class VoiceIoController(
    private val context: Context,
    private val ttsService: EdgeTtsService,
    private val asrService: VoiceInputService,
    private val live2DView: Live2DView?  // 可选（备用）
) {
    /** TTS 播报纯文本，自动驱动 LipSync */
    suspend fun speak(text: String)

    /** 停止当前 TTS 和 LipSync 动画 */
    fun stopSpeaking()

    /** ASR 开始监听，返回识别文本 */
    suspend fun listen(): String

    /** 释放资源 */
    fun release()
}
```

### 5.2 模块 2: ChatHistoryStore (新增)

```kotlin
object ChatHistoryStore {
    /** 保存消息列表（自动 trim 到 maxMessages 条） */
    fun save(context: Context, messages: List<DisplayMessage>, maxMessages: Int = 200)

    /** 读取消息列表，失败返回空列表 */
    fun load(context: Context): List<DisplayMessage>

    /** 清空聊天历史 */
    fun clear(context: Context)
}
```

### 5.3 模块 3: SummaryLlmClient 实现 (新增)

```kotlin
class LlmSummaryClient(
    private val provider: LLMProvider
) : SummaryLlmClient {
    override suspend fun summarize(recentTurns: List<ChatTurn>, personaName: String): String?
}
```

### 5.4 模块 4: LipSync 驱动 (EdgeTtsService 扩展)

```kotlin
// EdgeTtsService 新增回调
var onPlaybackStateChanged: ((Boolean) -> Unit)?  // true=开始播放, false=播放结束
```

---

## 6. 文件清单（新增/修改）

### 新增文件 (3 个)

| 文件 | 模块 | 说明 |
|---|---|---|
| `ChatHistoryStore.kt` | 模块2 | 聊天历史 JSON 持久化工具类 |
| `VoiceIoController.kt` | 模块1 | TTS+ASR+LipSync 统一协调器 (可选，亦可直接在 MainActivity 中处理) |
| `core/LlmSummaryClient.kt` | 模块3 | SummaryLlmClient 实现 (LLM API 调用) |

### 修改文件 (5 个)

| 文件 | 模块 | 改动 |
|---|---|---|
| `MainActivity.kt` | 1/2/3/4 | 添加 EdgeTtsService/VoiceInputService 实例化、记忆插件加载、聊天历史恢复、清空按钮、麦克风按钮、TTS 接线 + LipSync 协程 |
| `AndroidManifest.xml` | 1 | 确认 RECORD_AUDIO 权限已声明 |
| `MemoryDao.kt` | 3 | 新增 `deleteOldest(keepCount)` 查询 |
| `MemoryRecallHook.kt` | 3 | afterReceive 写入后自动 trim 到上限 |
| `EdgeTtsService.kt` | 4 | (可选) 暴露播放状态回调 |

### 无需改动的已有文件 (保留不变)

- `ChatService.kt` — 现有 hook 机制已完备
- `PluginManager.kt` — loadPlugins 接口不需要改
- `ChatHook.kt` (plugin-sdk) — beforeSend/afterReceive 已完备
- `VoiceInputService.kt` — listen/stopListening 已完备
- `Live2DView.kt` / `Live2DRenderer.kt` — setLipSyncParam 已完备
- `EmotionController.kt` — 表情解析已完备
- `plugin-sdk/*` — 接口契约不变
- `ModelRegistry.kt` / `SettingsRepository.kt` — 不变

---

## 7. 实施顺序

```
Wave A (基础设施) — 模块2: 聊天历史持久化 + 清空 UI
  原因: 纯数据层，无上游依赖；做完后模块3 可参考其持久化路径
  
Wave B (交互闭环) — 模块1: 语音接线 (TTS + ASR)
  原因: 依赖模块2 的 UI 改动（麦克风按钮在同区域）；做完后模块4 可基于 TTS 接线叠加
  
Wave C (视觉增强) — 模块4: LipSync
  原因: 依赖模块1 的 TTS 接线完成；属于锦上添花
  
Wave D (智能增强) — 模块3: 长期记忆集成
  原因: 大部分代码已完成，仅需集成 + SummaryLlmClient 实现；最后做降低回归风险
```

---

## 8. 风险与注意事项

### 8.1 技术风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Edge TTS WebSocket 在中国大陆被墙 | TTS 不可用 | 降级链已有：系统 TTS → 静默；V1.2 可选 sherpa-onnx 离线方案 |
| Android SpeechRecognizer 在某些 ROM 不可用 | ASR 不可用 | 降级为文本输入（voiceInput 返回空字符串时 UI 无变化） |
| MediaPlayer 播放与 LipSync 同步精度低 | 口型与语音不同步 | V1.1 方案A 是二元开关，不同步可接受；V1.2 升级音频振幅方案 |
| 记忆摘要 LLM 调用增加 API 费用 | 用户成本增加 | 每 10 轮才触发一次（~50 tokens prompt + ~30 tokens 摘要）；使用用户已配置的 LLM Provider，无额外 API |

### 8.2 架构约束

1. **TTS 与表情切换必须在同一时机触发**: `ChatService.runAfterReceive()` 内部——确保表情先切换（视觉反馈立即）再 TTS 播报（音频反馈延迟 1-3s）
2. **记忆插件不能影响对话核心链路**: 现有 `try-catch` 容错已到位，SummaryLlmClient 超时/失败静默跳过
3. **RECORD_AUDIO 权限按需申请**: 点击麦克风按钮时才弹出权限请求，不在启动时批量申请
4. **聊天历史不持久化 `ChatService.messages`**: 重启后 `ChatService` 从头开始（system prompt 重新注入），只恢复 UI 显示消息
5. **LipSync 不和 TTS 引擎绑定**: 方案A 只依赖 `isPlaying` 状态，后续切换到离线 TTS 不需改 LipSync 逻辑

### 8.3 回退策略

- **模块1**: TTS/ASR 任何一个失败不影响文本输入（降级回 V1.0 纯文本模式）
- **模块2**: JSON 文件读写失败→空列表启动（等价于 V1.0 行为）；文件损坏→删除重建
- **模块3**: 记忆插件加载失败→静默跳过（对话流程不受影响，`ChatService.hooks` 中缺失 memory hook）
- **模块4**: LipSync 异常→catch 后 mouth 设为 0.0，不影响 TTS 播放

---

## 9. 教训候选

- [架构设计] 教训：现有能力组件（EdgeTtsService/VoiceInputService/Live2DView.setLipSyncParam/memory plugin）已完整实现但未接线——"断链组件"是技术债务的隐蔽形式，代码存在但无人调用比代码缺失更难发现。标准：所有 public API 必须有调用方，否则标记为 @Deprecated 或写入集成待办。建议：设置 CI 静态分析规则（ktlint/Detekt custom rule）检测"无人调用的 public fun"。

- [模块设计] 教训：MemoryPlugin 已实现（Room + keyword matching + period summarization）但两个月未集成——插件化设计的受益（解耦）同时也是风险（遗忘集成）；与 V1.0 表情链路 T11 断线缺陷同源（ab59b45 插件化重构后 EmotionController 从未实例化）。标准：每个插件模块必须有集成测试验证 onLoad→hook registration→触发 完整链路。建议：新增 PluginIntegrationTest 基类，所有插件必须通过集成烟雾测试。

- [候选对比] 教训：V1.1 的离线语音候选（sherpa-onnx/Vosk/whisper.cpp）均不低于 50MB 模型增量，APK 体积会是后续离线化的主要瓶颈。标准：离线模型方案必须提供"按需下载"机制（用户手动触发下载，不捆绑进 APK），参考 sherpa-onnx 的模型仓库模式。建议：V1.2 引入 ModelDownloadManager（DownloadManager API + 解压到 externalFilesDir），不增加基础 APK 体积。
