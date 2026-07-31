package com.live2dai.plugin.agenttools

/**
 * 通知快捷回复桥接接口——插件只定义"需要什么"，具体实现由 core 注入。
 *
 * 快捷回复依赖 `NotificationListenerService`（需要用户在系统设置里单独授权"通知使用权"，
 * 且必须是一个真正注册的 Service 组件），这是插件模块无法自行声明的（骨架约定
 * `AndroidManifest.xml` 保持空清单，且通知使用权限是按 App 授权的系统级能力）。
 * 因此 `NotificationListenerService` 实现放在 core 侧，插件通过本接口消费其能力。
 *
 * 这是"接缝在 core、逻辑在插件"模式的第三次复用（vision 的 [com.live2dai.plugin.vision.ScreenshotProvider]、
 * memory/proactive 的 LLM 客户端、本接口），参见 `docs/plugin-template.md` 的通用指导。
 *
 * @see SendNotificationReplyTool 本接口的唯一消费方
 */
fun interface NotificationReplyBridge {
    /**
     * 快捷回复某个应用最近一条带快捷回复入口的通知。
     *
     * @param appName 目标应用名称（显示名，如「微信」）
     * @param replyText 回复内容（会真实发送给聊天对象）
     * @return 执行结果 JSON 字符串：`{"result":"..."}` 表示成功，`{"error":"..."}` 表示失败。
     *         失败原因可能包括：未授权通知使用权、找不到可回复通知、发送失败等
     *         （调用方不抛异常，全部以 JSON 形式返回，便于 LLM 理解并引导用户处理）。
     */
    suspend fun reply(appName: String, replyText: String): String
}
