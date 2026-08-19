# plan-task-chat-tool-loop — ChatService Tool-Calling 循环设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 设计 ToolDefinition 请求/响应循环如何接入 ChatService（core 侧改动）
> 前置依赖: Wave 0（`task-plugin-sdk` + `task-chat-refactor`）已完成，`PluginManager.tools` / `ChatService.hooks` 已就绪
> 被依赖: `task-plugin-agent-tools`（Wave 2）、`task-extract-vision` Wave 2 tool-call 模式

---

## 1. 需求概述

### 1.1 背景

当前 `ChatService` 只实现了 `ChatHook`（`beforeSend` / `afterReceive`）的对话生命周期挂钩，`ToolDefinition` 的请求/响应循环尚未接入。参见：

- `docs/plans/plan-task-plugin-architecture.md` §7：「`ToolDefinition` 的请求/响应循环（工具调用）接入 `ChatService` ⏳ 未做」
- `docs/plans/plan-task-plugin-agent-tools.md` §2.2：「子任务 0 — 扩展 ApiModels.kt + ChatService」
- `ChatService.kt` 源码中的 TODO 注释：`TODO(task-plugin-agent-tools, Wave 2): 目前仅接入 ChatHook...`

### 1.2 目标

在 `ChatService.sendMessage()` 中植入标准的 OpenAI 兼容 function-calling 循环：

```
用户消息
  → beforeSend hooks（已有）
  → [请求 LLM，带 tools 列表]
  → LLM 返回 tool_calls？
      是 → 查 PluginManager.tools 执行 → 结果回填 → 再次请求 → ...
      否 → 文本回复，退出循环
  → afterReceive hooks（已有）
```

### 1.3 验收标准

| # | 标准 | 优先级 |
| --- | ------ | -------- |
| A1 | 无工具注册时（`tools` 为空列表），行为与当前版本完全一致（回归） | 阻塞 |
| A2 | 注册一个 `requiresConfirmation=false` 的工具，LLM 返回对应 tool_call 后正确执行并回填结果 | 阻塞 |
| A3 | 注册一个 `requiresConfirmation=true` 的工具，用户拒绝时 `execute()` 从未被调用 | 阻塞 |
| A4 | 循环上限（max 8 轮）防止死循环 | 必须 |
| A5 | 单个 hook 或 tool 抛异常不打断对话主链路（try/catch 隔离） | 必须 |
| A6 | `DeepSeek API` / `Ollama` / `vLLM` 三种 provider 均可正常走通（OpenAI 兼容格式） | 必须 |

---

## 2. 系统架构

### 2.1 整体数据流

```
sendMessage(userText)
  │
  ├─ 1. beforeSend hooks（逐个 try/catch，非空返回值追加到上下文）
  │
  ├─ 2. tool-calling 循环（MAX_TOOL_ROUNDS = 8）
  │    │
  │    ├─ 2a. 构建 ChatRequest
  │    │      tools 列表为空？→ stream=true, tools=null  （快路径，与现状一致）
  │    │      tools 列表非空？→ stream=false, tools=[...]（慢路径，需解析 tool_calls）
  │    │
  │    ├─ 2b. HTTP POST /chat/completions
  │    │
  │    ├─ 2c. 解析响应
  │    │      choices[0].message.tool_calls 非空？
  │    │        YES → 2d 执行工具 → 2e 回填 → 回到 2a（循环）
  │    │        NO  → choices[0].message.content 是文本
  │    │              → 退出循环，进入步骤 3
  │    │
  │    ├─ 2d. 执行工具（逐个串行）
  │    │      查 PluginManager.tools 表 → 按 name 匹配
  │    │      匹配失败？→ 返回 "工具未注册" 错误
  │    │      匹配成功？→ 检查 requiresConfirmation
  │    │        false → 直接 execute(args)
  │    │        true  → ConfirmationGate.confirm(...)
  │    │                  同意 → execute(args)
  │    │                  拒绝 → 返回 "用户拒绝执行"
  │    │
  │    └─ 2e. 回填结果到 messages
  │           assistant 消息（含 tool_calls 字段）
  │           + tool 消息（role="tool", tool_call_id=..., content=...）
  │
  ├─ 3. 将最终文本按 Flow<String> 发射（慢路径一次发射整段，快路径逐 delta 发射）
  │
  ├─ 4. afterReceive hooks（逐个 try/catch）
  │
  └─ 5. trimConversationHistory()
```

### 2.2 快路径 vs 慢路径

| 维度 | 快路径（tools 为空） | 慢路径（tools 非空） |
| ------ | --------------------- | --------------------- |
| `stream` | `true` | `false` |
| `tools` 字段 | 不发送（或 `null`） | 发送工具列表 |
| 响应解析 | SSE delta 逐条解析 | 整段 JSON 反序列化 |
| Flow 发射 | 多个 delta 逐条 emit | 单次 emit 整段文本 |
| 向后兼容 | ✅ 完全一致 | 新增行为 |

**决策理由**：tool_calls 在 SSE 流式响应中是逐 chunk 增量传输的，解析复杂（需按 index 累积 `function.name` + `function.arguments` 片段）。MVP 用非流式请求处理工具调用循环，降低解析复杂度。流式化优化可留作后续迭代。

### 2.3 关键架构决策

#### 决策 1：ConfirmationGate 是 core 内部接口，不暴露到 PluginHost

```
问题：谁负责弹确认框？
回答：core（ChatService 持有 ConfirmationGate），不是插件。

原因：
1. 弹窗需要 Activity/Compose UI 上下文，插件不应持有 Activity 引用（内存泄漏）
2. 确认逻辑必须在所有工具上统一生效——如果分散在各插件里，一个插件"忘记检查"就让硬约束形同虚设
3. ConfirmationGate 是 core 内部接口（不放入 plugin-sdk），MainActivity 实现它并注入 ChatService
```

#### 决策 2：非流式工具调用循环 + 流式最终回复（MVP 简化）

```
工具循环迭代：stream=false（解析整段 tool_calls JSON）
最终自然语言回复：如 tools 为空则 stream=true（现状）；如 tools 非空则 stream=false（整段发射）
```

这避免了 SSE tool_call delta 累积解析的复杂性。后续可优化为：工具循环非流式 → 最后一次请求切换回流式。

#### 决策 3：工具结果消息永久保留在对话历史中

```
不做「工具结果仅在当前 turn 内可见」的隔离。
原因：LLM 可以从历史工具调用中学习上下文（如 OpenAI/Anthropic 推荐做法），
     且 trimConversationHistory 自然会裁剪旧消息。
```

---

## 3. 模块划分

### 3.1 改动模块

| 模块 | 文件 | 改动性质 |
| ------ | ------ | ---------- |
| `app` | `ApiModels.kt` | 修改：新增 tool_calls/tool_call_id/tools 相关数据类 |
| `app` | `ChatService.kt` | 修改：接入 tool-calling 循环 + ConfirmationGate |
| `app` | `ConfirmationGate.kt` | 新增：确认拦截接口（core 内部接口） |
| `plugin-sdk` | 无改动 | —（ToolDefinition 接口已完备） |
| `app` | `ChatServiceTest.kt` | 修改：新增 tool loop 测试用例 |

### 3.2 不改动的模块

- `PluginManager.kt`：`tools` 属性已返回 `CopyOnWriteArrayList`（活引用），无需改动
- `PluginHost.kt` / `ToolDefinition.kt`：接口已完备
- `MainActivity.kt`：仅需注入 `ConfirmationGate` 实现（设计文档覆盖，具体实现留给 developer）

---

## 4. 数据模型

### 4.1 ApiModels.kt 新增类型

```kotlin
// ── 请求侧：工具定义（序列化为 LLM API 格式） ──

@Serializable
data class FunctionSpec(
    val name: String,
    val description: String,
    val parameters: JsonObject,  // JSON Schema，由 ToolDefinition.parametersSchema 解析而来
    val strict: Boolean = false  // DeepSeek strict mode（Beta，默认关闭）
)

@Serializable
data class ToolDef(
    val type: String = "function",
    val function: FunctionSpec
)

// ── 响应侧：工具调用结果 ──

@Serializable
data class ToolCallFunction(
    val name: String,
    val arguments: String  // JSON 字符串，由 LLM 生成
)

@Serializable
data class ToolCall(
    val id: String,
    val type: String = "function",
    val function: ToolCallFunction
)
```

### 4.2 ApiModels.kt 修改现有类型

**ChatRequest** — 新增可选字段：

```kotlin
@Serializable
data class ChatRequest(
    val model: String = "deepseek-chat",
    val messages: List<ChatMessage>,
    val temperature: Double = 0.8,
    val max_tokens: Int = 300,
    val stream: Boolean = true,
    val tools: List<ToolDef>? = null,       // ← 新增：工具列表（null = 不发送）
    val tool_choice: String? = null          // ← 新增："auto"/"none"/"required"（默认 null = auto when tools present）
)
```

**ChatMessage** — 新增可选字段：

```kotlin
@Serializable
data class ChatMessage(
    val role: String = "assistant",
    val content: String? = null,
    val tool_calls: List<ToolCall>? = null,  // ← 新增：assistant 消息可能携带工具调用
    val tool_call_id: String? = null          // ← 新增：tool 消息携带调用 ID
)
```

**ChatChoice** — message 可能携带 tool_calls：

```kotlin
@Serializable
data class ChatChoice(
    val index: Int = 0,
    val delta: ChatMessage? = null,           // 流式：delta 中也可能含 tool_calls（增量）
    val message: ChatMessage? = null,         // 非流式：message 中含 tool_calls
    val finish_reason: String? = null
)
```

> **序列化兼容性说明**：`Json { ignoreUnknownKeys = true }` 已设置，新增字段为可空类型，旧 JSON 响应无这些字段时不会反序列化失败。

### 4.3 工具参数格式转换

`ToolDefinition.parametersSchema` 是 JSON 字符串（如 `{"type":"object","properties":...}`）。
在构建 `FunctionSpec` 时需将其解析为 `JsonObject`：

```kotlin
fun ToolDefinition.toFunctionSpec(): FunctionSpec {
    val params = Json.parseToJsonElement(parametersSchema).jsonObject
    return FunctionSpec(
        name = name,
        description = description,
        parameters = params,
        strict = false
    )
}
```

### 4.4 消息序列示例

**请求体（带工具）**：

```json
{
  "model": "deepseek-v4-flash",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "帮我打开微信"}
  ],
  "stream": false,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "open_app",
        "description": "打开手机上指定名称的应用",
        "parameters": {
          "type": "object",
          "properties": {
            "app_name": {"type": "string", "description": "应用名称"}
          },
          "required": ["app_name"]
        }
      }
    }
  ]
}
```

**LLM 响应（函数调用）**：

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "open_app",
          "arguments": "{\"app_name\":\"微信\"}"
        }
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```

**工具结果回填消息**：

```json
{"role": "tool", "tool_call_id": "call_abc123", "content": "{\"result\":\"已打开 微信\"}"}
```

---

## 5. 接口定义

### 5.1 ConfirmationGate（core 内部接口）

```kotlin
// 文件: app/src/main/java/com/live2d/ai/android/ConfirmationGate.kt

package com.live2d.ai.android

import com.live2d.ai.pluginsdk.ToolDefinition

/**
 * 危险工具执行前的确认拦截门。
 *
 * 这是 core 内部接口（不放入 plugin-sdk），原因：
 * 1. 弹确认框需要 Activity/Compose UI 上下文，属于 core 的 UI 层职责
 * 2. 插件不应持有 Activity 引用（内存泄漏风险，与 ScreenshotProvider 设计原则一致）
 * 3. 确认逻辑在 core 统一生效，保证所有 requiresConfirmation=true 的工具都被拦截
 *
 * 实现者：MainActivity（Compose Dialog）或单元测试（mock 直接返回 true/false）
 */
fun interface ConfirmationGate {
    /**
     * 请求用户确认执行危险工具。
     *
     * @param tool 待确认的工具定义（可展示 name/description 给用户）
     * @param argsJson LLM 生成的调用参数（可解析后展示关键字段给用户，如"回复内容: xxx"）
     * @return true 表示用户同意执行，false 表示拒绝
     */
    suspend fun confirm(tool: ToolDefinition, argsJson: String): Boolean
}
```

### 5.2 ChatService 构造函数变更

```kotlin
class ChatService(
    private val provider: LLMProvider = LLMProvider.deepSeek(),
    private val hooks: List<ChatHook> = emptyList(),
    private val tools: List<ToolDefinition> = emptyList(),  // ← 新增：工具注册表
    private val confirmationGate: ConfirmationGate? = null   // ← 新增：确认门（null = 拒绝所有危险操作）
)
```

**向后兼容**：

- 旧构造函数 `ChatService(apiKey, baseUrl)` 保留，`tools` 和 `confirmationGate` 默认值保持旧行为
- `tools` 为空时 `ChatService` 行为与现状完全一致（快路径，`stream=true`，无 `tools` 字段）

### 5.3 辅助方法

```kotlin
/**
 * 将 ToolDefinition 列表转换为 LLM API 所需的 tools 格式。
 * parametersSchema 为 JSON Schema 字符串，解析为 JsonObject 后嵌入 FunctionSpec。
 */
private fun buildToolsForRequest(): List<ToolDef>? {
    if (tools.isEmpty()) return null
    return tools.map { tool ->
        ToolDef(
            function = FunctionSpec(
                name = tool.name,
                description = tool.description,
                parameters = json.parseToJsonElement(tool.parametersSchema).jsonObject
            )
        )
    }
}
```

---

## 6. ChatService 主循环伪代码

```kotlin
fun sendMessage(userText: String): Flow<String> = flow {

    // ── Step 1: beforeSend hooks（已有，不变） ──
    val extraContext = StringBuilder()
    // ... (existing hook logic)

    messages.add(ChatMessage(role = "user", content = effectiveUserText))

    // ── Step 2: tool-calling 循环 ──
    val requestedTools = buildToolsForRequest()  // null if tools.isEmpty()
    val useStreaming = (requestedTools == null)   // 快路径时为 true

    var finalContent: String? = null
    var round = 0

    while (round < MAX_TOOL_ROUNDS) {
        round++

        // ── Step 2a: 构建请求 ──
        val request = ChatRequest(
            model = modelName,
            messages = messages,
            temperature = PersonaConfig.getTemperature(),
            max_tokens = PersonaConfig.getMaxTokens(),
            stream = false,  // 工具循环一律非流式（简化 tool_calls 解析）
            tools = requestedTools
            // tool_choice 不设置 → 默认 auto
        )

        // ── Step 2b: HTTP 请求 ──
        val httpResponse = executeHttpRequest(request)
        if (!httpResponse.isSuccessful) {
            emit("[错误] API请求失败: ${httpResponse.code}")
            return@flow
        }

        // ── Step 2c: 解析完整响应 ──
        val body = httpResponse.body?.string() ?: ""
        val chatResponse = json.decodeFromString<ChatResponse>(body)
        val choice = chatResponse.choices?.firstOrNull() ?: continue
        val message = choice.message ?: continue

        // ── Step 2d: 检测 tool_calls ──
        val toolCalls = message.tool_calls
        if (toolCalls.isNullOrEmpty()) {
            // ── 无工具调用：退出循环，获取文本内容 ──
            finalContent = message.content ?: ""
            messages.add(message)
            break
        }

        // ── Step 2e: 有工具调用：执行工具 ──
        messages.add(message)  // 先记录 assistant 消息（含 tool_calls）

        for (toolCall in toolCalls) {
            val toolName = toolCall.function.name
            val argsJson = toolCall.function.arguments

            // 查表
            val toolDef = tools.find { it.name == toolName }
            val result = if (toolDef == null) {
                """{"error":"未找到工具: $toolName"}"""
            } else if (toolDef.requiresConfirmation) {
                // 确认拦截
                val gate = confirmationGate
                if (gate == null || !gate.confirm(toolDef, argsJson)) {
                    """{"error":"用户拒绝执行 $toolName"}"""
                } else {
                    try {
                        toolDef.execute(argsJson)
                    } catch (e: Exception) {
                        """{"error":"工具执行异常: ${e.message}"}"""
                    }
                }
            } else {
                try {
                    toolDef.execute(argsJson)
                } catch (e: Exception) {
                    """{"error":"工具执行异常: ${e.message}"}"""
                }
            }

            // 回填结果
            messages.add(
                ChatMessage(
                    role = "tool",
                    tool_call_id = toolCall.id,
                    content = result
                )
            )
        }
        // 循环继续：messages 中已有 assistant(tool_calls) + tool results
        // LLM 看到这些消息后会决定是继续调工具还是生成最终文本
    }

    if (round >= MAX_TOOL_ROUNDS && finalContent == null) {
        emit("[错误] 工具调用超过最大轮次，已终止")
        return@flow
    }

    // ── Step 3: 发射最终内容 ──
    // 如果原本是流式路径（无工具），则走流式 SSE 发射
    // 如果是工具循环路径，finalContent 已经是完整文本，一次性发射
    if (useStreaming) {
        // 快路径：流式发射（复用现有逻辑，但需提取为独立方法）
        val sseFlow = executeStreamingRequest(modelName)
        // ... collect and emit deltas, build assistantContent
    } else {
        // 慢路径：一次性发射
        if (finalContent != null) {
            emit(finalContent)
        }
    }

    // ── Step 4: afterReceive hooks（已有，不变） ──
    // ...

    // ── Step 5: trimConversationHistory（已有，不变） ──
}
```

### 6.1 流式请求提取

将现有 `sendMessage()` 中的 SSE 流式逻辑提取为独立方法 `executeStreamingRequest()`，供快路径复用。慢路径（工具循环）直接使用 `executeHttpRequest()` + 整段 JSON 反序列化。

### 6.2 请求方法选择

```kotlin
/**
 * 执行非流式 HTTP 请求，返回完整响应体字符串。
 * 用于工具调用循环（需要解析完整 tool_calls JSON）。
 */
private suspend fun executeNonStreamingRequest(request: ChatRequest): Response {
    val requestBody = json.encodeToString(request).toRequestBody(mediaType)
    val httpRequest = buildHttpRequest(requestBody)
    return client.newCall(httpRequest).execute()
}

/**
 * 执行流式 SSE 请求，返回 Flow<String>（逐 delta 发射）。
 * 用于无工具注册时的快路径。
 */
private fun executeStreamingRequest(modelName: String, messages: List<ChatMessage>): Flow<String> = flow {
    // ... 现有流式逻辑 ...
}
```

---

## 7. 状态机与不变量

### 7.1 tool-calling 循环状态机

```
     ┌──────────────────────────────────────┐
     │                                      │
     ▼                                      │
 [构建请求] ──→ [HTTP POST] ──→ [解析响应] ──┤
                   ▲                        │
                   │                  tool_calls?
                   │                 /          \
                   │              YES            NO
                   │               │               │
                   │         [执行工具]        [提取文本]
                   │               │               │
                   │         [回填结果]        [退出循环]
                   │               │
                   └───────────────┘
                (最多 MAX_TOOL_ROUNDS 次)
```

### 7.2 关键不变量

| # | 不变量 | 验证方式 |
| --- | -------- | ---------- |
| I1 | `tools` 为空 → `requestedTools == null` → `stream=true` → 行为与现状完全一致 | 单元测试：无工具注册时请求体不含 `tools` 字段 |
| I2 | `requiresConfirmation=true` 且 `confirmationGate` 返回 `false` → `tool.execute()` 从未被调用 | 单元测试：mock 工具 + mock gate 返回 false，验证 execute 未被调用 |
| I3 | 单轮工具循环不超过 `MAX_TOOL_ROUNDS=8` | 单元测试：mock LLM 始终返回 tool_calls，验证第 9 轮被截断 |
| I4 | 单个 tool 执行抛异常不中断循环（try/catch 隔离） | 单元测试：mock 工具抛异常，验证循环继续到下一轮 |
| I5 | `confirmationGate == null` 且 `requiresConfirmation=true` → 工具被拒绝（安全默认） | 单元测试：不注入 gate，验证危险工具被拒绝 |
| I6 | `ChatMessage` 序列化兼容：无 `tool_calls`/`tool_call_id` 的旧 JSON 正常反序列化 | `ignoreUnknownKeys=true` 已设置，新增字段均为可空 |

### 7.3 对话历史消息角色组合

工具调用在一个 turn 内会产生以下消息序列：

```
user: "帮我打开微信"
assistant: {tool_calls: [{name: "open_app", ...}]}   ← content=null
tool: {tool_call_id: "call_xxx", content: '{"result":"已打开 微信"}'}
assistant: "已经帮你打开微信了喵~"                      ← 最终自然语言回复
```

`trimConversationHistory()` 按条数裁剪，不区分角色。工具相关消息（assistant(tool_calls) + tool）作为普通消息参与计数，自然裁剪。

---

## 8. 文件清单

| # | 文件路径 | 变更类型 | 说明 |
| --- | ---------- | ---------- | ------ |
| 1 | `app/src/main/java/com/live2d/ai/android/ApiModels.kt` | **修改** | 新增 `FunctionSpec`, `ToolDef`, `ToolCall`, `ToolCallFunction`；`ChatRequest` + `tools`/`tool_choice`；`ChatMessage` + `tool_calls`/`tool_call_id` |
| 2 | `app/src/main/java/com/live2d/ai/android/ChatService.kt` | **修改** | 构造函数 + `tools`/`confirmationGate` 参数；`sendMessage()` 植入 tool-calling 循环；提取 `executeStreamingRequest()` / `executeNonStreamingRequest()` |
| 3 | `app/src/main/java/com/live2d/ai/android/ConfirmationGate.kt` | **新增** | `fun interface ConfirmationGate` — 危险工具确认拦截 |
| 4 | `app/src/test/java/com/live2d/ai/android/ChatServiceTest.kt` | **修改** | 新增：无工具回归、工具执行、确认拒绝、循环上限、工具异常隔离、gate=null 安全默认 |
| 5 | `app/src/main/java/com/live2d/ai/android/MainActivity.kt` | **修改** | 注入 `ConfirmationGate` 实现（Compose AlertDialog），具体实现留给 developer |
| 6 | `plugin-sdk/` | **无改动** | `ToolDefinition` / `PluginHost` 接口已完备 |

---

## 9. 实施顺序

| 步骤 | 内容 | 依赖 | 验收 |
| ------ | ------ | ------ | ------ |
| S1 | 扩展 `ApiModels.kt` — 新增数据类，修改现有类添加可选字段 | 无 | 编译通过；旧测试全过 |
| S2 | 提取 `ChatService` 流式逻辑为 `executeStreamingRequest()` | S1 | 旧测试全过（回归） |
| S3 | 新增 `executeNonStreamingRequest()` | S2 | 编译通过 |
| S4 | 实现 `buildToolsForRequest()` / tool-calling 循环核心逻辑 | S3 | S4 的单元测试通过 |
| S5 | 新增 `ConfirmationGate.kt` 接口 | 无 | 编译通过 |
| S6 | 在 `sendMessage()` 中接入确认拦截逻辑 | S4, S5 | S6 的单元测试通过 |
| S7 | 编写 tool loop 全套单元测试 | S4, S6 | 全部新测试通过 + 旧测试无回归 |
| S8 | MainActivity 注入 `ConfirmationGate` 实现 | S5 | 编译通过（真机验收留到 task-plugin-agent-tools） |

---

## 10. 风险与注意事项

| # | 风险 | 严重度 | 缓解措施 |
| --- | ------ | -------- | ---------- |
| R1 | 慢路径（非流式）改变了用户感知的响应延迟（整段文本一次性出现 vs 逐字流式） | 中 | 仅在有工具注册时走慢路径；文档明确说明此 trade-off；后续可优化为「工具循环非流式 + 最终回复流式」 |
| R2 | `ChatMessage` 新增 `tool_calls` 字段后，`encodeDefaults = true` 可能导致空 `tool_calls` 被序列化为 `null` 发送到 LLM API | 低 | `null` 字段在 JSON 中与缺少字段等效，OpenAI 兼容 API 会忽略 `null` 值；实际验证 |
| R3 | `ChatMessage.content` 已为 nullable（reasoning 模型），但 `content=null` + `tool_calls` 非空是合法组合（DeepSeek 文档确认）；部分推理逻辑可能假设 `content` 非空 | 中 | 在 tool-calling 循环中优先检查 `tool_calls`，仅当其为空时才使用 `content`；`afterReceive` 中增加 `content` 空值保护 |
| R4 | `executeNonStreamingRequest()` 的 `readTimeout` 可能需要调整（当前 120s 足够） | 低 | 非流式请求通常比流式更快（一次返回整段），120s 足够 |
| R5 | 多个工具并发执行 vs 串行执行 | 低 | MVP 采用串行执行（按 tool_calls 数组顺序），因为依赖关系不确定（第二个工具可能需要第一个工具的结果）；后续可增加「无依赖工具组并行执行」优化 |
| R6 | `ConfirmationGate` 是 `suspend` 函数，在 `flow {}` 构建器内调用需确保不阻塞 `Dispatchers.IO` 上的协程 | 中 | `flowOn(Dispatchers.IO)` 在 `sendMessage()` 外层；`ConfirmationGate.confirm()` 由 MainActivity 实现，内部切到 `Dispatchers.Main` 弹 Dialog，不会阻塞 IO 线程 |

---

## 附录 A：DeepSeek API 工具调用协议速查

| 字段 | 位置 | 说明 |
| ------ | ------ | ------ |
| `tools[]` | 请求体 | 工具定义列表，每项 `{type:"function", function:{name, description, parameters}}` |
| `tool_choice` | 请求体 | `"auto"`（默认）/ `"none"` / `"required"` / `{type:"function", function:{name:"xxx"}}` |
| `message.tool_calls[]` | 响应 `choices[].message` | 工具调用数组，每项 `{id, type:"function", function:{name, arguments}}` |
| `finish_reason: "tool_calls"` | 响应 `choices[].finish_reason` | 表示模型调用了工具而非直接回复 |
| `role: "tool"` | 消息 `messages[]` | 工具结果消息，需带 `tool_call_id` 关联原调用 |
| `strict: true` | 工具定义 `function.strict` | Beta 功能，需 `base_url="https://api.deepseek.com/beta"` |

## 附录 B：与其他设计文档的交叉引用

| 文档 | 关联点 |
| ------ | -------- |
| `plan-task-plugin-architecture.md` §3.3 | ChatService 重构要求（已部分完成，本设计补齐 tool loop） |
| `plan-task-plugin-agent-tools.md` §2.1-2.2 | 确认机制职责归属 + 子任务 0 范围（本设计覆盖子任务 0 全部内容） |
| `plan-task-extract-vision.md` §1.2 | Vision 插件的 ToolDefinition 路径 Wave 2 启用（本设计是前置依赖） |
| `plan-task-local-llm.md` §3.3 | LLMProvider 抽象（本设计的 `executeNonStreamingRequest` 复用 `provider.baseUrl`/`apiKey`） |
