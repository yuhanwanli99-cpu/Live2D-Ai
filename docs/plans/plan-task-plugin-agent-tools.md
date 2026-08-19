# plan-task-plugin-agent-tools — Agent 工具执行插件设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 把骨架 `live2d-ai-plugin-agent-tools`（`docs/plans/test-task-plugin-skeletons.md` 已验收）落地为可操作手机的 `ToolDefinition` 集合
> 前置依赖: `task-plugin-agent-tools-skeleton`（已完成）；**硬依赖**：`ChatService` 尚未实现 tool-calling 循环（当前只有 hook 循环），本任务必须先确认/补齐 tool-call 接入点，否则工具注册了也不会被调用
> 不做的事: 不接入 core，功能验收通过后再排期接入

---

## 1. 需求概述

### 1.1 背景

对照 N.E.K.O. 的"Agent 工具执行"能力（能替用户操作手机，如打开 App、发消息）。这是三个 Wave 2 插件中**风险最高**的一个——涉及真实设备操作，必须把"确认机制"作为第一优先级设计，而不是事后补充。

### 1.2 MVP 范围内的工具

| 工具名 | 功能 | `requiresConfirmation` |
| ------ | ------ | ------ |
| `open_app` | 打开指定包名的 App（如"帮我打开微信"） | `false`（只是切前台，无数据风险） |
| `send_notification_reply` | 通过 `NotificationListenerService` 快捷回复消息 | `true`（会代替用户发出真实消息，必须确认） |
| `set_alarm` | 通过系统 `AlarmClock` Intent 设置闹钟/提醒 | `false`（系统会弹出确认页，天然有二次确认） |

**不做**（本 MVP 明确排除，避免范围蔓延）：模拟点击/无障碍自动化操作（`AccessibilityService` 全量操作）、读取通讯录/短信、任何涉及支付/转账的操作——这些风险级别更高，留到后续单独评估。

### 1.3 验收标准

1. `requiresConfirmation = true` 的工具，在 `execute()` 被真正调用前，**必须**有 core 侧的确认 UI 拦截，用户拒绝时 `execute()` 不会被调用（而不是"调用了但假装失败"——两者对安全审计的意义完全不同）
2. 三个工具各自的 `parametersSchema` 是合法 JSON Schema，且 `execute()` 对非法/缺失参数有清晰的错误信息返回（不是崩溃）
3. 每个工具的真实设备操作都通过 Android 官方 API/Intent（不使用 root/shell/无障碍等灰色手段）
4. 单元测试覆盖参数校验 + 确认流程的"未确认不执行"

---

## 2. 系统架构

### 2.1 确认机制是谁的职责？—— 关键架构决策

**问题**：`ToolDefinition.requiresConfirmation` 只是一个标记字段（见 `plugin-sdk/ToolDefinition.kt`），谁来真正弹确认框、谁来拦截？

**决策**：确认 UI 是 **core 的职责**，不是插件的职责。原因：

1. 弹窗需要 `Activity`/`Compose` UI 上下文，插件不应该持有 Activity 引用（否则内存泄漏风险，且违反"插件是纯逻辑层"的架构原则，与 vision 插件的 `ScreenshotProvider` 设计原则一致）
2. 确认逻辑应该在**所有工具**上统一生效，不能依赖每个插件自己老老实实检查——如果确认逻辑分散在各插件里，只要有一个插件"忘记检查"，硬约束就形同虚设

**因此本任务包含一处 core 侧改动**（不只是插件本身）：`ChatService`（或其 tool-calling 循环，见 2.2）在执行任何 `ToolDefinition.execute()` 之前，统一检查 `tool.requiresConfirmation`，为 true 则先调用一个新的 `PluginHost` 之外的 core 内部确认回调（`suspend fun confirmToolExecution(tool: ToolDefinition, argsJson: String): Boolean`），返回 false 直接跳过 `execute()`，把"用户拒绝执行"作为工具结果反馈给 LLM。

### 2.2 前置阻塞项：ChatService 还没有 tool-calling 循环

`docs/plans/plan-task-plugin-architecture.md` §4.3 已明确记录：

> `ToolDefinition` 的请求/响应循环（工具调用）接入 `ChatService` ⏳ 未做，留给 `task-plugin-agent-tools`（Wave 2），需要先扩展 `ApiModels.kt`

也就是说，本任务的范围**不只是写三个工具**，还包括：

1. 扩展 `ApiModels.kt`：请求体加 `tools` 字段（工具列表转成 LLM API 要求的 JSON Schema 格式），响应解析加 `tool_calls` 字段
2. 扩展 `ChatService`：收到 `tool_calls` 后，查 `PluginManager` 注册的 `ToolDefinition` 表，按 2.1 的确认流程执行，把结果作为 `tool` role 消息追加，再次请求 LLM 生成最终回复（标准 OpenAI-兼容 function-calling 循环）

这部分是 **core 侧改动**，建议作为本任务的**子任务 0**先行落地并单独验收（因为它是 vision 插件"Wave 2 待接入"能力和本插件共同的前置依赖，参见 `plan-task-extract-vision.md` §1.2 表格），再叠加 agent-tools 具体工具实现。

### 2.3 整体数据流

```
LLM 返回 tool_calls: [{name: "open_app", args: {...}}]
        │
        ▼
ChatService 查表 → 找到 OpenAppTool (ToolDefinition)
        │
        ▼
requiresConfirmation? 
  false → 直接 execute()
  true  → core 弹确认 UI → 用户同意? 
             是 → execute()
             否 → 返回 "用户拒绝执行" 作为工具结果
        │
        ▼
结果作为 tool 消息 → 再次请求 LLM → 生成最终自然语言回复
```

---

## 3. 数据模型

### 3.1 AgentToolsPluginConfig（扩展骨架版）

```kotlin
data class AgentToolsPluginConfig(
    val context: Context,   // PackageManager / Intent 都需要 Context
)
```

（相比 memory/proactive，这个插件不需要注入 LLM 客户端——它纯粹是"执行动作"，不生成文本）

### 3.2 各工具的 parametersSchema

**`open_app`**:
```json
{
  "type": "object",
  "properties": {
    "app_name": { "type": "string", "description": "应用名称，如「微信」「支付宝」" }
  },
  "required": ["app_name"]
}
```

**`send_notification_reply`**:
```json
{
  "type": "object",
  "properties": {
    "app_name": { "type": "string", "description": "目标应用名称" },
    "reply_text": { "type": "string", "description": "回复内容" }
  },
  "required": ["app_name", "reply_text"]
}
```

**`set_alarm`**:
```json
{
  "type": "object",
  "properties": {
    "hour": { "type": "integer", "description": "小时 0-23" },
    "minute": { "type": "integer", "description": "分钟 0-59" },
    "label": { "type": "string", "description": "闹钟备注，可选" }
  },
  "required": ["hour", "minute"]
}
```

---

## 4. 接口定义

### 4.1 OpenAppTool（应用名 → 包名，需要一次本地匹配）

```kotlin
class OpenAppTool(private val context: Context) : ToolDefinition {
    override val name = "open_app"
    override val description = "打开手机上指定名称的应用"
    override val parametersSchema = /* 见 3.2 */ ""
    override val requiresConfirmation = false

    override suspend fun execute(argsJson: String): String {
        val appName = parseAppName(argsJson) ?: return """{"error":"缺少 app_name 参数"}"""
        val packageName = resolvePackageByLabel(context, appName)
            ?: return """{"error":"未找到名为「$appName」的应用"}"""
        val intent = context.packageManager.getLaunchIntentForPackage(packageName)
            ?: return """{"error":"「$appName」没有可启动的入口"}"""
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
        return """{"result":"已打开 $appName"}"""
    }
}
```

`resolvePackageByLabel` 通过 `PackageManager.getInstalledApplications()` 遍历匹配显示名称（模糊匹配，忽略大小写），避免要求 LLM 直接猜测包名（对用户/LLM 都不友好）。

### 4.2 SendNotificationReplyTool（危险操作，`requiresConfirmation = true`）

```kotlin
class SendNotificationReplyTool(
    private val notificationBridge: NotificationReplyBridge,  // 见 4.3
) : ToolDefinition {
    override val name = "send_notification_reply"
    override val description = "代替用户快捷回复某个应用的最近一条通知消息（会真实发送，执行前需用户确认）"
    override val parametersSchema = /* 见 3.2 */ ""
    override val requiresConfirmation = true   // 硬约束：必须走 §2.1 的确认流程

    override suspend fun execute(argsJson: String): String {
        val (appName, replyText) = parseArgs(argsJson)
            ?: return """{"error":"参数不合法"}"""
        return notificationBridge.reply(appName, replyText)
    }
}
```

### 4.3 NotificationReplyBridge —— 为什么又是一个"注入接缝"？

快捷回复依赖 `NotificationListenerService`（需要用户在系统设置里单独授权"通知使用权"，且必须是一个真正注册的 `Service` 组件），这是**插件模块无法自行声明**的（Android Library module 理论上可以带 `Service`，但会打破"插件骨架 `AndroidManifest.xml` 保持空清单"的既有约定，且通知使用权限限制多为按 App 授权，由 core 持有更合理）。

**决策**：`NotificationListenerService` 实现放在 **core 侧**，插件只定义接口：

```kotlin
fun interface NotificationReplyBridge {
    suspend fun reply(appName: String, replyText: String): String  // 返回 JSON 结果字符串
}
```

这是本插件对"接缝在 core、逻辑在插件"模式的第三次复用（vision 的 `ScreenshotProvider`、memory/proactive 的 LLM 客户端接口、这里的通知桥接），说明该模式已经是本项目插件架构的**通用范式**，值得在 `docs/plans/plugin-template.md` 里补一条通用指导（见 §6）。

### 4.4 SetAlarmTool（走系统 Intent，天然有系统级二次确认）

```kotlin
class SetAlarmTool(private val context: Context) : ToolDefinition {
    override val name = "set_alarm"
    override val description = "设置一个系统闹钟"
    override val parametersSchema = /* 见 3.2 */ ""
    override val requiresConfirmation = false  // AlarmClock.ACTION_SET_ALARM 会跳系统 UI 二次确认，无需插件层再确认一次

    override suspend fun execute(argsJson: String): String {
        val (hour, minute, label) = parseArgs(argsJson) ?: return """{"error":"参数不合法"}"""
        val intent = Intent(AlarmClock.ACTION_SET_ALARM).apply {
            putExtra(AlarmClock.EXTRA_HOUR, hour)
            putExtra(AlarmClock.EXTRA_MINUTES, minute)
            label?.let { putExtra(AlarmClock.EXTRA_MESSAGE, it) }
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        if (intent.resolveActivity(context.packageManager) == null) {
            return """{"error":"设备上没有可处理闹钟设置的应用"}"""
        }
        context.startActivity(intent)
        return """{"result":"已跳转到闹钟设置 $hour:$minute"}"""
    }
}
```

---

## 5. 文件清单

| # | 文件路径 | 变更类型 |
| --- | --------- | --------- |
| 1 | `app/src/main/java/com/live2d/ai/android/ApiModels.kt` | 修改：请求体增加 `tools` 字段，响应解析增加 `tool_calls` |
| 2 | `app/src/main/java/com/live2d/ai/android/ChatService.kt` | 修改：接入 tool-calling 循环 + 统一确认拦截（§2.1/2.2，**子任务 0，建议先行验收**） |
| 3 | `app/src/main/java/com/live2d/ai/android/ToolConfirmationDialog.kt` | 新增（Compose）：危险工具的确认 UI |
| 4 | `app/src/main/java/com/live2d/ai/android/NotificationReplyListenerService.kt` | 新增：`NotificationListenerService` 实现，供插件注入的 `NotificationReplyBridge` 调用 |
| 5 | `live2d-ai-plugin-agent-tools/.../OpenAppTool.kt` | 新增 |
| 6 | `live2d-ai-plugin-agent-tools/.../SendNotificationReplyTool.kt` | 新增 |
| 7 | `live2d-ai-plugin-agent-tools/.../SetAlarmTool.kt` | 新增 |
| 8 | `live2d-ai-plugin-agent-tools/.../NotificationReplyBridge.kt` | 新增：接口定义 |
| 9 | `live2d-ai-plugin-agent-tools/.../AgentToolsPlugin.kt` | 修改：`onLoad` 里注册三个工具 |
| 10 | `live2d-ai-plugin-agent-tools/.../AgentToolsPluginConfig.kt` | 修改：按 3.1 补充真实字段 |
| 11 | `live2d-ai-plugin-agent-tools/src/test/.../OpenAppToolTest.kt` | 新增：mock PackageManager，验证匹配/未找到/无入口三种分支 |
| 12 | `live2d-ai-plugin-agent-tools/src/test/.../SendNotificationReplyToolTest.kt` | 新增：验证参数校验 + `requiresConfirmation == true` |
| 13 | `live2d-ai-plugin-agent-tools/src/test/.../SetAlarmToolTest.kt` | 新增 |
| 14 | `app/src/test/.../ChatServiceToolConfirmationTest.kt` | 新增：**最关键的测试**——mock 一个 `requiresConfirmation=true` 的工具，验证用户拒绝时 `execute()` 从未被调用 |

---

## 6. 风险与注意事项

| # | 风险 | 缓解措施 |
| --- | ------ | --------- |
| R1（**最高优先级**） | 确认机制如果实现有漏洞（如异步竞态导致 UI 还没弹出、`execute()` 已经跑了），会让"防误操作"的硬约束形同虚设 | `ChatServiceToolConfirmationTest`（文件清单 #14）必须作为验收阻塞项，且建议人工在真机上过一遍"拒绝确认"的实际操作流程，不能只信单元测试 |
| R2 | `send_notification_reply` 依赖用户手动授权"通知使用权"，如果用户未授权，功能应优雅降级而非崩溃 | `NotificationReplyBridge.reply()` 内部检测权限状态，未授权时返回明确错误信息（如"请先在设置中开启通知访问权限"），并可选让 LLM 用自然语言引导用户去开启 |
| R3 | `open_app` 的应用名模糊匹配可能匹配到错误的应用（如同名 App） | MVP 接受"取第一个模糊匹配结果"，返回结果里明确带上匹配到的应用名，让用户/LLM 能发现匹配错误并纠正，不做复杂的歧义消解 |
| R4 | tool-calling 循环（子任务 0）改动 `ChatService` 核心逻辑，是本次改动里唯一触及"主对话循环"的部分 | 必须保证：未接入任何工具（`toolDefinitions` 为空列表）时，行为与当前完全一致（不发送 `tools` 字段，不解析 `tool_calls`）——即向后兼容是硬性验收标准 |
| R5 | 三个工具都需要 Android `Context`/`PackageManager`/`Intent`，无法像 vision/memory 一样在纯 JVM 环境完整验证，只能验证参数解析等纯逻辑部分 | 沿用既有做法：纯逻辑部分（JSON 解析、字符串匹配算法）抽成不依赖 Context 的 pure function 单独测试，`Context` 相关部分用 Robolectric（vision 插件已验证可行）或人工 review + 真机验收 |

---

## 6.1 补充：`docs/plans/plugin-template.md` 建议增补的通用范式

本任务是第三次出现"插件定义接口、core 提供实现"模式（vision 的 `ScreenshotProvider`、memory/proactive 的 LLM 客户端、这里的 `NotificationReplyBridge`），建议在 `plugin-template.md` 新增一条通用指导：

> **当插件需要的能力依赖 Android 系统权限/组件（Context、Service、Activity 结果回调等）时，插件只定义 `fun interface` 描述"需要什么"，具体实现留给 core 通过构造参数注入——插件本身保持无 Android 组件依赖，便于独立测试和替换实现。**

（这条不是本任务的强制交付物，留给 `task-plugin-agent-tools` 实现者顺手补上即可）

---

## 7. 验收标准（供后续 tester 使用）

1. **子任务 0（tool-calling 循环）**：未注册任何工具时，`ChatService` 行为与改动前完全一致（回归测试）
2. **子任务 0**：注册一个 `requiresConfirmation=false` 的 mock 工具，LLM 返回对应 tool_call 后能正确执行并把结果回填给 LLM
3. **子任务 0（阻塞项）**：注册一个 `requiresConfirmation=true` 的 mock 工具，模拟用户点击"拒绝"，验证 `execute()` 从未被调用，且 LLM 收到"用户拒绝执行"的结果
4. `OpenAppToolTest`：应用存在/不存在/存在但无启动入口，三种分支均有对应错误信息
5. `SendNotificationReplyTool.requiresConfirmation == true`（编译期属性即可断言，无需运行时逻辑）
6. `SetAlarmToolTest`：非法时间参数（如 hour=25）返回错误而非崩溃
7. 三个插件工具 + core 改动，整体编译通过（含 `:app:assembleDebug`，需团队在正常环境验证，本地沙盒环境受限）
