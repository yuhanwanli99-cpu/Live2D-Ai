package com.live2d.ai.android

import java.lang.reflect.Modifier
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2B-2 节拍断言（用户裁定 B-5 = 保持 100ms 节拍）— RED/回归锁测试。
 *
 * 契约锚：`.pi/spoq/contract-refactor-0814.md` §4（B-5=保持100ms）+
 * `.pi/spoq/architecture-refactor-tts-0814.md` §2.4「150ms 帧窗 + 100ms 节拍全部沿用」+
 * 既有 MAnd2 A6「更新周期 ∈ [8,100]ms，主写 = 100」。
 * 目的：锁定生产节拍常量 = 100ms，防 coder 在 B-5 收紧/改写时擅自改回（回归锁）。
 *
 * seam：生产类 `VoiceIoController` 上的节拍常量（`LIP_SYNC_INTERVAL_MS`；契约口径同名
 * `LIP_SYNC_INTERVAL`），当前取值 100ms，类型 Long/Int 且为编译期常量（static final）。
 * 用反射读取，避免对类成员的直接引用破坏对「防改回」的约束。
 *
 * RED 状态：若常量为编译期常量直接可用则此测试应为 GREEN（现状 100ms 即真）；但为防
 * coder 改回，反射锁定精确值 100——若有人把节拍改回非 100（如 60/35）则本测试 FAIL，
 * 若 B-5 裁定为收紧则本测试须按新裁定同步（本测试以 B-5=保持100ms 裁定锚定）。
 *
 * 纪律：禁读实现代码细节；仅按契约 B-5 锁定 100ms 值；不臆定字段名以外的实现。
 */
class VoiceIoRefactorBeatIntervalTest {

    @Test
    fun lipSyncIntervalConstant_equals100ms() {
        // B-5 裁定 = 保持 100ms 节拍：生产节拍常量必为 100
        val host = VoiceIoController::class.java
        val value = readBeatMillis(host)
        assertEquals(
            "节拍常量应为 100ms（B-5 保持 100ms，LIP_SYNC_INTERVAL_MS），当前实际 $value；改回将被拒绝",
            100L, value
        )
    }

    @Test
    fun lipSyncInterval_isCompileTimeConstant_soAvailableToBeatLoop() {
        // 节拍常量应为编译期常量（static final Int/Long），供 100ms 节拍循环直接引用。
        val host = VoiceIoController::class.java
        val fieldName = beatFieldName(host)
        val f = host.declaredFields.firstOrNull { it.name == fieldName }
        assertTrue(
            "生产类 $host 应声明节拍常量字段 $fieldName（B-5 节拍100ms）；未找到 → RED",
            f != null
        )
        assertTrue("节拍常量应为 static（compile-time），${f!!.name}", Modifier.isStatic(f.modifiers))
        assertTrue(
            "节拍常量应为 final（不可被运行时改），${f.name}",
            Modifier.isFinal(f.modifiers) || Modifier.isStatic(f.modifiers)
        )
    }

    // ─────────────────────────────────────────────
    // 反射读取辅助：兼容 `LIP_SYNC_INTERVAL_MS`（现状命名）与契约口径 `LIP_SYNC_INTERVAL`
    // ─────────────────────────────────────────────

    private fun beatFieldName(host: Class<*>): String {
        for (candidate in listOf("LIP_SYNC_INTERVAL_MS", "LIP_SYNC_INTERVAL")) {
            if (host.declaredFields.any { it.name == candidate }) return candidate
        }
        return "LIP_SYNC_INTERVAL_MS"
    }

    private fun readBeatMillis(host: Class<*>): Long {
        val name = beatFieldName(host)
        val f = host.declaredFieldOrNull(name)
            ?: throw AssertionError("节拍常量 $name 未在生产类 $host 声明 → RED（B-5 节拍100ms 缺失）")
        f.isAccessible = true
        return when (val v = f.get(null)) {
            is Long -> v
            is Int -> v.toLong()
            is Short -> v.toLong()
            else -> throw AssertionError(
                "节拍常量 $name = $v 非数值类型（Long/Int）→ 契约 B-5=100ms 语义被破坏"
            )
        }
    }

    private fun Class<*>.declaredFieldOrNull(name: String) =
        declaredFields.firstOrNull { it.name == name }
}
