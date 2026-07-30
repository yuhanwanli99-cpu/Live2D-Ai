package com.live2dai.android

import android.content.Context
import android.graphics.BitmapFactory
import android.opengl.GLES20
import android.opengl.GLSurfaceView
import android.opengl.GLUtils
import android.opengl.Matrix
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.nio.ShortBuffer
import java.util.Collections
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.abs

/**
 * Live2D 渲染器 — 按 CubismRendererAndroid 的 doDrawModel() 算法实现。
 *
 * ## 架构
 *
 * ```
 * Live2DRenderer (GLSurfaceView.Renderer)
 *   ├── PurismModel     — Live2DNative JNI 包装，提供 CubismModel 兼容接口
 *   ├── ShaderManager   — 着色器加载/编译/缓存/选择
 *   └── modelSettingJson — model3.json 配置（动作/表情/纹理）
 * ```
 *
 * ## 渲染管线（对应 CubismRendererAndroid.doDrawModel()）
 *
 * 1. updateModel() — 更新模型状态（参数插值）
 * 2. buildDrawableList() — 构建 DrawableRenderData 列表，按 renderOrder 排序
 * 3. preDraw() — 重置 OpenGL 状态
 * 4. drawObjectLoop() — 逐个渲染 drawable：
 *    a. 检查可见性 (getDrawableDynamicFlagIsVisible)
 *    b. 选择着色器（根据 blendMode 和 isInvertedMask）
 *    c. 设置 blend 模式（预乘 alpha 混合）
 *    d. 设置面剔除
 *    e. 绑定纹理 + 设置 uniforms
 *    f. glDrawElements
 * 5. 清除状态
 *
 * ## 公开 API（与 Live2DView 兼容）
 * - startMotion(motionGroup, motionIndex)
 * - setExpression(expressionIndex)
 * - setLipSyncParam(value)
 * - dispose()
 */
class Live2DRenderer(
    private val context: Context,
    private val modelAssetPath: String
) : GLSurfaceView.Renderer {

    // ============================================================
    //  Drawable 渲染数据 — 包级可见（供 PurismClippingManager 使用）
    // ============================================================

    data class DrawableRenderData(
        val index: Int,
        val textureIndex: Int,
        val renderOrder: Int,
        val vertexCount: Int,
        val indexCount: Int,
        val positionsBuffer: FloatBuffer,
        val uvsBuffer: FloatBuffer,
        val indicesBuffer: ShortBuffer,
        val opacity: Float,
        val multiplyColor: FloatArray,
        val screenColor: FloatArray,
        val isDoubleSided: Boolean,
        val isInvertedMask: Boolean,
        val blendMode: Int,
        val maskCount: Int,
        val maskIndices: IntArray,
        val isVisible: Boolean
    )

    // ============================================================
    //  状态
    // ============================================================

    private var nativeAvailable = false
    private var fallbackActive = false
    private var modelPtr: Long = 0
    private var modelSettingJson: JSONObject? = null
    private val modelDir: String = modelAssetPath.substringBeforeLast("/")

    private var canvasWidth = 0f
    private var canvasHeight = 0f
    private var pixelsPerUnit = 1f
    private var viewportWidth = 0
    private var viewportHeight = 0

    private val glTextures = mutableMapOf<Int, Int>()

    // 参数索引缓存（按名称查找）
    private var paramNameToIndex: Map<String, Int> = emptyMap()
    private var paramAIndex = -1

    // 每帧 Drawable 渲染数据
    private var drawables: List<DrawableRenderData> = Collections.emptyList()

    // 渲染矩阵
    private val projectionMatrix = FloatArray(16)

    // ── Purism Core 模型包装 ──
    private var purismModel: PurismModel? = null

    // ── 着色器管理 ──
    private var shaderManager: ShaderManager? = null

    // ── 每个 drawable 的属性数组缓存（避免每帧重新获取） ──
    private var cachedTextureIndices: IntArray? = null
    private var cachedRenderOrders: IntArray? = null
    private var cachedOpacities: FloatArray? = null
    private var cachedVertexCounts: IntArray? = null
    private var cachedIndexCounts: IntArray? = null
    private var cachedConstantFlags: ByteArray? = null
    private var cachedDynamicFlags: ByteArray? = null
    private var cachedMultiplyColors: FloatArray? = null
    private var cachedScreenColors: FloatArray? = null
    private var cachedMaskCounts: IntArray? = null

    // ── 动画管线 ──
    private var animationPipeline: AnimationPipeline? = null

    /** 预加载的 Motion 数据 [(group)_(index) → MotionData] */
    private val loadedMotions: MutableMap<String, MotionData> = mutableMapOf()

    /** 预加载的表情数据 [name → ExpressionData] */
    private val loadedExpressions: MutableMap<String, ExpressionData> = mutableMapOf()

    /** 部件名到索引的映射 */
    private var partNameToIndex: Map<String, Int> = emptyMap()

    /** 模型元配置（从 cdi3.json + pose3.json 解析） */
    private var modelMeta: ModelMetaConfig? = null

    /** 上一帧时间戳（纳秒） */
    private var previousFrameNano: Long = System.nanoTime()

    /** 当前 deltaTime（秒） */
    private var deltaTime: Float = 0.016f

    /** 标记是否为第一帧 */
    private var isFirstFrame: Boolean = true

    /** 是否已 start idle motion */
    private var hasStartedIdle: Boolean = false

    // ── 兜底渲染 ──
    private var fallbackProgram = 0
    private val fallbackVertices = floatArrayOf(-1f, -1f, 0f, 1f, -1f, 0f, -1f, 1f, 0f, 1f, 1f, 0f)
    private var fallbackVbo = IntArray(1)
    private var time = 0f

    // ============================================================
    //  GLSurfaceView.Renderer 接口
    // ============================================================

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        GLES20.glClearColor(0f, 0f, 0f, 0f)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glBlendFunc(GLES20.GL_SRC_ALPHA, GLES20.GL_ONE_MINUS_SRC_ALPHA)
        GLES20.glEnable(GLES20.GL_CULL_FACE)
        GLES20.glFrontFace(GLES20.GL_CW)

        nativeAvailable = Live2DNative.load()

        if (nativeAvailable) {
            try {
                // 初始化 ShaderManager
                shaderManager = ShaderManager(context)
                shaderManager!!.initialize()
                android.util.Log.i("Live2DRenderer", "ShaderManager initialized")

                // 加载模型
                loadModel()
            } catch (e: Throwable) {
                android.util.Log.w("Live2DRenderer", "初始化失败: ${e.message}")
                e.printStackTrace()
                nativeAvailable = false
                fallbackActive = true
            }
        } else {
            fallbackActive = true
        }

        if (fallbackActive) initFallbackShader()
    }

    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
        viewportWidth = width
        viewportHeight = height
        GLES20.glViewport(0, 0, width, height)

        val viewAspect = width.toFloat() / height.toFloat()
        val ppu = if (pixelsPerUnit > 0f) pixelsPerUnit else 1f
        // Canvas half-size in model units (pixels / pixelsPerUnit)
        val halfW = canvasWidth / 2f
        val halfH = canvasHeight / 2f
        val canvasAspect = if (halfH > 0f) halfW / halfH else 1f

        // 模型坐标以原点为中心，范围约[-halfW, halfW]×[-halfH, halfH]
        val viewHalfH = if (canvasAspect > viewAspect) halfW / viewAspect else halfH
        val viewHalfW = if (canvasAspect > viewAspect) halfW else halfH * viewAspect
        Matrix.orthoM(projectionMatrix, 0, -viewHalfW, viewHalfW, -viewHalfH, viewHalfH, -1f, 1f)
    }

    override fun onDrawFrame(gl: GL10?) {
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)

        if (fallbackActive) { drawFallback(); return }

        if (nativeAvailable && modelPtr != 0L) {
            try {
                val model = purismModel ?: run { drawFallback(); return }
                val shaderMgr = shaderManager ?: run { drawFallback(); return }

                // 计算 delta time
                val now = System.nanoTime()
                if (!isFirstFrame) {
                    deltaTime = ((now - previousFrameNano) / 1_000_000_000f).coerceIn(0.001f, 0.1f)
                }
                previousFrameNano = now

                if (isFirstFrame) {
                    val pipeline = animationPipeline
                    if (pipeline != null) {
                        pipeline.createInitialSnapshot()
                    }
                    if (!hasStartedIdle) {
                        startIdleMotion()
                        hasStartedIdle = true
                    }
                    isFirstFrame = false
                }

                // ── 动画更新管线 ──
                val pipeline = animationPipeline
                if (pipeline != null) {
                    pipeline.update(deltaTime)
                } else {
                    model.update()
                }

                // 刷新 drawable 批量属性缓存
                refreshCachedAttributes()

                // 构建 DrawableRenderData 列表
                buildDrawableList()

                // preDraw() — 重置 OpenGL 状态
                preDraw()

                // drawObjectLoop() — 主渲染循环
                drawObjectLoop(shaderMgr)

                // 清除状态
                GLES20.glUseProgram(0)
                GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
            } catch (e: Throwable) {
                android.util.Log.w("Live2DRenderer", "渲染异常: ${e.message}")
                e.printStackTrace()
                drawFallback()
            }
        } else {
            drawFallback()
        }
    }

    // ============================================================
    //  公开 API（Live2DView 兼容接口）
    // ============================================================

    fun startMotion(motionGroup: String, motionIndex: Int) {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return
        try {
            // 优先使用预加载的 motion
            val key = "${motionGroup}_$motionIndex"
            val motionData = loadedMotions[key]
            if (motionData != null) {
                val pipeline = animationPipeline
                if (pipeline != null) {
                    val priority = when (motionGroup) {
                        "Idle" -> MotionPriority.IDLE
                        else -> MotionPriority.NORMAL
                    }
                    pipeline.motionManager.startMotion(motionData, priority)
                    return
                }
            }

            // 兜底：使用旧的 applyMotionFromJson
            val setting = modelSettingJson!!
            val motions = setting.getJSONObject("FileReferences").getJSONObject("Motions")
            val motionArray = motions.optJSONArray(motionGroup) ?: return
            if (motionIndex >= motionArray.length()) return
            val motionFile = motionArray.getJSONObject(motionIndex).optString("File") ?: return

            val path = "$modelDir/$motionFile"
            val bytes = context.assets.open(path).use { it.readBytes() }
            applyMotionFromJson(String(bytes))
        } catch (e: Throwable) {
            android.util.Log.w("Live2DRenderer", "动作播放失败: ${e.message}")
        }
    }

    fun setExpression(expressionIndex: Int) {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return
        try {
            val pipeline = animationPipeline
            val setting = modelSettingJson!!
            val expressions = setting.getJSONObject("FileReferences").optJSONArray("Expressions") ?: run {
                setExpressionFallback(expressionIndex)
                return
            }
            if (expressionIndex >= expressions.length()) {
                setExpressionFallback(expressionIndex)
                return
            }

            // 尝试使用预加载的表情
            val expFileName = expressions.getJSONObject(expressionIndex).optString("File", "")
            val expName = expressions.getJSONObject(expressionIndex).optString("Name", "")

            // 按名称查找
            var expData = loadedExpressions[expName]
            // 按文件名查找
            if (expData == null && expFileName.isNotEmpty()) {
                val nameFromFile = expFileName.substringAfterLast("/").substringBeforeLast(".")
                expData = loadedExpressions[nameFromFile]
            }

            if (expData != null && pipeline != null) {
                pipeline.expressionManager.setExpression(expData)
                return
            }

            // 兜底：使用旧的 applyExpressionFromJson
            val expFile = expressions.getJSONObject(expressionIndex).optString("File") ?: return
            val path = "$modelDir/$expFile"
            val bytes = context.assets.open(path).use { it.readBytes() }
            applyExpressionFromJson(String(bytes))
        } catch (e: Throwable) {
            setExpressionFallback(expressionIndex)
        }
    }

    fun setLipSyncParam(value: Float) {
        if (!nativeAvailable || modelPtr == 0L) return
        try {
            val pipeline = animationPipeline
            if (pipeline != null) {
                pipeline.setLipSyncValue(value)
            } else if (paramAIndex >= 0) {
                Live2DNative.setParameterValue(modelPtr, paramAIndex, value.coerceIn(0f, 1f))
            }
        } catch (_: Throwable) { }
    }

    fun onPause() {}
    fun onResume() {}

    fun dispose() {
        // 释放动画管线
        animationPipeline = null
        loadedMotions.clear()
        loadedExpressions.clear()

        // 释放 native 模型
        if (modelPtr != 0L) {
            try { Live2DNative.freeModel(modelPtr) } catch (_: Throwable) { }
            modelPtr = 0L
        }
        // 释放 ShaderManager
        shaderManager?.dispose()
        shaderManager = null
        purismModel = null
        // 释放 fallback
        if (fallbackProgram != 0) {
            GLES20.glDeleteProgram(fallbackProgram)
            fallbackProgram = 0
        }
        // 释放纹理
        for (id in glTextures.values) GLES20.glDeleteTextures(1, intArrayOf(id), 0)
        glTextures.clear()
    }

    // ============================================================
    //  模型加载
    // ============================================================

    private fun loadModel() {
        // 1. 解析 model3.json
        val modelBytes = context.assets.open(modelAssetPath).use { it.readBytes() }
        val jsonStr = modelBytes.toString(Charsets.UTF_8)
        modelSettingJson = JSONObject(jsonStr)
        val setting = modelSettingJson!!

        // 2. 加载 MOC3
        val mocFile = setting.getJSONObject("FileReferences").getString("Moc")
        val mocPath = "$modelDir/$mocFile"
        val mocBytes = context.assets.open(mocPath).use { it.readBytes() }

        modelPtr = Live2DNative.loadModel(mocBytes)
        if (modelPtr == 0L) throw RuntimeException("Native 模型加载失败")

        // 3. 画布信息
        Live2DNative.getCanvasInfo(modelPtr)?.let {
            canvasWidth = it[0] / it.getOrElse(4) { 1f }
            canvasHeight = it[1] / it.getOrElse(4) { 1f }
            pixelsPerUnit = it.getOrElse(4) { 1f }
        }
        android.util.Log.i("Live2DRenderer",
            "Canvas: ${canvasWidth}x${canvasHeight} (model units) ppu=$pixelsPerUnit")

        // 4. 加载纹理
        loadTextures()

        // 5. 缓存参数索引
        buildParameterIndex()

        // 6. 解析模型元配置（cdi3.json + pose3.json）
        loadModelMetaConfig()

        // 7. 初始化参数（使用元配置替代硬编码）
        initParameters(modelMeta)

        // 8. 初始更新
        Live2DNative.updateModel(modelPtr)

        // 9. 创建 PurismModel（动画系统需要）
        purismModel = PurismModel(modelPtr)

        // 10. 构建部件名到索引的映射（必须在 setupAnimationSystem 之前，Pose 需要）
        buildPartIndex()

        // 11. 加载动画系统（传入元配置）
        setupAnimationSystem(modelMeta)
    }

    private fun loadTextures() {
        val setting = modelSettingJson ?: return
        val textures = setting.getJSONObject("FileReferences").getJSONArray("Textures")
        for (i in 0 until textures.length()) {
            try {
                val texFile = textures.getString(i)
                val path = "$modelDir/$texFile"
                val bitmap = context.assets.open(path).use { BitmapFactory.decodeStream(it) } ?: continue

                val texIds = IntArray(1)
                GLES20.glGenTextures(1, texIds, 0)
                GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texIds[0])
                GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR_MIPMAP_LINEAR)
                GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
                GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
                GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
                GLUtils.texImage2D(GLES20.GL_TEXTURE_2D, 0, bitmap, 0)
                GLES20.glGenerateMipmap(GLES20.GL_TEXTURE_2D)
                bitmap.recycle()
                glTextures[i] = texIds[0]
            } catch (e: Exception) {
                android.util.Log.w("Live2DRenderer", "纹理 $i 加载失败: ${e.message}")
            }
        }
    }

    private fun buildParameterIndex() {
        val ids = Live2DNative.getParameterIds(modelPtr) ?: return
        val map = mutableMapOf<String, Int>()
        for (i in ids.indices) {
            map[ids[i]] = i
            if (ids[i] == "ParamA") paramAIndex = i
        }
        paramNameToIndex = map
    }

    private fun buildPartIndex() {
        val ids = Live2DNative.getPartIds(modelPtr) ?: return
        val map = mutableMapOf<String, Int>()
        for (i in ids.indices) {
            map[ids[i]] = i
        }
        partNameToIndex = map
    }

    /**
     * 初始化参数为默认值，使用模型元配置替代硬编码的字符串比较。
     *
     * 元配置优先：如果 modelMeta 可用，用语义索引设置眼睛参数。
     * 兜底：尝试旧的硬编码参数名（旧模型向后兼容）。
     */
    private fun initParameters(meta: ModelMetaConfig?) {
        // 参数设为默认值
        val defaults = Live2DNative.getParameterDefaultValues(modelPtr)
        if (defaults != null) {
            for (i in defaults.indices) {
                Live2DNative.setParameterValue(modelPtr, i, defaults[i])
            }
        }
        // ★ 不移除 part opacities — 让 MOC3 提供的初始值控制部件可见性
        // 之前这里把所有 part 设为 1.0，导致后臂（PartArmLB/PartArmRB）本应不可见却显示

        // 设置关键参数为正常显示值
        // ── 优先使用元配置语义索引 ──
        if (meta != null) {
            val si = meta.semantic
            if (si.eyeLeftOpenIdx >= 0) {
                Live2DNative.setParameterValue(modelPtr, si.eyeLeftOpenIdx, 1f)
                android.util.Log.i("Live2DRenderer",
                    "initParameters: eyeLeftOpen=${meta.parameters.find { it.index == si.eyeLeftOpenIdx }?.id}(idx=${si.eyeLeftOpenIdx}) = 1.0")
            }
            if (si.eyeRightOpenIdx >= 0) {
                Live2DNative.setParameterValue(modelPtr, si.eyeRightOpenIdx, 1f)
                android.util.Log.i("Live2DRenderer",
                    "initParameters: eyeRightOpen=${meta.parameters.find { it.index == si.eyeRightOpenIdx }?.id}(idx=${si.eyeRightOpenIdx}) = 1.0")
            }
        } else {
            // ── 兜底：尝试硬编码的名称（旧模型向后兼容） ──
            val ids = Live2DNative.getParameterIds(modelPtr)
            if (ids != null) {
                for (i in ids.indices) {
                    if (ids[i] == "PARAM_EYE_L_OPEN" || ids[i] == "PARAM_EYE_R_OPEN") {
                        Live2DNative.setParameterValue(modelPtr, i, 1f)
                    }
                }
            }
        }

        // Debug: 打印初始 part opacity
        val partIds = Live2DNative.getPartIds(modelPtr)
        val partOpacities = Live2DNative.getPartOpacities(modelPtr)
        if (partIds != null && partOpacities != null) {
            for (i in partIds.indices) {
                if (partIds[i].contains("Arm")) {
                    android.util.Log.i("Live2DRenderer", "Initial part: ${partIds[i]} = ${partOpacities[i]}")
                }
            }
        }
    }

    // ============================================================
    //  每帧数据刷新
    // ============================================================

    /**
     * 批量获取所有 drawable 的公用属性数组（避免每帧逐个调用 JNI）。
     */
    private fun refreshCachedAttributes() {
        cachedTextureIndices = Live2DNative.getDrawableTextureIndices(modelPtr)
        cachedRenderOrders = Live2DNative.getRenderOrders(modelPtr)
            ?: Live2DNative.getDrawableDrawOrders(modelPtr)
        cachedOpacities = Live2DNative.getDrawableOpacities(modelPtr)
        cachedVertexCounts = Live2DNative.getDrawableVertexCounts(modelPtr)
        cachedIndexCounts = Live2DNative.getDrawableIndexCounts(modelPtr)
        cachedConstantFlags = Live2DNative.getDrawableConstantFlags(modelPtr)
        cachedDynamicFlags = Live2DNative.getDrawableDynamicFlags(modelPtr)
        cachedMultiplyColors = Live2DNative.getDrawableMultiplyColors(modelPtr)
        cachedScreenColors = Live2DNative.getDrawableScreenColors(modelPtr)
        cachedMaskCounts = Live2DNative.getDrawableMaskCounts(modelPtr)
    }

    // ============================================================
    //  构建 DrawableRenderData 列表（每帧）
    // ============================================================

    /**
     * 从 PurismModel 获取所有 drawable 数据，构建 DrawableRenderData 列表。
     *
     * 对应 CubismRendererAndroid 中 drawObjectLoop 前获取 getModel().getRenderOrders()
     * 和 drawable 数据的过程。
     *
     * 顶点坐标从 Purism Core 返回的是 canvas 像素值，
     * PurismModel.getDrawableVertices 已自动除以 pixelsPerUnit 转换为模型单位。
     */
    private fun buildDrawableList() {
        val count = Live2DNative.getDrawableCount(modelPtr)
        if (count == 0) { drawables = Collections.emptyList(); return }

        val texIndices = cachedTextureIndices ?: return
        val orders = cachedRenderOrders ?: return
        val opacities = cachedOpacities ?: return
        val vertexCounts = cachedVertexCounts ?: return
        val indexCounts = cachedIndexCounts ?: return
        val constFlags = cachedConstantFlags ?: return
        val mulColors = cachedMultiplyColors ?: return
        val scrColors = cachedScreenColors ?: return
        val dynamicFlags = cachedDynamicFlags ?: return
        val maskCounts = cachedMaskCounts ?: return

        val list = mutableListOf<DrawableRenderData>()
        for (i in 0 until count) {
            // 从 PurismModel 获取顶点数据（已除以 ppu）
            val model = purismModel ?: continue
            val pos = model.getDrawableVertices(i)
            val uvs = model.getDrawableVertexUvs(i)
            val idx = model.getDrawableVertexIndices(i)
            val vc = vertexCounts.getOrElse(i) { 0 }
            val ic = indexCounts.getOrElse(i) { 0 }

            if (vc <= 0 || ic <= 0) continue

            // 创建 NIO 缓冲（直接字节序）
            val pb = ByteBuffer.allocateDirect(vc * 2 * 4)
                .order(ByteOrder.nativeOrder()).asFloatBuffer()
            pb.put(pos, 0, vc * 2); pb.position(0)

            val ub = ByteBuffer.allocateDirect(vc * 2 * 4)
                .order(ByteOrder.nativeOrder()).asFloatBuffer()
            ub.put(uvs, 0, vc * 2); ub.position(0)

            val ib = ByteBuffer.allocateDirect(ic * 2)
                .order(ByteOrder.nativeOrder()).asShortBuffer()
            ib.put(idx, 0, ic); ib.position(0)

            val f = constFlags.getOrElse(i) { 0 }.toInt() and 0xFF
            val dflags = dynamicFlags.getOrElse(i) { 0 }.toInt() and 0xFF

            // 获取 mask 索引
            var maskIds = IntArray(0)
            val mc = maskCounts.getOrElse(i) { 0 }
            if (mc > 0) {
                maskIds = Live2DNative.getDrawableMasks(modelPtr, i) ?: IntArray(0)
            }

            list.add(DrawableRenderData(
                index = i,
                textureIndex = texIndices.getOrElse(i) { 0 },
                renderOrder = orders.getOrElse(i) { 0 },
                vertexCount = vc,
                indexCount = ic,
                positionsBuffer = pb,
                uvsBuffer = ub,
                indicesBuffer = ib,
                opacity = opacities.getOrElse(i) { 1f },
                multiplyColor = floatArrayOf(
                    mulColors.getOrElse(i * 4) { 1f },
                    mulColors.getOrElse(i * 4 + 1) { 1f },
                    mulColors.getOrElse(i * 4 + 2) { 1f },
                    mulColors.getOrElse(i * 4 + 3) { 1f }
                ),
                screenColor = floatArrayOf(
                    scrColors.getOrElse(i * 4) { 0f },
                    scrColors.getOrElse(i * 4 + 1) { 0f },
                    scrColors.getOrElse(i * 4 + 2) { 0f },
                    scrColors.getOrElse(i * 4 + 3) { 0f }
                ),
                isDoubleSided = (f and Live2DNative.FLAG_DOUBLE_SIDED) != 0,
                isInvertedMask = (f and Live2DNative.FLAG_INVERTED_MASK) != 0,
                blendMode = if ((f and Live2DNative.FLAG_ADDITIVE) != 0) 1
                    else if ((f and Live2DNative.FLAG_MULTIPLICATIVE) != 0) 2
                    else 0,
                maskCount = mc,
                maskIndices = maskIds,
                isVisible = (dflags and Live2DNative.DYNAMIC_IS_VISIBLE) != 0
            ))
        }
        drawables = list
    }

    // ============================================================
    //  PreDraw — 重置 OpenGL 状态
    // ============================================================

    /**
     * 对应 CubismRendererAndroid.preDraw()。
     *
     * 在每帧主绘制前重置 OpenGL 状态：
     * - 禁用裁切、模板、深度测试
     * - 启用混合
     * - 全通道写入
     * - 解绑 GL 缓冲区
     */
    private fun preDraw() {
        GLES20.glDisable(GLES20.GL_SCISSOR_TEST)
        GLES20.glDisable(GLES20.GL_STENCIL_TEST)
        GLES20.glDisable(GLES20.GL_DEPTH_TEST)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glColorMask(true, true, true, true)
        GLES20.glBindBuffer(GLES20.GL_ELEMENT_ARRAY_BUFFER, 0)
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, 0)
    }

    // ============================================================
    //  DrawObjectLoop — 主渲染循环
    // ============================================================

    /**
     * 主渲染循环 — 对应 CubismRendererAndroid.drawObjectLoop()。
     *
     * 流程：
     * 1. 按 renderOrder 排序 drawable
     * 2. 遍历每个 drawable：
     *    a. 跳过不可见的
     *    b. 选择着色器类型
     *    c. 设置混合模式（预乘 alpha）
     *    d. 设置面剔除
     *    e. 绑定纹理，设置 uniforms
     *    f. glDrawElements
     *
     * 简化版本：跳过 offscreen 和掩码处理（当前模型掩码数据为 0）。
     */
    private fun drawObjectLoop(shaderMgr: ShaderManager) {
        if (drawables.isEmpty()) return

        // 按 renderOrder 排序
        val sortedDrawables = drawables.sortedBy { it.renderOrder }

        var currentShaderType: ShaderManager.ShaderType? = null
        var currentShader: ShaderManager.ShaderProgram? = null

        for (d in sortedDrawables) {
            // ★ 必须使用 dynamicFlagIsVisible（对应 CubismRendererAndroid）
            if (!d.isVisible) continue

            // 选择着色器类型
            val shaderType = selectShader(d.blendMode, d.isInvertedMask)

            // 仅在着色器类型变化时切换
            if (shaderType != currentShaderType) {
                val newShader = shaderMgr.get(shaderType)
                GLES20.glUseProgram(newShader.programId)
                // 设置 MVP 矩阵（一旦设置就跨 drawable 复用）
                GLES20.glUniformMatrix4fv(newShader.uMatrix, 1, false, projectionMatrix, 0)
                currentShaderType = shaderType
                currentShader = newShader
            }

            val shader = currentShader ?: continue

            // ── 混合模式（预乘 alpha） ──
            // 对应 CubismShaderAndroid.setupShaderProgramForDrawable 中的 blend 设置
            when (d.blendMode) {
                0 -> GLES20.glBlendFuncSeparate(
                    GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA,
                    GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA
                )
                1 -> GLES20.glBlendFuncSeparate(
                    GLES20.GL_ONE, GLES20.GL_ONE,
                    GLES20.GL_ZERO, GLES20.GL_ONE
                )
                2 -> GLES20.glBlendFunc(
                    GLES20.GL_ZERO, GLES20.GL_ONE_MINUS_SRC_COLOR)
            }

            // ── 面剔除 ──
            if (!d.isDoubleSided) {
                GLES20.glEnable(GLES20.GL_CULL_FACE)
            } else {
                GLES20.glDisable(GLES20.GL_CULL_FACE)
            }
            GLES20.glFrontFace(GLES20.GL_CCW)

            // ── 绑定主纹理 (s_texture0) ──
            val texId = glTextures[d.textureIndex]
            if (texId != null) {
                GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
                GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texId)
                GLES20.glUniform1i(shader.uTexture0, 0)
            }

            // ── 颜色 uniforms ──
            // baseColor: (1,1,1,opacity) — RGB 交由 multiply/screen 控制
            GLES20.glUniform4fv(shader.uBaseColor, 1, floatArrayOf(1f, 1f, 1f, d.opacity), 0)
            GLES20.glUniform4fv(shader.uMultiplyColor, 1, d.multiplyColor, 0)
            GLES20.glUniform4fv(shader.uScreenColor, 1, d.screenColor, 0)

            // ── 顶点属性 ──
            GLES20.glEnableVertexAttribArray(shader.aPosition)
            d.positionsBuffer.position(0)
            GLES20.glVertexAttribPointer(shader.aPosition, 2, GLES20.GL_FLOAT, false, 0, d.positionsBuffer)

            if (shader.aTexCoord >= 0) {
                GLES20.glEnableVertexAttribArray(shader.aTexCoord)
                d.uvsBuffer.position(0)
                GLES20.glVertexAttribPointer(shader.aTexCoord, 2, GLES20.GL_FLOAT, false, 0, d.uvsBuffer)
            }

            // ── 绘制 ──
            d.indicesBuffer.position(0)
            GLES20.glDrawElements(GLES20.GL_TRIANGLES, d.indexCount, GLES20.GL_UNSIGNED_SHORT, d.indicesBuffer)

            // 清理 vertex attrib 状态
            GLES20.glDisableVertexAttribArray(shader.aPosition)
            if (shader.aTexCoord >= 0) {
                GLES20.glDisableVertexAttribArray(shader.aTexCoord)
            }
        }

        // 恢复 OpenGL 默认状态
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBlendFuncSeparate(
            GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA,
            GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA
        )
        GLES20.glEnable(GLES20.GL_CULL_FACE)
    }

    // ============================================================
    //  着色器选择
    // ============================================================

    /**
     * 选择着色器类型。
     *
     * 对应 CubismShaderIndexCalculator.calculateShaderIndex() 的逻辑简化版。
     * 当前模型无 mask（maskCount == 0），所以始终返回 NORMAL。
     *
     * @param blendMode 0=normal, 1=additive, 2=multiplicative
     * @param isInvertedMask 是否反向 mask
     */
    private fun selectShader(blendMode: Int, isInvertedMask: Boolean): ShaderManager.ShaderType {
        // 当前模型无 mask，统一使用 NORMAL shader
        // blend mode 通过 glBlendFunc 控制，不需要切换 shader
        return ShaderManager.ShaderType.NORMAL
    }

    // ============================================================
    //  模型元配置加载 & 动画系统设置
    // ============================================================

    /**
     * 加载并解析 cdi3.json 配置文件，构建 ModelMetaConfig。
     */
    private fun loadModelMetaConfig() {
        val setting = modelSettingJson ?: return
        try {
            val fileRefs = setting.getJSONObject("FileReferences")
            val cdi3File = fileRefs.optString("DisplayInfo", "") ?: ""
            if (cdi3File.isEmpty()) {
                android.util.Log.i("Live2DRenderer", "No cdi3.json in model — using fallback hardcoded params")
                return
            }

            val cdi3Path = "$modelDir/$cdi3File"
            val cdi3Bytes = context.assets.open(cdi3Path).use { it.readBytes() }
            val cdi3Json = JSONObject(String(cdi3Bytes))

            // 加载 pose3.json（可选）
            val poseFile = fileRefs.optString("Pose", "") ?: ""
            val pose3Json = if (poseFile.isNotEmpty()) {
                try {
                    val posePath = "$modelDir/$poseFile"
                    val poseBytes = context.assets.open(posePath).use { it.readBytes() }
                    JSONObject(String(poseBytes))
                } catch (e: Exception) {
                    null
                }
            } else null

            modelMeta = ModelMetaConfig.parse(
                cdi3Json = cdi3Json,
                pose3Json = pose3Json,
                model3Json = setting,
                paramNameToIndex = paramNameToIndex,
                partNameToIndex = partNameToIndex
            )

            android.util.Log.i("Live2DRenderer", "ModelMetaConfig loaded (${modelMeta?.parameters?.size ?: 0} params)")
        } catch (e: Exception) {
            android.util.Log.w("Live2DRenderer", "Failed to load ModelMetaConfig: ${e.message}")
            modelMeta = null
        }
    }

    /**
     * 设置动画系统：加载所有 motion/expression，初始化 AnimationPipeline。
     *
     * @param meta 模型元配置（用于替代硬编码参数 ID 查找）
     */
    private fun setupAnimationSystem(meta: ModelMetaConfig?) {
        if (modelSettingJson == null || purismModel == null) return
        try {
            val setting = modelSettingJson!!
            val model = purismModel!!

            val loadFile: (String) -> ByteArray? = { path ->
                try {
                    context.assets.open(path).use { it.readBytes() }
                } catch (e: Exception) {
                    null
                }
            }

            // 加载所有 motions
            val motions = MotionLoader.loadAllMotions(setting, modelDir, loadFile)
            loadedMotions.putAll(motions)

            // 加载所有 expressions
            val expressions = MotionLoader.loadAllExpressions(setting, modelDir, loadFile)
            loadedExpressions.putAll(expressions)

            android.util.Log.i("Live2DRenderer", "Loaded ${loadedMotions.size} motions, ${loadedExpressions.size} expressions")

            // 创建 AnimationPipeline（传入 meta 替代硬编码参数 ID）
            animationPipeline = AnimationPipeline(model, paramNameToIndex, meta, partNameToIndex)

            // 用 meta 初始化 BreathManager（如果可用）
            if (meta != null && meta.semantic.breathParams.isNotEmpty()) {
                animationPipeline?.breath?.initialize(meta)
                android.util.Log.i("Live2DRenderer",
                    "BreathManager initialized from meta: ${meta.semantic.breathParams.size} params")
            }

            // 初始化物理
            try {
                val fileRefs = setting.getJSONObject("FileReferences")
                val physicsFile = fileRefs.optString("Physics", "") ?: ""
                if (physicsFile.isNotEmpty()) {
                    val physicsPath = "$modelDir/$physicsFile"
                    val physicsBytes = loadFile(physicsPath)
                    if (physicsBytes != null) {
                        animationPipeline?.physics?.initialize(physicsBytes)
                        android.util.Log.i("Live2DRenderer", "Physics loaded from $physicsFile")
                    }
                }
            } catch (e: Exception) {
                android.util.Log.w("Live2DRenderer", "Physics load skipped: ${e.message}")
            }

            // 初始化 Pose
            try {
                val fileRefs = setting.getJSONObject("FileReferences")
                val poseFile = fileRefs.optString("Pose", "") ?: ""
                if (poseFile.isNotEmpty()) {
                    val posePath = "$modelDir/$poseFile"
                    val poseBytes = loadFile(posePath)
                    if (poseBytes != null) {
                        val partIds = Live2DNative.getPartIds(modelPtr)
                        animationPipeline?.pose?.initialize(poseBytes, partIds, meta)
                        android.util.Log.i("Live2DRenderer", "Pose loaded from $poseFile (meta=${meta != null})")
                    }
                }
            } catch (e: Exception) {
                android.util.Log.w("Live2DRenderer", "Pose load skipped: ${e.message}")
            }

            android.util.Log.i("Live2DRenderer", "Animation system initialized")
        } catch (e: Exception) {
            android.util.Log.w("Live2DRenderer", "Failed to setup animation system: ${e.message}")
        }
    }

    /**
     * 启动 IDLE motion。
     * 如果没有预加载的 idle motion，不执行任何操作。
     */
    private fun startIdleMotion() {
        val pipeline = animationPipeline ?: return

        // 查找并启动第一个 IDLE motion
        var idleStarted = false
        for ((key, motionData) in loadedMotions) {
            if (key.startsWith("Idle_")) {
                android.util.Log.i("Live2DRenderer", "Starting idle motion: $key")
                pipeline.motionManager.startMotion(motionData, MotionPriority.IDLE)
                idleStarted = true
                break
            }
        }

        // 如果没找到 Idle 开头，尝试索引 0 的 motion
        if (!idleStarted && loadedMotions.isNotEmpty()) {
            // 启动第一个 motion（按字母序最前面的）
            val firstKey = loadedMotions.keys.first()
            val firstMotion = loadedMotions[firstKey]
            if (firstMotion != null) {
                android.util.Log.i("Live2DRenderer", "Starting first motion: $firstKey")
                pipeline.motionManager.startMotion(firstMotion, MotionPriority.IDLE)
            }
        }
    }

    // ============================================================
    //  动作曲线解析（保持原实现）
    // ============================================================

    /**
     * 从 motion3.json 解析参数曲线并应用到模型。
     */
    private fun applyMotionFromJson(json: String) {
        if (modelPtr == 0L) return
        try {
            val root = JSONObject(json)
            val meta = root.getJSONObject("Meta")
            val duration = meta.optDouble("Duration", 1.0)
            val curves = root.getJSONArray("Curves")

            val defaults = Live2DNative.getParameterDefaultValues(modelPtr) ?: return
            for (i in 0 until defaults.size) {
                Live2DNative.setParameterValue(modelPtr, i, defaults[i])
            }

            val sampleTime = duration * 0.3
            for (ci in 0 until curves.length()) {
                val curve = curves.getJSONObject(ci)
                if (curve.optString("Target", "") != "Parameter") continue
                val paramId = curve.getString("Id")
                val segments = curve.getJSONArray("Segments")

                val value = evaluateCurve(segments, sampleTime)
                if (!value.isNaN()) {
                    val idx = paramNameToIndex[paramId] ?: continue
                    Live2DNative.setParameterValue(modelPtr, idx, value.toFloat())
                }
            }

            Live2DNative.updateModel(modelPtr)
        } catch (e: Exception) {
            android.util.Log.w("Live2DRenderer", "解析动作 JSON 失败: ${e.message}")
        }
    }

    /**
     * 从 exp3.json 解析表情参数并应用到模型。
     */
    private fun applyExpressionFromJson(json: String) {
        if (modelPtr == 0L) return
        try {
            val root = JSONObject(json)
            val params = root.getJSONArray("Parameters")

            for (i in 0 until params.length()) {
                val p = params.getJSONObject(i)
                val paramId = p.getString("Id")
                val value = p.getDouble("Value").toFloat()
                val blend = p.optString("Blend", "Add")

                val idx = paramNameToIndex[paramId] ?: continue
                val current = Live2DNative.getParameterValues(modelPtr)?.getOrNull(idx) ?: continue

                val newValue = when (blend) {
                    "Add" -> (current + value).coerceIn(
                        Live2DNative.getParameterMinValues(modelPtr)?.getOrElse(idx) { -1f } ?: -1f,
                        Live2DNative.getParameterMaxValues(modelPtr)?.getOrElse(idx) { 1f } ?: 1f
                    )
                    "Multiply" -> current * (1f + value)
                    "Overwrite" -> value
                    else -> value
                }
                Live2DNative.setParameterValue(modelPtr, idx, newValue)
            }

            Live2DNative.updateModel(modelPtr)
        } catch (e: Exception) {
            android.util.Log.w("Live2DRenderer", "解析表情 JSON 失败: ${e.message}")
        }
    }

    /**
     * 兜底表情设置。
     */
    private fun setExpressionFallback(expressionIndex: Int) {
        val mouthOpen = when (expressionIndex) {
            0 -> 0.0f; 1 -> 0.3f; 2 -> 0.5f; 3 -> 0.8f
            4 -> 0.2f; 5 -> 0.4f; 6 -> 0.6f; 7 -> 0.9f
            else -> 0.0f
        }
        if (paramAIndex >= 0) {
            Live2DNative.setParameterValue(modelPtr, paramAIndex, mouthOpen)
            Live2DNative.updateModel(modelPtr)
        }
    }

    /**
     * 评估运动曲线在时间 t 的值。
     */
    private fun evaluateCurve(segments: org.json.JSONArray, time: Double): Double {
        if (segments.length() < 2) return Double.NaN

        var prevT = segments.getDouble(0)
        var prevV = segments.getDouble(1)
        var i = 2

        while (i < segments.length()) {
            val type = segments.getInt(i); i++

            when (type) {
                0 -> { // Linear
                    if (i + 1 >= segments.length()) break
                    val t1 = segments.getDouble(i); val v1 = segments.getDouble(i + 1)
                    i += 2
                    if (time in prevT..t1 || time in t1..prevT) {
                        val ratio = if (t1 != prevT) (time - prevT) / (t1 - prevT) else 0.0
                        return prevV + (v1 - prevV) * ratio
                    }
                    prevT = t1; prevV = v1
                }
                1 -> { // Bezier
                    if (i + 5 >= segments.length()) break
                    val c1x = segments.getDouble(i); val c1y = segments.getDouble(i + 1)
                    val c2x = segments.getDouble(i + 2); val c2y = segments.getDouble(i + 3)
                    val t1 = segments.getDouble(i + 4); val v1 = segments.getDouble(i + 5)
                    i += 6
                    if (time in prevT..t1 || time in t1..prevT) {
                        val ratio = if (t1 != prevT) (time - prevT) / (t1 - prevT) else 0.0
                        return prevV + (v1 - prevV) * ratio
                    }
                    prevT = t1; prevV = v1
                }
                2 -> { // Stepped
                    if (i >= segments.length()) break
                    val t1 = segments.getDouble(i); i++
                    if (time < t1) return prevV
                    prevT = t1
                }
            }
        }
        return prevV
    }

    // ============================================================
    //  兜底渲染
    // ============================================================

    private fun initFallbackShader() {
        val vs = "attribute vec4 aPosition; varying vec2 vPosition; void main(){gl_Position=aPosition;vPosition=aPosition.xy;}"
        val fs = "precision mediump float;varying vec2 vPosition;uniform float uTime;void main(){vec2 p=vPosition*0.8;float r=0.1+0.15*sin(uTime*0.5+p.x*2.0);float g=0.1+0.15*sin(uTime*0.7+p.y*2.0+1.0);float b=0.2+0.20*sin(uTime*0.6+(p.x+p.y)*1.5+2.0);float a=0.3+0.1*sin(uTime*0.3);gl_FragColor=vec4(r,g,b,a);}"
        fallbackProgram = createProgram(vs, fs)
        fallbackVbo = IntArray(1)
        GLES20.glGenBuffers(1, fallbackVbo, 0)
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, fallbackVbo[0])
        val buf = ByteBuffer.allocateDirect(fallbackVertices.size * 4).order(ByteOrder.nativeOrder()).asFloatBuffer()
        buf.put(fallbackVertices); buf.position(0)
        GLES20.glBufferData(GLES20.GL_ARRAY_BUFFER, fallbackVertices.size * 4, buf, GLES20.GL_STATIC_DRAW)
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, 0)
    }

    private fun drawFallback() {
        time += 0.016f
        GLES20.glUseProgram(fallbackProgram)
        val tl = GLES20.glGetUniformLocation(fallbackProgram, "uTime")
        GLES20.glUniform1f(tl, time)
        val pl = GLES20.glGetAttribLocation(fallbackProgram, "aPosition")
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, fallbackVbo[0])
        GLES20.glEnableVertexAttribArray(pl)
        GLES20.glVertexAttribPointer(pl, 3, GLES20.GL_FLOAT, false, 0, 0)
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)
        GLES20.glDisableVertexAttribArray(pl)
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER, 0)
        GLES20.glUseProgram(0)
    }

    /**
     * 编译+链接着色器程序。
     */
    private fun createProgram(vs: String, fs: String): Int {
        val v = compileShader(GLES20.GL_VERTEX_SHADER, vs)
        val f = compileShader(GLES20.GL_FRAGMENT_SHADER, fs)
        val p = GLES20.glCreateProgram()
        if (p == 0) throw RuntimeException("glCreateProgram 返回 0")
        GLES20.glAttachShader(p, v); GLES20.glAttachShader(p, f); GLES20.glLinkProgram(p)
        val s = IntArray(1); GLES20.glGetProgramiv(p, GLES20.GL_LINK_STATUS, s, 0)
        if (s[0] == 0) throw RuntimeException("着色器链接失败: ${GLES20.glGetProgramInfoLog(p)}")
        return p
    }

    private fun compileShader(type: Int, src: String): Int {
        val s = GLES20.glCreateShader(type)
        if (s == 0) throw RuntimeException("glCreateShader 返回 0")
        GLES20.glShaderSource(s, src); GLES20.glCompileShader(s)
        val st = IntArray(1); GLES20.glGetShaderiv(s, GLES20.GL_COMPILE_STATUS, st, 0)
        if (st[0] == 0) {
            val log = GLES20.glGetShaderInfoLog(s)
            GLES20.glDeleteShader(s)
            throw RuntimeException("着色器编译失败: $log")
        }
        return s
    }
}
