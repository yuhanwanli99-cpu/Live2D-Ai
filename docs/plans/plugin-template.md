# plugin-template — Live2D-Ai 插件通用模板/脚手架

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 把 `live2d-ai-plugin-vision`（Wave 1，已验收 PASS）沉淀为可复制的通用模板，
>        Wave 2 的 memory / proactive / agent-tools 插件按同一模板搭骨架（先不实现具体逻辑）
> 理念: 对齐 PiAgent「极简主循环 + 事件驱动扩展」——模板本身只关心"插件长什么样"，
>        不关心插件内部做什么；具体功能留给后续任务填充

---

## 1. 模板的三个约束（照抄 vision 插件即可）

1. **一个插件 = 一个 Gradle module**（`com.android.library`），命名 `live2d-ai-plugin-<name>`
2. **插件只依赖 `plugin-sdk`（`compileOnly` + 测试用 `testImplementation`）**，不依赖 core 的任何类
   （不 import `com.live2d.ai.android.*` 下的任何东西——这是插件与 core 解耦的硬边界）
3. **插件不持有 Android Context 之外的系统权限型能力**（截图、通知、定位等），
   一律通过插件自定义的 `xxxProvider` 函数式接口注入，由 core 提供实现（参考 vision 的 `ScreenshotProvider`）

## 2. 标准文件清单（每个插件仓库都长这样）

```
live2d-ai-plugin-<name>/
├── build.gradle.kts
├── src/main/
│   ├── AndroidManifest.xml              ← 空清单，无 Activity/Service/权限声明
│   └── java/com/live2d/ai/plugin/<name>/
│       ├── <Name>Plugin.kt              ← 实现 Live2DAiPlugin（id/version/onLoad/onUnload）
│       ├── <Name>PluginConfig.kt        ← 构造参数 data class + 所需 xxxProvider 接口
│       ├── <Name>Hook.kt                ← [可选] 实现 ChatHook
│       └── <Name>Tool.kt                ← [可选] 实现 ToolDefinition
└── src/test/java/com/live2d/ai/plugin/<name>/
    └── <Name>PluginTest.kt              ← 至少覆盖：onLoad 注册数量、onUnload 不抛异常、降级行为
```

## 3. build.gradle.kts 模板

```kotlin
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization") // 仅当插件需要 JSON 时保留
}

android {
    namespace = "com.live2d.ai.plugin.<name>"
    compileSdk = 35
    defaultConfig { minSdk = 26 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    testOptions { unitTests.isIncludeAndroidResources = true } // 需要 Robolectric 时开启
}

dependencies {
    compileOnly(project(":plugin-sdk"))
    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    // 按需增加: okhttp / kotlinx-serialization-json / robolectric / mockwebserver
}
```

## 4. `<Name>Plugin.kt` 模板

```kotlin
package com.live2d.ai.plugin.<name>

import com.live2d.ai.pluginsdk.Live2DAiPlugin
import com.live2d.ai.pluginsdk.PluginHost

/**
 * TODO(<name> 插件负责的能力一句话描述)。
 *
 * 骨架阶段：onLoad 注册的 hook/tool 均为 no-op 占位实现，
 * 后续任务在不改变本文件结构的前提下把 TODO 替换为真实逻辑。
 */
class <Name>Plugin(private val config: <Name>PluginConfig) : Live2DAiPlugin {
    override val id: String = "<name>"
    override val version: String = "0.1.0-skeleton"

    override fun onLoad(host: PluginHost) {
        // TODO: host.registerHook(...) / host.registerTool(...)
    }

    override fun onUnload() {
        // TODO: 释放资源（如有）
    }
}
```

## 5. `<Name>PluginConfig.kt` 模板

```kotlin
package com.live2d.ai.plugin.<name>

/**
 * 插件构造参数（从 core 注入）。骨架阶段先留空，
 * 待具体功能任务确定需要哪些配置项/Provider 接口后再补充字段。
 */
data class <Name>PluginConfig(
    val placeholder: Unit = Unit
)
```

## 6. `<Name>PluginTest.kt` 模板（骨架阶段的最小验证）

```kotlin
package com.live2d.ai.plugin.<name>

import com.live2d.ai.pluginsdk.ChatHook
import com.live2d.ai.pluginsdk.PersonaSnapshot
import com.live2d.ai.pluginsdk.PluginHost
import com.live2d.ai.pluginsdk.ToolDefinition
import org.junit.Assert.assertEquals
import org.junit.Test

class <Name>PluginTest {

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {}
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "小喵", systemPrompt = "prompt", model = "model")
    }

    @Test
    fun onLoad_doesNotThrow_andOnUnload_doesNotThrow() {
        val host = FakeHost()
        val plugin = <Name>Plugin(<Name>PluginConfig())
        plugin.onLoad(host)
        plugin.onUnload()
        // 骨架阶段：不断言具体注册数量，只保证生命周期方法可安全调用
    }
}
```

## 7. 骨架 → 功能任务的过渡规则

| 阶段 | 状态 | 谁来做 |
| ------ | ------ | ------ |
| 骨架（本任务） | Gradle module 建好、接口占位、编译通过、最小测试通过 | 本次已交付（见第 8 节） |
| 功能实现（后续任务） | 把 `<Name>PluginConfig` 补上真实字段、`onLoad` 注册真实 hook/tool、补充完整测试 | 交给对应功能的开发/测试子任务，**不改动本模板定的文件结构** |
| core 接入 | 在 `MainActivity.kt` 的 `pluginManager.loadPlugins(listOf(...))` 里加一行 | 功能任务验收通过后再接入，骨架阶段不接入（避免半成品插件影响 app 运行时行为） |

## 8. 本次已交付的骨架（对齐本模板）

| 插件 | Module | 对应 Wave 2 任务 |
| ------ | ------ | ------ |
| `live2d-ai-plugin-memory` | 新建，`MemoryPlugin`/`MemoryPluginConfig` 占位 | `task-plugin-memory` |
| `live2d-ai-plugin-proactive` | 新建，`ProactivePlugin`/`ProactivePluginConfig` 占位 | `task-plugin-proactive` |
| `live2d-ai-plugin-agent-tools` | 新建，`AgentToolsPlugin`/`AgentToolsPluginConfig` 占位 | `task-plugin-agent-tools` |

三个骨架模块已 `include` 进 `settings.gradle.kts`，**尚未**在 `app/build.gradle.kts` / `MainActivity.kt` 中接入
（按第 7 节规则，骨架阶段不接入 core，等功能任务完成验收后再接线）。
