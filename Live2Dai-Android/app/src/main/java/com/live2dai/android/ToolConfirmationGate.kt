package com.live2dai.android

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.coroutines.CompletableDeferred

/**
 * 正在等待用户确认的单个工具请求（供 Compose UI 展示）。
 */
data class PendingToolConfirmation(
    val toolName: String,
    val toolDescription: String,
    val argsJson: String,
)

/**
 * Compose 状态驱动的 [ConfirmationGate] 实现。
 *
 * [confirm] 在 ChatService 的工具循环协程（Dispatchers.IO）中被调用，这里挂起
 * （[CompletableDeferred.await]），直到用户在 MainActivity 的确认对话框上点击
 * 「同意/拒绝」——[respond] 由 UI 线程调用，不会阻塞 IO 协程。
 */
class ToolConfirmationGate : ConfirmationGate {

    /** 当前待确认的工具请求；null = 无待确认项（对话框隐藏）。Compose 可观察。 */
    var pending by mutableStateOf<PendingToolConfirmation?>(null)
        private set

    private var deferred: CompletableDeferred<Boolean>? = null

    override suspend fun confirm(tool: ToolDefinition, argsJson: String): Boolean {
        val d = CompletableDeferred<Boolean>()
        deferred = d
        pending = PendingToolConfirmation(tool.name, tool.description, argsJson)
        return try {
            d.await()
        } finally {
            pending = null
            deferred = null
        }
    }

    /**
     * 用户在确认对话框上做出决定（在 UI 线程调用）。
     *
     * @param approved true = 同意执行，false = 拒绝
     */
    fun respond(approved: Boolean) {
        deferred?.complete(approved)
    }
}
