package com.live2dai.android

import android.content.Context

/**
 * ASR Provider 注册表 — 管理所有已注册的 ASR Provider（V1.2 模块4）。
 *
 * 与 [TtsProviderRegistry] 同构（plan §4.4.4：简化版注册表）：
 * - [register]：注册（同名覆盖）
 * - [cascade]：按 [AsrProviderInfo.priority] 升序的降级链（0 最高，先尝试）
 * - [get]：按 id 查找；[all]：全部（供引擎选择 UI 展示）
 *
 * 线程安全：注册发生在启动期（MainActivity.onCreate），读取发生在语音识别时
 * （可能跨线程），故读写均加锁；返回列表均为快照。
 */
object AsrProviderRegistry {
    private val providers = mutableListOf<AsrProvider>()
    private val lock = Any()

    /** 注册 Provider（同名覆盖：同 id 的旧实例被移除后追加新实例）。 */
    fun register(provider: AsrProvider) = synchronized(lock) {
        providers.removeAll { it.info.id == provider.info.id }
        providers += provider
    }

    /** 注销 Provider（幂等：不存在时无操作）。 */
    fun unregister(providerId: String) = synchronized(lock) {
        providers.removeAll { it.info.id == providerId }
    }

    /** 按 id 查找 Provider；不存在返回 null。 */
    fun get(id: String): AsrProvider? = synchronized(lock) {
        providers.find { it.info.id == id }
    }

    /** 按 priority 升序的降级链（数字越小越优先，0 最高）。 */
    fun cascade(): List<AsrProvider> = synchronized(lock) {
        providers.sortedBy { it.info.priority }
    }

    /** 所有已注册 Provider 的快照（供引擎选择 UI）。 */
    fun all(): List<AsrProvider> = synchronized(lock) {
        providers.toList()
    }

    /** 清空注册表（测试隔离 / 热重载场景使用）。 */
    fun clear() = synchronized(lock) {
        providers.clear()
    }
}

/**
 * 注册 V1.2 默认 ASR Provider（模块4）。
 *
 * ASR 降级链（plan §4.4.4 / schema cascadeAndroid.asr）：
 *   Tier 1 (priority=0): sherpa_sensevoice（离线 SenseVoice）—— 模型未下载时
 *   listen 快速返回 ""（health=UNAVAILABLE），降级链自动落入 Tier 2，不阻塞
 *   Tier 2 (priority=1): system_speech_recognizer（Android SpeechRecognizer，离线兜底）
 *   Tier 3: 全失败 → VoiceIoController.listen 静默返回 ""
 *
 * 幂等：AsrProviderRegistry.register 同名覆盖，重复调用安全（如 Activity 重建）。
 * 调用点：MainActivity.onCreate（VoiceIoController 构造前）；VoiceIoController
 * init 亦有空表兜底（注册表为空时自动注册系统 ASR，保证 listen() 永远有降级）。
 */
fun registerDefaultAsrProviders(context: Context) {
    val appContext = context.applicationContext
    // sherpa 优先（离线，priority=0）；模型未下载时 health=UNAVAILABLE → listen 快速降级
    AsrProviderRegistry.register(SherpaOnnxAsrProvider(appContext))
    AsrProviderRegistry.register(SystemAsrProvider(appContext))
}
