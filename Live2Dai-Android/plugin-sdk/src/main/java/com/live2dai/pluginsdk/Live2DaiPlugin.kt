package com.live2dai.pluginsdk

/**
 * 所有 Live2Dai 插件的顶层入口。
 *
 * 插件以 Gradle 依赖（同进程）的形式接入 core，在 `Application.onCreate()`
 * 阶段由宿主发现并调用 [onLoad]。插件不应假设自己独占主线程，也不应长时间
 * 阻塞 [onLoad]（初始化重活请丢到协程里）。
 *
 * 参考 PiAgent 的 Extension 理念：core 只认识这个最小接口，具体能做什么事
 * 完全由插件通过 [PluginHost.registerHook] / [PluginHost.registerTool] 自己声明。
 */
interface Live2DaiPlugin {
    /** 插件唯一标识，如 "vision" / "memory" / "proactive"。建议全小写、短横线分隔。 */
    val id: String

    /** 插件版本号，遵循 semver，便于宿主做兼容性检查（当前仅记录，不强校验）。 */
    val version: String

    /**
     * core 启动时调用一次。插件应在这里向 [host] 注册自己的 [ChatHook] / [ToolDefinition]，
     * 不应在此直接持有 Android Context——如需 Context 请通过插件自身的初始化参数传入。
     */
    fun onLoad(host: PluginHost)

    /** app 进程销毁 / 插件被禁用时调用，插件应在这里释放资源（关闭连接、取消协程作用域等）。 */
    fun onUnload()
}
