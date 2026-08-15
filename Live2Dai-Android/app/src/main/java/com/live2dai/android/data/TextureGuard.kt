package com.live2dai.android.data

/**
 * 纹理上限防护纯函数（contract-R1-render §2.4 M1d，C10 拍板：降采样）。
 *
 * 对齐 PC `checkTextureFit` 的 targetSize 语义：超限纹理降采样到
 * ≤ GL_MAX_TEXTURE_SIZE 的 2 的幂尺寸，经 BitmapFactory.Options.inSampleSize
 * （2 的幂采样）实现。本文件仅含纯函数（JVM 可单测），GL 侧接线在
 * Live2DRenderer.loadTextures。
 */

/** 返回 ≤ maxSize 的最大 2 的幂；maxSize<=0 → 1；不溢出（2^30 量级安全）。 */
fun computeDownsampleTarget(maxSize: Int): Int {
    if (maxSize <= 0) return 1
    var target = 1
    while (target <= maxSize / 2) {
        target *= 2
    }
    return target
}

/** 判定是否需降采样：texW>maxSize || texH>maxSize（边界相等不降采样）。 */
fun needsDownsample(texW: Int, texH: Int, maxSize: Int): Boolean =
    texW > maxSize || texH > maxSize
