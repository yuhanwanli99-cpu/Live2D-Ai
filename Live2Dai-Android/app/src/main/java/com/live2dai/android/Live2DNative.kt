package com.live2dai.android

object Live2DNative {
    private var nativeLoaded = false

    fun load(): Boolean {
        if (nativeLoaded) return true
        return try {
            System.loadLibrary("live2dcubismcore")
            nativeLoaded = true
            android.util.Log.i("Live2DNative", "Native library loaded (Purism Core)")
            true
        } catch (e: UnsatisfiedLinkError) {
            android.util.Log.w("Live2DNative", "Failed to load native: ${e.message}")
            false
        }
    }

    fun isLoaded(): Boolean = nativeLoaded

    fun requireLoaded() {
        check(nativeLoaded) { "Live2D native library (Purism Core) not loaded" }
    }

    // Flag constants
    const val FLAG_ADDITIVE = 1
    const val FLAG_MULTIPLICATIVE = 2
    const val FLAG_DOUBLE_SIDED = 4
    const val FLAG_INVERTED_MASK = 8

    const val DYNAMIC_IS_VISIBLE = 1
    const val DYNAMIC_VISIBILITY_CHANGED = 2
    const val DYNAMIC_OPACITY_CHANGED = 4
    const val DYNAMIC_DRAW_ORDER_CHANGED = 8
    const val DYNAMIC_RENDER_ORDER_CHANGED = 16
    const val DYNAMIC_VERTEX_POSITIONS_CHANGED = 32
    const val DYNAMIC_BLEND_COLOR_CHANGED = 64

    external fun getVersion(): Long
    external fun getLatestMocVersion(): Long
    external fun loadModel(mocBytes: ByteArray): Long
    external fun freeModel(modelPtr: Long)
    external fun updateModel(modelPtr: Long)

    /**
     * C8b: 最近一次 csmUpdateModel 是否走了 light 路径（无输入变化 → 顶点冻结）。
     * 渲染器据此在 light 帧跳过逐 drawable 顶点重取/重传。
     */
    external fun getLastUpdateLight(modelPtr: Long): Boolean
    external fun getCanvasInfo(modelPtr: Long): FloatArray?
    external fun getParameterCount(modelPtr: Long): Int
    external fun getParameterIds(modelPtr: Long): Array<String>?
    external fun getParameterValues(modelPtr: Long): FloatArray?
    external fun setParameterValue(modelPtr: Long, index: Int, value: Float)
    external fun setParameterValues(modelPtr: Long, values: FloatArray)
    external fun getParameterMinValues(modelPtr: Long): FloatArray?
    external fun getParameterMaxValues(modelPtr: Long): FloatArray?
    external fun getParameterDefaultValues(modelPtr: Long): FloatArray?
    external fun getPartCount(modelPtr: Long): Int
    external fun getPartIds(modelPtr: Long): Array<String>?
    external fun getPartOpacities(modelPtr: Long): FloatArray?
    external fun setPartOpacity(modelPtr: Long, index: Int, value: Float)
    external fun getDrawableCount(modelPtr: Long): Int
    external fun getDrawableTextureIndices(modelPtr: Long): IntArray?
    external fun getDrawableDrawOrders(modelPtr: Long): IntArray?
    external fun getDrawableOpacities(modelPtr: Long): FloatArray?
    external fun getDrawableVertexCounts(modelPtr: Long): IntArray?
    external fun getDrawablePositions(modelPtr: Long, drawableIndex: Int): FloatArray?
    /**
     * C4-GC：将指定 drawable 的顶点写入调用方提供的复用缓冲（消除每帧 FloatArray 分配）。
     * 返回写入的顶点数（vCount）；输入非法/缓冲过小返回 -1。
     */
    external fun fillDrawablePositions(modelPtr: Long, drawableIndex: Int, out: FloatArray): Int
    external fun getDrawableUvs(modelPtr: Long, drawableIndex: Int): FloatArray?
    external fun getDrawableIndexCounts(modelPtr: Long): IntArray?
    external fun getDrawableIndices(modelPtr: Long, drawableIndex: Int): ShortArray?
    external fun getDrawableConstantFlags(modelPtr: Long): ByteArray?
    external fun getDrawableDynamicFlags(modelPtr: Long): ByteArray?
    external fun getDrawableMaskCounts(modelPtr: Long): IntArray?
    external fun getDrawableMasks(modelPtr: Long, drawableIndex: Int): IntArray?
    external fun getDrawableMultiplyColors(modelPtr: Long): FloatArray?
    external fun getDrawableScreenColors(modelPtr: Long): FloatArray?
    external fun getDrawableParentPartIndices(modelPtr: Long): IntArray?
    external fun getRenderOrders(modelPtr: Long): IntArray?

    // ====================================================================
    // M1 (Wave 1): engine 骨架（framework + purism 加载，C API 见 cpp/engine/）
    // 签名表锁定草案：engineUpdate/Draw/Set* 不带句柄，作用于当前引擎/当前模型；
    // engineInit 返回引擎句柄，engineRelease 以该句柄释放。
    // @JvmStatic: JNI 静态导出（Java_ 符号 + jclass 首参），主代理机械修正（AND-W1-M1-A3）
    // ====================================================================
    @JvmStatic external fun engineInit(): Long
    @JvmStatic external fun engineLoadModel(moc3Path: String, model3Path: String, cdi3Path: String): Long
    @JvmStatic external fun engineUpdate(dt: Float)
    @JvmStatic external fun engineDraw()
    @JvmStatic external fun engineSetLipSync(value: Float)
    @JvmStatic external fun engineSetMvpMatrix(matrix: FloatArray)
    @JvmStatic external fun engineSetParameters(params: Map<String, Float>)
    @JvmStatic external fun engineRelease(handle: Long)
}
