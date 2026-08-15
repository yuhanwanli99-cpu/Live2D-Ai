package com.live2dai.android.render

/**
 * 可调度更新器（seam，纯 JVM）。
 * 对应官方 ICubismUpdater 的 JVM 形态：[id] 唯一标识，[onUpdate] 每帧驱动。
 */
interface SchedulerUpdater {
    val id: String
    fun onUpdate(dt: Float)
}

/**
 * 更新调度器（seam，纯 JVM）：按注册序驱动各 updater 的 [SchedulerUpdater.onUpdate]。
 *
 * - [register]：防同 id 双注册（幂等忽略，保留首次注册的实例）；
 * - [unregister]：动态注销安全（注销未注册 id 为 no-op）；
 * - [frame]：按注册序调用（迭代快照，onUpdate 内注册/注销不抛并发修改）；
 * - [activeChannels]：当前已注册的 updater id 列表（注册序）。
 */
class UpdateScheduler {

    private val registered = LinkedHashMap<String, SchedulerUpdater>()

    /** 当前已注册的 updater id（注册序）。 */
    val activeChannels: List<String>
        get() = registered.keys.toList()

    /** 注册 updater；同 id 已注册则忽略（防双注册）。 */
    fun register(updater: SchedulerUpdater) {
        if (updater.id !in registered) {
            registered[updater.id] = updater
        }
    }

    /** 注销 updater；未注册 id 安全忽略。 */
    fun unregister(updaterId: String) {
        registered.remove(updaterId)
    }

    /** 按注册序调用各 updater 的 onUpdate(dt)。 */
    fun frame(dt: Float) {
        // 快照迭代：允许 onUpdate 内 register/unregister 而不抛并发修改
        for (updater in registered.values.toList()) {
            updater.onUpdate(dt)
        }
    }
}
