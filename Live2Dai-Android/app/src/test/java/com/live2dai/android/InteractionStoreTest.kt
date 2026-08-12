package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2dai.android.data.InteractionStore
import com.live2dai.android.data.Override
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config


/**
 * Android 用户交互持久化测试（契约 C-M5 §2.2/§3，M5-A5，Robolectric 可测）.
 *
 * 覆盖生产文件 `data/InteractionStore.kt`（**新增，RED 待实现**）：
 *   class InteractionStore(context: Context) {
 *       fun load(modelId: String): Override?    // key = "override_{modelId}"；无 key / 损坏 → null
 *       fun save(modelId: String, override: Override)   // 覆盖写
 *       fun clear(modelId: String)              // remove key
 *   }
 *   // SharedPreferences: name = "live2dai_interaction", MODE_PRIVATE
 *
 * 依据取自契约 C-M5 §2.2/§3 验收断言 M5-A5「无 key → null；save→load roundtrip；clear → null；
 * 每模型独立；prefs name/key 与 §2.2 一致」。
 * 禁止读实现源码（app/src/main 下 Live2DView/Live2DRenderer 等禁读、.pi/spoq/src/ 禁读）。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class InteractionStoreTest {

    // 契约 §2.2 锁定的 prefs name 与 key 前缀
    private fun prefsName(): String = "live2dai_interaction"
    private fun keyOf(modelId: String): String = "override_$modelId"

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun setUp() {
        // 每个用例从干净状态开始（清空交互 prefs）
        context.getSharedPreferences(prefsName(), Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    // M5-A5：无 key → null（冷启动未调整过）
    @Test
    fun load_noKey_returnsNull() {
        val store = InteractionStore(context)
        assertNull("从未保存 → load 应为 null", store.load("bai"))
    }

    // M5-A5：save → load roundtrip（含负 offset / 超钳制范围值也应原样往返，持久化不做钳制）
    @Test
    fun save_thenLoad_roundTrip_conservesAllFields() {
        val store = InteractionStore(context)
        val user = Override(2.5f, -0.4f, 0.3f)

        store.save("bai", user)

        val loaded = store.load("bai")
        assertEquals("load 后 scale 应一致", user.scale, loaded!!.scale, 1e-6f)
        assertEquals("load 后 xOffset 应一致", user.xOffset, loaded.xOffset, 1e-6f)
        assertEquals("load 后 yOffset 应一致", user.yOffset, loaded.yOffset, 1e-6f)
    }

    // M5-A5：save 覆盖写（后写覆盖先写，负载更新而非追加）
    @Test
    fun save_overwrite_updatesExistingKey() {
        val store = InteractionStore(context)
        store.save("bai", Override(1.0f, 0f, 0f))
        store.save("bai", Override(3.0f, 0.2f, -0.1f))   // 覆盖写，不产生第二份

        val loaded = store.load("bai")
        assertEquals("覆盖后 scale 应为新值", 3.0f, loaded!!.scale, 1e-6f)
        assertEquals("覆盖后 xOffset 应为新值", 0.2f, loaded.xOffset, 1e-6f)
        assertEquals("覆盖后 yOffset 应为新值", -0.1f, loaded.yOffset, 1e-6f)
        // prefs 中该 key 只有一份（无重复元素语义）
        val raw = context.getSharedPreferences(prefsName(), Context.MODE_PRIVATE)
        assertTrue("bai key 应含一条记录", raw.contains(keyOf("bai")))
    }

    // M5-A5：clear → load null（重置入口清偏好回 fit）
    @Test
    fun clear_removesKey_andLoadReturnsNull() {
        val store = InteractionStore(context)
        store.save("bai", Override(2.0f, 0.1f, 0.2f))
        assertEquals("save 后应先非空", 2.0f, store.load("bai")!!.scale, 1e-6f)

        store.clear("bai")

        assertNull("clear 后 load → null", store.load("bai"))
        val raw = context.getSharedPreferences(prefsName(), Context.MODE_PRIVATE)
        assertTrue("clear 后 prefs 应无该 key", !raw.contains(keyOf("bai")))
    }

    // M5-A5：每模型独立（modelId A 保存不影响 modelId B）
    @Test
    fun perModelIndependence_modelTypeA_doesNotAffectTypeB() {
        val store = InteractionStore(context)
        store.save("bai", Override(2.0f, 0.1f, 0.2f))

        // 单独检查另一模型（此前未保存过）→ null；而 bai 自己非空
        assertNull("未保存的 niziiro_mao 应 → null", store.load("niziiro_mao"))
        assertEquals("bai 自己应保持 2.0", 2.0f, store.load("bai")!!.scale, 1e-6f)
    }

    @Test
    fun perModelIndependence_twoModels_saveIndependently() {
        val store = InteractionStore(context)
        store.save("bai", Override(2.0f, 0.0f, 0.4f))
        store.save("niziiro_mao", Override(4.29f, -0.0102f, -0.0933f))

        val bai = store.load("bai")!!
        val niz = store.load("niziiro_mao")!!

        assertEquals("bai scale 独立", 2.0f, bai.scale, 1e-6f)
        assertEquals("niz scale 独立", 4.29f, niz.scale, 1e-6f)
        assertEquals("bai xOffset 独立", 0.0f, bai.xOffset, 1e-6f)
        assertEquals("niz xOffset 独立", -0.0102f, niz.xOffset, 1e-6f)
        assertEquals("bai yOffset 独立", 0.4f, bai.yOffset, 1e-6f)
        assertEquals("niz yOffset 独立", -0.0933f, niz.yOffset, 1e-6f)

        // clear 一个不影响另一个
        store.clear("bai")
        assertNull("clear bai → bai null", store.load("bai"))
        assertEquals("clear bai 不影响 niziiro_mao", 4.29f, store.load("niziiro_mao")!!.scale, 1e-6f)
    }

    // M5-A5：prefs name 与 key 契约一致（§2.2：name="live2dai_interaction"，key="override_{modelId}"）
    @Test
    fun persistence_usesContractPrefsNameAndKeyPrefix() {
        val store = InteractionStore(context)
        store.save("bai", Override(1.5f, -0.2f, 0.3f))

        val raw = context.getSharedPreferences(prefsName(), Context.MODE_PRIVATE)
        // 契约锁定的 key 前缀 override_ 落在 name="live2dai_interaction" 的 prefs 上
        assertTrue("prefs 应存储 key 'override_bai'", raw.contains("override_bai"))

        // 默认 prefs（其他 name）不应含该 key → 证明写在契约 prefs
        val defaultPrefs = context.getSharedPreferences("default", Context.MODE_PRIVATE)
        assertTrue("契约数据不得落在 default prefs", !defaultPrefs.contains("override_bai"))
    }

    // M5-A5：损坏存储 → load null（不崩溃）；恢复新 save 后正常
    @Test
    fun load_corruptedPrefValue_returnsNull_noCrash() {
        val store = InteractionStore(context)
        // 手工写入损坏的字符串（模拟底层被篡改/半截写入）
        store.save("bai", Override(1.0f, 0f, 0f))   // 先正常保存确保 key 存在
        context.getSharedPreferences(prefsName(), Context.MODE_PRIVATE)
            .edit().putString("override_bai", "{ 这不是合法内容").commit()

        assertNull("损坏 prefs 值 → load null 不崩溃", store.load("bai"))

        // 损坏被忽略后仍可正常覆盖保存并读回
        store.save("bai", Override(2.2f, 0f, 0f))
        assertEquals("损坏后可正常再 save→load", 2.2f, store.load("bai")!!.scale, 1e-6f)
    }

    // M5-A5：save 非法/损坏 override 内容（非有限）经由 store 写入但 load → null（契约 encode 防御一致）
    @Test
    fun save_roundTrip_rejectsNonFiniteInlineContent() {
        val store = InteractionStore(context)
        store.save("bai", Override(Float.NaN, 0f, 0f))   // NaN scale 交 encode 编码
        // 契约 §2.1 decodeOverride 对非有限值 → null；store.load 应继承该防御（不崩溃）
        assertNull("非有限 override 经 store → load null（继承 encode/decode 防御）", store.load("bai"))
    }
}
