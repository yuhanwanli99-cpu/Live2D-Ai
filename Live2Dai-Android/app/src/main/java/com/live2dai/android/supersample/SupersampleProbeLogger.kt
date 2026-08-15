package com.live2dai.android.supersample

/**
 * 超采样探针日志接收口（seam，纯 JVM）：可注入的 logger 接口，行文本由实现方决定去向
 * （logcat / 测试收集器 / 文件均可）。
 */
fun interface ProbeSink {
    /** 记录一行探针日志。 */
    fun log(line: String)
}

/**
 * 超采样 FBO 探针日志器（seam，纯 JVM，可注入 logger）。
 *
 * 契约 REND2-A1：超采样路径生效证据 = FBO 尺寸探针日志（命中正则 `FBO.*1440.*3200`）
 * + blit 完成日志（命中正则 `blit.*complete`）；每帧恰 1 条 FBO 行 + 1 条 blit 行
 * （帧内计数由 [onFrameEnd] 重置）；直渲兜底路径 [onDirectFallback] 触发时只输出
 * `DIRECT_FALLBACK` 行、不输出任何 FBO/blit 行（正常超采样路径下兜底计数 = 0）。
 *
 * 本 seam 只负责格式与计数，不触发任何 GL 调用；真实兜底触发条件（GL 失败分支）属
 * device 行为（Wave 6 REND2-A1 验收），不在本 seam 判定。
 */
class SupersampleProbeLogger(
    /** 日志接收口（默认 no-op）。 */
    private val sink: ProbeSink = ProbeSink { }
) {

    /** 便捷构造：直接传 lambda（`(String) -> Unit`）作 logger。 */
    constructor(logger: (String) -> Unit) : this(ProbeSink { line -> logger(line) })

    /** 当前帧已记录的 FBO 绑定行数（[onFrameEnd] 清零）。 */
    var frameFboCount: Int = 0
        private set

    /** 当前帧已记录的 blit 完成行数（[onFrameEnd] 清零）。 */
    var frameBlitCount: Int = 0
        private set

    /** 直渲兜底触发总计数（生命周期累计，正常超采样路径应为 0）。 */
    var fallbackCount: Int = 0
        private set

    /**
     * FBO 绑定探针：输出 `FBO bound <w> x <h> (supersample render target)`，
     * 命中正则 `FBO.*1440.*3200`（契约硬值 1440×3200）。
     */
    fun onFboBound(width: Int, height: Int) {
        frameFboCount++
        sink.log("FBO bound $width x $height (supersample render target)")
    }

    /**
     * [onFboBound] 的别名（bind 时态写法），行为与输出完全一致。
     */
    fun onFboBind(width: Int, height: Int) = onFboBound(width, height)

    /**
     * blit 完成探针：输出 `blit complete to default framebuffer`，
     * 命中正则 `blit.*complete`（含数字亦可，文案不锁定）。
     */
    fun onBlitComplete() {
        frameBlitCount++
        sink.log("blit complete to default framebuffer")
    }

    /**
     * 直渲兜底探针：输出 `DIRECT_FALLBACK`（不产生 FBO/blit 行）。
     * 正常超采样路径零触发（[fallbackCount] 保持 0）。
     */
    fun onDirectFallback() {
        fallbackCount++
        sink.log("DIRECT_FALLBACK")
    }

    /** 帧结束：重置帧内 FBO/blit 计数（[fallbackCount] 为生命周期累计，不清零）。 */
    fun onFrameEnd() {
        frameFboCount = 0
        frameBlitCount = 0
    }
}
