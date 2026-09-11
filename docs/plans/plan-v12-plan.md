# Live2D-Ai V1.2「语音重塑 + 离线 + 陪伴 + 可观测」架构设计文档

> 版本: v1.0 — 2026-08-06
> 架构师: software-architect (SPOQ Phase: architecting)
> 范围: 7 模块整体架构规划（Android 主攻 + PC 协同）
> 核心原则: **一切以中国大陆为准** — 任何依赖境外公共服务的能力必须有中国大陆可用方案或降级链

---

## 1. 需求概述

Live2D-Ai V1 核心链路（文本→LLM→[emotion]→Live2D）双端验收通过，V1.1 补齐语音闭环（VoiceIoController/LipSync/聊天持久化/长期记忆）。V1.2 聚焦四大升级：

| 方向 | 当前痛点 | V1.2 目标 |
|------|---------|----------|
| **语音重塑** | Edge TTS 在中国大陆被墙（HTTP 403），系统 TTS 音质差 | 多 Provider TTS 架构 + 国内云端 + 离线本地 |
| **离线能力** | ASR/TTS 100% 依赖网络 | 本地离线引擎（sherpa-onnx）覆盖无网场景 |
| **主动陪伴** | 用户必须主动召唤，竞品 6+/11 已有 | 时间感知主动问候（WorkManager） |
| **可观测性** | isSpeaking 不可观察，日志被 ROM 抑制 | StateFlow 暴露 + Log.i 规范 + 链路遥测 |

**架构核心设计原则**:
- **复用 V1.1 基础设施**: VoiceIoController、ChatHook、SettingsRepository、LLMProviderManager、PluginManager 全部保留
- **Provider 抽象优先**: TTS/ASR 先做接口抽象（参考 N.E.K.O. tts_client 模式），再逐个接入具体引擎
- **APK 体积预算 <150MB**: 离线模型按需下载，不捆绑进 APK

---

## 2. 系统架构

### 2.1 V1.2 整体架构概览

```
┌──────────────────────────────────────────────────────────────────┐
│                        Live2D-Ai V1.2                              │
├──────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────────┐│
│  │ 模块1: TTS   │  │ 模块4: ASR   │  │ 模块5: 引擎选择 UI         ││
│  │ Provider接口 │  │ Provider接口 │  │ (设置页: TTS/ASR 下拉)    ││
│  │ + 注册表    │  │ + 注册表     │  │ SettingsRepository 持久化  ││
│  └──────┬──────┘  └──────┬───────┘  └────────────────────────────┘│
│         │                │                                         │
│  ┌──────┴──────┐  ┌──────┴───────┐                               │
│  │ Cloud Stream │  │ System ASR   │                               │
│  │ (CosyVoice)  │  │ (SpeechRec.) │                               │
│  │ Local Onnx   │  │ sherpa-onnx  │                               │
│  │ System TTS   │  │ ASR模型      │                               │
│  │ Silent       │  │ Silent       │                               │
│  └──────┬──────┘  └──────┬───────┘                               │
│         │                │                                         │
│         └────────┬───────┘                                         │
│                  │                                                 │
│  ┌───────────────┴───────────────┐                                │
│  │      VoiceIoController        │  ← V1.1 现存，适配多Provider  │
│  │  (统一TTS/ASR/LipSync协调器)  │                                │
│  └───────────────┬───────────────┘                                │
│                  │                                                 │
│  ┌───────────────┴───────────────┐                                │
│  │         ChatService           │  ← V1.0 现存，不变             │
│  └───────────────────────────────┘                                │
│                                                                    │
│  ┌───────────────────────────────┐                                │
│  │     模块6: 主动陪伴插件        │  ← WorkManager + ChatHook     │
│  │  live2d-ai-plugin-proactive    │                                │
│  └───────────────────────────────┘                                │
│                                                                    │
│  ┌───────────────────────────────┐                                │
│  │     模块7: 可观测性            │  ← StateFlow + Log.i + Telemetry│
│  │  isSpeaking / 链路遥测        │                                │
│  └───────────────────────────────┘                                │
│                                                                    │
└──────────────────────────────────────────────────────────────────┘
```

### 2.2 模块依赖图

```
                    ┌──────────────────┐
                    │  模块1: TtsProvider │ ← P0 架构基础
                    │  接口 + 注册表     │
                    └────────┬─────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
  ┌───────┴───────┐  ┌──────┴──────┐  ┌───────┴──────┐
  │ 模块2: CosyVoice│  │ 模块3: sherpa│  │ 模块5: 引擎  │
  │ 云端TTS Worker │  │ onnx 离线TTS │  │ 选择 UI      │
  └───────┬───────┘  └──────┬──────┘  └───────┬──────┘
          │                  │                  │
          └──────────────────┼──────────────────┘
                             │
  ┌───────────────┐  ┌──────┴──────┐  ┌──────────────────┐
  │ 模块4: ASR    │  │ VoiceIo     │  │ 模块7: 可观测性  │
  │ Provider接口  │──┤ Controller  │──┤ (isSpeaking等)   │
  └───────────────┘  └──────┬──────┘  └──────────────────┘
                             │
                    ┌────────┴────────┐
                    │ 模块6: 主动陪伴  │ ← P1，独立于TTS/ASR
                    │ 插件            │
                    └─────────────────┘
```

**集成顺序**: 模块1(TtsProvider抽象) → 模块2+3(具体TTS Worker) → 模块4(ASR抽象) → 模块5(引擎选择UI) → 模块6(主动陪伴) → 模块7(可观测性)

---

## 3. 模块划分

---

### 模块 1: TtsProvider 接口抽象 + 注册表（P0，架构基础）

#### 3.1.1 职责

将现有的单引擎 `EdgeTtsService` 重构为多 Provider 架构——定义 `TtsProvider` 接口、实现注册表、改造 `VoiceIoController` 支持降级链切换。这是后续所有 TTS 模块的架构基础。

**设计理念**: 参考 N.E.K.O. `tts_client/workers/` 的三分类思想（`ws_bistream` / `http_sentence` / `local`），适配 Android 移动端。

#### 3.1.2 候选对比表: Provider 架构模式

| 方案 | 描述 | 复杂度 | 灵活性 | 与现有代码兼容 | 推荐 |
|------|------|--------|--------|---------------|------|
| **A: TtsProvider 接口 + 注册表** | 定义 `interface TtsProvider`，每个引擎实现该接口；注册表 `TtsProviderRegistry` 管理实例；`VoiceIoController` 按降级链遍历 | 中 | ★★★★★ | ✅ 需改造 VoiceIoController.speak() 内部逻辑 | ✅ **推荐** |
| B: AbstractTtsService 基类 | EdgeTtsService 改名为抽象基类，每个引擎继承 | 低 | ★★★ | ✅ 改动最小 | 🔶 备选（但扩展性不如方案A） |
| C: 工厂模式 (TtsFactory) | 类似 PC 端 TTSFactory，静态工厂 + if-else | 极低 | ★★ | ✅ 与现有 pattern 最接近 | ❌ 不推荐（if-else 膨胀） |
| D: Plugin 模式 (TTS 引擎作为插件) | TTS 引擎作为 ChatHook 插件注入 | 高 | ★★★★★ | ⚠️ 架构差异大 | ❌ 过度设计 |

**结论**: **方案 A (TtsProvider 接口 + 注册表)** — 与 LLMProviderManager 的 enum + switch 模式互补（TTS 引擎比 LLM 多，且涉及降级链遍历），注册表模式更适合。N.E.K.O. 的 `_registry_meta.py` 也证明注册表是最成熟的多 Provider 模式。

#### 3.1.3 关键类设计: TtsProvider 接口

```kotlin
/**
 * TTS Provider 接口 — 所有 TTS 引擎的统一合约。
 *
 * 分类对标 N.E.K.O. tts_client/workers/ 三分类：
 * - ws_bistream: WebSocket 流式双工（CosyVoice DashScope Realtime API）
 * - http_sentence: HTTP 按句请求（阿里云 REST API / 火山引擎等）
 * - local: 本地离线推理（sherpa-onnx / 系统 TTS）
 *
 * 设计原则：
 * - speak() 是挂起函数，阻塞到播放完成或失败
 * - stop() 立即停止播放（新消息打断）
 * - isSpeaking: StateFlow 供 UI/LipSync/可观测性使用
 * - 所有实现必须容错（内部异常不向外抛，通过 providerInfo.health 暴露）
 */
interface TtsProvider {
    /** Provider 元数据（id / name / category / 输入输出方式 / 离线能力） */
    val info: TtsProviderInfo

    /** 播放状态流（模块7: 可观测性）— true=正在播放，false=空闲 */
    val isSpeaking: StateFlow<Boolean>

    /**
     * 合成并播放文本。挂起直到播放完成、失败或被打断。
     * @param text 已剥离 [emotion] 标签的纯文本
     * @throws CancellationException 当被 stop() 打断时抛出（协程正常取消）
     */
    suspend fun speak(text: String)

    /** 立即停止当前播放（打断）。幂等。 */
    fun stop()

    /** 释放引擎资源（音频播放器、WebSocket 连接等）。幂等。 */
    fun release()
}
```

#### 3.1.4 关键类设计: TtsProviderInfo / TtsProviderCategory

```kotlin
/** Provider 分类（对标 N.E.K.O. worker 分类） */
enum class TtsProviderCategory {
    /** WebSocket 流式双工 — 首音频延迟最低 */
    CLOUD_WS,
    /** HTTP 按句请求 — 实现最简单 */
    CLOUD_HTTP,
    /** 本地离线推理 — 零网络依赖 */
    LOCAL,
}

/** Provider 元数据（对标 N.E.K.O. _registry_meta.py dataclass） */
data class TtsProviderInfo(
    val id: String,                    // "cosyvoice_ws" / "sherpa_onnx_kokoro" / "system_tts"
    val name: String,                  // "CosyVoice 云端" / "Kokoro 离线" / "系统 TTS"
    val category: TtsProviderCategory,
    val requiresNetwork: Boolean,     // false 为离线可用
    val health: StateFlow<TtsHealth>, // OK / DEGRADED / UNAVAILABLE
    val priority: Int,                // 降级链优先级（数字越小越优先，0 最高）
    val configurable: Boolean,        // 是否有用户可配置参数（API Key 等）
)
```

#### 3.1.5 关键类设计: TtsProviderRegistry

```kotlin
/**
 * TTS Provider 注册表 — 管理所有已注册的 TTS Provider。
 *
 * 对标 N.E.K.O. _registry_meta.py：
 * - 每个 provider 通过 register() 注册
 * - 按 priority 排序构成降级链
 * - get() 按 id 查找
 * - all() 返回所有可用 provider（供 UI 选择）
 */
object TtsProviderRegistry {
    private val _providers = mutableListOf<TtsProvider>()

    fun register(provider: TtsProvider) { _providers += provider }
    fun unregister(providerId: String) { _providers.removeAll { it.info.id == providerId } }

    /** 按优先级排序的降级链（数字越小越优先） */
    fun cascade(): List<TtsProvider> = _providers.sortedBy { it.info.priority }

    /** 按 id 查找 */
    fun get(id: String): TtsProvider? = _providers.find { it.info.id == id }

    /** 所有已注册 provider（供引擎选择 UI） */
    fun all(): List<TtsProvider> = _providers.toList()

    /** 仅离线可用 */
    fun offlineOnly(): List<TtsProvider> = _providers.filter { !it.info.requiresNetwork }
}
```

#### 3.1.6 降级链重构

**Android 端 V1.2 降级链**:

```
Tier 1 (priority=0): CosyVoice 云端 WS (阿里云 DashScope)     ✨ 新增
  └─ 失败 → Tier 2 (priority=1): sherpa-onnx + Kokoro 离线  ✨ 新增
      └─ 失败 → Tier 3 (priority=2): Android 系统 TTS       保留
          └─ 失败 → Tier 4 (priority=3): 静默降级            保留
```

**PC 端 V1.2 降级链**:

```
Tier 1 (priority=0): 本地 CosyVoice / sherpa-onnx ✨ 新增
  └─ 失败 → Tier 2 (priority=1): 云端 CosyVoice (阿里云) ✨ 新增
      └─ 失败 → Tier 3 (priority=2): Edge TTS (海外备选)    保留
          └─ 失败 → Tier 4 (priority=3): 静默降级            保留
```

#### 3.1.7 与现有代码的接线点

| 现有组件 | 改造方式 | 说明 |
|---------|---------|------|
| `EdgeTtsService` | 实现 `TtsProvider` 接口 → `EdgeTtsProvider` | 重命名，保留现有 WebSocket+MediaPlayer 逻辑 |
| `VoiceIoController.speak()` | 改为遍历 `TtsProviderRegistry.cascade()` | 传入 providerId → `get(id)?.speak()`；失败遍历降级链 |
| `VoiceIoController.stopSpeaking()` | 遍历所有 provider 调用 `stop()` | 确保切 provider 后旧 provider 资源释放 |
| `SettingsRepository` | 新增 `ttsProviderId: String` | 持久化用户选择的 TTS 引擎 |

#### 3.1.8 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| **`TtsProvider.kt`** | **新增** | TtsProvider 接口 + TtsProviderInfo + TtsProviderCategory |
| **`TtsProviderRegistry.kt`** | **新增** | 注册表 + 降级链 |
| `EdgeTtsService.kt` → **`EdgeTtsProvider.kt`** | **重命名+改造** | 实现 TtsProvider 接口 |
| **`SystemTtsProvider.kt`** | **新增** | 剥离 EdgeTtsService 中的 Android TTS 逻辑为独立 Provider |
| `VoiceIoController.kt` | **修改** | 接入注册表，降级链遍历 |
| `SettingsRepository.kt` | **修改** | 新增 `ttsProviderId` |
| `MainActivity.kt` | **修改** | 构建时注册 provider → TtsProviderRegistry |

---

### 模块 2: 阿里云 CosyVoice 云端 Worker（P0，中国大陆核心）

#### 3.2.1 职责

通过阿里云 DashScope Realtime API（WebSocket 流式）或语音合成 REST API 接入 CosyVoice 云端 TTS，作为 Android/PC 双端的 Tier 1 TTS Provider。

#### 3.2.2 候选对比表: CosyVoice 接入方式

| 方案 | 接入方式 | 流式 | 延迟 | 许可/成本 | Android SDK | 推荐 |
|------|---------|------|------|----------|-------------|------|
| **A: DashScope Realtime API (WebSocket)** | `wss://dashscope.aliyuncs.com/api-ws/v1/realtime` | ✅ 流式双工 | <500ms | 阿里云免费额度 (每月百万字符) | 无官方 SDK，HTTP/WS 直连 | ✅ **Android/PC 推荐** |
| B: DashScope HTTP REST (speech-synthesizer) | `POST /api-ws/v1/speech-synthesizer/{model}` | 按句切分 | 1-3s (整句合成) | 同上 | 无官方 SDK | 🔶 备选（text较长时延迟明显） |
| C: SiliconFlow CosyVoice2 API | `POST https://api.siliconflow.cn/v1/audio/speech` | ✅ stream=true | <1s | SiliconFlow 免费额度 | 无 SDK → HTTP 直连 | 🔶 备选（依赖 SiliconFlow 中转） |
| D: 阿里云智能语音交互 SDK (AliNls) | 官方 Android/iOS SDK | ✅ 流式 | <500ms | 同上 | ✅ **官方 Android SDK** | 🔶 备选（SDK 封装度高，灵活度低） |

**结论**: **方案 A (DashScope Realtime API WebSocket)** — 与现有 EdgeTtsService 的 WebSocket 模式最接近（OkHttp WebSocket），代码复用度高；支持流式双工，首音频延迟 <500ms；阿里云在中国大陆延迟最低。PC 端已有 CosyVoice Gradio API 配置（`cosyvoice_tts`/`cosyvoice2_tts`），可复用为本地推理接入。

#### 3.2.3 关键类设计: CosyVoiceTtsProvider

```kotlin
/**
 * CosyVoice 云端 TTS Provider（DashScope Realtime API WebSocket 流式）。
 *
 * Android 端使用 OkHttp WebSocket 直连（无官方 Android SDK，走 HTTP/WS 即可）。
 * PC 端可复用 DashScope Realtime API 或已有 Gradio API（cosyvoice2_tts 配置）。
 *
 * 分类: CLOUD_WS（对标 N.E.K.O. ws_bistream 类 worker）
 */
class CosyVoiceTtsProvider(
    private val context: Context,
    private val apiKey: String,           // 阿里云 DashScope API Key
    private val voice: String = "longxiaochun", // 默认音色
    private val model: String = "cosyvoice-v1",
) : TtsProvider {

    override val info = TtsProviderInfo(
        id = "cosyvoice_ws",
        name = "CosyVoice 云端",
        category = TtsProviderCategory.CLOUD_WS,
        requiresNetwork = true,
        priority = 0,
        configurable = true,
    )

    private val client = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    // WebSocket 连接 + MediaPlayer 播放
    // 复用 EdgeTtsProvider 的 WebSocket→MP3→MediaPlayer 播放模式

    override suspend fun speak(text: String) { /* WebSocket 流式合成 → MediaPlayer 播放 */ }
    override fun stop() { /* 关闭 WS + 停 MediaPlayer */ }
    override fun release() { /* 释放资源 */ }
}
```

#### 3.2.4 认证配置: API Key 管理

- **存储**: `SettingsRepository` 新增 `cosyvoiceApiKey: String`
- **UI**: 设置页新增「阿里云 API Key」输入框（P0 必须项，无内置免费 key）
- **说明**: 阿里云提供每月百万字符免费额度，需用户自行注册 DashScope 账号获取 API Key
- **PC 端**: `conf.yaml` 新增 `cosyvoice_cloud` 配置段（api_key/voice/model），与已有 `cosyvoice_tts` Gradio 配置共存

#### 3.2.5 降级链位置

- **Android**: Tier 1 (priority=0)，失败 → sherpa-onnx Kokoro 离线 (Tier 2)
- **PC**: 本地 CosyVoice (Tier 1) → 云端 CosyVoice (Tier 2，本模块提供)

#### 3.2.6 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| **`CosyVoiceTtsProvider.kt`** | **新增** | 实现 TtsProvider，OkHttp WebSocket 到 DashScope |
| `SettingsRepository.kt` | **修改** | 新增 `cosyvoiceApiKey` |
| `MainActivity.kt` | **修改** | 注册 CosyVoiceTtsProvider |
| `activity_settings.xml` / Compose 设置页 | **修改** | API Key 输入框 |
| **PC**: `conf.yaml.template` | **修改** | 新增 `cosyvoice_cloud` 配置段 |
| **PC**: `config_manager/tts.py` | **修改** | 新增 `CosyVoiceCloudTtsConfig` Pydantic model |
| **PC**: `tts_factory.py` | **修改** | 新增 `cosyvoice_cloud` → CosyVoiceCloudTTSEngine |

---

### 模块 3: sherpa-onnx + Kokoro Android 离线 TTS（P1）

#### 3.3.1 职责

通过 sherpa-onnx Android AAR 集成 Kokoro 中文语音模型（或 VITS-zh 中文模型），实现 Android 端完全离线 TTS——零网络依赖，中国大陆完全可用。

#### 3.3.2 候选对比表: Android 离线 TTS 方案

| 方案 | 模型 | APK 增量 | 中文音质 | 延迟 | 成熟度 | 推荐 |
|------|------|---------|---------|------|--------|------|
| **A: sherpa-onnx + Kokoro (zh)** | kokoro-multi-lang-v1.0.onnx (~82MB / ~340MB 多语言) | 0 (按需下载) | ⭐⭐⭐ 待实测 | <500ms | ⭐⭐⭐⭐ (sherpa-onnx 官方支持) | ✅ **V1.2 推荐** |
| B: sherpa-onnx + VITS-zh | vits-melo-tts-zh (~100MB) | 0 (按需下载) | ⭐⭐⭐⭐ | <1s | ⭐⭐⭐⭐ (sherpa-onnx 维护) | 🔶 V1.3 中文升级 |
| C: sherpa-onnx + Piper (zh) | piper-zh-hui-medium.onnx (~50MB) | 0 (按需下载) | ⭐⭐ | <500ms | ⭐⭐⭐ | ❌ 中文不够自然 |
| D: Kokoro-FastAPI (需自架服务器) | Kokoro 82MB ONNX | 0 (但需服务器) | ⭐⭐⭐ | ~2s (网络) | ⭐⭐⭐ | ❌ 不是本地离线 |

**结论**: **方案 A (sherpa-onnx + Kokoro 多语言模型)** — 唯一同时满足"Android 原生 AAR"+"<100MB 可下载"+"Apache 2.0"+"流式低延迟"的方案。Kokoro 中文音质待实测（调研标注 ⭐⭐⭐ 中等），但作为离线兜底已优于系统 TTS。若测后不达标，方案 B (VITS-zh) 是备选升级路径。

#### 3.3.3 sherpa-onnx Android 集成

**依赖方式**（参考 [sherpa-onnx 官方 Android 文档](https://k2-fsa.github.io/sherpa/onnx/tts/android.html)）:

```kotlin
// app/build.gradle.kts
dependencies {
    implementation("com.k2fsa.sherpa:onnx-tts-engine:1.12.0")
    // 或本地 AAR（如网络不可达）
    // implementation(files("libs/sherpa-onnx-tts-1.12.0.aar"))
}
```

**模型管理**: 不打包进 APK — 首次使用时从 GitHub Releases 下载到 `externalFilesDir/models/`（约 82MB），下载前显示确认对话框（告知大小）。`ModelDownloadManager` 使用 Android `DownloadManager` API 或 OkHttp 直连。

**本地路径**: `context.getExternalFilesDir(null)!!.resolve("models/kokoro-multi-lang-v1.0.onnx")`

#### 3.3.4 关键类设计: SherpaOnnxTtsProvider

```kotlin
/**
 * sherpa-onnx 离线 TTS Provider。
 *
 * 分类: LOCAL（对标 N.E.K.O. local 类 worker）
 *
 * 关键实现要点:
 * - sherpa-onnx OfflineTts 引擎初始化（加载 ONNX 模型 + tokens）
 * - 调用 generate() 生成 WAV 音频 → 写入 temp → MediaPlayer 播放
 * - isSpeaking StateFlow 跟踪播放状态
 * - 模型下载管理（ModelDownloadManager）
 */
class SherpaOnnxTtsProvider(
    private val context: Context,
    private val modelPath: String, // 模型文件路径
) : TtsProvider {

    override val info = TtsProviderInfo(
        id = "sherpa_onnx_kokoro",
        name = "Kokoro 离线",
        category = TtsProviderCategory.LOCAL,
        requiresNetwork = false, // ← 离线可用
        priority = 1,
        configurable = false,
    )

    // sherpa-onnx 引擎初始化
    // private val ttsEngine: OfflineTts = OfflineTts(
    //     OfflineTtsConfig(modelConfig, ...)
    // )

    override suspend fun speak(text: String) { /* 生成音频 → MediaPlayer 播放 */ }
    override fun stop() { /* 停止播放 */ }
    override fun release() { /* 释放引擎 */ }
}
```

#### 3.3.5 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| **Kokoro 中文音质不达标** | 离线 TTS 体验差 | 降级到系统 TTS（音质已知可用）；V1.3 换 VITS-zh 模型 |
| **sherpa-onnx AAR 体积** | ~15MB 引擎 + ~82MB 模型 | 模型按需下载；AAR 体积可控（仅引擎 .so） |
| **模型下载失败/中断** | Provider UNHEALTHY | 显示健康状态 + 提供重试按钮；回退到降级链下一层 |
| **Android JNI .so 兼容性** | 某些机型崩溃 | sherpa-onnx 官方支持 arm64-v8a/armeabi-v7a/x86_64；构建时过滤 |

#### 3.3.6 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| **`SherpaOnnxTtsProvider.kt`** | **新增** | 实现 TtsProvider，sherpa-onnx Kokoro 引擎 |
| **`ModelDownloadManager.kt`** | **新增** | 下载管理（进度/重试/校验） |
| `app/build.gradle.kts` | **修改** | 新增 sherpa-onnx 依赖 |
| `MainActivity.kt` | **修改** | 注册 SherpaOnnxTtsProvider |

---

### 模块 4: Android ASR 离线化（P1）

#### 4.4.1 职责

引入离线 ASR 能力（sherpa-onnx SenseVoice/paraformer-zh），与现有的 Android SpeechRecognizer 共存——重构 `VoiceInputService` 为 `AsrProvider` 接口 + 注册表模式。

#### 4.4.2 候选对比表: Android ASR 方案

| 方案 | 模型 | APK 增量 | 离线 | 中文准确率 | 延迟 | 成熟度 | 推荐 |
|------|------|---------|------|-----------|------|--------|------|
| **A: sherpa-onnx + SenseVoice (zh)** | sense-voice-small (~120MB) | 0 (按需下载) | ✅ | ★★★★ | <500ms 流式 | ⭐⭐⭐⭐⭐ | ✅ **V1.2 推荐** |
| B: sherpa-onnx + paraformer-zh | paraformer-zh-small (~100MB) | 0 (按需下载) | ✅ | ★★★★★ | <1s | ⭐⭐⭐⭐ | 🔶 备选 |
| C: whisper.cpp (tiny/small) | ggml-tiny.bin (~77MB) / small (~466MB) | 0 (按需下载) | ✅ | ★★★ (tiny) / ★★★★ (small) | 1-5s | ⭐⭐⭐ | ❌ 体积大/延迟高 |
| D: Android SpeechRecognizer (现有) | 0 | 0 | 🔶 部分离线 (Gboard) | ★★★☆ | 0.5-2s | ★★★★ (系统服务) | ✅ 保留为降级链 |
| E: sherpa-onnx + Zipformer | zipformer-zh (~100MB) | 0 | ✅ | ★★★★★ | <500ms | ⭐⭐⭐⭐⭐ | 🔶 V1.3 升级 |

**结论**: **方案 A (sherpa-onnx + SenseVoice)** — sherpa-onnx 官方 Android AAR 支持，SenseVoice 中文识别率优秀（阿里开源），流式低延迟。保留 Android SpeechRecognizer 作为降级链和 ASR 引擎选择之一。

#### 4.4.3 AsrProvider 接口设计

```kotlin
/**
 * ASR Provider 接口。
 *
 * 简化于 TtsProvider——ASR 链路简单（listen→返回文本），不需要复杂的播放状态管理。
 */
interface AsrProvider {
    val info: AsrProviderInfo

    /**
     * 开始监听并返回识别文本。
     * @return 识别文本；失败/超时返回空字符串
     */
    suspend fun listen(): String

    /** 停止监听 */
    fun stopListening()

    /** 释放资源 */
    fun release()
}

data class AsrProviderInfo(
    val id: String,                 // "sherpa_sensevoice" / "system_speech_recognizer"
    val name: String,
    val requiresNetwork: Boolean,
    val health: StateFlow<TtsHealth>,
    val priority: Int,
)
```

#### 4.4.4 与现有代码的接线

| 现有组件 | 改造方式 |
|---------|---------|
| `VoiceInputService` | 抽取 `AsrProvider` 接口 → `SystemAsrProvider` 实现 |
| `VoiceIoController.listen()` | 改为遍历 `AsrProviderRegistry.cascade()` |
| `SettingsRepository` | 新增 `asrProviderId: String` |

**ASR 降级链**: 离线 sherpa-onnx (Tier1) → 系统 SpeechRecognizer (Tier2) → 静默返回""

#### 4.4.5 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| **`AsrProvider.kt`** | **新增** | AsrProvider 接口 + AsrProviderInfo |
| **`AsrProviderRegistry.kt`** | **新增** | ASR 注册表（简化版 TtsProviderRegistry） |
| **`SherpaOnnxAsrProvider.kt`** | **新增** | sherpa-onnx SenseVoice ASR |
| `VoiceInputService.kt` → **`SystemAsrProvider.kt`** | **重命名+改造** | 实现 AsrProvider 接口 |
| `VoiceIoController.kt` | **修改** | 接入 AsrProviderRegistry |
| `SettingsRepository.kt` | **修改** | 新增 `asrProviderId` |

---

### 模块 5: TTS/ASR 引擎选择 UI（P1）

#### 5.5.1 职责

在 Compose 设置页添加 TTS 引擎选择器（下拉列表）+ ASR 引擎选择（可选），持久化到 `SettingsRepository`。对标竞品中 8/11 已有类似功能。

#### 5.5.2 候选对比表: 引擎选择 UI 模式

| 方案 | 描述 | 复杂度 | 用户自由度 | 推荐 |
|------|------|--------|-----------|------|
| **A: Provider 下拉列表 (RadioGroup)** | 显示 TtsProviderRegistry.all() → 单选 | 低 | ★★★★ | ✅ **推荐** |
| B: Provider 列表 + 开关 (Toggle Switch) | 每个 Provider 有独立开关（可同时启用多个作降级链） | 中 | ★★★★★（但理解成本高） | 🔶 备选（过度设计） |
| C: 简单文本配置项 | conf.yaml 或文本字段输入 provider id | 极低 | ★★ | ❌ 用户不友好 |

**结论**: **方案 A (下拉列表)** — 简单直观，对标竞品标准做法。降级链为系统内置逻辑（用户只需选择首选引擎），后台自动尝试下一层。

#### 5.5.3 UI 设计

```
设置页
├── LLM Provider (现有: DeepSeek / Zhipu / Local)
├── ────────────
├── ✨ TTS 引擎 (新增)
│   ├── ◉ CosyVoice 云端 ← 需要 API Key
│   │   └── [阿里云 API Key 输入框]  (仅选中时展开)
│   ├── ○ Kokoro 离线 ← 需要下载模型
│   │   └── [下载模型按钮] [进度条]  (仅选中时展开)
│   ├── ○ 系统 TTS
│   └── ℹ️ 降级链: CosyVoice → Kokoro → 系统TTS → 静默
├── ✨ ASR 引擎 (新增)
│   ├── ◉ 本地离线 (SenseVoice) ← 需要下载模型
│   ├── ○ 系统语音识别
│   └── ℹ️ 降级链: 离线 → 系统ASR → 静默
├── ────────────
├── ✨ 主动陪伴 (新增)
│   └── [开关] 主动问候 (开启后在闲聊间隔超过2小时后主动问候)
├── ────────────
└── ... (其他设置)
```

#### 5.5.4 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| `SettingsRepository.kt` | **修改** | 新增 `ttsProviderId` / `asrProviderId` / `proactiveEnabled` |
| `ui/settings/SettingsScreen.kt` (或等效 Compose) | **修改** | TTS 引擎选择器 UI + ASR 引擎选择器 |
| `MainActivity.kt` | **修改** | 引擎切换时重新配置 VoiceIoController |

---

### 模块 6: 主动陪伴插件（P1）

#### 5.6.1 职责

基于已有骨架 `live2d-ai-plugin-proactive`（`docs/plans/plan-task-plugin-proactive.md` 详细设计），落地为可工作的主动陪伴插件：`WorkManager` 周期检查 → 时间感知 → `PluginHost.sendAssistantMessage()` 主动说话。

#### 5.6.2 参考设计

已有完整设计文档 `docs/plans/plan-task-plugin-proactive.md`（约 300 行），核心组件：

| 组件 | 文件 | 状态 |
|------|------|------|
| `ProactivePlugin` | onLoad 注册 WorkManager + ChatHook | 已有骨架 |
| `LastInteractionTracker` | SharedPreferences 时间戳 | 已设计待实现 |
| `ProactiveCheckWorker` | CoroutineWorker 决策 + 触发 | 已设计待实现 |
| `ProactiveTrackingHook` | ChatHook.afterReceive → 更新时间戳 | 已设计待实现 |
| `GreetingLlmClient` | fun interface (LLM 生成问候) | 已设计待实现 |

#### 5.6.3 V1.2 增强: 记忆驱动话题

V1.2 的主动陪伴比 MVP 设计多了一项能力：**基于长期记忆生成个性化话题**。

```kotlin
// ProactiveCheckWorker.doWork() 增强
// 1. 检查时间条件（冷却/静默时段/空闲阈值）
// 2. 从 MemoryPlugin 获取最近记忆 → 生成话题上下文
// 3. GreetingLlmClient.generateGreeting(persona, systemPrompt, idleMinutes, memories)
// 4. host.sendAssistantMessage(greeting)
```

#### 5.6.4 V1.2 开发增强: Debug 短周期开关

**教训**: 长周期功能 E2E 验收受限（记忆插件 10 轮阈值）。V1.2 必须内置 debug 开关。

```kotlin
// ProactivePluginConfig
val idleThresholdMinutes: Long = if (BuildConfig.DEBUG) 5L else 180L,  // debug: 5分钟
val cooldownMinutes: Long = if (BuildConfig.DEBUG) 2L else 360L,        // debug: 2分钟
val workManagerIntervalMinutes: Long = if (BuildConfig.DEBUG) 15L else 15L, // WorkManager 最小 15 分钟硬约束
```

#### 5.6.5 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| `live2d-ai-plugin-proactive/` 下多个文件 | **修改** | 按 plan-task-plugin-proactive.md 落地 |
| `live2d-ai-plugin-proactive/build.gradle.kts` | **修改** | 新增 `work-runtime-ktx` 依赖 |
| `SettingsRepository.kt` | **修改** | 新增 `proactiveEnabled: Boolean` |
| `MainActivity.kt` | **修改** | 加载 ProactivePlugin |

---

### 模块 7: 可观测性（P2）

#### 5.7.1 职责

暴露 `isSpeaking: StateFlow<Boolean>` 供 LipSync/UI 观察；对话记录统一；TTS/ASR 链路遥测事件；核心链路日志规范。

#### 5.7.2 设计

##### 7.2.1 isSpeaking StateFlow

已在 TtsProvider 接口设计中覆盖（`val isSpeaking: StateFlow<Boolean>`）。`VoiceIoController` 暴露组合后的状态：

```kotlin
// VoiceIoController 新增
val isSpeaking: StateFlow<Boolean> = combine(
    providers.map { it.info.health } + providers.map { it.isSpeaking }
) { /* 任一 provider 处于播放状态 = true */ }
```

**使用方**:
- LipSync 动画: `voiceIoController.isSpeaking.collect { if (it) startLipSync() else stopLipSync() }`（替代当前 EdgeTtsService.onPlaybackStateChanged 回调模式）
- UI 指示器: 输入栏旁「正在说话…」动画
- 主动陪伴: 正在说话时不触发问候

##### 7.2.2 对话记录统一

V1.1 已有 `ChatHistoryStore`（JSON 持久化）和 MemoryPlugin（Room 数据库）。V1.2 不改动，保持两个独立持久化层。

##### 7.2.3 链路遥测事件

```kotlin
/** TTS/ASR 链路事件（Log.i 级别，国产 ROM 兼容） */
sealed class VoiceTelemetryEvent {
    data class TtsStarted(val providerId: String, val textLen: Int) : VoiceTelemetryEvent()
    data class TtsCompleted(val providerId: String, val durationMs: Long, val audioBytes: Int) : VoiceTelemetryEvent()
    data class TtsFailed(val providerId: String, val error: String) : VoiceTelemetryEvent()
    data class TtsDegraded(val fromProvider: String, val toProvider: String, val reason: String) : VoiceTelemetryEvent()
    data class AsrStarted(val providerId: String) : VoiceTelemetryEvent()
    data class AsrCompleted(val providerId: String, val textLen: Int, val durationMs: Long) : VoiceTelemetryEvent()
    data class AsrFailed(val providerId: String, val error: String) : VoiceTelemetryEvent()
}

object VoiceTelemetry {
    private const val TAG = "VoiceTelemetry"

    fun log(event: VoiceTelemetryEvent) {
        Log.i(TAG, event.toString())  // Log.i — 国产 ROM 兼容（教训: Log.d 被抑制）
        // 可选: 写入本地环形缓冲区（最近 100 条），供调试页展示
    }
}
```

##### 7.2.4 日志规范

| 级别 | 用途 | 示例 |
|------|------|------|
| **Log.i** | 核心链路关键节点（TTS/ASR 启停、降级、模型加载） | `VoiceTelemetry.log(TtsStarted(...))` |
| Log.w | 非致命异常（降级、超时） | `"CosyVoice 云端不可用，降级到 Kokoro: 403"` |
| Log.e | 致命异常（资源释放失败、崩溃前状态） | `"sherpa-onnx 引擎初始化失败"` |
| Log.d | 调试细节（仅在 BuildConfig.DEBUG 时编译） | `"WebSocket 帧 #7: 4096 bytes"` |

#### 5.7.5 新增/修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| **`VoiceTelemetry.kt`** | **新增** | 链路遥测事件定义 + 日志/环形缓冲区 |
| `VoiceIoController.kt` | **修改** | 暴露 isSpeaking StateFlow；注入遥测日志 |
| 各个 TtsProvider 实现 | **修改** | speak/stop 位置插入遥测日志 |

---

## 4. 数据模型

### 4.1 TtsProviderInfo (注册表条目)

```kotlin
data class TtsProviderInfo(
    val id: String,                    // "cosyvoice_ws" | "sherpa_onnx_kokoro" | "system_tts" | "edge_tts"
    val name: String,                  // 人类可读名称
    val category: TtsProviderCategory,  // CLOUD_WS | CLOUD_HTTP | LOCAL
    val requiresNetwork: Boolean,
    val health: StateFlow<TtsHealth>,  // OK | DEGRADED | UNAVAILABLE
    val priority: Int,                 // 降级链顺序（0 = 最高）
    val configurable: Boolean,         // 是否有用户配置参数
)

enum class TtsHealth { OK, DEGRADED, UNAVAILABLE }
```

### 4.2 AsrProviderInfo (注册表条目)

```kotlin
data class AsrProviderInfo(
    val id: String,                    // "sherpa_sensevoice" | "system_speech_recognizer"
    val name: String,
    val requiresNetwork: Boolean,
    val health: StateFlow<TtsHealth>,
    val priority: Int,
)
```

### 4.3 SettingsRepository 新增 Key

```kotlin
// SettingsRepository 新增常量
const val KEY_TTS_PROVIDER_ID = "tts_provider_id"
const val KEY_ASR_PROVIDER_ID = "asr_provider_id"
const val KEY_COSYVOICE_API_KEY = "cosyvoice_api_key"
const val KEY_PROACTIVE_ENABLED = "proactive_enabled"

// 默认值
const val DEFAULT_TTS_PROVIDER_ID = "cosyvoice_ws"   // 首选云端的 CosyVoice
const val DEFAULT_ASR_PROVIDER_ID = "sherpa_sensevoice" // 首选离线 SenseVoice
```

### 4.4 VoiceTelemetryEvent (遥测事件)

```kotlin
sealed class VoiceTelemetryEvent {
    // 包含: providerId, textLen, audioBytes, durationMs, error, fromProvider, toProvider, reason
}
```

### 4.5 PC 端 conf.yaml 新增配置段

```yaml
# V1.2 新增: 云端 CosyVoice (DashScope)
cosyvoice_cloud:
  api_key: ""      # 阿里云 DashScope API Key
  voice: "longxiaochun"
  model: "cosyvoice-v1"
  stream: true

# V1.2 新增: TTS Provider 优先级
tts_provider_priority: "cosyvoice_tts"  # 首选本地 CosyVoice Gradio
tts_cascade_enabled: true               # 启用降级链

# V1.2 新增: ASR Provider
asr_provider: "sherpa_sensevoice"  # 或 "faster_whisper" / "funasr"
```

---

## 5. 接口定义（契约摘要）

完整契约见 `docs/plans/plan-v12-plan.schema.json`。

| 接口 | 签名 | 说明 |
|------|------|------|
| `TtsProvider.speak` | `suspend fun speak(text: String)` | 合成并播放，挂起到完成/失败 |
| `TtsProvider.stop` | `fun stop()` | 立即停止播放 |
| `TtsProvider.isSpeaking` | `val isSpeaking: StateFlow<Boolean>` | 播放状态流 |
| `TtsProviderRegistry.cascade` | `fun cascade(): List<TtsProvider>` | 按优先级返回降级链 |
| `AsrProvider.listen` | `suspend fun listen(): String` | 开始监听 → 返回识别文本 |
| `VoiceIoController.isSpeaking` | `val isSpeaking: StateFlow<Boolean>` | 组合后的播放状态（多Provider） |
| `VoiceTelemetry.log` | `fun log(event: VoiceTelemetryEvent)` | 写入 Log.i + 环形缓冲区 |
| `ProactiveCheckWorker.doWork` | `suspend fun doWork(): Result` | WorkManager 周期检查 |
| `GreetingLlmClient.generateGreeting` | `suspend fun generateGreeting(personaName, systemPrompt, idleMinutes, memories?): String?` | LLM 话题生成 |

---

## 6. 文件清单（新增/修改）

### 新增文件 (14 个)

| # | 文件 | 模块 | 说明 |
|---|------|------|------|
| 1 | `app/.../TtsProvider.kt` | 1 | TtsProvider 接口 + TtsProviderInfo + TtsProviderCategory |
| 2 | `app/.../TtsProviderRegistry.kt` | 1 | TTS Provider 注册表 + 降级链 |
| 3 | `app/.../TtsHealth.kt` | 1 | TtsHealth enum + StateFlow 工具 |
| 4 | `app/.../SystemTtsProvider.kt` | 1 | 从 EdgeTtsService 剥离的系统 TTS Provider |
| 5 | `app/.../CosyVoiceTtsProvider.kt` | 2 | 阿里云 DashScope WebSocket TTS |
| 6 | `app/.../SherpaOnnxTtsProvider.kt` | 3 | sherpa-onnx Kokoro 离线 TTS |
| 7 | `app/.../ModelDownloadManager.kt` | 3 | 模型下载管理（进度/重试/校验） |
| 8 | `app/.../AsrProvider.kt` | 4 | AsrProvider 接口 + AsrProviderInfo |
| 9 | `app/.../AsrProviderRegistry.kt` | 4 | ASR Provider 注册表 |
| 10 | `app/.../SherpaOnnxAsrProvider.kt` | 4 | sherpa-onnx SenseVoice 离线 ASR |
| 11 | `app/.../VoiceTelemetry.kt` | 7 | 链路遥测事件定义 + Log.i 日志 |

### 修改文件 (11 个)

| # | 文件 | 模块 | 改动 |
|---|------|------|------|
| 12 | `EdgeTtsService.kt` → `EdgeTtsProvider.kt` | 1 | 实现 TtsProvider 接口；剥离系统 TTS 逻辑 |
| 13 | `VoiceIoController.kt` | 1/4/7 | 接入注册表；暴露 isSpeaking；遥测 |
| 14 | `SettingsRepository.kt` | 1/4/5/6 | 新增 keys |
| 15 | `MainActivity.kt` | 1/2/3/4/6 | 注册 providers；加载主动陪伴插件 |
| 16 | `app/build.gradle.kts` | 3/4 | 新增 sherpa-onnx 依赖 |
| 17 | `ui/settings/SettingsScreen.kt` | 5 | TTS/ASR 引擎选择器 UI |
| 18 | `live2d-ai-plugin-proactive/` 下多个文件 | 6 | 按 plan-task-plugin-proactive.md 落地 |
| 19 | `system/PersonaService.kt` 或等效 | 6 | ProactivePlugin GreetingLlmClient 实现注入 |

### PC 端改动文件 (4 个)

| # | 文件 | 模块 | 改动 |
|---|------|------|------|
| 20 | `config_manager/tts.py` | 2 | 新增 `CosyVoiceCloudTtsConfig` |
| 21 | `tts_factory.py` | 2 | 新增 `cosyvoice_cloud` engine |
| 22 | `tts/cosyvoice_cloud_tts.py` | 2 | DashScope WebSocket TTS 实现 |
| 23 | `conf.yaml.template` | 2/7 | 新增 cloud_tts / asr / observer 配置 |

### 无需改动的文件（保留不变）

- `ChatService.kt` — 现有 hook 机制不变
- `PluginManager.kt` — 现有接口已完备
- `ChatHook.kt` (plugin-sdk) — 不变
- `Live2DView.kt` / `Live2DRenderer.kt` — 不变
- `EmotionController.kt` — 不变
- `LLMProvider.kt` / `LLMProviderManager.kt` — 不变
- `PersonaConfig.kt` — 不变

---

## 7. Wave 分组建议

### Wave A (P0, 基础设施): 模块1 + 模块2

**目标**: TTS Provider 架构 + CosyVoice 云端接入 = 中国大陆语音可用性闭环

```
任务: task-v12-tts-provider  — 模块1 (TtsProvider接口+注册表+EdgeTts改造)
任务: task-v12-cosyvoice-tts  — 模块2 (CosyVoice DashScope 云端 TTS)
```

**依赖**: 模块2 依赖模块1（TtsProvider 接口先就位）
**验收**: CosmicVoice 在 Android 真机播放语音（中国大陆网络环境）

### Wave B (P1, 离线闭环): 模块3 + 模块4 + 模块5

**目标**: 离线 TTS/ASR 可用 + 引擎选择 UI

```
任务: task-v12-sherpa-tts    — 模块3 (sherpa-onnx Kokoro 离线 TTS)
任务: task-v12-sherpa-asr    — 模块4 (ASR Provider 接口 + sherpa-onnx 离线 ASR)
任务: task-v12-engine-ui     — 模块5 (引擎选择 UI)
```

**依赖**: 模块3 依赖模块1；模块4 相对独立；模块5 依赖模块3+4
**验收**: 关闭网络后 TTS/ASR 仍可工作，设置页可切换引擎

### Wave C (P1-P2, 体验增强): 模块6 + 模块7

**目标**: 主动陪伴 + 可观测性

```
任务: task-v12-proactive     — 模块6 (主动陪伴插件落地)
任务: task-v12-observability — 模块7 (isSpeaking + 遥测 + 日志规范)
```

**依赖**: 模块6 独立于 TTS/ASR（仅依赖 ChatHook + PluginHost）；模块7 依赖模块1-4
**验收**: 长时间闲置后自动问候 + isSpeaking StateFlow 可观察 + 遥测日志可读

### Wave D (PC 协同): PC 端改造

**目标**: PC 端 CosyVoice 云端 + 降级链重构

```
任务: task-v12-pc-cosyvoice  — PC 端 CosyVoice 云端 + TTS 降级链重配
```

---

## 8. 风险与注意事项

### 8.1 技术风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| **Kokoro 中文音质不达标** | 中 | 中 | 降级到系统 TTS（已知可用）；V1.3 换 VITS-zh (方案B) |
| **CosyVoice Android SDK 不确定性** | 低 | 低 | 走 HTTP/WS 直连（不需要 SDK），参考现有 EdgeTtsService WebSocket pattern |
| **sherpa-onnx AAR 版本兼容性** | 低 | 中 | 指定固定版本 1.12.0；真机 CI 验证 |
| **APK 体积膨胀** | 低 | 高 | 所有模型按需下载（DownloadManager），不打包进 APK；引擎 .so 文件约 15MB |
| **WorkManager Doze 延迟** | 高 | 低 | 主动陪伴触达是安级别的（非秒级），Doze 延迟在可接受范围 |
| **DashScope API 变更** | 低 | 中 | 接口契约化，provider 独立替换 |

### 8.2 架构约束

1. **插件 SDK 接口不动** — ChatHook/PluginHost 现有接口已完备，V1.2 不改动
2. **VoiceIoController 保持统一协调器角色** — TTS+ASR+LipSync 三合一入口点不变
3. **设置页渐进增强** — 在现有 Compose 设置页基础上增量添加，不重建
4. **PC 端 conf.yaml 向后兼容** — 新增配置段必须有合理默认值，不破坏现有配置
5. **所有 Provider 实现必须 Thread-Safe** — speak()/stop() 可能从不同线程调用
6. **模型文件不捆绑 APK** — 大小 > 10MB 的模型文件通过 ModelDownloadManager 按需下载

### 8.3 回退策略

- **模块1 (TtsProvider)**: 若接口设计有误 → 回归 EdgeTtsService 直接调用（保留原文件备份）
- **模块2 (CosyVoice)**: API 不可用 → VoiceIoController 降级链自动跳过，静默降至 Tier 2
- **模块3 (sherpa-onnx)**: 模型下载失败/引擎崩溃 → Provider health = UNAVAILABLE，自动跳过
- **模块4 (ASR)**: 离线 ASR 不可用 → 降级到系统 SpeechRecognizer（保留现有代码）
- **模块5 (引擎UI)**: UI 崩溃 → 默认 provider = cosyvoice_ws / system
- **模块6 (主动陪伴)**: WorkManager 任务失败 → 已注册的 ChatHook 仍记录时间，不影响对话链路
- **模块7 (可观测性)**: 遥测日志写入失败 → catch + 静默（不影响主链路）

### 8.4 竞品对齐度

| V1.2 能力 | 竞品覆盖率 | 状态 |
|-----------|-----------|------|
| TTS 多引擎 | Open-LLM-VTuber 20+ / Soul of Waifu 6+ / VPet-Ultra Piper+Kokoro | V1.2 从 1 引擎→4 引擎 |
| ASR 本地引擎 | OL-VTuber sherpa-onnx / VPet-Ultra Whisper | V1.2 从 1 引擎→3 引擎 |
| 主动陪伴 | 6+/11 竞品有 | V1.2 首次引入 |
| 引擎选择 UI | 8/11 竞品有 | V1.2 首次引入 |
| LipSync | Soul of Waifu/Meuxe 有 | V1.1 已交付 |
| 长期记忆 | 9/11 竞品有 | V1.1 已交付 |

---

## 9. 教训候选

- [架构设计] 教训：EdgeTtsService 的 WebSocket→MP3→MediaPlayer 播放链路可作为 TtsProvider 实现样板——降级链的重心不是"让代码能运行所有引擎"，而是"让 Provider 接口足够窄，新引擎接入成本足够低"。标准：新引擎接入仅需实现 `speak(text)` / `stop()` / `release()` 三个方法 + `info` 元数据，播放细节（MediaPlayer/AudioTrack/文件管理）由 Provider 内部封装。建议：第一个 Provider（CosyVoice）实现完成后，用 sherpa-onnx Provider 验证接入成本——若超过 200 行增量代码，接口设计偏重。

- [离线模型管理] 教训：sherpa-onnx 等多平台运行时库的模型下载/校验/版本管理是离线化的隐性成本——不是"下载一个 .onnx 文件"那么简单。标准：ModelDownloadManager 必须覆盖进度通知、断点续传、SHA256 校验、存储空间预检四件事。建议：参考 sherpa-onnx 官方 Android demo（`sherpa-onnx/android/SherpaOnnxTts`）的模型管理实现。

- [可观测性] 教训：V1.1 VoiceIoController 的 `logD` (Log.d) 被国产 ROM (vivo) 抑制导致 LipSync 取证日志全部丢失——关键状态日志必须 Log.i。标准：任何生产环境需要取证观察的关键路径日志（播放状态切换、降级决策、ASR结果）必须使用 Log.i/W/E 级别，仅内部调试细节（帧数据、SSML 内容）可用 Log.d。建议：CI Lint 规则检测 `Log.d` 在 VoiceIoController/TtsProvider/AsrProvider 路径中的使用，强制要求改为 Log.i 或加 `if (BuildConfig.DEBUG)` 守卫。

---

## 10. 附录

### A. N.E.K.O. TtsProvider 注册表模式参考

```python
# _registry_meta.py (简化版)
@dataclass
class TtsProviderMeta:
    name: str
    category: str          # "ws_bistream" | "http_sentence" | "local"
    input_type: str        # "text"
    output_type: str       # "audio_stream" | "audio_file"

# 注册表示例
_providers = {
    "cosyvoice": TtsProviderMeta(name="CosyVoice", category="ws_bistream", ...),
    "gptsovits": TtsProviderMeta(name="GPT-SoVITS", category="local", ...),
    "doubao":    TtsProviderMeta(name="豆包 TTS", category="http_sentence", ...),
}
```

### B. sherpa-onnx Android 集成关键代码片段 (参考)

```kotlin
// sherpa-onnx OfflineTts 初始化
val config = OfflineTtsConfig(
    model = OfflineTtsModelConfig(
        vits = OfflineTtsVitsModelConfig(
            model = "/path/to/model.onnx",
            lexicon = "",
            tokens = "/path/to/tokens.txt",
            dataDir = "",
            dictDir = "",
            noiseScale = 0.667f,
            noiseScaleW = 0.8f,
            lengthScale = 1.0f,
        ),
        numThreads = 4,
        provider = "cpu",
        debug = false,
    ),
    maxNumSentences = 2,
)

val tts = OfflineTts(config)
val audio = tts.generate("你好世界", sid = 0, speed = 1.0f)
// audio.samples: FloatArray, audio.sampleRate: Int
// 写入 WAV 文件 → MediaPlayer 播放
```

### C. DashScope Realtime API 简化调用流程

```
1. HTTP GET wss://dashscope.aliyuncs.com/api-ws/v1/realtime
   Header: Authorization: Bearer {apiKey}
   → 返回 WebSocket URL (含临时 token)

2. WebSocket connect → 发送 start-synthesis 事件
   {
     "header": { "task_id": "...", "event": "start-synthesis" },
     "payload": {
       "model": "cosyvoice-v1",
       "voice": "longxiaochun",
       "text": "你好，我是猫娘~",
       "format": "mp3"
     }
   }

3. 接收流式音频二进制帧 → 合并 → MediaPlayer 播放
   接收 result-generated 事件 → 合成完成
```
