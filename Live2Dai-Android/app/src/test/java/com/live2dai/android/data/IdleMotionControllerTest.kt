package com.live2dai.android.data

import java.util.Random
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * R13 程序化待机（IdleMotionController）纯函数/纯状态 RED 断言（眼神漂移 + 头部微动 + 身体低频摆动）。
 *
 * 依据（唯一真相源，禁读 src/ 实现）：
 *  - .pi/spoq/search-report-expression.md §2.1（moeru-ai/airi 眼神扫视数值可直接复用）：
 *      EYE_SACCADE 间隔 400-4400ms；focusTarget → ParamEyeBallX ∈ [-0.5,0.5]、ParamEyeBallY ∈ [-0.5,0.35]；
 *      平滑落点 = lerp(current, target, 0.3)（阻尼，非瞬切）；
 *      head focusController（damping 插值）+ 头部微动幅度 ±5-15（上限 ±30-45）；低频 sine（IdleAnimator 分层错相位）。
 *  - .pi/spoq/search-report-expression.md §2.1 官方标准参数表：ParamAngleX/Y ∈ [-30,30]、ParamEyeBallX/Y ∈ [-1,1]。
 *  - .pi/spoq/model-facts-bai.md §四：bai 有 ParamEyeBallX/Y、ParamAngleX/Y/Z（头部）、多段 ParamBodyAngle 身体。
 *
 * 封装的待实现生产符号（data/IdleMotionController.kt，**均不存在，编译期 RED 属预期**，cil=coder 待实现）：
 *   data class IdleSample(
 *       val eyeBallX: Float, val eyeBallY: Float,     // 眼神平滑输出
 *       val headAngleX: Float, val headAngleY: Float, // 头部微动（度）
 *       val bodySway: Float,                          // 身体低频摆动（归一化，驱动 ParamBodyAngle 通道）> 不写死具体 param 名，见备注
 *   )
 *   object IdleMotionConfig {
 *       const val EYE_DAMPING = 0.3f                 // 眼神阻尼 lerp 系数（airi 锁定）
 *       const val SACCADE_MIN_MS = 400f              // 扫视最小间隔
 *       const val SACCADE_MAX_MS = 4400f             // 扫视最大间隔
 *       const val EYE_TARGET_X_AMP = 0.5f            // EyeBallX 目标幅值 → [-0.5, 0.5]
 *       const val EYE_TARGET_Y_POS = 0.35f           // EyeBallY 目标上界 → 0.35
 *       const val EYE_TARGET_Y_NEG = 0.5f            // EyeBallY 目标下界幅度 → -0.5
 *       const val HEAD_AMP_MAX_DEG = 15f             // 头部微动幅度上限（±5-15）
 *       const val BAI_ANGLE_CLAMP_DEG = 30f          // bai 头角参数钳制 ±30（官方标准表）
 *       const val BAI_EYEBALL_CLAMP = 1f             // bai 眼球参数钳制 ±1（官方标准表）
 *   }
 *   // 纯函数眼神平滑（可单测）：current + (target - current) * damping
 *   fun dampedLerp(current: Float, target: Float, damping: Float = IdleMotionConfig.EYE_DAMPING): Float
 *   class IdleMotionController(private val random: Random = Random.Default) {
 *       fun update(deltaSeconds: Float): IdleSample
 *       val currentEyeTargetX: Float       // 当前扫视目标（测试面）
 *       val currentEyeTargetY: Float
 *       val saccadeCount: Int              // 已触发扫视次数（测试面）
 *   }
 *
 * 设计说明：
 *   - 眨眼（EyeLOpen/ROpen）已由既有 EyeBlinkManager 承担 → **IdleMotionController 不触碰眼开合/呼吸通道**，
 *     IdleSample 无 EyeLOpen/ROpen/Breath 字段（本测试与既有 Blink 测试无冲突，注明）。
 *   - 头部/身体为「正弦/阻尼」型连续动画（每帧更新 → 连续平滑），眼神为「扫视门控」型（离散跳变 + lerp 平滑）——
 *     「不同频率（分形感）」即由此结构可机械断言：窗口内头部逐帧变化率远高于眼神扫视稀疏度。
 *   - 身体为低频：窗口内身体过零（符号翻转）次数 < 头部过零次数（低频 → 周期长 → 过零稀少）。
 */
class IdleMotionControllerTest {

    private val dt = 1f / 60f // 60 FPS 帧步

    // ═══════════════════════════════════════════════════════════
    // IM-01：眼神阻尼常量 + dampedLerp 纯函数（lerp 阻尼 0.3）
    // ═══════════════════════════════════════════════════════════

    /** IM-01-①：EYE_DAMPING == 0.3（airi 锁定 lerp 阻尼 0.3）。 */
    @Test
    fun eyeDamping_constant_isPointThree() {
        assertEquals("眼神阻尼应 == 0.3（airi eye-motions.ts）", 0.3f, IdleMotionConfig.EYE_DAMPING, 0f)
    }

    /** IM-01-②：dampedLerp 数学（current+(target-current)*damping）+ 默认阻尼 0.3。 */
    @Test
    fun dampedLerp_movesPointThreeTowardTarget_byDefault() {
        assertEquals("0 → 1，一步 lerp(0.3) = 0.3", 0.3f, dampedLerp(0f, 1f), 1e-6f)
        assertEquals("0.5 → 0，一步 lerp(0.3) = 0.5-0.15 = 0.35", 0.35f, dampedLerp(0.5f, 0f), 1e-6f)
        assertEquals("1.0 → 0，一步 lerp(0.3) = 0.7", 0.7f, dampedLerp(1f, 0f), 1e-6f)
        // 显式 damping 参数
        assertEquals("显式 damping=1.0 → 瞬时命中 target", 1.0f, dampedLerp(0f, 1f, 1.0f), 1e-6f)
        assertEquals("显式 damping=0.5 → 中点 0.5", 0.5f, dampedLerp(0f, 1f, 0.5f), 1e-6f)
    }

    /** IM-01-③：眼神非瞬切（阻尼平滑）。断言：扫视间窗口内，眼神输出到当前目标的「缺口」每帧单调收敛（不增大），
     *  即输出以 lerp 阻尼逐步逼近目标而非逐帧瞬跳。新扫视边界重置基准（缺口回新目标满值后续再收敛）。 */
    @Test
    fun eyeSmoothing_isDampedNotInstantaneous_onSaccadeTargetChange() {
        val ctl = makeController(seed = 42L)
        var lastGap = -1f // 未初始化哨兵
        var targetX = ctl.currentEyeTargetX
        var violations = 0
        var checked = 0
        for (i in 0 until 2400) { // 40s @60fps
            val beforeCount = ctl.saccadeCount
            val s = ctl.update(dt)
            if (ctl.saccadeCount > beforeCount) {
                // 新扫视：目标更新，重锚基准（此帧不参与缺口收敛比较）
                targetX = ctl.currentEyeTargetX
                lastGap = kotlin.math.abs(targetX - s.eyeBallX)
                continue
            }
            val gap = kotlin.math.abs(ctl.currentEyeTargetX - s.eyeBallX)
            if (lastGap >= 0f) {
                // 阻尼约束：扫视间缺口应单调收敛（不增大）。若激增 → 瞬跳违规。
                if (gap > lastGap + 1e-3f) violations++
                checked++
            }
            lastGap = gap
        }
        assertTrue(
            "眼神应阻尼平滑（扫视间缺口单调收敛，0 次缺口激增），违规 $violations 次",
            violations == 0,
        )
        assertTrue("应有足够帧参与收敛检查（checked=$checked > 0）", checked > 0)
    }

    // ═══════════════════════════════════════════════════════════
    // IM-02：眼神扫视间隔 ∈ [400,4400]ms
    // ═══════════════════════════════════════════════════════════

    /** IM-02：驱动 60s 逐帧，每次扫视（saccadeCount 递增）间隔落 [0.4,4.4]s。固定种子可复现。 */
    @Test
    fun saccadeIntervals_fallWithin_400to4400ms() {
        val ctl = makeController(seed = 7L)
        var accumulatedSinceSaccade = 0f
        var observedIntervals = 0
        for (i in 0 until 3600) { // 60s @60fps
            val before = ctl.saccadeCount
            ctl.update(dt)
            accumulatedSinceSaccade += dt
            if (ctl.saccadeCount > before) {
                assertTrue(
                    "扫视间隔应 ∈ [0.4,4.4]s，实际 ${accumulatedSinceSaccade}s",
                    accumulatedSinceSaccade in 0.4f..4.4f,
                )
                observedIntervals++
                accumulatedSinceSaccade = 0f
            }
        }
        assertTrue(
            "60s 内应观察到多次扫视（稀疏但非零），实际 $observedIntervals 次",
            observedIntervals >= 2,
        )
        // 上界确认：任一已知间隔被拦截在上限内（若实现超上限这里即红）
        assertTrue("扫视间隔上限断言已覆盖 ≥$observedIntervals 次且全部 ∈[400,4400]", observedIntervals >= 2)
    }

    /** IM-02-②：EID 范围常量锁定（400/4400）。 */
    @Test
    fun saccadeRangeConstants_matchResearch() {
        assertEquals("扫视最小间隔 == 400ms", 400f, IdleMotionConfig.SACCADE_MIN_MS, 0f)
        assertEquals("扫视最大间隔 == 4400ms", 4400f, IdleMotionConfig.SACCADE_MAX_MS, 0f)
        assertTrue("min < max", IdleMotionConfig.SACCADE_MIN_MS < IdleMotionConfig.SACCADE_MAX_MS)
    }

    // ═══════════════════════════════════════════════════════════
    // IM-03：眼神目标值域（X ∈ [-0.5,0.5]、Y ∈ [-0.5,0.35]）
    // ═══════════════════════════════════════════════════════════

    /** IM-03-①：任意扫视目标 X ∈ [-0.5,0.5]、Y ∈ [-0.5,0.35]（airi focusTarget 折算）。 */
    @Test
    fun saccadeTargets_stayWithinEyeBallRanges() {
        val ctl = makeController(seed = 123L)
        for (i in 0 until 2400) { // 40s
            val before = ctl.saccadeCount
            ctl.update(dt)
            if (ctl.saccadeCount > before) {
                assertTrue(
                    "EyeBallX 目标应 ∈ [-0.5,0.5]，实际 ${ctl.currentEyeTargetX}",
                    ctl.currentEyeTargetX in -0.5f..0.5f,
                )
                assertTrue(
                    "EyeBallY 目标应 ∈ [-0.5,0.35]，实际 ${ctl.currentEyeTargetY}",
                    ctl.currentEyeTargetY in -0.5f..0.35f,
                )
            }
        }
        // 至少要扫几次才有意义
        assertTrue("40s 内应触发多次扫视", ctl.saccadeCount >= 2)
    }

    // ═══════════════════════════════════════════════════════════
    // IM-04：头部微动幅度 ≤15°（且 ≤bai 钳制 30°）
    // ═══════════════════════════════════════════════════════════

    /** IM-04-①：头部角输出绝对值 ≤ 15°（±5-15 设计上限）；滑动不超 bai ±30。 */
    @Test
    fun headMotion_amplitudeWithin_15deg_andBai30degClamp() {
        val ctl = makeController(seed = 5L)
        var maxHead = 0f
        for (i in 0 until 3600) {
            val s = ctl.update(dt)
            maxHead = maxOf(maxHead, kotlin.math.abs(s.headAngleX), kotlin.math.abs(s.headAngleY))
        }
        assertTrue(
            "头部微动幅度应 ≤15°（±5-15），实测 max |angleX/angleY|=$maxHead°",
            maxHead <= IdleMotionConfig.HEAD_AMP_MAX_DEG,
        )
        assertTrue(
            "头部幅度不得超 bai 参数钳制 ±30°",
            maxHead <= IdleMotionConfig.BAI_ANGLE_CLAMP_DEG,
        )
    }

    // ═══════════════════════════════════════════════════════════
    // IM-05：头部 vs 眼神不同频率（分形感）——头部连续逐帧 vs 眼神稀疏扫视
    // ═══════════════════════════════════════════════════════════

    /** IM-05：窗口内头部逐帧变化（连续动画）帧占比 >> 眼神扫视触发帧占比（稀疏）→ 头部频率高于扫视门控的"跳变率"。 */
    @Test
    fun headIsContinuousPerFrame_whileEyeSaccadesAreSparse_differentFrequency() {
        val ctl = makeController(seed = 9L)
        var prevHeadX = 0f
        var headChangedFrames = 0
        val totalFrames = 2400
        for (i in 0 until totalFrames) {
            val s = ctl.update(dt)
            if (kotlin.math.abs(s.headAngleX - prevHeadX) > 1e-6f) headChangedFrames++
            prevHeadX = s.headAngleX
        }
        val headChangeRatio = headChangedFrames.toFloat() / totalFrames
        assertTrue(
            "头部应逐帧连续变化（正弦/阻尼型）：变化帧占比=$headChangeRatio 应 >0.9",
            headChangeRatio > 0.9f,
        )
        // 眼神扫视稀疏：40s 内扫视次数应远小于帧数（非逐帧跳变）
        assertTrue(
            "眼神为稀疏扫视门控：40s 内扫视 ${ctl.saccadeCount} 次应 << 2400 帧",
            ctl.saccadeCount < totalFrames / 10,
        )
    }

    // ═══════════════════════════════════════════════════════════
    // IM-06：身体低频（周期 > 头部）——窗口内身体过零次数 < 头部过零次数
    // ═══════════════════════════════════════════════════════════

    /** IM-06：60s 窗口内，身体摆动过零（符号翻转）次数 < 头部过零次数 → 身体周期更长（低频）。 */
    @Test
    fun bodySwayIsLowerFrequencyThanHead_zeroCrossingCount() {
        val ctl = makeController(seed = 31L)
        var headZero = 0
        var bodyZero = 0
        var prevHead = 0f
        var prevBody = 0f
        var headInit = false
        var bodyInit = false
        for (i in 0 until 3600) { // 60s @60fps
            val s = ctl.update(dt)
            if (headInit && (prevHead >= 0f) != (s.headAngleY >= 0f)) headZero++
            if (bodyInit && (prevBody >= 0f) != (s.bodySway >= 0f)) bodyZero++
            prevHead = s.headAngleY
            prevBody = s.bodySway
            headInit = true
            bodyInit = true
        }
        assertTrue(
            "身体应低频（周期>头部）：身体过零 $bodyZero 次应 < 头部过零 $headZero 次",
            bodyZero < headZero,
        )
        assertTrue("头部应有足够过零（确为振荡非静止）：$headZero", headZero >= 2)
    }

    /** IM-06-②：身体示意通道存在且幅度正常（归一化域，避免越界写坏 ParamBodyAngle）。 */
    @Test
    fun bodySway_boundedNormalized() {
        val ctl = makeController(seed = 11L)
        var maxBody = 0f
        for (i in 0 until 3600) {
            maxBody = maxOf(maxBody, kotlin.math.abs(ctl.update(dt).bodySway))
        }
        assertTrue("bodySway 幅度应归一化（低频小幅），实测 max=$maxBody", maxBody in 0.001f..0.3f)
    }

    // ═══════════════════════════════════════════════════════════
    // IM-07：bai 参数全域钳制（Angle ±30、EyeBall ±1）+ 不碰眨眼/呼吸通道
    // ═══════════════════════════════════════════════════════════

    /** IM-07-①：bai 硬钳制常量（±30 头角 / ±1 眼球）。 */
    @Test
    fun baiClampConstants_matchStandardParameterTable() {
        assertEquals("bai 头角钳制 == 30°", 30f, IdleMotionConfig.BAI_ANGLE_CLAMP_DEG, 0f)
        assertEquals("bai 眼球钳制 == 1", 1f, IdleMotionConfig.BAI_EYEBALL_CLAMP, 0f)
    }

    /** IM-07-②：IdleSample 不含眨眼（EyeLOpen/ROpen）与呼吸字段（眨眼/呼吸归既有 EyeBlinkManager/BreathManager，防通道冲突）。 */
    @Test
    fun idleSample_doesNotExposeBlinkOrBreathChannel() {
        assertFalse(
            "IdleSample 字段不得含 EyeLOpen/EYEOPEN（眨眼归 EyeBlinkManager）",
            IdleSample::class.java.declaredFields.any { it.name.contains("EyeL") || it.name.contains("EYEOPEN") },
        )
        assertFalse(
            "IdleSample 字段不得含 Breath（呼吸归 BreathManager）",
            IdleSample::class.java.declaredFields.any { it.name.contains("Breath") },
        )
    }

    // ═══════════════════════════════════════════════════════════
    // 测试辅助
    // ═══════════════════════════════════════════════════════════

    private fun makeController(seed: Long): IdleMotionController = IdleMotionController(Random(seed))
}
