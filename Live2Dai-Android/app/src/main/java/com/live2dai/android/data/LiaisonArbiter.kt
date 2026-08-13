package com.live2dai.android.data

// ============================================================
//  R17 统一优先级仲裁器（LiaisonArbiter）：单入口管理所有联动源
//
//  契约: .pi/spoq/T8-benchmark-red.md §三 + LiaisonArbiterTest.kt 文件头
//  （真值表 12 条机械规则，tester 已锁定）
//
//  优先级（需求实锤）: 情绪姿态动作(emotion) > 说话(speaking) > 表情(expression) > idle
//
//  通道划分（tester 裁定）:
//  - BODY_HEAD 通道（身体角 + 头角）候选源按优先级: emotion(3) > speaking(2) > idle(1)
//    （expression 6 键为 brow/eye/mouth，不驱动 body/head 角 → 不参赛，见 R13）
//  - MOUTH 通道（口型参数）候选源按优先级: speaking(2) > expression(1)
//    （emotion 姿态帧不驱动口型，不参赛）
//
//  yield 语义 = 参与某通道且在该通道落败的活跃源集合（部分让位亦计入；
//  仍驱动通道由 drives 表达——如 emotion+speaking: speaking body/head 让位但 mouth 仍驱动）。
//
//  纯 JVM object（无 native / android.* 依赖），可确定性单测。
//  接入点: AnimationPipeline.update 每帧收集 4 源激活状态 → arbitrate → 各落点按 drives 执行。
// ============================================================

/**
 * R17 统一联动仲裁器（emotion > speaking > expression > idle）。
 */
object LiaisonArbiter {

    /** 联动源（声明序 = 全局优先级降序） */
    enum class Source { EMOTION, SPEAKING, EXPRESSION, IDLE }

    /** 参数通道 */
    enum class Channel { BODY_HEAD, MOUTH }

    /** 各联动源激活状态输入（全默认 false） */
    data class Sources(
        val emotionActive: Boolean = false,
        val speakingActive: Boolean = false,
        val expressionActive: Boolean = false,
        val idleActive: Boolean = false,
    )

    /**
     * 仲裁结果:
     * - [winner]: 全局最高优先级活跃源（emotion>speaking>expression>idle；全 inactive → null）
     * - [drives]: 各通道由谁驱动（无驱动则不含 key）
     * - [yieldSources]: 在参与通道上被高优先级源让位的所有活跃源（至少让位 1 通道）
     */
    data class ArbitrationResult(
        val winner: Source?,
        val drives: Map<Channel, Source>,
        val yieldSources: Set<Source>,
    )

    /** 全局优先级序（首个 active = winner） */
    private val WINNER_ORDER = listOf(Source.EMOTION, Source.SPEAKING, Source.EXPRESSION, Source.IDLE)

    /** BODY_HEAD 通道候选序（expression 6 键 = 眉/眼/嘴，不驱动 body/head 角 → 不参赛） */
    private val BODY_HEAD_ORDER = listOf(Source.EMOTION, Source.SPEAKING, Source.IDLE)

    /** MOUTH 通道候选序（emotion 姿态帧不驱动口型 → 不参赛） */
    private val MOUTH_ORDER = listOf(Source.SPEAKING, Source.EXPRESSION)

    private fun isActive(source: Source, s: Sources): Boolean = when (source) {
        Source.EMOTION -> s.emotionActive
        Source.SPEAKING -> s.speakingActive
        Source.EXPRESSION -> s.expressionActive
        Source.IDLE -> s.idleActive
    }

    /**
     * 统一仲裁（机械规则，真值表 12 条全部覆盖）:
     * 1. winner = [WINNER_ORDER] 首个 active；全 false → null。
     * 2. BODY_HEAD = [BODY_HEAD_ORDER] 首个 active。
     * 3. MOUTH = [MOUTH_ORDER] 首个 active。
     * 4. yieldSources = 参与某通道且在该通道落败的活跃源集合:
     *    (a) BODY_HEAD: emotion active → speaking 与 idle 均让位；
     *                   speaking active 且 emotion 否 → idle 让位；
     *    (b) MOUTH: speaking active → expression 让位。
     */
    fun arbitrate(sources: Sources): ArbitrationResult {
        val winner = WINNER_ORDER.firstOrNull { isActive(it, sources) }

        val bodyHead = BODY_HEAD_ORDER.firstOrNull { isActive(it, sources) }
        val mouth = MOUTH_ORDER.firstOrNull { isActive(it, sources) }

        val drives = buildMap {
            if (bodyHead != null) put(Channel.BODY_HEAD, bodyHead)
            if (mouth != null) put(Channel.MOUTH, mouth)
        }

        val yields = mutableSetOf<Source>()
        // (a) BODY_HEAD 通道落败方
        if (sources.emotionActive) {
            if (sources.speakingActive) yields += Source.SPEAKING
            if (sources.idleActive) yields += Source.IDLE
        } else if (sources.speakingActive) {
            if (sources.idleActive) yields += Source.IDLE
        }
        // (b) MOUTH 通道落败方（speaking 在 emotion active 时 body/head 让位、但 mouth 仍驱动）
        if (sources.speakingActive && sources.expressionActive) yields += Source.EXPRESSION

        return ArbitrationResult(winner, drives, yields)
    }
}
