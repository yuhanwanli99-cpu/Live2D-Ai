package com.live2dai.android.data

/**
 * 半身投影边界纯函数（contract-v3-rename §2.1，K5 定案，可 JVM 单测）。
 *
 * 返回投影视口的 4 边界 [left, right, top, bottom]（模型单位），供渲染端 orthoM 使用：
 *   halfW = viewWidth / 2；halfH = viewHeight / 2
 *   halfW' = halfW / scale；halfH' = halfH / scale
 *   left=-halfW'，right=halfW'，bottom=-halfH' + yOffset，top=halfH' + yOffset
 *
 * scale=1f, yOffset=0f 时输出与现状（全模型居中）逐位一致，零回归；
 * yOffset > 0 把上下边界同向上移 → 模型下半身超出视口被裁切，等效"上半身特写"。
 *
 * 渲染端（Live2DRenderer.onSurfaceChanged）先按 canvas/视口宽高比计算全模型
 * viewHalfW/viewHalfH，再以 (viewHalfW*2, viewHalfH*2) 传入本函数应用半身缩放/偏移。
 *
 * @param scale        缩放（>1 放大；1 = 现状）
 * @param yOffset      垂直偏移（模型单位，默认 0 = 现状）
 * @param viewWidth    视口全宽（模型单位；渲染端传 viewHalfW*2）
 * @param viewHeight   视口全高（模型单位；渲染端传 viewHalfH*2）
 * @param canvasWidth  画布全宽（模型单位，签名保留；宽高比调整由渲染端完成）
 * @param canvasHeight 画布全高（模型单位，签名保留；宽高比调整由渲染端完成）
 * @return FloatArray[4] = [left, right, top, bottom]
 */
fun computeProjection(
    scale: Float,
    yOffset: Float,
    viewWidth: Float,
    viewHeight: Float,
    canvasWidth: Float,
    canvasHeight: Float
): FloatArray {
    val halfW = viewWidth / 2f
    val halfH = viewHeight / 2f
    val safeScale = if (scale > 0f) scale else 1f
    val halfWp = halfW / safeScale
    val halfHp = halfH / safeScale
    // [left, right, top, bottom]；left=-halfW'，right=halfW'，top=halfH'+yOffset，bottom=-halfH'+yOffset
    return floatArrayOf(-halfWp, halfWp, halfHp + yOffset, -halfHp + yOffset)
}
