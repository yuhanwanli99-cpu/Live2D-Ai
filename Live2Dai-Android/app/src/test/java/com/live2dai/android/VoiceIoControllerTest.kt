package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.mock
import org.mockito.kotlin.verify
import org.mockito.kotlin.verifyNoInteractions
import org.mockito.kotlin.whenever

/**
 * VoiceIoController（V1.1 模块1 语音闭环协调器）单元测试。
 *
 * EdgeTtsService / VoiceInputService 以 Mockito mock 注入，验证控制器的
 * 接线语义（转发 / 容错 / 打断 / 释放 / 空文本跳过），不触碰真实网络与系统服务。
 *
 * 契约见 docs/plan-v11-plan.schema.json → functions.VoiceIoController.*：
 * - speak(text): 播报纯文本，失败静默降级（throws: []）
 * - stopSpeaking(): 同步打断当前播报
 * - listen(): 返回识别文本；失败返回 ""
 * - release(): 释放 TTS + ASR 资源
 */
class VoiceIoControllerTest {

    private lateinit var context: Context
    private lateinit var ttsService: EdgeTtsService
    private lateinit var asrService: VoiceInputService
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        // context 仅用于构造函数签名兼容（服务均以 mock 注入，控制器不会真正调用 context）
        context = mock<Context>()
        ttsService = mock()
        asrService = mock()
        controller = VoiceIoController(context, ttsService, asrService)
    }

    // ─────────────────────────────────────────────
    // speak（说）
    // ─────────────────────────────────────────────

    @Test
    fun testSpeakForwardsTextToTts() = runTest {
        controller.speak("主人好喵")
        runBlocking { verify(ttsService).speak("主人好喵") }
    }

    @Test
    fun testSpeakSkipsBlankText() = runTest {
        controller.speak("   ")
        verifyNoInteractions(ttsService)
    }

    @Test
    fun testSpeakSwallowsTtsException() = runTest {
        // TTS 内部抛异常 → 控制器必须静默吞掉，不向调用方抛错（契约 throws: []）
        runBlocking { whenever(ttsService.speak("测试")).thenThrow(RuntimeException("tts boom")) }
        controller.speak("测试")
    }

    // ─────────────────────────────────────────────
    // listen（听）
    // ─────────────────────────────────────────────

    @Test
    fun testListenReturnsRecognizedText() = runTest {
        runBlocking { whenever(asrService.listen()).thenReturn("你好世界") }
        assertEquals("你好世界", controller.listen())
    }

    @Test
    fun testListenReturnsEmptyOnAsrFailure() = runTest {
        // ASR 抛异常 → 返回空字符串（UI 无变化，降级为文本输入）
        runBlocking { whenever(asrService.listen()).thenThrow(RuntimeException("asr boom")) }
        assertEquals("", controller.listen())
    }

    // ─────────────────────────────────────────────
    // stopSpeaking（打断）
    // ─────────────────────────────────────────────

    @Test
    fun testStopSpeakingStopsTts() {
        controller.stopSpeaking()
        verify(ttsService).stop()
    }

    // ─────────────────────────────────────────────
    // release（释放）
    // ─────────────────────────────────────────────

    @Test
    fun testReleaseReleasesTtsAndAsr() {
        controller.release()
        verify(ttsService).release()
        verify(asrService).stopListening()
    }
}
