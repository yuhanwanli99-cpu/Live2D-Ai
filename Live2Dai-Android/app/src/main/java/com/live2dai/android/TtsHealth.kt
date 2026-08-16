package com.live2dai.android

/**
 * TTS 引擎健康状态（V1.2 模块1，契约见 docs/plans/plan-v12-plan.schema.json → dataModels.TtsHealth）。
 *
 * 供可观测性（模块7）与降级链决策使用；当前 Wave 仅维护状态值，后续 Wave 接遥测时消费。
 */
enum class TtsHealth {
    /** 引擎可用，上次调用成功 */
    OK,

    /** 引擎可用但性能下降或部分功能受限（如模型正在下载 / 网络抖动降级过一次） */
    DEGRADED,

    /** 引擎不可用，降级链自动跳过 */
    UNAVAILABLE,
}
