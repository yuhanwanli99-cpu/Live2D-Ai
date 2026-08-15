package com.live2dai.android.data

// ============================================================
//  Wave 3 · 3-2 待机桥（A-3 裁定：C1 直写 / C2 合成 motion）
//
//  职责：桥接层每帧调用通道（producer），把 IdleMotionController（或任意
//  producer）的输出参数帧节流后经 sink 写入渲染层。不改 IdleMotionController
//  内部（回归门 RG1-A3 覆盖其既有测试全绿）。
//
//  节流语义（契约 3-2）：
//  - 每帧调 producer（onFrame 每次调用都取最新帧，取帧不节流）；
//  - 节流 throttleFrames 帧才经 sink 写一次（整帧界；首次调用强制写）；
//  - onFrame 返回本帧是否实际写了 sink。
//
//  纯 JVM（无 android.* / native 依赖），sink/producer 均为注入，可确定性单测。
// ============================================================

/** 参数写出目标（渲染层参数写入接缝；fun interface 便于 lambda 注入）。 */
fun interface IdleParamSink {
    /** 写入一帧参数（key = Live2D 参数名，value = 目标值）。 */
    fun write(params: Map<String, Float>)
}

/**
 * 待机桥配置。
 *
 * @param throttleFrames 帧节流间隔：每隔 throttleFrames 帧经 sink 写一次（≥1，≤0 按 1 防御）。
 * @param minIntervalMs 最小写间隔（ms）防御位：当前 [IdleBridge.onFrame] 签名仅携带
 *                      frameIndex（无时间戳），故帧节流为整帧界、本字段作为保留防御配置
 *                      由调用方在具备时间源的场景下自行校验（文档化，不参与帧判定）。
 */
data class IdleBridgeConfig(
    val throttleFrames: Int = 2,
    val minIntervalMs: Long = 30,
)

/**
 * 待机参数桥：每帧从 [producer] 取参数帧，按帧节流写入 [sink]。
 *
 * @param sink 参数写出目标。
 * @param config 节流配置。
 * @param producer 参数帧来源（如 `{ idleController.update(dt) -> toParams(it) }`）；
 *                 每次 [onFrame] / [forceFlush] 调用都会执行。
 */
class IdleBridge(
    private val sink: IdleParamSink,
    private val config: IdleBridgeConfig,
    private val producer: () -> Map<String, Float>,
) {

    private var writeCountVal: Long = 0L
    private var lastWriteFrameIndex: Long = Long.MIN_VALUE
    private val threshold: Long = config.throttleFrames.coerceAtLeast(1).toLong()

    /** 已写入 sink 的次数（含 [forceFlush]）。 */
    val writeCount: Long get() = writeCountVal

    /**
     * 每帧推进：调 producer 取最新参数帧；距上次写入 ≥ throttleFrames 帧
     * （或首次调用）时经 sink 写出。
     *
     * @param frameIndex 当前帧号（整帧界节流依据；非连续递增亦可，按间隔判定）。
     * @return 本帧是否实际写了 sink。
     */
    fun onFrame(frameIndex: Long): Boolean {
        val params = producer()
        val shouldWrite = lastWriteFrameIndex == Long.MIN_VALUE ||
            frameIndex - lastWriteFrameIndex >= threshold
        if (shouldWrite) {
            sink.write(params)
            writeCountVal++
            lastWriteFrameIndex = frameIndex
        }
        return shouldWrite
    }

    /**
     * 立即冲刷：producer + 写 sink（不受帧节流限制，帧节流窗口不变）。
     *
     * @return 本次写入的参数帧（producer 返回值）。
     */
    fun forceFlush(): Map<String, Float> {
        val params = producer()
        sink.write(params)
        writeCountVal++
        return params
    }
}
