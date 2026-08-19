package com.live2d.ai.android.sherpa

import com.k2fsa.sherpa.onnx.FeatureConfig
import com.k2fsa.sherpa.onnx.GeneratedAudio
import com.k2fsa.sherpa.onnx.OfflineModelConfig
import com.k2fsa.sherpa.onnx.OfflineRecognizer
import com.k2fsa.sherpa.onnx.OfflineRecognizerConfig
import com.k2fsa.sherpa.onnx.OfflineSenseVoiceModelConfig
import com.k2fsa.sherpa.onnx.OfflineTts
import com.k2fsa.sherpa.onnx.OfflineTtsConfig
import com.k2fsa.sherpa.onnx.OfflineTtsKokoroModelConfig
import com.k2fsa.sherpa.onnx.OfflineTtsModelConfig
import com.live2d.ai.android.SherpaAsrEngine
import com.live2d.ai.android.SherpaOnnxEngineFactory
import com.live2d.ai.android.SherpaOnnxEngineRegistry
import com.live2d.ai.android.SherpaTtsAudio
import com.live2d.ai.android.SherpaTtsEngine
import java.io.File

/**
 * sherpa-onnx 真实引擎实现（条件源码集，仅当 `app/libs/sherpa-onnx-*.aar` 存在时参与编译）。
 *
 * 直接引用 AAR 的 Kotlin API（sherpa-onnx/kotlin-api）：OfflineTts / OfflineRecognizer，
 * JNI 类加载即触发 `System.loadLibrary("sherpa-onnx-jni")`；JVM 单测环境（无 Android .so）
 * 抛 UnsatisfiedLinkError → 由 [SherpaOnnxEngineImplFactory] 的 try/catch 转成 null →
 * Provider 降级（health=UNAVAILABLE）。
 *
 * API 要点（kotlin-api 源码实测）：所有 Config 均为 Kotlin `data class`（var 属性 + 非空默认值），
 * 不能用 null 参数、不能直接调用 setXxx 方法——用属性赋值 / 具名构造参数。
 *
 * 入口：[SherpaOnnxRegistrar] 静态初始化向 [SherpaOnnxEngineRegistry] 自注册。
 */

/** 条件源码集入口：主代码 `Class.forName` 引导时静态初始化，注册真实工厂。 */
object SherpaOnnxRegistrar {
    init {
        SherpaOnnxEngineRegistry.register(SherpaOnnxEngineImplFactory)
    }
}

/** 真实引擎工厂（创建失败/引擎不可用 → null，不抛异常给上层）。 */
internal object SherpaOnnxEngineImplFactory : SherpaOnnxEngineFactory {

    override fun createTts(modelDir: File): SherpaTtsEngine? = try {
        SherpaOnnxTtsEngineImpl(modelDir)
    } catch (e: Throwable) {
        android.util.Log.w("SherpaOnnx", "离线 TTS 引擎初始化失败: ${e.message}")
        null
    }

    override fun createAsr(modelDir: File): SherpaAsrEngine? = try {
        SherpaOnnxAsrEngineImpl(modelDir)
    } catch (e: Throwable) {
        android.util.Log.w("SherpaOnnx", "离线 ASR 引擎初始化失败: ${e.message}")
        null
    }
}

/**
 * Kokoro 离线 TTS 引擎（sherpa-onnx OfflineTts）。
 *
 * 模型文件（tar 解压后，见 ModelDownloadManager.KOKORO_TTS —— 用户裁定 B-9=Kokoro-82M-v1.1-zh）：
 * model.onnx / model.int8.onnx + tokens.txt + voices.bin + espeak-ng-data/ + lexicon-*.txt。
 * voices 参数为 voices.bin 文件路径（官方 Java/Kotlin API 语义，非 python 的逗号串）。
 *
 * v1.1-zh 配置对齐官方 Kotlin 示例（k2-fsa.github.io/sherpa/onnx/tts/all/Chinese-English/
 * kokoro-multi-lang-v1_1.html）：lang 留空 + lexicon 双文件（lexicon-us-en.txt + lexicon-zh.txt）；
 * dictDir 自 v1.12.15 起废弃（官方 Validate 仅告警，可忽略）。
 */
internal class SherpaOnnxTtsEngineImpl(modelDir: File) : SherpaTtsEngine {

    private val tts: OfflineTts

    init {
        val model = resolveModelFile(modelDir)
            ?: throw IllegalArgumentException("Kokoro 模型缺失（${modelDir.absolutePath} 下无 model*.onnx）")
        val tokens = File(modelDir, "tokens.txt")
        if (!tokens.exists()) throw IllegalArgumentException("Kokoro tokens 缺失: ${tokens.absolutePath}")
        val voices = File(modelDir, "voices.bin")

        val kokoro = OfflineTtsKokoroModelConfig(
            model = model.absolutePath,
            voices = voices.absolutePath,
            tokens = tokens.absolutePath,
            dataDir = File(modelDir, "espeak-ng-data").absolutePath,
            lexicon = lexiconFiles(modelDir),
            lang = "", // v1.1-zh 官方 Kotlin 示例：lang 留空，由 lexicon 双文件驱动 G2P（中英混排）
            dictDir = "", // unused（kotlin-api 注释）
            lengthScale = 1.0f,
        )
        val modelConfig = OfflineTtsModelConfig(
            kokoro = kokoro,
            numThreads = 2,
            debug = false,
            provider = "cpu",
        )
        val config = OfflineTtsConfig(
            model = modelConfig,
            maxNumSentences = 2,
        )
        // AssetManager=null → 模型走文件路径；类加载触发 loadLibrary，失败抛 UnsatisfiedLinkError
        tts = OfflineTts(config = config)
    }

    override fun sampleRate(): Int = tts.sampleRate()

    override fun synthesize(text: String, sid: Int, speed: Float): SherpaTtsAudio? {
        if (text.isBlank()) return null
        val audio: GeneratedAudio = tts.generate(text, sid = sid, speed = speed)
        val samples = audio.samples
        if (samples.isEmpty()) return null
        return SherpaTtsAudio(samples, audio.sampleRate)
    }

    override fun release() {
        try {
            tts.release()
        } catch (_: Throwable) {
        }
    }

    private fun resolveModelFile(dir: File): File? =
        listOf("model.onnx", "model.int8.onnx").map { File(dir, it) }.firstOrNull { it.exists() }

    /** lexicon 文件（逗号分隔；缺失时返回空串，Kokoro 内建 G2P 兜底）。 */
    private fun lexiconFiles(dir: File): String =
        listOf("lexicon-us-en.txt", "lexicon-zh.txt")
            .map { File(dir, it) }
            .filter { it.exists() }
            .joinToString(",") { it.absolutePath }
}

/**
 * SenseVoice 离线 ASR 引擎（sherpa-onnx OfflineRecognizer）。
 *
 * 模型文件：model.int8.onnx（或 model.onnx）+ tokens.txt。
 * 配置对齐官方 Java 示例（NonStreamingDecodeFileSenseVoice）：
 * - OfflineSenseVoiceModelConfig：model / language="zh" / useInverseTextNormalization
 * - OfflineModelConfig：senseVoice + tokens + numThreads + debug（无需显式 modelType，C++ 自动识别）
 * - OfflineRecognizerConfig：featConfig(16000Hz/80 维) + modelConfig + decodingMethod="greedy_search"
 */
internal class SherpaOnnxAsrEngineImpl(modelDir: File) : SherpaAsrEngine {

    private val recognizer: OfflineRecognizer

    init {
        val modelFile = resolveModelFile(modelDir)
            ?: throw IllegalArgumentException("SenseVoice 模型缺失（${modelDir.absolutePath} 下无 model*.onnx）")
        val tokensFile = File(modelDir, "tokens.txt")
        if (!tokensFile.exists()) throw IllegalArgumentException("SenseVoice tokens 缺失: ${tokensFile.absolutePath}")

        val featConfig = FeatureConfig().apply {
            sampleRate = 16000
            featureDim = 80
        }
        val senseVoice = OfflineSenseVoiceModelConfig().apply {
            model = modelFile.absolutePath
            language = "zh"
            useInverseTextNormalization = true
        }
        val modelConfig = OfflineModelConfig().apply {
            this.senseVoice = senseVoice
            numThreads = 2
            debug = false
            provider = "cpu"
            this.tokens = tokensFile.absolutePath
        }
        val config = OfflineRecognizerConfig().apply {
            this.featConfig = featConfig
            this.modelConfig = modelConfig
            decodingMethod = "greedy_search"
            maxActivePaths = 4
        }
        recognizer = OfflineRecognizer(config = config)
    }

    override fun recognize(pcmSamples: FloatArray, sampleRate: Int): String {
        if (pcmSamples.isEmpty()) return ""
        val stream = recognizer.createStream()
        try {
            stream.acceptWaveform(pcmSamples, sampleRate)
            recognizer.decode(stream)
            return recognizer.getResult(stream).text
        } catch (e: Throwable) {
            android.util.Log.w("SherpaOnnx", "ASR 识别异常: ${e.message}")
            return ""
        } finally {
            try {
                stream.release()
            } catch (_: Throwable) {
            }
        }
    }

    override fun release() {
        try {
            recognizer.release()
        } catch (_: Throwable) {
        }
    }

    private fun resolveModelFile(dir: File): File? =
        listOf("model.int8.onnx", "model.onnx").map { File(dir, it) }.firstOrNull { it.exists() }
}
