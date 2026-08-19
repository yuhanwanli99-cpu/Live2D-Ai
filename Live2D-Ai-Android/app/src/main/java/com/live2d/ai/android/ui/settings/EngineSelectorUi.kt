package com.live2d.ai.android.ui.settings

import com.live2d.ai.android.DownloadState
import com.live2d.ai.android.TtsHealth
import com.live2d.ai.android.TtsProvider

/**
 * 引擎选择 UI 展示模型 + 纯映射函数（V1.2 模块5，plan §5.5.2 方案A RadioGroup）。
 *
 * 设计目标：UI 渲染与可测试逻辑分离——设置页直接消费本文件的纯函数产物，
 * 不依赖 Android/Compose 运行时，可在 JVM 单测中直接验证：
 * - provider 注册表列表 → [EngineOptionUi]（单选渲染 + health/网络徽标 + 选中态）
 * - [DownloadState] → 下载状态文案 / "下载"按钮可用性
 * - provider 显示名列表 → 降级链摘要（"Kokoro（sherpa 离线）"）
 * - 模型体积 → 人类可读大小文案
 */

/** 引擎单选展示模型（一个 provider 一行）。 */
data class EngineOptionUi(
    /** Provider 唯一标识（TtsProviderInfo.id） */
    val id: String,
    /** 人类可读名称（如 "Kokoro（sherpa 离线）"） */
    val name: String,
    /** 是否需要网络（true = 云端，false = 离线可用） */
    val requiresNetwork: Boolean,
    /** 引擎健康状态快照（OK / DEGRADED / UNAVAILABLE → 正常/降级/不可用） */
    val health: TtsHealth,
    /** 是否有用户可配置参数（sherpa 单引擎无云端可配置项 → 恒 false） */
    val configurable: Boolean,
    /** 是否为当前选中（单选高亮） */
    val isSelected: Boolean,
)

/**
 * TTS Provider 列表 → 引擎选择展示模型（纯函数，可单测）。
 *
 * @param providers 来自 [com.live2d.ai.android.TtsProviderRegistry.all()]
 * @param selectedId 当前首选引擎 ID（SettingsRepository.ttsProviderId）
 */
fun ttsProvidersToUi(
    providers: List<TtsProvider>,
    selectedId: String,
): List<EngineOptionUi> = providers.map { p ->
    EngineOptionUi(
        id = p.info.id,
        name = p.info.name,
        requiresNetwork = p.info.requiresNetwork,
        health = p.info.health.value,
        configurable = p.info.configurable,
        isSelected = p.info.id == selectedId,
    )
}

/**
 * 健康状态 → 中文徽标文案（正常/降级/不可用）。 */
fun healthLabel(health: TtsHealth): String = when (health) {
    TtsHealth.OK -> "正常"
    TtsHealth.DEGRADED -> "降级"
    TtsHealth.UNAVAILABLE -> "不可用"
}

/** 模型下载状态 → 状态文案（未下载/下载中 x%/校验中/已下载/下载失败）。 */
fun downloadStatusLabel(state: DownloadState?): String = when (state) {
    null, is DownloadState.Idle -> "未下载"
    is DownloadState.Downloading -> "下载中 ${(state.progress * 100).toInt()}%"
    DownloadState.Verifying -> "校验中"
    is DownloadState.Completed -> "已下载"
    is DownloadState.Failed -> "下载失败"
}

/**
 * 下载状态 → "下载"按钮是否可点击（纯函数）。
 *
 * 语义：未开始 / 初始态 / 失败（可重试）→ 可点；下载中 / 校验中 / 已完成 → 禁用
 * （去重：同一模型不并发双开下载；已完成无需再下）。
 */
fun canStartDownload(state: DownloadState?): Boolean = when (state) {
    null, is DownloadState.Idle, is DownloadState.Failed -> true
    is DownloadState.Downloading, DownloadState.Verifying, is DownloadState.Completed -> false
}

/** 降级链摘要（provider 显示名按降级链顺序 join）。 */
fun cascadeSummary(providerNames: List<String>): String = providerNames.joinToString(" → ")

/** 模型体积 → 人类可读大小文案（如 "82.0MB"）。 */
fun formatModelSize(bytes: Long): String = when {
    bytes >= 1024L * 1024 * 1024 -> "%.1fGB".format(bytes / 1024.0 / 1024 / 1024)
    bytes >= 1024L * 1024 -> "%.1fMB".format(bytes / 1024.0 / 1024)
    bytes >= 1024L -> "%.1fKB".format(bytes / 1024.0)
    else -> "${bytes}B"
}
