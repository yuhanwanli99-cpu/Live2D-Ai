package com.live2d.ai.android

import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * TtsProviderRegistry（V1.2 模块1 注册表 + 降级链）单元测试。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → functions.TtsProviderRegistry.*：
 * - register：注册（同名覆盖）
 * - cascade：按 priority 升序的降级链（0 最高，先尝试）
 * - get：按 id 查找
 * - all：全部 Provider 快照（供 UI）
 * - offlineOnly：仅离线可用
 */
class TtsProviderRegistryTest {

    /** 构造最小 TtsProvider 测试桩（id + priority，其余字段取默认值）。 */
    private fun stubProvider(id: String, priority: Int): TtsProvider {
        val health = MutableStateFlow(TtsHealth.OK)
        val info = TtsProviderInfo(
            id = id,
            name = "stub-$id",
            category = TtsProviderCategory.LOCAL,
            requiresNetwork = false,
            health = health,
            priority = priority,
            configurable = false,
        )
        return object : TtsProvider {
            override val info = info
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
    }

    @Before
    fun setUp() {
        TtsProviderRegistry.clear()
    }

    @After
    fun tearDown() {
        TtsProviderRegistry.clear()
    }

    // ─────────────────────────────────────────────
    // register / get
    // ─────────────────────────────────────────────

    @Test
    fun registerThenGetReturnsProvider() {
        val edge = stubProvider("edge_tts", 1)
        TtsProviderRegistry.register(edge)
        assertSame("get 应返回注册的实例", edge, TtsProviderRegistry.get("edge_tts"))
    }

    @Test
    fun getUnknownIdReturnsNull() {
        assertNull("未注册的 id 应返回 null", TtsProviderRegistry.get("cosyvoice_ws"))
    }

    @Test
    fun registerSameIdOverwrites() {
        val first = stubProvider("edge_tts", 1)
        val second = stubProvider("edge_tts", 2)
        TtsProviderRegistry.register(first)
        TtsProviderRegistry.register(second)
        assertSame("同名注册应覆盖旧实例", second, TtsProviderRegistry.get("edge_tts"))
        assertEquals("同名覆盖后注册表只有 1 个实例", 1, TtsProviderRegistry.all().size)
    }

    // ─────────────────────────────────────────────
    // cascade（降级链按 priority 升序）
    // ─────────────────────────────────────────────

    @Test
    fun cascadeSortedByPriorityAscending() {
        val system = stubProvider("system_tts", 2)
        val edge = stubProvider("edge_tts", 1)
        // 乱序注册：system 先、edge 后
        TtsProviderRegistry.register(system)
        TtsProviderRegistry.register(edge)

        val chain = TtsProviderRegistry.cascade()
        assertEquals("降级链长度应为 2", 2, chain.size)
        assertSame("priority=1 的 edge 应在前", edge, chain[0])
        assertSame("priority=2 的 system 应在后", system, chain[1])
    }

    @Test
    fun cascadePriorityZeroIsFirst() {
        // 模拟 CosyVoice(0) 注册后自动前移（plan §3.1.6：后续 Wave 插更优 priority）
        val cosy = stubProvider("cosyvoice_ws", 0)
        val edge = stubProvider("edge_tts", 1)
        val system = stubProvider("system_tts", 2)
        TtsProviderRegistry.register(edge)
        TtsProviderRegistry.register(system)
        TtsProviderRegistry.register(cosy)

        val chain = TtsProviderRegistry.cascade()
        assertSame("priority=0 应排最前", cosy, chain[0])
        assertEquals(
            "降级链顺序应为 cosy → edge → system",
            listOf("cosyvoice_ws", "edge_tts", "system_tts"),
            chain.map { it.info.id }
        )
    }

    // ─────────────────────────────────────────────
    // all / unregister / offlineOnly / clear
    // ─────────────────────────────────────────────

    @Test
    fun allReturnsAllRegistered() {
        TtsProviderRegistry.register(stubProvider("edge_tts", 1))
        TtsProviderRegistry.register(stubProvider("system_tts", 2))
        assertEquals(2, TtsProviderRegistry.all().size)
    }

    @Test
    fun unregisterRemovesProvider() {
        TtsProviderRegistry.register(stubProvider("edge_tts", 1))
        TtsProviderRegistry.register(stubProvider("system_tts", 2))
        TtsProviderRegistry.unregister("edge_tts")
        assertNull(TtsProviderRegistry.get("edge_tts"))
        assertNotNull(TtsProviderRegistry.get("system_tts"))
    }

    @Test
    fun offlineOnlyFiltersNetworkProviders() {
        val health = MutableStateFlow(TtsHealth.OK)
        val edge = stubProvider("edge_tts", 1)
        // 覆盖 requiresNetwork=true（云端 provider）
        val cloudInfo = edge.info.copy(category = TtsProviderCategory.CLOUD_WS, requiresNetwork = true)
        val cloudStub = object : TtsProvider {
            override val info = cloudInfo
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
        TtsProviderRegistry.register(cloudStub)
        TtsProviderRegistry.register(stubProvider("system_tts", 2))

        val offline = TtsProviderRegistry.offlineOnly()
        assertEquals("仅离线 provider 应被过滤", listOf("system_tts"), offline.map { it.info.id })
    }

    @Test
    fun clearEmptiesRegistry() {
        TtsProviderRegistry.register(stubProvider("edge_tts", 1))
        TtsProviderRegistry.clear()
        assertTrue("clear 后注册表应为空", TtsProviderRegistry.all().isEmpty())
        assertTrue("clear 后降级链应为空", TtsProviderRegistry.cascade().isEmpty())
    }
}
