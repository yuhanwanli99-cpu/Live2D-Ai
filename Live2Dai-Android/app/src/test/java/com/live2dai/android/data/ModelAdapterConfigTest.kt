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
 * 依据 .pi/spoq/contract-v3-adapter.md + contract-M-AND.md：
 * - A1 schema 完整性：adapter JSON 含 §1.2 全部必填字段；键集/值域约束全过。
 * - A2 加载成功：ModelAdapterConfig.load(context, modelId) 返回 AdapterData，字段与 §2 基线逐项一致
 *   （nimotionAliases 7 键 / vowelParamMap 6 键 / vowelRawMap 6 键 / emotionIndex 8 键 /
 *    idlePool 6 键 + U=(0.4,0.8) + ParamU=0.56）。
 * - A3 加载失败回退：load 返回 null 时行为等价（null = 降级链第一级 -> 调用方走现有硬编码常量）。
 * - A7 U 修正：vowelParamMap["U"]["ParamU"] == 0.56f（修正值 0.8×0.7，D4 定案）。
 * - G3（contract-M-AND §2.2）：load(context, modelId) 按 modelId 读 {modelId}.adapter.json，
 *   缓存随模型切换刷新；bai → bai.adapter.json、niziiro_mao → niziiro_mao.adapter.json；
 *   未知 modelId（asset 缺失）→ null。
 *
 * 状态说明：asset 已由 Gradle copyModelAdapter 同步（bai.adapter.json + niziiro_mao.adapter.json）。
 * A3 用受控 fakeContext 使 assets.open() 抛 IOException → load 返回 null。只用 app/src/test/，不碰实现。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ModelAdapterConfigTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    private val eps = 1e-6f

    // ─────────────────────────────────────────────────────────────
    //  A2 加载成功：读真实 assets/niziiro_mao.adapter.json（copyModelAdapter 生成）
    //  字段与契约 §2 基线逐项一致
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A2_load_parsesRealAdapterFromAssets() {
        val data: AdapterData? = ModelAdapterConfig.load(context, "niziiro_mao")
        assertNotNull("load(ctx, modelId) 应成功解析 niziiro_mao adapter（asset 已由 copyModelAdapter 同步）", data)
        val adapter = data!!
        assertNotNull(adapter)

        // modelId / displayName（契约 §1.2 + registry 一致）
        assertEquals("niziiro_mao", adapter.modelId)
        assertEquals("Niziiro Mao", adapter.displayName)

        // motionAliases：7 键（锁定键集，不许增减）
        assertEquals(
            "motionAliases 应为契约 §1.2 的 7 键",
            setOf("idle", "mtn_02", "mtn_03", "mtn_04", "wave", "special_02", "special_03"),
            adapter.motionAliases.keys
        )
        // idle：group=Idle, index=0（Android 键 Idle_0）
        assertEquals("Idle", adapter.motionAliases["idle"]!!.group)
        assertEquals(0, adapter.motionAliases["idle"]!!.index)
        assertEquals("motions/mtn_01.motion3.json", adapter.motionAliases["idle"]!!.file)
        // wave：空组，index=3（Android 键 _3）
        assertEquals("", adapter.motionAliases["wave"]!!.group)
        assertEquals(3, adapter.motionAliases["wave"]!!.index)
        assertEquals("motions/special_01.motion3.json", adapter.motionAliases["wave"]!!.file)
        // 全表 (group,index) 与 Android TAG_TO_KEY 派生键逐项一致（契约 §2.1）
        // derived key = "${group}_$index"（空组为 "_N"）
        val derivedKeys = adapter.motionAliases.mapValues { (_, a) ->
            if (a.group.isEmpty()) "_${a.index}" else "${a.group}_${a.index}"
        }
        assertEquals(
            "派生键集应与 Android MotionAliasTable 一致：Idle_0,_0.._5",
            setOf("Idle_0", "_0", "_1", "_2", "_3", "_4", "_5"),
            derivedKeys.values.toSet()
        )

        // vowelParamMap：6 键 {A,I,U,E,O,SIL}，值 ∈ [0,1]
        assertEquals(
            "vowelParamMap 应为 6 键 {A,I,U,E,O,SIL}",
            setOf("A", "I", "U", "E", "O", "SIL"),
            adapter.vowelParamMap.keys
        )
        // A2 各元音精确值（契约 §2.2 表 2）
        assertParamMap(adapter, "A", 0.8f, 0f, 0f, 0f, 0.04f)
        assertParamMap(adapter, "I", 0f, 0.8f, 0f, 0f, 0f)
        assertParamMap(adapter, "U", 0f, 0f, 0.56f, 0f, 0f)   // A7 修正值
        assertParamMap(adapter, "E", 0.15f, 0f, 0f, 0.35f, 0f)
        assertParamMap(adapter, "O", 0f, 0f, 0.21f, 0f, 0.6f)
        assertParamMap(adapter, "SIL", 0f, 0f, 0f, 0f, 0f)
        // 值域约束：所有 Param ∈ [0,1]
        for ((label, params) in adapter.vowelParamMap) {
            for ((param, v) in params) {
                assertTrue("$label.$param=$v 应 ∈ [0,1]", v in 0f..1f)
            }
        }

        // vowelRawMap：6 键；U=(0.4,0.8) 修正
        assertEquals(6, adapter.vowelRawMap.size)
        assertEquals(setOf("A", "I", "U", "E", "O", "SIL"), adapter.vowelRawMap.keys)
        assertEquals(0.4f, adapter.vowelRawMap["U"]!!.first, eps)
        assertEquals(0.8f, adapter.vowelRawMap["U"]!!.second, eps)
        assertEquals(0.8f, adapter.vowelRawMap["A"]!!.first, eps)
        assertEquals(0.2f, adapter.vowelRawMap["A"]!!.second, eps)
        assertEquals(0.0f, adapter.vowelRawMap["SIL"]!!.first, eps)
        assertEquals(0.0f, adapter.vowelRawMap["SIL"]!!.second, eps)

        // idlePool：6 键 [_0.._5]（契约 §2.1 派生键一致）
        assertEquals(
            "idlePool 应为 [_0.._5] 6 键",
            listOf("_0", "_1", "_2", "_3", "_4", "_5"),
            adapter.idlePool
        )

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
    //  A7 U 修正：vowelParamMap["U"]["ParamU"] == 0.56f（D4：mf×0.7 = 0.8×0.7）
    //  buildVisemeFrame("U") 输出 ParamU==0.56f —— 等价断言：adapter 数据层直接锁死
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A7_uParam_isPoint56() {
        val data: AdapterData? = ModelAdapterConfig.load(context, "niziiro_mao")
        assertNotNull("load(ctx, modelId) 应成功", data)
        val u = data!!.vowelParamMap["U"]
        assertNotNull("vowelParamMap 应含 U", u)
        assertEquals(
            "buildVisemeFrame(\"U\") 的 ParamU 应为 0.56f（U 修正值 0.8×0.7，契约 §2.2/D4）",
            0.56f, u!!["ParamU"]!!, eps
        )
        // raw 也应同步修正为 (0.4, 0.8)
        assertEquals(0.4f, data.vowelRawMap["U"]!!.first, eps)
        assertEquals(0.8f, data.vowelRawMap["U"]!!.second, eps)
    }

    // ─────────────────────────────────────────────────────────────
    //  A3 加载失败回退：load 返回 nullable；缺失 asset → null（降级链第一级）
    //  调用方（MotionTags/VisemeAnalyzer）收到 null 走现有硬编码常量（行为等价，另测）
    // ─────────────────────────────────────────────────────────────

    @Test
    fun A3_loadNullable_contract() {
        // 方案 A（裁定）：不依赖真实合并 asset（那会让 A3 与 A2/A7 同产物反向断言冲突）。
        // 改用受控 fakeContext：mock Context.assets.open() 抛 IOException，锁定缺失/损坏路径。
        // 契约 §1.3：load 解析/读取失败 → null（回退门，降级链第一级，调用方走硬编码兜底）。
        val fakeContext = mock(Context::class.java)
        val fakeAssets = mock(AssetManager::class.java)
        `when`(fakeContext.assets).thenReturn(fakeAssets)
        `when`(fakeAssets.open(anyString())).thenThrow(IOException("asset 缺失/损坏（受控，方案 A）"))

        val data: AdapterData? = ModelAdapterConfig.load(fakeContext, "niziiro_mao")
        assertNull(
            "asset 打开抛 IOException → load 应返回 null（回退门，不崩溃）",
            data
        )
    }

    // ─────────────────────────────────────────────────────────────
    //  G3（contract-M-AND §2.2 / §3 B3）：load(context, modelId) 按 modelId 加载
    //  bai → bai.adapter.json（口型 ParamMouthOpenY/ParamMouthForm，无 ParamA/I/U/E/O，motionAliases={}，idlePool=[]）
    //  niziiro_mao → niziiro_mao.adapter.json（五元音 + U=(0.4,0.8)/ParamU=0.56，motionAliases 7 键，idlePool 6 键）
    //  未知 modelId（asset 缺失）→ null；缓存随模型切换刷新
    // ─────────────────────────────────────────────────────────────

    @Test
    fun G3_load_bai_readsBaiAdapter_perModelDispatch() {
        // G3 核心：切 bai → 读 bai.adapter.json（此前硬编码 niziiro_mao.adapter.json 导致口型静默失效）
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
            // 各元音开口度不同（语音学正确：A 大开 / I 扁小 / SIL 闭口）——按真实 adapter 逐值断言
            val expectedMouth = mapOf(
                "A" to 0.8f, "I" to 0.5f, "U" to 0.4f,
                "E" to 0.6f, "O" to 0.7f, "SIL" to 0.0f
            )
            val expectedForm = mapOf(
                "A" to 0.4f, "I" to 0.2f, "U" to 0.1f,
                "E" to 0.3f, "O" to 0.5f, "SIL" to 0.0f
            )
            assertEquals(
                "bai[$vowel] ParamMouthOpenY 应与 bai.adapter.json 一致",
                expectedMouth[vowel]!!, params!!["ParamMouthOpenY"]!!, eps
            )
            assertEquals(
                "bai[$vowel] ParamMouthForm 应与 bai.adapter.json 一致",
                expectedForm[vowel]!!, params["ParamMouthForm"]!!, eps
            )
            // 硬约束（§2.2）：bai 口型键不得出现 ParamA/I/U/E/O（写进 cdi3 不存在的参数 = 口型静默失效根因）
            assertTrue("bai 口型不得含 ParamA", !params.containsKey("ParamA"))
            assertTrue("bai 口型不得含 ParamI", !params.containsKey("ParamI"))
            assertTrue("bai 口型不得含 ParamU", !params.containsKey("ParamU"))
            assertTrue("bai 口型不得含 ParamE", !params.containsKey("ParamE"))
            assertTrue("bai 口型不得含 ParamO", !params.containsKey("ParamO"))
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
    fun G3_load_niziiroMao_readsNiziiroMaoAdapter_perModelDispatch() {
        // G3：切回 niziiro_mao → 读 niziiro_mao.adapter.json（五元音口型回归）
        val data: AdapterData? = ModelAdapterConfig.load(context, "niziiro_mao")
        assertNotNull("load(ctx, modelId=niziiro_mao) 应读取 niziiro_mao.adapter.json", data)
        assertEquals("niziiro_mao", data!!.modelId)

        // 五元音口型 ParamA/I/U/E/O 回归 + U 修正 (0.4,0.8)/ParamU=0.56（§2.2 + A7 修正值）
        val uParams = data.vowelParamMap["U"]
        assertNotNull("niziiro_mao.U 应存在", uParams)
        assertEquals(0.56f, uParams!!["ParamU"]!!, eps)
        assertEquals(0.4f, data.vowelRawMap["U"]!!.first, eps)
        assertEquals(0.8f, data.vowelRawMap["U"]!!.second, eps)
        val aParams = data.vowelParamMap["A"]
        assertEquals(0.8f, aParams!!["ParamA"]!!, eps)

        // motionAliases 7 键 + idlePool 6 键（§2.2）
        assertEquals("niziiro_mao.motionAliases 应为 7 键", 7, data.motionAliases.size)
        assertEquals("niziiro_mao.idlePool 应为 6 键 [_0.._5]", listOf("_0", "_1", "_2", "_3", "_4", "_5"), data.idlePool)
    }

    @Test
    fun G3_load_unknownModelId_returnsNull_assetMissingPath() {
        // §2.2/§2.3：modelId 无对应 {modelId}.adapter.json 资产 → load 返回 null（asset 缺失路径，降级链第一级）
        val data: AdapterData? = ModelAdapterConfig.load(context, "model_that_does_not_exist")
        assertNull("未知 modelId（无对应 asset）→ load 应返回 null（asset 缺失路径）", data)
    }

    @Test
    fun G3_load_cacheRefreshesOnModelSwitch() {
        // §2.2：缓存随模型切换刷新 —— load(bai) 后 currentAdapter().modelId==bai；再 load(niziiro_mao) 后 ==niziiro_mao
        //（修旧硬编码：单一缓存仅在首启固定 niziiro_mao，不随模型切换）
        val bai: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull(bai)
        assertEquals("切 bai 后 currentAdapter() 应指向 bai.adapter", "bai", ModelAdapterConfig.currentAdapter()?.modelId)

        val mao: AdapterData? = ModelAdapterConfig.load(context, "niziiro_mao")
        assertNotNull(mao)
        assertEquals("切 niziiro_mao 后 currentAdapter() 应指向 niziiro_mao.adapter（缓存随模型切换刷新）", "niziiro_mao", ModelAdapterConfig.currentAdapter()?.modelId)

        // 再切回 bai，缓存再次刷新
        val bai2: AdapterData? = ModelAdapterConfig.load(context, "bai")
        assertNotNull(bai2)
        assertEquals("二次切 bai 后 currentAdapter() 应回到 bai.adapter", "bai", ModelAdapterConfig.currentAdapter()?.modelId)
    }

    // ─────────────────────────────────────────────────────────────
    //  AB-04 黑盒断言：bai.adapter.json 解析（契约 contract-android-v0 §2）
    //  bai 模型无 motion/exp3 → motionAliases={}、idlePool=[]（解析器 isSchemaValid 已由
    //  coder-A1 对"无 motion 模型"豁免 motionAliases/idlePool 非空校验，emotionIndex 8 键仍强制）。
    //  黑盒：构造 bai 内容（含 emotionIndex 8 键）→ ModelAdapterConfig.parse(...) 非 null 且字段正确。
    //  注意 [UNSURE]：A1 契约未锁 load-to-bai 的入口 API（load(context) 硬编码 niziiro_mao；
    //  多 adapter 加载 load(context, modelId) 属 A3 决策 D7-B，已确认待重排）。故本测试按任务指令
    //  走 parse(String) 路径；若 coder 未暴露公共 parse 则可编译失败，需测试经理/主代理定夺。
    // ─────────────────────────────────────────────────────────────

    @Test
    fun AB04_parse_baiAdapter_returnsNonNullWithEmotionIndex() {
        // 黑盒：读真实 assets/bai.adapter.json（coder-A1 已 copyModelAdapter 拷入 + AB-01 补 emotionIndex）
        // → ModelAdapterConfig.parse(内容) 必须返回非 null。bai 为无 motion 模型（motionAliases={}/idlePool=[]），
        // isSchemaValid 已由 coder-A1 豁免 motionAliases/idlePool 非空校验（消费真实 asset，非最小 fixture）。
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
    //  A3U-01 / A3U-09 / AP-30（contract-bai-params-v0.md §9 + §7）：
    //  emotionParamMap 结构与 D6「出现即完整」校验的零回归断言
    //  （依契约定案：字段可选；字段存在则强制 8 键 × _1/_2/_3 完整，否则解析失败。
    //   niziiro_mao 无该字段不得被 D6 误伤 — AP-30/AB-06 硬约束）
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

    @Test
    fun A3U09_niziiroMao_noEmotionParamMap_parsesNonNull() {
        // D6「出现即完整」：字段可选；niziiro_mao 无 emotionParamMap 字段 → 必须仍解析成功（AP-30/AB-06 零回归）
        val json = readAsset("niziiro_mao.adapter.json")
        val data: AdapterData? = ModelAdapterConfig.parse(json)
        assertNotNull("niziiro_mao 无 emotionParamMap 不得被 D6 误伤，parse 应非 null（AP-30 硬约束）", data)
        assertEquals("niziiro_mao", data!!.modelId)
        assertTrue("niziiro_mao 无 emotionParamMap → 空 map 或字段缺省", data.emotionParamMap.isEmpty() || !json.contains("emotionParamMap"))
    }

    // ─────────────────────────────────────────────────────────────
    //  AB-04 helper：读 assets 下真实 adapter json 文本
    // ─────────────────────────────────────────────────────────────

    private fun readAsset(name: String): String {
        val ins = context.assets.open(name)
        return ins.bufferedReader(Charsets.UTF_8).use { it.readText() }
    }

    // ─────────────────────────────────────────────────────────────
    //  helpers
    // ─────────────────────────────────────────────────────────────

    private fun assertParamMap(
        data: AdapterData,
        label: String,
        a: Float, i: Float, u: Float, e: Float, o: Float
    ) {
        val params = data.vowelParamMap[label]
        assertNotNull("vowelParamMap 应含 $label", params)
        assertEquals("$label.ParamA", a, params!!["ParamA"]!!, eps)
        assertEquals("$label.ParamI", i, params["ParamI"]!!, eps)
        assertEquals("$label.ParamU", u, params["ParamU"]!!, eps)
        assertEquals("$label.ParamE", e, params["ParamE"]!!, eps)
        assertEquals("$label.ParamO", o, params["ParamO"]!!, eps)
    }
}
