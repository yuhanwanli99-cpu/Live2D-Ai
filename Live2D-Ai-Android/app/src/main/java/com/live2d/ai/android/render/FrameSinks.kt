package com.live2d.ai.android.render

/**
 * 帧参数表（seam，纯 JVM）：外部（口型/表情/交互）写入参数，GL 线程帧首 drain 取走。
 *
 * 帧序契约（contract-refactor-0814 §2.3-④）：drain(参数表) → update(dt) → draw()；
 * 参数写入先于更新、更新先于绘制。写入为"最新值覆盖"语义，drain 取走并清空。
 */
open class FrameSinks {

    private val table = LinkedHashMap<String, Float>()

    /** 写入单个参数（最新值覆盖）。 */
    @Synchronized
    open fun write(name: String, value: Float) {
        table[name] = value
    }

    /** 批量写入（最新值覆盖）。 */
    @Synchronized
    open fun writeAll(values: Map<String, Float>) {
        table.putAll(values)
    }

    /**
     * 帧首 drain：取走当前全部参数并清空（参数写入先于更新）。
     * 无 pending 时返回空表（不分配）。
     */
    @Synchronized
    open fun drain(): Map<String, Float> {
        if (table.isEmpty()) return emptyMap()
        val out = LinkedHashMap(table)
        table.clear()
        return out
    }

    /** 清空 pending 表（不触发任何写）。 */
    @Synchronized
    open fun clear() {
        table.clear()
    }

    /** 当前 pending 参数个数（测试/诊断用）。 */
    @Synchronized
    fun size(): Int = table.size

    /** 当前 pending 是否为空。 */
    @Synchronized
    fun isEmpty(): Boolean = table.isEmpty()
}
