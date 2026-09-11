# plan-task-plugin-architecture — Core/插件化架构设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 参考 PiAgent「极简核心 + 事件驱动扩展」理念，把 Live2D-Ai 拆分为「精简 core（Live2D 渲染 + 基础对话闭环）」+「独立插件仓库」
> 约束: 仅设计与任务拆分，供交给开发团队/子 agent 执行；本文档不直接改动源码

---

## 1. 背景与理念对齐

PiAgent 核心理念：**极简主循环 + 事件驱动扩展**。核心 Runtime 只保留"LLM + 工具 + 循环"，一切复杂能力通过 Extension（事件钩子 + 工具注册 + 中间件）挂载，不进主循环。

Live2D-Ai 对应落地：
- **Core（本仓库，`Live2D-Ai-Android`）**：只保留"Live2D 渲染 + 单轮语音/文字对话闭环"，代码量、依赖、权限保持最小。
- **插件（独立仓库）**：记忆系统、主动陪伴、Agent 工具执行、多供应商扩展等，全部通过标准协议对接 core，core 不感知插件内部实现。

---

## 2. Core 最小功能边界

### 2.1 保留在 core 的模块（现有文件）

| 文件 | 职责 | 保留原因 |
| ------ | ------ | ---------- |
| `Live2DNative.kt` / `Live2DRenderer.kt` / `Live2DView.kt` / `PurismModel.kt` / `PurismClippingManager.kt` / `ShaderManager.kt` | Live2D 渲染管线（Purism Core） | 项目根本差异化能力，不可插件化（性能敏感、JNI 强耦合） |
| `AnimationSystem.kt` | 动作/物理驱动 | 渲染层的一部分 |
| `LLMProvider.kt` / `LLMProviderManager.kt` | 单轮 LLM 流式对话（DeepSeek 等） | 对话闭环最小必需品 |
| `EdgeTtsService.kt` | TTS 合成 | 对话闭环最小必需品 |
| `VoiceInputService.kt` | ASR 语音输入 | 对话闭环最小必需品 |
| `EmotionController.kt` | 解析 `[emotion]` 标签驱动表情 | 渲染与对话的最小粘合层 |
| `PersonaConfig.kt` | 加载 `shared/persona.yaml` | 人设是 core 的最小状态 |
| `ChatService.kt` | 对话主循环编排（收发消息、状态机） | 相当于 PiAgent 的 "core loop"，需要重构为**可挂钩子**的循环 |
| `MainActivity.kt` / `FloatingWindowService.kt` / `ui/` | UI 与桌宠悬浮窗宿主 | 应用外壳 |

### 2.2 拆出为插件的模块

| 文件/能力 | 目标插件仓库 | 拆分方式 |
| ------ | ------ | ------ |
| `VisionService.kt`（GLM-4.6V 截图理解） | `live2d-ai-plugin-vision` | 通过 `ToolPlugin` 接口注册一个"看屏幕"工具 + 一个 `BeforeSendHook`（可选：在用户提问前附加截图描述） |
| 记忆系统（五维记忆，当前未实现） | `live2d-ai-plugin-memory` | 通过 `ContextProviderHook`：在每次请求前注入记忆摘要；`onMessage` 事件后写入新记忆 |
| 主动陪伴（定时/事件触发主动搭话，当前未实现） | `live2d-ai-plugin-proactive` | 通过 `SchedulerPlugin`：注册后台触发器，触发时调用 core 暴露的 `sendAssistantMessage()` API |
| Agent 工具执行（操作手机/浏览器，当前未实现） | `live2d-ai-plugin-agent-tools` | 通过 `ToolPlugin` 接口注册可被 LLM 调用的工具（如打开 App、点击坐标），中间件拦截危险操作需用户确认 |
| `LLMProviderManager.kt` 中除默认 DeepSeek 外的供应商 | 可选下沉为 `live2d-ai-plugin-provider-*` | 通过 `ProviderPlugin` 接口注册新的 LLM/TTS/ASR 供应商 |

---

## 3. 插件对接协议设计

### 3.1 通信方式：进程内 Kotlin 接口（非跨进程）

- 决策：Android 端插件优先采用**同进程动态加载**（非独立 APK/AIDL），原因：
  1. 语音/渲染对延迟敏感，跨进程 IPC 增加复杂度和延迟
  2. 个人项目阶段，插件数量少，先验证协议，不过早引入 AIDL 复杂度
  3. 未来插件数量增多、需要独立安装/卸载时，再升级为「独立 APK + AIDL/ContentProvider」，协议接口设计需向前兼容
- 插件以 **Gradle 依赖（aar/jar）** 形式引入 core app，在 `Application.onCreate()` 阶段注册

### 3.2 核心接口（`live2d-ai-plugin-sdk` 模块，需新建）

```kotlin
// 插件生命周期与身份
interface Live2DAiPlugin {
    val id: String                 // 插件唯一标识，如 "vision"
    val version: String
    fun onLoad(host: PluginHost)   // core 启动时调用，注入 host 引用
    fun onUnload()
}

// core 暴露给插件的能力（宿主接口）
interface PluginHost {
    fun registerHook(hook: ChatHook)
    fun registerTool(tool: ToolDefinition)
    fun sendAssistantMessage(text: String, emotion: String? = null) // 供主动陪伴插件调用
    fun getPersona(): PersonaConfig
}

// 事件钩子（对应 PiAgent 的 Extension 事件监听）
interface ChatHook {
    suspend fun beforeSend(userMessage: String, context: ChatContext): String? // 返回非空则替换/追加上下文
    suspend fun afterReceive(assistantMessage: String, emotion: String?)       // 记忆写入等
}

// 工具注册（对应 PiAgent 的工具注册 + 中间件拦截）
interface ToolDefinition {
    val name: String
    val description: String
    val parametersSchema: String   // JSON Schema
    suspend fun execute(argsJson: String): String
    val requiresConfirmation: Boolean  // 危险操作需用户确认（中间件拦截点）
}
```

### 3.3 `ChatService.kt` 重构要求

- 现有 `ChatService` 需重构为持有 `List<ChatHook>` 和 `List<ToolDefinition>`（通过 `PluginHost` 注册）
- 对话主循环顺序：`beforeSend hooks → LLM 调用（含工具调用循环）→ afterReceive hooks → TTS/表情`
- 工具调用：LLM 返回 tool_call 时，查表 `ToolDefinition`，若 `requiresConfirmation` 则先弹窗确认

---

## 4. 任务拆分（交给开发团队/子 agent 执行）

### Wave 0（并行，无依赖）

| 任务 ID | 任务 | 产出 |
| ------ | ------ | ------ |
| `task-plugin-sdk` | 新建 `live2d-ai-plugin-sdk` Gradle module，定义第 3.2 节接口（`Live2DAiPlugin`/`PluginHost`/`ChatHook`/`ToolDefinition`） | 新 module + 接口源码 + KDoc |
| `task-chat-refactor` | 重构 `ChatService.kt`：拆出 hook/tool 注册点，主循环改为 `beforeSend → LLM(+tool loop) → afterReceive` | 修改后的 `ChatService.kt` + 单元测试（mock hook 验证调用顺序） |

### Wave 1（依赖 Wave 0）

| 任务 ID | 任务 | 产出 |
| ------ | ------ | ------ |
| `task-extract-vision` | 将 `VisionService.kt` 迁出到新仓库 `live2d-ai-plugin-vision`，实现为 `ToolDefinition`（"看屏幕"工具）；core 中移除 `VisionService.kt` 直接引用，改为通过 `plugin-sdk` 依赖注册 | 新仓库 + core 端接入验证（能通过插件跑通截图理解） |

### Wave 2（依赖 Wave 1，验证通过后再排期）

采用 MVP 策略：先按 `docs/plans/plugin-template.md` 交付**统一骨架**（三个 module 均可编译、无功能、无副作用），再分别排期功能实现任务。

| 任务 ID | 任务 | 状态 |
| ------ | ------ | ------ |
| `task-plugin-template` | 提炼 `docs/plans/plugin-template.md` 骨架模板（module 结构/build.gradle.kts/Plugin+Config+Test 代码模板/骨架→功能→接入 core 的转换规则） | ✅ 已交付 |
| `task-plugin-memory-skeleton` | 按模板生成 `live2d-ai-plugin-memory` 骨架（no-op `onLoad`/`onUnload`，占位 Config，骨架测试） | ✅ 已交付 |
| `task-plugin-proactive-skeleton` | 按模板生成 `live2d-ai-plugin-proactive` 骨架 | ✅ 已交付 |
| `task-plugin-agent-tools-skeleton` | 按模板生成 `live2d-ai-plugin-agent-tools` 骨架 | ✅ 已交付 |
| `task-plugin-memory` | 按 `ChatHook`（`beforeSend` 注入记忆摘要 / `afterReceive` 写入）实现记忆插件真实功能 | ⏳ 待排期 |
| `task-plugin-proactive` | 实现调度器插件，调用 `PluginHost.sendAssistantMessage()` | ⏳ 待排期 |
| `task-plugin-agent-tools` | 实现 `ToolDefinition` 形式的手机操作工具，`requiresConfirmation = true` | ⏳ 待排期 |

---

## 5. 验收标准

1. Core app 移除 `VisionService.kt` 后仍可正常编译运行（无插件时降级为无视觉能力，而非崩溃）
2. 安装 `live2d-ai-plugin-vision` 依赖后，"看屏幕"功能通过 `ToolDefinition` 注册生效，效果与迁出前一致
3. `ChatHook` 的 `beforeSend`/`afterReceive` 调用顺序有单元测试覆盖
4. 危险工具（`requiresConfirmation = true`）执行前必须弹出确认 UI，无法绕过

## 6. 可复用开源项目调研

| 需求 | 推荐 | 说明 |
| ------ | ------ | ------ |
| **工具调用协议** | [MCP（Model Context Protocol）](https://github.com/modelcontextprotocol) + 官方 [Kotlin SDK](https://github.com/modelcontextprotocol/kotlin-sdk) | 桌面端 `shared/mcp_tools.json` / `Live2D-Ai-pc` 已经用 MCP 生态（time/ddg-search/weather/filesystem/sequential-thinking，均通过 `uvx` 启动）。`ToolDefinition`（第 3.2 节）字段已按 MCP Tool 结构对齐（name/description/JSON Schema/文本结果），但**不直接引入 MCP SDK 的 transport 层**——因为 MCP 服务器多是 `uvx` 启动的 Python 子进程，Android 无法直接跑；Android 侧工具本质是"本地能力"（截屏、读通知等），只借用 MCP 的描述结构，方便未来打通两端。 |
| **记忆系统** | ❌ 不建议直接引入 [mem0](https://github.com/mem0ai/mem0) | mem0 只有 Python/TypeScript SDK，无原生 Kotlin/Android 支持；接入需经 Node/Python 运行时，对手机端过重。建议 `live2d-ai-plugin-memory` 自建：Room/SQLite 存储 + 简单摘要策略（近期 N 轮 + 定期用 LLM 生成摘要写入"事实记忆"表），比照 N.E.K.O. 的五维记忆分层设计但只做 2-3 层（近期/事实/人格）即可满足个人项目需求。 |
| **插件加载机制** | ✅ 当前采用「编译期 Gradle module 依赖」（同进程） | 调研了 Android 传统插件化框架 RePlugin（360）/ Shadow（腾讯）/ VirtualAPK（滴滴）：均基于 Hook ClassLoader/AMS 实现运行时热加载，优点是可独立分发插件 APK，但对新版本 Android（10+/12+）兼容性风险高、实现复杂，对个人项目投入产出比低。**Google 官方 Dynamic Feature Modules** 更安全，但主要用于按需下载减包，且强依赖 Google Play 分发，不适合纯开源自分发场景。→ 现阶段维持编译期依赖的最简方案；若未来插件数量增多、需要用户自由安装/卸载第三方插件，再评估升级到 Shadow（安全性和 Kotlin 友好度最好）。 |
| **VTuber 渲染/协议参考** | [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber)（已用于 `Live2D-Ai-pc`） | 桌面端已复用；可参考其 WebSocket 消息协议设计（如 `type: control/audio/text`）作为未来"Android core 暴露本地 API 给外部插件/设备"的协议参考，但本阶段不需要引入。 |
| **主动陪伴调度** | ✅ AndroidX `WorkManager`（官方，非第三方） | 无需额外开源依赖，`live2d-ai-plugin-proactive` 内部用 `WorkManager` 定期触发 + 通过 `PluginHost.sendAssistantMessage()` 回调即可，避免引入自定义 Service 常驻带来的电量/后台限制问题。 |

## 7. 骨架代码交付状态

Wave 0 的 `task-plugin-sdk` 与 `task-chat-refactor` 已提前落地骨架（供团队直接在此基础上继续开发，而非从零开始）：

| 文件 | 状态 |
| ------ | ------ |
| `Live2D-Ai-Android/plugin-sdk/`（新 Gradle module） | ✅ 已创建，接口定义完成（`Live2DAiPlugin`/`PluginHost`/`ChatHook`/`ToolDefinition`），已通过独立编译验证 |
| `Live2D-Ai-Android/app/.../PluginManager.kt` | ✅ 已创建，`PluginHost` 的默认实现 + 插件注册表 |
| `Live2D-Ai-Android/app/.../ChatService.kt` | ✅ 已重构，接入 `beforeSend`/`afterReceive` 钩子调用点，向后兼容（无插件时行为不变，已有单元测试无需修改） |
| `Live2D-Ai-Android/app/.../MainActivity.kt` | ✅ 已接入 `PluginManager`（当前 `loadPlugins(emptyList())`，接真实插件时改这一行即可） |
| `ToolDefinition` 的请求/响应循环（工具调用）接入 `ChatService` | ⏳ 未做，留给 `task-plugin-agent-tools`（Wave 2），需要先扩展 `ApiModels.kt` |
| `live2d-ai-plugin-vision` / `live2d-ai-plugin-memory` / `live2d-ai-plugin-proactive` / `live2d-ai-plugin-agent-tools` 具体实现 | ⏳ 未做，按 Wave 1/2 任务表交给开发团队 |

## 8. 风险与注意事项

| # | 风险 | 缓解 |
| --- | ------ | ------ |
| R1 | 同进程插件无法独立卸载/热更新 | 先验证协议可行性，Wave 2 后再评估是否升级为独立 APK + AIDL |
| R2 | `ChatService` 重构可能引入并发问题（多 hook 同时改上下文） | hook 按注册顺序串行执行，`beforeSend` 返回值链式叠加，不允许并行修改共享状态 |
| R3 | 插件 API 尚不稳定，过早固化影响后续插件开发 | `live2d-ai-plugin-sdk` 单独 module + 独立版本号，允许非破坏性演进（新增接口默认方法） |
