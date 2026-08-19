package com.live2d.ai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test


/**
 * Android 用户交互纯函数断言（契约 C-M5 §2.1/§3，M5-A1~A4 + M5-A6 优先级语义数据锁）。
 *
 * 覆盖生产文件 `data/InteractionMath.kt`（**新增，RED 待实现**）：
 *   const val INTERACTION_SCALE_MIN = 0.1f
 *   const val INTERACTION_SCALE_MAX = 5.0f
 *   fun clampScale(scale: Float): Float                    // coerceIn(0.1f, 5.0f)；NaN→1.0（防御）
 *   fun pinchScale(oldScale, startDistancePx, currentDistancePx): Float
 *       // = clampScale(oldScale * current/start)；start<=0 或非有限 → oldScale
 *   fun dragPxToModel(dxPx, dyPx, viewportW, viewportH, boundsLeft, boundsRight,
 *                     boundsTop, boundsBottom): Pair<Float, Float>
 *       // dxModel = dxPx * (boundsRight - boundsLeft)/viewportW（屏幕 x 与模型 x 同向）
 *       // dyModel = -dyPx * (boundsTop - boundsBottom)/viewportH（屏幕 y 向下 ↔ 模型 y 向上）
 *   fun encodeOverride(scale, xOffset, yOffset): String    // JSON {"version":1,...}
 *   fun decodeOverride(json: String?): Override?           // 非法/缺字段/version!=1/非有限 → null
 *   data class Override(val scale: Float, val xOffset: Float, val yOffset: Float)
 *
 * 依据全部取自契约 C-M5 §2.1/§2.3 + §3 验收断言表（M5-A1/A2/A3/A4 + R8 语义 M5-A6）。
 * 禁止读实现源码（app/src/main 下 Live2DView/Live2DRenderer 等禁读、.pi/spoq/src/ 禁读）。
 */

// M5-A1：clampScale + 钳制上下界常量
class InteractionMathTest {

    @Test
    fun clampScale_boundsConstants_matchContract() {
        // 契约束死：INTERACTION_SCALE_MIN==0.1f / MAX==5.0f（researched：Open-LLM-VTuber MIN=0.1、
        // N.E.K.O. SCALE_MAX=5.0、Open-LLM-VTuber MAX_SCALE=5.0）
        assertEquals("INTERACTION_SCALE_MIN 应 == 0.1f", 0.1f, INTERACTION_SCALE_MIN, 0f)
        assertEquals("INTERACTION_SCALE_MAX 应 == 5.0f", 5.0f, INTERACTION_SCALE_MAX, 0f)
        assertTrue("MIN 应 < MAX", INTERACTION_SCALE_MIN < INTERACTION_SCALE_MAX)
    }

    @Test
    fun clampScale_lowerBound_upperBound_noop() {
        // 契约锁定值：0.05→0.1、1.0→1.0、7.0→5.0
        assertEquals("0.05 应钳到下界 0.1", 0.1f, clampScale(0.05f), 1e-6f)
        assertEquals("1.0 在界内应不变", 1.0f, clampScale(1.0f), 1e-6f)
        assertEquals("7.0 应钳到上界 5.0", 5.0f, clampScale(7.0f), 1e-6f)
    }

    @Test
    fun clampScale_nan_returnsOne_defensive() {
        // 契约防御：NaN → 1.0（coerceIn 对 NaN 的 <、> 都 false，若不特判会返回 NaN，故必须防御命中 1.0）
        assertEquals("NaN 应防御回 1.0", 1.0f, clampScale(Float.NaN), 0f)
        assertEquals("正无穷应钳到上界 5.0", 5.0f, clampScale(Float.POSITIVE_INFINITY), 0f)
        assertEquals("负无穷应钳到下界 0.1", 0.1f, clampScale(Float.NEGATIVE_INFINITY), 0f)
    }

    // M5-A2：pinchScale 捏合比例 + 钳制 + 非法输入防御
    @Test
    fun pinchScale_contractLockedValues() {
        // start=100/current=200/old=1.0 → 1.0*200/100 = 2.0 → 2.0
        assertEquals("放大 2 倍钳制前命中 2.0", 2.0f, pinchScale(1.0f, 100f, 200f), 1e-4f)
        // start=100/current=50/old=4.0 → 4.0*50/100 = 2.0 → 2.0
        assertEquals("捏合缩小比例命中 2.0（由 4.0 降至 2.0）", 2.0f, pinchScale(4.0f, 100f, 50f), 1e-4f)
    }

    @Test
    fun pinchScale_clampHit_lowerAndUpper() {
        // 钳制命中：当前/起点比例 5/100 → 0.05 → 下钳 0.1
        assertEquals("放大比例 200/100 钳制命中", 0.1f, pinchScale(1.0f, 100f, 5f), 1e-4f)
        // 600/100 → 6.0 → 上钳 5.0
        assertEquals("捏合放大 6.0 应钳到 5.0", 5.0f, pinchScale(1.0f, 100f, 600f), 1e-4f)
    }

    @Test
    fun pinchScale_invalidStart_returnsOldScale_unchanged() {
        // 契约：startDistancePx<=0 → 返回 oldScale（不变）
        assertEquals("start=0 应返回 oldScale", 1.0f, pinchScale(1.0f, 0f, 200f), 1e-6f)
        assertEquals("start<0 应返回 oldScale", 2.0f, pinchScale(2.0f, -5f, 100f), 1e-6f)
        // 非有限（防御）：start 为 NaN / Inf → 返回 oldScale
        assertEquals("start=NaN 应返回 oldScale", 1.0f, pinchScale(1.0f, Float.NaN, 200f), 1e-6f)
        assertEquals("start=+Inf 应返回 oldScale", 1.0f, pinchScale(1.0f, Float.POSITIVE_INFINITY, 200f), 1e-6f)
    }

    @Test
    fun pinchScale_nonFiniteCurrent_returnsOldScale_defensive() {
        // 契约守卫「!isFinite → oldScale」：current 非有限使比例非有限，防御回 oldScale
        assertEquals("current=+Inf 应返回 oldScale", 1.0f, pinchScale(1.0f, 100f, Float.POSITIVE_INFINITY), 1e-6f)
        assertEquals("current=NaN 应返回 oldScale", 2.0f, pinchScale(2.0f, 100f, Float.NaN), 1e-6f)
    }

    // M5-A3：dragPxToModel 拖拽量纲 + 符号
    @Test
    fun dragPxToModel_contractLockedValues() {
        // viewport 1000×2000、bounds(-1,1,2,-2)（左=-1 右=1 上=2 下=-2，宽 2 高 4 模型单位）
        val (dxModel, dyModel) = dragPxToModel(250f, 500f, 1000f, 2000f, -1f, 1f, 2f, -2f)

        // dxModel = 250 * (1-(-1))/1000 = 250*2/1000 = 0.5（屏幕 x 与模型 x 同向）
        assertEquals("dxPx=250 → dxModel=0.5", 0.5f, dxModel, 1e-4f)
        // dyModel = -500 * (2-(-2))/2000 = -500*4/2000 = -1.0（屏幕 y 向下 → 模型 y 向上，符号翻转）
        assertEquals("dyPx=+500(屏幕向下) → dyModel=-1.0(模型向上)", -1.0f, dyModel, 1e-4f)
    }

    @Test
    fun dragPxToModel_signAndDimensionVsSymmetricBBox() {
        // 符号：屏幕右拖 → 模型正方向；屏下拖右手则负（由公式 -dyPx）决定
        val (dxRight, dyUp) = dragPxToModel(100f, -100f, 1000f, 2000f, -1f, 1f, 2f, -2f)
        assertTrue("屏幕右拖(px+) → dxModel 为正", dxRight > 0f)
        // dx=100*2/1000=0.2；dy=-(-100)*4/2000=+0.2
        assertEquals("dx=100 → 0.2", 0.2f, dxRight, 1e-4f)
        // 屏幕向上拖(动态上移 dyPx<0) → 模型正方向
        assertTrue("屏幕向上拖(dyPx<0) → dyModel 为正（模型向上）", dyUp > 0f)
        assertEquals("dy=-100 → dyModel=+0.2", 0.2f, dyUp, 1e-4f)

        // 量纲：相同 px 位移在更窄 viewport 映射更大幅度（线性比例）
        val (dxNarrow, _) = dragPxToModel(250f, 500f, 500f, 2000f, -1f, 1f, 2f, -2f)
        assertEquals("viewportW 减半 → dxModel 加倍（0.5→1.0）", 1.0f, dxNarrow, 1e-4f)
    }

    // M5-A4：encode/decodeOverride 往返一致 + 损坏/缺字段/版本/非有限 → null
    @Test
    fun override_roundTrip_conservesAllFields() {
        val json = encodeOverride(1.5f, -0.2f, 0.3f)
        assertTrue("json 应以 version:1 开头形态", json.contains("\"version\":1"))

        val decoded = decodeOverride(json)
        assertNotNull("合法 json 应能解码", decoded)
        assertEquals("scale 往返一致", 1.5f, decoded!!.scale, 1e-6f)
        assertEquals("xOffset 往返一致", -0.2f, decoded.xOffset, 1e-6f)
        assertEquals("yOffset 往返一致", 0.3f, decoded.yOffset, 1e-6f)
    }

    @Test
    fun override_decode_invalidJson_returnsNull_noCrash() {
        assertNull("非法 json → null 不崩溃", decodeOverride("这不是合法 JSON!!!"))
        assertNull("null 输入 → null", decodeOverride(null))
        assertNull("空字符串 → null", decodeOverride(""))
    }

    @Test
    fun override_decode_missingField_returnsNull() {
        // 缺 yOffset 字段 → null（字段缺失不默认补零，契约「字段缺失 → null」）
        val missingField = """{"version":1,"scale":1.5,"xOffset":-0.2}"""
        assertNull("缺 yOffset 字段 → null", decodeOverride(missingField))
        // 缺 scale 字段 → null
        val missingScale = """{"version":1,"xOffset":-0.2,"yOffset":0.3}"""
        assertNull("缺 scale 字段 → null", decodeOverride(missingScale))
    }

    @Test
    fun override_decode_wrongVersion_returnsNull() {
        val futureVersion = """{"version":2,"scale":1.5,"xOffset":-0.2,"yOffset":0.3}"""
        assertNull("version!=1 → null（版本不兼容不崩溃）", decodeOverride(futureVersion))
    }

    @Test
    fun override_decode_nonFiniteValue_returnsNull() {
        // 非有限值防御：yOffset 为 1e999（JVM 解析溢出为 +Infinity）→ null，不崩
        val nonFinite = """{"version":1,"scale":1.5,"xOffset":-0.2,"yOffset":1e999}"""
        assertNull("yOffset 非有限(1e999→+Inf) → null", decodeOverride(nonFinite))
        // scale 为字符串（类型非法）→ null
        val wrongType = """{"version":1,"scale":"big","xOffset":-0.2,"yOffset":0.3}"""
        assertNull("scale 类型非法(字符串) → null", decodeOverride(wrongType))
    }

    // ============================================================
    // M5-A6：用户 override 覆盖 fit > 默认（R8 优先级语义数据锁）
    // ------------------------------------------------------------------
    // [UNSURE] 契约 C-M5 §2.1 未定义优先级解析的具名纯函数：§2.3 将优先级描述为
    // `Live2DRenderer` 渲染器状态字段（userScale/userXOffset/userYOffset，nullable）+ elvis
    // `displayScale = userScale ?: fitScale`，测试面落在 C-M1 锁定文件（禁读/时序错开）。
    // 此处按契约 §2.3 公式不重算，仅以数据表锁「契约决策语义」；真正走渲染器的集成断言
    // 属 M5-A9（真机），本层无法以纯函数触达。建议 coder 在 InteractionMath.kt 同包提供
    // `data class InteractionState(...)` 或 `fun resolveOverride(...)` 具名纯函数，使本测试能
    // 对齐生产符号（当前为 spec 级 RED，未绑定生产符号）。——已在上报中如实标注。
    private fun resolveInteractiveScale(userScale: Float?, fitScale: Float): Float = userScale ?: fitScale

    private fun resolveInteractiveOffset(userOffset: Float?, fitOffset: Float): Float = userOffset ?: fitOffset

    @Test
    fun priority_overrideTakesPrecedenceOverFit() {
        val fit = CharacterFit(5.643f, 0.0003f, -0.122f)                 // C-M1 fit（bai 锁定）
        val user = Override(2.0f, 0.5f, 0.3f)                            // 用户拖拽/捏合结果

        // 有 override → 生效值取 override，不重算 fit（§2.3 时机1/2「有 override → 保持 override 绝对值」）
        assertEquals("有 override → displayScale 取 override", user.scale, resolveInteractiveScale(user.scale, fit.scale), 1e-6f)
        assertEquals("有 override → xOffset 取 override", user.xOffset, resolveInteractiveOffset(user.xOffset, fit.xOffset), 1e-6f)
        assertEquals("有 override → yOffset 取 override", user.yOffset, resolveInteractiveOffset(user.yOffset, fit.yOffset), 1e-6f)
    }

    @Test
    fun priority_noOverride_fitTakesEffect() {
        val fit = CharacterFit(5.643f, 0.0003f, -0.122f)                 // C-M1 fit（bai 锁定）

        // 无 override（null）→ fit 生效（§2.3 时机1「无 → fit（C-M1 路径）」）
        assertEquals("无 override → displayScale 取 fit", fit.scale, resolveInteractiveScale(null, fit.scale), 1e-6f)
        assertEquals("无 override → xOffset 取 fit", fit.xOffset, resolveInteractiveOffset(null, fit.xOffset), 1e-6f)
        assertEquals("无 override → yOffset 取 fit", fit.yOffset, resolveInteractiveOffset(null, fit.yOffset), 1e-6f)
    }

    @Test
    fun priority_fitDegenerate_defaultFallsBack() {
        // fit 退化 → C-M1 返回 CharacterFit(1,0,0)（默认）。override 与 fit 皆空 → 默认 (1,0,0)
        val degenerateFit = CharacterFit(1f, 0f, 0f)                     // C-M1 退化 = 现状（架构 §2.3/§3.1）
        assertEquals("fit 退化 → scale=1 默认", 1f, resolveInteractiveScale(null, degenerateFit.scale), 1e-6f)
        assertEquals("fit 退化 → xOffset=0 默认", 0f, resolveInteractiveOffset(null, degenerateFit.xOffset), 1e-6f)
        assertEquals("fit 退化 → yOffset=0 默认", 0f, resolveInteractiveOffset(null, degenerateFit.yOffset), 1e-6f)
    }
}
