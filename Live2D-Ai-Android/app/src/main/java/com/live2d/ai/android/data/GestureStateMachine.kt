package com.live2d.ai.android.data

import kotlin.math.sqrt

/**
 * 捏合/拖拽手势状态机（纯 Kotlin，无 Android 依赖；contract-R1-interaction §2.2）。
 *
 * 输入事件显式携带 pointerId；输出命令流由消费方（Live2DView → Live2DRenderer 公开接口）执行。
 *
 * ## 事件语义表（T2-R1-interaction-red §2.1 锁定，本实现逐条满足 TC1-TC8）
 *
 * | 事件 | 命令发射规则 |
 * |---|---|
 * | `Down(id,x,y)` | 全新手势起点（防御性复位旧状态）；记录首指；活动指针数 <2 → `[]` |
 * | `PointerDown(id,x,y)` | 加活动指；size==2 → `[BeginPinch(startDist=两实时指针欧氏距离)]` **恰一次** |
 * | `Move(pointers)` | size≥2 → `[ApplyPinch(实时欧氏距离)]`（**原始距离，本机不钳制**，钳制归 InteractionMath 只读消费）；size==1 → 单指拖拽规则（见下）；拖动 → hasAdjusted=true |
 * | `Up(id,x,y)` | 按 id 移除；size==0：hasAdjusted → `[SaveOverride]`，否则 → `[Tap]`；size==1 残留 → `[]`（续单指拖拽，锚点 = 剩余指针当前位置，无幽灵坐标） |
 * | `Cancel` | 全复位（isPinching/isDragging=false），**不发射 SaveOverride**；后续 Down 全新起点 |
 *
 * ## 单指拖拽规则
 * pressOrigin = 该指成为唯一活动指时的位置；首帧 Move 位移超 touchSlopPx → 从 pressOrigin 起
 * `[ApplyDrag(dx,dy)]`（全量，非帧差）；后续逐帧 `[ApplyDrag(帧差)]`；位移 ≤slop 的 Up → `[Tap]`。
 *
 * ## pointerId 锚定
 * 活动指针以 `Map<id, (x,y)>` 跟踪（LinkedHashMap 保插入序 = 落指序），计算距离时取的是
 * id 对应的实时坐标，禁止 `getX(0)/getX(1)` 索引漂移（TC4 乱序 id 防御）。
 *
 * ## 缩放钳制
 * [0.1, 5.0] 由消费方 InteractionMath.clampScale/pinchScale 只读消费（contract-R1-interaction §2.2），
 * 本状态机仅转发原始距离（TC6：ApplyPinch(10000)/ApplyPinch(1) 原样透传）。
 */
sealed class GestureEvent {
    /** 首指按下（全新手势起点）。 */
    data class Down(val id: Int, val x: Float, val y: Float) : GestureEvent()

    /** 额外手指按下（第二指起）。 */
    data class PointerDown(val id: Int, val x: Float, val y: Float) : GestureEvent()

    /** 移动：全量活动指针快照 id → (x, y)（视图坐标 px）。 */
    data class Move(val pointers: Map<Int, Pair<Float, Float>>) : GestureEvent()

    /** 某指抬起。 */
    data class Up(val id: Int, val x: Float, val y: Float) : GestureEvent()

    /** 手势取消（平台 CANCEL 或前景层抢断）。 */
    object Cancel : GestureEvent()
}

/** 输出命令流（消费方 = Live2DView → renderer 公开接口）。 */
sealed class GestureCommand {
    object Noop : GestureCommand()

    /** 捏合开始：捕获基准（第二指按下恰一次；startDistancePx = 两指实时欧氏距离）。 */
    data class BeginPinch(val startDistancePx: Float) : GestureCommand()

    /** 捏合进行：转发原始距离（钳制由 InteractionMath 只读消费）。 */
    data class ApplyPinch(val distancePx: Float) : GestureCommand()

    /** 单指拖拽帧位移（px，原样转发；renderer 内部经 dragPxToModel 换算）。 */
    data class ApplyDrag(val dxPx: Float, val dyPx: Float) : GestureCommand()

    /** 手势结束且发生了有效调整：消费方持久化 override（renderer.saveUserOverride）。 */
    object SaveOverride : GestureCommand()

    /** 阈值内 UP、未调整：消费方不消费（Compose 点击语义）。 */
    object Tap : GestureCommand()

    /** 手势取消/复位（Cancel 时发射；消费方仅复位内部状态，不触 renderer 状态重算）。 */
    object Reset : GestureCommand()
}

/**
 * 手势状态机。[touchSlopPx] 为平台 touch slop（Live2DView 注入 `ViewConfiguration.getScaledTouchSlop`）。
 *
 * 公开状态：[isPinching]（活动指针 ≥2）、[isDragging]（单指拖拽已激活）、[hasAdjusted]（本手势发生过有效调整）。
 */
class GestureStateMachine(private val touchSlopPx: Float) {

    private val activePointers = LinkedHashMap<Int, Pair<Float, Float>>()

    /** 单指拖拽锚点：该指成为唯一活动指时的位置。 */
    private var pressOriginX = 0f
    private var pressOriginY = 0f

    /** 上一帧拖拽位置（帧差基准）。 */
    private var lastDragX = 0f
    private var lastDragY = 0f

    private var dragging = false
    private var adjusted = false

    /** 双指（及以上）激活：捏合模式。 */
    val isPinching: Boolean get() = activePointers.size >= 2

    /** 单指拖拽已激活（捏合模式下为 false）。 */
    val isDragging: Boolean get() = dragging && activePointers.size == 1

    /** 本手势发生过有效调整（拖拽超 slop 或捏合已应用）；决定 Up 时是否 SaveOverride。 */
    val hasAdjusted: Boolean get() = adjusted

    /** 处理一个输入事件，返回命令流（空列表 = 无动作）。 */
    fun onEvent(e: GestureEvent): List<GestureCommand> = when (e) {
        is GestureEvent.Down -> onDown(e)
        is GestureEvent.PointerDown -> onPointerDown(e)
        is GestureEvent.Move -> onMove(e)
        is GestureEvent.Up -> onUp(e)
        is GestureEvent.Cancel -> {
            reset()
            listOf(GestureCommand.Reset)
        }
    }

    private fun onDown(e: GestureEvent.Down): List<GestureCommand> {
        // Down 是 Android 手势的必然起点：防御性复位旧状态，保证「全新起点」语义（TC7 后续 Down）。
        if (activePointers.isNotEmpty()) reset()
        activePointers[e.id] = e.x to e.y
        pressOriginX = e.x
        pressOriginY = e.y
        lastDragX = e.x
        lastDragY = e.y
        return emptyList()
    }

    private fun onPointerDown(e: GestureEvent.PointerDown): List<GestureCommand> {
        activePointers[e.id] = e.x to e.y
        if (activePointers.size == 2) {
            // 第二指按下 → BeginPinch 恰一次；退出单指拖拽转入捏合模式
            dragging = false
            val (a, b) = firstTwoPositions() ?: return emptyList()
            return listOf(GestureCommand.BeginPinch(distance(a, b)))
        }
        return emptyList()
    }

    private fun onMove(e: GestureEvent.Move): List<GestureCommand> {
        val pointers = e.pointers
        if (pointers.isEmpty()) return emptyList()
        // 全量快照 → 更新（含防御性补录未知指针）活动指针实时位置（pointerId 锚定）
        for ((id, pos) in pointers) {
            activePointers[id] = pos
        }
        return if (pointers.size >= 2) {
            // 捏合模式：转发原始欧氏距离，不钳制（TC6）；退出单指拖拽
            dragging = false
            adjusted = true
            val (a, b) = firstTwoPositions() ?: return emptyList()
            listOf(GestureCommand.ApplyPinch(distance(a, b)))
        } else {
            onSinglePointerMove(pointers.entries.first())
        }
    }

    private fun onSinglePointerMove(entry: Map.Entry<Int, Pair<Float, Float>>): List<GestureCommand> {
        val (x, y) = entry.value
        if (!dragging) {
            val dx = x - pressOriginX
            val dy = y - pressOriginY
            if (dx * dx + dy * dy > touchSlopPx * touchSlopPx) {
                // 首帧超 slop：从 pressOrigin 起发全量位移（非帧差）
                dragging = true
                adjusted = true
                lastDragX = x
                lastDragY = y
                return listOf(GestureCommand.ApplyDrag(dx, dy))
            }
            return emptyList()
        }
        val dx = x - lastDragX
        val dy = y - lastDragY
        lastDragX = x
        lastDragY = y
        if (dx != 0f || dy != 0f) {
            return listOf(GestureCommand.ApplyDrag(dx, dy))
        }
        return emptyList()
    }

    private fun onUp(e: GestureEvent.Up): List<GestureCommand> {
        if (!activePointers.containsKey(e.id)) return emptyList()
        activePointers.remove(e.id)
        return when {
            activePointers.isEmpty() -> {
                dragging = false
                val save = adjusted
                adjusted = false
                pressOriginX = 0f
                pressOriginY = 0f
                lastDragX = 0f
                lastDragY = 0f
                if (save) listOf(GestureCommand.SaveOverride) else listOf(GestureCommand.Tap)
            }
            activePointers.size == 1 -> {
                // 残留单指：续单指拖拽，锚点 = 剩余指针实时位置（TC5 无幽灵坐标）
                dragging = false
                val (x, y) = activePointers.values.first()
                pressOriginX = x
                pressOriginY = y
                lastDragX = x
                lastDragY = y
                emptyList()
            }
            else -> emptyList()
        }
    }

    private fun firstTwoPositions(): Pair<Pair<Float, Float>, Pair<Float, Float>>? {
        val it = activePointers.entries.iterator()
        if (!it.hasNext()) return null
        val a = it.next().value
        if (!it.hasNext()) return null
        val b = it.next().value
        return a to b
    }

    private fun distance(a: Pair<Float, Float>, b: Pair<Float, Float>): Float {
        val dx = a.first - b.first
        val dy = a.second - b.second
        return sqrt(dx * dx + dy * dy)
    }

    private fun reset() {
        activePointers.clear()
        dragging = false
        adjusted = false
        pressOriginX = 0f
        pressOriginY = 0f
        lastDragX = 0f
        lastDragY = 0f
    }
}
