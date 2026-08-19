package com.live2d.ai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * MotionTags 决策逻辑纯类测试（M7：MotionAliasTable / KeywordMotionTable / IdleMotionSelector）。
 * 对齐蓝本：LipSyncMathTest.kt（纯函数断言，无 Android/GL 依赖）。
 */
class MotionTagsTest {

    // ══════════════════════════════════════════════
    // MotionAliasTable（M7 §2.2 + M1 §5.3）
    // ══════════════════════════════════════════════

    @Test
    fun A1_knownAlias() {
        // R1-model M3：bai 无 motion → motionAliases={} 空集，任何别名均无映射
        assertNull(MotionAliasTable.resolve("wave"))
        assertNull(MotionAliasTable.resolve("idle"))
        assertNull(MotionAliasTable.resolve("special_02"))
    }

    @Test
    fun A2_caseInsensitive_trim() {
        // R1-model M3：bai 空集 → 大小写/空白规范化后仍无映射
        assertNull(MotionAliasTable.resolve("WAVE"))
        assertNull(MotionAliasTable.resolve(" Idle "))
        assertNull(MotionAliasTable.resolve("IDLE"))
    }

    @Test
    fun A3_unknownAlias() {
        assertNull(MotionAliasTable.resolve("dance"))
    }

    @Test
    fun A4_blank() {
        assertNull(MotionAliasTable.resolve(""))
        assertNull(MotionAliasTable.resolve("   "))
    }

    @Test
    fun A5_tableIntegrity() {
        // R1-model M3：bai.adapter.json 无 Motions ↔ motionAliases 空集（C25/C29 双空一致）
        assertTrue("motionAliases 应为空集", MotionAliasTable.TAG_TO_KEY.isEmpty())
        assertTrue("motionAliases 值集应为空", MotionAliasTable.TAG_TO_KEY.values.isEmpty())
    }

    // ══════════════════════════════════════════════
    // KeywordMotionTable（M7 §2.4 + §4.2）
    // ══════════════════════════════════════════════

    @Test
    fun B1_singleHit() {
        val hits = KeywordMotionTable.find("主人再见！")
        assertEquals(1, hits.size)
        assertEquals(2, hits[0].position)
        assertEquals("special_02", hits[0].alias)
    }

    @Test
    fun B2_multipleSorted() {
        val hits = KeywordMotionTable.find("挥手告别，拜拜")
        assertTrue(hits.size >= 1)
        // 按 position 升序
        for (i in 1 until hits.size) {
            assertTrue("position 应升序", hits[i].position >= hits[i - 1].position)
        }
    }

    @Test
    fun B3_sameAliasDedup() {
        val hits = KeywordMotionTable.find("拜拜！再见！")
        // 同别名 special_02：只保留最早命中
        val special02Hits = hits.filter { it.alias == "special_02" }
        assertEquals("同别名仅最早命中", 1, special02Hits.size)
        assertEquals(0, special02Hits[0].position)
    }

    @Test
    fun B4_noHit() {
        val hits = KeywordMotionTable.find("今天天气不错")
        assertTrue(hits.isEmpty())
    }

    @Test
    fun B5_blankEmpty() {
        assertTrue(KeywordMotionTable.find("").isEmpty())
        assertTrue(KeywordMotionTable.find("   ").isEmpty())
    }

    // ══════════════════════════════════════════════
    // IdleMotionSelector（M7 §2.3）
    // ══════════════════════════════════════════════

    @Test
    fun C1_emptyPool() {
        val selector = IdleMotionSelector(emptyList())
        for (i in 1..5) {
            assertNull("池空 → 恒 null", selector.next())
        }
    }

    @Test
    fun C2_noRepeatWindow() {
        val pool = listOf("_0", "_1", "_2", "_3", "_4", "_5")
        val selector = IdleMotionSelector(pool, noRepeat = 3)
        val recent = mutableListOf<String>()
        for (i in 1..6) {
            val next = selector.next()
            assertNotNull(next)
            // 不在最近 3 个历史内
            if (recent.size >= 3) {
                assertTrue("$next 不应在最近 3 个历史 $recent 中", next !in recent.takeLast(3))
            }
            recent.add(next!!)
        }
        // 6 次内不重复（6 键 pool，3 窗口保证每键至少一次不同）
        assertEquals(6, recent.size)
    }

    @Test
    fun C3_shuffleBag_reset() {
        val pool = listOf("_0", "_1", "_2", "_3", "_4", "_5")
        val selector = IdleMotionSelector(pool, noRepeat = 3)
        val results = mutableListOf<String?>()
        for (i in 1..10) {
            val next = selector.next()
            results.add(next)
            assertNotNull("取 $i 次不应死锁", next)
        }
        // 10 次全无异常即通过
        assertEquals(10, results.size)
    }

    @Test
    fun C4_defaultPool() {
        // R1-model M3 §2.3：bai 无 motion → idle 池默认空（FALLBACK_POOL=[]）
        assertTrue("默认 idle 池应为空", IdleMotionSelector.DEFAULT_POOL.isEmpty())
    }
}
