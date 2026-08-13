package com.live2dai.android

import android.content.Context
import android.graphics.BitmapFactory
import android.opengl.GLES20
import android.opengl.GLSurfaceView
import android.opengl.GLUtils
import android.opengl.Matrix
import com.live2dai.android.data.BBox
import com.live2dai.android.data.CharacterFit
import com.live2dai.android.data.InteractionStore
import com.live2dai.android.data.Override
import com.live2dai.android.data.characterBBox
import com.live2dai.android.data.computeCharacterFit
import com.live2dai.android.data.computeDownsampleTarget
import com.live2dai.android.data.dragPxToModel
import com.live2dai.android.data.needsDownsample
import com.live2dai.android.data.pinchScale
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.nio.ShortBuffer
import java.util.Collections
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.abs
import kotlin.math.ceil

/** M7 idle 轮换间隔（毫秒；contract-M7-android-l3.md §5.2-3，锁默认 12s） */
private const val IDLE_SWITCH_INTERVAL_MS = 12_000L

/** C8h：超采样开关（可配置；帧率影响大时可置 false → 直渲，零行为变化） */
private const val SUPERSAMPLE_ENABLED = true

/**
 * C8h：超采样倍率（= 2K/1080p）：FBO 尺寸 = ceil(surface × SCALE)。
 * iQOO 11 Pro surface 1080×2400 × 4/3 = 1440×3200（物理 2K 同分辨率，填充 ≈1.78×）。
 * 用倍率而非硬编码 1440×3200：任意 surface 宽高比/方向下 FBO 与 surface 同宽高比
 * （投影矩阵只依赖宽高比 → 零形变，直渲/超采样逐像素同构）；目标设备上结果与
 * 任务常量（SUPERSAMPLE_W/H=1440/3200）逐位一致。
 */
private const val SUPERSAMPLE_SCALE = 4f / 3f

/** C7d：LipSync 口型让位目标参数（表情帧/表情管线在 TTS 播放期间跳过此参数，口型归 LipSync 独占） */
private const val MOUTH_OPEN_Y_ID = "ParamMouthOpenY"

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
    private var modelDir: String = modelAssetPath.substringBeforeLast("/")

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

    // ── 半身显示（K5：显示层相机调整，不改模型/rig，contract-v3-rename §2.1）──
    // 默认值 = 全模型（scale=1f, yOffset=0f）→ 零行为变化；由 UI/配置按需开启半身。
    private var displayScale = 1f
    private var displayYOffset = 0f

    // ── M1 角色 bbox contain-fit（contract-C-M1 §2.3）──
    // displayXOffset = 水平平移（模型单位，additive，默认 0 = 现状）；fitBBoxModel = 模型单位 bbox 缓存（load 时算一次）。
    private var displayXOffset = 0f
    private var fitBBoxModel: BBox? = null

    // ── C-M5 用户交互（contract-C-M5 §2.3）──
    // 优先级：用户 override > fit（M1）> 默认(1f, 0f, 0f)。
    // fitScale/fitXOffset/fitYOffset = M1 contain-fit 结果（仅无 override 时写状态）；
    // userScale/userXOffset/userYOffset = 用户调整值（null = 未调整 → 走 fit）；
    // displayScale/displayXOffset/displayYOffset = 生效值（user ?: fit）。
    private var fitScale = 1f
    private var fitXOffset = 0f
    private var fitYOffset = 0f
    private var userScale: Float? = null
    private var userXOffset: Float? = null
    private var userYOffset: Float? = null

    /** 交互持久化（每模型独立 key，contract-C-M5 §2.2；由 Live2DView 在 load/swap 时绑定 modelId） */
    private val interactionStore = InteractionStore(context)

    /** 当前模型 ID（override 存储 key） */
    private var currentModelId: String? = null

    /** 捏合手势基准（双指落下时捕获：当前生效 scale + 起始距离 px） */
    private var pinchBaseScale = 1f
    private var pinchStartDistancePx = 0f

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

    /** [C8f-probe] TEMPORARY: log out-of-envelope drawn drawables during frames 150-220. */
    private var c8fProbeFrame = 0
    private var c8fProbeArmed = false

    // ── C8h：超采样渲染（2K 离屏 FBO → 下采样 blit，解决 1080p 内容放大 1.33x 到物理 2K 的观感模糊）──
    // 原理：iQOO 11 Pro 物理屏 1440×3200（2K）但系统锁 1080×2400 显示模式（vivo 限制），
    // 1080p 内容被显示控制器放大 1.33x → 边缘模糊。方案：以 2K 离屏 FBO 渲染模型，
    // 再下采样 blit 回 1080p surface（GL_LINEAR 过滤）→ 1080p 缓冲内的边缘已抗锯齿，
    // 显示放大后观感显著锐利（GPU 富余：Adreno 740 2K 填充 ≈1.78×，C-Perf8 实测 2K 直渲 ~123fps）。

    /** 超采样 FBO id（0=未创建；onSurfaceChanged 重建） */
    private var supersampleFbo = 0

    /** 超采样颜色纹理 id（0=未创建；GL_RGBA + GL_LINEAR） */
    private var supersampleTexture = 0

    /** 超采样渲染尺寸（FBO 宽/高；FBO 绑定期间的 glViewport 尺寸） */
    private var supersampleRenderW = 0
    private var supersampleRenderH = 0

    /** 超采样实际生效（FBO 完整 + 尺寸合法；false → 直渲兜底，零行为变化） */
    private var supersampleActive = false

    /** 下采样 blit 着色器程序（0=未创建；EGL context 重建后经 onSurfaceCreated 置 0 惰性重建） */
    private var blitProgram = 0
    private var blitAPosition = -1
    private var blitATexCoord = -1
    private var blitUTexture = -1
    private var blitQuadBuffer: FloatBuffer? = null

    /** [C8h-probe] TEMPORARY: periodic FBO readback (every 300 frames) to verify 2K content vs blit output. */
    private var c8hProbeFrame = 0

    // ── C-Perf2：per-drawable DirectBuffer 对象池 ──
    // 消除每帧 250×3 次 ByteBuffer.allocateDirect（750 DirectBuffer/帧 → major GC 元凶）。
    // 每 drawable 常驻 3 个直接缓冲（positions/uvs/indices），容量按首帧顶点/索引数分配，
    // 后续帧 clear() 后覆盖写入（内容与新建 buffer 逐字节一致）；异常增长时按需扩容。
    // 池与模型生命周期绑定：releaseModelResources()/dispose() 清空。仅 GL 线程访问。
    private var pooledPositions: Array<FloatBuffer?> = emptyArray()
    private var pooledUvs: Array<FloatBuffer?> = emptyArray()
    private var pooledIndices: Array<ShortBuffer?> = emptyArray()

    // ── C-Perf2：静态 drawable 数据缓存（UV / 顶点索引）──
    // UV 与顶点索引在 Cubism 模型内恒定（不随参数/动画变化，仅顶点位置动态）：首帧经
    // model.getDrawableVertexUvs/getDrawableVertexIndices 获取后按 drawable 索引缓存，
    // 消除每帧 500 次 JNI 往返 + 500 个数组分配。顶点位置（getDrawableVertices）每帧变化，
    // 必须每帧重取，不缓存。
    private var cachedUvsData: Array<FloatArray?> = emptyArray()
    private var cachedIndicesData: Array<ShortArray?> = emptyArray()

    // ── C8b：light 帧顶点跳过 —— 池内位置缓冲已填充标记 ──
    // buildDrawableList 在 native light 帧（无输入变化 → 顶点冻结）跳过 getDrawableVertices
    // JNI 重取 + DirectBuffer 重写，直接复用池内上一 full 帧的内容（light 语义保证逐位一致）。
    // 该数组标记每个 drawable 的位置缓冲是否已被至少一次 full 帧填充过（未填充 → 必须重取，
    // 防首帧/换模型后池内容为空或串模型）。模型释放时随池一起清空。
    private var positionsPopulated = BooleanArray(0)

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

    // ── C-Perf4：模型动画更新降频（每 MODEL_UPDATE_INTERVAL_FRAMES 帧执行一次
    //    pipeline.update / csmUpdateModel；deltaTime 跨帧累积传递，动画速度
    //    （呼吸/眨眼/物理/motion）保持真实时间语义；渲染管线每帧执行）──

    /** 模型动画更新间隔（帧数）；主代理裁定 = 2（≈30Hz 模型动画 + 60Hz 渲染） */
    private val MODEL_UPDATE_INTERVAL_FRAMES = 2

    /** 降频帧计数：0..INTERVAL-1，到 INTERVAL-1 即执行一次 update */
    private var modelUpdateFrameCounter = 0

    /** 自上次 update 以来的真实时间累积（秒），在 update 帧一次性传给管线 */
    private var modelUpdateDeltaAccum = 0f

    // ── C-Perf6：GL 状态缓存（state cache）——仅去重"重复设置相同状态"的调用；
    //    blend/cull/frontFace/texture 按上次实际值脏检查，渲染语义（同帧绘制序/
    //    混合/裁剪）零变化；context 重建/纹理重载后经 resetGlStateCache() 失效 ──

    /** 上次实际设置的混合模式（-1=未知，首次必设） */
    private var cachedBlendMode = -1

    /** 上次实际设置的 GL_CULL_FACE 开关（null=未知） */
    private var cachedCullEnabled: Boolean? = null

    /** 上次实际设置的 glFrontFace 值（0=未知） */
    private var cachedFrontFace = 0

    /** 上次绑定到 TEXTURE0 的纹理 id（-1=未知） */
    private var cachedBoundTextureId = -1

    /** 已启用顶点属性数组的着色器类型（null=未启用；循环结束统一禁用） */
    private var attribsEnabledShaderType: ShaderManager.ShaderType? = null

    /** baseColor uniform 复用缓冲区（避免每 drawable 分配 FloatArray） */
    private val baseColorScratch = FloatArray(4)

    // ── C-Perf6：GL 调用计数器（诊断插桩；进程级单例，每 60 帧窗口打印平均/帧）──
    private object GlCounters {
        var drawElements = 0L
        var blendFunc = 0L
        var bindTexture = 0L
        var activeTexture = 0L
        var frontFace = 0L
        var cullFace = 0L
        var useProgram = 0L
        var uniform = 0L
        var vertexAttribPointer = 0L
        var attribArray = 0L
    }

    /** C-Perf6 诊断窗口累积（60 帧求平均打印一次，含 fps） */
    private var glCountWindowFrame = 0
    private var glCountWindowStartMs = 0L
    private var glCountWindowDraw = 0L
    private var glCountWindowBlend = 0L
    private var glCountWindowBindTex = 0L
    private var glCountWindowActiveTex = 0L
    private var glCountWindowFrontFace = 0L
    private var glCountWindowCullFace = 0L
    private var glCountWindowUseProgram = 0L
    private var glCountWindowUniform = 0L
    private var glCountWindowVap = 0L
    private var glCountWindowAttribArray = 0L

    /** 标记是否为第一帧 */
    private var isFirstFrame: Boolean = true

    /** 是否已 start idle motion */
    private var hasStartedIdle: Boolean = false

    // ── M7：motion 标签触发回落 + idle 多样化（contract-M7-android-l3.md §1.3-5 / §5）──

    /** 经 startMotionByTag 播放的 motion 回落标记（仅 NORMAL 优先级的 M7 触发路径生效） */
    private var triggeredMotionActive: Boolean = false

    /** 触发 motion 的时长（秒，播满一轮回落 idle 用） */
    private var triggeredMotionDuration: Float = 0f

    /** 触发 motion 已播时间（秒，每帧累积） */
    private var triggeredMotionElapsed: Float = 0f

    /** idle 轮换计时（秒，仅 IDLE 优先级播放时累积） */
    private var idleElapsed: Float = 0f

    /** idle 轮换器（懒初始化：DEFAULT_POOL ∩ loadedMotions；模型切换后重建） */
    private var idleSelector: IdleMotionSelector? = null

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

        // C-Perf6：context（重）建后 GL 状态回到驱动默认，状态缓存必须失效
        resetGlStateCache()

        // C8h：context（重）建 → 旧 FBO/纹理/blit program 的 GL id 全部失效。
        // 置 0 防用旧 id；FBO 由随后的 onSurfaceChanged（GLSurfaceView 每次 surface
        // 创建后必调，带真实尺寸）重建，blit program 在首次 blit 时惰性重建。
        supersampleFbo = 0
        supersampleTexture = 0
        supersampleRenderW = 0
        supersampleRenderH = 0
        supersampleActive = false
        blitProgram = 0
        blitQuadBuffer = null

        nativeAvailable = Live2DNative.load()

        if (nativeAvailable) {
            try {
                // 初始化 ShaderManager
                shaderManager = ShaderManager(context)
                shaderManager!!.initialize()
                android.util.Log.i("Live2DRenderer", "ShaderManager initialized")

                // 加载模型
                loadModel(modelAssetPath)
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

        // C8h：surface 尺寸变化 / EGL context 重建 → 重建超采样 FBO（尺寸按 surface × SCALE，
        // 失败自动回退直渲）。重建会把 TEXTURE0 绑定改写 → cachedBoundTextureId 置 -1（方法内）。
        recreateSupersampleFbo(width, height)

        // M1 contain-fit（contract-C-M1 §2.3，A6 时机 2）+ C-M5 §2.3 时机 2：
        // 视口变化 → 仅当无用户 override 才重算 fit；有 override → 保持 override 绝对值。
        // 随后按生效值（user ?: fit）经 computeProjection 落 orthoM。
        android.util.Log.i(
            "Live2DRenderer",
            "[C8e-probe] onSurfaceChanged viewport=${width}x${height} " +
                "canvas=${canvasWidth}x${canvasHeight} userOverride=${hasUserOverride()} " +
                "userScale=$userScale userX=$userXOffset userY=$userYOffset " +
                "fitScale=$fitScale fitX=$fitXOffset fitY=$fitYOffset"
        )
        applyFitAndProjection()
    }

    /**
     * 半身显示：缩放 + 垂直偏移（模型单位）（contract-v3-rename §2.1，K5 定案）。
     * 实现：修改投影矩阵的 viewHalfW/viewHalfH（scale>1 放大）+ 投影中心上移（裁切下半身，露出上半身）。
     * 默认值 = 现状（scale=1f, yOffset=0f）→ 零行为变化。
     */
    fun setDisplayRegion(scale: Float = 1f, yOffset: Float = 0f) {
        displayScale = scale
        displayYOffset = yOffset
        if (viewportWidth > 0 && viewportHeight > 0) {
            recomputeProjection()
        }
    }

    /** 按当前 displayScale/displayYOffset 重新计算投影矩阵（GL 线程调用）。 */
    private fun recomputeProjection() {
        applyFitAndProjection()
    }

    /**
     * M1 fit + C-M5 override 优先级下的投影落点（GL 线程）：
     * 视口变化/重算时，仅当无用户 override 才重算 fit（C-M5 §2.3 时机 2）；
     * 有 override → 保持 override 绝对值。随后按生效值经 computeProjection 落 orthoM。
     */
    private fun applyFitAndProjection() {
        val (viewHalfW, viewHalfH) = viewHalfExtents() ?: return
        if (!hasUserOverride()) {
            applyCharacterFitToState(viewHalfW, viewHalfH)
        }
        applyProjection()
    }

    /** 当前视口下的 viewHalfW/viewHalfH（模型单位；视口/画布未就绪 → null）。 */
    private fun viewHalfExtents(): Pair<Float, Float>? {
        if (viewportWidth <= 0 || viewportHeight <= 0 ||
            canvasWidth <= 0f || canvasHeight <= 0f
        ) return null
        val viewAspect = viewportWidth.toFloat() / viewportHeight.toFloat()
        val halfW = canvasWidth / 2f
        val halfH = canvasHeight / 2f
        val canvasAspect = if (halfH > 0f) halfW / halfH else 1f
        // 模型坐标以原点为中心，范围约[-halfW, halfW]×[-halfH, halfH]
        val viewHalfH = if (canvasAspect > viewAspect) halfW / viewAspect else halfH
        val viewHalfW = if (canvasAspect > viewAspect) halfW else halfH * viewAspect
        return viewHalfW to viewHalfH
    }

    /** 当前生效 display 状态的投影边界（模型单位 [left, right, top, bottom]；未就绪 → null）。 */
    private fun currentProjectionBounds(): FloatArray? {
        val (viewHalfW, viewHalfH) = viewHalfExtents() ?: return null
        // 半身显示（K5）+ M1 fit + C-M5 override：缩放 + 水平/垂直偏移，经 computeProjection 纯函数应用（contract-v3-rename §2.1）。
        // scale=1f, yOffset=0f, xOffset=0f（默认/退化）时 bounds == [-viewHalfW, viewHalfW, viewHalfH, -viewHalfH] → 与现状 orthoM 逐位一致，零回归。
        return computeProjection(
            displayScale, displayYOffset,
            viewHalfW * 2f, viewHalfH * 2f,
            canvasWidth, canvasHeight,
            displayXOffset
        )
    }

    /** 按生效值写投影矩阵（唯一 orthoM 调用点，参数序不动）。 */
    private fun applyProjection() {
        val bounds = currentProjectionBounds()
        // [C8e-probe] TEMPORARY: log projection state each call. Remove after capture.
        android.util.Log.i(
            "Live2DRenderer",
            "[C8e-probe] applyProjection viewport=${viewportWidth}x${viewportHeight} " +
                "canvas=${canvasWidth}x${canvasHeight} " +
                "displayScale=$displayScale displayXOffset=$displayXOffset displayYOffset=$displayYOffset " +
                "bounds=${bounds?.joinToString(",") ?: "NULL"} " +
                "fitScale=$fitScale fitXOffset=$fitXOffset fitYOffset=$fitYOffset " +
                "user=${userScale},${userXOffset},${userYOffset}"
        )
        if (bounds == null) return
        // Android Matrix.orthoM 签名: (m, offset, left, right, bottom, top, near, far)！
        // bounds=[left, right, top, bottom] → 第 5/6 参数必须传 bottom/top（bounds[3]/bounds[2]）。
        // 之前传 bounds[2]/bounds[3]（top/bottom）→ 投影 y 轴翻转 → 真机渲染上下颠倒（spoq-fix 真机修复）。
        Matrix.orthoM(projectionMatrix, 0, bounds[0], bounds[1], bounds[3], bounds[2], -1f, 1f)
    }

    /** 是否有用户 override（任一维度被调整过 → 覆盖 fit）。 */
    private fun hasUserOverride(): Boolean =
        userScale != null || userXOffset != null || userYOffset != null

    /** 生效值 = 用户 override ?: fit（C-M5 §2.3 优先级：用户 override > fit > 默认(1,0,0)）。 */
    private fun refreshEffectiveDisplay() {
        displayScale = userScale ?: fitScale
        displayXOffset = userXOffset ?: fitXOffset
        displayYOffset = userYOffset ?: fitYOffset
    }

    // ============================================================
    //  M1 角色 bbox contain-fit（contract-C-M1 §2.3）
    // ============================================================

    /**
     * load 完成后按运行时顶点聚合角色 bbox（模型单位），写入 fit 缓存。
     *
     * 数据源 = PurismModel.getDrawableVertices(i)（与 GL 缓冲同一份运行时顶点；Purism Core
     * 已返回模型单位坐标（范围约[-0.5,0.5]），**无需除 ppu / 乘 k**——k 换算是上上次 bug，
     * 架构 §3.0/§3.1）。参与规则（架构 §3.1，与 drawObjectLoop 可见性判定同源）：
     *   getDrawableDynamicFlagIsVisible(i) && getDrawableVertexCount(i) > 0 && getDrawableTextureIndex(i) >= 0
     * 聚合 = characterBBox(participating)（FitMath 纯函数，退化返回 null）→ fitBBoxModel = rawBBox（模型单位）。
     *
     * 与 PC（pixi-live2d-display 应用 Layout 缩放）不同：Android 渲染器（Purism）不读 Layout 不应用缩放
     * （G8 实证），渲染实际尺寸 = 原生顶点范围，故 fit 的 bbox 必须 = 运行时顶点聚合。曾误改 Layout 方案
     * （bbox=0.28×0.42 偏小）→ fit scale=3.39 → 真机"被大腿占满"；本修复回退运行时聚合（bbox≈0.57×1.39
     * → fit 后角色完整可见占屏 ~95%）。
     *
     * 退化（画布非法 / 无参与 drawable / 聚合 bbox 非法）→ fitBBoxModel=null（fit 不生效 = scale 1 现状），
     * logcat 告警。
     */
    private fun computeFitBBox() {
        fitBBoxModel = null
        val model = purismModel ?: run {
            android.util.Log.w("Live2DRenderer", "[fit] degenerate (no purism model)")
            return
        }
        if (canvasWidth <= 0f || canvasHeight <= 0f ||
            !canvasWidth.isFinite() || !canvasHeight.isFinite()
        ) {
            android.util.Log.w("Live2DRenderer", "[fit] degenerate (invalid canvas)")
            return
        }
        val participating = ArrayList<FloatArray>()
        val count = model.getDrawableCount()
        for (i in 0 until count) {
            if (!model.getDrawableDynamicFlagIsVisible(i)) continue
            if (model.getDrawableVertexCount(i) <= 0) continue
            if (model.getDrawableTextureIndex(i) < 0) continue
            participating.add(model.getDrawableVertices(i))
        }
        if (participating.isEmpty()) {
            android.util.Log.w("Live2DRenderer", "[fit] degenerate (no participating drawables)")
            return
        }
        val bbox = characterBBox(participating) ?: run {
            android.util.Log.w("Live2DRenderer", "[fit] degenerate (invalid aggregated bbox)")
            return
        }
        fitBBoxModel = bbox
    }

    /**
     * 按当前视口把 fit 缓存 bbox 换算成 contain-fit 值，写入 fitScale/fitXOffset/fitYOffset
     * 并经 [refreshEffectiveDisplay] 落到生效值（无 override 时 display* == fit*）。
     *
     * 退化（缓存空）→ 恢复默认（fit=scale 1/xOffset 0/yOffset 0 → display* 默认 = 现状，零行为变化）。
     * 非退化 → 输出 logcat 调试行（格式契约，供 E2E 机器对拍）：
     *   [fit] bbox=x:[minX,maxX] y:[minY,maxY] scale=.. xOffset=.. yOffset=..（模型单位）
     *
     * @param viewHalfW 视口半宽（模型单位，与 onSurfaceChanged/recomputeProjection 同源）
     * @param viewHalfH 视口半高（模型单位）
     */
    private fun applyCharacterFitToState(viewHalfW: Float, viewHalfH: Float) {
        val bbox = fitBBoxModel ?: run {
            fitScale = 1f
            fitXOffset = 0f
            fitYOffset = 0f
            refreshEffectiveDisplay()
            return
        }
        val fit: CharacterFit = computeCharacterFit(
            bbox,
            viewHalfW * 2f, viewHalfH * 2f,
            canvasWidth, canvasHeight
        )
        fitScale = fit.scale
        fitXOffset = fit.xOffset
        fitYOffset = fit.yOffset
        refreshEffectiveDisplay()
        android.util.Log.i(
            "Live2DRenderer",
            "[fit] bbox=x:[${bbox.minX},${bbox.maxX}] y:[${bbox.minY},${bbox.maxY}] " +
                "scale=${fit.scale} xOffset=${fit.xOffset} yOffset=${fit.yOffset}"
        )
    }

    // ============================================================
    //  C-M5 用户交互（contract-C-M5 §2.3；GL 线程调用，由 Live2DView 手势接线触发）
    // ============================================================

    /** 设置当前模型 ID（override 持久化 key；Live2DView load/swap 时调用）。 */
    fun setCurrentModelId(modelId: String) {
        currentModelId = modelId
    }

    /** 应用该模型存储的用户 override（无 → 走 C-M1 fit）。load/swapModel 完成后调用。 */
    fun applyStoredUserOverride() {
        val stored = currentModelId?.let { interactionStore.load(it) }
        setUserOverride(stored)
    }

    /** 设置用户 override（null = 清除，回 fit）。 */
    fun setUserOverride(override: Override?) {
        userScale = override?.scale
        userXOffset = override?.xOffset
        userYOffset = override?.yOffset
        refreshEffectiveDisplay()
        recomputeProjection()
    }

    /**
     * 单指拖拽增量（屏幕像素 → 模型单位内部换算；GL 线程）。
     *
     * ★ deviation（契约 §2.3「直接加进 displayXOffset」的符号修正）：offset 平移的是投影窗口——
     * 窗口 +x → 模型在屏幕上 -x（d(screenX)/d(xOffset) < 0）；yOffset 同理（+yOffset → 模型下移）。
     * 若按契约字面「+=」应用，角色会与手指反向移动。这里反向应用（-=），保证真机 M5-A9①
     * 「单指拖拽 → 角色跟随移动」成立；dragPxToModel 纯函数本身与契约 §2.1 逐字一致（M5-A3 锁）。
     */
    fun applyUserDragPx(dxPx: Float, dyPx: Float) {
        val bounds = currentProjectionBounds() ?: return
        val (dxModel, dyModel) = dragPxToModel(
            dxPx, dyPx,
            viewportWidth.toFloat(), viewportHeight.toFloat(),
            bounds[0], bounds[1], bounds[2], bounds[3]
        )
        userXOffset = (userXOffset ?: fitXOffset) - dxModel
        userYOffset = (userYOffset ?: fitYOffset) - dyModel
        refreshEffectiveDisplay()
        recomputeProjection()
        android.util.Log.i("Live2DRenderer", "[interaction] drag dxModel=$dxModel dyModel=$dyModel")
    }

    /** 双指捏合开始：捕获基准（当前生效 scale + 起始双指距离 px）。 */
    fun beginUserPinch(startDistancePx: Float) {
        pinchBaseScale = displayScale
        pinchStartDistancePx = startDistancePx
    }

    /** 双指捏合进行：userScale = pinchScale(基准, 起始距离, 当前距离)，钳制 [0.1, 5.0]。 */
    fun applyUserPinch(currentDistancePx: Float) {
        userScale = pinchScale(pinchBaseScale, pinchStartDistancePx, currentDistancePx)
        refreshEffectiveDisplay()
        recomputeProjection()
        android.util.Log.i("Live2DRenderer", "[interaction] pinch scale=${userScale}")
    }

    /** 手势结束：把当前生效状态写入持久化（仅 view 判定发生了有效调整后调用）。 */
    fun saveUserOverride() {
        val id = currentModelId ?: return
        interactionStore.save(id, Override(displayScale, displayXOffset, displayYOffset))
    }

    /** 重置入口：清除该模型 override + 立即重算 fit 并生效（回 C-M1 构图）。 */
    fun resetUserOverride() {
        val id = currentModelId
        if (id != null) interactionStore.clear(id)
        userScale = null
        userXOffset = null
        userYOffset = null
        refreshEffectiveDisplay()
        recomputeProjection()
        android.util.Log.i("Live2DRenderer", "[interaction] reset → fit")
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
                    // C-Perf4：新模型（含热切换）从更新帧重新开始计数
                    modelUpdateFrameCounter = 0
                    modelUpdateDeltaAccum = 0f
                    isFirstFrame = false
                }

                // ── 动画更新管线（C-Perf4 降频：每 MODEL_UPDATE_INTERVAL_FRAMES 帧执行一次，
                //    deltaTime 跨帧累积一次性传入 → 呼吸/眨眼/物理/motion 按真实时间推进，
                //    速度语义不变；刷新/build/draw 仍每帧执行，参数状态按更新粒度呈现）──
                modelUpdateDeltaAccum += deltaTime
                modelUpdateFrameCounter++
                if (modelUpdateFrameCounter >= MODEL_UPDATE_INTERVAL_FRAMES) {
                    val pipeline = animationPipeline
                    if (pipeline != null) {
                        pipeline.update(modelUpdateDeltaAccum)
                    } else {
                        model.update()
                    }
                    modelUpdateDeltaAccum = 0f
                    modelUpdateFrameCounter = 0
                }

                // ── M7：触发 motion 播满一轮回落 idle + idle 多样化轮换（§1.3-5 / §5.2-3）──
                checkTriggeredMotionFallback()
                updateIdleRotation(deltaTime)

                // 刷新 drawable 批量属性缓存
                refreshCachedAttributes()

                // 构建 DrawableRenderData 列表
                buildDrawableList()

                // C8h：超采样路径 → 渲染到 2K FBO，再下采样 blit 回 surface；
                // 否则直渲（行为与优化前逐位一致）。投影矩阵只依赖宽高比
                // （surface 1080×2400 ≡ FBO 1440×3200 宽高比 0.45），同一投影矩阵两用，
                // 交互映射（viewportWidth/Height = surface 尺寸）零改动。
                if (supersampleActive) {
                    // 1. 渲染到 2K FBO（视口 = FBO 尺寸）
                    GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, supersampleFbo)
                    GLES20.glViewport(0, 0, supersampleRenderW, supersampleRenderH)
                    GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)

                    // preDraw() — 重置 OpenGL 状态
                    preDraw()

                    // drawObjectLoop() — 主渲染循环（输出到 FBO）
                    drawObjectLoop(shaderMgr)

                    // 2. 生成 mipmap（下采样 blit 的三线性过滤源；FBO 内容完整后生成）
                    GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, supersampleTexture)
                    GLES20.glGenerateMipmap(GLES20.GL_TEXTURE_2D)

                    // [C8h-probe] TEMPORARY: periodic readback — FBO(2K) torso strip vs SURFACE(1080p) torso strip
                    c8hProbeFrame++
                    if (c8hProbeFrame % 300 == 0) {
                        val px = ByteBuffer.allocateDirect(300 * 4).order(ByteOrder.nativeOrder())
                        GLES20.glReadPixels(533, 1590, 300, 1, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, px)
                        val sb = StringBuilder("[C8h-probe] FBO(2K) x533-832 y1590(torso): ")
                        for (i in 0 until 300) sb.append(px.get(i * 4).toInt() and 0xFF).append(',')
                        android.util.Log.i("Live2DRenderer", sb.toString())
                    }

                    // 3. 下采样 blit：FBO 纹理 → 默认 framebuffer（视口 = surface 尺寸）
                    GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
                    GLES20.glViewport(0, 0, viewportWidth, viewportHeight)
                    drawSupersampleBlit()

                    // [C8h-probe] TEMPORARY: surface 1080p torso strip (bottom-origin y=1200 = top y 1200)
                    if (c8hProbeFrame % 300 == 0) {
                        val px = ByteBuffer.allocateDirect(300 * 4).order(ByteOrder.nativeOrder())
                        GLES20.glReadPixels(400, 1200, 300, 1, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, px)
                        val sb = StringBuilder("[C8h-probe] SURFACE(1080p) x400-699 y1200(torso): ")
                        for (i in 0 until 300) sb.append(px.get(i * 4).toInt() and 0xFF).append(',')
                        android.util.Log.i("Live2DRenderer", sb.toString())
                    }

                    // 清除状态（含 FBO 纹理解绑 + 缓存失效，防下帧 state cache 误判跳过绑定）
                    GLES20.glUseProgram(0)
                    GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
                    GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, 0)
                    cachedBoundTextureId = -1
                } else {
                    // preDraw() — 重置 OpenGL 状态
                    preDraw()

                    // drawObjectLoop() — 主渲染循环
                    drawObjectLoop(shaderMgr)

                    // 清除状态
                    GLES20.glUseProgram(0)
                    GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
                }
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

    /**
     * 按语义标签（motionLabel 别名）播放 motion（M7，contract-M7-android-l3.md §1）。
     *
     * 解析链路：tag → MotionAliasTable.resolve() → loadedMotions 键 → MotionData →
     *           pipeline.motionManager.startMotion(data, priority)
     *
     * @param tag     语义别名（M1 §5 别名表；大小写不敏感，trim 后查表）
     * @param priority 播放优先级（默认 NORMAL=2）；若解析键以 "Idle_" 开头 → 强制 IDLE=1
     *                （与 Live2DRenderer.startMotion 的 group=="Idle" → IDLE 规则一致）
     * @return true = 已开始播放；false = 未知 tag / 键未加载 / 被更高优先级阻止或同优先级不打断
     *         （不抛异常，静默降级）
     */
    fun startMotionByTag(tag: String, priority: Int = MotionPriority.NORMAL): Boolean {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return false
        try {
            val key = MotionAliasTable.resolve(tag)
            if (key == null) {
                android.util.Log.w("MotionManagerTag", "未知 tag: $tag")
                return false
            }
            val data = loadedMotions[key]
            if (data == null) {
                android.util.Log.w("MotionManagerTag", "模型无 motion 键: $key (tag=$tag)")
                return false
            }
            val pipeline = animationPipeline ?: return false
            val finalPriority = if (key.startsWith("Idle_")) MotionPriority.IDLE else priority
            val started = pipeline.motionManager.startMotion(data, finalPriority)
            if (started) {
                // M7 回落标记：仅对 NORMAL 优先级的 M7 触发路径生效（IDLE 触发回落无意义）
                triggeredMotionActive = finalPriority == MotionPriority.NORMAL
                triggeredMotionDuration = data.duration
                triggeredMotionElapsed = 0f
            }
            return started
        } catch (e: Throwable) {
            android.util.Log.w("Live2DRenderer", "startMotionByTag 异常: ${e.message}")
            return false
        }
    }

    /**
     * M7 触发 motion 回落 idle（contract-M7-android-l3.md §1.3-5）。
     * 触发类 motion 若 loop=true → 经 startMotionByTag 触发的 motion 会循环播放；
     * 播满一轮（elapsed >= duration）→ stopAllMotions + startIdleMotion（§5 兜底，idle 轮换随后接管）。
     * 只对 triggeredMotionActive 标记路径生效——现有 startMotion 路径（非 M7 触发）行为不变，零回归。
     */
    private fun checkTriggeredMotionFallback() {
        if (!triggeredMotionActive) return
        val pipeline = animationPipeline ?: run {
            triggeredMotionActive = false
            return
        }
        // 当前无播放 / 优先级不再匹配（被更高优先级打断 / 被新 startMotion 覆盖）→ 清标记，不回落
        val currentPrio = pipeline.motionManager.currentPriority
        if (currentPrio == null || currentPrio != MotionPriority.NORMAL) {
            triggeredMotionActive = false
            return
        }
        triggeredMotionElapsed += deltaTime
        if (triggeredMotionElapsed >= triggeredMotionDuration) {
            android.util.Log.i("Live2DRenderer", "M7 触发 motion 播满一轮，回落 idle")
            triggeredMotionActive = false
            pipeline.motionManager.stopAllMotions()
            startIdleMotion()
        }
    }

    /**
     * M7 idle 多样化轮换（contract-M7-android-l3.md §5.2-3）。
     * IDLE 优先级播放期间累积 idleElapsed，达 IDLE_SWITCH_INTERVAL_MS → 经 IdleMotionSelector
     * 抽选下一个 idle 键播放（仍 IDLE 优先级）。非 IDLE 播放中 → 不累计。
     * 轮换只作用于 IDLE 优先级，绝不触碰 NORMAL/FORCE 播放。
     */
    private fun updateIdleRotation(deltaTime: Float) {
        val pipeline = animationPipeline ?: return
        if (pipeline.motionManager.currentPriority == MotionPriority.IDLE) {
            idleElapsed += deltaTime
            if (idleElapsed >= IDLE_SWITCH_INTERVAL_MS / 1000f) {
                idleElapsed = 0f
                val nextKey = idleSelectorForCurrentModel().next() ?: return
                val data = loadedMotions[nextKey] ?: return
                pipeline.motionManager.startMotion(data, MotionPriority.IDLE)
            }
        } else {
            idleElapsed = 0f
        }
    }

    /**
     * 当前模型 idle 轮换器（懒初始化，模型切换后重建）。
     * 池 = IdleMotionSelector.DEFAULT_POOL ∩ loadedMotions（未加载键剔除；交集为空 → 池空 → 走兜底）。
     */
    private fun idleSelectorForCurrentModel(): IdleMotionSelector {
        idleSelector?.let { return it }
        val pool = IdleMotionSelector.DEFAULT_POOL.filter { loadedMotions.containsKey(it) }
        return IdleMotionSelector(pool).also { idleSelector = it }
    }

    fun setExpression(expressionIndex: Int, intensity: Float = 1.0f) {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return
        try {
            val pipeline = animationPipeline
            val setting = modelSettingJson!!
            val expressions = setting.getJSONObject("FileReferences").optJSONArray("Expressions") ?: run {
                setExpressionFallback(expressionIndex, intensity)
                return
            }
            if (expressionIndex >= expressions.length()) {
                setExpressionFallback(expressionIndex, intensity)
                return
            }

            // 尝试使用预加载的表情（Name → 文件名 stem 查找，M2 起抽取为 resolveExpressionData 复用）
            val expData = resolveExpressionData(expressionIndex)
            if (expData != null && pipeline != null) {
                pipeline.expressionManager.setExpression(expData, intensity)
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

    /**
     * 按索引解析 ExpressionData（model3.json Expressions 数组 → loadedExpressions）。
     * 查找规则（与既有 setExpression 完全一致）：Name → 文件名 stem；
     * 无 Expressions 数组 / 索引越界 → null。
     */
    private fun resolveExpressionData(expressionIndex: Int): ExpressionData? {
        val setting = modelSettingJson ?: return null
        val expressions =
            setting.getJSONObject("FileReferences").optJSONArray("Expressions") ?: return null
        if (expressionIndex >= expressions.length()) return null

        val expFileName = expressions.getJSONObject(expressionIndex).optString("File", "")
        val expName = expressions.getJSONObject(expressionIndex).optString("Name", "")

        // 按名称查找
        var expData = loadedExpressions[expName]
        // 按文件名查找
        if (expData == null && expFileName.isNotEmpty()) {
            val nameFromFile = expFileName.substringAfterLast("/").substringBeforeLast(".")
            expData = loadedExpressions[nameFromFile]
        }
        return expData
    }

    /**
     * 结构化表情混合（M2，contract-M2-M3 §1.3）：多表情加权混合 + 参数覆盖。
     *
     * @param mix (EMOTION_MAP 索引, weight) 列表，按数组顺序应用；
     *            索引查不到的表情（模型无此表情文件）→ 跳过该项，其余照常（部分生效，不降级）。
     * @param overrides 参数覆盖乘数（key = 参数 ID，value = 乘数）
     */
    fun setExpressionMix(mix: List<Pair<Int, Float>>, overrides: Map<String, Float>) {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return
        try {
            val pipeline = animationPipeline ?: return
            val resolved = mutableListOf<Pair<ExpressionData, Float>>()
            for ((index, weight) in mix) {
                val expData = resolveExpressionData(index) ?: continue
                resolved.add(expData to weight)
            }
            pipeline.expressionManager.setExpressionMix(resolved, overrides)
        } catch (e: Throwable) {
            android.util.Log.w("Live2DRenderer", "结构化表情混合失败: ${e.message}")
        }
    }

    /**
     * 当前模型是否含可用的 Expressions（exp3）配置（A3 双路径分发判据，contract-bai-params-v0 AP-01）。
     *
     * modelSettingJson `FileReferences.Expressions` 缺失/空 或 loadedExpressions 为空 → false；
     * 模型未加载 → false。bai（无 exp3）→ false。
     */
    fun hasExpressions(): Boolean {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return false
        return try {
            val setting = modelSettingJson!!
            val expressions = setting.getJSONObject("FileReferences").optJSONArray("Expressions")
            if (expressions == null || expressions.length() == 0) return false
            loadedExpressions.isNotEmpty()
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * C7d：TTS 播放状态（LipSync 口型让位判定源）。
     * 经 [setLipSyncSpeakingProvider] 注入 VoiceIoController 组合 isSpeaking 流（任一 TtsProvider 在播 = true）；
     * 未注入 → false（不剔除 MouthOpenY，行为与 R16 前一致）。
     */
    @Volatile
    private var lipSyncSpeaking: () -> Boolean = { false }

    /**
     * 注入/更新 TTS 播放状态源（C7d 口型让位）。
     * 由 Live2DView 转发 VoiceIoController.attachLive2DView 接线（组合 isSpeaking）；
     * null → 复位 false（不剔除 MouthOpenY）。
     */
    fun setLipSyncSpeakingProvider(provider: (() -> Boolean)?) {
        lipSyncSpeaking = provider ?: { false }
    }

    /**
     * 参数驱动表情注入（A3 P1，contract-bai-params-v0 AP-11~17）：
     * 以目标绝对值帧（blend=Overwrite）注入 ExpressionManager，复用淡入淡出管线。
     *
     * - native/model 未就绪 → 静默 return（与 setExpression 同风格，不抛异常）
     * - key ∉ paramNameToIndex（moc3）→ 跳过（静默，防未知/中文名参数写坏；计跳过数记日志）
     * - 不 clamp、不走 setParameter（fear._3 EyeLOpen=1.3 / surprise._3=1.5 原样进 ExpressionData，
     *   收敛交给 applyExpression 的 moc3 min/max clamp，AP-15）
     * - fade：expressionFadeTime = fadeMs/1000f（fadeMs 先 coerceIn(20,1000)，协议 H 段值域）
     * - 落点：expressionManager.setExpression(expData, 1.0f)（D5-2：帧值已编码强度，intensity 只作 fade 权重上限）
     * - 全部 key 被过滤（空帧）→ 不调 setExpression（静默 no-op，AP-16）
     * - C7d 口型让位：TTS 播放期间（isSpeaking=true）表情帧 MouthOpenY 键剔除（不参与 fade；
     *   LipSync 继续独占口型；TTS 结束 → 下次表情帧应用时 MouthOpenY 恢复）。
     */
    fun setParametricExpression(parameters: Map<String, Float>, fadeMs: Int) {
        if (!nativeAvailable || modelPtr == 0L || modelSettingJson == null) return
        try {
            val pipeline = animationPipeline ?: return
            val fade = fadeMs.coerceIn(20, 1000)

            // C7d 口型让位：TTS 播放期间表情帧 MouthOpenY 剔除（不参与 fade；LipSync 独占口型）
            val speaking = lipSyncSpeaking()

            // 过滤：只保留 moc3 内嵌参数 Id（paramNameToIndex 来源 = native getParameterIds）
            val params = mutableListOf<ExpressionParameter>()
            var skipped = 0
            var mouthOpenYFiltered = 0
            for ((key, value) in parameters) {
                if (speaking && key == MOUTH_OPEN_Y_ID) {
                    mouthOpenYFiltered++
                    continue
                }
                if (paramNameToIndex.containsKey(key)) {
                    params.add(ExpressionParameter(id = key, value = value, blend = "Overwrite"))
                } else {
                    skipped++
                    android.util.Log.w("Live2DRenderer", "setParametricExpression: param not found: $key（跳过）")
                }
            }
            if (mouthOpenYFiltered > 0) {
                android.util.Log.i(
                    "Live2DRenderer",
                    "setParametricExpression: LipSync 播放中剔除 MouthOpenY ×$mouthOpenYFiltered（口型让位，LipSync 独占）"
                )
            }
            android.util.Log.i(
                "Live2DRenderer",
                "setParametricExpression: 参数命中 ${params.size}，跳过 $skipped（fadeMs=$fade）"
            )
            if (params.isEmpty()) return

            val expData = ExpressionData(params)
            pipeline.expressionManager.expressionFadeTime = fade / 1000f
            pipeline.expressionManager.setExpression(expData, 1.0f)
        } catch (e: Throwable) {
            android.util.Log.w("Live2DRenderer", "setParametricExpression 异常: ${e.message}")
        }
    }

    /**
     * 自动眨眼开关（A3 D2-A，contract-bai-params-v0 AP-19）：
     * 参数驱动表情激活期间暂停自动眨眼（autoBlinkEnabled=false），回 neutral 恢复 true。
     * pipeline 未就绪 → 静默（null-safe，不抛异常）。
     */
    fun setAutoBlink(enabled: Boolean) {
        val pipeline = animationPipeline ?: return
        pipeline.autoBlinkEnabled = enabled
    }

    /**
     * 当前模型 cdi3.json 参数 ID 白名单（M2 parameterOverrides key 校验源，emotion-protocol.md C3）。
     * 模型未加载 / 无 cdi3.json → 空集。
     */
    fun getParameterWhitelist(): Set<String> {
        return modelMeta?.parameters?.mapTo(mutableSetOf()) { it.id } ?: emptySet()
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

    /**
     * 按参数 ID 设置 Live2D 参数值（M5 VisemeAnalyzer 集成点）。
     * 模型无此参数 → 静默跳过；value 自动 coerceIn [0,1]。
     */
    fun setParameter(parameterId: String, value: Float) {
        if (!nativeAvailable || modelPtr == 0L) return
        try {
            val idx = paramNameToIndex[parameterId] ?: return
            Live2DNative.setParameterValue(modelPtr, idx, value.coerceIn(0f, 1f))
        } catch (_: Throwable) {}
    }

    fun onPause() {}
    fun onResume() {}

    fun dispose() {
        // 释放模型资源（native 模型、纹理、动画管线、motions/expressions）
        releaseModelResources()
        // 释放 ShaderManager（GL 上下文级资源）
        shaderManager?.dispose()
        shaderManager = null
        // 释放 fallback 着色器
        if (fallbackProgram != 0) {
            GLES20.glDeleteProgram(fallbackProgram)
            fallbackProgram = 0
        }
        // C8h：释放超采样 FBO/纹理 + blit 着色器（GL 上下文级资源）
        releaseSupersampleFbo()
        if (blitProgram != 0) {
            GLES20.glDeleteProgram(blitProgram)
            blitProgram = 0
        }
        blitQuadBuffer = null
    }

    /**
     * 释放当前模型占用的资源（必须在 GL 线程调用）。
     *
     * 保留 ShaderManager / fallback 着色器等 GL 上下文级资源，供下一次模型加载复用。
     * 与 [dispose] 的区别：dispose() 在 View 销毁时释放全部资源；
     * 本方法用于模型热切换（Live2DView.switchModel → swapModel）时只清理模型级资源。
     */
    private fun releaseModelResources() {
        // 释放动画管线
        animationPipeline = null
        loadedMotions.clear()
        loadedExpressions.clear()

        // M7：重置标签触发 / idle 轮换状态（模型切换后重建）
        triggeredMotionActive = false
        triggeredMotionDuration = 0f
        triggeredMotionElapsed = 0f
        idleElapsed = 0f
        idleSelector = null

        // 释放 native 模型
        if (modelPtr != 0L) {
            try { Live2DNative.freeModel(modelPtr) } catch (_: Throwable) { }
            modelPtr = 0L
        }
        purismModel = null

        // 释放纹理
        for (id in glTextures.values) GLES20.glDeleteTextures(1, intArrayOf(id), 0)
        glTextures.clear()

        // 重置 per-model 状态（下次加载前恢复初始值）
        modelSettingJson = null

        // M1：清 fit bbox 缓存（防热切换串值；display 值由下次 load/视口重算覆盖）
        fitBBoxModel = null

        // C-M5：清用户 override / fit 结果 / 捏合基准（防热切换串值；override 由 view 在 swap 后重新应用）
        userScale = null
        userXOffset = null
        userYOffset = null
        fitScale = 1f
        fitXOffset = 0f
        fitYOffset = 0f
        pinchBaseScale = 1f
        pinchStartDistancePx = 0f
        paramNameToIndex = emptyMap()
        paramAIndex = -1
        partNameToIndex = emptyMap()
        modelMeta = null
        drawables = Collections.emptyList()
        cachedTextureIndices = null
        cachedRenderOrders = null
        cachedOpacities = null
        cachedVertexCounts = null
        cachedIndexCounts = null
        cachedConstantFlags = null
        cachedDynamicFlags = null
        cachedMultiplyColors = null
        cachedScreenColors = null
        cachedMaskCounts = null
        // C-Perf2：清空 per-drawable 缓冲池与静态数据缓存（模型释放/热切换时，防串模型）
        pooledPositions = emptyArray()
        pooledUvs = emptyArray()
        pooledIndices = emptyArray()
        cachedUvsData = emptyArray()
        cachedIndicesData = emptyArray()
        // C8b：位置缓冲填充标记随池清空（防换模型后 light 帧误跳重取）
        positionsPopulated = BooleanArray(0)
    }

    /**
     * 在 GL 线程热切换模型（由 Live2DView.switchModel 在 queueEvent 中调用）。
     *
     * 释放当前模型资源 → 加载新模型。失败时设置 fallbackActive=true（渲染兜底
     * 渐变着色器），不向外抛出。
     *
     * @return true=加载成功；false=失败（已启用兜底渲染）
     */
    fun swapModel(assetPath: String): Boolean {
        releaseModelResources()
        fallbackActive = false
        isFirstFrame = true
        hasStartedIdle = false
        previousFrameNano = System.nanoTime()
        deltaTime = 0.016f

        if (!nativeAvailable) {
            fallbackActive = true
            return false
        }
        return try {
            modelDir = assetPath.substringBeforeLast("/")
            loadModel(assetPath)
            true
        } catch (e: Throwable) {
            android.util.Log.w("Live2DRenderer", "切换模型失败: ${e.message}")
            fallbackActive = true
            false
        }
    }

    // ============================================================
    //  模型加载
    // ============================================================

    private fun loadModel(assetPath: String) {
        // 1. 解析 model3.json
        val modelBytes = context.assets.open(assetPath).use { it.readBytes() }
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

        // C-Perf6：纹理重载（新 glGenTextures id）后 TEXTURE0 实际绑定未知，缓存失效
        resetGlStateCache()

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

        // 12. M1 contain-fit（contract-C-M1 §2.3，A6 时机 1）：load 完成后一次性聚合 bbox，
        //     并按当前视口写 fit 值 + 落投影（首帧前视口未知 → recomputeProjection 内部跳过，等 onSurfaceChanged）。
        computeFitBBox()
        recomputeProjection()
    }

    private fun loadTextures() {
        val setting = modelSettingJson ?: return
        val textures = setting.getJSONObject("FileReferences").getJSONArray("Textures")

        // M1d 纹理上限防护（contract-R1-render §2.4，C10 拍板：降采样）：加载前探测设备上限
        val maxTexSize = IntArray(1)
        GLES20.glGetIntegerv(GLES20.GL_MAX_TEXTURE_SIZE, maxTexSize, 0)
        val maxSize = maxTexSize[0]
        android.util.Log.i("Live2DRenderer", "[tex] maxSize=$maxSize")

        for (i in 0 until textures.length()) {
            try {
                val texFile = textures.getString(i)
                val path = "$modelDir/$texFile"

                // 先只读边界拿原始尺寸（inJustDecodeBounds），超限则按 2 的幂降采样
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                context.assets.open(path).use { BitmapFactory.decodeStream(it, null, bounds) }
                val texW = bounds.outWidth
                val texH = bounds.outHeight

                val downsample = needsDownsample(texW, texH, maxSize)
                val sample = if (downsample) {
                    // 目标尺寸 = ≤ maxSize 的 2 的幂（对齐 PC checkTextureFit 的 targetSize 语义）
                    val target = computeDownsampleTarget(maxSize)
                    var s = 1
                    while (texW / s > target || texH / s > target) s *= 2
                    s
                } else {
                    1
                }

                // M1a 预乘 alpha（contract-R1-render §2.1，C9 拍板：上传侧预乘，零着色器改动）
                val options = BitmapFactory.Options().apply {
                    inSampleSize = sample
                    inPremultiplied = true
                }
                val bitmap = context.assets.open(path).use { BitmapFactory.decodeStream(it, null, options) } ?: continue
                android.util.Log.i(
                    "Live2DRenderer",
                    "[tex] maxSize=$maxSize loaded=${bitmap.width}x${bitmap.height} downsample=$downsample"
                )

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
    /**
     * C-Perf2：确保对象池/静态缓存数组容量 ≥ drawable 数（懒扩容，仅模型首次构建时发生）。
     */
    private fun ensureBufferPoolSize(size: Int) {
        if (pooledPositions.size < size) {
            pooledPositions = pooledPositions.copyOf(size)
            pooledUvs = pooledUvs.copyOf(size)
            pooledIndices = pooledIndices.copyOf(size)
            cachedUvsData = cachedUvsData.copyOf(size)
            cachedIndicesData = cachedIndicesData.copyOf(size)
            // C8b：填充标记随池扩容（新槽位默认 false → 必重取，安全）
            if (positionsPopulated.size < size) {
                positionsPopulated = positionsPopulated.copyOf(size)
            }
        }
    }

    /**
     * C-Perf2：取（或首次创建）drawable 的 FloatBuffer 池项，并 clear() 到可写状态。
     * 内容语义与 ByteBuffer.allocateDirect(floatCount*4) 新建后写入完全一致：
     * clear() 后 position=0 / limit=capacity，写入区外的残留字节不会被 GL 读取
     * （GL 按 vertexCount/indexCount 精确消费）。
     */
    private fun obtainFloatBuffer(pool: Array<FloatBuffer?>, index: Int, floatCount: Int): FloatBuffer {
        var buf = pool[index]
        if (buf == null || buf.capacity() < floatCount) {
            buf = ByteBuffer.allocateDirect(floatCount * 4)
                .order(ByteOrder.nativeOrder()).asFloatBuffer()
            pool[index] = buf
        }
        buf.clear()
        return buf
    }

    /**
     * C-Perf2：取（或首次创建）drawable 的 ShortBuffer 池项，并 clear() 到可写状态。
     */
    private fun obtainShortBuffer(pool: Array<ShortBuffer?>, index: Int, shortCount: Int): ShortBuffer {
        var buf = pool[index]
        if (buf == null || buf.capacity() < shortCount) {
            buf = ByteBuffer.allocateDirect(shortCount * 2)
                .order(ByteOrder.nativeOrder()).asShortBuffer()
            pool[index] = buf
        }
        buf.clear()
        return buf
    }

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

        // C-Perf2：首帧一次性扩容池/缓存数组到 drawable 数（后续帧零分配）
        ensureBufferPoolSize(count)

        // C8b：light 帧（native 无输入变化 → 顶点冻结）跳过逐 drawable 顶点重取/重传。
        // 池内 DirectBuffer 保留上一 full 帧内容 = 当前帧正确顶点（light 语义保证逐位一致）；
        // positionsPopulated 守卫首帧/换模型（池内容未填充或串模型时强制重取）。
        // 仅跳过顶点复制路径（~N 次 JNI + 41k 浮点拷贝）；refreshCachedAttributes 的
        // 数组级属性（不透明度/顺序/动态标志）仍每帧执行——10 次 JNI、开销小，且动态标志
        // 在 light 帧由 csmResetDrawableDynamicFlags 重置后保持与上一 full 帧一致，必须重读。
        val lastUpdateLight = Live2DNative.getLastUpdateLight(modelPtr)

        val list = mutableListOf<DrawableRenderData>()
        for (i in 0 until count) {
            // 从 PurismModel 获取顶点数据（已除以 ppu）
            val model = purismModel ?: continue
            val vc = vertexCounts.getOrElse(i) { 0 }
            val ic = indexCounts.getOrElse(i) { 0 }

            if (vc <= 0 || ic <= 0) continue

            // C8b：light 且池内已填充 → 不重取顶点（复用池内容）；否则 full 帧/首帧重取
            val skipVertexFetch = lastUpdateLight &&
                i < positionsPopulated.size && positionsPopulated[i]
            val pos: FloatArray? = if (skipVertexFetch) null else model.getDrawableVertices(i)
            if (pos != null && i < positionsPopulated.size) {
                positionsPopulated[i] = true
            }

            // C-Perf2：UV/顶点索引模型内恒定 → 首帧获取后缓存复用（消除 500 次/帧 JNI）；
            // 顶点位置每帧变化 → 每帧重取（原语义，行为零变化）
            var uvs = cachedUvsData.getOrNull(i)
            if (uvs == null) {
                uvs = model.getDrawableVertexUvs(i)
                cachedUvsData[i] = uvs
            }
            var idx = cachedIndicesData.getOrNull(i)
            if (idx == null) {
                idx = model.getDrawableVertexIndices(i)
                cachedIndicesData[i] = idx
            }

            // C-Perf2：per-drawable DirectBuffer 对象池复用（消除 750 次/帧 allocateDirect）
            // 写入语义与原实现一致：position 0 起写 vc*2/ic 个元素，随后 position(0) 供 GL 消费。
            val pb = obtainFloatBuffer(pooledPositions, i, vc * 2)
            if (pos != null) {
                // full 帧/首帧：重写顶点（pool clear() 后 position=0，写入区外残留字节不被 GL 消费）
                pb.put(pos, 0, vc * 2)
            }
            // C8b light 帧（pos == null）：不动 pb 内容——clear() 后位置/容量与上次相同，
            // 池内保留上一 full 帧的顶点（light 语义保证逐位一致），仅 position(0) 供 GL 消费。
            pb.position(0)

            val ub = obtainFloatBuffer(pooledUvs, i, vc * 2)
            ub.put(uvs, 0, vc * 2); ub.position(0)

            val ib = obtainShortBuffer(pooledIndices, i, ic)
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
     * 简化版本：跳过 offscreen 和掩码处理（M1f 裁定，contract-R1-render §2.6）：
     * bai 真机 maskCounts=0，本轮不接线；未来接入 mask 模型时按 d.maskCount>0
     * 分派 MASKED/MASKED_INVERTED + FBO 渲染（对齐 PC doDrawModel 全接线）。
     */
    private fun drawObjectLoop(shaderMgr: ShaderManager) {
        if (drawables.isEmpty()) return

        // 按 renderOrder 排序
        val sortedDrawables = drawables.sortedBy { it.renderOrder }

        // C-Perf6 诊断：帧起始计数器快照（帧末求差 = 本帧 drawObjectLoop 内 GL 调用数）
        val countStart = glCountersSnapshot()

        // [C8f-probe] TEMPORARY: arm at frame 150 to match native probe, then log
        // actually-drawn drawables whose buffer bbox exceeds the visible envelope.
        c8fProbeFrame++
        if (c8fProbeFrame == 150) c8fProbeArmed = true
        if (c8fProbeArmed) {
            android.util.Log.i("Live2DRenderer", "[C8f-probe] armed at frame=$c8fProbeFrame")
        }

        var currentShaderType: ShaderManager.ShaderType? = null
        var currentShader: ShaderManager.ShaderProgram? = null

        for (d in sortedDrawables) {
            // ★ 必须使用 dynamicFlagIsVisible（对应 CubismRendererAndroid）
            if (!d.isVisible) continue

            // 选择着色器类型
            val shaderType = selectShader(d.blendMode, d.isInvertedMask)

            // 仅在着色器类型变化时切换（selectShader 恒 NORMAL → 每帧仅 1 次 glUseProgram；
            // matrix uniform / s_texture0 采样器常量 0 随 shader 切换设置一次即跨 drawable 复用）
            if (shaderType != currentShaderType) {
                val newShader = shaderMgr.get(shaderType)
                GlCounters.useProgram++
                GLES20.glUseProgram(newShader.programId)
                // 设置 MVP 矩阵（一旦设置就跨 drawable 复用）
                GlCounters.uniform++
                GLES20.glUniformMatrix4fv(newShader.uMatrix, 1, false, projectionMatrix, 0)
                // C-Perf6：s_texture0 采样器常量 0，仅随 shader 切换设置一次（原每 drawable 重复设）
                GlCounters.uniform++
                GLES20.glUniform1i(newShader.uTexture0, 0)
                // C-Perf6：顶点属性数组启用上提——同一 shader 的属性位置不变，仅首次启用；
                // 循环结束统一禁用（原每 drawable 启/停 ×2）
                if (attribsEnabledShaderType != shaderType) {
                    GlCounters.attribArray++
                    GLES20.glEnableVertexAttribArray(newShader.aPosition)
                    if (newShader.aTexCoord >= 0) {
                        GlCounters.attribArray++
                        GLES20.glEnableVertexAttribArray(newShader.aTexCoord)
                    }
                    attribsEnabledShaderType = shaderType
                }
                currentShaderType = shaderType
                currentShader = newShader
            }

            val shader = currentShader ?: continue

            // [C8f-probe] TEMPORARY: log actually-drawn drawables whose buffer bbox
            // extends beyond the visible geometry envelope (fragment region). Remove after capture.
            if (c8fProbeArmed) {
                val p = d.positionsBuffer
                val pn = p.remaining() / 2
                var mnX = 1e9f; var mxX = -1e9f; var mnY = 1e9f; var mxY = -1e9f
                p.position(0)
                val pbuf = p.duplicate()
                var bi = 0
                while (bi < pn * 2) {
                    val x = pbuf.get(bi); val y = pbuf.get(bi + 1)
                    if (x < mnX) mnX = x; if (x > mxX) mxX = x
                    if (y < mnY) mnY = y; if (y > mxY) mxY = y
                    bi += 2
                }
                if (mxX > 0.26f || mxY > 0.655f) {
                    android.util.Log.i(
                        "Live2DRenderer",
                        "[C8f-probe] DRAWN di=${d.index} " +
                            "bbox=[%.3f..%.3f, %.3f..%.3f] vc=$pn tex=${d.textureIndex} opacity=${d.opacity} " +
                            "order=${d.renderOrder} blend=${d.blendMode}".format(mnX, mxX, mnY, mxY)
                    )
                }
                p.position(0)
            }

            // ── 混合模式（预乘 alpha） ──
            // C-Perf6 状态缓存：blendMode 与上次相同 → 跳过（GL blend 状态跨帧持久）
            if (d.blendMode != cachedBlendMode) {
                GlCounters.blendFunc++
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
                cachedBlendMode = d.blendMode
            }

            // ── 面剔除 ──
            // C-Perf6 状态缓存：isDoubleSided 与上次相同 → 跳过 enable/disable；
            // frontFace 恒 CCW，仅与上次不一致时设置
            val wantCull = !d.isDoubleSided
            if (cachedCullEnabled != wantCull) {
                GlCounters.cullFace++
                if (wantCull) {
                    GLES20.glEnable(GLES20.GL_CULL_FACE)
                } else {
                    GLES20.glDisable(GLES20.GL_CULL_FACE)
                }
                cachedCullEnabled = wantCull
            }
            if (cachedFrontFace != GLES20.GL_CCW) {
                GlCounters.frontFace++
                GLES20.glFrontFace(GLES20.GL_CCW)
                cachedFrontFace = GLES20.GL_CCW
            }

            // ── 绑定主纹理 (s_texture0) ──
            val texId = glTextures[d.textureIndex]
            if (texId == null) {
                // M1d 纹理防护（contract-R1-render §2.4）：texId==null → 跳过绘制，
                // 替代现状"不绑定仍画"的残留纹素风险
                android.util.Log.w("Live2DRenderer", "[tex] null texId skipped")
                continue
            }
            // C-Perf6 状态缓存：textureIndex 与上次相同 → 跳过 activeTexture+bindTexture
            // （renderOrder 排序后同纹理 drawable 相邻，收益最大）
            if (cachedBoundTextureId != texId) {
                GlCounters.activeTexture++
                GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
                GlCounters.bindTexture++
                GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texId)
                cachedBoundTextureId = texId
            }

            // ── 颜色 uniforms（每 drawable 值可变，保持每 drawable 设置）──
            // baseColor: (1,1,1,opacity) — RGB 交由 multiply/screen 控制
            // C-Perf6：复用 scratch 数组，避免每 drawable 分配 FloatArray
            baseColorScratch[0] = 1f
            baseColorScratch[1] = 1f
            baseColorScratch[2] = 1f
            baseColorScratch[3] = d.opacity
            GlCounters.uniform++
            GLES20.glUniform4fv(shader.uBaseColor, 1, baseColorScratch, 0)
            GlCounters.uniform++
            GLES20.glUniform4fv(shader.uMultiplyColor, 1, d.multiplyColor, 0)
            GlCounters.uniform++
            GLES20.glUniform4fv(shader.uScreenColor, 1, d.screenColor, 0)

            // ── 顶点属性（pointer 每 drawable 必设——client-side buffer 不同）──
            d.positionsBuffer.position(0)
            GlCounters.vertexAttribPointer++
            GLES20.glVertexAttribPointer(shader.aPosition, 2, GLES20.GL_FLOAT, false, 0, d.positionsBuffer)

            if (shader.aTexCoord >= 0) {
                d.uvsBuffer.position(0)
                GlCounters.vertexAttribPointer++
                GLES20.glVertexAttribPointer(shader.aTexCoord, 2, GLES20.GL_FLOAT, false, 0, d.uvsBuffer)
            }

            // ── 绘制 ──
            d.indicesBuffer.position(0)
            GlCounters.drawElements++
            GLES20.glDrawElements(GLES20.GL_TRIANGLES, d.indexCount, GLES20.GL_UNSIGNED_SHORT, d.indicesBuffer)
        }

        // 恢复 OpenGL 默认状态（C-Perf6：cache-aware——仅在与目标状态不一致时调用，
        // 最终状态与优化前完全一致：blend normal / cull on / frontFace CW / attribs off / unit0）
        GlCounters.activeTexture++
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        if (cachedBlendMode != 0) {
            GlCounters.blendFunc++
            GLES20.glBlendFuncSeparate(
                GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA,
                GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA
            )
            cachedBlendMode = 0
        }
        if (cachedCullEnabled != true) {
            GlCounters.cullFace++
            GLES20.glEnable(GLES20.GL_CULL_FACE)
            cachedCullEnabled = true
        }
        // M1c 面剔除状态恢复（contract-R1-render §2.3）：循环内 per-drawable 设为 CCW，
        // 结束必须恢复 onSurfaceCreated 初始 glFrontFace(GL_CW)，防状态漂移
        if (cachedFrontFace != GLES20.GL_CW) {
            GlCounters.frontFace++
            GLES20.glFrontFace(GLES20.GL_CW)
            cachedFrontFace = GLES20.GL_CW
        }
        // C-Perf6：顶点属性数组恢复禁用（与优化前循环结束状态一致）
        val lastEnabledShaderType = attribsEnabledShaderType
        if (lastEnabledShaderType != null) {
            val lastShader = shaderMgr.get(lastEnabledShaderType)
            GlCounters.attribArray++
            GLES20.glDisableVertexAttribArray(lastShader.aPosition)
            if (lastShader.aTexCoord >= 0) {
                GlCounters.attribArray++
                GLES20.glDisableVertexAttribArray(lastShader.aTexCoord)
            }
            attribsEnabledShaderType = null
        }

        // C-Perf6 诊断：窗口累积 + 每 60 帧打印平均每帧 GL 调用数（含 fps）
        accumulateGlCounters(countStart)
    }

    // ============================================================
    //  C-Perf6 — GL 状态缓存/计数辅助
    // ============================================================

    /**
     * 使 GL 状态缓存失效（context 重建 / 纹理重载后 GL 实际状态未知，
     * 下一次 drawObjectLoop 的首个 drawable 无条件重设全部状态）。
     */
    private fun resetGlStateCache() {
        cachedBlendMode = -1
        cachedCullEnabled = null
        cachedFrontFace = 0
        cachedBoundTextureId = -1
        attribsEnabledShaderType = null
    }

    /** C-Perf6：快照当前 GL 计数器（drawObjectLoop 帧起始调用，帧末求差） */
    private fun glCountersSnapshot(): LongArray = longArrayOf(
        GlCounters.drawElements, GlCounters.blendFunc, GlCounters.bindTexture,
        GlCounters.activeTexture, GlCounters.frontFace, GlCounters.cullFace,
        GlCounters.useProgram, GlCounters.uniform, GlCounters.vertexAttribPointer,
        GlCounters.attribArray
    )

    /** C-Perf6：帧末累积窗口计数，每 60 帧打印平均每帧 GL 调用数 + fps */
    private fun accumulateGlCounters(start: LongArray) {
        if (glCountWindowFrame == 0) glCountWindowStartMs = System.currentTimeMillis()
        glCountWindowFrame++
        glCountWindowDraw += GlCounters.drawElements - start[0]
        glCountWindowBlend += GlCounters.blendFunc - start[1]
        glCountWindowBindTex += GlCounters.bindTexture - start[2]
        glCountWindowActiveTex += GlCounters.activeTexture - start[3]
        glCountWindowFrontFace += GlCounters.frontFace - start[4]
        glCountWindowCullFace += GlCounters.cullFace - start[5]
        glCountWindowUseProgram += GlCounters.useProgram - start[6]
        glCountWindowUniform += GlCounters.uniform - start[7]
        glCountWindowVap += GlCounters.vertexAttribPointer - start[8]
        glCountWindowAttribArray += GlCounters.attribArray - start[9]
        if (glCountWindowFrame >= 60) {
            val n = glCountWindowFrame.toFloat()
            val elapsedMs = (System.currentTimeMillis() - glCountWindowStartMs).coerceAtLeast(1)
            val fps = (n * 1000f) / elapsedMs
            android.util.Log.i(
                "Live2DRenderer",
                "[C-Perf6] GL calls/frame (${glCountWindowFrame}-frame window, fps=$fps): " +
                    "drawElements=${(glCountWindowDraw / n).toInt()} " +
                    "blendFunc=${(glCountWindowBlend / n).toInt()} " +
                    "bindTexture=${(glCountWindowBindTex / n).toInt()} " +
                    "activeTexture=${(glCountWindowActiveTex / n).toInt()} " +
                    "frontFace=${(glCountWindowFrontFace / n).toInt()} " +
                    "cullFace=${(glCountWindowCullFace / n).toInt()} " +
                    "useProgram=${(glCountWindowUseProgram / n).toInt()} " +
                    "uniform=${(glCountWindowUniform / n).toInt()} " +
                    "vertexAttribPointer=${(glCountWindowVap / n).toInt()} " +
                    "attribArray=${(glCountWindowAttribArray / n).toInt()} " +
                    "TOTAL=${((glCountWindowDraw + glCountWindowBlend + glCountWindowBindTex +
                        glCountWindowActiveTex + glCountWindowFrontFace + glCountWindowCullFace +
                        glCountWindowUseProgram + glCountWindowUniform + glCountWindowVap +
                        glCountWindowAttribArray) / n).toInt()}"
            )
            glCountWindowFrame = 0
            glCountWindowDraw = 0
            glCountWindowBlend = 0
            glCountWindowBindTex = 0
            glCountWindowActiveTex = 0
            glCountWindowFrontFace = 0
            glCountWindowCullFace = 0
            glCountWindowUseProgram = 0
            glCountWindowUniform = 0
            glCountWindowVap = 0
            glCountWindowAttribArray = 0
        }
    }

    // ============================================================
    //  C8h — 超采样 FBO 生命周期 + 下采样 blit
    // ============================================================

    /**
     * 重建超采样 FBO（GL 线程；onSurfaceChanged：surface 尺寸变化 / EGL context 重建后）。
     *
     * 流程：释放旧 FBO/纹理 → 分配 2K 颜色纹理（GL_RGBA + GL_LINEAR + CLAMP_TO_EDGE）→
     * 挂到新 FBO 的 COLOR_ATTACHMENT0 → glCheckFramebufferStatus 完整性校验。
     *
     * 失败（尺寸非法 / 分配失败 / FBO 不完整）→ supersampleActive=false，直渲兜底，不抛、不黑屏。
     * FBO 尺寸 = ceil(surface × SUPERSAMPLE_SCALE)：与 surface 同宽高比（零形变），
     * 目标设备（1080×2400）上 = 1440×3200（物理 2K 同分辨率）。
     *
     * ⚠️ 本方法会改写 GL_TEXTURE0 绑定 → 调用后必须 cachedBoundTextureId=-1（已内建）。
     */
    private fun recreateSupersampleFbo(surfaceW: Int, surfaceH: Int) {
        releaseSupersampleFbo()
        if (!SUPERSAMPLE_ENABLED) {
            android.util.Log.i("Live2DRenderer", "[C8h] supersample disabled (config)")
            return
        }
        if (surfaceW <= 0 || surfaceH <= 0) {
            android.util.Log.w("Live2DRenderer", "[C8h] skip FBO (invalid surface ${surfaceW}x${surfaceH})")
            return
        }
        val rw = ceil(surfaceW * SUPERSAMPLE_SCALE).toInt()
        val rh = ceil(surfaceH * SUPERSAMPLE_SCALE).toInt()

        // 颜色纹理（mipmap 化：下采样 blit 用 GL_LINEAR_MIPMAP_LINEAR 三线性过滤——
        // 1.33× 缩小仅靠 2-tap GL_LINEAR 会采样缺失（边缘重新锯齿化）；mip 级预过滤
        // 后采样 = 面积平均，2K 边缘在 1080p 缓冲内呈现正确的抗锯齿过渡）
        val texIds = IntArray(1)
        GLES20.glGenTextures(1, texIds, 0)
        if (texIds[0] == 0) {
            android.util.Log.w("Live2DRenderer", "[C8h] glGenTextures failed → 直渲兜底")
            return
        }
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texIds[0])
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR_MIPMAP_LINEAR)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexImage2D(
            GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, rw, rh, 0,
            GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, null
        )

        // FBO + 完整性校验
        val fboIds = IntArray(1)
        GLES20.glGenFramebuffers(1, fboIds, 0)
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, fboIds[0])
        GLES20.glFramebufferTexture2D(
            GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0,
            GLES20.GL_TEXTURE_2D, texIds[0], 0
        )
        val status = GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER)
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
        // FBO 创建期间 TEXTURE0 被改写 → 状态缓存失效（防下帧跳过绑定用错纹理）
        cachedBoundTextureId = -1

        if (status != GLES20.GL_FRAMEBUFFER_COMPLETE) {
            android.util.Log.w(
                "Live2DRenderer",
                "[C8h] FBO incomplete (status=$status, ${rw}x${rh}) → 直渲兜底"
            )
            GLES20.glDeleteTextures(1, texIds, 0)
            GLES20.glDeleteFramebuffers(1, fboIds, 0)
            return
        }

        supersampleFbo = fboIds[0]
        supersampleTexture = texIds[0]
        supersampleRenderW = rw
        supersampleRenderH = rh
        supersampleActive = true
        android.util.Log.i(
            "Live2DRenderer",
            "[C8h] supersample FBO ready ${rw}x${rh} (surface ${surfaceW}x${surfaceH}, scale=$SUPERSAMPLE_SCALE)"
        )
    }

    /** 释放超采样 FBO/纹理（GL 线程；尺寸变化重建 / dispose 时调用；幂等）。 */
    private fun releaseSupersampleFbo() {
        if (supersampleFbo != 0) {
            GLES20.glDeleteFramebuffers(1, intArrayOf(supersampleFbo), 0)
            supersampleFbo = 0
        }
        if (supersampleTexture != 0) {
            GLES20.glDeleteTextures(1, intArrayOf(supersampleTexture), 0)
            supersampleTexture = 0
        }
        supersampleRenderW = 0
        supersampleRenderH = 0
        supersampleActive = false
    }

    /**
     * 下采样 blit 着色器初始化（GL 线程，惰性：首次 blit 前调用；context 重建后经
     * onSurfaceCreated 置 blitProgram=0 重新初始化）。
     *
     * 顶点直通 NDC（全屏四边形，2 个 CW 三角形——适配 drawObjectLoop 清理后的
     * cull=enabled + frontFace=CW，不被剔除）；片元 = 采样 FBO 纹理（GL_LINEAR 下采样）。
     * uv v=0 ↔ 渲染目标底边（FBO texel(0,0)=视口左下角）→ 屏幕底边 → 画面不翻转。
     */
    private fun initBlitShader() {
        val vs = "attribute vec2 a_position;attribute vec2 a_texCoord;varying vec2 v_texCoord;" +
            "void main(){gl_Position=vec4(a_position,0.0,1.0);v_texCoord=a_texCoord;}"
        val fs = "precision mediump float;varying vec2 v_texCoord;uniform sampler2D u_texture;" +
            "void main(){gl_FragColor=texture2D(u_texture,v_texCoord);}"
        blitProgram = createProgram(vs, fs)
        blitAPosition = GLES20.glGetAttribLocation(blitProgram, "a_position")
        blitATexCoord = GLES20.glGetAttribLocation(blitProgram, "a_texCoord")
        blitUTexture = GLES20.glGetUniformLocation(blitProgram, "u_texture")
        // 全屏四边形（2 个 CW 三角形）：T1(-1,-1)(-1,1)(1,1) + T2(-1,-1)(1,1)(1,-1)
        val quad = floatArrayOf(
            -1f, -1f, 0f, 0f,
            -1f, 1f, 0f, 1f,
            1f, 1f, 1f, 1f,
            -1f, -1f, 0f, 0f,
            1f, 1f, 1f, 1f,
            1f, -1f, 1f, 0f
        )
        blitQuadBuffer = ByteBuffer.allocateDirect(quad.size * 4)
            .order(ByteOrder.nativeOrder()).asFloatBuffer().apply {
                put(quad)
                position(0)
            }
    }

    /**
     * 下采样 blit：FBO 纹理 → 默认 framebuffer（GL 线程；前置条件：FBO 已解绑、
     * glViewport 已设 surface 尺寸）。
     *
     * 混合保持 drawObjectLoop 清理后的 normal（ONE, ONE_MINUS_SRC_ALPHA）：默认 FB 帧首已
     * glClear(0,0,0,0) → blit 结果 = FBO 内容（预乘颜色直出），透明区 = 清屏色，与直渲逐像素一致。
     * cull/blend/frontFace 状态零改动（quad CW 缠绕 + 同 blend func）→ C-Perf6 状态缓存不失效。
     * 失败（blit 着色器编译失败 / 缓冲缺失）→ supersampleActive=false 直渲兜底，不抛。
     */
    private fun drawSupersampleBlit() {
        if (blitProgram == 0) {
            try {
                initBlitShader()
            } catch (e: Throwable) {
                android.util.Log.w("Live2DRenderer", "[C8h] blit shader init failed: ${e.message}")
                supersampleActive = false
                return
            }
        }
        val quad = blitQuadBuffer ?: run {
            android.util.Log.w("Live2DRenderer", "[C8h] blit quad buffer missing → 直渲兜底")
            supersampleActive = false
            return
        }
        GLES20.glUseProgram(blitProgram)
        GLES20.glUniform1i(blitUTexture, 0)
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, supersampleTexture)

        quad.position(0)
        GLES20.glEnableVertexAttribArray(blitAPosition)
        GLES20.glVertexAttribPointer(blitAPosition, 2, GLES20.GL_FLOAT, false, 16, quad)
        quad.position(2)
        GLES20.glEnableVertexAttribArray(blitATexCoord)
        GLES20.glVertexAttribPointer(blitATexCoord, 2, GLES20.GL_FLOAT, false, 16, quad)

        GLES20.glDrawArrays(GLES20.GL_TRIANGLES, 0, 6)

        GLES20.glDisableVertexAttribArray(blitAPosition)
        GLES20.glDisableVertexAttribArray(blitATexCoord)
        GLES20.glUseProgram(0)
    }

    // ============================================================
    //  着色器选择
    // ============================================================

    /**
     * 选择着色器类型。
     *
     * 对应 CubismShaderIndexCalculator.calculateShaderIndex() 的逻辑简化版。
     * M1f 裁定（contract-R1-render §2.6）：mask 裁剪跳过不接线——bai 真机
     * maskCounts=0，无 mask 可裁剪；本轮 selectShader 恒 NORMAL，零行为变化。
     * 未来接入 mask 模型时按 d.maskCount>0 分派 MASKED/MASKED_INVERTED + FBO 渲染。
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

            // C7d：LipSync 口型让位接线——表情层在 TTS 播放期间跳过 MouthOpenY（口型归 LipSync 独占，
            // 防双写者交替写同一参数致嘴部抽搐）。lambda 读本 renderer 的注入源（@Volatile），
            // 随 setLipSyncSpeakingProvider 更新实时生效（表情帧先于 speak 应用的主路径也覆盖）。
            animationPipeline?.expressionManager?.lipSyncMouthOpenYYield = { lipSyncSpeaking() }

            // R17：说话联动激活源接线——同一 VoiceIoController 组合 isSpeaking 流（任一 TtsProvider 在播 = true）
            // → SpeakingMotion 头部点头/身体起伏驱动源（经 LiaisonArbiter 仲裁写 ParamAngleX/ParamBodyAngleY）
            animationPipeline?.speakingActiveProvider = { lipSyncSpeaking() }

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
     * 兜底表情设置（无 Expressions 配置或预加载失败时）。
     *
     * 优先走 AnimationPipeline 的 ExpressionManager，使兜底表情与正常表情
     * 一样获得交叉淡变（旧表情淡出、新表情淡入）；管线不可用时才硬切。
     */
    private fun setExpressionFallback(expressionIndex: Int, intensity: Float = 1.0f) {
        val mouthOpen = when (expressionIndex) {
            0 -> 0.0f; 1 -> 0.3f; 2 -> 0.5f; 3 -> 0.8f
            4 -> 0.2f; 5 -> 0.4f; 6 -> 0.6f; 7 -> 0.9f
            else -> 0.0f
        }
        val pipeline = animationPipeline
        if (pipeline != null && paramAIndex >= 0) {
            // 合成一个只含 ParamA 的表情，交给 ExpressionManager 交叉淡变
            pipeline.expressionManager.setExpression(
                ExpressionData(
                    listOf(ExpressionParameter(id = "ParamA", value = mouthOpen, blend = "Overwrite"))
                ),
                intensity
            )
            return
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

/**
 * 半身投影边界纯函数（contract-v3-rename §2.1，K5 定案，可 JVM 单测）。
 *
 * ⚠️ computeProjection 重复收敛（T1.2）：唯一实现在 `com.live2dai.android.data`
 * （data/ProjectionMath.kt，ProjectionMathTest 断言目标）。本顶层函数保留仅为源码兼容，
 * 直接转发到单一实现，行为逐位等价（零回归）。新代码请直接使用 data.computeProjection。
 */
fun computeProjection(
    scale: Float,
    yOffset: Float,
    viewWidth: Float,
    viewHeight: Float,
    canvasWidth: Float,
    canvasHeight: Float,
    xOffset: Float = 0f
): FloatArray = com.live2dai.android.data.computeProjection(
    scale = scale,
    yOffset = yOffset,
    viewWidth = viewWidth,
    viewHeight = viewHeight,
    canvasWidth = canvasWidth,
    canvasHeight = canvasHeight,
    xOffset = xOffset
)
