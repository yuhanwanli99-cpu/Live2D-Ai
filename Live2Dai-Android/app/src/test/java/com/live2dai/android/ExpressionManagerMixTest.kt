package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.ArgumentMatchers.anyFloat
import org.mockito.ArgumentMatchers.anyInt
import org.mockito.kotlin.any
import org.mockito.kotlin.doAnswer
import org.mockito.kotlin.mock
import org.mockito.kotlin.whenever

/**
 * ExpressionManager 结构化表情混合单元测试（M2 §1.3 setExpressionMix）。
 *
 * 对齐蓝本：ExpressionManagerTest.kt（AnimationPipeline + PurismModel(0L) + paramNameToIndex 构造风格）。
 * 验证 setExpressionMix 逐帧 apply 数学（加权混合/Overwrite/overrides/空 mix/防参数竞争）。
 */
class ExpressionManagerMixTest {

    private lateinit var expressionManager: AnimationPipeline.ExpressionManager
    private val paramNameToIndex = mapOf("ParamA" to 0, "ParamB" to 1)

    // 几个 ExpressionData：Add(+0.5)、Multiply(1.5)、Overwrite(0.5)、Overwrite(0.8)
    private val expAdd = ExpressionData(
        listOf(ExpressionParameter("ParamA", 0.5f, "Add"))
    )
    private val expMultiply = ExpressionData(
        listOf(ExpressionParameter("ParamA", 1.5f, "Multiply"))
    )
    private val expOverwrite05 = ExpressionData(
        listOf(ExpressionParameter("ParamA", 0.5f, "Overwrite"))
    )
    private val expOverwrite08 = ExpressionData(
        listOf(ExpressionParameter("ParamA", 0.8f, "Overwrite"))
    )

    @Before
    fun setUp() {
        val pipeline = AnimationPipeline(
            model = PurismModel(0L),
            paramNameToIndex = paramNameToIndex
        )
        expressionManager = pipeline.expressionManager
        expressionManager.expressionFadeTime = 0.5f
    }

    /**
     * P1-3 修复：不构造真实 PurismModel(0L)（触达 native JNI → JVM 单测 UnsatisfiedLinkError），
     * 改用 mock：getParameterValues 返回预置 float 数组，setParameterValues 记录写入值。
     *
     * 行为等价：getParameterValues() 返回当前存储的副本（apply 读取 base → 计算 → 写回），
     * setParameterValues() 将入参副本存入 storage，后续 getParameterValues() 读到写回值。
     * getParameterDefaultValues() 返回预置 default 数组（B4 断言依赖 default=0）。
     */
    private fun createMockModel(
        initial: FloatArray,
        defaults: FloatArray = FloatArray(initial.size),
        // 默认宽边界：不干扰 B3/B4 等的纯数学断言；B5/B9 等 clamp 用例显式传 ±1
        minValue: Float = -100f,
        maxValue: Float = 100f,
    ): PurismModel {
        // 内部存储数组；getParameterValues 返回真实引用（实现会原地写回快照→storage 实时更新）
        val storage = initial.copyOf()
        val model = mock<PurismModel>()
        whenever(model.getParameterValues()).thenAnswer { storage }
        whenever(model.getParameterDefaultValues()).thenAnswer { defaults.copyOf() }
        // 单参数写回（实现实际使用的 API：setParameterValue(idx: Int, value: Float)）
        doAnswer { inv ->
            val idx = inv.getArgument<Int>(0)
            val value = inv.getArgument<Float>(1)
            storage[idx] = value
            null
        }.`when`(model).setParameterValue(anyInt(), anyFloat())
        // 数组写回（保险，防实现混用数组 API）
        doAnswer { inv ->
            val arr = inv.getArgument<FloatArray>(0)
            for (i in arr.indices) storage[i] = arr[i]
            null
        }.`when`(model).setParameterValues(any())
        // min/max：默认 [-1f,1f]，使 B5（clamp 到 max）与 B9 的 clamp 安全通过，
        // 且不改变断言语义（clamp 上限 1.0 仍在 [-1f,1f] 内）
        whenever(model.getParameterMinValues()).thenReturn(FloatArray(initial.size) { minValue })
        whenever(model.getParameterMaxValues()).thenReturn(FloatArray(initial.size) { maxValue })
        return model
    }

    // ── B3: mix Add + Multiply 顺序敏感 ──

    @Test
    fun B3_mixAddMultiply_orderSensitive() {
        // expAdd(Add +0.5) 在 expMultiply(Multiply 1.5) 前
        // base=0.2, w=1.0: (0.2+0.5*1.0)*(1+1.5*1.0)=0.7*2.5=1.75
        expressionManager.setExpressionMix(
            mix = listOf(expAdd to 1.0f, expMultiply to 1.0f),
            overrides = emptyMap()
        )
        expressionManager.update(0.5f) // 完全淡入

        val model = createMockModel(floatArrayOf(0.2f, 0f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // (0.2+0.5)*(1+1.5) = 0.7*2.5 = 1.75
        assertEquals(1.75f, values!![0], 1e-4f)
    }

    // ── B3b: 顺序交换结果不同 ──

    @Test
    fun B3b_orderSwapped_differs() {
        // expMultiply(Multiply 1.5) 在 expAdd(Add +0.5) 前
        // base=0.2: 0.2*(1+1.5)=0.5; 0.5+0.5=1.0
        expressionManager.setExpressionMix(
            mix = listOf(expMultiply to 1.0f, expAdd to 1.0f),
            overrides = emptyMap()
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0.2f, 0f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // 0.2*2.5=0.5; 0.5+0.5=1.0
        assertEquals(1.0f, values!![0], 1e-4f)
    }

    // ── B4: Overwrite 后应用者胜 ──

    @Test
    fun B4_overwriteLaterWins() {
        // Overwrite 从 default 计算：default+(value-default)*w
        // default=0.0，先 0.5 后 0.8
        expressionManager.setExpressionMix(
            mix = listOf(expOverwrite05 to 1.0f, expOverwrite08 to 1.0f),
            overrides = emptyMap()
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0f, 0f))
        val defaults = model.getParameterDefaultValues()
        // 确认 default 为 0
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // 后应用者胜：终值 ≈ default+(0.8-default)*1.0=0.8
        assertEquals(0.8f, values!![0], 1e-4f)
    }

    // ── B5: overrides 在 mix 之后叠加 ──

    @Test
    fun B5_overridesAfterMix_clamp() {
        // mix: ExpAdd(Add+0.5) → base=0.2→0.7; override: ParamA multiplier=2.0 → 0.7*2.0=1.4 → clamp 到 max
        expressionManager.setExpressionMix(
            mix = listOf(expAdd to 1.0f),
            overrides = mapOf("ParamA" to 2.0f)
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0.2f, 0f), minValue = -1f, maxValue = 1f)
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // base(0.2)+0.5=0.7; 0.7*effectiveMultiplier — effective=1+(2-1)*1.0=2.0 → 1.4 → clamp 到 max=1.0
        assertEquals(1.0f, values!![0], 1e-4f)
    }

    // ── B6: 空 mix 保持当前表情状态 ──

    @Test
    fun B6_emptyMix_keepsState() {
        // 先设置一个表情
        expressionManager.setExpression(expAdd)
        expressionManager.update(0.5f)

        // 空 mix + overrides
        expressionManager.setExpressionMix(
            mix = emptyList(),
            overrides = mapOf("ParamB" to 1.5f)
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0f, 0.2f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // 表情 add→ParamA: 0+0.5=0.5; override→ParamB: 0.2*effective(1+(1.5-1)*1.0)=0.2*1.5=0.3
        assertEquals(0.5f, values!![0], 1e-3f)
        assertEquals(0.3f, values!![1], 1e-3f)
    }

    // ── B7: 查不到项跳过 ──

    @Test
    fun B7_missingExpression_skip() {
        // 一个不存在的表达式（索引 99）+ 一个合法项
        val badExp = ExpressionData(
            listOf(ExpressionParameter("NoSuchParam", 0.5f, "Add"))
        )
        expressionManager.setExpressionMix(
            mix = listOf(badExp to 0.5f, expAdd to 1.0f),
            overrides = emptyMap()
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0.2f, 0f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // badExp 跳过；expAdd: 0.2+0.5=0.7
        assertEquals(0.7f, values!![0], 1e-4f)
    }

    // ── B8: 归一化权重 applied ──

    @Test
    fun B8_weightNormalized_applied() {
        expressionManager.setExpressionMix(
            mix = listOf(expAdd to 0.53846f, expMultiply to 0.46154f),
            overrides = emptyMap()
        )
        expressionManager.update(0.5f)

        val model = createMockModel(floatArrayOf(0.2f, 0f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // 先 expAdd: 0.2+0.5*0.53846=0.46923; 后 expMultiply: 0.46923*(1+1.5*0.46154)=0.46923*1.69231=0.794...
        val expected = 0.2f + 0.5f * 0.53846f
        val afterMultiply = expected * (1f + 1.5f * 0.46154f)
        assertEquals(afterMultiply, values!![0], 1e-3f)
    }

    // ── B9: setExpressionMix 与 setExpression 互斥 ──

    @Test
    fun B9_mixExclusive_replaceSingle() {
        // 先 setExpression(single)
        expressionManager.setExpression(expOverwrite05)
        expressionManager.update(0.5f)
        assertSame(expOverwrite05, expressionManager.currentExpression)

        // 再 setExpressionMix → 替换状态
        expressionManager.setExpressionMix(
            mix = listOf(expAdd to 1.0f),
            overrides = emptyMap()
        )
        assertNull("setExpressionMix 清除 currentExpression", expressionManager.currentExpression)
        // mix 应在淡入中
        expressionManager.update(0.25f)

        val model = createMockModel(floatArrayOf(0.2f, 0f))
        expressionManager.apply(model)
        val values = model.getParameterValues()
        // 淡入一半: effectiveWeight=0.53846*0.5=0.26923 → 0.2+0.5*0.26923=0.3346...
        assertNotNull(values)
        assertTrue("值应在 0.2-0.7 之间", values!![0] in 0.2f..0.7f)
    }

    // ── B10: 淡入进度 effective ──

    @Test
    fun B10_fadeProgress_effective() {
        // 淡入中途（progress=0.5）
        expressionManager.setExpressionMix(
            mix = listOf(expAdd to 1.0f),
            overrides = mapOf("ParamB" to 2.0f)
        )
        expressionManager.update(0.25f) // progress=0.5

        val model = createMockModel(floatArrayOf(0.2f, 0.1f))
        expressionManager.apply(model)

        val values = model.getParameterValues()
        assertNotNull(values)
        // mix: effectiveWeight=1.0*0.5=0.5 → 0.2+0.5*0.5=0.45
        assertEquals(0.45f, values!![0], 1e-3f)
        // override: effective=1+(2-1)*0.5=1.5 → 0.1*1.5=0.15
        assertEquals(0.15f, values[1], 1e-3f)
    }
}
