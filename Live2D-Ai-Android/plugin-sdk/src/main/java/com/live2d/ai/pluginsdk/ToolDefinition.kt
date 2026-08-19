package com.live2d.ai.pluginsdk

/**
 * 可被 LLM 通过 function-calling / tool-call 调用的工具定义。
 *
 * 字段命名与结构刻意向 **MCP（Model Context Protocol）Tool** 对齐
 * （`name` / `description` / JSON Schema 参数 / 文本结果），原因：
 * 1. 桌面端（`Live2D-Ai-pc/open-llm-vtuber`）已经通过 `shared/mcp_tools.json`
 *    使用 MCP 工具生态（time / ddg-search / weather / filesystem / ...）；
 *    Android 端工具协议向 MCP 对齐，未来才有可能让同一份工具描述在两端复用，
 *    或者让 core 反过来暴露一个 MCP Server 供桌面端/外部 Agent 调用手机能力。
 * 2. MCP 已有官方 Kotlin SDK（`io.modelcontextprotocol:kotlin-sdk`），如果某个插件
 *    未来需要连接真正的外部 MCP Server（而不是纯 Android 本地能力），可以直接在插件内部
 *    使用该 SDK 实现，无需 core 关心传输细节——`ToolDefinition` 只是 core 与工具之间的最终边界。
 *
 * 注意：当前 Android 侧的工具通常是"本地能力"（截屏、读通讯录等），不是通过
 * stdio/subprocess 连接外部 MCP Server（`uvx` 这类子进程在 Android 上不可用），
 * 所以本接口不直接依赖 MCP SDK 的 transport 层，只借用其 Tool 描述结构。
 */
interface ToolDefinition {
    /** 工具名，LLM 通过这个名字发起调用，建议 snake_case，如 "capture_screen". */
    val name: String

    /** 工具功能描述，会被拼进 LLM 的工具列表，帮助模型判断何时调用。 */
    val description: String

    /** 参数的 JSON Schema（字符串形式），描述与 MCP `inputSchema` 字段等价。 */
    val parametersSchema: String

    /**
     * 危险操作标记。为 true 时，core 必须在执行前弹出用户确认 UI，
     * 用户拒绝则直接返回失败结果，不得静默执行——这是防止 Agent 工具滥用的硬约束。
     */
    val requiresConfirmation: Boolean

    /**
     * 执行工具调用。
     *
     * @param argsJson LLM 生成的调用参数（JSON 字符串，需自行按 [parametersSchema] 解析）
     * @return 执行结果文本，会作为 tool 消息喂回给 LLM 继续生成回复
     */
    suspend fun execute(argsJson: String): String
}
