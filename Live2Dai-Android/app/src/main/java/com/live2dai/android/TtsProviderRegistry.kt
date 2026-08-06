package com.live2dai.android

import android.content.Context
import com.live2dai.android.data.SettingsRepository

/**
 * TTS Provider 注册表 — 管理所有已注册的 TTS Provider（V1.2 模块1，架构地基）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.TtsProviderRegistry.*：
 * - [register]：注册（同名覆盖，后续 Wave 重新注册同 id 的新实现时旧实例被替换）
 * - [cascade]：按 [TtsProviderInfo.priority] 升序的降级链（0 最高，先尝试）
 * - [get]：按 id 查找；[all]：全部（供引擎选择 UI 展示）
 *
 * 线程安全：注册发生在启动期（MainActivity.onCreate），读取发生在语音播报时
 * （可能跨线程），故读写均加锁；返回列表均为快照，避免并发修改异常。
 */
object TtsProviderRegistry {
    private val providers = mutableListOf<TtsProvider>()
    private val lock = Any()

    /** 注册 Provider（同名覆盖：同 id 的旧实例被移除后追加新实例）。 */
    fun register(provider: TtsProvider) = synchronized(lock) {
        providers.removeAll { it.info.id == provider.info.id }
        providers += provider
    }

    /** 注销 Provider（幂等：不存在时无操作）。 */
    fun unregister(providerId: String) = synchronized(lock) {
        providers.removeAll { it.info.id == providerId }
    }

    /** 按 id 查找 Provider；不存在返回 null。 */
    fun get(id: String): TtsProvider? = synchronized(lock) {
        providers.find { it.info.id == id }
    }

    /** 按 priority 升序的降级链（数字越小越优先，0 最高）。 */
    fun cascade(): List<TtsProvider> = synchronized(lock) {
        providers.sortedBy { it.info.priority }
    }

    /** 所有已注册 Provider 的快照（供引擎选择 UI）。 */
    fun all(): List<TtsProvider> = synchronized(lock) {
        providers.toList()
    }

    /** 仅离线可用的 Provider（无需网络）。 */
    fun offlineOnly(): List<TtsProvider> = synchronized(lock) {
        providers.filter { !it.info.requiresNetwork }
    }

    /**
     * 清空注册表（测试隔离 / 热重载场景使用）。
     */
    fun clear() = synchronized(lock) {
        providers.clear()
    }
}

/**
 * 注册 V1.2 默认 TTS Provider（Wave B：CosyVoice + sherpa 离线 + Edge TTS + 系统 TTS 降级链）。
 *
 * 降级链（plan §3.1.6）：
 *   Tier 1 (priority=0): CosyVoice 云端（DashScope WebSocket）—— API Key 未配置时
 * speak 快速失败（health=UNAVAILABLE），降级链自动落入 Tier 2，不阻塞
 *   Tier 2 (priority=1): Kokoro 离线（sherpa-onnx，模块3）—— 模型未下载时
 * speak 快速失败（health=UNAVAILABLE），继续降级；同 priority 下注册顺序决定稳定排序：
 * sherpa 注册在 EdgeTTS 之前 → cascade() 中 sherpa 先于 edge（离线优先，中国大陆可用）
 *   Tier 3 (priority=2): Android 系统 TTS（离线兜底）
 *   Tier 4 (priority=1 的 Edge TTS 云端/海外备选，Wave A 契约偏差保持 priority=1，
 *   靠注册顺序排在 sherpa 之后)
 *   Tier 5: 全失败 → VoiceIoController 静默降级
 *
 * 幂等：TtsProviderRegistry.register 同名覆盖，重复调用安全（如 Activity 重建）。
 * 调用点：MainActivity.onCreate（VoiceIoController 构造前，保证注册表就绪）。
 */
fun registerDefaultTtsProviders(context: Context) {
    val appContext = context.applicationContext
    val settings = SettingsRepository(appContext)
    // CosyVoice priority=0，cascade() 自动排降级链第一位（plan §3.1.6 Tier 1）
    TtsProviderRegistry.register(cosyVoiceProvider(appContext, settings))
    // sherpa(priority=1) 注册在 EdgeTTS 之前：同 priority 稳定排序保证离线优先（模块3）
    TtsProviderRegistry.register(SherpaOnnxTtsProvider(appContext))
    TtsProviderRegistry.register(EdgeTtsProvider(appContext))
    TtsProviderRegistry.register(SystemTtsProvider(appContext))
}

/**
 * 用当前 SettingsRepository 构建 CosyVoice Provider（apiKey + 可选专属 wsUrl）。
 * wsUrl 空串时 provider 内部回退默认公共 inference 端点。
 */
private fun cosyVoiceProvider(
    appContext: Context,
    settings: SettingsRepository,
): CosyVoiceTtsProvider = CosyVoiceTtsProvider(
    appContext,
    settings.aliyunApiKey,
    wsUrl = settings.cosyvoiceWsUrl,
)

/**
 * 用当前 SettingsRepository.aliyunApiKey 重建 CosyVoice Provider（V1.2 模块5 引擎选择 UI 调用）。
 *
 * 设置页修改阿里云 API Key 后调用：register 同名覆盖，替换启动时注册的旧实例
 * （旧实例持构造时的空 Key），使新 Key 立即生效，无需重启 App（plan §3.2.4）。
 * 幂等：Key 未变化时重复调用仅重建同参数实例，无副作用。
 */
fun refreshCosyVoiceProvider(context: Context) {
    val appContext = context.applicationContext
    val settings = SettingsRepository(appContext)
    TtsProviderRegistry.register(cosyVoiceProvider(appContext, settings))
}
