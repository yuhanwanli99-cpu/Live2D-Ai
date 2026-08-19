package com.live2d.ai.android

import android.content.Context
import android.opengl.GLES20
import android.util.Log

/**
 * 着色器管理器 — 从 assets/shaders/ 加载、编译、缓存、选择着色器。
 *
 * 支持 4 种着色器类型：
 * - SETUP_MASK: mask FBO 渲染
 * - NORMAL: 无 mask 的主渲染
 * - MASKED: 有 mask 的主渲染
 * - MASKED_INVERTED: 反向 mask 的主渲染
 *
 * 从 CubismJavaFramework 提取的着色器文件（assets/shaders/）：
 *   VertShaderSrc.vert / FragShaderSrc.frag
 *   VertShaderSrcSetupMask.vert / FragShaderSrcSetupMask.frag
 *   VertShaderSrcMasked.vert / FragShaderSrcMask.frag
 *   FragShaderSrcMaskInverted.frag
 */
class ShaderManager(private val context: Context) {

    enum class ShaderType {
        SETUP_MASK,
        NORMAL,
        MASKED,
        MASKED_INVERTED
    }

    /**
     * 编译好的着色器程序及其 uniform/attribute 位置。
     * aTexCoord = -1 表示该着色器无纹理坐标属性（当前全部有）。
     * uClipMatrix = -1 表示无 clip 矩阵（NORMAL/SETUP_MASK 无 clip 矩阵 uniform NORMAL）。
     * uChannelFlag = -1 表示无通道选择（NORMAL 无）。
     * uTexture1 = -1 表示无 mask 纹理（NORMAL/SETUP_MASK 无）。
     */
    data class ShaderProgram(
        val programId: Int,
        val aPosition: Int,
        val aTexCoord: Int,
        val uMatrix: Int,
        val uClipMatrix: Int,
        val uBaseColor: Int,
        val uMultiplyColor: Int,
        val uScreenColor: Int,
        val uChannelFlag: Int,
        val uTexture0: Int,
        val uTexture1: Int
    )

    private val programs = mutableMapOf<ShaderType, ShaderProgram>()

    /**
     * 初始化：加载并编译所有着色器。
     * 在 GL 线程（onSurfaceCreated）中调用。
     */
    fun initialize() {
        loadAndCompile(ShaderType.SETUP_MASK, "VertShaderSrcSetupMask", "FragShaderSrcSetupMask")
        loadAndCompile(ShaderType.NORMAL, "VertShaderSrc", "FragShaderSrc")
        loadAndCompile(ShaderType.MASKED, "VertShaderSrcMasked", "FragShaderSrcMask")
        loadAndCompile(ShaderType.MASKED_INVERTED, "VertShaderSrcMasked", "FragShaderSrcMaskInverted")
    }

    /**
     * 获取指定类型的着色器。
     */
    fun get(type: ShaderType): ShaderProgram {
        return programs[type] ?: throw IllegalStateException("Shader $type not initialized")
    }

    /**
     * 根据 drawable 属性选择合适的着色器类型。
     */
    fun selectShaderType(isMasked: Boolean, isInvertedMask: Boolean): ShaderType {
        return when {
            isMasked && isInvertedMask -> ShaderType.MASKED_INVERTED
            isMasked -> ShaderType.MASKED
            else -> ShaderType.NORMAL
        }
    }

    /**
     * 释放所有着色器程序。
     */
    fun dispose() {
        for (prog in programs.values) {
            GLES20.glDeleteProgram(prog.programId)
        }
        programs.clear()
    }

    /**
     * 从 assets 加载着色器源码、编译、链接。
     */
    private fun loadAndCompile(type: ShaderType, vertName: String, fragName: String) {
        val vertSrc = loadAsset("shaders/$vertName.vert")
        val fragSrc = loadAsset("shaders/$fragName.frag")
        val programId = compileAndLink("${type.name}", vertSrc, fragSrc)
        programs[type] = extractLocations(type, programId)
    }

    /**
     * 从 assets 读取文本文件。
     */
    private fun loadAsset(path: String): String {
        return context.assets.open(path).bufferedReader().use { it.readText() }
    }

    /**
     * 编译 vertex + fragment shader 并链接程序。
     */
    private fun compileAndLink(label: String, vertSrc: String, fragSrc: String): Int {
        val vertexShader = compileShader(GLES20.GL_VERTEX_SHADER, vertSrc)
        val fragmentShader = compileShader(GLES20.GL_FRAGMENT_SHADER, fragSrc)

        val program = GLES20.glCreateProgram()
        if (program == 0) {
            throw RuntimeException("glCreateProgram returned 0 for $label")
        }
        GLES20.glAttachShader(program, vertexShader)
        GLES20.glAttachShader(program, fragmentShader)
        GLES20.glLinkProgram(program)

        val linkStatus = IntArray(1)
        GLES20.glGetProgramiv(program, GLES20.GL_LINK_STATUS, linkStatus, 0)
        if (linkStatus[0] == 0) {
            val log = GLES20.glGetProgramInfoLog(program)
            GLES20.glDeleteProgram(program)
            throw RuntimeException("Shader link failed ($label): $log")
        }

        // 链接完成后可以删除 shader 对象
        GLES20.glDeleteShader(vertexShader)
        GLES20.glDeleteShader(fragmentShader)

        return program
    }

    /**
     * 编译单个着色器。
     */
    private fun compileShader(type: Int, source: String): Int {
        val shader = GLES20.glCreateShader(type)
        if (shader == 0) {
            throw RuntimeException("glCreateShader returned 0 (type=$type)")
        }
        GLES20.glShaderSource(shader, source)
        GLES20.glCompileShader(shader)

        val compileStatus = IntArray(1)
        GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, compileStatus, 0)
        if (compileStatus[0] == 0) {
            val log = GLES20.glGetShaderInfoLog(shader)
            val typeName = if (type == GLES20.GL_VERTEX_SHADER) "vertex" else "fragment"
            GLES20.glDeleteShader(shader)
            throw RuntimeException("$typeName shader compile error: $log")
        }
        return shader
    }

    /**
     * 提取 uniform/attribute 位置缓存。
     * 不同着色器类型有不同 uniform 集合：
     *
     * SETUP_MASK: u_clipMatrix (=uMatrix), u_channelFlag, u_baseColor, s_texture0
     * NORMAL: u_matrix (=uMatrix), u_baseColor, u_multiplyColor, u_screenColor, s_texture0
     * MASKED/MASKED_INVERTED: u_matrix, u_clipMatrix, u_channelFlag, u_baseColor, u_multiplyColor, u_screenColor, s_texture0, s_texture1
     */
    private fun extractLocations(type: ShaderType, programId: Int): ShaderProgram {
        val aPosition = GLES20.glGetAttribLocation(programId, "a_position")
        val aTexCoord = GLES20.glGetAttribLocation(programId, "a_texCoord")

        // 统一位置映射:
        // SETUP_MASK 使用 u_clipMatrix 作为主矩阵
        val uMatrix: Int
        val uClipMatrix: Int

        when (type) {
            ShaderType.SETUP_MASK -> {
                uMatrix = GLES20.glGetUniformLocation(programId, "u_clipMatrix")
                uClipMatrix = -1
            }
            ShaderType.NORMAL -> {
                uMatrix = GLES20.glGetUniformLocation(programId, "u_matrix")
                uClipMatrix = -1
            }
            ShaderType.MASKED, ShaderType.MASKED_INVERTED -> {
                uMatrix = GLES20.glGetUniformLocation(programId, "u_matrix")
                uClipMatrix = GLES20.glGetUniformLocation(programId, "u_clipMatrix")
            }
        }

        val uBaseColor = GLES20.glGetUniformLocation(programId, "u_baseColor")
        val uMultiplyColor = GLES20.glGetUniformLocation(programId, "u_multiplyColor")
        val uScreenColor = GLES20.glGetUniformLocation(programId, "u_screenColor")

        // NORMAL 没有 u_channelFlag
        val uChannelFlag: Int
        val uTexture1: Int

        when (type) {
            ShaderType.NORMAL -> {
                uChannelFlag = -1
                uTexture1 = -1
            }
            ShaderType.SETUP_MASK -> {
                uChannelFlag = GLES20.glGetUniformLocation(programId, "u_channelFlag")
                uTexture1 = -1
            }
            ShaderType.MASKED, ShaderType.MASKED_INVERTED -> {
                uChannelFlag = GLES20.glGetUniformLocation(programId, "u_channelFlag")
                uTexture1 = GLES20.glGetUniformLocation(programId, "s_texture1")
            }
        }

        val uTexture0 = GLES20.glGetUniformLocation(programId, "s_texture0")

        return ShaderProgram(
            programId = programId,
            aPosition = aPosition,
            aTexCoord = aTexCoord,
            uMatrix = uMatrix,
            uClipMatrix = uClipMatrix,
            uBaseColor = uBaseColor,
            uMultiplyColor = uMultiplyColor,
            uScreenColor = uScreenColor,
            uChannelFlag = uChannelFlag,
            uTexture0 = uTexture0,
            uTexture1 = uTexture1
        )
    }
}
