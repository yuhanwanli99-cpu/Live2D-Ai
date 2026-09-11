# plan-task-local-llm — 本地 LLM 支持架构设计

> 版本: 1.0.0
> 日期: 2026-07-29
> 目标: 在 Android 端支持 Ollama / vLLM 等本地 LLM 后端，可切换云端 DeepSeek
> 约束: 仅设计，不产出实现代码

---

## 1. 需求概述

当前 ChatService 硬编码 DeepSeek API 端点（`https://api.deepseek.com/v1`），无法使用本地 LLM。
本方案在最小改动下实现：

- **Provider 抽象**：统一 DeepSeek（云端）和本地 LLM（Ollama/vLLM）的接口
- **运行时切换**：设置界面选择 Provider，无需重启
- **OpenAI 兼容 API**：Ollama 和 vLLM 均支持 `/v1/chat/completions`，与现有 ChatService 零摩擦
- **SSE 流式响应**：沿用现有 SSE 解析，无协议变更
- **自动降级**：本地不可用时，透明回退 DeepSeek

---

## 2. 系统架构

```
┌──────────────────────────────────────────────────────┐
│                     MainActivity                      │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ MainScreen  │  │ SettingsScreen│  │ SettingsBtn │ │
│  │ (unchanged) │  │   (NEW)      │  │   (NEW)     │ │
│  └──────┬──────┘  └──────┬───────┘  └─────────────┘ │
│         │                │                            │
│         ▼                ▼                            │
│  ┌──────────────────────────────────────────────┐    │
│  │            LLMProviderManager (NEW)           │    │
│  │  ┌──────────────┐  ┌──────────────────────┐  │    │
│  │  │ SharedPrefs  │  │  HealthCheck         │  │    │
│  │  │ persistence  │  │  GET /v1/models      │  │    │
│  │  └──────────────┘  └──────────────────────┘  │    │
│  └──────────────────────┬───────────────────────┘    │
│                         │ provide LLMProvider         │
│                         ▼                             │
│  ┌──────────────────────────────────────────────┐    │
│  │               ChatService                     │    │
│  │  (modified: accept dynamic LLMProvider)       │    │
│  │  POST {baseUrl}/chat/completions              │    │
│  │  SSE streaming (unchanged)                    │    │
│  └──────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────┘
```

### 数据流

```
User taps "发送"
  → MainScreen.sendMessage(text)
    → LLMProviderManager.getActiveProvider()
      → if LOCAL: healthCheck(baseUrl)
        → OK: use LOCAL config
        → FAIL: show Toast("本地 LLM 不可用，已切换至云端"), use DEEPSEEK config
    → ChatService(provider).sendMessage(text)
      → POST {provider.baseUrl}/chat/completions
      → SSE streaming → Flow<String>
    → MainScreen 渲染气泡 + TTS
```

---

## 3. 模块划分

### 3.1 LLMProvider — 数据模型 (NEW)

**职责**: 封装一个 Provider 的全部连接参数。

```kotlin
// 文件: app/.../LLMProvider.kt

enum class ProviderType { DEEPSEEK, LOCAL }

data class LLMProvider(
    val type: ProviderType,
    val baseUrl: String,      // e.g. "http://192.168.1.100:11434/v1"
    val apiKey: String,       // OpenAI-compatible "Bearer" token; "" for Ollama
    val modelName: String,    // e.g. "qwen2.5:7b", "llama3.1:8b"
    val displayName: String   // UI label, e.g. "本地 Ollama (Qwen2.5)"
)
```

**预置 Provider**:

| type | baseUrl | apiKey | modelName | displayName |
|------|---------|--------|-----------|-------------|
| DEEPSEEK | `https://api.deepseek.com/v1` | `BuildConfig.DEEPSEEK_API_KEY` | `PersonaConfig.getModel()` | DeepSeek 云端 |

本地 Provider 由用户在设置界面输入，持久化到 SharedPreferences。

### 3.2 LLMProviderManager — 运行时管理 (NEW)

**职责**: Provider 选择、持久化、健康检查、降级逻辑。

```kotlin
// 文件: app/.../LLMProviderManager.kt

object LLMProviderManager {
    // ── 持久化键 ──
    private const val PREFS_NAME = "llm_provider"
    private const val KEY_TYPE = "provider_type"        // "DEEPSEEK" | "LOCAL"
    private const val KEY_LOCAL_URL = "local_base_url"  // "http://..."
    private const val KEY_LOCAL_KEY = "local_api_key"   // "" if none
    private const val KEY_LOCAL_MODEL = "local_model"   // "qwen2.5:7b"

    // ── API ──
    fun init(context: Context)                            // 冷启动加载
    fun getActiveProvider(): LLMProvider                  // 当前选中的 Provider
    fun getDeepSeekProvider(): LLMProvider                // 内置云端 Provider
    fun getLocalProvider(): LLMProvider?                  // 用户配置的本地 Provider (可能为 null)
    fun saveLocalConfig(url: String, key: String, model: String)  // 持久化本地配置
    fun setProviderType(type: ProviderType)               // 切换 Provider
    suspend fun healthCheck(url: String): HealthResult    // 连通性检查
}

sealed class HealthResult {
    object Ok : HealthResult()
    data class Error(val message: String, val code: Int?) : HealthResult()
}
```

**降级策略**:

- `getActiveProvider()` 返回 LOCAL 时，调用方在执行请求前先调 `healthCheck()`
- 健康检查失败 → 输出 Toast，`getActiveProvider()` 内部自动回退为 DEEPSEEK（仅本次，不修改持久化偏好）
- 或者提供 `getActiveProviderWithFallback()` 直接返回降级后的 Provider

### 3.3 ChatService — 适配动态 Provider (MODIFY)

**现状**: 构造函数硬编码 `baseUrl = "https://api.deepseek.com/v1"`，`apiKey` 从 `BuildConfig` 读取。

**改动**（向后兼容）:

```kotlin
// 文件: app/.../ChatService.kt

class ChatService(
    private val provider: LLMProvider  // NEW: 主构造参数
) {
    // 兼容旧代码的次级构造函数
    constructor(apiKey: String, baseUrl: String) : this(
        LLMProvider(
            type = ProviderType.DEEPSEEK,
            baseUrl = baseUrl,
            apiKey = apiKey,
            modelName = "deepseek-chat",
            displayName = "DeepSeek"
        )
    )

    // 内部使用 provider.baseUrl / provider.apiKey / provider.modelName
    // sendMessage() 中 modelName 来源：provider.modelName ?: PersonaConfig.getModel()
}
```

**关键**: SSE 解析逻辑完全不变 — Ollama/vLLM 的 OpenAI-compatible 端点返回格式与 DeepSeek 完全一致：

```
data: {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"喵~"}}]}
```

### 3.4 SettingsScreen — 设置界面 (NEW)

**职责**: Compose UI，配置本地 LLM 参数。

**布局设计**:

```
┌─────────────────────────────────────┐
│  ← 返回          LLM 设置           │
├─────────────────────────────────────┤
│                                     │
│  当前 Provider                      │
│  ┌─────────────────────────────┐   │
│  │ ○ DeepSeek 云端              │   │
│  │ ○ 本地 LLM (Ollama/vLLM)    │   │
│  └─────────────────────────────┘   │
│                                     │
│  ── 本地 LLM 配置 ──               │
│                                     │
│  Base URL                          │
│  ┌─────────────────────────────┐   │
│  │ http://192.168.1.100:11434  │   │
│  └─────────────────────────────┘   │
│                                     │
│  API Key (可选)                    │
│  ┌─────────────────────────────┐   │
│  │ ollama 通常无需 key         │   │
│  └─────────────────────────────┘   │
│                                     │
│  Model Name                        │
│  ┌─────────────────────────────┐   │
│  │ qwen2.5:7b                  │   │
│  └─────────────────────────────┘   │
│                                     │
│  ┌──────────────┐                  │
│  │  测试连接     │                  │
│  └──────────────┘                  │
│  ✓ 连接成功！发现模型: qwen2.5:7b  │
│                                     │
│  ┌──────────────────────────────┐  │
│  │          保 存                │  │
│  └──────────────────────────────┘  │
└─────────────────────────────────────┘
```

**交互流程**:

1. 选择 Provider 类型（RadioButton）
2. 填写 Base URL / API Key / Model Name（仅 LOCAL 时启用）
3. 点"测试连接" → `LLMProviderManager.healthCheck(url)` → 显示结果
4. 点"保存" → `LLMProviderManager.saveLocalConfig(...)`

### 3.5 MainActivity / MainScreen — 集成 (MODIFY)

**改动点**:

1. `onCreate` 中: `LLMProviderManager.init(this)`
2. `MainScreen` 顶部栏增加设置齿轮图标 → 导航到 `SettingsScreen`
3. `sendMessage` 中: Provider 降级逻辑

```kotlin
// MainActivity.onCreate()
LLMProviderManager.init(this)
chatService = ChatService(LLMProviderManager.getActiveProvider())

// MainScreen 发消息前降级检查
val provider = LLMProviderManager.getActiveProvider()
if (provider.type == ProviderType.LOCAL) {
    val health = LLMProviderManager.healthCheck(provider.baseUrl)
    if (health !is HealthResult.Ok) {
        // Toast + 降级到 DeepSeek
        chatService = ChatService(LLMProviderManager.getDeepSeekProvider())
    }
}
chatService.sendMessage(text).collect { ... }
```

---

## 4. 数据模型

### 4.1 SharedPreferences Schema

| Key | Type | Default | Description |
| ----- | ------ | --------- | ------------- |
| `provider_type` | String | `"DEEPSEEK"` | 当前选中的 Provider 类型 |
| `local_base_url` | String | `""` | 本地 LLM Base URL |
| `local_api_key` | String | `""` | 本地 LLM API Key（可为空） |
| `local_model` | String | `""` | 本地 LLM 模型名 |

### 4.2 Provider 状态机

```
                    ┌──────────┐
         app start  │ DEEPSEEK │ (default)
         ─────────► │  (active) │
                    └─────┬────┘
                          │ user selects LOCAL in settings
                          ▼
                    ┌──────────┐
                    │  LOCAL   │
                    │ (active) │
                    └─────┬────┘
                          │ health check fails on send
                          ▼ (transient fallback)
                    ┌──────────┐
                    │ DEEPSEEK │ (this request only)
                    │ (fallback)│
                    └─────┬────┘
                          │ next request: retry LOCAL
                          ▼
                    ┌──────────┐
                    │  LOCAL   │
                    │ (active) │ ← if still down, fallback again
                    └──────────┘
```

**设计决策**: 降级是临时的（per-request），不修改持久化偏好。用户下次发送消息时会重新尝试本地 LLM。
这避免了"本地 LLM 暂时不可用 → 永久切换回云端"的糟糕体验。

---

## 5. 接口定义

### 5.1 ChatService（修改后）

```
class ChatService(provider: LLMProvider)
  - sendMessage(userText: String): Flow<String>   // 不变
  - resetConversation(): Unit                      // 不变
  - updateProvider(provider: LLMProvider): Unit    // NEW: 运行时切换 Provider
```

### 5.2 LLMProviderManager

```
object LLMProviderManager
  + init(context: Context): Unit
  + getActiveProvider(): LLMProvider
  + getDeepSeekProvider(): LLMProvider
  + getLocalProvider(): LLMProvider?
  + saveLocalConfig(url: String, key: String, model: String): Unit
  + setProviderType(type: ProviderType): Unit
  + healthCheck(url: String): HealthResult        // suspend
  + getActiveProviderWithFallback(): LLMProvider   // suspend, auto-fallback
```

### 5.3 SettingsScreen

```
@Composable fun SettingsScreen(
    onBack: () -> Unit
)
```

### 5.4 健康检查协议

```
GET {baseUrl}/models
  → 200 OK: HealthResult.Ok
  → 其他: HealthResult.Error(code, body)
  → 超时 (5s): HealthResult.Error("连接超时", null)

备选（wake word 更轻量）:
HEAD {baseUrl}/
  → 200: Ok
```

---

## 6. 文件清单

### 新增文件

| 文件 | 职责 |
| ------ | ------ |
| `app/src/main/java/com/live2d/ai/android/LLMProvider.kt` | `ProviderType` 枚举 + `LLMProvider` 数据类 |
| `app/src/main/java/com/live2d/ai/android/LLMProviderManager.kt` | Provider 管理、持久化、健康检查、降级 |
| `app/src/main/java/com/live2d/ai/android/ui/SettingsScreen.kt` | 设置界面 (Compose) |

### 修改文件

| 文件 | 改动范围 |
| ------ | ---------- |
| `app/src/main/java/com/live2d/ai/android/ChatService.kt` | 构造函数接受 `LLMProvider`、`updateProvider()`、移除硬编码 URL |
| `app/src/main/java/com/live2d/ai/android/MainActivity.kt` | 初始化 LLMProviderManager、导航到 SettingsScreen、发消息前降级检查 |
| `app/build.gradle.kts` | **无改动**（OkHttp、kotlinx-serialization 已就绪） |

---

## 7. 实施顺序

### Phase 1: 数据层（无 UI 变更）

1. 创建 `LLMProvider.kt` — 纯数据模型，零依赖
2. 创建 `LLMProviderManager.kt` — SharedPreferences 读写 + 健康检查
3. 修改 `ChatService.kt` — 接受 `LLMProvider`，添加工厂方法兼容旧调用
4. **验证**: 单元测试 `LLMProviderManager` 读写 + `ChatService` 使用自定义 URL

### Phase 2: UI 集成

1. 创建 `ui/SettingsScreen.kt` — Compose 设置界面
2. 修改 `MainActivity.kt` / `MainScreen` — 增加设置入口 + 降级逻辑
3. **验证**: 完整流程 — 配置本地 LLM URL → 发送消息 → 收到回复

### Phase 3: 边界情况

1. 添加降级 Toast 提示
2. 处理 OkHttp 超时（局域网硬件响应慢，适当放宽 readTimeout）
3. 测试：WiFi 断开、本地 LLM 关机、Base URL 格式错误

---

## 8. 风险与注意事项

### 8.1 Android 网络安全

**问题**: Android P (API 28+) 默认禁止明文 HTTP 流量。本地 LLM（Ollama）通常运行在 `http://192.168.x.x:11434`。

**方案**:

- 在 `AndroidManifest.xml` 中添加 `android:usesCleartextTraffic="true"`（已有，因为项目 minSdk=26）
- 或在 `res/xml/network_security_config.xml` 中精确放行局域网 IP 段 `192.168.0.0/16`、`10.0.0.0/8`
- **推荐**: 精确放行，安全最佳实践

### 8.2 局域网延迟

本地 LLM 在消费级硬件上推理延迟可能 5-30 秒/token，远超云端 API。

**对策**:

- OkHttp `readTimeout` 对本地 Provider 放宽至 300s（已有 ChatService 默认 120s，可能需要按 Provider 区分）
- 设置界面提示用户预期延迟

### 8.3 模型名不一致

Ollama 模型名格式: `qwen2.5:7b`，vLLM 格式: `Qwen/Qwen2.5-7B-Instruct`。

**对策**: 健康检查时调用 `GET /v1/models`，返回可用模型列表供用户下拉选择（Phase 2 增强）。

### 8.4 API Key 为空时 Headers

Ollama 不需要 API Key，但 DeepSeek 必须。若 `apiKey` 为空字符串：

```kotlin
// ChatService 中
if (provider.apiKey.isNotEmpty()) {
    request.addHeader("Authorization", "Bearer ${provider.apiKey}")
}
```

### 8.5 现有测试兼容

`ChatService` 构造函数签名变更 → 需要更新单元测试中的 `ChatService(apiKey, baseUrl)` 调用 → 使用次级构造函数或 `LLMProvider` 工厂方法。

### 8.6 DeepSeek API Key 保留

`BuildConfig.DEEPSEEK_API_KEY` 继续用于云端 Provider，不受本地 LLM 配置影响。
本地 LLM 的 API Key 由用户手动输入，存储在 SharedPreferences（非加密，安全等级同 `local.properties`）。

---

## 9. 设计决策记录

| 决策 | 选项 A | 选项 B | 选择 | 理由 |
| ------ | -------- | -------- | ------ | ------ |
| Provider 存储 | SharedPreferences | Room/DataStore | **SharedPreferences** | 配置项仅 4 个 KV，无需数据库 |
| 健康检查时机 | 后台定时轮询 | 发消息前按需检查 | **发消息前按需检查** | 无后台开销，失败后果可控 |
| 降级持久性 | 永久切换 | 仅本次请求 | **仅本次请求** | 本地 LLM 暂时不可用不应改变用户偏好 |
| Setting UI 位置 | Dialog | 独立 Screen | **独立 Screen** | 设置项较多，Dialog 空间不足 |
| UI 目录 | 顶级包 | `ui/` 子包 | **`ui/` 子包** | 为后续更多 Screen 预留组织空间 |
| Llama.cpp 支持 | 纳入设计 | 暂不纳入 | **暂不纳入** | Llama.cpp 无标准 OpenAI 端点，需独立适配层 |
