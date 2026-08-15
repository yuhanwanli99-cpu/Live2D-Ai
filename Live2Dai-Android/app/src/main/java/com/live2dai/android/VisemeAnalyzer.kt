package com.live2dai.android

import android.util.Log
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

// ============================================================================
// VisemeAnalyzer — Kotlin 移植 pymouth（MIT）VowelAnalyser + PymouthVisemeDriver
//
// Copyright (c) 2026 Sakura Motion Project
// SPDX-License-Identifier: MIT
//
// 本文件移植自 pymouth（https://github.com/organics2016/pymouth）：
//   VowelAnalyser（MFCC 提取 + DTW 模板匹配 + softmax）与 PymouthVisemeDriver
//   （VOWEL_MAP 映射）管线，DTW 模板常量逐字拷贝（契约 M4-M5 §3.5）。
// pymouth 许可：MIT License，Copyright (c) 2024 organics —— 依据其 MIT 条款
// 保留上游版权声明与许可文本（全文见 CREDITS.md §6.3）。
// ============================================================================

/**
 * Kotlin 移植 pymouth（Apache-2.0）VowelAnalyser + PymouthVisemeDriver 的元音分析器。
 * 输入 PCM 帧 → 输出 VisemeFrame 序列（label + 五元音参数，直接对齐 bai 的 MouthOpenY/MouthForm 映射）。
 *
 * 线程安全：非线程安全，单协程内使用（VoiceIoController 单收集协程保证）。
 */
class VisemeAnalyzer(
    /** 固定采样率（Hz），必须与 AudioFrame.sampleRate 一致；不一致的帧被忽略 */
    val sampleRate: Int,
    /** 分析窗口（ms）；默认 150；必须 > 0 */
    val frameMs: Int = DEFAULT_FRAME_MS,
    /** softmax 温度；默认 10.0（越大概率越平滑） */
    val temperature: Float = DEFAULT_TEMPERATURE,
) {
    companion object {
        const val DEFAULT_FRAME_MS: Int = 150
        const val DEFAULT_TEMPERATURE: Float = 10.0f

        /** 合法元音标签集合（与 pymouth _PYMOUTH_KEYS 值集一致） */
        val VOWEL_LABELS: Set<String> = setOf("A", "I", "U", "E", "O", "SIL")

        private const val TAG = "VisemeAnalyzer"

        // ── DTW 模板常量（analyser.py L7-48 原值，逐字拷贝，禁改）──

        internal val V_A = listOf(
            floatArrayOf(157.28203f, -18.084631f), floatArrayOf(184.55794f, -70.21922f),
            floatArrayOf(180.11708f, -69.37925f), floatArrayOf(186.3136f, -68.28505f),
            floatArrayOf(184.01038f, -62.837513f), floatArrayOf(181.70918f, -56.22454f),
            floatArrayOf(183.93184f, -55.787033f), floatArrayOf(177.58957f, -55.56417f),
            floatArrayOf(210.36815f, -42.245583f),
        )
        internal val V_I = listOf(
            floatArrayOf(154.23402f, -66.638794f), floatArrayOf(109.41006f, -76.42681f),
            floatArrayOf(104.646484f, -79.092224f), floatArrayOf(100.69435f, -73.16504f),
            floatArrayOf(106.442406f, -68.50664f), floatArrayOf(104.717026f, -67.13239f),
            floatArrayOf(98.70581f, -67.052155f), floatArrayOf(90.54036f, -64.939255f),
            floatArrayOf(96.68744f, -4.7852263f),
        )
        internal val V_U = listOf(
            floatArrayOf(177.38f, -21.598547f), floatArrayOf(163.56247f, -60.48896f),
            floatArrayOf(162.29306f, -65.40919f), floatArrayOf(147.10324f, -77.276535f),
            floatArrayOf(145.81586f, -81.49585f), floatArrayOf(141.00285f, -80.76966f),
            floatArrayOf(137.3823f, -83.69654f), floatArrayOf(125.162674f, -86.36088f),
            floatArrayOf(182.829f, -57.803486f),
        )
        internal val V_E = listOf(
            floatArrayOf(45.139225f, -3.552611f), floatArrayOf(82.19922f, -65.22143f),
            floatArrayOf(154.64558f, -90.886116f), floatArrayOf(153.86205f, -97.447235f),
            floatArrayOf(158.62253f, -77.25729f), floatArrayOf(155.23709f, -56.107735f),
            floatArrayOf(149.8651f, -64.127594f), floatArrayOf(145.0244f, -55.381966f),
            floatArrayOf(175.49411f, -33.858017f),
        )
        internal val V_O = listOf(
            floatArrayOf(224.24329f, -10.556553f), floatArrayOf(193.64792f, -22.315527f),
            floatArrayOf(186.96034f, -20.628428f), floatArrayOf(184.96133f, -16.40874f),
            floatArrayOf(186.04208f, -22.023905f), floatArrayOf(185.25256f, -11.928689f),
            floatArrayOf(185.24428f, -5.041305f), floatArrayOf(181.70624f, -3.2469425f),
            floatArrayOf(131.43114f, 3.3550904f),
        )
        internal val V_SILENCE = listOf(
            floatArrayOf(52.750603f, -4.9177713f), floatArrayOf(84.026146f, 21.443743f),
            floatArrayOf(77.42841f, 2.3937027f), floatArrayOf(85.50844f, 21.711079f),
            floatArrayOf(80.75325f, 7.308112f), floatArrayOf(82.622925f, 16.251738f),
            floatArrayOf(69.67996f, 10.19037f), floatArrayOf(80.5309f, 6.884417f),
            floatArrayOf(71.37026f, 22.482405f),
        )

        /** 全部模板列表（按 SIL, A, I, U, E, O 顺序——与 _audio2vowel prob 字典顺序一致） */
        internal val ALL_TEMPLATES: List<List<FloatArray>> = listOf(V_SILENCE, V_A, V_I, V_U, V_E, V_O)

        // ── pymouth VOWEL_MAP（pymouth_viseme.py，锁死；T2.2 U 修正）──

        /** 硬编码兜底（adapter 缺失/解析失败时，零行为变化；同步 bai raw 值（C27/C4），Bai 口型 = MouthOpenY/MouthForm 对） */
        private val FALLBACK_VOWEL_MAP: Map<String, FloatArray> = mapOf(
            "A" to floatArrayOf(0.8f, 0.4f),
            "I" to floatArrayOf(0.5f, 0.2f),
            "U" to floatArrayOf(0.4f, 0.1f),
            "E" to floatArrayOf(0.6f, 0.3f),
            "O" to floatArrayOf(0.7f, 0.5f),
            "SIL" to floatArrayOf(0.0f, 0.0f),
        )

        /**
         * 元音嘴型表 (mouthOpen, mouthForm)：配置优先（adapter.vowelRawMap，已为 bai raw 值），
         * 缺失 → 硬编码常量兜底（回归门）。buildVisemeFrame 公式不变（锁死）。
         */
        internal val VOWEL_MAP: Map<String, FloatArray>
            get() = ModelAdapterConfig.currentAdapter()?.vowelRawMap
                ?.mapValues { (_, pair) -> floatArrayOf(pair.first, pair.second) }
                ?: FALLBACK_VOWEL_MAP

        // ── MFCC/STFT 参数 ──

        /** n_fft 候选列表（analyser.py n_fft_ref_list） */
        internal val N_FFT_CANDIDATES = intArrayOf(128, 512, 1024, 2048, 4096)

        /** 每帧时长（ms）——发音帧 */
        internal const val FRAME_MS_23: Int = 23

        /** STFT hop length */
        internal const val HOP_LENGTH: Int = 512

        /** mel 滤波器数量 */
        internal const val N_MELS: Int = 128

        /** DCT 系数数量（取前 3，丢弃第 0 → MFCC1/MFCC2） */
        internal const val N_MFCC: Int = 3

        /** MFCC 帧数阈值：< 5 直接判 SIL */
        internal const val MIN_MFCC_FRAMES: Int = 5

        /** librosa power_to_db 默认 amin */
        internal const val AMIN: Float = 1e-10f

        /** librosa power_to_db 默认 top_db */
        internal const val TOP_DB: Float = 80.0f
    }

    // ── 实例状态 ──

    /** 样本缓冲（累积未处理样本） */
    private val sampleBuffer = mutableListOf<Float>()

    /** 时间戳追踪：当前缓冲窗口起点（ms，相对于重置起点） */
    private var bufferStartMs: Long = 0

    /** 已产出的全部 viseme */
    private val timeline = mutableListOf<VisemeFrame>()

    /** 每个窗口的样本数 */
    private val frameSize: Int

    init {
        require(frameMs > 0) { "frameMs must be positive, got $frameMs" }
        frameSize = sampleRate * frameMs / 1000
    }

    // ── 公开 API ──

    /**
     * 喂入一帧 PCM。内部缓冲样本，每累积满一个 frameMs 窗口即分析产出；
     * 单次调用会循环消费缓冲区中全部完整窗口（分批喂入 == 整段喂入，P2-1）。
     * @return 本次调用最后产出的 VisemeFrame；缓冲区不足一个窗口 → null（不产出帧）；
     *         frame.sampleRate 与构造不一致 → null（忽略该帧）；
     *         内部异常 → null（静默降级，Log.w 一次）
     */
    fun processFrame(frame: AudioFrame): VisemeFrame? {
        if (frame.sampleRate != sampleRate) {
            logW("sampleRate mismatch: frame=${frame.sampleRate}, analyzer=$sampleRate — frame ignored")
            return null
        }
        if (frame.samples.isEmpty()) return null

        try {
            // 累积到 sampleBuffer
            for (s in frame.samples) {
                sampleBuffer.add(s)
            }
            // 循环消费：缓冲区积累多少完整窗口就产出多少帧，
            // 分批喂入与整段喂入产出完全一致的 timeline（chunkSizeInvariance，P2-1）。
            // 返回最后产出的帧；无完整窗口（缓冲不足）→ null（不产出帧）。
            var last: VisemeFrame? = null
            while (sampleBuffer.size >= frameSize) {
                // 提取窗口
                val window = FloatArray(frameSize)
                for (i in 0 until frameSize) {
                    window[i] = sampleBuffer[i]
                }
                // 移除已消耗的样本
                repeat(frameSize) { sampleBuffer.removeAt(0) }

                val ts = bufferStartMs
                bufferStartMs += frameMs.toLong()

                last = analyzeWindow(window, ts)
            }
            return last
        } catch (e: Exception) {
            logW("processFrame error: ${e.message}")
            return null
        }
    }

    /** 本次会话（上次 reset 以来）已产出的全部 viseme，按 timestampMs 升序。 */
    fun getVisemeTimeline(): List<VisemeFrame> = timeline.toList()

    /** 清空内部缓冲与 timeline（每次新 speak 前由调用方调用）。 */
    fun reset() {
        sampleBuffer.clear()
        bufferStartMs = 0
        timeline.clear()
    }

    /** 日志输出带单元测试防护（本地 JVM 测试无 Android 运行时，直接调 Log 抛 "not mocked"）——与 EmotionController/VoiceIoController 同模式。 */
    private fun logW(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
        }
    }

    // ── 内部分析管线 ──

    private fun analyzeWindow(window: FloatArray, timestampMs: Long): VisemeFrame {
        // Step 1: MFCC 提取
        val mfccFrames = MfccExtractor.extract(window, sampleRate)

        // Step 2: 短窗规则 → 直接判 SIL
        if (mfccFrames.size < MIN_MFCC_FRAMES) {
            val frame = buildVisemeFrame("SIL", timestampMs)
            timeline.add(frame)
            return frame
        }

        // Step 3: DTW 分类
        val probs = VowelClassifier.classify(mfccFrames, temperature)

        // Step 4: argmax → label
        val label = LabelPicker.pick(probs)

        // Step 5: VOWEL_MAP + adapter（bai）→ VisemeFrame
        val frame = buildVisemeFrame(label, timestampMs)
        timeline.add(frame)
        return frame
    }

    private fun buildVisemeFrame(label: String, timestampMs: Long): VisemeFrame {
        val moMf = VOWEL_MAP[label] ?: VOWEL_MAP["SIL"]!!
        val mo = moMf[0]
        val mf = moMf[1]
        return VisemeFrame(
            timestampMs = timestampMs,
            vowelLabel = label,
            mouthOpen = mo,
            mouthForm = mf,
            paramA = (when (label) {
                "A" -> mo; "E" -> mo * 0.3f; else -> 0f
            }).coerceIn(0f, 1f),
            paramI = (when (label) {
                "I" -> mf; else -> 0f
            }).coerceIn(0f, 1f),
            paramU = (when (label) {
                "U" -> mf * 0.7f; "O" -> mf * 0.3f; else -> 0f
            }).coerceIn(0f, 1f),
            paramE = (when (label) {
                "E" -> mo * 0.7f; else -> 0f
            }).coerceIn(0f, 1f),
            paramO = (when (label) {
                "A" -> mf * 0.2f; "O" -> mo; else -> 0f
            }).coerceIn(0f, 1f),
        )
    }
}

// ============================================================================
// MfccExtractor — STFT → mel → log → DCT → MFCC1/2
// ============================================================================

internal object MfccExtractor {

    /**
     * 从 PCM 窗口提取 MFCC 帧序列（每帧 2 维：MFCC1, MFCC2）。
     * 移植 librosa.feature.mfcc 管线（n_fft 由 get_n_fft 决定，hop=512，mel=128，dct_type=1）。
     */
    fun extract(window: FloatArray, sampleRate: Int): List<FloatArray> {
        val nFft = getNfft(window.size, sampleRate)

        // 1. STFT: center=True, reflect pad, Hann window, power=2.0
        val powerSpec = stftPower(window, nFft, sampleRate)

        // 2. Mel filterbank (128 filters, fmin=0, fmax=sr/2, htk=false)
        val melSpec = applyMelFilterbank(powerSpec, nFft, sampleRate)

        // 3. Log: 10*log10(max(amin, S)), top_db=80 clip
        val logMelSpec = powerToDb(melSpec)

        // 4. DCT type 1, ortho, take first 3 coefs, discard [0]
        return dctMfcc(logMelSpec)
    }

    // ── get_n_fft（analyser.py 原逻辑）──

    internal fun getNfft(audioBlockSize: Int, sampleRate: Int, milliseconds: Int = 23): Int {
        val target = sampleRate * milliseconds / 1000
        if (target > audioBlockSize) return audioBlockSize
        return VisemeAnalyzer.N_FFT_CANDIDATES.minByOrNull {
            kotlin.math.abs(it - target)
        } ?: 512
    }

    // ── STFT 功率谱 ──

    private fun stftPower(signal: FloatArray, nFft: Int, sampleRate: Int): List<FloatArray> {
        val padWidth = nFft / 2
        val padded = reflectPad(signal, padWidth)
        val nFrames = 1 + (padded.size - nFft) / VisemeAnalyzer.HOP_LENGTH

        val hann = hannWindow(nFft)
        val frames = mutableListOf<FloatArray>()

        for (i in 0 until nFrames) {
            val start = i * VisemeAnalyzer.HOP_LENGTH
            // 提取窗口 + 加窗
            val re = FloatArray(nFft)
            val im = FloatArray(nFft)
            for (j in 0 until nFft) {
                re[j] = padded[start + j] * hann[j]
            }
            // FFT
            fft(re, im)
            // 功率谱（只取前 nFft/2+1 个 bin——但 mel filterbank 需要全部 bin）
            // librosa 的 melspectrogram 返回 (n_mels, n_frames)，其输入 STFT 为 (n_fft/2+1, n_frames)
            val nBins = nFft / 2 + 1
            val power = FloatArray(nBins)
            for (j in 0 until nBins) {
                power[j] = re[j] * re[j] + im[j] * im[j]
            }
            frames.add(power)
        }
        return frames
    }

    // ── Mel filterbank ──

    private fun applyMelFilterbank(powerFrames: List<FloatArray>, nFft: Int, sampleRate: Int): List<FloatArray> {
        val nBins = nFft / 2 + 1
        val filters = buildMelFilters(sampleRate, nFft, VisemeAnalyzer.N_MELS)
        return powerFrames.map { power ->
            val mel = FloatArray(VisemeAnalyzer.N_MELS)
            for (m in filters.indices) {
                var sum = 0f
                val f = filters[m]
                for (k in 0 until nBins) {
                    sum += power[k] * f[k]
                }
                mel[m] = sum
            }
            mel
        }
    }

    private fun buildMelFilters(sr: Int, nFft: Int, nMels: Int): List<FloatArray> {
        val nBins = nFft / 2 + 1
        val fMin = 0f
        val fMax = sr / 2f

        val melMin = hzToMel(fMin)
        val melMax = hzToMel(fMax)

        // nMels+2 个均匀 mel 点
        val melPoints = FloatArray(nMels + 2)
        for (i in melPoints.indices) {
            melPoints[i] = melMin + i * (melMax - melMin) / (nMels + 1)
        }

        // 转 Hz → FFT bin
        val binPoints = IntArray(nMels + 2)
        for (i in binPoints.indices) {
            val hz = melToHz(melPoints[i])
            binPoints[i] = ((nFft + 1) * hz / sr).toInt().coerceIn(0, nBins - 1)
        }

        // 构建三角滤波器
        val filters = mutableListOf<FloatArray>()
        for (m in 0 until nMels) {
            val left = binPoints[m]
            val center = binPoints[m + 1]
            val right = binPoints[m + 2]
            val filter = FloatArray(nBins)
            if (center > left) {
                for (k in left until center) {
                    filter[k] = (k - left).toFloat() / (center - left).toFloat()
                }
            }
            if (right > center) {
                for (k in center..right) {
                    filter[k] = (right - k).toFloat() / (right - center).toFloat()
                }
            }
            // 确保中心 bin 归 1
            if (center < nBins) filter[center] = max(filter[center], 1f)
            filters.add(filter)
        }
        return filters
    }

    private fun hzToMel(f: Float): Float = 2595f * log10(1f + f / 700f)
    private fun melToHz(m: Float): Float = 700f * (10f.pow(m / 2595f) - 1f)

    // ── Log (power_to_db) ──

    private fun powerToDb(melFrames: List<FloatArray>): List<FloatArray> {
        // 计算全局最大值（用于 top_db clip）
        var globalMax = Float.NEGATIVE_INFINITY
        for (frame in melFrames) {
            for (v in frame) {
                if (v > globalMax) globalMax = v
            }
        }
        val threshold = globalMax / 10f.pow(VisemeAnalyzer.TOP_DB / 10f) // 10^(top_db/10) below peak

        return melFrames.map { frame ->
            val db = FloatArray(frame.size)
            for (i in frame.indices) {
                val s = max(frame[i], VisemeAnalyzer.AMIN)
                val sDb = 10f * log10(s)
                db[i] = max(sDb, 10f * log10(max(threshold, VisemeAnalyzer.AMIN)))
            }
            db
        }
    }

    // ── DCT type 1, ortho, take n_mfcc=3, discard [0] → [MFCC1, MFCC2] ──

    private fun dctMfcc(logMelFrames: List<FloatArray>): List<FloatArray> {
        if (logMelFrames.isEmpty()) return emptyList()
        val N = logMelFrames[0].size // N_MELS = 128
        val result = mutableListOf<FloatArray>()
        // 预计算余弦表（所有帧共用）
        val cosTable = Array(VisemeAnalyzer.N_MFCC) { k ->
            FloatArray(N) { n ->
                cos(PI.toFloat() * k * n / (N - 1).toFloat())
            }
        }
        // ortho 归一化因子
        val scale = 1f / sqrt(2f * (N - 1).toFloat())
        val scale0 = scale / sqrt(2f)

        for (frame in logMelFrames) {
            val dct = FloatArray(VisemeAnalyzer.N_MFCC)
            for (k in 0 until VisemeAnalyzer.N_MFCC) {
                var sum = 0.0
                for (n in 0 until N) {
                    sum += frame[n].toDouble() * cosTable[k][n].toDouble()
                }
                dct[k] = (2.0 * sum).toFloat()
            }
            // ortho normalization
            dct[0] *= scale0
            for (k in 1 until VisemeAnalyzer.N_MFCC) {
                dct[k] *= scale
            }
            // discard [0], take [1], [2]
            result.add(floatArrayOf(dct[1], dct[2]))
        }
        return result
    }

    // ── Hann window ──

    private fun hannWindow(n: Int): FloatArray {
        return FloatArray(n) { i ->
            (0.5f * (1f - cos(2.0 * PI * i / (n - 1).toDouble()))).toFloat()
        }
    }

    // ── Reflect padding ──

    private fun reflectPad(signal: FloatArray, padWidth: Int): FloatArray {
        val n = signal.size
        val result = FloatArray(n + 2 * padWidth)
        for (i in 0 until padWidth) {
            result[i] = signal[padWidth - i]
        }
        signal.copyInto(result, padWidth)
        for (i in 0 until padWidth) {
            result[padWidth + n + i] = signal[n - 2 - i]
        }
        return result
    }

    // ── Radix-2 FFT（Cooley-Tukey，原地，仅支持 2 的幂）──

    private fun fft(re: FloatArray, im: FloatArray) {
        val n = re.size
        // 位逆序
        var j = 0
        for (i in 1 until n) {
            var bit = n shr 1
            while (j and bit != 0) {
                j = j xor bit
                bit = bit shr 1
            }
            j = j xor bit
            if (i < j) {
                val tr = re[i]; re[i] = re[j]; re[j] = tr
                val ti = im[i]; im[i] = im[j]; im[j] = ti
            }
        }
        // 蝶形
        var len = 2
        while (len <= n) {
            val halfLen = len shr 1
            val angle = (-2.0 * PI / len).toFloat()
            val wRe = cos(angle)
            val wIm = sin(angle)
            var i = 0
            while (i < n) {
                var curRe = 1f
                var curIm = 0f
                for (k in 0 until halfLen) {
                    val idxA = i + k
                    val idxB = i + k + halfLen
                    val tRe = curRe * re[idxB] - curIm * im[idxB]
                    val tIm = curRe * im[idxB] + curIm * re[idxB]
                    re[idxB] = re[idxA] - tRe
                    im[idxB] = im[idxA] - tIm
                    re[idxA] = re[idxA] + tRe
                    im[idxA] = im[idxA] + tIm
                    val nextRe = curRe * wRe - curIm * wIm
                    curIm = curRe * wIm + curIm * wRe
                    curRe = nextRe
                }
                i += len
            }
            len = len shl 1
        }
    }
}

// ============================================================================
// VowelClassifier — DTW × 6 → softmax → 概率
// ============================================================================

internal object VowelClassifier {

    /**
     * 对 MFCC 帧序列执行 6 次 DTW（symmetric2, euclidean, distance_only）→
     * 距离取负 → softmax(temperature) → FloatArray(6) 概率 [SIL,A,I,U,E,O]。
     */
    fun classify(mfccFrames: List<FloatArray>, temperature: Float): FloatArray {
        val distances = FloatArray(6)
        for (t in 0 until 6) {
            val tmpl = VisemeAnalyzer.ALL_TEMPLATES[t]
            distances[t] = dtwSymmetric2(tmpl, mfccFrames)
        }
        return softmax(distances, temperature)
    }

    // ── DTW symmetric2 (distance_only=True) ──

    /**
     * symmetric2 step pattern:
     *   D[i][j] = min(
     *       D[i-1][j-1] + 1 × d(i,j),     // diagonal
     *       D[i-1][j]   + 2 × d(i,j),     // horizontal (double weight)
     *       D[i][j-1]   + 2 × d(i,j)      // vertical (double weight)
     *   )
     * normalizedDistance = D[N-1][M-1] / (N + M)
     */
    private fun dtwSymmetric2(template: List<FloatArray>, query: List<FloatArray>): Float {
        val N = template.size
        val M = query.size
        if (N == 0 || M == 0) return Float.POSITIVE_INFINITY

        // 局部距离矩阵（按需计算，不存储全矩阵——逐行 DP）
        var prevRow = FloatArray(M) { Float.POSITIVE_INFINITY }

        for (i in 0 until N) {
            val curRow = FloatArray(M)
            for (j in 0 until M) {
                val dist = euclidean2d(template[i], query[j])
                if (i == 0 && j == 0) {
                    curRow[j] = dist
                } else if (i == 0) {
                    curRow[j] = prevRow[j - 1] + 2f * dist  // only horizontal from left
                } else if (j == 0) {
                    // only vertical from above
                    curRow[j] = prevRow[j] + 2f * dist
                } else {
                    val diag = prevRow[j - 1] + dist           // 1×
                    val horiz = prevRow[j] + 2f * dist         // 2×
                    val vert = curRow[j - 1] + 2f * dist       // 2×
                    curRow[j] = minOf(diag, minOf(horiz, vert))
                }
            }
            prevRow = curRow
        }

        val cost = prevRow[M - 1]
        return cost / (N + M).toFloat()
    }

    private fun euclidean2d(a: FloatArray, b: FloatArray): Float {
        val dx = a[0] - b[0]
        val dy = a[1] - b[1]
        return sqrt(dx * dx + dy * dy)
    }

    // ── Softmax ──

    /**
     * 带温度的 softmax：x = -distance；result[i] = exp((x[i]-maxX)/T) / sum(exp(...))
     */
    private fun softmax(distances: FloatArray, temperature: Float): FloatArray {
        val x = FloatArray(distances.size) { -distances[it] / temperature }
        val maxX = x.maxOrNull() ?: 0f
        var sum = 0f
        for (i in x.indices) {
            x[i] = exp(x[i] - maxX)
            sum += x[i]
        }
        for (i in x.indices) {
            x[i] /= sum
        }
        return x
    }
}

// ============================================================================
// LabelPicker — argmax → label
// ============================================================================

internal object LabelPicker {

    private val LABELS = arrayOf("SIL", "A", "I", "U", "E", "O")

    /**
     * 取概率最大的标签；空 → SIL。
     */
    fun pick(probs: FloatArray): String {
        if (probs.isEmpty()) return "SIL"
        var bestIdx = 0
        var bestVal = probs[0]
        for (i in 1 until probs.size) {
            if (probs[i] > bestVal) {
                bestVal = probs[i]
                bestIdx = i
            }
        }
        return LABELS.getOrElse(bestIdx) { "SIL" }
    }
}
