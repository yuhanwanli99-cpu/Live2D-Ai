package com.live2dai.android

/**
 * 跨线程参数写串行化通道（M-AND-3 / AND-L5，选型 4-A queueEvent 归队）。
 *
 * 解决的问题（AND-L5）：Main 线程 LipSync 协程直写 native 模型参数
 * （`model.setParameterValue` → JNI 直写 float 数组）与 GL 线程渲染管线
 * （`pipeline.update` 同写参数）并发 → 无锁原生竞态/撕裂。
 *
 * 本通道把「参数提交」与「native 写」解耦，归入 GL 线程（手势路径 queueEvent
 * 同款先例，Live2DView.kt 483-585）：
 * - **producer**（任意线程，含 Main 嘴型协程）调用 [submit]：只写入 @Volatile pending
 *   表（每参数仅保留最新值，覆盖语义、无积压增长），**不触发任何 native 写**
 *   （AND3-A2：producer 线程 submit 期间 native 写计数 = 0）。
 * - **consumer**（仅 GL 线程调用，Live2DRenderer.onDrawFrame 帧首）调用 [drain]：
 *   一次性取出 pending 最新值，逐参数回调 [onApply] —— native 写唯一发生点，
 *   回调线程 == GL/渲染线程（AND3-A2）。
 *
 * 时序（AND3-A4）：submit → 下一次 drain 帧首应用（延迟 ≤ 1 帧；120fps ≈ 8ms，
 * 100ms 口型节拍下无感）；drain 前不应用。
 *
 * 生命周期（AND3-A3）：onPause/解绑/release 后 GL 线程不再产生帧 → 不再 drain →
 * 未 drain 的 pending 不再触发 native 写（解绑后 onApply 计数 = 0）。
 *
 * 线程模型：pending 引用以 @Volatile 发布，生产者单协程串行提交；
 * drain 属消费侧（契约硬约束：仅 GL 线程），不与 submit 并发竞争同一槽。
 *
 * @see Live2DView.setLipSyncParam（producer 入口）
 * @see Live2DRenderer.onDrawFrame（consumer 帧首 drain）
 */
class LipSyncParamChannel {

    /** pending 参数表：name → 最新值（覆盖语义）。@Volatile 发布保证生产者写入对 GL 线程可见。 */
    @Volatile
    private var pending: Map<String, Float> = emptyMap()

    /**
     * producer 提交：任意线程调用。
     * 仅更新 pending 表（每参数仅保留最新值，无积压增长），不触发 native 写。
     *
     * @param name 参数标识（生产路径使用 [MOUTH_VALUE_KEY]；测试可传任意键）
     * @param value 目标值（不做任何 clamp/变换，原样入表，由消费侧按需处理）
     */
    fun submit(name: String, value: Float) {
        pending = pending + (name to value)
    }

    /**
     * consumer 取出并应用：仅 GL 线程调用（契约硬约束）。
     * 把当前 pending 全量快照取出并清空，逐参数回调 [onApply]（每参数仅一次、仅最新值）；
     * 无 pending 时 [onApply] 不被调用。
     *
     * @param onApply native 写回调（在调用方线程 = GL/渲染线程执行）
     */
    fun drain(onApply: (String, Float) -> Unit) {
        val snapshot = pending
        if (snapshot.isEmpty()) return
        pending = emptyMap()
        for ((name, value) in snapshot) {
            onApply(name, value)
        }
    }

    companion object {
        /**
         * [Live2DView.setLipSyncParam] 提交使用的逻辑键：一个口型开合值批量写全部口型参数
         * （ParamA + ParamMouthOpenY + meta 扩展 mouthOpenIndices，见 AnimationPipeline.setLipSyncValue）。
         * 消费侧（Live2DRenderer drain onApply）按此键路由到批量写。
         */
        const val MOUTH_VALUE_KEY = "MouthOpenValue"
    }
}
