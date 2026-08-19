package com.live2d.ai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Before
import org.junit.Test

/**
 * ExpressionManager 表情交叉淡变单元测试（级2-A 修复）。
 *
 * 验证 setExpression 切换时旧表情不再瞬间消失（弹回默认值 → 闪烁），
 * 而是与新表情交叉淡变：
 * - 新表情权重 = timer / fadeTime（0→1 淡入）
 * - 旧表情权重 = 切换瞬间显示权重 × (1 - timer / fadeTime)（1→0 淡出，仅当 timer ≤ fadeTime）
 * - 淡出完成后旧表情被清除，权重归一化到新表情
 */
class ExpressionManagerTest {

    private lateinit var expressionManager: AnimationPipeline.ExpressionManager

    private val joy = ExpressionData(
        listOf(ExpressionParameter("ParamA", 0.5f, "Overwrite"))
    )
    private val surprise = ExpressionData(
        listOf(ExpressionParameter("ParamA", 0.8f, "Overwrite"))
    )

    @Before
    fun setUp() {
        // AnimationPipeline 构造只做纯 Map 查找，不触达 native
        val pipeline = AnimationPipeline(
            model = PurismModel(0L),
            paramNameToIndex = mapOf("ParamA" to 0)
        )
        expressionManager = pipeline.expressionManager
        expressionManager.expressionFadeTime = 0.5f
    }

    @Test
    fun newExpressionFadesInFromZeroToFull() {
        expressionManager.setExpression(joy)

        assertEquals(0f, expressionManager.expressionWeight, 1e-4f)

        expressionManager.update(0.25f)
        assertEquals(0.5f, expressionManager.expressionWeight, 1e-4f)

        expressionManager.update(0.25f)
        assertEquals(1f, expressionManager.expressionWeight, 1e-4f)
    }

    @Test
    fun switchExpressionCrossFadesOldAndNew() {
        // joy 完全淡入后再切换
        expressionManager.setExpression(joy)
        expressionManager.update(0.5f)
        assertSame(joy, expressionManager.currentExpression)
        assertNull(expressionManager.previousExpression)

        // 切换瞬间：旧表情保留为淡出对象，新表情从 0 开始
        expressionManager.setExpression(surprise)
        assertSame(surprise, expressionManager.currentExpression)
        assertSame(joy, expressionManager.previousExpression)
        assertEquals(0f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(1f, expressionManager.previousExpressionWeight, 1e-4f)

        // 一半时间：新旧各占 0.5 —— 交叉淡变中点
        expressionManager.update(0.25f)
        assertEquals(0.5f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(0.5f, expressionManager.previousExpressionWeight, 1e-4f)

        // 淡入完成：新表情 1.0，旧表情 0 并被清除
        expressionManager.update(0.25f)
        assertEquals(1f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)
        assertNull(expressionManager.previousExpression)
    }

    @Test
    fun oldExpressionFadesOutOnlyDuringFadeWindow() {
        expressionManager.setExpression(joy)
        expressionManager.update(0.5f)
        expressionManager.setExpression(surprise)

        // 超过 fadeTime 后旧表情权重必须为 0（不再参与叠加）
        expressionManager.update(0.6f)
        assertEquals(1f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)
        assertNull(expressionManager.previousExpression)

        // 继续更新也不会复现旧表情权重
        expressionManager.update(0.1f)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)
    }

    @Test
    fun firstExpressionHasNoFadingPrevious() {
        // 首次设置：没有旧表情可淡出，previousExpressionWeight 恒为 0
        expressionManager.setExpression(joy)
        assertNull(expressionManager.previousExpression)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)

        expressionManager.update(0.25f)
        assertNull(expressionManager.previousExpression)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)
    }

    @Test
    fun interruptedFadeOldStartsFromCapturedWeight() {
        // joy 只淡入到 0.5 就被打断：旧表情应从 0.5 继续淡出，而不是跳回 1.0
        expressionManager.setExpression(joy)
        expressionManager.update(0.25f)
        assertEquals(0.5f, expressionManager.expressionWeight, 1e-4f)

        expressionManager.setExpression(surprise)
        assertEquals(0.5f, expressionManager.previousExpressionWeight, 1e-4f)

        expressionManager.update(0.25f)
        assertEquals(0.5f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(0.25f, expressionManager.previousExpressionWeight, 1e-4f)
    }

    @Test
    fun settingSameExpressionInstanceDoesNotKeepItAsPrevious() {
        // 同一个实例再次设置：不作为旧表情重复叠加（避免双重应用）
        expressionManager.setExpression(joy)
        expressionManager.update(0.5f)

        expressionManager.setExpression(joy)
        assertNull(expressionManager.previousExpression)
        assertSame(joy, expressionManager.currentExpression)
    }

    @Test
    fun clearExpressionResetsBothCurrentAndPrevious() {
        expressionManager.setExpression(joy)
        expressionManager.update(0.5f)
        expressionManager.setExpression(surprise)

        expressionManager.clearExpression()

        assertNull(expressionManager.currentExpression)
        assertNull(expressionManager.previousExpression)
        assertEquals(0f, expressionManager.expressionWeight, 1e-4f)
        assertEquals(0f, expressionManager.previousExpressionWeight, 1e-4f)
    }
}
