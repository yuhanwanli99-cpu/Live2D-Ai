package com.live2dai.android.render

/**
 * 嘴型引擎 sink（seam，纯 JVM）。
 * 对应官方 CubismLipSyncUpdater 输入（ParamMouthOpenY 驱动）的 JVM 形态。
 */
interface LipSyncEngineSink {
    fun setLipSync(value: Float)
}

/**
 * 嘴型 drain 桥（seam，纯 JVM）。
 *
 * - [drainAndPush]：clamp 到 [0,1] 后推给 [LipSyncEngineSink.setLipSync]；
 *   每帧至多推 1 次（帧内防重复：本帧已推过则后续调用忽略）；
 * - [onFrameEnd]：帧末复位，允许下一帧再次推送。
 */
class LipSyncDrainBridge(
    private val sink: LipSyncEngineSink,
) {

    private var pushedThisFrame = false

    /** clamp [0,1] 后推送；本帧已推送过则忽略（至多 1 次/帧）。NaN 防御（审计 R1）：视为 0f。 */
    fun drainAndPush(value: Float) {
        if (pushedThisFrame) return
        pushedThisFrame = true
        val safe = if (value.isNaN()) 0f else value
        sink.setLipSync(safe.coerceIn(0f, 1f))
    }

    /** 帧末复位，下一帧允许再次推送。 */
    fun onFrameEnd() {
        pushedThisFrame = false
    }
}
