package com.live2dai.android.core

import android.util.Log
import com.live2dai.android.Live2DView
import com.live2dai.android.data.EmotionTagTable

/**
 * 视觉表情落点装饰器（Wave 4.1 修复 2：抽审 FAIL「apply/release 不触 Live2DView」的整改）。
 *
 * 在既有 [ExpressionSink]（状态机语义：apply 记录 + 收敛）之上叠加**视觉落点**：
 * 把 apply/release 转发到 [delegate]（状态机语义不变），同时经 [viewProvider]
 * 取当前 Live2DView 引用，调用其公开表情入口 [Live2DView.setParametricExpression]
 * （只调用、不改 Live2DView/Live2DRenderer），使表情真正作用于模型渲染。
 *
 * - [apply]：`emotion` → [EmotionTagTable.frameFor] 参数帧（bai 6 键域，未知情绪
 *   回退 neutral；null = neutral，对齐 [ExpressionSink] 接口文档）→ setParametricExpression(frame, 300ms)；
 * - [release]：复位 neutral 帧（fade 500ms，对齐 EmotionController emotionToNeutral 默认）。
 *
 * viewProvider 懒解析（onLive2DViewWired 之前调用 → null → 仅转发 delegate，视觉静默）。
 */
class VisualExpressionSink(
    private val delegate: ExpressionSink,
    private val viewProvider: () -> Live2DView?,
) : ExpressionSink {

    companion object {
        private const val TAG = "VisualExpressionSink"

        /** 表情淡入（ms）：对齐 EmotionController emotionSwitchFadeMs 默认 300（契约 H 段）。 */
        private const val EMOTION_SWITCH_FADE_MS = 300

        /** 回 neutral 淡入（ms）：对齐 EmotionController emotionToNeutralFadeMs 默认 500（契约 H 段）。 */
        private const val NEUTRAL_FADE_MS = 500
    }

    /** 应用表情：状态机语义转发 + 视觉落点（Live2DView.setParametricExpression）。 */
    override fun apply(emotion: String?) {
        delegate.apply(emotion)
        val view = viewProvider() ?: return
        val frame = EmotionTagTable.frameFor(emotion, fallbackNeutral = true)
        Log.i(TAG, "apply: emotion=$emotion → 参数帧 ${frame.size} 键（setParametricExpression, fade=${EMOTION_SWITCH_FADE_MS}ms）")
        view.setParametricExpression(frame, EMOTION_SWITCH_FADE_MS)
    }

    /** 复位 neutral：状态机语义转发 + 视觉落点（neutral 帧淡入）。 */
    override fun release() {
        delegate.release()
        val view = viewProvider() ?: return
        val neutral = EmotionTagTable.frameFor("neutral", fallbackNeutral = true)
        Log.i(TAG, "release: 复位 neutral（参数帧 ${neutral.size} 键, fade=${NEUTRAL_FADE_MS}ms）")
        view.setParametricExpression(neutral, NEUTRAL_FADE_MS)
    }
}
