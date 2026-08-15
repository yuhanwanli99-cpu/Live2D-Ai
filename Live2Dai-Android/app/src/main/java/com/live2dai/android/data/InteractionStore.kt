package com.live2dai.android.data

import android.content.Context
import android.content.SharedPreferences

/**
 * 用户模型交互状态持久化（contract-C-M5 §2.2，Robolectric 可测）。
 *
 * - SharedPreferences：name = "live2dai_interaction"，MODE_PRIVATE
 * - 每模型独立：key = "override_{modelId}"，value = [encodeOverride] 的 JSON 字符串
 * - key 存在 = 用户已调整（override 覆盖自动 fit）；key 缺失/损坏 → 无 override（走 C-M1 fit）
 */
class InteractionStore(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)

    /** 读取某模型的用户 override；无 key / 损坏 → null（不崩溃）。 */
    fun load(modelId: String): Override? {
        val raw = prefs.getString(keyFor(modelId), null) ?: return null
        return decodeOverride(raw)
    }

    /** 覆盖写某模型的用户 override。非有限值（NaN/Inf）经 [encodeOverride] 省略字段 → load 解码返回 null（拒绝，不崩溃）。 */
    fun save(modelId: String, override: Override) {
        prefs.edit()
            .putString(
                keyFor(modelId),
                encodeOverride(override.scale, override.xOffset, override.yOffset)
            )
            .apply()
    }

    /** 清除某模型的用户 override（回 fit）。 */
    fun clear(modelId: String) {
        prefs.edit().remove(keyFor(modelId)).apply()
    }

    /** key 规则（§2.2：key = "override_{modelId}"）。 */
    fun keyFor(modelId: String): String = "override_$modelId"

    companion object {
        const val PREF_NAME = "live2dai_interaction"
    }
}
