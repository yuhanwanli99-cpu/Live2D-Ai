package com.live2dai.android.data

import android.content.Context
import android.content.res.AssetManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.mockito.ArgumentMatchers.anyString
import org.mockito.Mockito.`when`
import org.mockito.Mockito.mock
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.IOException
import com.live2dai.android.ModelAdapterConfig
import com.live2dai.android.ModelAdapterConfig.AdapterData
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull

/**
 * ModelAdapterConfig 单元测试（契约 v3-adapter §1.3 / §2 / §5，T1.2 批1；
 *             contract-M-AND §2.2 / §2.3 / §3 B3，G3 按 modelId 加载）。
 *
 * 依据 .pi/spoq/contract-v3-adapter.md + contract-M-AND.md + contract-R1-model（D2：bai 唯一化）：
 * - A1 schema 完整性：adapter JSON 含 §1.2 全部必填字段；键集/值域约束全过。
 * - A2 加载成功：ModelAdapterConfig.load(context, modelId=bai) 返回 AdapterData，字段与 §2 基线逐项一致
 *   （bai：motionAliases={} / vowelParamMap 6 键（各含 ParamMouthOpenY+ParamMouthForm）/
 *    vowelRawMap 6 键列表 / emotionIndex 8 键 / idlePool=[]）。
 * - A3 加载失败回退：load 返回 null 时行为等价（null = 降级链第一级 -> 调用方走现有硬编码常量）。
 * - G3（contract-M-AND §2.2）：load(context, modelId) 按 modelId 读 {modelId}.adapter.json；
 *   bai → bai.adapter.json；未知 modelId（asset 缺失）→ null。
 * - M3（contract-R1-model D2）后唯一模型为 bai：niziiro_mao.adapter.json 资产已移除，
 *   故本文件全部夹具/断言锚定 bai（五元音 ParamA/I/U/E/O 与 U=0.56 修正随 niziiro 移除而失效，已删）。
 *   默认适配常量 ModelAdapterConfig.ASSET_NAME 重锚 bai 属 M3-A4 grep 门（批量红绿收敛，非本单测）。
 *
 * 状态说明：asset 已由 Gradle copyModelAdapter 同步（bai.adapter.json）。
 * A3 用受控 fakeContext 使 assets.open() 抛 IOException → load 返回 null。只用 app/src/test/，不碰实现。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ModelAdapterConfigTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    private val eps = 1e-6f

    // ─────────────────────────────────────────────────────────────
    //  A2 加载成功：读真实 assets/bai.adapter.json（copyModelAdapter 生成）
    //  字段与契约 §3 基线逐项一致（bai 唯一模型，D2）
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A2_load_parsesRealAdapterFromAssets() {
        val data: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull("load(ctx, modelId=bai) 应成功解析 bai adapter（asset 已由 copyModelAdapter 同步）", data)
        val adapter = data!!
        assertNotNull(adapter)

        // modelId / displayName（契约 §1.2 + registry 一致）
        assertEquals("bai", adapter.modelId)
        assertEquals("白-免费版", adapter.displayName)

        // motionAliases：bai 无 motion → 必须为空 {}（D2 / model-facts-bai §六）
        assertTrue("bai.motionAliases 应为空 {}（无 motion 目录，MotionLoader 静默跳过）", adapter.motionAliases.isEmpty())

        // vowelParamMap：6 键 {A,I,U,E,O,SIL}，每个 label 恰含 {ParamMouthOpenY, ParamMouthForm}（无五元音 ParamA/I/U/E/O）
        assertEquals(
            "bai.vowelParamMap 应为 6 键 {A,I,U,E,O,SIL}",
            setOf("A", "I", "U", "E", "O", "SIL"),
            adapter.vowelParamMap.keys
        )
        val expectedMouth = mapOf("A" to 0.8f, "I" to 0.5f, "U" to 0.4f, "E" to 0.6f, "O" to 0.7f, "SIL" to 0.0f)
        val expectedForm = mapOf("A" to 0.4f, "I" to 0.2f, "U" to 0.1f, "E" to 0.3f, "O" to 0.5f, "SIL" to 0.0f)
        for (vowel in setOf("A", "I", "U", "E", "O", "SIL")) {
            val params = adapter.vowelParamMap[vowel]
            assertNotNull("bai.vowelParamMap[$vowel] 应存在", params)
            assertEquals("bai[$vowel] 应恰含 2 键（MouthOpenY+MouthForm）", setOf("ParamMouthOpenY", "ParamMouthForm"), params!!.keys)
            assertEquals("bai[$vowel] ParamMouthOpenY", expectedMouth[vowel]!!, params["ParamMouthOpenY"]!!, eps)
            assertEquals("bai[$vowel] ParamMouthForm", expectedForm[vowel]!!, params["ParamMouthForm"]!!, eps)
            // bai 口型键不得出现五元音 ParamA/I/U/E/O（写进 cdi3 不存在的参数 = 口型静默失效根因，model-facts-bai §五）
            assertTrue("bai 口型不得含 ParamA", !params.containsKey("ParamA"))
            assertTrue("bai 口型不得含 ParamI", !params.containsKey("ParamI"))
            assertTrue("bai 口型不得含 ParamU", !params.containsKey("ParamU"))
            assertTrue("bai 口型不得含 ParamE", !params.containsKey("ParamE"))
            assertTrue("bai 口型不得含 ParamO", !params.containsKey("ParamO"))
            // 值域约束：所有 Param ∈ [0,1]
            for ((param, v) in params) {
                assertTrue("$vowel.$param=$v 应 ∈ [0,1]", v in 0f..1f)
            }
        }

        // vowelRawMap：6 键，值为 [open, form]（模型事实锁定）
        assertEquals(6, adapter.vowelRawMap.size)
        assertEquals(setOf("A", "I", "U", "E", "O", "SIL"), adapter.vowelRawMap.keys)
        assertEquals(0.8f, adapter.vowelRawMap["A"]!!.first, eps)
        assertEquals(0.4f, adapter.vowelRawMap["A"]!!.second, eps)
        assertEquals(0.5f, adapter.vowelRawMap["I"]!!.first, eps)
        assertEquals(0.2f, adapter.vowelRawMap["I"]!!.second, eps)
        assertEquals(0.4f, adapter.vowelRawMap["U"]!!.first, eps)
        assertEquals(0.1f, adapter.vowelRawMap["U"]!!.second, eps)
        assertEquals(0.0f, adapter.vowelRawMap["SIL"]!!.first, eps)
        assertEquals(0.0f, adapter.vowelRawMap["SIL"]!!.second, eps)

        // idlePool：bai 无 motion → 必须为空 []
        assertTrue("bai.idlePool 应为空 []（无 idle 动画池）", adapter.idlePool.isEmpty())

        // emotionIndex：8 键，值 0~7 唯一（契约 §2.3）
        assertEquals(8, adapter.emotionIndex.size)
        assertEquals(
            "emotionIndex 应为 8 键（契约 §2.3）",
            setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"),
            adapter.emotionIndex.keys
        )
        assertEquals(0, adapter.emotionIndex["neutral"])
        assertEquals(1, adapter.emotionIndex["fear"])
        assertEquals(2, adapter.emotionIndex["sadness"])
        assertEquals(3, adapter.emotionIndex["anger"])
        assertEquals(4, adapter.emotionIndex["disgust"])
        assertEquals(5, adapter.emotionIndex["joy"])
        assertEquals(6, adapter.emotionIndex["smirk"])
        assertEquals(7, adapter.emotionIndex["surprise"])
        // 值 0..7 唯一
        val values = adapter.emotionIndex.values
        assertEquals("emotionIndex 值 0~7 应唯一", setOf(0, 1, 2, 3, 4, 5, 6, 7), values.toSet())
    }

    // ─────────────────────────────────────────────────────────────
    //  A3 加载失败回退：load 返回 nullable；缺失 asset → null（降级链第一级）
    //  调用方（MotionTags/VisemeAnalyzer）收到 null 走现有硬编码常量（行为等价，另测）
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A3_loadNullable_contract() {
        // 方案 A（裁定）：不依赖真实合并 asset。改用受控 fakeContext：mock Context.assets.open()
        // 抛 IOException，锁定缺失/损坏路径。契约 §1.3：load 解析/读取失败 → null（回退门）。
        val fakeContext = mock(Context::class.java)
        val fakeAssets = mock(AssetManager::class.java)
        `when`(fakeContext.assets).thenReturn(fakeAssets)
        `when`(fakeAssets.open(anyString())).thenThrow(IOException("asset 缺失/损坏（受控，方案 A）"))

        val data: AdapterData? = ModelAdapterConfig.load(fakeContext, "bai")
        assertNull(
            "asset 打开抛 IOException → load 应返回 null（回退门，不崩溃）",
            data
        )
    }

    // ─────────────────────────────────────────────────────────────
    //  G3（contract-M-AND §2.2 / §3 B3）：load(context, modelId) 按 modelId 加载
    //  bai → bai.adapter.json（口型 ParamMouthOpenY/ParamMouthForm，无 ParamA/I/U/E/O，motionAliases={}，idlePool=[]）
    //  未知 modelId（asset 缺失）→ null
    // ─────────────────────────────────────────────────────────────

    @Test
    fun G3_load_bai_readsBaiAdapter_perModelDispatch() {
        // G3：切 bai → 读 bai.adapter.json（唯一模型，D2）
        val data: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull("load(ctx, modelId=bai) 应读取 bai.adapter.json", data)
        assertEquals("bai", data!!.modelId)
        assertEquals("白-免费版", data.displayName)

        // vowelParamMap：口型键为 ParamMouthOpenY/ParamMouthForm（无 ParamA/I/U/E/O —— bai.cdi3 无这 5 参数）
        assertEquals(
            "bai.vowelParamMap 应为 {A,I,U,E,O,SIL} 六元音键",
            setOf("A", "I", "U", "E", "O", "SIL"),
            data.vowelParamMap.keys
        )
        for (vowel in listOf("A", "I", "U", "E", "O", "SIL")) {
            val params = data.vowelParamMap[vowel]
            assertNotNull("bai.vowelParamMap[$vowel] 应存在", params)
            assertTrue("bai[$vowel] 口型键不得含五元音 ParamA/I/U/E/O", !params!!.keys.any { it.startsWith("Param") && it != "ParamMouthOpenY" && it != "ParamMouthForm" })
        }

        // 无 motion/exp3：motionAliases={}、idlePool=[]（§2.2）
        assertTrue("bai.motionAliases 应为空 {}（无 motion 目录，MotionLoader 静默跳过）", data.motionAliases.isEmpty())
        assertTrue("bai.idlePool 应为空 []（无 idle 动画池）", data.idlePool.isEmpty())

        // emotionIndex 8 键（口型/表情索引两源均 8 键，§2.2 + G5）
        assertEquals(8, data.emotionIndex.size)
        assertEquals(
            setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"),
            data.emotionIndex.keys
        )
    }

    @Test
    fun G3_load_unknownModelId_returnsNull_assetMissingPath() {
        // §2.2/§2.3：modelId 无对应 {modelId}.adapter.json 资产 → load 返回 null（asset 缺失路径，降级链第一级）
        val data: AdapterData? = ModelAdapterConfig.load(context, "model_that_does_not_exist")
        assertNull("未知 modelId（无对应 asset）→ load 应返回 null（asset 缺失路径）", data)
    }

    @Test
    fun G3_load_cache_honorsBaiSingleModel() {
        // M3（D2）：唯一模型 bai。load(bai) 后 currentAdapter() 保持 bai（单模型，无切换残留）。
        val bai1: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull(bai1)
        assertEquals("load(bai) 后 currentAdapter() 应指向 bai.adapter", "bai", ModelAdapterConfig.currentAdapter()?.modelId)

        // 再 load(bai)，缓存语义稳定（幂等；唯一模型不变）
        val bai2: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull(bai2)
        assertEquals("二次 load(bai) 后 currentAdapter() 仍应指向 bai.adapter", "bai", ModelAdapterConfig.currentAdapter()?.modelId)
    }

    // ─────────────────────────────────────────────────────────────
    //  AB-04 黑盒断言：bai.adapter.json 解析（契约 contract-android-v0 §2）
    //  bai 模型无 motion/exp3 → motionAliases={}、idlePool=[]（解析器 isSchemaValid 已由
    //  coder-A1 对"无 motion 模型"豁免 motionAliases/idlePool 非空校验，emotionIndex 8 键仍强制）。
    //  黑盒：构造 bai 内容（含 emotionIndex 8 键）→ ModelAdapterConfig.parse(...) 非 null 且字段正确。
    //  注意 [UNSURE]：A1 契约未锁 load-to-bai 的入口 API；本测试按任务指令走 parse(String) 路径；
    //  若 coder 未暴露公共 parse 则可编译失败，需测试经理/主代理定夺。
    // ─────────────────────────────────────────────────────────────

    @Test
    fun AB04_parse_baiAdapter_returnsNonNullWithEmotionIndex() {
        // 黑盒：读真实 assets/bai.adapter.json（coder-A1 已 copyModelAdapter 拷入 + AB-01 补 emotionIndex）
        // → ModelAdapterConfig.parse(内容) 必须返回非 null。bai 为无 motion 模型（motionAliases={}/idlePool=[]）。
        val baiJson = readAsset("bai.adapter.json")
        val data: AdapterData? = ModelAdapterConfig.parse(baiJson)
        // 黑盒硬断言：bai 内容解析必须返回非 null（不因 motionAliases={}/idlePool=[] 校验失败）
        assertNotNull("bai 无 motion 模型应被豁免 motionAliases/idlePool 非空校验，parse 不得返回 null", data)
        // 字段正确：modelId / displayName
        assertEquals("bai", data!!.modelId)
        assertTrue("displayName 应非空", !data.displayName.isNullOrEmpty())
        // motionAliases / idlePool 保留空（无 motion 模型事实）
        assertTrue("bai.motionAliases 应为空 map", data.motionAliases.isEmpty())
        assertTrue("bai.idlePool 应为空 list", data.idlePool.isEmpty())
        // emotionIndex 8 键（AB-01，仍强制非空校验）
        assertEquals(8, data.emotionIndex.size)
        assertEquals(
            setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"),
            data.emotionIndex.keys
        )
        assertEquals(0, data.emotionIndex["neutral"])
        assertEquals(5, data.emotionIndex["joy"])
        assertEquals(7, data.emotionIndex["surprise"])
        // 口型段保留（AB-03）：vowelParamMap A 段驱动 ParamMouthOpenY/ParamMouthForm
        assertEquals(0.8f, data.vowelParamMap["A"]!!["ParamMouthOpenY"]!!, eps)
    }

    @Test
    fun AB04_parse_baiAdapter_missingEmotionIndex_returnsNull() {
        // AB-01/AB-04 边界：bai 解析时 emotionIndex 8 键仍强制非空（isSchemaValid 豁免仅限
        // motionAliases/idlePool）。构造缺 emotionIndex 的最小无-motion 内容（其余豁免字段齐）
        // → 解析失败返回 null（不去硬编码兜底非 null 假象。）
        val baiMissingEmotion = """
            {
              "modelId": "bai",
              "displayName": "白-免费版",
              "motionAliases": {},
              "vowelParamMap": {
                "A":   { "ParamMouthOpenY": 0.8, "ParamMouthForm": 0.4 },
                "I":   { "ParamMouthOpenY": 0.5, "ParamMouthForm": 0.2 },
                "U":   { "ParamMouthOpenY": 0.4, "ParamMouthForm": 0.1 },
                "E":   { "ParamMouthOpenY": 0.6, "ParamMouthForm": 0.3 },
                "O":   { "ParamMouthOpenY": 0.7, "ParamMouthForm": 0.5 },
                "SIL": { "ParamMouthOpenY": 0.0, "ParamMouthForm": 0.0 }
              },
              "vowelRawMap": {
                "A":   [0.8, 0.4],
                "I":   [0.5, 0.2],
                "U":   [0.4, 0.1],
                "E":   [0.6, 0.3],
                "O":   [0.7, 0.5],
                "SIL": [0.0, 0.0]
              },
              "idlePool": [],
              "emotionParamMap": {
                "neutral": {
                  "_1": { "ParamBrowLY": 0.0, "ParamBrowRY": 0.0, "ParamEyeLOpen": 0.75, "ParamEyeROpen": 0.75, "ParamMouthForm": 0.5, "ParamMouthOpenY": 0.0 }
                }
              },
              "fadeMs": { "emotionSwitch": 300, "emotionToNeutral": 500, "microExpression": 200, "lipSync": 50 },
              "degradation": { "noVowelParamsFallback": "RMS", "unknownAlias": "ignore", "emptyExpressionMix": "no-op" }
            }
        """.trimIndent()
        assertNull(
            "bai 缺 emotionIndex（8 键强制非空）应解析失败返回 null（契约 §0 路径①）",
            ModelAdapterConfig.parse(baiMissingEmotion)
        )
    }

    // ─────────────────────────────────────────────────────────────
    //  A3U-01（contract-bai-params-v0.md §9）：emotionParamMap 结构与 D6「出现即完整」校验
    //  的零回归断言（依契约定案：字段存在则强制 8 键 × _1/_2/_3 完整，否则解析失败）
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A3U01_bai_emotionParamMap_structureAndValueDomain() {
        // 读真实 bai.adapter.json：emotionParamMap 8 emotion × 3 强度 × 6 键 = 144 值
        val baiJson = readAsset("bai.adapter.json")
        val data: AdapterData? = ModelAdapterConfig.parse(baiJson)
        assertNotNull("bai 应解析成功且渲染出 emotionParamMap", data)
        val map = data!!.emotionParamMap

        // 8 键（契约 §1 含 neutral/fear/sadness/anger/disgust/joy/smirk/surprise）
        val emotions = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
        assertEquals("emotionParamMap 应为 8 键", emotions, map.keys)

        val framesKey = setOf("_1", "_2", "_3")
        val keys6 = setOf("ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen", "ParamMouthForm", "ParamMouthOpenY")
        var total = 0
        for (emotion in emotions) {
            val frames = map[emotion]
            assertNotNull("emotion=$emotion 应存在强度档", frames)
            assertEquals("emotion=$emotion 应含 _1/_2/_3 三档", framesKey, frames!!.keys)
            for (band in framesKey) {
                val frame = frames[band]
                assertNotNull("emotion=$emotion $band 帧应非空（D6 出现即完整）", frame)
                assertEquals("emotion=$emotion $band 应为 6 键", keys6, frame!!.keys)
                for ((k, v) in frame) {
                    assertTrue("$emotion.$band.$k=$v 应 ∈ [-1.0, 2.0]（协议 H 段）", v in -1.0f..2.0f)
                    total++
                }
            }
        }
        assertEquals("值总数应为 8×3×6 = 144（A3U-01）", 144, total)
        // 值域端点抽验（实测）：surprise._3 EyeLOpen=1.5 / smirk._3 MouthForm=-0.7
        assertEquals(1.5f, map["surprise"]!!["_3"]!!["ParamEyeLOpen"]!!, eps)
        assertEquals(-0.7f, map["smirk"]!!["_3"]!!["ParamMouthForm"]!!, eps)
    }

    // ─────────────────────────────────────────────────────────────
    //  AB-04 helper：读 assets 下真实 adapter json 文本
    // ─────────────────────────────────────────────────────────────

    private fun readAsset(name: String): String {
        val ins = context.assets.open(name)
        return ins.bufferedReader(Charsets.UTF_8).use { it.readText() }
    }
}
