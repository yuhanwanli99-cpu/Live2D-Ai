package com.live2d.ai.android.core

import java.util.concurrent.atomic.AtomicBoolean

/**
 * 渲染质量三档（用户需求：流畅 / 均衡 / 高清）。
 *
 * 纯 JVM 枚举 + 全局当前值容器，**零 Android 依赖**（渲染器 GL 线程 / 设置 UI 主线程共享；
 * [RenderQualityConfig.current] 为 @Volatile，跨线程可见）。
 *
 * 各档位渲染语义（接线点：Live2DRenderer 超采样 FBO 重建 + Live2DView.MSAAConfigChooser
 * EGL 配置选择）：
 *
 * 1. [SMOOTH]（默认）：超采样关（scale=1.0 直渲）、MSAA 关 —— 最低 GPU 填充率；
 * 2. [BALANCED]：超采样关 + MSAA 4x 开（EGL 配置层启用 —— EGLConfigChooser 按
 *    [msaaEnabled] 选 4x 配置；**MSAA 属 EGL 上下文属性，切换需 EGL 上下文重建 → 重启生效**，
 *    设置页 UI 已注明"重启生效"）；
 * 3. [HD]：超采样开（scale=4/3：1080×2400 → 1440×3200 物理 2K 同分辨率）、MSAA 关
 *    （防 2K 模式 GPU 填充瓶颈，历史 C-Perf5 结论：2K × 4xMSAA = 等效 ~1840 万像素填充/帧）。
 */
enum class RenderQuality {
    SMOOTH,
    BALANCED,
    HD;

    /** 超采样倍率（>1.0 才建 FBO；SMOOTH/BALANCED = 1.0 直渲，与 C8h 优化前逐位一致）。 */
    val supersampleScale: Float
        get() = when (this) {
            SMOOTH -> 1f
            BALANCED -> 1f
            HD -> 4f / 3f
        }

    /** 是否启用 EGL 4x MSAA（仅 BALANCED；需 EGL 上下文重建/重启生效）。 */
    val msaaEnabled: Boolean
        get() = this == BALANCED

    /** 设置页三档选择按钮文案。 */
    val label: String
        get() = when (this) {
            SMOOTH -> "流畅"
            BALANCED -> "均衡"
            HD -> "高清"
        }

    /** 设置页档位说明副文案（含重启生效提示）。 */
    val hint: String
        get() = when (this) {
            SMOOTH -> "直渲 · 无抗锯齿，帧率优先"
            BALANCED -> "MSAA 4x 抗锯齿（需重启生效）"
            HD -> "2K 超采样渲染，画质优先"
        }
}

/**
 * 渲染质量全局当前值容器。
 *
 * - [current]：渲染器 GL 线程每帧读取（超采样 scale / 档位判定），设置页 UI 主线程写入
 *   → @Volatile 保证跨线程可见。
 * - [restoreFromStorage]：进程内**一次性**从持久化恢复（"重启恢复"语义：App 重启后
 *   Live2DRenderer 构造时恢复，先于任何渲染帧生效）；运行期设置页选择直写 [current]，
 *   不被后续 surface 重建 / 新 renderer 实例重复覆盖。
 * - 默认 [RenderQuality.SMOOTH]（未持久化 / 未知值经 [fromStorage] 兜底）。
 */
object RenderQualityConfig {

    @Volatile
    var current: RenderQuality = RenderQuality.SMOOTH

    /** 进程内是否已从持久化恢复（防多 renderer 实例 / surface 重建覆盖运行期选择）。 */
    private val restoredFromStorage = AtomicBoolean(false)

    /**
     * 从持久化值恢复当前档位（进程内一次性；调用方 = Live2DRenderer 构造时读取
     * SettingsRepository.renderQuality —— "重启恢复"落点）。
     *
     * @param stored 持久化档位（SettingsRepository.renderQuality；未知值已由 [fromStorage] 兜底）
     */
    fun restoreFromStorage(stored: RenderQuality) {
        if (restoredFromStorage.compareAndSet(false, true)) {
            current = stored
        }
    }

    /** 按持久化字符串解析档位（缺失 / 未知值 → SMOOTH 兜底，零行为变化）。 */
    fun fromStorage(value: String?): RenderQuality =
        RenderQuality.entries.firstOrNull { it.name == value } ?: RenderQuality.SMOOTH
}
