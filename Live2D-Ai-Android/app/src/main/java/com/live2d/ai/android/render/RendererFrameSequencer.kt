package com.live2d.ai.android.render

/**
 * 渲染帧步序器（seam，纯 JVM）：严格执行 参数 drain/写入 → update(dt) → draw()。
 *
 * 帧序契约（contract-refactor-0814 §2.3-④）：参数写入先于更新、更新先于绘制。
 * - [FrameSinks]：参数表（外部线程写入 pending，帧首 [frame] drain 取走并清空）；
 * - [onUpdate]/[onDraw]：可注入的帧步钩子（默认 no-op）。
 *
 * 步序可观测性（供测试断言）：测试可覆写 [FrameSinks.drain] 记录 drain 步，
 * 并通过 [onUpdate]/[onDraw] 记录 update/draw 步，断言顺序 drain → update → draw。
 * 引擎渲染接线（下一步骤）将注入真实 update/draw 实现。
 */
class RendererFrameSequencer(
    /** 模型更新步钩子（dt 秒，默认 no-op）。 */
    var onUpdate: (dt: Float) -> Unit = {},
    /** 绘制步钩子（默认 no-op）。 */
    var onDraw: () -> Unit = {}
) {

    /** 执行一帧：drain(参数表) → update(dt) → draw()。 */
    fun frame(dt: Float, sinks: FrameSinks) {
        sinks.drain()
        onUpdate(dt)
        onDraw()
    }
}
