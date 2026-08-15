package com.live2dai.android

import android.util.Log
import java.io.File

/**
 * sherpa-onnx 引擎的类型化抽象层（V1.2 模块3/4）。
 *
 * 设计动机（见 task-v12-offline 调研报告）：sherpa-onnx 官方**不在 Maven Central 发布
 * Android AAR**（plan §3.3.3 的 `com.k2fsa.sherpa:onnx-tts-engine:1.12.0` 坐标实测不存在），
 * 引擎以 GitHub Releases 的 `.aar` 分发（`app/libs/sherpa-onnx-*.aar`，gitignore 不随仓库）。
 * 为保证无 AAR 环境（CI / 新克隆）仍可编译，引擎实现放在**条件源码集**
 * `src/sherpaOnnxEngine/kotlin/`（build.gradle.kts 按 AAR 是否存在决定是否参与编译），
 * 本文件只定义类型化接口；真实实现（SherpaOnnxEngineImpl）由
 * [SherpaOnnxEngineRegistry.tryBootstrap] 通过一次反射引导注册（静态自注册）。
 *
 * AAR 缺失时：factory 保持 null → Provider health=UNAVAILABLE → 降级链继续
 * （与模型未下载行为一致，plan §8.3 回退策略）。
 */

/** 合成结果：PCM 单声道 float 采样 + 采样率（对应 sherpa-onnx GeneratedAudio）。 */
data class SherpaTtsAudio(val samples: FloatArray, val sampleRate: Int)

/** 离线 TTS 引擎（sherpa-onnx OfflineTts 的类型化包装）。 */
interface SherpaTtsEngine {
    /**
     * 合成文本。
     * @param sid 音色序号（Kokoro voices.bin 内序号）
     * @param speed 语速（1.0 = 正常）
     * @return PCM 音频；合成失败返回 null（不抛）
     */
    fun synthesize(text: String, sid: Int, speed: Float): SherpaTtsAudio?

    /** 引擎采样率（WAV 头使用）。 */
    fun sampleRate(): Int

    /** 释放引擎（幂等）。 */
    fun release()
}

/** 离线 ASR 引擎（sherpa-onnx OfflineRecognizer 的类型化包装）。 */
interface SherpaAsrEngine {
    /**
     * 整段识别 PCM 音频。
     * @return 识别文本；无匹配/失败返回 ""
     */
    fun recognize(pcmSamples: FloatArray, sampleRate: Int): String

    /** 释放引擎（幂等）。 */
    fun release()
}

/** 引擎工厂：由条件源码集实现并注册（AAR 存在时）；createXxx 失败返回 null（不抛）。 */
interface SherpaOnnxEngineFactory {
    /** 创建离线 TTS 引擎；引擎不可用/初始化失败返回 null。 */
    fun createTts(modelDir: File): SherpaTtsEngine?

    /** 创建离线 ASR 引擎；引擎不可用/初始化失败返回 null。 */
    fun createAsr(modelDir: File): SherpaAsrEngine?
}

/**
 * 引擎注册表 + 引导入口。
 *
 * [tryBootstrap] 通过一次 `Class.forName("com.live2dai.android.sherpa.SherpaOnnxRegistrar")`
 * 触发条件源码集的静态初始化（自注册真实工厂）：
 * - AAR + 条件源码集存在 → 注册真实工厂（JNI 类随类加载触达 System.loadLibrary）
 * - AAR 缺失 → ClassNotFoundException → 静默，factory=null（Provider 降级）
 *
 * 线程安全：register/tryBootstrap/factoryOrNull 均加锁（synchronized 可重入，
 * Class.forName 静态初始化回调 register 不会死锁）。
 */
object SherpaOnnxEngineRegistry {
    private const val TAG = "SherpaOnnx"

    private val lock = Any()
    private var factory: SherpaOnnxEngineFactory? = null

    /** 注册工厂（条件源码集静态初始化时调用；测试可注入假工厂）。 */
    fun register(factory: SherpaOnnxEngineFactory) = synchronized(lock) {
        this.factory = factory
        Log.i(TAG, "sherpa-onnx 引擎工厂已注册")
    }

    /** 测试隔离：清空工厂（回到不可用状态）。 */
    fun clear() = synchronized(lock) {
        factory = null
    }

    /**
     * 尝试激活真实引擎（幂等；Provider 首次使用时调用）。
     * 失败（AAR 未集成）静默——Provider 保持 UNAVAILABLE 降级。
     */
    fun tryBootstrap() {
        synchronized(lock) {
            if (factory != null) return
            try {
                Class.forName("com.live2dai.android.sherpa.SherpaOnnxRegistrar")
            } catch (e: Throwable) {
                // ClassNotFoundException（无 AAR/条件源码集未编译）或 LinkageError
                Log.i(TAG, "sherpa-onnx 引擎不可用（AAR 未集成），离线 TTS/ASR 降级: ${e.message}")
            }
        }
    }

    /** 当前工厂（可能为 null）。 */
    fun factoryOrNull(): SherpaOnnxEngineFactory? = synchronized(lock) { factory }
}
