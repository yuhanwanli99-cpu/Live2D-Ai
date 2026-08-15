// =============================================================================
// RED 测试 — R17 统一优先级仲裁器（单入口管理所有联动源）
// T8 测试设计师 | 依据: .pi/spoq/translation.md R17 + search-report-expression.md
//
// 【接口契约 — 给 coder】
// 新类:  package com.live2dai.android.data
//        object LiaisonArbiter   （纯函数，JVM 可测，禁 Android 依赖）
//
//  优先级（需求实锤）: 情绪姿态动作(emotion) > 说话(speaking) > 表情(expression) > idle
//
//  通道划分（tester 裁定）:
//     - BODY_HEAD 通道（身体角 + 头角）候选源按优先级: emotion(3) > speaking(2) > idle(1)
//       （expression 6 键为 brow/eye/mouth，不驱动 body/head 角 → 不参赛 body/head，见 R13）
//     - MOUTH 通道（口型参数）候选源按优先级: speaking(2) > expression(1)
//       （emotion 姿态帧不驱动口型，不参赛 mouth）
//
//  数据结构:
//     enum class Source      { EMOTION, SPEAKING, EXPRESSION, IDLE }
//     enum class Channel     { BODY_HEAD, MOUTH }
//     data class Sources(
//         val emotionActive:   Boolean = false,
//         val speakingActive:  Boolean = false,
//         val expressionActive:Boolean = false,
//         val idleActive:      Boolean = false
//     )
//     data class ArbitrationResult(
//         val winner: Source?,      // 全局最高优先级活跃源（emotion>speaking>expression>idle，无人则为 null）
//         val drives: Map<Channel, Source>,  // 各通道由谁驱动（无驱动则不含 key）
//         val yieldSources: Set<Source>      // 在参与通道上被高优先级源让位的所有活跃源（至少让位 1 通道）
//     )
//
//  仲裁规则（机械）:
//     1. winner = 按 [EMOTION,SPEAKING,EXPRESSION,IDLE] 序取首个 active；全 false → null。
//     2. BODY_HEAD winner = 按 [EMOTION,SPEAKING,IDLE] 序取首个 active。
//     3. MOUTH    winner = 按 [SPEAKING,EXPRESSION] 序取首个 active。
//     4. yieldSources = 参与某通道且在该通道落败的活跃源集合；
//        (a) BODY_HEAD: 若 emotion active，speaking 与 idle 均落败 body/head；
//            若 speaking active 且 emotion 否，idle 落败 body/head；
//        (b) MOUTH: 若 speaking active，expression 落败 mouth。
//        注意：speaking 在 emotion active 时 body/head 让位、但 mouth 仍驱动——
//        此时 speaking 仍计入 yieldSources（让位≥1 通道），其仍驱动 mouth 由 drives 表达。
//
//  真值表 12 条（见测试体，每条 lock 具体语义）。
// =============================================================================

package com.live2dai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LiaisonArbiterTest {

    private fun arb(
        e: Boolean = false, s: Boolean = false, x: Boolean = false, i: Boolean = false
    ) = LiaisonArbiter.arbitrate(LiaisonArbiter.Sources(e, s, x, i))

    // ---- 单源基线 ----

    @Test
    fun `LA-01 all inactive nothing drives no winner`() {
        val r = arb()
        assertNull(r.winner)
        assertTrue("全 false 无驱动", r.drives.isEmpty())
        assertTrue(r.yieldSources.isEmpty())
    }

    @Test
    fun `LA-02 emotion alone drives body-head wins overall`() {
        val r = arb(e = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertFalse("emotion 不驱动口型", r.drives.containsKey(LiaisonArbiter.Channel.MOUTH))
        assertTrue(r.yieldSources.isEmpty())
    }

    @Test
    fun `LA-03 speaking alone drives body-head and mouth`() {
        val r = arb(s = true)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.winner)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertTrue(r.yieldSources.isEmpty())
    }

    @Test
    fun `LA-04 expression alone drives mouth only`() {
        val r = arb(x = true)
        assertEquals(LiaisonArbiter.Source.EXPRESSION, r.winner)
        assertEquals(LiaisonArbiter.Source.EXPRESSION, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertFalse(r.drives.containsKey(LiaisonArbiter.Channel.BODY_HEAD))
        assertTrue(r.yieldSources.isEmpty())
    }

    @Test
    fun `LA-05 idle alone drives body-head only`() {
        val r = arb(i = true)
        assertEquals(LiaisonArbiter.Source.IDLE, r.winner)
        assertEquals(LiaisonArbiter.Source.IDLE, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertFalse(r.drives.containsKey(LiaisonArbiter.Channel.MOUTH))
        assertTrue(r.yieldSources.isEmpty())
    }

    // ---- 二元冲突（低优先级让位）----

    @Test
    fun `LA-06 speaking beats idle on body-head idle yields head-body`() {
        // 需求例：speaking 时 idle 的头/身通道让位
        val r = arb(s = true, i = true)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.winner)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(setOf(LiaisonArbiter.Source.IDLE), r.yieldSources)
    }

    @Test
    fun `LA-07 emotion beats idle idle yields all angles`() {
        // 需求例：emotion 时 idle 全角度让位
        val r = arb(e = true, i = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(setOf(LiaisonArbiter.Source.IDLE), r.yieldSources)
    }

    @Test
    fun `LA-08 emotion beats speaking on body-head speaking keeps mouth`() {
        val r = arb(e = true, s = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        // speaking 口型仍驱动（不写冲突口型参数 only on speaking 让位 body/head）
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        // speaking 已在 body/head 让位 → 计入 yieldSources；emotion 不驱动口型故不 yield mouth
        assertEquals(setOf(LiaisonArbiter.Source.SPEAKING), r.yieldSources)
    }

    @Test
    fun `LA-09 speaking beats expression on mouth expression yields`() {
        // 需求例：说话(口型) > 表情 —— expression 口型让位
        val r = arb(s = true, x = true)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.winner)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(setOf(LiaisonArbiter.Source.EXPRESSION), r.yieldSources)
    }

    // ---- 多源 ----

    @Test
    fun `LA-10 emotion beats all on body-head speaking beats expression on mouth`() {
        // 全部四源都 active：emotion 全局赢 body/head；speaking 赢 mouth；
        // speaking 在 body/head 让位 emotion、expression 在 mouth 让位 speaking、idle 让位 body/head
        val r = arb(e = true, s = true, x = true, i = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(
            setOf(
                LiaisonArbiter.Source.SPEAKING,
                LiaisonArbiter.Source.EXPRESSION,
                LiaisonArbiter.Source.IDLE
            ),
            r.yieldSources
        )
    }

    @Test
    fun `LA-11 emotion speaking idle body-head emotion mouth speaking`() {
        val r = arb(e = true, s = true, i = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(
            setOf(LiaisonArbiter.Source.SPEAKING, LiaisonArbiter.Source.IDLE),
            r.yieldSources
        )
    }

    @Test
    fun `LA-12 emotion expression idle emotion body-head expression mouth idle yields`() {
        // 无 speaking：emotion 赢 body/head；expression 赢 mouth；idle 让位 body/head
        val r = arb(e = true, x = true, i = true)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.winner)
        assertEquals(LiaisonArbiter.Source.EMOTION, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.EXPRESSION, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(setOf(LiaisonArbiter.Source.IDLE), r.yieldSources)
    }

    @Test
    fun `LA-13 speaking expression idle speaking on both expression and idle yield`() {
        val r = arb(s = true, x = true, i = true)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.winner)
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.BODY_HEAD])
        assertEquals(LiaisonArbiter.Source.SPEAKING, r.drives[LiaisonArbiter.Channel.MOUTH])
        assertEquals(
            setOf(LiaisonArbiter.Source.EXPRESSION, LiaisonArbiter.Source.IDLE),
            r.yieldSources
        )
    }
}
