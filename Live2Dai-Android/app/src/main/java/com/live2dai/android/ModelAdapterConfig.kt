package com.live2dai.android

import android.content.Context
import android.util.Log
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * 模型适配配置（contract-v3-adapter §1.3，T1.2）。
 *
 * 读 `shared/model-adapter/niziiro_mao.adapter.json`（Gradle copyModelAdapter 任务同步到
 * `src/main/assets/`，见 app/build.gradle.kts §1.4）。解析失败 → 返回 null（调用方回退
 * 现有硬编码常量，行为不变——降级链第一级）。
 *
 * 加载语义（contract-v3-adapter §3.1）：
 * - 惰性：首次调用 [load] 在 `Live2DView.initialize` 时触发，非类初始化时。
 * - 缓存：每次 [load] 的结果写入 [cached]，消费方（MotionAliasTable / IdleMotionSelector /
 *   VisemeAnalyzer / EmotionController）经 [currentAdapter] 读取；null → 各自硬编码常量兜底。
 * - 失败不粘滞：本次 [load] 失败 → 清空缓存并返回 null，下次可重试（测试回归门 A2/A3 顺序无关）。
 *
 * 双端共享单源：禁止在 Android 侧另建配置文件（只此一份 adapter JSON）。
 */
object ModelAdapterConfig {

    /** assets 中的适配配置文件名（Gradle copyModelAdapter 任务同步） */
    const val ASSET_NAME = "niziiro_mao.adapter.json"

    private const val TAG = "ModelAdapterConfig"

    /** motion 别名（tag → 模型内 motion 定位；group 可为空字符串 ""，非笔误） */
    data class MotionAlias(val group: String, val index: Int, val file: String)

    /** 解析成功后的适配数据（contract-v3-adapter §1.3 签名，字段锁死） */
    data class AdapterData(
        val modelId: String,
        val displayName: String,
        val motionAliases: Map<String, MotionAlias>,      // tag → (group, index, file)
        val vowelParamMap: Map<String, Map<String, Float>>, // label → param → value
        val vowelRawMap: Map<String, Pair<Float, Float>>,   // label → (mouthOpen, mouthForm)
        val idlePool: List<String>,
        val emotionIndex: Map<String, Int>,
        // A3（contract-bai-params-v0 §0）：参数驱动表情数据。缺省默认值 → 既有构造调用零回归。
        val emotionParamMap: Map<String, Map<String, Map<String, Float>>> = emptyMap(), // emotion → 强度档(_1/_2/_3) → 参数Id → 值
        val fadeMs: FadeMsConfig = FadeMsConfig(),
    )

    /** A3 fade 时长配置（contract-bai-params-v0 D4：协议 H 段单源；缺失 → 默认 300/500） */
    data class FadeMsConfig(
        val emotionSwitch: Int = 300,
        val emotionToNeutral: Int = 500,
    )

    /** 当前已加载的适配数据；未加载/加载失败 → null（调用方用硬编码常量兜底） */
    @Volatile
    private var cached: AdapterData? = null

    private val lock = Any()

    /**
     * 从 assets 读取并解析；失败 → null（调用方用现有常量兜底）。
     * 每次调用都实际读取（生产只在 Live2DView.initialize 调用一次，等价惰性加载+缓存）；
     * 成功 → 缓存；失败 → 清空缓存并返回 null（回归门 A2/A3 顺序无关，不粘滞）。
     * 线程安全。
     */
    fun load(context: Context): AdapterData? {
        val loaded = try {
            val raw = context.assets.open(ASSET_NAME)
                .use { it.readBytes().toString(Charsets.UTF_8) }
            parse(raw)
        } catch (e: Exception) {
            Log.w(TAG, "加载适配配置失败（assets 缺失/读取异常），使用硬编码常量兜底: ${e.message}")
            null
        }
        synchronized(lock) { cached = loaded }
        return loaded
    }

    /** 当前生效的适配数据（null = 未加载/失败 → 消费方走硬编码兜底）。 */
    fun currentAdapter(): AdapterData? = synchronized(lock) { cached }

    /**
     * 解析适配配置 JSON 字符串；损坏/缺关键字段 → null（调用方用硬编码常量兜底）。
     * 包内可见，供单元测试直接构造（不依赖 Android Context）。
     */
    internal fun parse(jsonText: String): AdapterData? {
        return try {
            val file = json.decodeFromString<AdapterFileDto>(jsonText)
            val vowelRaw = file.vowelRawMap.mapValues { (_, v) ->
                Pair(if (v.size > 0) v[0] else 0f, if (v.size > 1) v[1] else 0f)
            }
            if (!isSchemaValid(file, vowelRaw)) {
                Log.w(TAG, "适配配置 schema 约束不满足，使用硬编码常量兜底")
                return null
            }
            AdapterData(
                modelId = file.modelId,
                displayName = file.displayName,
                motionAliases = file.motionAliases.mapValues { (_, a) ->
                    MotionAlias(group = a.group, index = a.index, file = a.file)
                },
                vowelParamMap = file.vowelParamMap,
                vowelRawMap = vowelRaw,
                idlePool = file.idlePool,
                emotionIndex = file.emotionIndex,
                // A3：缺省 → 空 map / 默认 fade（AP-27 防御），与 DTO 可选字段对应
                emotionParamMap = file.emotionParamMap ?: emptyMap(),
                fadeMs = FadeMsConfig(
                    emotionSwitch = file.fadeMs?.emotionSwitch ?: 300,
                    emotionToNeutral = file.fadeMs?.emotionToNeutral ?: 500,
                ),
            )
        } catch (e: Exception) {
            Log.w(TAG, "适配配置 JSON 解析失败，使用硬编码常量兜底: ${e.message}")
            null
        }
    }

    /** 测试钩子：直接注入/清空缓存（null = 清空 → 消费方回退硬编码常量）。 */
    internal fun setForTest(data: AdapterData?) {
        synchronized(lock) { cached = data }
    }

    /** 测试钩子：清空缓存（回归门：load 失败后消费方逐字节等价于硬编码常量）。 */
    internal fun reset() {
        synchronized(lock) { cached = null }
    }

    /**
     * 最小 schema 校验（contract-v3-adapter §1.2 机械可校验约束的子集）：
     * 五元音 6 键齐全（buildVisemeFrame 依赖，缺失会 NPE → 宁可回退常量）、
     * 别名/表情/idle 池非空。违反 → 视为解析失败（null）。
     *
     * 无 motion 模型豁免（contract-android-v0 §0 路径①，bai 兼容点）：
     * 模型无任何 motion（motionAliases={} 且 idlePool=[]，如 bai）→ 豁免
     * motionAliases/idlePool 非空校验；有 motion 的模型（niziiro_mao）仍强制非空。
     * emotionIndex 不受豁免，始终强制非空。
     */
    private fun isSchemaValid(file: AdapterFileDto, vowelRaw: Map<String, Pair<Float, Float>>): Boolean {
        val vowels = setOf("A", "I", "U", "E", "O", "SIL")
        if (vowels.any { it !in file.vowelParamMap }) return false
        if (vowels.any { it !in vowelRaw }) return false
        val hasNoMotions = file.motionAliases.isEmpty() && file.idlePool.isEmpty()
        if (!hasNoMotions) {
            if (file.motionAliases.isEmpty()) return false
            if (file.idlePool.isEmpty()) return false
        }
        if (file.emotionIndex.isEmpty()) return false
        // A3 D6（contract-bai-params-v0 §0，"出现即完整"）：emotionParamMap 字段可选；
        // 存在（非 null）则强制 8 键 × 每键 _1/_2/_3 完整且每帧非空，否则解析失败（fail-fast）。
        // 缺失（null，如 niziiro_mao）→ 不校验（A3U-09 零回归门）。
        val epm = file.emotionParamMap
        if (epm != null) {
            val standardEmotions = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
            if (epm.keys != standardEmotions) return false
            for (tiers in epm.values) {
                if (tiers.keys != setOf("_1", "_2", "_3")) return false
                for (frame in tiers.values) {
                    if (frame.isEmpty()) return false
                }
            }
        }
        return true
    }

    // ── 序列化 DTO（adapter JSON 结构；degradation 为文档字段，ignoreUnknownKeys 忽略）──

    @Serializable
    private data class MotionAliasDto(val group: String, val index: Int, val file: String)

    @Serializable
    private data class FadeMsDto(
        val emotionSwitch: Int = 300,
        val emotionToNeutral: Int = 500,
    )

    @Serializable
    private data class AdapterFileDto(
        val modelId: String,
        val displayName: String,
        val motionAliases: Map<String, MotionAliasDto>,
        val vowelParamMap: Map<String, Map<String, Float>>,
        val vowelRawMap: Map<String, List<Float>>,   // [mouthOpen, mouthForm]
        val idlePool: List<String>,
        val emotionIndex: Map<String, Int>,
        // A3：可选字段（缺省 null → 不校验；JSON 缺省 → ignoreUnknownKeys 无关，声明即解析）
        val emotionParamMap: Map<String, Map<String, Map<String, Float>>>? = null,
        val fadeMs: FadeMsDto? = null,
    )

    private val json = Json { ignoreUnknownKeys = true }
}
