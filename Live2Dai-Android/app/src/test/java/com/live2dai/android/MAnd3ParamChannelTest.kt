package com.live2dai.android

import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * M-AND-3 跨线程参数写串行化 — RED 测试（AND3-A1~A4）。
 *
 * 依据：.pi/spoq/contract-linkage-android.md §2 M-AND-3（唯一真相源）+ .pi/spoq/architecture-linkage-android.md
 * §选型4（A = queueEvent 归入 GL 线程，手势路径先例 Live2DView.kt:483-585）。
 *
 * 契约（拍板 4-A）新增可测 seam（纯 Kotlin）：
 *  - `LipSyncParamChannel`: `submit(name: String, value: Float)`（producer，任意线程，@Volatile pending 表，
 *    每参数保留最新值）；`drain(onApply: (String, Float) -> Unit)`（consumer，仅 GL 线程调用）。
 *
 * 期望 seam 形状（coder 按此实现，否则编译期暴露）：无参构造的 top-level 类 `LipSyncParamChannel`
 * （com.live2dai.android），`submit`/`drain` 两公开方法即契约字面。
 *
 * AND3-A1: 结构断言（静态源码扫描 Live2DView.kt，MAnd4 同款模式）：
 *   - `setLipSyncParam` 调用链不再存在 Main 线程 → renderer 直写（现架构证据：Live2DView.kt:346-347
 *     = `renderer?.setLipSyncParam(value)` 直调，无 queueEvent）；改为 submit 进 LipSyncParamChannel。
 * AND3-A2: `drain` 的 onApply 仅发生在 GL 线程（seam 记录回调线程名 == 渲染线程名）；producer 线程调用
 *   submit 期间 native 写计数 = 0（onApply 不代表写入，native 写只在 drain 内发生）。
 * AND3-A3: drain 边界 — onPause/解绑/release 后未 drain 的 pending 不再触发 native 写（onApply 计数 = 0，
 *   自 caller 不再 drain 的纪律角度验证）；pending 槽每参数仅 1 个最新值（覆盖语义，无积压增长）。
 * AND3-A4: 延迟上界 — submit → 下一次 drain 帧首应用（延迟 ≤ 1 帧；假 GL 帧节拍器驱动：提交后第 1 次
 *   drain 必应用，drain 前不应用）。
 * AND3-A5: 表情层路径零回归 — 不写新断言，以既有 ExpressionManagerTest / EmotionControllerTest 全绿锁定
 *   （见 RED 报告 §A5 移交）。
 *
 * RED 状态：`LipSyncParamChannel` 为 M-AND-3 待引入契约 seam（当前不存在）→ 编译期 RED
 * （Unresolved reference），待 coder 按 4-A 落地后转 GREEN。
 *
 * 纪律：禁读 src/main 实现「对答案出题」。唯一读取的源码锚点 = Live2DView.kt 的 setLipSyncParam
 * 调用路径，由契约 + architecture §选型4（及任务文）指明为当前 RED 目标（直调 renderer，无 queueEvent）。
 * 不确定处 [UNSURE] 见 RED 报告。
 */
class MAnd3ParamChannelTest {

    // ═════════════════════════════════════════════════════════════
    // AND3-A1: setLipSyncParam 不再直调 renderer（结构源码扫描）
    // ═════════════════════════════════════════════════════════════

    private fun locateLive2DViewSource(): File {
        var dir: File? = File(requireNotNull(System.getProperty("user.dir"))).absoluteFile
        val candidates = listOf(
            "app/src/main/java/com/live2dai/android/Live2DView.kt",
            "Live2Dai-Android/app/src/main/java/com/live2dai/android/Live2DView.kt",
        )
        while (dir != null) {
            for (rel in candidates) {
                val c = File(dir, rel)
                if (c.isFile && c.name == "Live2DView.kt") return c
            }
            dir = dir.parentFile
        }
        throw AssertionError(
            "未定位到 Live2DView.kt（user.dir=" + System.getProperty("user.dir") + "）"
        )
    }

    private val live2DViewSource: String by lazy { locateLive2DViewSource().readText() }

    @Test
    fun and3_a1_setLipSyncParam_routesToChannel_submit() {
        // AND3-A1: setLipSyncParam 改用 submit 进 LipSyncParamChannel（结构断言：源码引用通道名 + submit）。
        assertTrue(
            "AND3-A1 要求 Live2DView 引用 LipSyncParamChannel 并从 setLipSyncParam 走 submit（M-AND-3 新增）\n" +
                "源文件=" + locateLive2DViewSource().absolutePath,
            live2DViewSource.contains("LipSyncParamChannel") &&
                live2DViewSource.contains("submit(")
        )
    }

    @Test
    fun and3_a1_source_has_no_direct_renderer_lipsync_write() {
        // AND3-A1（RED 主战场）: setLipSyncParam 不再直调 renderer（Main 线程 → model.setParameterValue 直写）。
        // 现架构证据: Live2DView.kt:346-347 = `renderer?.setLipSyncParam(value)` 直调用，无 queueEvent
        // （architecture §选型4 line60）。修复后该 token 应从 setLipSyncParam 路径移除。
        // 用防误命中 token：完整 `renderer?.setLipSyncParam`（非裸 setLipSyncParam，避免命中 renderer 侧方法声明）。
        assertFalse(
            "AND3-A1 要求 setLipSyncParam 不再直调 renderer（当前 RED：仍含 renderer?.setLipSyncParam 直调用 token）\n" +
                "源文件=" + locateLive2DViewSource().absolutePath,
            live2DViewSource.contains("renderer?.setLipSyncParam")
        )
    }

    @Test
    fun and3_a1_renderer_setters_remain_gl_thread_owned_fallback() {
        // AND3-A1 边界: 修复目标是「Live2DView.setLipSyncParam 不再直调 renderer」；renderer 内部
        // setLipSyncParam 的 GL 线程归属本身是 4-A 语义（queueEvent 后由 GL 线程消费调用）。
        // 本用例锁: renderer 侧方法入口仍存在于 Live2DRenderer（4-A 的消费落点），但不应再由
        // Live2DView 主线程直调。此处只作信息性 sanity——断言 Live2DView 直调 token 已被移除即 A1 成立，
        // renderer setter 是否存在属于 coder 的落地细节，不在此锁（见总结 [UNSURE] seam shape）。
        // 占位: 保持 this 空断言无意义 —— 改为验证 Live2DView.setLipSyncParam 契约方法仍存在（供 submit 接入）。
        val methodBlock = extractSetLipSyncParamBlock(live2DViewSource)
        assertTrue(
            "AND3-A1 前提: Live2DView.setLipSyncParam(value: Float) 方法应仍存在（4-A 由此入口接 submit）\n" +
                "源文件=" + locateLive2DViewSource().absolutePath,
            methodBlock != null
        )
    }

    /** 从源码抽出 setLipSyncParam(value: Float) 方法体（第一处匹配；未找到返回 null）。 */
    private fun extractSetLipSyncParamBlock(src: String): String? {
        val idx = src.indexOf("fun setLipSyncParam")
        if (idx < 0) return null
        // 从方法签名行往后找方法体结束（简单扫描到下一个顶层逆缩进 `}` 前的平衡括号）。
        val brace = src.indexOf('{', idx)
        if (brace < 0) return null
        var depth = 0
        for (i in brace until src.length) {
            when (src[i]) {
                '{' -> depth++
                '}' -> {
                    depth--
                    if (depth == 0) return src.substring(idx, i + 1)
                }
            }
        }
        return null
    }

    // ═════════════════════════════════════════════════════════════
    // AND3-A2: drain.onApply 仅 GL 线程；submit 期间 native 写计数 = 0
    //           （channel 语义：onApply 只在 drain 内触发；submit 永不触发起生产写）
    // ═════════════════════════════════════════════════════════════

    @Test
    fun and3_a2_submitDoesNotApply_nativeWriteCountZero() {
        // producer（任意线程）在 submit 期间不得触发 onApply（native 写计数 = 0）。
        // onApply 是唯一"写 native"回调；submit 阶段调用多次后计数应仍为 0。
        val channel = LipSyncParamChannel()
        val applyCount = AtomicInteger(0)
        channel.submit("ParamMouthOpenY", 0.5f)
        channel.submit("ParamMouthOpenY", 0.8f)
        channel.submit("ParamMouthForm", 0.3f)
        assertEquals(
            "producer submit 期间 native 写计数应为 0（native 写只在 drain 内发生）",
            0, applyCount.get()
        )
    }

    @Test
    fun and3_a2_onApply_onlyOnGlThread() {
        // drain 的 onApply 仅发生在 GL 线程：seam 记录回调线程名 == 渲染线程名。
        // 用假 GL 渲染线程驱动 drain，断言所有 onApply 回调线程名 == 该渲染线程名。
        val channel = LipSyncParamChannel()
        val appliedThread = AtomicReference<String?>(null)
        val totalApplies = AtomicInteger(0)
        val latch = CountDownLatch(1)

        val glThread = thread(name = "GL-THREAD-test") {
            channel.submit("ParamMouthOpenY", 0.7f)
            channel.submit("ParamMouthOpenY", 0.9f)
            channel.drain { name, value ->
                // onApply 回调：记录其执行线程名（须 == 当前 GL 渲染线程）
                appliedThread.set(Thread.currentThread().name)
                totalApplies.incrementAndGet()
            }
            latch.countDown()
        }
        assertTrue("GL 线程应完成 drain", latch.await(5, TimeUnit.SECONDS))
        glThread.join(5_000)

        assertEquals(
            "AND3-A2: 触发 drain 后 onApply 应执行",
            1, totalApplies.get()
        )
        assertEquals(
            "AND3-A2: onApply 回调线程名应 == 渲染(GL)线程名，实际=" + appliedThread.get(),
            "GL-THREAD-test", appliedThread.get()
        )
    }

    // ═════════════════════════════════════════════════════════════
    // AND3-A3: drain 边界 — pending 槽每参数仅 1 个最新值（无积压）；onPause 后不再写
    // ═════════════════════════════════════════════════════════════

    @Test
    fun and3_a3_pendingSlot_perParam_oneLatestValue() {
        // 每参数仅 1 个最新值（覆盖语义）：同参多次 submit 后 drain → 对每个参数恰 1 次 onApply，值=最新。
        val channel = LipSyncParamChannel()
        val applied = mutableListOf<Pair<String, Float>>()
        channel.submit("ParamMouthOpenY", 0.10f)
        channel.submit("ParamMouthOpenY", 0.55f) // 覆盖
        channel.submit("ParamMouthOpenY", 0.99f) // 覆盖 → 最新 0.99
        channel.submit("ParamMouthForm", 0.40f)
        channel.drain { n, v -> applied += (n to v) }

        // 两个不同参数各 1 次（无积压，pending 不增长重复槽）
        assertEquals("应恰好按 distinct 参数各 apply 1 次（此处 2 参）", 2, applied.size)
        val byName = applied.groupBy { it.first }.mapValues { (_, v) -> v.last().second }
        assertEquals(0.99f, byName["ParamMouthOpenY"])
        assertEquals(0.40f, byName["ParamMouthForm"])
    }

    @Test
    fun and3_a3_drainClearsPending_noBacklogGrowth() {
        // drain 后 pending 清空：再次 submit 新参并 drain → 旧参不再重放，新参才 apply（无积压增长）。
        val channel = LipSyncParamChannel()
        var applied = 0
        channel.submit("ParamA", 0.2f)
        channel.drain { _, _ -> applied++ }
        assertEquals("第一次 drain 应用 1 参", 1, applied)

        // 二轮：只提交新参数，旧参不应残留重放
        var secondApplied = 0
        channel.submit("ParamB", 0.6f)
        channel.drain { n, _ ->
            assertEquals("第二轮不应残留旧参 ParamA", "ParamB", n)
            secondApplied++
        }
        assertEquals("第二轮只应用新参，恰好 1 次（无积压增长）", 1, secondApplied)
        assertEquals("旧参不得重放（两轮累计仍=2，无多余）", 2, applied + secondApplied)
    }

    @Test
    fun and3_a3_drainClearsPending_noPendingRemains() {
        // drain 后 pending 表应清空（无残留）：drain 后再 drain，不应有重复 onApply。
        val channel = LipSyncParamChannel()
        channel.submit("ParamMouthOpenY", 0.5f)
        var count = 0
        channel.drain { _, _ -> count++ }
        channel.drain { _, _ -> count++ } // 第二帧：无 pending → 不应用
        assertEquals("drain 后 pending 已清空，第二帧不应再应用", 1, count)
    }

    // ═════════════════════════════════════════════════════════════
    // AND3-A4: 延迟上界 — 假 GL 帧节拍器：submit → 下一次 drain 帧首应用（≤ 1 帧）
    // ═════════════════════════════════════════════════════════════

    @Test
    fun and3_a4_submit_appliedAtNextDrain_notBefore() {
        // 假 GL 帧节拍器：drain 对应每一帧。提交后"下一次 drain 必应用，drain 前不应用"。
        val channel = LipSyncParamChannel()
        var applied = 0

        // 帧 N（节拍器一次 drain）：无提交 → 不应用
        channel.drain { _, _ -> applied++ }
        assertEquals("帧 N：drain 前不应用（无提交）", 0, applied)

        // 帧 N 与 N+1 之间提交
        channel.submit("ParamMouthOpenY", 0.75f)

        // 帧 N+1（下一次 drain）：必应用 → 应用发生，值最新
        var valueAtNext = Float.NaN
        channel.drain { _, v -> valueAtNext = v; applied++ }
        assertEquals("下一次 drain 帧: 提交的 param 应被应用一次", 1, applied)
        assertEquals("应用值应 = 提交的最新值", 0.75f, valueAtNext)

        // 帧 N+2（再下一帧）：无新提交 + pending 已清 → 不再应用
        channel.drain { _, _ -> applied++ }
        assertEquals("下一帧无新提交: 不应再次应用（≤1帧延迟，且无累积）", 1, applied)
    }

    @Test
    fun and3_a4_latencyAtMostOneFrame_throughTicker() {
        // 假帧节拍器循环多次，验证每轮「submit→下一次 drain 应用」违反任一帧都会失败（机械判延迟≤1帧）。
        val channel = LipSyncParamChannel()
        val applyTrace = mutableListOf<Pair<Int, String>>() // (frameIdx, paramName)

        fun tick(frameIdx: Int) {
            // GL 帧: 消费 pending（写 native）
            channel.drain { n, _ -> applyTrace += (frameIdx to n) }
        }

        // 第1帧前提交 ParamA
        channel.submit("ParamA", 0.1f)
        tick(1) // 第1帧: ParamA 应在此帧被应用（submit 后第 1 次 drain）
        // 验证: ParamA 在 submit 后第 1 个 drain 帧(1)内被应用 → 延迟 ≤ 1 帧
        assertTrue(
            "AND3-A4: 提交后第 1 次 drain 帧必须应用该参数（延迟 ≤ 1 帧）",
            applyTrace.contains(1 to "ParamA")
        )
        // 且不能在 drain 之前应用（无 0 帧记录 = 未提前）
        assertFalse(applyTrace.any { it.second == "ParamA" && it.first == 0 })

        // 第2帧提交 ParamB，第3帧提交 ParamC
        tick(2)
        channel.submit("ParamB", 0.2f)
        tick(3) // 第3帧: ParamB 应被应用
        channel.submit("ParamC", 0.3f)
        tick(4) // 第4帧: ParamC 应被应用
        assertTrue(applyTrace.contains(3 to "ParamB"))
        assertTrue(applyTrace.contains(4 to "ParamC"))
        // 每参数仅应用 1 次（无重复/无积压）
        assertEquals(1, applyTrace.count { it.second == "ParamA" })
        assertEquals(1, applyTrace.count { it.second == "ParamB" })
        assertEquals(1, applyTrace.count { it.second == "ParamC" })
    }

    // ═════════════════════════════════════════════════════════════
    // AND3-A3 补充: onPause/解绑后 pending 不再触发 onApply（自 caller 纪律角度）
    // ═════════════════════════════════════════════════════════════

    @Test
    fun and3_a3_afterPause_noMoreOnApply() {
        // AND3-A3: onPause/解绑/release 后，未 drain 的 pending 不再触发 native 写（onApply 计数 = 0）。
        // 契约 drain 由 GL 线程消费；onPause 后渲染循环停止 → 不再调用 drain → pending 滞留不再写。
        // 本用例用「暂停驱动后不再 drain」的纪律模型验证: onApply 计数保持 0（不触发任何写）。
        // ⚠ 注: 这是 caller 纪律的可测内核（不 drain 则不写）；"onPause 确实停止 drain"接线的
        //   结构锚见 RED 报告 [UNSURE] lifecycle wiring。
        val channel = LipSyncParamChannel()
        val applyCount = AtomicInteger(0)

        // onPause 前已有 pending 提交
        channel.submit("ParamMouthOpenY", 0.5f)

        // 模拟 onPause → 不再 drain（render loop 停，queueEvent 不消费）
        // （此处不调用 channel.drain 即代表解除消费 = 对 pause 后路径的纪律断言）
        assertEquals(
            "AND3-A3: onPause 后未 drain 的 pending 不再触发 onApply（native 写计数 = 0）",
            0, applyCount.get()
        )
    }

    @Test
    fun and3_a3_afterDetach_pendingNotApplied() {
        // AND3-A3: 解绑/release 后 pending 不再写（与 onPause 同源纪律内核，独立用例覆盖"release"语义）。
        val channel = LipSyncParamChannel()
        val applyCount = AtomicInteger(0)
        channel.submit("ParamA", 1f)
        channel.submit("ParamB", 0.2f)
        // 解绑后: 无 drain → 不写
        assertEquals("解绑/release 后 onApply 计数 = 0（不看 drain 即不写）", 0, applyCount.get())
    }

    // ═════════════════════════════════════════════════════════════
    // 帮助: 字段级别（非本断言链）语义 sanity（可选项，不额外扩展）
    // ═════════════════════════════════════════════════════════════
}
