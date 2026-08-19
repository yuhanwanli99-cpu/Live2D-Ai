package com.live2d.ai.android.core

/**
 * 视图宿主接缝（纯 JVM，无 Android 依赖——JVM 可测硬约束；契约 §2.2「视图接线」）。
 *
 * 由 [LoopCoordinator.attachView] 以**替换语义**绑定（新引用覆盖旧引用，绝不追加——I6，
 * 双 hook 累积根因的反转）。实现归 2C-3/A 片；本文件仅声明接缝签名。
 *
 * 全部方法带空默认实现：宿主未接/假宿主不实现时零行为（I6 接线幂等 + 替换语义不受影响）。
 * 契约 §2.2 Live2DHost 完整面（initialize/onResume/onPause/dispose/attachMouthChannel）为 A 片职责，
 * 归 2C-3 汇合接线，不在本接缝重复声明。
 */
interface LoopViewHost {
    /** 绑定回调（attachView 替换后对新宿主调用一次；同一引用重复 attach 为 no-op）。 */
    fun onAttach() {}

    /** 解绑回调（宿主被替换/解除绑定时调用一次）。 */
    fun onDetach() {}

    /** 嘴型参数归零（I4 执行落点：宿主经 A 片嘴型通道写 0）。 */
    fun zeroMouth() {}

    /** 环路状态变更回调（默认空实现；宿主可观察 IDLE/SENDING/SPEAKING）。 */
    fun onState(state: LoopState) {}
}
