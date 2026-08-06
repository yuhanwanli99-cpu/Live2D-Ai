package com.live2dai.android

import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Before
import org.junit.Test

/**
 * AsrProviderRegistry（V1.2 模块4 注册表 + ASR 降级链）单元测试。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.AsrProviderRegistry.*：
 * - register：注册（同名覆盖）
 * - cascade：按 priority 升序的降级链（0 最高，先尝试）
 * - get / unregister / all / clear
 */
class AsrProviderRegistryTest {

    /** 构造最小 AsrProvider 测试桩（id + priority）。 */
    private fun stubProvider(id: String, priority: Int): AsrProvider {
        val info = AsrProviderInfo(
            id = id,
            name = "stub-$id",
            requiresNetwork = false,
            health = MutableStateFlow(TtsHealth.OK),
            priority = priority,
        )
        return object : AsrProvider {
            override val info = info
            override suspend fun listen(): String = ""
            override fun stopListening() {}
            override fun release() {}
        }
    }

    @Before
    fun setUp() {
        AsrProviderRegistry.clear()
    }

    @After
    fun tearDown() {
        AsrProviderRegistry.clear()
    }

    @Test
    fun registerThenGetReturnsProvider() {
        val sherpa = stubProvider("sherpa_sensevoice", 0)
        AsrProviderRegistry.register(sherpa)
        assertSame("get 应返回注册的实例", sherpa, AsrProviderRegistry.get("sherpa_sensevoice"))
    }

    @Test
    fun getUnknownIdReturnsNull() {
        assertNull("未注册的 id 应返回 null", AsrProviderRegistry.get("sherpa_sensevoice"))
    }

    @Test
    fun registerSameIdOverwrites() {
        val first = stubProvider("sherpa_sensevoice", 0)
        val second = stubProvider("sherpa_sensevoice", 1)
        AsrProviderRegistry.register(first)
        AsrProviderRegistry.register(second)
        assertSame("同名注册应覆盖旧实例", second, AsrProviderRegistry.get("sherpa_sensevoice"))
        assertEquals("同名覆盖后注册表只有 1 个实例", 1, AsrProviderRegistry.all().size)
    }

    @Test
    fun cascadeSortedByPriorityAscending() {
        // 乱序注册：system(1) 先、sherpa(0) 后 → cascade 应 sherpa 在前
        AsrProviderRegistry.register(stubProvider("system_speech_recognizer", 1))
        AsrProviderRegistry.register(stubProvider("sherpa_sensevoice", 0))

        val chain = AsrProviderRegistry.cascade()
        assertEquals(
            "降级链应为 sherpa → system（plan §4.4.4）",
            listOf("sherpa_sensevoice", "system_speech_recognizer"),
            chain.map { it.info.id }
        )
    }

    @Test
    fun unregisterRemovesProvider() {
        AsrProviderRegistry.register(stubProvider("sherpa_sensevoice", 0))
        AsrProviderRegistry.register(stubProvider("system_speech_recognizer", 1))
        AsrProviderRegistry.unregister("sherpa_sensevoice")
        assertNull(AsrProviderRegistry.get("sherpa_sensevoice"))
        assertEquals("仅剩 system", listOf("system_speech_recognizer"), AsrProviderRegistry.all().map { it.info.id })
    }

    @Test
    fun clearEmptiesRegistry() {
        AsrProviderRegistry.register(stubProvider("sherpa_sensevoice", 0))
        AsrProviderRegistry.clear()
        assertEquals("clear 后注册表应为空", 0, AsrProviderRegistry.all().size)
        assertEquals("clear 后降级链应为空", 0, AsrProviderRegistry.cascade().size)
    }
}
