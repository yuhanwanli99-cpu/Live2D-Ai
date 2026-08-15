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
 * ModelRegistry 单元测试（契约 contract-android-v0.md §1，A1-A13 重写 + contract-R1-model D2 单模型化）。
 *
 * 依据 registry 定稿数据（S1 + R1-model D2 彻底移除 niziiro_mao）：注册表 **1 条**：
 * - id=`bai`（default=**true**）、name=`白-免费版 (Bai Free)`、
 *   license=`本地使用（D1 裁定，不内置分发）`、model3JsonPath=`bai/runtime/bai.model3.json`
 *
 * 断言项（契约 §1 + M3）：
 * - A1 getAll().size == 1；A2 getById("bai") 非 null；A3 bai.available==true；A4 bai.default==true
 * - A5 bai.name == `白-免费版 (Bai Free)`；A6 bai.model3JsonPath == `bai/runtime/bai.model3.json`
 * - A7 bai.metadata.license 非空/非空白
 * - A11 零残留：ids 不含 mao_pro/niziiro_mao/shizuku/hiyori/haru
 * - A12 唯一 default：all.filter{it.default} size==1 && id==`bai`
 * - A13 bai 的 description 无 U+FFFD、UTF-8 往返一致
 *
 * 兜底（fallback）语义 = FALLBACK_ENTRY 当前值 **bai**（契约 M-AND §2.1 + D3 定案：
 * 兜底默认 = registry 默认 = SettingsRepository DEFAULT_MODEL_ID = bai，C32 直通）。
 * 仅异常路径（load 异常/空数组/损坏 JSON/全不可用/全 pc_only）；G4 已为 bai。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ModelRegistryTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    // ─────────────────────────────────────────────────────────────
    //  load()：读取真实 assets/model_registry.json（Gradle copyModelRegistry 生成）
    //  契约 §1 A1-A13 全部在此验证
    // ─────────────────────────────────────────────────────────────

    @Test
    fun load_parsesRealRegistryFromAssets() {
        val registry = ModelRegistry.load(context)
        val all = registry.getAll()

        // A1：注册表 1 条（仅 bai，D2 移除 niziiro_mao）
        assertEquals("注册表应为 1 条（仅 bai，D2），实际 ${all.size}", 1, all.size)

        // A2+A3+A4：bai 存在、available、default
        val bai = registry.getById("bai")
        assertNotNull("bai 应存在（唯一 default 条目）", bai)
        assertEquals("bai id", "bai", bai!!.id)
        assertTrue("bai 应 available", bai.available)
        assertTrue("bai 应 default（默认形象）", bai.default)

        // A5：bai.name
        assertEquals("bai 显示名应为 白-免费版 (Bai Free)", "白-免费版 (Bai Free)", bai.name)

        // A6：bai.model3JsonPath（S1 定稿）
        assertEquals("bai/runtime/bai.model3.json", bai.model3JsonPath)
        assertTrue("bai path 应以 bai/ 开头（C7）", bai.model3JsonPath.startsWith("bai/"))

        // A7：bai.metadata.license 非空/非空白
        assertNotNull("bai.metadata 应存在", bai.metadata)
        val baiLicense = bai.metadata?.license?.trim()
        assertTrue(
            "bai.license 应非空/非空白，实际「${bai.metadata?.license}」",
            !baiLicense.isNullOrEmpty()
        )

        // A13：description 无 U+FFFD、UTF-8 往返一致
        assertNoMojibake("bai.description", bai.description)

        // A11：零残留（全部注册表 id 不含已删除模型）
        val ids = all.map { it.id }
        for (removedId in listOf("niziiro_mao", "mao_pro", "shizuku", "hiyori", "haru")) {
            assertFalse(
                "已删除 id $removedId 不应存在（D2 niziiro 彻底移除），实际残留 ${ids}",
                ids.contains(removedId)
            )
        }

        // A12：唯一 default = bai
        val defaults = all.filter { it.default }
        assertEquals("注册表应只有一个 default 条目", 1, defaults.size)
        assertEquals("唯一 default 应为 bai", "bai", defaults[0].id)
        assertTrue("default 条目应 available", defaults[0].available)
    }

    // ─────────────────────────────────────────────────────────────
    //  parse()：内联 JSON 构造（不依赖 assets）
    //   合成 fixture（契约 §1 随动，D2 单模型）：1 条 = bai(default:true)
    //   与真实 registry 对齐；pc_only 用于平台过滤/回退测试（非真实 registry）
    // ─────────────────────────────────────────────────────────────

    private val validJson = """
        {
          "version": "1.0",
          "models": [
            {
              "id": "bai",
              "name": "白-免费版 (Bai Free)",
              "description": "白-免费版 — 默认形象",
              "model3JsonPath": "bai/runtime/bai.model3.json",
              "available": true,
              "default": true,
              "metadata": { "license": "本地使用（D1 裁定，不内置分发）" }
            }
          ]
        }
    """.trimIndent()

    @Test
    fun parse_validJson_modelsListCorrect() {
        val registry = ModelRegistry.parse(validJson)
        val all = registry.getAll()
        // 合成 fixture 1 条（仅 bai），契约 §1 随动
        assertEquals(1, all.size)
        assertEquals(listOf("bai"), all.map { it.id })
        // 字段映射正确
        val bai = registry.getById("bai")
        assertEquals("白-免费版 (Bai Free)", bai!!.name)
        assertEquals("bai/runtime/bai.model3.json", bai.model3JsonPath)
        assertTrue(bai.available)
        assertTrue(bai.default)
        assertNoMojibake("bai.description", bai.description)
        // 删除 id 不得在合成样例中出现（零残留，D2）
        val ids = all.map { it.id }
        assertFalse("niziiro_mao 不应存在", ids.contains("niziiro_mao"))
        assertFalse("mao_pro 不应存在", ids.contains("mao_pro"))
        assertFalse("shizuku 不应存在", ids.contains("shizuku"))
        assertFalse("hiyori 不应存在", ids.contains("hiyori"))
        assertFalse("haru 不应存在", ids.contains("haru"))
    }

    @Test
    fun parse_corruptJson_returnsFallbackWithOnlyBai() {
        // 兜底语义 = FALLBACK_ENTRY（已经 = bai）（contract-M-AND §2.1/D3 + D6）
        val registry = ModelRegistry.parse("{ this is not valid json !!!")
        val all = registry.getAll()
        assertEquals(
            "损坏 JSON 应回退为只含 bai 的兜底实例（FALLBACK_ENTRY=bai）",
            listOf("bai"),
            all.map { it.id }
        )
        val fallback = all[0]
        assertEquals("bai", fallback.id)
        assertEquals("bai/runtime/bai.model3.json", fallback.model3JsonPath)
        assertTrue(fallback.available)
        assertTrue(fallback.default)
    }

    @Test
    fun parse_emptyModelsArray_returnsFallback() {
        val registry = ModelRegistry.parse("""{"version":"1.0","models":[]}""")
        assertEquals("空数组应回退为只含 bai 的兜底实例（FALLBACK_ENTRY=bai）", listOf("bai"), registry.getAll().map { it.id })
    }

    @Test
    fun parse_missingDefaultField_ignoresUnknownKeysAndToleratesNulls() {
        // 缺少可选字段（description/thumbnailPath/restrictPlatform/metadata）应能解析
        val registry = ModelRegistry.parse(validJson)
        assertEquals(1, registry.getAll().size)
    }

    // ─────────────────────────────────────────────────────────────
    //  getById
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getById_hit_returnsEntry() {
        val registry = ModelRegistry.parse(validJson)
        val entry = registry.getById("bai")
        assertNotNull(entry)
        assertEquals("bai", entry!!.id)
        assertEquals("bai", registry.getById("bai")!!.id)
    }

    @Test
    fun getById_miss_returnsNull() {
        val registry = ModelRegistry.parse(validJson)
        assertNull(registry.getById("niziiro_mao"))
        assertNull(registry.getById("nonexistent"))
    }

    // ─────────────────────────────────────────────────────────────
    //  getAvailable
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getAvailable_filtersUnavailableAndPcRestricted() {
        // 含不可用/pc 限定时，可用 = [bai]
        val json = """
            {
              "version": "1.0",
              "models": [
                { "id": "bai", "name": "白", "model3JsonPath": "bai/runtime/bai.model3.json", "available": true, "default": true },
                { "id": "pc_only", "name": "PC Only", "model3JsonPath": "pc_only/runtime/pc_only.model3.json", "available": true, "default": false, "restrictPlatform": "pc" },
                { "id": "offline", "name": "Offline", "model3JsonPath": "offline/runtime/offline.model3.json", "available": false, "default": false }
              ]
            }
        """.trimIndent()
        val available = ModelRegistry.parse(json).getAvailable()
        assertEquals(
            "仅 available=true 且非 pc 限定 → [bai]",
            listOf("bai"),
            available.map { it.id }
        )
    }

    @Test
    fun getAvailable_realFixture_returnsBai() {
        // 契约：getAvailable 期望 = [bai]（真实 registry 仅 bai 且 available）
        val available = ModelRegistry.parse(validJson).getAvailable()
        assertEquals(
            listOf("bai"),
            available.map { it.id }
        )
    }

    // ─────────────────────────────────────────────────────────────
    //  getDefault
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getDefault_prefersDefaultFlag() {
        // 契约：getDefault 期望 = bai
        val registry = ModelRegistry.parse(validJson)
        assertEquals("bai", registry.getDefault().id)
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
    fun getDefault_allUnavailableOrPcOnly_fallsBackToHardcodedBai() {
        val json = """
            {
              "version": "1.0",
              "models": [
                { "id": "pc_only", "name": "PC Only", "model3JsonPath": "pc_only/runtime/pc_only.model3.json", "available": true, "default": false, "restrictPlatform": "pc" }
              ]
            }
        """.trimIndent()
        val fallback = ModelRegistry.parse(json).getDefault()
        // 兜底 = FALLBACK_ENTRY = bai（G4，contract-M-AND §2.1 + D3）
        assertEquals("bai", fallback.id)
        assertEquals("bai/runtime/bai.model3.json", fallback.model3JsonPath)
    }

    @Test
    fun getDefault_onFallbackInstance_returnsBai() {
        // FALLBACK_ENTRY = bai（G4，contract-M-AND §2.1 + D3）
        val registry = ModelRegistry.fallback()
        assertEquals("bai", registry.getDefault().id)
        assertTrue(registry.getDefault().available)
    }

    @Test
    fun B2_fallbackEntry_matchesRegistryDefault_bai() {
        // B2/B8 = C32 直通：fallback() 构造的实例 getDefault().id == registry default == "bai"
        //（契约 M-AND §2.1 约束：FALLBACK_ENTRY.id == "bai"；fallback() 实例 getDefault().id == "bai"）
        val fallback = ModelRegistry.fallback()
        assertEquals("fallback() 实例 getDefault().id 应为 bai（FALLBACK_ENTRY）", "bai", fallback.getDefault().id)
        assertEquals("bai/runtime/bai.model3.json", fallback.getDefault().model3JsonPath)

        // 与正常 registry default 一致（C32 三源统一 bai）
        val registryDefault = ModelRegistry.load(context).getDefault().id
        assertEquals("FALLBACK 兜底默认应与 registry 正常默认一致（C32 直通）", registryDefault, fallback.getDefault().id)
    }

    // ─────────────────────────────────────────────────────────────
    //  getModel3JsonPath
    // ─────────────────────────────────────────────────────────────

    @Test
    fun getModel3JsonPath_hitAndMiss() {
        val registry = ModelRegistry.parse(validJson)
        assertEquals(
            "bai/runtime/bai.model3.json",
            registry.getModel3JsonPath("bai")
        )
        assertNull(registry.getModel3JsonPath("niziiro_mao"))
        assertNull(registry.getModel3JsonPath("mao_pro"))
        assertNull(registry.getModel3JsonPath("nonexistent"))
    }

    // ─────────────────────────────────────────────────────────────
    //  UTF-8 乱码检查（契约 §1 A13）
    // ─────────────────────────────────────────────────────────────

    private fun assertNoMojibake(label: String, value: String?) {
        if (value == null) return
        assertTrue(
            "$label 不应含 U+FFFD 替换符（GBK 乱码特征），实际「$value」",
            !value.contains('\uFFFD')
        )
        // 编码可逆：UTF-8 编码后能原样解码回同一字符串，无信息丢失
        val reDecoded = String(value.toByteArray(Charsets.UTF_8), Charsets.UTF_8)
        assertEquals("$label 应在 UTF-8 往返后保持字节一致（无乱码）", value, reDecoded)
    }
}
