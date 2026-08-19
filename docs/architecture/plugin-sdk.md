# Live2D-Ai 插件 / 扩展 SDK 最小骨架

> 状态：接口草案（未接入构建）
> 目标：先立住“核心不感知插件、插件不碰核心”的边界，后续再逐个实现记忆/视觉/主动陪伴/工具。

## 1. 设计原则

- 核心只依赖接口，不依赖具体插件实现。
- 插件通过编译期 Gradle 依赖挂载，不引入运行时热加载复杂度。
- 危险工具（`requiresConfirmation = true`）必须由 core 统一确认，不能依赖插件自觉。

## 2. 最小接口骨架（Kotlin 草案）

```kotlin
interface Live2DAiPlugin {
    val id: String
    val version: String
    fun onLoad(host: PluginHost)
    fun onUnload()
}

interface PluginHost {
    fun registerHook(hook: ChatHook)
    fun registerTool(tool: ToolDefinition)
    suspend fun sendAssistantMessage(text: String, emotion: String? = null)
}

interface ChatHook {
    suspend fun beforeSend(userMessage: String, context: Any?): String?
    suspend fun afterReceive(assistantMessage: String, emotion: String?)
}

interface ToolDefinition {
    val name: String
    val description: String
    val parametersSchema: String
    val requiresConfirmation: Boolean
    suspend fun execute(argsJson: String): String
}
```

## 3. 建议落地顺序

1. 新建 `Live2D-Ai-Android/plugin-sdk/` 纯 Kotlin 模块，只包含上述接口 + 单元测试。
2. core 侧 `PluginManager` 只做注册表，不实现具体插件。
3. 视觉 / 记忆 / 主动陪伴 / Agent 工具各自独立模块实现这些接口。
4. 先在 PC 端用 MCP 工具验证协议，再决定 Android 端是否引入 MCP transport。

## 4. 不做什么（当前阶段）

- 不做运行时动态加载 / AIDL / Shadow。
- 不做插件市场 / 独立安装包。
- 不把插件逻辑写进 `core/`、`LoopCoordinator`、`VoiceIoController`。
