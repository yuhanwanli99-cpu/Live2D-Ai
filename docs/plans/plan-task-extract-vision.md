# plan-task-extract-vision — 视觉插件提取设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 将 `VisionService.kt` 迁出为独立插件 `live2d-ai-plugin-vision`，实现 `Live2DAiPlugin` + `ToolDefinition` + `ChatHook`三合一；core 移除 VisionService.kt
> 前置依赖: Wave 0（`task-plugin-sdk` + `task-chat-refactor`）已完成，`PluginManager`/`ChatService` 均已接入 hook 注册点

---

## 1. 需求概述

### 1.1 背景

`VisionService.kt`（37 行，约 80 行有效代码）通过 Zhipu GLM-4V API 实现截图理解。当前直接放在 core 的 `com.live2d.ai.android` 包下，与 Live2D 渲染管线平级——违反了「极简 core + 插件扩展」的架构原则。

好消息：`VisionService` **没有被任何其他文件 import 或调用**（grep 确认仅定义文件自身 + `PersonaConfig.getVisionModel()` 被它引用），因此迁出是**纯删除+纯新建**，无需修改任何调用点。

### 1.2 迁出后的能力形态

| 能力 | 注册方式 | 何时可用 | 触发时机 |
| ------ | --------- | --------- | --------- |
| 截图上下文注入 | `ChatHook.beforeSend` | Wave 1（立即） | 每次用户发送消息前，自动截图→描述→注入上下文 |
| 按需"看屏幕"工具 | `ToolDefinition("capture_screen")` | Wave 2（待 ChatService 接入 tool-calling loop） | LLM 主动调用 `capture_screen` 工具时执行 |

### 1.3 验收标准

1. Core 移除 `VisionService.kt` 后编译通过，无插件时行为不变（无视觉能力但不崩溃）
2. `app/build.gradle.kts` 添加插件依赖后，`PluginManager.loadPlugins()` 装载 → 截图描述自动注入对话上下文
3. 插件独立仓库 `live2d-ai-plugin-vision` 可独立编译
4. `BuildConfig.ZHIPU_API_KEY` 和 `SettingsRepository.zhipuApiKey` 继续留在 core（作为配置源），仅通过构造函数传入插件

---

## 2. 系统架构

### 2.1 数据流

```
┌──────────────────────────────────────────────────────────┐
│                      Core (app)                          │
│                                                          │
│  MainActivity                                            │
│    │                                                     │
│    ├─ SettingsRepository.zhipuApiKey ───┐                │
│    ├─ PersonaConfig.getVisionModel() ──┐│                │
│    │                                   ││                │
│    └─ PluginManager.loadPlugins([      ││                │
│         VisionPlugin(                  ││                │
│           apiKey       ←──────────────┘│                │
│           visionModel  ←───────────────┘                │
│           screenshotProvider ← AndroidScreenshotProvider │
│         )                                                │
│       ])                                                 │
│         │                                                │
│         ▼  onLoad(host)                                  │
│    ┌──────────────────────────────────┐                  │
│    │ host.registerHook(VisionHook)    │                  │
│    │ host.registerTool(ScreenTool)    │                  │
│    └──────────────────────────────────┘                  │
│         │                                                │
│         ▼  ChatService.sendMessage()                     │
│    ┌──────────────────────────────────┐                  │
│    │ for hook in hooks:               │                  │
│    │   beforeSend(msg, ctx) → 注入     │                  │
│    │     截图描述到上下文              │                  │
│    └──────────────────────────────────┘                  │
└──────────────────────────────────────────────────────────┘
```

### 2.2 组件关系图

```
┌─────────────────────────────┐
│     live2d-ai-plugin-sdk     │  ← 纯 JVM module（无 Android）
│  Live2DAiPlugin / ChatHook  │
│  ToolDefinition / PluginHost│
│  ChatContext / PersonaSnap  │
└──────────┬──────────────────┘
           │ compileOnly (vision plugin 依赖它)
           ▼
┌─────────────────────────────┐
│  live2d-ai-plugin-vision     │  ← Android Library module
│                             │
│  VisionPlugin               │  ← implements Live2DAiPlugin
│  VisionDescribeHook         │  ← implements ChatHook
│  ScreenCaptureTool          │  ← implements ToolDefinition
│  ScreenshotProvider         │  ← interface (定义在插件内)
│  GlmVisionClient            │  ← GLM-4V HTTP 客户端（从 VisionService 迁移）
└──────────┬──────────────────┘
           │ implementation (core app 依赖它)
           ▼
┌─────────────────────────────┐
│       Core (:app)           │
│                             │
│  PluginManager              │
│  MainActivity (构造+装载)    │
│  AndroidScreenshotProvider  │  ← implements ScreenshotProvider
│  SettingsRepository         │  ← 提供 zhipuApiKey
│  PersonaConfig              │  ← 提供 getVisionModel()
│  ChatService (已有 hook 点)  │
└─────────────────────────────┘
```

### 2.3 关键设计决策：截图由 core 提供

**问题**：屏幕截图需要 Android `MediaProjection` / `screencap` 权限，插件要不要自己持有？

**决策**：插件**不持有**截图能力。插件定义 `ScreenshotProvider` 接口（`suspend fun capture(): Bitmap?`），由 core 注入实现。理由：

1. `MediaProjection` 需要 `startActivityForResult` 等 UI 交互，插件作为纯逻辑层不应侵入 Activity
2. core 是唯一知道窗口/上下文的对象，截图实现天然属于 core
3. 未来如果截图来源扩展（如相机、外部设备），插件接口无需改变

---

## 3. 模块划分

### 3.1 插件模块 `live2d-ai-plugin-vision`

| 子模块 | 文件 | 职责 |
| -------- | ------ | ------ |
| **插件入口** | `VisionPlugin.kt` | 实现 `Live2DAiPlugin`，在 `onLoad` 中注册 hook + tool |
| **对话钩子** | `VisionDescribeHook.kt` | 实现 `ChatHook.beforeSend`：截图→GLM-4V 描述→返回上下文字符串 |
| **工具定义** | `ScreenCaptureTool.kt` | 实现 `ToolDefinition`：`capture_screen` 工具，name/description/JSON Schema/execute |
| **视觉客户端** | `GlmVisionClient.kt` | 从现有 `VisionService.kt` 迁移：Bitmap→Base64→HTTP POST→解析响应 |
| **截图接口** | `ScreenshotProvider.kt` | 插件内部接口：`suspend fun capture(): Bitmap?`，由 core 实现 |
| **插件配置** | `VisionPluginConfig.kt` | 数据类：`apiKey` / `visionModel` / `screenshotProvider` |

### 3.2 插件外部依赖

```kotlin
// build.gradle.kts
dependencies {
    compileOnly(project(":plugin-sdk"))          // Live2DAiPlugin 等接口
    implementation("com.squareup.okhttp3:okhttp:4.12.0")    // HTTP 请求
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")  // JSON
    // Android SDK (compileSdk) — 需要 Bitmap / Base64
}
```

### 3.3 Core 侧新增

| 文件 | 职责 |
|------|------|
| `AndroidScreenshotProvider.kt` | 实现 `ScreenshotProvider` 接口，调用 Android screencap |

### 3.4 Core 侧删除

| 文件 | 说明 |
|------|------|
| `VisionService.kt` | 完整迁出，不留存根 |

---

## 4. 数据模型

### 4.1 VisionPluginConfig（插件内部）

```kotlin
package com.live2d.ai.plugin.vision

import android.graphics.Bitmap

/** 截图提供者——插件不自己截图，由宿主注入实现 */
fun interface ScreenshotProvider {
    suspend fun capture(): Bitmap?
}

/** 插件构造参数（从 core 注入） */
data class VisionPluginConfig(
    val apiKey: String,
    val visionModel: String,
    val screenshotProvider: ScreenshotProvider,
)
```

### 4.2 GlmVisionClient（从 VisionService 迁移）

原本 `VisionService` 的方法变为独立的客户端类：

```kotlin
class GlmVisionClient(
    private val apiKey: String,
    private val baseUrl: String = "https://open.bigmodel.cn/api/paas/v4",
) {
    suspend fun describeImage(
        bitmap: Bitmap,
        model: String,
        prompt: String = "请描述这张图片的内容",
    ): String
}
```

变化点：

- `visionModel` 不再从 `PersonaConfig.getVisionModel()` 读取，改为参数注入
- 不再依赖 `BuildConfig.ZHIPU_API_KEY`，改为构造注入
- 其他逻辑（Base64编码、HTTP POST、JSON解析）保持不变

### 4.3 ToolDefinition 参数 JSON Schema

`capture_screen` 工具的参数 schema：

```json
{
  "type": "object",
  "properties": {
    "prompt": {
      "type": "string",
      "description": "分析截图时的自定义提示词，默认简要描述屏幕内容"
    }
  },
  "required": []
}
```

### 4.4 ChatHook 注入格式

`VisionDescribeHook.beforeSend()` 返回的上下文字符串格式：

```
[系统: 当前屏幕内容]
屏幕截图描述: {GLM-4V 返回的描述文本}
```

此字符串由 `ChatService.sendMessage()` 拼接到 `userMessage` 后面（作为 `[插件附加上下文]` 段）。

---

## 5. 接口定义

### 5.1 Live2DAiPlugin 实现（VisionPlugin）

```kotlin
class VisionPlugin(private val config: VisionPluginConfig) : Live2DAiPlugin {
    override val id: String = "vision"
    override val version: String = "1.0.0"

    override fun onLoad(host: PluginHost) {
        // 注册 ChatHook（Wave 1 立即可用）
        host.registerHook(
            VisionDescribeHook(
                client = GlmVisionClient(config.apiKey),
                visionModel = config.visionModel,
                screenshotProvider = config.screenshotProvider,
            )
        )
        // 注册 ToolDefinition（Wave 2 tool-calling 启用后可用）
        host.registerTool(
            ScreenCaptureTool(
                client = GlmVisionClient(config.apiKey),
                visionModel = config.visionModel,
                screenshotProvider = config.screenshotProvider,
            )
        )
    }

    override fun onUnload() {
        // OkHttp client 由 GC 回收，无需显式关闭
    }
}
```

### 5.2 ChatHook 实现（VisionDescribeHook）

```kotlin
class VisionDescribeHook(
    private val client: GlmVisionClient,
    private val visionModel: String,
    private val screenshotProvider: ScreenshotProvider,
) : ChatHook {

    override suspend fun beforeSend(
        userMessage: String,
        context: ChatContext,
    ): String? {
        val bitmap = screenshotProvider.capture() ?: return null
        val description = client.describeImage(bitmap, visionModel, "简要描述当前屏幕内容，50字以内")
        return if (description.isNotEmpty()) {
            "[系统: 当前屏幕内容]\n屏幕截图描述: $description"
        } else null
    }

    override suspend fun afterReceive(assistantMessage: String, emotion: String?) {
        // 视觉插件无需处理 afterReceive
    }
}
```

### 5.3 ToolDefinition 实现（ScreenCaptureTool）

```kotlin
class ScreenCaptureTool(
    private val client: GlmVisionClient,
    private val visionModel: String,
    private val screenshotProvider: ScreenshotProvider,
) : ToolDefinition {

    override val name = "capture_screen"
    override val description = "截取当前屏幕并理解屏幕内容。可用于查看用户正在看什么。"
    override val parametersSchema = """{...}"""  // 见 4.3
    override val requiresConfirmation = false    // 截图无安全风险，无需确认

    override suspend fun execute(argsJson: String): String {
        val prompt = parsePrompt(argsJson) ?: "请描述当前屏幕内容"
        val bitmap = screenshotProvider.capture()
            ?: return """{"error": "无法截取屏幕"}"""
        return client.describeImage(bitmap, visionModel, prompt)
    }
}
```

### 5.4 Core 侧 ScreenshotProvider 实现（AndroidScreenshotProvider）

```kotlin
// 放置在 app 模块中（com.live2d.ai.android）
class AndroidScreenshotProvider(
    private val context: Context,  // applicationContext
) : ScreenshotProvider {
    override suspend fun capture(): Bitmap? = withContext(Dispatchers.IO) {
        // 方案 A：通过 /system/bin/screencap（需要 root 或 shell 权限）
        // 方案 B：通过 MediaProjection API（需要前台权限弹窗）
        // 方案 C：通过 AccessibilityService.takeScreenshot()（Android 11+, 需无障碍权限）
        // 初期返回 null，后续按需实现
        null
    }
}
```

> **注意**：Android 截图需要权限，实现是**独立的后续任务**。插件接口设计已预留 `ScreenshotProvider` 接缝，不影响插件本身的编译和逻辑正确性。当截图未实现时，`capture()` 返回 null → `beforeSend` 返回 null → 对话流程不受影响。

### 5.5 PluginHost 接口（无需变更）

当前 `PluginHost` 已提供 `registerHook` + `registerTool`，VisionPlugin 的注册路径已完全覆盖。**不需要为本次提取扩展 PluginHost**。

---

## 6. 文件清单

### 6.1 新增文件（live2d-ai-plugin-vision 仓库）

| # | 文件路径 | 说明 |
| --- | --------- | ------ |
| 1 | `live2d-ai-plugin-vision/build.gradle.kts` | Android Library 构建配置 |
| 2 | `live2d-ai-plugin-vision/src/main/AndroidManifest.xml` | 空清单（无 Activity/Service） |
| 3 | `live2d-ai-plugin-vision/src/main/java/com/live2d/ai/plugin/vision/VisionPlugin.kt` | Live2DAiPlugin 实现 |
| 4 | `live2d-ai-plugin-vision/src/main/java/com/live2d/ai/plugin/vision/VisionPluginConfig.kt` | 构造参数 + ScreenshotProvider 接口 |
| 5 | `live2d-ai-plugin-vision/src/main/java/com/live2d/ai/plugin/vision/GlmVisionClient.kt` | GLM-4V HTTP 客户端（从 VisionService 迁移） |
| 6 | `live2d-ai-plugin-vision/src/main/java/com/live2d/ai/plugin/vision/VisionDescribeHook.kt` | ChatHook 实现 |
| 7 | `live2d-ai-plugin-vision/src/main/java/com/live2d/ai/plugin/vision/ScreenCaptureTool.kt` | ToolDefinition 实现 |
| 8 | `live2d-ai-plugin-vision/src/test/java/com/live2d/ai/plugin/vision/GlmVisionClientTest.kt` | HTTP 客户端单元测试 |
| 9 | `live2d-ai-plugin-vision/src/test/java/com/live2d/ai/plugin/vision/VisionPluginTest.kt` | 插件注册流程测试 |

### 6.2 修改文件（Core 仓库）

| # | 文件路径 | 变更类型 | 说明 |
| --- | --------- | --------- | ------ |
| 10 | `app/build.gradle.kts` | 修改 | 添加插件依赖 |
| 11 | `app/src/main/java/com/live2d/ai/android/VisionService.kt` | **删除** | 完整移除 |
| 12 | `app/src/main/java/com/live2d/ai/android/MainActivity.kt` | 修改 | `loadPlugins()` 传入 VisionPlugin 实例 |
| 13 | `app/src/main/java/com/live2d/ai/android/AndroidScreenshotProvider.kt` | 新增 | ScreenshotProvider 的空壳实现 |
| 14 | `settings.gradle.kts` | 修改（可选） | 若以 Gradle module 形式引入，需 `include` |

### 6.3 不修改的文件

| 文件 | 原因 |
| ------ | ------ |
| `plugin-sdk/**` | PluginHost 接口已满足需求，无需变更 |
| `PluginManager.kt` | `loadPlugins()` 接受 `List<Live2DAiPlugin>`，已就绪 |
| `ChatService.kt` | beforeSend/afterReceive 钩子点已就绪 |
| `PersonaConfig.kt` | `getVisionModel()` 继续留在 core，作为配置源 |
| `SettingsRepository.kt` | `zhipuApiKey` 继续留在 core，作为配置源 |
| `app/build.gradle.kts` BuildConfig 段 | `ZHIPU_API_KEY` 继续生成（作为配置源传给插件） |
| `ui/settings/SettingsScreen.kt` | Zhipu Key 设置 UI 保留在 core |

---

## 7. 实施顺序

### Phase 1: 插件仓库搭建（并行）

1. **创建插件目录结构**：按 6.1 的文件清单创建 `live2d-ai-plugin-vision/`
2. **编写 `build.gradle.kts`**：Android Library 模块，compileSdk=35，依赖 plugin-sdk
3. **迁移 GlmVisionClient**：从 `VisionService.kt` 复制 HTTP 逻辑，去掉对 `PersonaConfig`/`BuildConfig` 的依赖，改为参数化
4. **实现 VisionPlugin / VisionDescribeHook / ScreenCaptureTool**
5. **编写单元测试**（用 MockWebServer 模拟 GLM-4V 响应 + mock ScreenshotProvider）
6. **独立编译验证**：`./gradlew :live2d-ai-plugin-vision:assemble`

### Phase 2: Core 侧接入

1. **修改 `app/build.gradle.kts`**：添加插件依赖（`implementation(project(":live2d-ai-plugin-vision"))`）
2. **修改 `settings.gradle.kts`**：添加 `include(":live2d-ai-plugin-vision")`
3. **删除 `VisionService.kt`**
4. **新增 `AndroidScreenshotProvider.kt`**（空壳实现，capture() 返回 null）
5. **修改 `MainActivity.kt`**：在 `loadPlugins()` 中构造并传入 VisionPlugin
6. **全量编译验证**：`./gradlew :app:assembleDebug`

### Phase 3: 端到端验证

1. **安装到设备**：验证无 VisionService 时对话流程正常（无崩溃）
2. **验证插件注册**：确认 VisionPlugin.onLoad() 被调用，hook/tool 注册到 PluginManager
3. **截图功能验证**：实现 `AndroidScreenshotProvider.capture()` 后，确认截图描述注入到对话上下文

---

## 8. 风险与注意事项

| # | 风险 | 影响 | 缓解措施 |
| --- | ------ | ------ | --------- |
| R1 | **Android 截图权限复杂**（需 MediaProjection 或 AccessibilityService），`ScreenshotProvider` 可能长期返回 null | "看屏幕"功能暂时不可用，但不影响其他功能 | `ScreenshotProvider` 是接口注入，capture()=null 时 beforeSend 返回 null，ChatService 跳过该 hook——零影响 |
| R2 | **ZHIPU_API_KEY 环境变量在 Android 上不可用**（System.getenv 在 Android app 进程中可能不可靠） | 插件收不到有效 API Key | VisionService 原代码也有此问题（`System.getenv("ZHIPU_API_KEY")`），本次迁出不加剧；建议仅依赖 `BuildConfig` + `SettingsRepository` |
| R3 | **plugin-sdk 是纯 JVM module，vision 插件是 Android Library** | 编译兼容性 | Android Library 可以通过 `compileOnly(project(":plugin-sdk"))` 引用纯 JVM module——已验证可行（plugin-sdk 已独立编译通过） |
| R4 | **每轮对话都截图+调用 GLM-4V** → API 费用和延迟 | 用户感知慢、费用高 | 可后续加节流策略（如每 N 轮截图一次、检测屏幕变化再截图），不在本次范围 |
| R5 | **截图包含敏感信息**（密码、聊天内容）发送到云端 API | 隐私风险 | 作为 TODO 标注：未来可加本地 OCR 预过滤或用户可关闭选项；当前阶段先跑通流程 |

---

## 9. 与 Architecture Plan 的对齐检查

| Architecture Plan 要求 | 本设计是否满足 |
| ------------------------ | -------------- |
| 插件实现为 ToolDefinition（"看屏幕"工具） | ✅ `ScreenCaptureTool` 实现 `ToolDefinition` |
| core 移除 VisionService.kt 直接引用 | ✅ VisionService.kt 完整删除，core 无任何 import |
| 通过 plugin-sdk 依赖注册 | ✅ VisionPlugin 通过 `PluginHost.registerHook/registerTool` 注册 |
| 同进程 Gradle 依赖方式 | ✅ `implementation(project(":live2d-ai-plugin-vision"))` |
| 零侵入：旧模型/无插件时不受影响 | ✅ capture()=null 时 hook 静默跳过，对话流程不变 |

---

## 10. 附录：schema.json 说明

配套文件 `docs/plans/plan-task-extract-vision.schema.json` 定义了插件构造参数的 JSON Schema，用于未来如果插件以独立配置驱动（非硬编码构造）的场景。当前阶段 plugin 通过 Kotlin 构造函数接收参数，schema.json 仅作为接口契约文档。
