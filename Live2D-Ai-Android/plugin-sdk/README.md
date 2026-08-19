# Live2D-Ai Plugin SDK（最小骨架）

纯 Kotlin/JVM 模块，定义 core 与插件之间的最小接口：

- `Live2DAiPlugin`：插件入口
- `PluginHost`：core 暴露给插件的能力
- `ChatHook`：对话前后钩子
- `ToolDefinition`：LLM 工具调用定义

当前只恢复接口骨架，不接入 core，也不实现具体插件。

设计说明见 [docs/architecture/plugin-sdk.md](../../docs/architecture/plugin-sdk.md)。
