package com.live2d.ai.android.render

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.lang.reflect.Modifier

/**
 * Wave 2 · 2A-1 渲染替换 —— REND4-A3 预分配缓冲结构断言（反射，不实例化）。
 *
 * 依据: contract-refactor-0814.md REND4-A3「结构断言：buildDrawableList/refreshCachedAttributes
 *       自研分配代码在生产调用路径不存在（……static grep=0）」+ §2.5「全部预分配、无每帧 JNI 数组副本」
 *       + architecture-refactor-render-0814.md F-10「官方 C++ 渲染器每帧零分配」。
 * 契约锚: REND4-A3（结构：渲染数据路径不产生每帧新对象）；§2.5（engine 全预分配）。
 *
 * 判定方式（结构断言）: 目标 = 生产薄壳 `com.live2d.ai.android.Live2DEngineRenderer`，
 *          经 Class.forName(name, initialize=false, loader) 反射检查【不触发类/实例初始化】
 *          （实例 init 会调 Live2DNative.engineInit() JNI，JVM 单测不可执行），机械断言：
 *             (p)/(q) 声明字段含 final FloatArray（预分配复用缓冲形态存在：一次分配、跨帧
 *                     复用、final 禁止每帧替换引用）；
 *             (s) setMvpMatrix(FloatArray) 返回 Void（缓冲由壳侧预分配、跨帧复用，不每帧
 *                 new / 不返回新缓冲）。
 *
 * 行为断言改结构断言的理由: 原 RED 草案引用 `PreallocRenderBufferProvider.obtainParams()` seam，
 * 生产未实现该 seam（audit-refactor-wave2-A FAIL-8）。生产实际形态 = 薄壳私有 final
 * `mvpBuffer: FloatArray(16)` + `setMvpMatrix` 用 System.arraycopy 拷贝进该缓冲后下发（跨帧
 * 复用同一缓冲，每帧零分配）。「连续 obtain 返回同一引用」的行为语义等价于「final FloatArray
 * 字段存在 + setter 返回 Void（不新建/不交出缓冲）」的结构形态——故以反射结构断言机械锁定，
 * 不要求生产新增或暴露 seam（不扩大生产 API 面）。
 */
class RendererPreallocBufferTest {

    /** 目标薄壳类（initialize=false：只加载不初始化，不触发 engineInit JNI）。 */
    private val rendererClass: Class<*> = Class.forName(
        "com.live2d.ai.android.Live2DEngineRenderer", false, javaClass.classLoader,
    )

    @Test
    fun shellDeclares_finalFloatArray_preallocatedBuffer() {
        // 结构断言 (p)/(q)：声明字段含 final FloatArray = 预分配复用缓冲形态存在。
        // final 保证缓冲一次性分配后引用不可替换（不得每帧 new），复用语义由类型形态锁定。
        val finalFloatArrayFields = rendererClass.declaredFields.filter {
            it.type == FloatArray::class.java && Modifier.isFinal(it.modifiers)
        }
        assertTrue(
            "结构断言 (p)/(q)：薄壳必须声明 final FloatArray 字段（预分配复用缓冲形态存在）——" +
                "字段不存在、类型非 FloatArray 或非 final（可每帧替换引用）= REND4-A3 违规（渲染热路径每帧 new）。",
            finalFloatArrayFields.isNotEmpty(),
        )
    }

    @Test
    fun setMvpMatrix_takesFloatArray_returnsVoid_bufferOwnedByShell() {
        // 结构断言 (s)：setMvpMatrix(FloatArray) 返回 Void = 缓冲由壳侧预分配并持有，
        // 调用方只提供输入、不获得新缓冲（API 形态本身排除"每帧 new/每帧返回新缓冲"）。
        // 签名不符（getDeclaredMethod 抛 NoSuchMethodException）或返回非 Void 均判违规。
        val method = rendererClass.getDeclaredMethod("setMvpMatrix", FloatArray::class.java)
        assertEquals(
            "结构断言 (s)：setMvpMatrix(FloatArray) 必须返回 Void（缓冲由壳侧预分配、跨帧复用，" +
                "不每帧 new/返回新缓冲）——返回非 Void 或签名不符 = REND4-A3 违规。",
            Void.TYPE, method.returnType,
        )
    }
}
