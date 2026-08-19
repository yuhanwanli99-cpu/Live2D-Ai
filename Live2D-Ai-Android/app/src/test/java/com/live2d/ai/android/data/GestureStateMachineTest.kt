package com.live2d.ai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 捏合缩放手势状态机纯函数断言（契约 R1-interaction §2.2/§3，GS-A1~A6）。
 *
 * 覆盖生产文件 `data/GestureStateMachine.kt`（**新增，RED 待实现**，纯 Kotlin 无 Android 依赖，JVM 可单测）：
 *   sealed class GestureEvent {
 *       data class Down(val id: Int, val x: Float, val y: Float)
 *       data class PointerDown(val id: Int, val x: Float, val y: Float)
 *       data class Move(val pointers: Map<Int, Pair<Float, Float>>)   // id → (x, y) 视图坐标 px
 *       data class Up(val id: Int, val x: Float, val y: Float)
 *       object Cancel
 *   }
 *   sealed class GestureCommand {
 *       object Noop
 *       data class BeginPinch(val startDistancePx: Float)
 *       data class ApplyPinch(val distancePx: Float)
 *       data class ApplyDrag(val dxPx: Float, val dyPx: Float)
 *       object SaveOverride
 *       object Tap
 *       object Reset
 *   }
 *   class GestureStateMachine(private val touchSlopPx: Float) {
 *       fun onEvent(e: GestureEvent): List<GestureCommand>
 *       val isPinching: Boolean; val isDragging: Boolean; val hasAdjusted: Boolean
 *   }
 *
 * 行为契约（对齐 PC interaction.ts 语义，契约 §2.2）：
 *   - 双指激活：activePointers.size ≥ 2 → 以两个活动 pointer 的「实时坐标」算距离（**pointerId 锚定**，
 *     禁止 getX(0)/getX(1) 索引漂移）；BeginPinch(startDist) 恰在第二指按下时发出一次。
 *   - 单指拖拽：DOWN 后首 MOVE 位移 > touchSlopPx → ApplyDrag；位移 ≤ slop 的 UP → Tap。
 *   - 缩放钳制 [0.1,5.0] 由 InteractionMath.clampScale（**只读消费**）承担，状态机「不重复钳制」——
 *     状态机仅以 px 距离转发 ApplyPinch(currentDist)。
 *   - 任意 Up 后单指残留 → 续单指拖拽（起点 = 剩余 pointer 当前位置，无幽灵坐标）；再 PointerDown → 重新 BeginPinch（新 startDist）。
 *   - Cancel → 全复位（丢弃手势，不 SaveOverride）；仅 hasAdjusted 为真才发 SaveOverride。
 *
 * 依据全部取自契约 R1-interaction §2.2/§3 验收断言表（GS-A1~A6）。
 * 禁止读实现源码（app/src/main 下 Live2DView/Live2DRenderer/GestureStateMachine 禁读）。
 *
 * 说明：本测试为「契约级 RED」，按契约 §2.2 具名封装书写。生产符号 GestureStateMachine 尚不存在，
 * 编译期即 RED（属预期，契约 GS-A1~A7 待 coder 实现 GREEN）。各用例最终命令流由 coder 实现对齐本
 * 文件锁定的 spec 级断言。
 */
class GestureStateMachineTest {

    private val slop = 10f

    // ═══════════════════════════════════════════════════════════
    // GS-A1：单指拖拽 + tap（不超 slop）
    // ═══════════════════════════════════════════════════════════

    /** GS-A1-①：Down→Move(超 slop)→Move→Up → [ApplyDrag×n, SaveOverride]（末次 Up 收尾 SaveOverride）。 */
    @Test
    fun singleDrag_overSlop_emitsApplyDragThenSaveOverrideOnUp() {
        val sm = GestureStateMachine(slop)

        assertEquals("首指 Down 不产生命令", emptyList<GestureCommand>(), sm.onEvent(GestureEvent.Down(0, 10f, 10f)))

        // 位移 = 0（原地）< slop → 无命令（tap 挂起）
        assertEquals(
            "超 slop 前小位移 MOVE 不产生命令",
            emptyList<GestureCommand>(),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (10f to 10f)))),
        )

        // 位移累计 = (30,30)-(10,10) = (20,20)，幅值 ~28.3 > slop(10) → 进入拖拽，首帧增量从 pressOrigin 起
        assertEquals(
            "首帧超 slop MOVE → ApplyDrag(dx=20,dy=20)",
            listOf<GestureCommand>(GestureCommand.ApplyDrag(20f, 20f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (30f to 30f)))),
        )

        // 后续 Move → 逐帧增量（40-10=30）
        assertEquals(
            "第二帧 MOVE → ApplyDrag(dx=20,dy=20)（30→50）",
            listOf<GestureCommand>(GestureCommand.ApplyDrag(20f, 20f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (50f to 50f)))),
        )

        assertTrue("已超 slop → isDragging 为真", sm.isDragging)
        assertTrue("发生拖拽调整 → hasAdjusted 为真", sm.hasAdjusted)
        assertFalse("单指模式 isPinching 应为假", sm.isPinching)

        // 末次 Up（唯一手指抬起）→ 已调整 → SaveOverride
        assertEquals(
            "拖拽收尾 UP → SaveOverride",
            listOf<GestureCommand>(GestureCommand.SaveOverride),
            sm.onEvent(GestureEvent.Up(0, 50f, 50f)),
        )
    }

    /** GS-A1-②：位移未超 slop 的 UP → [Tap]（无 SaveOverride）。 */
    @Test
    fun singleTap_withinSlop_emitsTapNoSaveOverride() {
        val sm = GestureStateMachine(slop)

        sm.onEvent(GestureEvent.Down(0, 10f, 10f))
        assertEquals(
            "位移 ≤ slop 的 MOVE 不产生命令",
            emptyList<GestureCommand>(),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (14f to 12f)))),
        )
        // 位移 (4,2) 幅值 ~4.5 ≤ slop(10) → UP = Tap
        assertEquals(
            "超 slop 前置 UP → Tap（不含 SaveOverride）",
            listOf<GestureCommand>(GestureCommand.Tap),
            sm.onEvent(GestureEvent.Up(0, 14f, 12f)),
        )
        assertFalse("未调整 → hasAdjusted 为假", sm.hasAdjusted)
        assertFalse("未拖拽 → isDragging 为假", sm.isDragging)
    }

    // ═══════════════════════════════════════════════════════════
    // GS-A2：捏合激活（BeginPinch 恰一次）+ ApplyPinch 距离 = 两 pointerId 实时欧氏距离 + id 锚定
    // ═══════════════════════════════════════════════════════════

    /** GS-A2-①：Down(id0)→PointerDown(id1) → [BeginPinch(startDist)] 恰一次；Move(双指) → [ApplyPinch(实时距离)]。 */
    @Test
    fun pinchActivation_beginPinchOnceThenApplyPinch() {
        val sm = GestureStateMachine(slop)

        assertEquals("首指 Down 无命令", emptyList<GestureCommand>(), sm.onEvent(GestureEvent.Down(0, 0f, 0f)))

        // 第二指按下 → 两活动 pointer (0,0)+(3,4)，距离=5 → BeginPinch(5) 恰一次
        assertEquals(
            "第二指按下 → BeginPinch(startDist=5) 恰一次",
            listOf<GestureCommand>(GestureCommand.BeginPinch(5f)),
            sm.onEvent(GestureEvent.PointerDown(1, 3f, 4f)),
        )
        assertTrue("双指激活 → isPinching 为真", sm.isPinching)

        // 双指 Move：实时坐标 (0,0)+(6,8) → 距离=10 → ApplyPinch(10)
        assertEquals(
            "双指 MOVE → ApplyPinch(当前欧氏距离=10)",
            listOf<GestureCommand>(GestureCommand.ApplyPinch(10f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (6f to 8f)))),
        )
        // (0,0)+(9,12) → 距离=15 → ApplyPinch(15)
        assertEquals(
            "双指 MOVE → ApplyPinch(实时距离=15)",
            listOf<GestureCommand>(GestureCommand.ApplyPinch(15f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (9f to 12f)))),
        )
    }

    /** GS-A2-②：乱序 pointerId（id=3 先落、id=7 后落）验证距离按 pointerId 锚定、非索引位置。 */
    @Test
    fun pinchActivation_pointerIdAnchored_outOfOrderIds() {
        val sm = GestureStateMachine(slop)

        sm.onEvent(GestureEvent.Down(3, 0f, 0f))
        // 第二指 id=7（非 1）→ 距离按 id3(0,0)+id7(3,4)=5 计算，与「索引」无关
        assertEquals(
            "pointerId 锚定：Down(3)→PointerDown(7) → BeginPinch(5)（与索引编号无关）",
            listOf<GestureCommand>(GestureCommand.BeginPinch(5f)),
            sm.onEvent(GestureEvent.PointerDown(7, 3f, 4f)),
        )
        // 双指 Move 乱序 map（key=7 在前）→ 仍按 id 锚定得 (0,0)+(0,8) 距离=8
        assertEquals(
            "乱序 map：id7(0,8)+id3(0,0) → ApplyPinch(8)",
            listOf<GestureCommand>(GestureCommand.ApplyPinch(8f)),
            sm.onEvent(GestureEvent.Move(mapOf(7 to (0f to 8f), 3 to (0f to 0f)))),
        )
    }

    // ═══════════════════════════════════════════════════════════
    // GS-A3：索引漂移防御（Up 残留单指续拖拽 + 重新 BeginPinch 新 startDist）
    // ═══════════════════════════════════════════════════════════

    /** GS-A3：Move 期间 Up(id0) 残留 id1 → 续单指拖拽（起点=id1 当前位置，无幽灵坐标）；再 PointerDown(id2) → 重新 BeginPinch（新 startDist）。 */
    @Test
    fun pinchToOneRemainingPointer_dragContinuesFromRealIdPosition_noPhantom() {
        val sm = GestureStateMachine(slop)

        sm.onEvent(GestureEvent.Down(0, 0f, 0f))                 // id0 首指
        assertEquals(
            listOf<GestureCommand>(GestureCommand.BeginPinch(100f)),
            sm.onEvent(GestureEvent.PointerDown(1, 0f, 100f)),   // id1 第二指：距离 100
        )
        assertEquals(
            listOf<GestureCommand>(GestureCommand.ApplyPinch(200f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (0f to 200f)))),
        )

        // Up(id0) → 残留 id1（唯一活动指）；续单指拖拽起点锚定 = id1 当前位置 (0,200)（禁止 getX(0) 幽灵坐标）
        // 命令流：手势未结束（尚有 id1），不 SaveOverride；此时 isPinching 应转假、isDragging 待下一帧超 slop 为真
        val afterUp0 = sm.onEvent(GestureEvent.Up(0, 0f, 0f))
        assertEquals("Up(id0) 残留 id1 → 不 SaveOverride，不产生调整命令", emptyList<GestureCommand>(), afterUp0)
        assertFalse("Up(id0) 后仅一指 → isPinching 为假", sm.isPinching)

        // 续单指拖拽：id1 从 (0,200) → (0,300)，位移 100 > slop → ApplyDrag(dx=0,dy=100)（基于 id1 真实坐标）
        assertEquals(
            "续单指拖拽起点=id1 当前位置(0,200)，移动 100px → ApplyDrag(0,100)（无 getX(0) 幽灵）",
            listOf<GestureCommand>(GestureCommand.ApplyDrag(0f, 100f)),
            sm.onEvent(GestureEvent.Move(mapOf(1 to (0f to 300f)))),
        )

        // 再 PointerDown(id2) → 重新 BeginPinch（新 startDist = id1(0,300)+id2(60,300) 距离=60，非旧 100）
        assertEquals(
            "再落第二指 id2 → 重新 BeginPinch(新 startDist=60)",
            listOf<GestureCommand>(GestureCommand.BeginPinch(60f)),
            sm.onEvent(GestureEvent.PointerDown(2, 60f, 300f)),
        )
        assertTrue("重入捏合 → isPinching 恢复为真", sm.isPinching)
    }

    // ═══════════════════════════════════════════════════════════
    // GS-A4：钳制命中（状态机不重复钳制，转发原始距离；renderer 侧 pinchScale 消费后 ∈[0.1,5.0]）
    // ═══════════════════════════════════════════════════════════

    /** GS-A4：状态机仅转发原始 ApplyPinch(当前距离)（不钳制）；将该距离喂 InteractionMath.pinchScale → scale ∈[0.1,5.0]。 */
    @Test
    fun pinchApply_barelyClampedByInteractionMathConsumption() {
        val sm = GestureStateMachine(slop)

        // 起点距离 100 → 双指 MOVE 极度放大到当前距离 10000（原始，不钳制）
        sm.onEvent(GestureEvent.Down(0, 0f, 0f))
        sm.onEvent(GestureEvent.PointerDown(1, 0f, 100f))
        assertEquals(
            "状态机转发原始 ApplyPinch(10000)（自身不钳制）",
            listOf<GestureCommand>(GestureCommand.ApplyPinch(10000f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (0f to 10000f)))),
        )

        // verify InteractionMath.pinchScale（只读消费）对上界钳制命中 [0.1,5.0]
        val startDist = 100f
        assertTrue(
            "pinchScale(1.0,start=100,current=10000) 应 ≤ 5.0",
            pinchScale(1.0f, startDist, 10000f) <= 5.0f,
        )
        assertTrue(
            "pinchScale(1.0,start=100,current=10000) 应 ≥ 0.1",
            pinchScale(1.0f, startDist, 10000f) >= 0.1f,
        )

        // 反向：极度捏合到当前距离 1（原始，不钳制）
        assertEquals(
            "状态机转发原始 ApplyPinch(1)（自身不钳制）",
            listOf<GestureCommand>(GestureCommand.ApplyPinch(1f)),
            sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (0f to 1f)))),
        )
        assertTrue(
            "pinchScale(1.0,start=100,current=1) 应 ≥ 0.1（下钳命中）",
            pinchScale(1.0f, startDist, 1f) >= 0.1f,
        )
    }

    // ═══════════════════════════════════════════════════════════
    // GS-A5：Cancel 全复位
    // ═══════════════════════════════════════════════════════════

    /** GS-A5：任意阶段 Cancel → 全复位（isPinching/isDragging=false、不 SaveOverride、后续 Down 全新起点）。 */
    @Test
    fun cancel_resetsAllState_noSaveOverride_freshGestureAfter() {
        val sm = GestureStateMachine(slop)

        // 先进入捏合调整状态
        sm.onEvent(GestureEvent.Down(0, 0f, 0f))
        sm.onEvent(GestureEvent.PointerDown(1, 0f, 100f))
        sm.onEvent(GestureEvent.Move(mapOf(0 to (0f to 0f), 1 to (0f to 250f))))
        assertTrue("Cancel 前 isPinching 为真", sm.isPinching)

        val cancelCmds = sm.onEvent(GestureEvent.Cancel)
        assertFalse(
            "Cancel 命令流不得含 SaveOverride（丢弃手势不持久化）",
            cancelCmds.any { it is GestureCommand.SaveOverride },
        )
        assertFalse("Cancel 后 isPinching=false", sm.isPinching)
        assertFalse("Cancel 后 isDragging=false", sm.isDragging)

        // 后续 Down 全新起点：重开捏合应重新 BeginPinch（非沿用旧 startDist）
        sm.onEvent(GestureEvent.Down(0, 5f, 5f))
        assertEquals(
            "Cancel 后新手势 Down→PointerDown 重新 BeginPinch(新 startDist=3)",
            listOf<GestureCommand>(GestureCommand.BeginPinch(3f)),
            sm.onEvent(GestureEvent.PointerDown(1, 8f, 5f)),
        )
    }

    // ═══════════════════════════════════════════════════════════
    // GS-A6：tap 不消费语义（UP ≤ slop 不向 renderer 发任何状态调整）
    // ═══════════════════════════════════════════════════════════

    /** GS-A6：UP ≤ slop 的 Tap 命令 → 消费方返回 false（不 queueEvent renderer 状态）。纯函数面断言 Tap 命令 = 唯一输出且无调整命令。 */
    @Test
    fun tap_emitsOnlyTapCommand_noAdjustmentNoSave() {
        val sm = GestureStateMachine(slop)

        sm.onEvent(GestureEvent.Down(0, 100f, 100f))
        sm.onEvent(GestureEvent.Move(mapOf(0 to (105f to 100f))))  // 位移 5 ≤ slop(10)

        val upCmds = sm.onEvent(GestureEvent.Up(0, 105f, 100f))
        assertEquals(
            "UP ≤ slop → 唯一命令 Tap（无 ApplyDrag/ApplyPinch/SaveOverride）",
            listOf<GestureCommand>(GestureCommand.Tap),
            upCmds,
        )
        // 契约 GS-A6：Live2DView 侧收到 Tap → 消费返回值 false（本层纯函数只保证不产出调整命令）
        // 接线断言（renderer 零调用）由 Live2DView 真实接线测试/E2E 兜底；此处锁纯函数输出面。
    }
}
