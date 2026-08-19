package com.live2d.ai.android.ui.settings

import com.live2d.ai.android.DownloadState
import com.live2d.ai.android.TtsHealth
import com.live2d.ai.android.TtsProvider
import com.live2d.ai.android.TtsProviderCategory
import com.live2d.ai.android.TtsProviderInfo
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 引擎选择 UI 纯函数测试（V1.2 模块5）。
 *
 * 覆盖 plan §5.5.2 方案A 的 UI 数据源映射与状态渲染逻辑：
 * - provider 列表 → EngineOptionUi（id/name/health/网络/选中态）
 * - 健康状态中文文案
 * - 模型下载状态文案 + 下载按钮可用性（"下载"触发判定）
 * - 降级链摘要
 */
class EngineSelectorUiTest {

    // ── 测试桩 ──

    private fun stubTts(
        id: String,
        name: String,
        health: TtsHealth = TtsHealth.OK,
        requiresNetwork: Boolean = false,
        configurable: Boolean = false,
    ): TtsProvider {
        val info = TtsProviderInfo(
            id = id,
            name = name,
            category = TtsProviderCategory.LOCAL,
            requiresNetwork = requiresNetwork,
            health = MutableStateFlow(health),
            priority = 1,
            configurable = configurable,
        )
        return object : TtsProvider {
            override val info = info
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
    }

    // ── TTS provider 列表 → 展示模型 ──

    @Test
    fun ttsProvidersToUi_mapsAllFields() {
        val providers = listOf(
            stubTts(
                id = "cosyvoice_ws",
                name = "CosyVoice 云端",
                health = TtsHealth.UNAVAILABLE,
                requiresNetwork = true,
                configurable = true,
            ),
            stubTts(id = "system_tts", name = "系统 TTS"),
        )
        val ui = ttsProvidersToUi(providers, selectedId = "system_tts")

        assertEquals("映射后条数与 provider 数一致", 2, ui.size)
        val cosy = ui[0]
        assertEquals("cosyvoice_ws", cosy.id)
        assertEquals("CosyVoice 云端", cosy.name)
        assertEquals("health 应取 StateFlow 当前快照值", TtsHealth.UNAVAILABLE, cosy.health)
        assertTrue("云端 provider requiresNetwork=true", cosy.requiresNetwork)
        assertTrue("configurable provider 应标记需配置", cosy.configurable)
        assertFalse("未选中项 isSelected=false", cosy.isSelected)

        val system = ui[1]
        assertTrue("选中项 isSelected=true", system.isSelected)
        assertFalse("离线 provider requiresNetwork=false", system.requiresNetwork)
    }

    @Test
    fun ttsProvidersToUi_marksSelectedBySelectedId() {
        val providers = listOf(
            stubTts(id = "a", name = "A"),
            stubTts(id = "b", name = "B"),
        )
        val ui = ttsProvidersToUi(providers, selectedId = "b")
        assertEquals(listOf(false, true), ui.map { it.isSelected })
    }

    @Test
    fun ttsProvidersToUi_unknownSelectedId_marksNoneSelected() {
        val providers = listOf(stubTts(id = "a", name = "A"))
        val ui = ttsProvidersToUi(providers, selectedId = "not_registered")
        assertTrue("注册表无该 id 时不应有选中项", ui.none { it.isSelected })
    }

    @Test
    fun ttsProvidersToUi_emptyList_returnsEmpty() {
        assertTrue(ttsProvidersToUi(emptyList(), selectedId = "x").isEmpty())
    }

    // ── 健康状态文案 ──

    @Test
    fun healthLabel_returnsChineseLabels() {
        assertEquals("正常", healthLabel(TtsHealth.OK))
        assertEquals("降级", healthLabel(TtsHealth.DEGRADED))
        assertEquals("不可用", healthLabel(TtsHealth.UNAVAILABLE))
    }

    // ── 模型下载状态 ──

    @Test
    fun downloadStatusLabel_coversAllStates() {
        assertEquals("未下载", downloadStatusLabel(null))
        assertEquals("未下载", downloadStatusLabel(DownloadState.Idle))
        assertEquals("下载中 42%", downloadStatusLabel(DownloadState.Downloading(0.42f, 42, 100)))
        assertEquals("校验中", downloadStatusLabel(DownloadState.Verifying))
        assertEquals("已下载", downloadStatusLabel(DownloadState.Completed("/models/kokoro")))
        assertEquals("下载失败", downloadStatusLabel(DownloadState.Failed("网络错误")))
    }

    @Test
    fun canStartDownload_allowsOnlyIdleAndFailed() {
        assertTrue("初始 null 可下载", canStartDownload(null))
        assertTrue("Idle 可下载", canStartDownload(DownloadState.Idle))
        assertTrue("失败可重试", canStartDownload(DownloadState.Failed("err")))
        assertFalse("下载中禁止并发双开", canStartDownload(DownloadState.Downloading(0.5f, 1, 2)))
        assertFalse("校验中禁止", canStartDownload(DownloadState.Verifying))
        assertFalse("已完成无需再下", canStartDownload(DownloadState.Completed("/x")))
    }

    // ── 降级链摘要 / 模型体积 ──

    @Test
    fun cascadeSummary_joinsProviderNamesWithArrow() {
        assertEquals(
            "CosyVoice 云端 → Kokoro 离线 → 系统 TTS",
            cascadeSummary(listOf("CosyVoice 云端", "Kokoro 离线", "系统 TTS"))
        )
        assertEquals("空链应返回空串", "", cascadeSummary(emptyList()))
    }

    @Test
    fun formatModelSize_readable() {
        assertEquals("82.0MB", formatModelSize(82L * 1024 * 1024))
        assertEquals("163.0MB", formatModelSize(163L * 1024 * 1024))
        assertEquals("1.0GB", formatModelSize(1024L * 1024 * 1024))
        assertEquals("512B", formatModelSize(512))
    }
}
