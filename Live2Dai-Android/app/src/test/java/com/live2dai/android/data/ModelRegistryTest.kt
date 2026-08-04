package com.live2dai.android.data

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * ModelRegistry 单元测试。
 *
 * 覆盖 plan-android.schema.json testContracts 中的 ModelRegistryTest 用例：
 * - 解析正常 JSON → models 列表正确（含真实 assets 集成用例）
 * - 解析损坏 JSON → 返回只含 mao_pro 的兜底实例
 * - getById 命中 / 未命中
 * - getAvailable 过滤（available=false + restrictPlatform=pc）
 * - getDefault 优先级（default=true → 首个 available → 硬编码 mao_pro）
 * - getModel3JsonPath 正常与 null 分支
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ModelRegistryTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    // ─────────────────────────────────────────────────────────────
    //  load()：读取真实 assets/model_registry.json（Gradle copyModelRegistry 生成）
    // ─────────────────────────────────────────────────────────────

    @Test
    fun load_parsesRealRegistryFromAssets() {
        val registry = ModelRegistry.load(context)
        val all = registry.getAll()

        // 真实注册表（shared/model_registry.json）包含 4 个条目
        assertTrue("注册表应包含至少 2 个模型，实际 ${all.size}", all.size >= 2)

        val mao = registry.getById("mao_pro")
        assertNotNull("mao_pro 应存在", mao)
        assertEquals("mao_pro", mao!!.id)
        assertTrue("mao_pro 应 available", mao.available)
        assertEquals("mao_pro/runtime/mao_pro.model3.json", mao.model3JsonPath)

        // 唯一 default 且可用（plan-registry 决策：default 必须唯一）
        val defaults = all.filter { it.default }
        assertEquals("注册表应只有一个 default 条目", 1, defaults.size)
        assertEquals("mao_pro", defaults[0].id)
        assertTrue("default 条目应 available", defaults[0].available)

        // 已登记但未上线的模型应保留在 getAll() 中
        val unavailable = all.filter { !it.available }
        assertTrue("注册表中应有 unavailable 条目", unavailable.isNotEmpty())
    }

    // ─────────────────────────────────────────────────────────────
    //  parse()：内联 JSON 构造（不依赖 assets）
    // ─────────────────────────────────────────────────────────────

    private val validJson = """
        {
          "version": "1.0",
          "models": [
            {
              "id": "mao_pro",
              "name": "Mao Pro",
              "description": "默认形象",
              "model3JsonPath": "mao_pro/runtime/mao_pro.model3.json",
              "available": true,
              "default": true
            },
            {
              "id": "shizuku",
              "name": "Shizuku",
              "description": "官方示例角色",
              "model3JsonPath": "shizuku/runtime/shizuku.model3.json",
              "available": true,
              "default": false
            },
            {
              "id": "pc_only",
              "name": "PC Only",
              "model3JsonPath": "pc_only/runtime/pc_only.model3.json",
              "available": true,
              "default": false,
              "restrictPlatform": "pc"
            },
            {
              "id": "hiyori",
              "name": "Hiyori",
              "model3JsonPath": "hiyori/runtime/hiyori.model3.json",
              "available": false,
              "default": false
            }
          ]
        }
    """.trimIndent()

    @Test
    fun parse_validJson_modelsListCorrect() {
        val registry = ModelRegistry.parse(validJson)
        val all = registry.getAll()
        assertEquals(4, all.size)
        assertEquals(listOf("mao_pro", "shizuku", "pc_only", "hiyori"), all.map { it.id })
        // 字段映射正确
        val shizuku = registry.getById("shizuku")
        assertEquals("Shizuku", shizuku!!.name)
        assertEquals("官方示例角色", shizuku.description)
        assertEquals("shizuku/runtime/shizuku.model3.json", shizuku.model3JsonPath)
        assertTrue(shizuku.available)
        assertFalse(shizuku.default)
    }

    @Test
    fun parse_corruptJson_returnsFallbackWithOnlyMaoPro() {
        val registry = ModelRegistry.parse("{ this is not valid json !!!")
        val all = registry.getAll()
        assertEquals("损坏 JSON 应回退为只含 mao_pro 的兜底实例", listOf("mao_pro"), all.map { it.id })
        val fallback = all[0]
        assertEquals("mao_pro/runtime/mao_pro.model3.json", fallback.model3JsonPath)
        assertTrue(fallback.available)
        assertTrue(fallback.default)
    }

    @Test
    fun parse_emptyModelsArray_returnsFallback() {
        val registry = ModelRegistry.parse("""{"version":"1.0","models":[]}""")
        assertEquals(listOf("mao_pro"), registry.getAll().map { it.id })
    }

    @Test
    fun parse_missingDefaultField_ignoresUnknownKeysAndToleratesNulls() {
        // 缺少可选字段（description/thumbnailPath/restrictPlatform/metadata）应能解析
        val registry = ModelRegistry.parse(validJson)
        assertEquals(4, registry.getAll().size)
    }

    // ─────────────────────────────────────────────────────────────
    //  getById
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getById_hit_returnsEntry() {
        val registry = ModelRegistry.parse(validJson)
        val entry = registry.getById("shizuku")
        assertNotNull(entry)
        assertEquals("shizuku", entry!!.id)
    }

    @Test
    fun getById_miss_returnsNull() {
        val registry = ModelRegistry.parse(validJson)
        assertNull(registry.getById("nonexistent"))
    }

    // ─────────────────────────────────────────────────────────────
    //  getAvailable
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getAvailable_filtersUnavailableAndPcRestricted() {
        val registry = ModelRegistry.parse(validJson)
        val available = registry.getAvailable()
        assertEquals(
            "仅 available=true 且非 pc 限定",
            listOf("mao_pro", "shizuku"),
            available.map { it.id }
        )
    }

    // ─────────────────────────────────────────────────────────────
    //  getDefault
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getDefault_prefersDefaultFlag() {
        val registry = ModelRegistry.parse(validJson)
        assertEquals("mao_pro", registry.getDefault().id)
    }

    @Test
    fun getDefault_multipleDefaults_takesFirst() {
        val json = """
            {
              "version": "1.0",
              "models": [
                { "id": "a", "name": "A", "model3JsonPath": "a/a.model3.json", "available": true, "default": true },
                { "id": "b", "name": "B", "model3JsonPath": "b/b.model3.json", "available": true, "default": true }
              ]
            }
        """.trimIndent()
        assertEquals("a", ModelRegistry.parse(json).getDefault().id)
    }

    @Test
    fun getDefault_noDefault_takesFirstAvailable() {
        val json = """
            {
              "version": "1.0",
              "models": [
                { "id": "a", "name": "A", "model3JsonPath": "a/a.model3.json", "available": false, "default": false },
                { "id": "b", "name": "B", "model3JsonPath": "b/b.model3.json", "available": true, "default": false }
              ]
            }
        """.trimIndent()
        assertEquals("b", ModelRegistry.parse(json).getDefault().id)
    }

    @Test
    fun getDefault_allUnavailableOrPcOnly_fallsBackToHardcodedMaoPro() {
        val json = """
            {
              "version": "1.0",
              "models": [
                { "id": "hiyori", "name": "Hiyori", "model3JsonPath": "hiyori/hiyori.model3.json", "available": false, "default": false },
                { "id": "pc_only", "name": "PC Only", "model3JsonPath": "pc_only/pc_only.model3.json", "available": true, "default": false, "restrictPlatform": "pc" }
              ]
            }
        """.trimIndent()
        val fallback = ModelRegistry.parse(json).getDefault()
        assertEquals("mao_pro", fallback.id)
        assertEquals("mao_pro/runtime/mao_pro.model3.json", fallback.model3JsonPath)
    }

    @Test
    fun getDefault_onFallbackInstance_returnsMaoPro() {
        val registry = ModelRegistry.fallback()
        assertEquals("mao_pro", registry.getDefault().id)
        assertTrue(registry.getDefault().available)
    }

    // ─────────────────────────────────────────────────────────────
    //  getModel3JsonPath
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getModel3JsonPath_hitAndMiss() {
        val registry = ModelRegistry.parse(validJson)
        assertEquals("shizuku/runtime/shizuku.model3.json", registry.getModel3JsonPath("shizuku"))
        assertNull(registry.getModel3JsonPath("nonexistent"))
    }
}
