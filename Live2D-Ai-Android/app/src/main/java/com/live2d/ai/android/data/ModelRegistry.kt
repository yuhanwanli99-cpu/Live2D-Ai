package com.live2d.ai.android.data

import android.content.Context
import android.util.Log
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Live2D 模型注册表条目。
 *
 * 与 `shared/model_registry.json` 的 models[] 条目一一对应，
 * 字段定义与 `plan-registry.schema.json` 的 ModelEntry 完全一致。
 */
@Serializable
data class ModelMetadata(
    val creator: String? = null,
    val license: String? = null,
    val version: String? = null,
    val status: String? = null
)

/**
 * Live2D 模型注册表条目。
 *
 * 与 `shared/model_registry.json` 的 models[] 条目一一对应，
 * 字段定义与 `plan-registry.schema.json` 的 ModelEntry 完全一致。
 */
@Serializable
data class ModelEntry(
    val id: String,
    val name: String,
    val description: String? = null,
    val model3JsonPath: String,
    val thumbnailPath: String? = null,
    val available: Boolean,
    val default: Boolean,
    val restrictPlatform: String? = null,
    val metadata: ModelMetadata? = null
)

/**
 * 注册表 JSON 文件的顶层结构。
 */
@Serializable
data class ModelRegistryFile(
    val version: String,
    val models: List<ModelEntry>
)

/**
 * 跨平台 Live2D 模型注册表（Android 端读取器）。
 *
 * 数据来源：`assets/model_registry.json`（构建时由 Gradle copyModelRegistry
 * 任务从 `shared/model_registry.json` 复制，见 app/build.gradle.kts）。
 *
 * 实例创建后字段不可变（val），查询方法均为纯内存读操作，天然线程安全。
 *
 * ## 兜底策略（与 plan-registry 决策一致）
 * 注册表缺失 / 损坏 / models 为空 → 返回只含硬编码 `bai` 条目的兜底实例，
 * 保证任何情况下 `getDefault()` 都不抛异常。
 * 兜底默认 = registry 默认 = bai（G4 统一，D3 定案，见 .pi/spoq/architecture-android-fix.md）。
 */
class ModelRegistry private constructor(
    private val entries: List<ModelEntry>
) {

    /** 按 ID 查找模型条目，不存在返回 null。 */
    fun getById(id: String): ModelEntry? =
        entries.firstOrNull { it.id == id }

    /** 返回所有注册条目（含 available=false 的未上线模型），不可变列表。 */
    fun getAll(): List<ModelEntry> = entries

    /**
     * 返回当前平台可用条目：`available=true` 且 `restrictPlatform != "pc"`。
     * 预留平台过滤逻辑（当前数据无 pc 限定条目）。
     */
    fun getAvailable(): List<ModelEntry> =
        entries.filter { it.available && it.restrictPlatform != "pc" }

    /**
     * 默认模型，按优先级：
     * 1. `default=true` 的第一个条目
     * 2. 第一个 `available=true` 且非 pc 限定的条目
     * 3. 硬编码 `bai` 兜底（与 shared/model_registry.json 默认条目一致）
     * 不抛异常，最坏情况返回硬编码条目。
     */
    fun getDefault(): ModelEntry =
        entries.firstOrNull { it.default }
            ?: entries.firstOrNull { it.available && it.restrictPlatform != "pc" }
            ?: FALLBACK_ENTRY

    /** 便捷方法：直接返回某模型的 model3.json 相对路径（不含 `live2d/` 前缀），不存在返回 null。 */
    fun getModel3JsonPath(id: String): String? =
        getById(id)?.model3JsonPath

    companion object {
        private const val TAG = "ModelRegistry"

        /** assets 中的注册表文件名（由 Gradle copyModelRegistry 任务生成）。 */
        private const val ASSET_NAME = "model_registry.json"

        /** 硬编码兜底条目 — 与 shared/model_registry.json 的默认条目（bai）保持一致（G4 定案，D3）。 */
        val FALLBACK_ENTRY = ModelEntry(
            id = "bai",
            name = "白-免费版 (Bai Free)",
            description = "白-免费版 — B站 Live2D 模型（16384 纹理，128 参数，无 motion/exp3），打磨默认形象",
            model3JsonPath = "bai/runtime/bai.model3.json",
            thumbnailPath = null,
            available = true,
            default = true,
            restrictPlatform = null,
            metadata = ModelMetadata(
                creator = "Bilibili",
                license = "本地使用（D1 裁定，不内置分发）",
                version = "1.0"
            )
        )

        private val json = Json { ignoreUnknownKeys = true }

        /**
         * 从 assets/model_registry.json 加载并解析注册表。
         * 任何异常（文件缺失、JSON 损坏）都内部消化，返回兜底实例，不向外抛出。
         */
        fun load(context: Context): ModelRegistry {
            return try {
                val raw = context.assets.open(ASSET_NAME)
                    .use { it.readBytes().toString(Charsets.UTF_8) }
                parse(raw)
            } catch (e: Exception) {
                Log.w(TAG, "加载模型注册表失败（assets 缺失或读取异常），使用兜底 bai: ${e.message}")
                fallback()
            }
        }

        /**
         * 解析注册表 JSON 字符串。损坏 JSON / 空 models → 兜底实例，不抛出。
         * 包内可见，供单元测试直接构造。
         */
        internal fun parse(jsonText: String): ModelRegistry {
            return try {
                val file = json.decodeFromString<ModelRegistryFile>(jsonText)
                if (file.models.isEmpty()) {
                    Log.w(TAG, "注册表 models 为空数组，使用兜底 bai")
                    fallback()
                } else {
                    ModelRegistry(file.models)
                }
            } catch (e: Exception) {
                Log.w(TAG, "注册表 JSON 解析失败，使用兜底 bai: ${e.message}")
                fallback()
            }
        }

        /** 构造只含硬编码 bai 条目的兜底实例。 */
        internal fun fallback(): ModelRegistry = ModelRegistry(listOf(FALLBACK_ENTRY))
    }
}
