package com.live2dai.android.render

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 3 · 3-1 官方 updater 注册矩阵测试（JVM，纯函数 seam `UpdaterRegistry`）。
 *
 * 依据: contract-refactor-0814.md ANIM1-A1（A-4=D1 物理激活 + 开关回退 D2 可用）+ 条件激活 §4 A-4=D1
 *       + architecture-refactor-render-0814.md 5.1 官方系统接线表（通道激活矩阵）+ F-5（bai 资产事实：
 *       无 motion3/exp3/pose3、有 physics3/ParamEyeLOpen/ROpen/ParamMouthOpenY/ParamA、无 ParamBreath）
 *       + 1.D（物理 D1 启用官方 Physics、开关回退 D2）
 *       + schedule-refactor-0814.md Wave3-1「updater 注册矩阵测试」。
 * 契约锚: ANIM1-A1；架构 5.1 表；schedule Wave3-1 行。
 *
 * 生产 seam 对齐（主代理下发）：
 *   - `UpdaterRegistry` 为 object（非类）；`scan(caps, physicsEnabled=true)` 返回
 *     `List<RegisteredUpdater(channel, updaterId)>`——scan 结果即「应注册清单」，
 *     「激活」= 出现在 scan 结果中（无 isActive 字段）。
 *   - `ModelAnimationCapabilities` 7 布尔：hasMotionAssets/hasExpressionAssets/hasPoseAssets/
 *     hasEyeBlinkParams/hasBreathParams/hasPhysicsAssets/hasLipSyncParams。
 *   - 生产激活矩阵（生产代码即裁定）：MOTION/EXPRESSION/POSE 无资产 → 不注册；
 *     EYE_BLINK/BREATH/LIP_SYNC 恒注册（BREATH 恒注册但视觉 no-op，bai 无 ParamBreath）；
 *     PHYSICS 按 physicsEnabled 注册（资产位不参与注册判定，见 PHYSICS 测试内说明）。
 *
 * 判定方式: 机械断言通道激活矩阵（bai 事实）：
 *             (a) MOTION/EXPRESSION/POSE 无资产 → 不在 scan 结果（no-op，不注册）；
 *             (b) EYE_BLINK → 恒注册（激活）；
 *             (c) BREATH → 恒注册（注册但视觉 no-op，bai 无 ParamBreath）；
 *             (d) PHYSICS 按 physicsEnabled：true 注册 / false 不注册（A-4 D1/D2）；
 *             (e) LIP_SYNC → 恒注册（激活）；
 *             (f) 全能力模型 → 7 通道全覆盖不遗漏（注册序 = UpdaterChannel 声明序）。
 *           便捷入口 scan(caps) 默认物理开启（D1）。
 */
class UpdaterRegistryTest {

    // bai 唯一模型能力事实（架构 F-5 逐项输入，对齐生产 ModelAnimationCapabilities 字段）
    private val bai = ModelAnimationCapabilities(
        hasMotionAssets = false,
        hasExpressionAssets = false,
        hasPoseAssets = false,
        hasPhysicsAssets = true,
        hasEyeBlinkParams = true,
        hasBreathParams = false,
        hasLipSyncParams = true,
    )

    private fun activeChannels(caps: ModelAnimationCapabilities, physicsEnabled: Boolean): Set<UpdaterChannel> =
        UpdaterRegistry.scan(caps, physicsEnabled).map { it.channel }.toSet()

    @Test
    fun scan_bai_physicsOn_channelsMatchActivationMatrix() {
        // bai 事实 + A-4=D1（物理开）→ 注册通道集合（EYE_BLINK/BREATH/LIP_SYNC 恒注册 + PHYSICS 按开关）
        val active = activeChannels(bai, physicsEnabled = true)
        assertEquals(
            "bai 在 D1 下眨眼+呼吸(no-op)+物理+嘴型注册（motion/expression/pose 无资产不注册）",
            setOf(
                UpdaterChannel.EYE_BLINK,
                UpdaterChannel.BREATH,
                UpdaterChannel.PHYSICS,
                UpdaterChannel.LIP_SYNC,
            ),
            active,
        )
    }

    @Test
    fun scan_allCapabilities_coversAllSevenChannels() {
        // 扫描矩阵覆盖全部 7 个官方通道（F-12: CubismUserModel 聚合 7 个更新面），全能力模型不遗漏
        val allCaps = ModelAnimationCapabilities(
            hasMotionAssets = true,
            hasExpressionAssets = true,
            hasPoseAssets = true,
            hasPhysicsAssets = true,
            hasEyeBlinkParams = true,
            hasBreathParams = true,
            hasLipSyncParams = true,
        )
        val channels = UpdaterRegistry.scan(allCaps, physicsEnabled = true).map { it.channel }
        assertEquals(
            "全能力模型 scan 必须覆盖全部 7 个官方 updater 通道（注册序 = 通道声明序）",
            listOf(
                UpdaterChannel.MOTION,
                UpdaterChannel.EXPRESSION,
                UpdaterChannel.EYE_BLINK,
                UpdaterChannel.BREATH,
                UpdaterChannel.PHYSICS,
                UpdaterChannel.POSE,
                UpdaterChannel.LIP_SYNC,
            ),
            channels,
        )
    }

    @Test
    fun noAssetChannels_areNotRegistered() {
        // bai 无 motion3/exp3/pose3 → no-op（F-13: 官方 MotionManager/ExpressionManager 空集合 no-op）
        // 生产 seam 口径：「不激活」= 不在 scan 结果中（scan 结果即应注册清单）
        val channels = UpdaterRegistry.scan(bai).map { it.channel }.toSet()
        assertTrue("MOTION 无资产应不注册 (no-op)", UpdaterChannel.MOTION !in channels)
        assertTrue("EXPRESSION 无 exp3 应不注册 (no-op)", UpdaterChannel.EXPRESSION !in channels)
        assertTrue("POSE 无 pose3 应不注册 (no-op)", UpdaterChannel.POSE !in channels)
    }

    @Test
    fun eyeBlinkChannel_isRegistered() {
        // 有 ParamEyeLOpen/ROpen → 眨眼激活（架构 5.1「眨眼激活」）
        val scan = UpdaterRegistry.scan(bai)
        assertTrue(
            "bai 有眨眼参数 → EYE_BLINK 应注册",
            scan.single { it.channel == UpdaterChannel.EYE_BLINK }.updaterId.isNotBlank(),
        )
    }

    @Test
    fun breathChannel_isAlwaysRegistered_visualNoOpForBai() {
        // 生产裁定：BREATH 恒注册（bai 无 ParamBreath → cdi 无参数驱动 = 视觉 no-op）。
        // 注册形态按生产代码锁定：恒注册（与眨眼/嘴型同口径），视觉 no-op 由资产事实保证。
        val scan = UpdaterRegistry.scan(bai)
        assertTrue(
            "bai 无 ParamBreath → BREATH 仍恒注册（视觉 no-op 由 cdi 无参数驱动保证，F-5）",
            scan.any { it.channel == UpdaterChannel.BREATH },
        )
    }

    @Test
    fun physicsChannel_activeWithD1_inactiveWhenSwitchedToD2() {
        // A-4=D1：物理激活；D2 开关关闭 → 物理不注册（ANIM1-A1「开关回退 D2 可用」）
        val withPhysics = activeChannels(bai, physicsEnabled = true)
        assertTrue("D1 物理开启 → PHYSICS 应注册", UpdaterChannel.PHYSICS in withPhysics)

        val physicsOff = activeChannels(bai, physicsEnabled = false)
        assertTrue("D2 开关关闭 → PHYSICS 应移除/inactive", UpdaterChannel.PHYSICS !in physicsOff)
        // 其余恒注册通道不因物理开关受影响（仅物理通道切换）
        assertEquals(
            "D2 下眨眼+呼吸+嘴型保留（物理被移除）",
            setOf(UpdaterChannel.EYE_BLINK, UpdaterChannel.BREATH, UpdaterChannel.LIP_SYNC),
            physicsOff,
        )
    }

    @Test
    fun scanConvenience_defaultsToD1() {
        // scan(caps) 便捷入口默认物理开启（A-4=D1 交付默认）
        val active = UpdaterRegistry.scan(bai, physicsEnabled = true).map { it.channel }
        val activeConvenience = UpdaterRegistry.scan(bai).map { it.channel }
        assertEquals("scan(caps) 便捷入口应等价于 scan(caps, physicsEnabled=true)（默认 D1）", active, activeConvenience)
    }

    @Test
    fun lipSyncChannel_alwaysRegistered_whenMouthParamsExist() {
        // 有 ParamMouthOpenY/ParamA → 嘴型恒注册（架构 5.1「嘴型激活」；§2.3 硬约束）
        val active = activeChannels(bai, physicsEnabled = true)
        assertTrue("bai 有嘴型参数 → LIP_SYNC 应恒注册", UpdaterChannel.LIP_SYNC in active)
    }

    @Test
    fun physicsRegistration_gatedByPhysicsEnabledFlagOnly() {
        // 生产 seam 裁定：PHYSICS 注册判定仅按 physicsEnabled 开关（资产位不参与注册判定；
        // 资产事实（F-5 盘点）由资产层保证，scan 层不放重复闸）。本测试锁定该生产口径。
        val noPhysicsAsset = bai.copy(hasPhysicsAssets = false)
        val active = activeChannels(noPhysicsAsset, physicsEnabled = true)
        assertTrue("生产口径：physicsEnabled=true → PHYSICS 注册（资产位不参与注册判定）", UpdaterChannel.PHYSICS in active)

        val physicsOnWithAsset = activeChannels(bai.copy(hasPhysicsAssets = true), physicsEnabled = false)
        assertTrue("physicsEnabled=false → PHYSICS 不注册（即便有资产）", UpdaterChannel.PHYSICS !in physicsOnWithAsset)
    }
}
