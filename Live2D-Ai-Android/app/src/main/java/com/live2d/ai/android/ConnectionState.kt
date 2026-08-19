package com.live2d.ai.android

/**
 * 连接状态（R1-ui M4b：对齐 PC chat-header 的「已连接/未连接」状态指示）。
 *
 * Android 端直连 LLM API（无持久 WS 连接），以请求生命周期驱动：
 * - [Connecting] 连接中：请求在途（正在建立连接 / 等待响应）
 * - [Connected] 已连接：最近一次请求成功完成（服务端可达）
 * - [Disconnected] 未连接：初始状态 / 网络失败（连接建立或读取异常）
 *
 * 契约：contract-R1-ui.md §2.2 / §3 M4-B1/M4-B2（签名由 T3b-R1-ui-wave2-red.md §3 锁定）。
 */
sealed interface ConnectionState {
    object Connecting : ConnectionState
    object Connected : ConnectionState
    object Disconnected : ConnectionState
}

/**
 * 连接状态 → 中文文案（纯函数，JVM 可单测；契约 M4-B2 逐字锁定）。
 *
 * Connecting→"连接中"、Connected→"已连接"、Disconnected→"未连接"。
 *
 * when 对 sealed 接口穷尽覆盖三成员：全函数、不抛缺分支异常、不返回空串
 * （「空/异常态防御」以全函数性落地——参数为非空 sealed 类型，null 无法表达，见 T3b §5.2）。
 */
fun connectionLabel(state: ConnectionState): String = when (state) {
    ConnectionState.Connecting -> "连接中"
    ConnectionState.Connected -> "已连接"
    ConnectionState.Disconnected -> "未连接"
}
