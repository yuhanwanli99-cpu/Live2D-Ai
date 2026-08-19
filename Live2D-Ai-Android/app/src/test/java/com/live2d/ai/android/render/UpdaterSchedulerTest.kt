package com.live2d.ai.android.render

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 3 · 3-1 UpdateScheduler 编排测试（JVM，纯函数 seam `UpdateScheduler`）。
 *
 * 依据: architecture-refactor-render-0814.md F-12（官方 CubismUpdateScheduler + ICubismUpdater + 7 updater
 *       「更新调度 = 官方 5.3 更新架构」）+ schedule-refactor-0814.md Wave3-1「UpdateScheduler 编排；
 *       注册顺序/每帧调用序（官方 Scheduler 语义: 按注册序 update）；动态注册/注销」。
 * 契约锚: 架构 F-12（官方 Scheduler 按注册序 update）；schedule Wave3-1 行；ANIM1-A1（物理开关回退 D2 =
 *       动态注销 Physics updater 的结构免疫面）。
 *
 * 生产 seam 对齐（主代理下发）：
 *   - `SchedulerUpdater{ id: String; onUpdate(dt: Float) }`（官方 ICubismUpdater 的 JVM 形态）；
 *   - `UpdateScheduler.register(updater)` 单注册（防同 id 双注册，幂等忽略保留首实例）、
 *     `unregister(updaterId: String)`（未注册 id 为 no-op）、`frame(dt)`（注册序快照迭代）、
 *     `activeChannels: List<String>`（已注册 id，注册序）。
 *
 * 判定方式: `UpdateScheduler` seam（JVM 可测）：
 *             (a) 注册序 == activeChannels 序；每帧 update 按当前注册序恰一次；
 *             (b) 每帧调用序稳定（同一注册序下多次 frame 序不变，官方 Scheduler 语义）；
 *             (c) 动态注销（如物理 D2 回退移除 Physics）后序重排、被移除通道不再被调用；
 *             (d) 动态注册追加到末尾（不破坏已有序）；
 *             (e) 未注册通道不被调用；dt 正确透传每个 updater。
 *          注: 不锁具体通道次序（官方内部初始序未在契约明列，防猜）——只锁「序稳定 + 同序可重放 + 动态增删后
 *          前序保持」，序条目由 UpdaterRegistry.scan 扫描结果确定（上波 T1 已锁激活矩阵）。
 */
class UpdaterSchedulerTest {

    /** 记录 id + dt 的 fake updater（生产 seam：id 为唯一标识，onUpdate 每帧驱动）。 */
    private class FakeUpdater(val channel: UpdaterChannel) : SchedulerUpdater {
        override val id: String = channel.name
        val calls = mutableListOf<Float>()
        override fun onUpdate(dt: Float) { calls.add(dt) }
    }

    private fun fake(channels: List<UpdaterChannel>): List<FakeUpdater> = channels.map { FakeUpdater(it) }

    private fun UpdateScheduler.registerAll(updaters: List<SchedulerUpdater>) =
        updaters.forEach { register(it) }

    @Test
    fun registerPreservesCallOrder_perFrame() {
        val updaters = fake(listOf(UpdaterChannel.EYE_BLINK, UpdaterChannel.PHYSICS, UpdaterChannel.LIP_SYNC))
        val scheduler = UpdateScheduler()
        scheduler.registerAll(updaters)

        // activeChannels 序 == 注册序（id 列表）
        assertEquals(
            updaters.map { it.id },
            scheduler.activeChannels,
        )

        scheduler.frame(dt = 0.016f)
        // 每帧每个 updater 恰被调用一次，dt 正确透传
        for (u in updaters) {
            assertEquals("每帧每个 updater 恰被调 1 次", 1, u.calls.size)
            assertEquals("dt 正确透传", 0.016f, u.calls.single(), 1e-6f)
        }
    }

    @Test
    fun frameInvokesEachActiveOnce_inRegistrationOrderAcrossFrames() {
        val updaters = fake(listOf(UpdaterChannel.EYE_BLINK, UpdaterChannel.LIP_SYNC))
        val scheduler = UpdateScheduler()
        scheduler.registerAll(updaters)

        scheduler.frame(0.016f)
        scheduler.frame(0.03f)

        // 每帧各 1 次；第二帧 dt=0.03 透传
        assertEquals(listOf(0.016f, 0.03f), updaters[0].calls)
        assertEquals(listOf(0.016f, 0.03f), updaters[1].calls)
    }

    @Test
    fun unregisterRemovesChannel_andNotInvokedAnymore() {
        val eye = FakeUpdater(UpdaterChannel.EYE_BLINK)
        val phys = FakeUpdater(UpdaterChannel.PHYSICS)
        val scheduler = UpdateScheduler()
        scheduler.registerAll(listOf(eye, phys))

        scheduler.frame(0.01f)
        // 物理开关回退 D2（ANIM1-A1）→ 注销 Physics（按 id）
        scheduler.unregister(phys.id)

        assertEquals(
            "注销后仅 EYE_BLINK 保留",
            listOf(eye.id),
            scheduler.activeChannels,
        )
        scheduler.frame(0.02f)

        assertEquals("被注销通道不再被调用", listOf(0.01f), phys.calls)
        assertEquals("剩余通道继续按序调用", listOf(0.01f, 0.02f), eye.calls)
    }

    @Test
    fun dynamicRegister_appendsAfterExistingOrder() {
        val blink = FakeUpdater(UpdaterChannel.EYE_BLINK)
        val lipSync = FakeUpdater(UpdaterChannel.LIP_SYNC)
        val scheduler = UpdateScheduler()
        scheduler.register(blink)
        scheduler.frame(0.01f)

        // 运行中动态追加 LipSync
        scheduler.register(lipSync)
        assertEquals(
            "追加到已有序之后（不破坏前序）",
            listOf(blink.id, lipSync.id),
            scheduler.activeChannels,
        )

        scheduler.frame(0.02f)
        assertEquals(listOf(0.01f, 0.02f), blink.calls)
        assertEquals(listOf(0.02f), lipSync.calls)
    }

    @Test
    fun unregisterUnknownChannel_isSafeNoOp() {
        val blink = FakeUpdater(UpdaterChannel.EYE_BLINK)
        val scheduler = UpdateScheduler()
        scheduler.register(blink)
        // 注销未注册 id：幂等安全（防崩溃，结构免疫）
        scheduler.unregister(UpdaterChannel.POSE.name)
        scheduler.unregister(blink.id)
        scheduler.unregister(blink.id)   // 重复注销
        assertTrue("注销后 activeChannels 应为空且不崩溃", scheduler.activeChannels.isEmpty())
        scheduler.frame(0.01f)  // 空调度帧安全
        assertEquals("空调度后无 updater 被调用", 0, blink.calls.size)
    }

    @Test
    fun registerTwiceSameId_doesNotDoubleInvoke() {
        // 同一 id 重复注册 → 只保留一次（防「同 updater 双注册」（freeze A1 hook 去重的结构免疫））
        val blink = FakeUpdater(UpdaterChannel.EYE_BLINK)
        val blinkDup = FakeUpdater(UpdaterChannel.EYE_BLINK)
        val scheduler = UpdateScheduler()
        scheduler.register(blink)
        scheduler.register(blinkDup)

        // 同 id 只注册一次：activeChannels 无重复 id
        assertEquals("同一 id 不得重复注册", listOf(blink.id), scheduler.activeChannels)

        scheduler.frame(0.01f)
        val totalInvocations = blink.calls.size + blinkDup.calls.size
        assertEquals("同 id 每帧仅调用一次（防双注册）", 1, totalInvocations)
    }

    @Test
    fun frame_withNoRegistration_isSafe() {
        val scheduler = UpdateScheduler()
        assertTrue("未注册任何 updater 时 activeChannels 应为空", scheduler.activeChannels.isEmpty())
        scheduler.frame(0.016f)  // 空调度不崩溃
        // 无断言失败即 PASS（fail-safe，I7 精神）
    }
}
