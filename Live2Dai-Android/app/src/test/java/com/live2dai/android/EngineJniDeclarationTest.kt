package com.live2dai.android

import java.lang.reflect.Method
import java.lang.reflect.Modifier
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Wave 1 · M1 engine 骨架 + JNI 声明切换 —— RED 测试。
 *
 * 依据: contract-refactor-0814.md §2.5（engine JNI 面草案）+ §3 M1 断言 + schedule Wave 1-2。
 * 契约锚: §2.5 "engine* 系列：engineInit / engineLoadModel(moc3, model3.json, cdi3) /
 *         engineUpdate(dt) / engineDraw / engineSetLipSync / engineSetMvpMatrix(float[16]) /
 *         engineSetParameters / engineRelease。约束：全部预分配、无每帧 JNI 数组副本；
 *         loadLibrary("live2dcubismcore") 名称保留。"
 *
 * 判定方式: 纯 JVM 反射断言（J-P1 机械判定）。通过 getDeclaredMethods() 检查
 *          Live2DNative 类是否声明 engine* 系列 native 方法及其参数类型。
 *          当前声明尚未切换为 engine* → 反射断言失败 = RED。
 *
 * 说明（契约锁定前草案）:
 *   - 契约 §2.5 只给函数名 + 语义，未锁定 Kotlin 签名。本测试采用的签名映射表见
 *     wave1-m1-red.md §3，标注「契约锁定前草案」。coder 须按该表实现，
 *     Live2DNative.kt 声明 native 方法签名与之一致，否则本测试不转 GREEN。
 *   - JVM 侧绝不真调用 native 方法（会 UnsatisfiedLinkError），只做声明面反射断言。
 *   - 「无每帧 JNI 数组副本」与「loadLibrary("live2dcubismcore")」两项约束：
 *       * loadLibrary 由 REND4/跨架构收敛期 device 证据链覆盖（本 RED 波不断言）。
 *       * 每帧零副本归 REND4-A1（device，Wave 6 验收），本测试只锁声明面。
 */
class EngineJniDeclarationTest {

    // 用 Class.forName 定位（不依赖 Live2DNative 是 object 还是 class 的声明形态——
    // 该形态属性 M1 实现自由面，测试不应绑定。JVM 下 getDeclaredMethods 可遍历 object/class 的成员）。
    private val targetClass: Class<*> = resolveLive2DNative()

    private fun resolveLive2DNative(): Class<*> =
        Class.forName("com.live2dai.android.Live2DNative")

    /**
     * 预期 engine 系列 native 方法签名映射表（契约锁定前草案，与 wave1-m1-red.md §3 同源）。
     * key = 方法名；value = 参数类型数组（Class）+ 返回类型。
     */
    private data class ExpectedNative(
        val name: String,
        val paramTypes: List<Class<*>>,
        val returnType: Class<*>,
    )

    private val expectedNatives: List<ExpectedNative> = listOf(
        // engineInit(): Long —— 初始化引擎，返回框架句柄
        ExpectedNative("engineInit", listOf(), java.lang.Long.TYPE),
        // engineLoadModel(moc3, model3.json, cdi3): Long —— 加载模型，返回模型句柄
        ExpectedNative(
            "engineLoadModel",
            listOf(String::class.java, String::class.java, String::class.java),
            java.lang.Long.TYPE,
        ),
        // engineUpdate(dt: Float) —— 更新动画（秒）; 返回 Unit
        ExpectedNative("engineUpdate", listOf(java.lang.Float.TYPE), java.lang.Void.TYPE),
        // engineDraw() —— 渲染一帧; 返回 Unit
        ExpectedNative("engineDraw", listOf(), java.lang.Void.TYPE),
        // engineSetLipSync(value: Float) —— 嘴型参数（0 闭嘴 / 1 全开）; 返回 Unit
        ExpectedNative("engineSetLipSync", listOf(java.lang.Float.TYPE), java.lang.Void.TYPE),
        // engineSetMvpMatrix(matrix: FloatArray) —— MvpMatrix float[16]; 返回 Unit
        ExpectedNative("engineSetMvpMatrix", listOf(FloatArray::class.java), java.lang.Void.TYPE),
        // engineSetParameters(params: Map<String,Float>) —— 参数帧批量写（fade 后置面）; 返回 Unit
        ExpectedNative("engineSetParameters", listOf(Map::class.java), java.lang.Void.TYPE),
        // engineRelease(handle: Long) —— 释放模型; 返回 Unit
        ExpectedNative("engineRelease", listOf(java.lang.Long.TYPE), java.lang.Void.TYPE),
    )

    @Test
    fun red_live2dnative_declares_all_engine_native_methods() {
        val declared: Set<String> = targetClass.declaredMethods.map { it.name }.toSet()

        for (expected in expectedNatives) {
            if (expected.name !in declared) {
                fail(
                    "RED [AND-W1-M1-A1]: Live2DNative 未声明 engine native 方法 " +
                        "'${expected.name}'(签名契约草案 ${expected.paramTypes.map { it.simpleName }})。\n" +
                        "契约锚: contract §2.5 engine ${expected.name}。\n" +
                        "当前 Live2DNative 的声明方法集 = ${declared.sorted()}.\n" +
                        "coder 须按 wave1-m1-red.md §3 签名映射表声明全部 engine* 系列 native 方法。"
                )
                return
            }
            // 精确定位方法并断言为 native（不存在时 NoSuchMethodException → RED）
            val method: Method = targetClass.getDeclaredMethod(
                expected.name, *expected.paramTypes.toTypedArray(),
            )
            assertTrue(
                "RED [AND-W1-M1-A1]: '${expected.name}' 必须是 native 方法（当前非 native）。",
                Modifier.isNative(method.modifiers),
            )
        }
    }

    @Test
    fun red_engineMethod_signature_matches_draft_mapping() {
        for (expected in expectedNatives) {
            // 用 getDeclaredMethod 精确匹配参数类型，锁定签名（防止仅同名但签名漂移）
            val method: Method = try {
                targetClass.getDeclaredMethod(expected.name, *expected.paramTypes.toTypedArray())
            } catch (e: NoSuchMethodException) {
                fail(
                    "RED [AND-W1-M1-A2]: '${expected.name}' 签名与契约草案映射不符。\n" +
                        "期望参数类型 = ${expected.paramTypes.map { it.simpleName }}。\n" +
                        "契约草图: §2.5 下表（见 wave1-m1-red.md §3）。当前未切换 → 反射查无此签名 = RED。"
                )
                return
            }
            assertEquals(
                "RED [AND-W1-M1-A2]: '${expected.name}' 返回类型应与契约草案映射一致。",
                expected.returnType,
                method.returnType,
            )
        }
    }

    @Test
    fun red_all_engine_methods_are_implicit_public_static_native_injni() {
        // JNI 符号 Java_com_live2dai_android_Live2DNative_<name> 要求方法为 static + public。
        // 断言 static 修饰符，保证导出的 JNI 符号名正确（nm_check.sh 反向印证）。
        for (expected in expectedNatives) {
            val method: Method = try {
                targetClass.getDeclaredMethod(expected.name, *expected.paramTypes.toTypedArray())
            } catch (e: NoSuchMethodException) {
                continue // 由 red_engineMethod_signature 报告
            }
            assertTrue(
                "RED [AND-W1-M1-A3]: '${expected.name}' 必须是 static（JNI 导出符号前缀 Java_ 的前提）。",
                Modifier.isStatic(method.modifiers),
            )
        }
    }
}
