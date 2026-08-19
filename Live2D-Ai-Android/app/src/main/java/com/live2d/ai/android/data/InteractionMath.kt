package com.live2d.ai.android.data

import org.json.JSONObject

/**
 * 模型交互纯函数（contract-C-M5 §2.1，可 JVM 单测，无 Android 依赖）。
 *
 * - 缩放钳制 [INTERACTION_SCALE_MIN, INTERACTION_SCALE_MAX]（researched：
 *   Open-LLM-VTuber MIN_SCALE=0.1 / MAX_SCALE=5.0；N.E.K.O. SCALE_MAX=5.0）
 * - 捏合比例 = oldScale * currentDistancePx / startDistancePx（start<=0 或任一距离非有限 → oldScale 不变）
 * - 拖拽 px → 模型单位（bounds = 当前投影边界 [left, right, top, bottom]，模型单位）
 * - override JSON 编解码：{"version":1,"scale":..,"xOffset":..,"yOffset":..}
 */

/** 缩放钳制下界（researched: Open-LLM-VTuber MIN_SCALE=0.1） */
const val INTERACTION_SCALE_MIN = 0.1f

/** 缩放钳制上界（researched: Open-LLM-VTuber MAX_SCALE=5.0；N.E.K.O. SCALE_MAX=5.0） */
const val INTERACTION_SCALE_MAX = 5.0f

/**
 * 用户调整后的显示状态（模型单位）。
 * scale 钳制在 [0.1, 5.0]；xOffset/yOffset 无界（允许拖出屏，重置入口兜底）。
 */
data class Override(val scale: Float, val xOffset: Float, val yOffset: Float)

/**
 * 缩放钳制：coerceIn(INTERACTION_SCALE_MIN, INTERACTION_SCALE_MAX)。
 * NaN（防御）→ 1.0f（不 propagate 非有限值）。
 */
fun clampScale(scale: Float): Float {
    if (scale.isNaN()) return 1f
    return scale.coerceIn(INTERACTION_SCALE_MIN, INTERACTION_SCALE_MAX)
}

/**
 * 捏合缩放：= clampScale(oldScale * currentDistancePx / startDistancePx)。
 * startDistancePx <= 0 或非有限（防御，防除零），或 currentDistancePx 非有限
 * （防御，防 NaN/Inf 按比例放大出界）→ 返回 oldScale 不变。
 */
fun pinchScale(oldScale: Float, startDistancePx: Float, currentDistancePx: Float): Float {
    if (startDistancePx <= 0f || !startDistancePx.isFinite()) return oldScale
    if (!currentDistancePx.isFinite()) return oldScale
    return clampScale(oldScale * currentDistancePx / startDistancePx)
}

/**
 * 拖拽像素位移 → 模型单位位移。
 *
 * bounds 为当前投影边界 [left, right, top, bottom]（模型单位，renderer 状态）：
 *   dxModel = dxPx * (boundsRight - boundsLeft) / viewportW    （屏幕 x 与模型 x 同向）
 *   dyModel = -dyPx * (boundsTop - boundsBottom) / viewportH   （屏幕 y 向下 ↔ 模型 y 向上，符号翻转）
 *
 * 结果直接与 displayXOffset/displayYOffset 同量纲（模型单位）。
 */
fun dragPxToModel(
    dxPx: Float,
    dyPx: Float,
    viewportW: Float,
    viewportH: Float,
    boundsLeft: Float,
    boundsRight: Float,
    boundsTop: Float,
    boundsBottom: Float
): Pair<Float, Float> {
    val dxModel = dxPx * (boundsRight - boundsLeft) / viewportW
    val dyModel = -dyPx * (boundsTop - boundsBottom) / viewportH
    return dxModel to dyModel
}

/**
 * 编码用户 override 为 JSON：{"version":1,"scale":..,"xOffset":..,"yOffset":..}。
 * 数值以 double 写出（roundtrip 精确，Float→JSON→Float 无精度损失）。
 * 非有限值（NaN/Inf，防御：JSON 禁止写 NaN/Inf，直接 put 会抛 JSONException）→ 省略对应字段，
 * 使 [decodeOverride] 因缺字段（optDouble 默认 NaN → 非有限）返回 null —— 不抛异常且 roundtrip 被拒绝。
 */
fun encodeOverride(scale: Float, xOffset: Float, yOffset: Float): String {
    val root = JSONObject().put("version", 1)
    if (scale.isFinite()) root.put("scale", scale.toDouble())
    if (xOffset.isFinite()) root.put("xOffset", xOffset.toDouble())
    if (yOffset.isFinite()) root.put("yOffset", yOffset.toDouble())
    return root.toString()
}

/**
 * 解码用户 override。非法 JSON / 字段缺失 / version!=1 / 非有限值（NaN/Inf）→ null（不崩溃）。
 */
fun decodeOverride(json: String?): Override? {
    if (json == null) return null
    return try {
        val root = JSONObject(json)
        val version = root.opt("version")
        if (version !is Number || version.toDouble() != 1.0) return null
        val scale = root.optDouble("scale", Double.NaN)
        val xOffset = root.optDouble("xOffset", Double.NaN)
        val yOffset = root.optDouble("yOffset", Double.NaN)
        if (!scale.isFinite() || !xOffset.isFinite() || !yOffset.isFinite()) return null
        Override(scale.toFloat(), xOffset.toFloat(), yOffset.toFloat())
    } catch (e: Exception) {
        null
    }
}

/**
 * 生效值组合（contract-C-M5 §2.3 优先级：用户 override > fit（C-M1）> 默认(1f, 0f, 0f)）。
 *
 * 用户 override 存在 → 原样生效；否则取 fit 结果。
 * （computeCharacterFit 退化时自身返回 (1f, 0f, 0f)，因此「fit 退化 → 默认」由该函数天然覆盖。）
 */
fun effectiveOverride(user: Override?, fit: CharacterFit): Override =
    user ?: Override(fit.scale, fit.xOffset, fit.yOffset)
