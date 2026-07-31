package com.live2dai.plugin.proactive

import com.live2dai.pluginsdk.PluginHost

/**
 * 进程内单例桥接：把 [PluginHost] 与 [GreetingLlmClient] 暴露给 [ProactiveCheckWorker]。
 *
 * 为什么需要它（docs/plan-task-plugin-proactive.md §6 R1）：
 * [ProactiveCheckWorker] 由 WorkManager 系统调度创建，生命周期独立于插件，
 * 构造时拿不到 [ProactivePlugin.onLoad] 传入的 [PluginHost] 引用。
 * 因此 onLoad 时把引用写进这个桥，Worker 执行时再读出来；
 * onUnload 时清空，避免泄漏 core 对象。
 *
 * 两个字段都标 @Volatile：Worker 可能在任意线程执行（WorkManager 的 Executor），
 * 与 onLoad/onUnload（主线程）之间存在跨线程可见性要求。
 */
internal object ProactiveHostBridge {

    @Volatile
    var host: PluginHost? = null

    /** LLM 问候生成器——fun interface 无法序列化进 WorkRequest inputData，只能走桥。 */
    @Volatile
    var greetingLlmClient: GreetingLlmClient? = null
}
