package com.live2dai.android

/**
 * PurismModel — 将 Purism Core (Live2DNative JNI) 封装为 CubismModel 兼容接口。
 *
 * 关键职责：
 * 1. 从 Live2DNative 获取原始 drawable 数据
 * 2. 将 canvas 像素坐标除以 pixelsPerUnit 转换为模型单位
 * 3. 提供打包数据的便捷方法（getDrawableMasks, getRenderOrders 等）
 *
 * 对应 CubismJavaFramework 的 CubismModel 类。
 *
 * ## 坐标系
 *
 * Purism Core 返回的顶点坐标是 canvas 像素值（如 0~1280）。
 * CubismRendererAndroid 期望的是模型单位（除以 ppu 后）。
 * 本类在 getDrawableVertices / getDrawableVertexPositions 中自动除以 pixelsPerUnit。
 */
class PurismModel(val modelPtr: Long) {

    // ============================================================
    //  参数快照（用于 save/loadParameters）
    // ============================================================

    /**
     * 参数值快照缓存。
     * loadParameters 从 Core 读到缓存，saveParameters 从缓存写回 Core。
     * 这样 motion/expression/breath 的修改可以叠加在上一帧的最终状态上。
     */
    var savedParameterValues: FloatArray? = null
        private set

    // ============================================================
    //  Canvas 信息
    // ============================================================

    private var canvasWidthPixels: Float = 0f
    private var canvasHeightPixels: Float = 0f
    private var pixelsPerUnit: Float = 1f

    /** 以像素为单位的画布宽度 */
    fun getCanvasWidthPixel(): Float = canvasWidthPixels

    /** 以像素为单位的画布高度 */
    fun getCanvasHeightPixel(): Float = canvasHeightPixels

    /** 以模型单位（除以 ppu）为单位的画布宽度 */
    fun getCanvasWidth(): Float = canvasWidthPixels / pixelsPerUnit

    /** 以模型单位（除以 ppu）为单位的画布高度 */
    fun getCanvasHeight(): Float = canvasHeightPixels / pixelsPerUnit

    /** 每单位像素数 */
    fun getPixelPerUnit(): Float = pixelsPerUnit

    /** 从 Live2DNative 刷新 canvas 信息 */
    private fun refreshCanvasInfo() {
        Live2DNative.getCanvasInfo(modelPtr)?.let {
            canvasWidthPixels = it.getOrElse(0) { 0f }
            canvasHeightPixels = it.getOrElse(1) { 0f }
            pixelsPerUnit = it.getOrElse(4) { 1f }
        }
    }

    // ============================================================
    //  模型生命周期
    // ============================================================

    /** 更新模型（每帧调用，计算参数插值） */
    fun update() {
        Live2DNative.updateModel(modelPtr)
    }

    /**
     * 从 Purism Core 读取所有参数当前值，保存到缓存。
     * 在每帧动画更新前调用，将参数恢复到帧开始时的快照。
     */
    fun loadParameters() {
        // 从缓存恢复参数（将上一帧保存的状态写回 native）
        val saved = savedParameterValues ?: return
        for (i in saved.indices) {
            Live2DNative.setParameterValue(modelPtr, i, saved[i])
        }
    }

    /**
     * 保存当前参数状态到缓存（供下一帧的 loadParameters 恢复）
     */
    fun saveParameters() {
        val values = Live2DNative.getParameterValues(modelPtr) ?: return
        var target = savedParameterValues
        if (target == null || target.size != values.size) {
            target = FloatArray(values.size)
            savedParameterValues = target
        }
        for (i in values.indices) {
            target[i] = values[i]
        }
    }

    // ============================================================
    //  Drawable 基本信息
    // ============================================================

    /** Drawable 数量 */
    fun getDrawableCount(): Int {
        return Live2DNative.getDrawableCount(modelPtr)
    }

    /** 纹理索引（drawableIndex → model texture index） */
    fun getDrawableTextureIndex(drawableIndex: Int): Int {
        val indices = Live2DNative.getDrawableTextureIndices(modelPtr) ?: return 0
        return indices.getOrElse(drawableIndex) { 0 }
    }

    /** 顶点索引数量 */
    fun getDrawableVertexIndexCount(drawableIndex: Int): Int {
        val counts = Live2DNative.getDrawableIndexCounts(modelPtr) ?: return 0
        return counts.getOrElse(drawableIndex) { 0 }
    }

    /** 顶点索引数据（short[]） */
    fun getDrawableVertexIndices(drawableIndex: Int): ShortArray {
        return Live2DNative.getDrawableIndices(modelPtr, drawableIndex) ?: ShortArray(0)
    }

    /** 顶点数量 */
    fun getDrawableVertexCount(drawableIndex: Int): Int {
        val counts = Live2DNative.getDrawableVertexCounts(modelPtr) ?: return 0
        return counts.getOrElse(drawableIndex) { 0 }
    }

    /** 顶点位置（模型单位，已除以 ppu），xy 交错 */
    fun getDrawableVertices(drawableIndex: Int): FloatArray {
        // Purism Core 已返回模型单位坐标（范围约[-0.5,0.5]），无需除以 ppu
        return Live2DNative.getDrawablePositions(modelPtr, drawableIndex) ?: FloatArray(0)
    }

    /** getDrawableVertices 的别名 */
    fun getDrawableVertexPositions(drawableIndex: Int): FloatArray = getDrawableVertices(drawableIndex)

    // ============================================================
    //  UV 坐标
    // ============================================================

    /** 纹理 UV 坐标（uv 交错） */
    fun getDrawableVertexUvs(drawableIndex: Int): FloatArray {
        return Live2DNative.getDrawableUvs(modelPtr, drawableIndex) ?: FloatArray(0)
    }

    // ============================================================
    //  渲染属性
    // ============================================================

    /** 不透明度 (0~1) */
    fun getDrawableOpacity(drawableIndex: Int): Float {
        val opacities = Live2DNative.getDrawableOpacities(modelPtr) ?: return 1f
        return opacities.getOrElse(drawableIndex) { 1f }
    }

    /** Multiply 颜色 [R,G,B,A] */
    fun getDrawableMultiplyColor(drawableIndex: Int): FloatArray {
        val colors = Live2DNative.getDrawableMultiplyColors(modelPtr) ?: return floatArrayOf(1f, 1f, 1f, 1f)
        val i = drawableIndex * 4
        return if (i + 3 < colors.size) {
            floatArrayOf(colors[i], colors[i + 1], colors[i + 2], colors[i + 3])
        } else {
            floatArrayOf(1f, 1f, 1f, 1f)
        }
    }

    /** Screen 颜色 [R,G,B,A] */
    fun getDrawableScreenColor(drawableIndex: Int): FloatArray {
        val colors = Live2DNative.getDrawableScreenColors(modelPtr) ?: return floatArrayOf(0f, 0f, 0f, 0f)
        val i = drawableIndex * 4
        return if (i + 3 < colors.size) {
            floatArrayOf(colors[i], colors[i + 1], colors[i + 2], colors[i + 3])
        } else {
            floatArrayOf(0f, 0f, 0f, 0f)
        }
    }

    // ============================================================
    //  Flags（从 constantFlags 和 dynamicFlags 解析）
    // ============================================================

    /** 是否为反向 mask */
    fun getDrawableInvertedMask(drawableIndex: Int): Boolean {
        val flags = Live2DNative.getDrawableConstantFlags(modelPtr) ?: return false
        return (flags.getOrElse(drawableIndex) { 0 }.toInt() and Live2DNative.FLAG_INVERTED_MASK) != 0
    }

    /** 混合模式: 0=normal, 1=additive, 2=multiplicative */
    fun getDrawableBlendModeType(drawableIndex: Int): Int {
        val flags = Live2DNative.getDrawableConstantFlags(modelPtr) ?: return 0
        val f = flags.getOrElse(drawableIndex) { 0 }.toInt() and 0xFF
        return when {
            (f and Live2DNative.FLAG_ADDITIVE) != 0 -> 1
            (f and Live2DNative.FLAG_MULTIPLICATIVE) != 0 -> 2
            else -> 0
        }
    }

    /** 是否启用背面剔除（false = 双面渲染 = 不剔除） */
    fun getDrawableCulling(drawableIndex: Int): Boolean {
        val flags = Live2DNative.getDrawableConstantFlags(modelPtr) ?: return true
        val f = flags.getOrElse(drawableIndex) { 0 }.toInt() and 0xFF
        // FLAG_DOUBLE_SIDED 置位 = 不剔除 = culling disabled
        return (f and Live2DNative.FLAG_DOUBLE_SIDED) == 0
    }

    // ============================================================
    //  Dynamic Flags（每帧变化）
    // ============================================================

    /** 动态可见性标记 */
    fun getDrawableDynamicFlagIsVisible(drawableIndex: Int): Boolean {
        val flags = Live2DNative.getDrawableDynamicFlags(modelPtr) ?: return false
        return (flags.getOrElse(drawableIndex) { 0 }.toInt() and Live2DNative.DYNAMIC_IS_VISIBLE) != 0
    }

    /** 顶点位置是否变化 */
    fun getDrawableDynamicFlagVertexPositionsDidChange(drawableIndex: Int): Boolean {
        val flags = Live2DNative.getDrawableDynamicFlags(modelPtr) ?: return false
        return (flags.getOrElse(drawableIndex) { 0 }.toInt() and Live2DNative.DYNAMIC_VERTEX_POSITIONS_CHANGED) != 0
    }

    // ============================================================
    //  排序数据
    // ============================================================

    /** 渲染顺序数组（索引 = drawableIndex/offscreenIndex，值 = renderOrder） */
    fun getRenderOrders(): IntArray {
        return Live2DNative.getRenderOrders(modelPtr)
            ?: Live2DNative.getDrawableDrawOrders(modelPtr)
            ?: IntArray(0)
    }

    // ============================================================
    //  Mask 数据
    // ============================================================

    /** 每个 drawable 的 mask 索引列表（二维） */
    fun getDrawableMasks(): Array<IntArray> {
        val count = getDrawableCount()
        val result = java.util.ArrayList<IntArray>(count)
        for (i in 0 until count) {
            result.add(Live2DNative.getDrawableMasks(modelPtr, i) ?: IntArray(0))
        }
        return result.toTypedArray()
    }

    /** 每个 drawable 的 mask 数量 */
    fun getDrawableMaskCounts(): IntArray {
        return Live2DNative.getDrawableMaskCounts(modelPtr) ?: IntArray(getDrawableCount()) { 0 }
    }

    /** 单个 drawable 的 mask 索引列表 */
    fun getDrawableMasks(drawableIndex: Int): IntArray {
        return Live2DNative.getDrawableMasks(modelPtr, drawableIndex) ?: IntArray(0)
    }

    /** 单个 drawable 的 mask 数量 */
    fun getDrawableMaskCount(drawableIndex: Int): Int {
        val counts = Live2DNative.getDrawableMaskCounts(modelPtr) ?: return 0
        return counts.getOrElse(drawableIndex) { 0 }
    }

    // ============================================================
    //  Part 层级
    // ============================================================

    /** Drawable 的父 Part 索引 */
    fun getDrawableParentPartIndex(drawableIndex: Int): Int {
        val indices = Live2DNative.getDrawableParentPartIndices(modelPtr) ?: return -1
        return indices.getOrElse(drawableIndex) { -1 }
    }

    /** 批量 Drawable 父 Part 索引 */
    fun getDrawableParentPartIndices(): IntArray {
        return Live2DNative.getDrawableParentPartIndices(modelPtr) ?: IntArray(0)
    }

    /** Part 数量 */
    fun getPartCount(): Int {
        return Live2DNative.getPartCount(modelPtr)
    }

    /** Part ID 列表 */
    fun getPartIds(): Array<String>? {
        return Live2DNative.getPartIds(modelPtr)
    }

    // ============================================================
    //  参数控制
    // ============================================================

    /** 参数数量 */
    fun getParameterCount(): Int {
        return Live2DNative.getParameterCount(modelPtr)
    }

    /** 参数 ID 列表 */
    fun getParameterIds(): Array<String>? {
        return Live2DNative.getParameterIds(modelPtr)
    }

    /** 参数值列表 */
    fun getParameterValues(): FloatArray? {
        return Live2DNative.getParameterValues(modelPtr)
    }

    /** 设置参数值 */
    fun setParameterValue(index: Int, value: Float) {
        Live2DNative.setParameterValue(modelPtr, index, value)
    }

    /** 批量设置参数值 */
    fun setParameterValues(values: FloatArray) {
        Live2DNative.setParameterValues(modelPtr, values)
    }

    /** 参数最小值 */
    fun getParameterMinValues(): FloatArray? {
        return Live2DNative.getParameterMinValues(modelPtr)
    }

    /** 参数最大值 */
    fun getParameterMaxValues(): FloatArray? {
        return Live2DNative.getParameterMaxValues(modelPtr)
    }

    /** 参数默认值 */
    fun getParameterDefaultValues(): FloatArray? {
        return Live2DNative.getParameterDefaultValues(modelPtr)
    }

    /** Part 不透明度列表 */
    fun getPartOpacities(): FloatArray? {
        return Live2DNative.getPartOpacities(modelPtr)
    }

    /** 设置 Part 不透明度 */
    fun setPartOpacity(index: Int, value: Float) {
        Live2DNative.setPartOpacity(modelPtr, index, value)
    }

    // ============================================================
    //  Canvas 信息初始化
    // ============================================================

    /** 初始化 canvas 信息（构造时由工厂方法调用） */
    fun initCanvasInfo() {
        refreshCanvasInfo()
    }
}
