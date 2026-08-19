package com.live2d.ai.android

import android.graphics.RectF
import android.opengl.GLES20
import android.opengl.Matrix

/**
 * Purism 版 Clipping Manager — 纯 Kotlin 实现的 Cubism Mask Premake 算法。
 *
 * 职责：
 * 1. 收集 mask group → clipped drawable 的映射（collectClippingContexts）
 * 2. 计算每个 mask group 的包围盒（calcClippedTotalBounds）
 * 3. 在 mask texture 上分配布局（setupLayoutBounds）
 * 4. 生成 clip 变换矩阵（createMatrices）
 * 5. 渲染 mask 到 FBO（renderMaskBuffers）
 *
 * 对应 CubismJavaFramework 的 ACubismClippingManager + CubismClippingManagerAndroid。
 *
 * 使用 PurismModel 替代旧版 ModelDataProvider 接口。
 */
class PurismClippingManager(
    private val model: PurismModel
) {

    // ============================================================
    //  数据结构
    // ============================================================

    /**
     * Clipping Context — 唯一 mask group 的运行时数据。
     * 每个 context 对应一组[同一组 mask drawable]覆盖的[一组 clipped drawable]。
     */
    data class ClippingContext(
        /** 此 context 包含的 mask drawable 索引 */
        val clippingIdList: IntArray,
        /** 被这些 mask 覆盖的 drawable 索引 */
        val clippedDrawableIndexList: MutableList<Int> = mutableListOf(),

        // ── 每帧计算（由 setupLayoutBounds 填充）──
        /** 通道索引 0=R, 1=G, 2=B, 3=A */
        var layoutChannelIndex: Int = 0,
        /** layout bounds [left, top, right, bottom] in UV space [0,1] */
        var layoutBounds: FloatArray = floatArrayOf(0f, 0f, 1f, 1f),
        /** maskBuffer 索引 */
        var bufferIndex: Int = 0,

        // ── 每帧计算（由 calcClippedTotalBounds 填充）──
        /** 包围盒 (left, top, right, bottom) in model space */
        var allClippedDrawRect: RectF = RectF(),
        /** 是否使用中（有顶点数据） */
        var isUsing: Boolean = false,

        // ── 每帧计算（由 createMatrices 填充）──
        /** 4x4 矩阵 — 用于 SetupMask vertex shader (u_clipMatrix) */
        var matrixForMask: FloatArray = createIdentityMatrix(),
        /** 4x4 矩阵 — 用于 Masked vertex shader (u_clipMatrix) */
        var matrixForDraw: FloatArray = createIdentityMatrix()
    )

    /**
     * Mask 缓冲区 — 封装 FBO + texture。
     */
    data class MaskBuffer(
        var fboId: Int = 0,
        var textureId: Int = 0,
        val width: Int = DEFAULT_MASK_SIZE,
        val height: Int = DEFAULT_MASK_SIZE
    )

    // ============================================================
    //  状态
    // ============================================================

    /** 唯一的 mask group 列表（clipping context for mask rendering） */
    private val clippingContextListForMask = mutableListOf<ClippingContext>()

    /** drawable 索引 → 对应的 clipping context（无 mask 的 drawable 为 null） */
    private val clippingContextListForDraw = mutableListOf<ClippingContext?>()

    /** FBO 缓冲区列表 */
    private val maskBuffers = mutableListOf<MaskBuffer>()

    /** 需要几个 render texture（默认 1） */
    private var renderTextureCount: Int = 1

    /** FBO / texture 是否已初始化 */
    private var maskBuffersInitialized = false

    /** 上一帧使用的 mask 纹理 ID（用于 bind 优化） */
    private var lastMaskTextureId: Int = -1

    // ── C4-GC：mask 分组静态化状态 ──
    //  drawable→mask 组关联（collectClippingContexts 产物）是 moc3 加载后恒定的
    //  静态模型数据，不再每帧重建；仅在首次 prepareFrame（或 invalidateMaskBuffers
    //  失效）时构建一次。每帧只做变形相关的包围盒/布局/矩阵更新。
    private var clippingContextsInitialized = false

    /** 顶点复用缓冲（容量 = 最大 mask drawable 顶点数 × 2，一次分配，消除每帧 FloatArray） */
    private var boundsVertexBuffer: FloatArray = FloatArray(0)

    // ============================================================
    //  公有方法
    // ============================================================

    /**
     * 每帧入口：更新 mask 包围盒、分配布局、生成矩阵。
     *
     * C4-GC：mask 分组（drawable→mask 组关联）是静态模型数据，仅首次（或
     * invalidateMaskBuffers 失效后）构建一次；每帧仅做变形相关更新
     * （calcClippedTotalBounds 随 deformer 变形，setupLayoutBounds/createMatrices 随包围盒）。
     */
    fun prepareFrame() {
        if (!clippingContextsInitialized) {
            collectClippingContexts()
            clippingContextsInitialized = true
        }
        calcClippedTotalBounds()
        setupLayoutBounds()
        createMatrices()
    }

    /**
     * 获取指定 bufferIndex 的 MaskBuffer。
     */
    fun getMaskBuffer(bufferIndex: Int): MaskBuffer {
        return maskBuffers.getOrElse(bufferIndex) {
            ensureMaskBuffers()
            maskBuffers.getOrElse(bufferIndex) {
                throw IllegalStateException("MaskBuffer $bufferIndex not available")
            }
        }
    }

    /**
     * 获取 drawable 对应的 clipping context（用于主渲染阶段）。
     */
    fun getClippingContextForDraw(drawableIndex: Int): ClippingContext? {
        return clippingContextListForDraw.getOrNull(drawableIndex)
    }

    /**
     * 获取 clipping contexts for mask rendering（用于 mask FBO 渲染）。
     */
    fun getMaskContexts(): List<ClippingContext> = clippingContextListForMask

    /**
     * 获取 mask buffer 列表。
     */
    fun getMaskBuffers(): List<MaskBuffer> = maskBuffers

    /**
     * 返回 clippingContextListForMask 的数量（用于确保 FBO 已初始化）。
     */
    fun getMaskContextCount(): Int = clippingContextListForMask.size

    /**
     * 返回 maskBuffers 是否已初始化。
     */
    fun isMaskBuffersInitialized(): Boolean = maskBuffersInitialized

    /**
     * 渲染所有 mask 到 FBO。
     * 需要在 GL 线程中调用，且必须先调用 prepareFrame()。
     *
     * @param setupMaskShader 已绑定的 SETUP_MASK shader 程序
     * @param drawableFinder  根据 drawable index 查找 DrawableRenderData 的函数
     */
    fun renderMaskBuffers(
        setupMaskShader: ShaderManager.ShaderProgram,
        drawableFinder: (Int) -> Live2DRenderer.DrawableRenderData?,
        textureResolver: (Int) -> Int?
    ) {
        if (clippingContextListForMask.isEmpty()) return

        ensureMaskBuffers()

        // 保存当前 viewport
        val savedViewport = IntArray(4)
        GLES20.glGetIntegerv(GLES20.GL_VIEWPORT, savedViewport, 0)

        GLES20.glUseProgram(setupMaskShader.programId)
        GLES20.glDisable(GLES20.GL_DEPTH_TEST)
        GLES20.glDisable(GLES20.GL_STENCIL_TEST)

        var currentBufferIndex = -1

        for (ctx in clippingContextListForMask) {
            if (!ctx.isUsing) continue

            // 切换 FBO
            if (ctx.bufferIndex != currentBufferIndex) {
                if (currentBufferIndex >= 0) {
                    GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
                }
                currentBufferIndex = ctx.bufferIndex

                val buf = maskBuffers[ctx.bufferIndex]
                GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, buf.fboId)
                GLES20.glViewport(0, 0, buf.width, buf.height)

                // Cubism 约定：白色 (1,1,1,1) = "不画"（mask 被抠除区域）
                GLES20.glClearColor(1f, 1f, 1f, 1f)
                GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
            }

            // 正确 blend mode 用于 mask 渲染
            // 源 = GL_ZERO, 目标 = GL_ONE_MINUS_SRC_COLOR
            // 效果: 在白色画布上"挖出" mask 形状（黑色 = 可见区域）
            GLES20.glEnable(GLES20.GL_BLEND)
            GLES20.glBlendFuncSeparate(
                GLES20.GL_ZERO, GLES20.GL_ONE_MINUS_SRC_COLOR,
                GLES20.GL_ZERO, GLES20.GL_ONE_MINUS_SRC_ALPHA
            )

            // 设置 SetupMask uniforms
            GLES20.glUniformMatrix4fv(setupMaskShader.uMatrix, 1, false, ctx.matrixForMask, 0)
            GLES20.glUniform4fv(setupMaskShader.uChannelFlag, 1, CHANNEL_COLORS[ctx.layoutChannelIndex], 0)

            // u_baseColor = layout bounds 映射到 NDC [-1,1]
            val l = ctx.layoutBounds
            val baseColor = floatArrayOf(
                l[0] * 2f - 1f,      // left   → -1..1
                l[1] * 2f - 1f,      // top    → -1..1
                l[2] * 2f - 1f,      // right  → -1..1
                l[3] * 2f - 1f       // bottom → -1..1
            )
            GLES20.glUniform4fv(setupMaskShader.uBaseColor, 1, baseColor, 0)

            // 渲染此 clip context 的所有 mask drawable
            for (maskIdx in ctx.clippingIdList) {
                val d = drawableFinder(maskIdx) ?: continue

                // 面剔除（与主循环同语义）：单面 mask drawable 剔除 + frontFace=CCW。
                // 若不设，mask pass 沿用上一帧 drawObjectLoop 清理后的 frontFace=CW →
                // CCW 绕向的单面 mask 形状被剔除 → mask 缺失/反向。
                if (d.isDoubleSided) {
                    GLES20.glDisable(GLES20.GL_CULL_FACE)
                } else {
                    GLES20.glEnable(GLES20.GL_CULL_FACE)
                    GLES20.glFrontFace(GLES20.GL_CCW)
                }

                // 绑定 mask drawable 自己的纹理
                val texId = textureResolver(d.textureIndex) ?: continue
                GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
                GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texId)
                GLES20.glUniform1i(setupMaskShader.uTexture0, 0)

                // mask drawable 的 multiply/screen colors
                GLES20.glUniform4fv(setupMaskShader.uMultiplyColor, 1, d.multiplyColor, 0)
                GLES20.glUniform4fv(setupMaskShader.uScreenColor, 1, d.screenColor, 0)

                // 传递 vertex 数据
                GLES20.glEnableVertexAttribArray(setupMaskShader.aPosition)
                d.positionsBuffer.position(0)
                GLES20.glVertexAttribPointer(setupMaskShader.aPosition, 2, GLES20.GL_FLOAT, false, 0, d.positionsBuffer)

                if (setupMaskShader.aTexCoord >= 0) {
                    GLES20.glEnableVertexAttribArray(setupMaskShader.aTexCoord)
                    d.uvsBuffer.position(0)
                    GLES20.glVertexAttribPointer(setupMaskShader.aTexCoord, 2, GLES20.GL_FLOAT, false, 0, d.uvsBuffer)
                }

                d.indicesBuffer.position(0)
                GLES20.glDrawElements(GLES20.GL_TRIANGLES, d.indexCount, GLES20.GL_UNSIGNED_SHORT, d.indicesBuffer)

                GLES20.glDisableVertexAttribArray(setupMaskShader.aPosition)
                if (setupMaskShader.aTexCoord >= 0) {
                    GLES20.glDisableVertexAttribArray(setupMaskShader.aTexCoord)
                }
            }
        }

        // 结束最后的 FBO
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
        GLES20.glUseProgram(0)

        // 恢复 clear color（mask pass 清除为 (1,1,1,1) 白——超采样分支在 mask pass 之后才 glClear，
        // 不恢复则主渲染清屏变白 → 必须恢复 (0,0,0,0)）
        GLES20.glClearColor(0f, 0f, 0f, 0f)

        // 恢复 viewport
        GLES20.glViewport(savedViewport[0], savedViewport[1], savedViewport[2], savedViewport[3])

        // 恢复标准 blend
        GLES20.glBlendFunc(GLES20.GL_SRC_ALPHA, GLES20.GL_ONE_MINUS_SRC_ALPHA)
        GLES20.glEnable(GLES20.GL_CULL_FACE)
        GLES20.glFrontFace(GLES20.GL_CW)

        lastMaskTextureId = -1
    }

    /**
     * 清理 OpenGL 资源。
     */
    fun dispose() {
        for (buf in maskBuffers) {
            if (buf.fboId != 0) {
                GLES20.glDeleteFramebuffers(1, intArrayOf(buf.fboId), 0)
            }
            if (buf.textureId != 0) {
                GLES20.glDeleteTextures(1, intArrayOf(buf.textureId), 0)
            }
        }
        maskBuffers.clear()
        maskBuffersInitialized = false
        clippingContextsInitialized = false
        clippingContextListForMask.clear()
        clippingContextListForDraw.clear()
        boundsVertexBuffer = FloatArray(0)
    }

    // ============================================================
    //  内部方法 — Mask 分组
    // ============================================================

    /**
     * 收集 clipping context（**模型加载后只执行一次**）。
     * 等效于 CubismJavaFramework 的 ACubismClippingManager.initialize()。
     *
     * C4-GC：
     * - mask 分组关系是静态模型数据 → 由 prepareFrame 首次调用 / invalidateMaskBuffers 触发重建；
     * - 整数 Long key（maskIds 排序后按位打包）替代 `sorted().joinToString(",")` 消除 String 分配；
     * - mask 数据全部走 PurismModel 静态缓存（零 JNI/零数组分配）；
     * - 按最大 mask drawable 顶点数一次分配顶点复用缓冲。
     */
    private fun collectClippingContexts() {
        val count = model.getDrawableCount()

        // 静态缓存：mask 计数 / mask 索引列表 / 顶点数（模型内恒定，仅首次访问走 JNI）
        val maskCounts = model.getDrawableMaskCountsCached()
        val maskLists = model.getDrawableMasksCached()
        val vertexCounts = model.getDrawableVertexCountsCached()

        // C4-GC：Long key 替代 String key（≤4 个 mask 按位打包精确无碰撞；>4 折叠+内容比对兜底）
        val maskKeyToContext = HashMap<Long, ClippingContext>()
        clippingContextListForMask.clear()
        clippingContextListForDraw.clear()

        var maxMaskVertexCount = 0

        for (drawableIndex in 0 until count) {
            val maskCount = maskCounts.getOrElse(drawableIndex) { 0 }
            if (maskCount == 0) {
                clippingContextListForDraw.add(null)
                continue
            }

            val maskIds = maskLists.getOrElse(drawableIndex) { IntArray(0) }
            val sortedIds = maskIds.sortedArray()
            val key = maskGroupKey(sortedIds)

            var ctx = maskKeyToContext[key]
            if (ctx == null) {
                ctx = ClippingContext(
                    clippingIdList = maskIds,
                    clippedDrawableIndexList = mutableListOf(drawableIndex)
                )
                maskKeyToContext[key] = ctx
                clippingContextListForMask.add(ctx)
            } else {
                // Long key 碰撞兜底（仅 >4 mask 折叠场景可达；≤4 打包精确）:
                // 以排序内容做集合相等比对，防止不同 mask 集合误合并。
                if (ctx.clippingIdList.size != maskIds.size || !sameMaskSet(ctx.clippingIdList, sortedIds)) {
                    val exact = clippingContextListForMask.firstOrNull {
                        it.clippingIdList.size == maskIds.size && sameMaskSet(it.clippingIdList, sortedIds)
                    }
                    if (exact != null) {
                        ctx = exact
                    } else {
                        ctx = ClippingContext(
                            clippingIdList = maskIds,
                            clippedDrawableIndexList = mutableListOf(drawableIndex)
                        )
                        maskKeyToContext[key] = ctx
                        clippingContextListForMask.add(ctx)
                    }
                }
                ctx.clippedDrawableIndexList.add(drawableIndex)
            }
            clippingContextListForDraw.add(ctx)

            // 统计最大 mask drawable 顶点数 → 顶点复用缓冲容量（一次分配）
            for (mi in maskIds) {
                val vc = vertexCounts.getOrElse(mi) { 0 }
                if (vc > maxMaskVertexCount) maxMaskVertexCount = vc
            }
        }

        // C4-GC：顶点复用缓冲一次分配（容量 = 最大 mask drawable 顶点数 × 2 交错 xy）
        if (boundsVertexBuffer.size < maxMaskVertexCount * 2) {
            boundsVertexBuffer = FloatArray(maxMaskVertexCount * 2)
        }
    }

    /**
     * mask 集合相等判定（忽略顺序；a 与 sortedB 比较）。
     */
    private fun sameMaskSet(a: IntArray, sortedB: IntArray): Boolean {
        if (a.size != sortedB.size) return false
        val sortedA = a.sortedArray()
        for (i in sortedA.indices) {
            if (sortedA[i] != sortedB[i]) return false
        }
        return true
    }

    /**
     * mask 组 Long key：排序后按位打包（每个 mask 索引 16 位，≤4 个精确无碰撞；
     * 超 4 个折叠剩余 id 作哈希，由 sameMaskSet 内容比对兜底）。
     */
    private fun maskGroupKey(sortedIds: IntArray): Long {
        var key = 0L
        val n = sortedIds.size
        for (i in 0 until minOf(n, 4)) {
            key = (key shl 16) or (sortedIds[i].toLong() and 0xFFFFL)
        }
        if (n > 4) {
            for (i in 4 until n) {
                key = key xor (sortedIds[i].toLong() and 0xFFFFL)
            }
        }
        return key
    }

    // ============================================================
    //  内部方法 — 包围盒计算
    // ============================================================

    /**
     * 计算每个 clip context 的包围盒（模型单位空间）。
     *
     * 对应 CubismJavaFramework 的 ACubismClippingManager.calcClippedTotalBounds()。
     * MASK_MARGIN = 0.05 — 包围盒外扩 5% margin。
     *
     * C4-GC（分配风暴修复）：
     * - 每帧仍执行（mask 顶点随 deformer 变形）；
     * - 只处理可见（DYNAMIC_IS_VISIBLE，与主循环同源）的 mask drawable（任务 2a）；
     *   安全兜底：某 context 的 mask 全部不可见时回退全量，防 mask 整组消失；
     * - 顶点经 fillDrawablePositions 写入复用 FloatArray 缓冲（成员变量一次分配），
     *   消除每帧 86 次 getDrawableVertices JNI FloatArray 分配（任务 2b）。
     */
    private fun calcClippedTotalBounds() {
        // 每帧单次取动态可见性位（475 字节小数组；渲染器主循环同源 DYNAMIC_IS_VISIBLE）
        val dynFlags = Live2DNative.getDrawableDynamicFlags(model.modelPtr)
        val vertexCounts = model.getDrawableVertexCountsCached()

        for (ctx in clippingContextListForMask) {
            var minX = Float.MAX_VALUE
            var minY = Float.MAX_VALUE
            var maxX = -Float.MAX_VALUE
            var maxY = -Float.MAX_VALUE
            var hasVertices = false

            // 可见过滤：任一 mask drawable 可见才启用过滤；全不可见 → 回退全量（防整组消失）
            var anyVisible = false
            if (dynFlags != null) {
                for (maskIdx in ctx.clippingIdList) {
                    if ((dynFlags.getOrElse(maskIdx) { 0 }.toInt() and Live2DNative.DYNAMIC_IS_VISIBLE) != 0) {
                        anyVisible = true
                        break
                    }
                }
            }

            for (maskIdx in ctx.clippingIdList) {
                if (dynFlags != null && anyVisible &&
                    (dynFlags.getOrElse(maskIdx) { 0 }.toInt() and Live2DNative.DYNAMIC_IS_VISIBLE) == 0
                ) {
                    continue
                }

                // 复用缓冲写入（容量防御：顶点数为模型内恒定静态值，首帧已按 max 预分配）
                val vc = vertexCounts.getOrElse(maskIdx) { 0 }
                if (boundsVertexBuffer.size < vc * 2) {
                    boundsVertexBuffer = FloatArray(vc * 2)
                }
                val written = Live2DNative.fillDrawablePositions(model.modelPtr, maskIdx, boundsVertexBuffer)
                if (written <= 0) continue

                val n = written * 2
                var vi = 0
                while (vi < n) {
                    val x = boundsVertexBuffer[vi]
                    val y = boundsVertexBuffer[vi + 1]
                    if (x < minX) minX = x
                    if (y < minY) minY = y
                    if (x > maxX) maxX = x
                    if (y > maxY) maxY = y
                    vi += 2
                }
                hasVertices = true
            }

            if (!hasVertices) {
                ctx.isUsing = false
                continue
            }

            // 外扩 margin
            val marginX = (maxX - minX) * MASK_MARGIN
            val marginY = (maxY - minY) * MASK_MARGIN

            ctx.allClippedDrawRect = RectF(
                minX - marginX, minY - marginY,
                maxX + marginX, maxY + marginY
            )
            ctx.isUsing = true
        }
    }

    // ============================================================
    //  内部方法 — Layout Bounds 分配
    // ============================================================

    /**
     * 在 mask texture 上分配每个 clip context 的 UV 区域。
     * 等效于 ACubismClippingManager.setupLayoutBounds()。
     */
    private fun setupLayoutBounds() {
        val usingCount = clippingContextListForMask.count { it.isUsing }
        if (usingCount == 0) return

        renderTextureCount = maxOf(1, (usingCount + CLIPPING_MASK_MAX_COUNT_ON_DEFAULT - 1) / CLIPPING_MASK_MAX_COUNT_ON_DEFAULT)

        var currentCtx = 0

        for (rtIdx in 0 until renderTextureCount) {
            val startIdx = currentCtx
            val endIdx = minOf(currentCtx + CLIPPING_MASK_MAX_COUNT_ON_DEFAULT, usingCount)
            val remaining = endIdx - startIdx

            if (remaining <= 0) break

            val channelAssigned = IntArray(4) { 0 }

            if (remaining <= 4) {
                channelAssigned[0] = remaining
            } else {
                val perChannel = (remaining + 3) / 4
                var allocated = 0
                for (c in 0 until 4) {
                    val count = minOf(perChannel, remaining - allocated)
                    channelAssigned[c] = count
                    allocated += count
                    if (allocated >= remaining) break
                }
            }

            for (ch in 0 until 4) {
                val count = channelAssigned[ch]
                if (count == 0) continue

                val channelStartCtx = startIdx + channelAssigned.take(ch).sum()

                val (cols, rows) = when {
                    count <= 1 -> Pair(1, 1)
                    count <= 2 -> Pair(2, 1)
                    count <= 4 -> Pair(2, 2)
                    else -> Pair(3, 3)
                }

                val cellW = 1.0f / cols
                val cellH = 1.0f / rows

                for (cell in 0 until count) {
                    val ctxIdx = channelStartCtx + cell
                    if (ctxIdx >= clippingContextListForMask.size) break
                    val ctx = clippingContextListForMask[ctxIdx]
                    if (!ctx.isUsing) continue

                    val col = cell % cols
                    val row = cell / cols

                    ctx.layoutChannelIndex = ch
                    ctx.layoutBounds = floatArrayOf(
                        col * cellW,
                        row * cellH,
                        (col + 1) * cellW,
                        (row + 1) * cellH
                    )
                    ctx.bufferIndex = rtIdx
                }
            }

            currentCtx = endIdx
        }
    }

    // ============================================================
    //  内部方法 — 矩阵生成
    // ============================================================

    /**
     * 生成每个 clip context 的两个矩阵。
     *
     * matrixForMask: 用于 SetupMask vertex shader (u_clipMatrix)
     * matrixForDraw: 用于 Masked vertex shader (u_clipMatrix)
     *
     * 对应 CubismJavaFramework 的 ACubismClippingManager.createMatrixForMask()
     * 和 createMatrixForDraw()。
     */
    private fun createMatrices() {
        for (ctx in clippingContextListForMask) {
            if (!ctx.isUsing) continue

            val bounds = ctx.allClippedDrawRect
            val layout = ctx.layoutBounds

            val boundsWidth = bounds.width()
            val boundsHeight = bounds.height()
            val layoutWidth = layout[2] - layout[0]
            val layoutHeight = layout[3] - layout[1]

            if (boundsWidth <= 0f || boundsHeight <= 0f) {
                ctx.isUsing = false
                continue
            }

            ctx.matrixForMask = createOrthoForMask(ctx)
            ctx.matrixForDraw = createClipMatrixForDraw(ctx)
        }
    }

    /**
     * 创建 matrixForMask。
     *
     * 顶点管线（先应用者先写，对齐 Cubism ACubismClippingManager.createMatrixForMask）：
     *   v -= bounds 原点 → 缩放到 layout 尺寸 → 平移到 layout UV 位置 → ×2 去 NDC → −1,−1 居中
     *   即 M = T(−1,−1) · S(2,2) · T(layout) · S(scale) · T(−bounds)
     * ⚠️ Android Matrix.translateM/scaleM 为**右乘**（m = m·X，后调用者先作用于顶点），
     * 故下面按“顶点先应用”的逆序调用（T(−bounds) 最后调用 = 最先作用于顶点）。
     * 输出 = NDC 坐标（SetupMask 片元用 u_baseColor（layout NDC 边界）做 isInside 边界测试）。
     */
    private fun createOrthoForMask(ctx: ClippingContext): FloatArray {
        val bounds = ctx.allClippedDrawRect
        val layout = ctx.layoutBounds

        val boundsWidth = bounds.width()
        val boundsHeight = bounds.height()
        val layoutWidth = layout[2] - layout[0]
        val layoutHeight = layout[3] - layout[1]

        val scaleX = layoutWidth / boundsWidth
        val scaleY = layoutHeight / boundsHeight

        val m = FloatArray(16)
        Matrix.setIdentityM(m, 0)

        // 目标矩阵 M = T(−bounds) · S(scale) · T(layout) · S(2) · T(−1,−1)
        // Android Matrix.*M 为右乘（m ← m·X），后调用者先作用于顶点 → 按顶点管线逆序调用：
        Matrix.translateM(m, 0, -1f, -1f, 0f)                     // 最后作用于顶点：UV → NDC 居中
        Matrix.scaleM(m, 0, 2f, 2f, 1f)                           // UV [0,1] → [0,2]
        Matrix.translateM(m, 0, layout[0], layout[1], 0f)         // 平移到 layout UV 位置
        Matrix.scaleM(m, 0, scaleX, scaleY, 1f)                   // 缩放到 layout 尺寸
        Matrix.translateM(m, 0, -bounds.left, -bounds.top, 0f)    // 最先作用于顶点：v -= bounds 原点

        return m
    }

    /**
     * 创建 matrixForDraw。
     *
     * 顶点管线（先应用者先写，对齐 Cubism ACubismClippingManager.createMatrixForDraw）：
     *   v -= bounds 原点 → 缩放到 layout 尺寸 → 平移到 layout UV 位置
     *   即 M = T(layout) · S(scale) · T(−bounds)
     * ⚠️ Android Matrix.translateM/scaleM 为**右乘**（m = m·X，后调用者先作用于顶点），
     * 故下面按“顶点先应用”的逆序调用（T(−bounds) 最后调用 = 最先作用于顶点）。
     * 输出 = mask 纹理 UV 空间 [0,1]（Masked 片元用 v_clipPos.xy/w 采样 s_texture1）。
     */
    private fun createClipMatrixForDraw(ctx: ClippingContext): FloatArray {
        val bounds = ctx.allClippedDrawRect
        val layout = ctx.layoutBounds

        val boundsWidth = bounds.width()
        val boundsHeight = bounds.height()
        val layoutWidth = layout[2] - layout[0]
        val layoutHeight = layout[3] - layout[1]

        val scaleX = layoutWidth / boundsWidth
        val scaleY = layoutHeight / boundsHeight

        val m = FloatArray(16)
        Matrix.setIdentityM(m, 0)

        // 目标矩阵 M = T(layout) · S(scale) · T(−bounds)
        // Android Matrix.*M 为右乘（m ← m·X），后调用者先作用于顶点 → 按顶点管线逆序调用：
        Matrix.translateM(m, 0, layout[0], layout[1], 0f)         // 最后作用于顶点：平移到 layout UV 位置
        Matrix.scaleM(m, 0, scaleX, scaleY, 1f)                   // 缩放到 layout 尺寸
        Matrix.translateM(m, 0, -bounds.left, -bounds.top, 0f)    // 最先作用于顶点：v -= bounds 原点

        return m
    }

    // ============================================================
    //  内部方法 — FBO 管理
    // ============================================================

    /**
     * 确保 mask FBO 和 texture 已创建。
     */
    fun ensureMaskBuffers() {
        if (maskBuffersInitialized && maskBuffers.size >= renderTextureCount) return

        if (maskBuffers.isNotEmpty()) {
            for (buf in maskBuffers) {
                if (buf.fboId != 0) GLES20.glDeleteFramebuffers(1, intArrayOf(buf.fboId), 0)
                if (buf.textureId != 0) GLES20.glDeleteTextures(1, intArrayOf(buf.textureId), 0)
            }
            maskBuffers.clear()
        }

        for (i in 0 until renderTextureCount) {
            val texIds = IntArray(1)
            GLES20.glGenTextures(1, texIds, 0)
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texIds[0])
            GLES20.glTexImage2D(
                GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA,
                DEFAULT_MASK_SIZE, DEFAULT_MASK_SIZE, 0,
                GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, null
            )
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)

            val fboIds = IntArray(1)
            GLES20.glGenFramebuffers(1, fboIds, 0)
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, fboIds[0])
            GLES20.glFramebufferTexture2D(
                GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0,
                GLES20.GL_TEXTURE_2D, texIds[0], 0
            )

            val status = GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER)
            if (status != GLES20.GL_FRAMEBUFFER_COMPLETE) {
                android.util.Log.w("PurismClippingManager",
                    "Mask FBO $i incomplete: status=$status")
            }

            maskBuffers.add(MaskBuffer(
                fboId = fboIds[0],
                textureId = texIds[0],
                width = DEFAULT_MASK_SIZE,
                height = DEFAULT_MASK_SIZE
            ))
        }

        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, 0)
        maskBuffersInitialized = true
    }

    /**
     * 销毁旧 FBO 并标记需要重建。
     * C4-GC：同时失效静态 mask 分组（下次 prepareFrame 重建），
     * 供模型切换/资源失效场景复用。
     */
    fun invalidateMaskBuffers() {
        clippingContextsInitialized = false
        clippingContextListForMask.clear()
        clippingContextListForDraw.clear()
        maskBuffersInitialized = false
        if (maskBuffers.isNotEmpty()) {
            for (buf in maskBuffers) {
                if (buf.fboId != 0) GLES20.glDeleteFramebuffers(1, intArrayOf(buf.fboId), 0)
                if (buf.textureId != 0) GLES20.glDeleteTextures(1, intArrayOf(buf.textureId), 0)
            }
            maskBuffers.clear()
        }
    }

    companion object {
        /** 创建一个 4x4 单位矩阵 */
        fun createIdentityMatrix(): FloatArray {
            val m = FloatArray(16)
            Matrix.setIdentityM(m, 0)
            return m
        }

        /** 通道颜色: R(1,0,0,0), G(0,1,0,0), B(0,0,1,0), A(0,0,0,1) */
        val CHANNEL_COLORS = arrayOf(
            floatArrayOf(1f, 0f, 0f, 0f),
            floatArrayOf(0f, 1f, 0f, 0f),
            floatArrayOf(0f, 0f, 1f, 0f),
            floatArrayOf(0f, 0f, 0f, 1f),
        )

        /** 默认 mask 纹理尺寸 */
        const val DEFAULT_MASK_SIZE: Int = 512

        /** 单 RT 时最大 mask 数（9 格 × 4 通道） */
        const val CLIPPING_MASK_MAX_COUNT_ON_DEFAULT: Int = 36

        /** 包围盒外扩 margin */
        const val MASK_MARGIN: Float = 0.05f
    }
}
