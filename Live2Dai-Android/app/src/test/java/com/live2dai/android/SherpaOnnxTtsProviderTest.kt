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
import java.io.File

/**
 * SherpaOnnxTtsProvider（V1.2 模块3：sherpa-onnx + Kokoro 离线 TTS）单元测试。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → providersV12.sherpa_onnx_kokoro：
 * - id="sherpa_onnx_kokoro" / LOCAL / requiresNetwork=false / priority=1 / configurable=false
 * - 模型未下载 → health=UNAVAILABLE，speak 快速失败（不发网络、不触引擎）→ 降级链继续
 * - 引擎不可用（AAR 未集成 / JVM 无原生库）→ speak 抛异常、health=UNAVAILABLE
 *
 * 说明：真实 JNI 合成无法在 JVM 单测运行（无 Android .so）；本测试验证降级路径与
 * 元数据契约。模型文件就绪 + 引擎不可用场景（第 4 个用例）同时覆盖"无 AAR 环境"与
 * "JVM 环境原生库不可用"两种真实降级。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class SherpaOnnxTtsProviderTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        SherpaOnnxEngineRegistry.clear()
        TtsProviderRegistry.clear()
    }

    @After
    fun tearDown() {
        SherpaOnnxEngineRegistry.clear()
        TtsProviderRegistry.clear()
    }

    private fun provider(): SherpaOnnxTtsProvider {
        val manager = ModelDownloadManager(context)
        // 清理测试模型目录，保证"未下载"初始状态
        manager.modelDir(ModelDownloadManager.KOKORO_TTS).deleteRecursively()
        return SherpaOnnxTtsProvider(context, manager, ModelDownloadManager.KOKORO_TTS)
    }

    // ─────────────────────────────────────────────
    // 元数据契约
    // ─────────────────────────────────────────────

    @Test
    fun info_matchesSchemaContract() {
        val p = provider()
        assertEquals("sherpa_onnx_kokoro", p.info.id)
        assertEquals("Kokoro 离线", p.info.name)
        assertEquals(TtsProviderCategory.LOCAL, p.info.category)
        assertFalse("离线可用：requiresNetwork=false", p.info.requiresNetwork)
        assertEquals("CosyVoice(0) 之后的 Tier 2：priority=1", 1, p.info.priority)
        assertFalse("无用户可配置参数", p.info.configurable)
        assertEquals("Provider id 常量一致", SherpaOnnxTtsProvider.PROVIDER_ID, p.info.id)
    }

    // ─────────────────────────────────────────────
    // 模型未下载 → 快速失败（核心降级路径）
    // ─────────────────────────────────────────────

    @Test
    fun modelNotDownloaded_healthUnavailable_speakFailsFast() = runBlocking {
        val p = provider()
        assertEquals("模型未下载初始 health 应为 UNAVAILABLE", TtsHealth.UNAVAILABLE, p.info.health.value)
        assertFalse(p.isModelReady())

        // 快速失败（不发网络、不触引擎）——失败原因指向模型未下载
        val exception = try {
            p.speak("你好世界")
            throw AssertionError("speak 应快速失败")
        } catch (e: IllegalStateException) {
            e
        }
        assertTrue("失败原因应指向模型未下载: ${exception.message}", exception.message!!.contains("未下载"))
        assertEquals("失败后 health 保持 UNAVAILABLE", TtsHealth.UNAVAILABLE, p.info.health.value)
        p.stop()
        p.release()
    }

    @Test
    fun speak_blankTextSkipped() = runBlocking {
        val p = provider()
        p.speak("   ") // 空白文本：直接跳过，不抛异常
        p.release()
    }

    // ─────────────────────────────────────────────
    // 模型就绪但引擎不可用（JVM 无原生库 / AAR 未集成）
    // ─────────────────────────────────────────────
    // 该场景两种环境行为一致：createTts 返回 null → speak 抛异常、health=UNAVAILABLE

    @Test
    fun modelReadyButEngineUnavailable_speakFailsAndDegrades() = runBlocking {
        val manager = ModelDownloadManager(context)
        val modelDir = manager.modelDir(ModelDownloadManager.KOKORO_TTS)
        modelDir.deleteRecursively()
        modelDir.mkdirs()
        // 伪造已就绪模型文件（模拟下载完成）
        File(modelDir, "model.onnx").writeBytes(ByteArray(4))
        File(modelDir, "tokens.txt").writeText("t")
        File(modelDir, "voices.bin").writeBytes(ByteArray(4))

        val p = SherpaOnnxTtsProvider(context, manager, ModelDownloadManager.KOKORO_TTS)
        assertEquals("模型就绪初始 health 应为 OK", TtsHealth.OK, p.info.health.value)
        assertTrue(p.isModelReady())

        // JVM 环境：引擎初始化必失败（System.loadLibrary UnsatisfiedLinkError / 或 AAR 缺失 ClassNotFound）
        val exception = try {
            p.speak("你好")
            throw AssertionError("speak 应因引擎不可用而失败")
        } catch (e: IllegalStateException) {
            e
        }
        assertTrue("失败原因应指向引擎不可用: ${exception.message}", exception.message!!.contains("引擎不可用"))
        assertEquals("引擎失败后 health 应为 UNAVAILABLE", TtsHealth.UNAVAILABLE, p.info.health.value)
        p.release()
    }

    // ─────────────────────────────────────────────
    // wav 头（纯文件逻辑）
    // ─────────────────────────────────────────────

    @Test
    fun wavHeader_writtenLittleEndianPcm() {
        // writeWav：44 字节 RIFF 头 + 16-bit PCM 数据（不依赖引擎，纯文件逻辑）
        val p = provider()
        val file = File(context.cacheDir, "wav_test.wav")
        p.writeWav(file, floatArrayOf(0f, 0.5f, -0.5f), 16000)
        val bytes = file.readBytes()
        assertTrue(bytes.size == 44 + 6)
        assertEquals("RIFF".toByteArray().toList(), bytes.copyOfRange(0, 4).toList())
        assertEquals("WAVE".toByteArray().toList(), bytes.copyOfRange(8, 12).toList())
        // 第二个采样 0.5 → 0.5*32767=16383 (0x3FFF)，LE 写入
        assertEquals(0xFF, bytes[46].toInt() and 0xFF)
        assertEquals(0x3F, bytes[47].toInt() and 0xFF)
        file.delete()
        p.release()
    }
}
