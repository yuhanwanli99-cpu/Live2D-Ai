package com.live2dai.android.core

/**
 * 核心环路三态（契约 refactor-0814 §2.1，裁定 C-6=6A 整段播报前提）。
 *
 * 无交叠态：`IDLE`（空闲动画）→ `SENDING`（LLM 流式在途）→ `SPEAKING`（表情应用+TTS 播放+嘴型）。
 */
enum class LoopState {
    /** 空闲：无 LLM 请求在途、无播报在途。 */
    IDLE,

    /** LLM 请求在途（流式回复中）；至多一个请求（I1）。 */
    SENDING,

    /** TTS 播报在途（唯一入口 = Completed 转移，I2/I3）；表情已应用（播报期资源，I5）。 */
    SPEAKING,
}
