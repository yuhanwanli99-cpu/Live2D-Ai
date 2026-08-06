package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * SherpaOnnxAsrProvider（V1.2 模块4：sherpa-onnx + SenseVoice 离线 ASR）单元测试。
 *
 * 契约见 docs/plan-v12-plan.schema.json → providersV12.sherpa_sensevoice：
 * - id="sherpa_sensevoice" / requiresNetwork=false / priority=0（ASR 降级链第一位）
 * - 模型未下载 → health=UNAVAILABLE，listen 快速返回 ""（不发网络、不触引擎）→ 降级到系统 ASR
 *
 * 真实录音 + JNI 识别无法在 JVM 单测运行（无麦克风/无 Android .so）；本测试验证
 * 降级路径与元数据契约。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class SherpaOnnxAsrProviderTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        SherpaOnnxEngineRegistry.clear()
    }

    @After
    fun tearDown() {
        SherpaOnnxEngineRegistry.clear()
    }

    private fun provider(): SherpaOnnxAsrProvider {
        val manager = ModelDownloadManager(context)
        manager.modelDir(ModelDownloadManager.SENSEVOICE_ASR).deleteRecursively()
        return SherpaOnnxAsrProvider(context, manager, ModelDownloadManager.SENSEVOICE_ASR)
    }

    @Test
    fun info_matchesSchemaContract() {
        val p = provider()
        assertEquals("sherpa_sensevoice", p.info.id)
        assertEquals("SenseVoice 离线 ASR", p.info.name)
        assertFalse("离线可用：requiresNetwork=false", p.info.requiresNetwork)
        assertEquals("ASR 降级链第一位：priority=0", 0, p.info.priority)
        assertEquals("Provider id 常量一致", SherpaOnnxAsrProvider.PROVIDER_ID, p.info.id)
    }

    @Test
    fun modelNotDownloaded_healthUnavailable_listenReturnsEmpty() = runBlocking {
        val p = provider()
        assertEquals("模型未下载初始 health 应为 UNAVAILABLE", TtsHealth.UNAVAILABLE, p.info.health.value)
        assertFalse(p.isModelReady())

        // 契约：失败返回空串（不抛异常），降级链继续 → 系统 ASR
        assertEquals("", p.listen())
        assertEquals("快速失败后 health 保持 UNAVAILABLE", TtsHealth.UNAVAILABLE, p.info.health.value)
        p.stopListening()
        p.release()
    }
}
