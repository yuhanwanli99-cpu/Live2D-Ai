package com.live2dai.android

/**
 * M7 motion 标签映射 / 关键词兜底 / idle 多样化 —— 决策逻辑纯类。
 *
 * 设计（contract-M7-android-l3.md §2.1）：模型固定（mao_pro 主模型）、无运行时配置需求、
 * M1 §5 表已锁死键值 → 常量对象（非配置文件）；纯类可被本地 JVM 单测直接断言，
 * renderer 侧仅薄胶水。
 */

/**
 * motionLabel 别名 → loadedMotions 键 映射表。
 * 值来源：contract-M1 §5.3 表（mao_pro Android 键列；键格式 = "${group}_$i"，loadAllMotions L1312）。
 * 注意：mao_pro 空组键形如 "_0"，不是笔误——组名是空字符串（M1 §5.1 实读）。
 * shizuku 模型无这些键 → 解析出的键查 loadedMotions 失败 → 静默忽略，天然按模型适配。
 */
object MotionAliasTable {

    /** tag → loadedMotions 键。语义列（wave=special_01 是否"挥手"）[UNSURE-1] 待验证，键映射已锁定。 */
    val TAG_TO_KEY: Map<String, String> = mapOf(
        "idle"       to "Idle_0",   // mtn_01（Idle 组）
        "wave"       to "_3",       // special_01（ParamAngleY 摆至 -22.9°，大动作）— 语义 [UNSURE-1]
        "special_02" to "_4",       // special_02（-22.8°）— [UNSURE-1]
        "special_03" to "_5",       // special_03（-25.9°）— [UNSURE-1]
        "mtn_02"     to "_0",       // mtn_02（3.47s 普通动作）
        "mtn_03"     to "_1",       // mtn_03（4.40s 普通动作）
        "mtn_04"     to "_2",       // mtn_04（4.20s 普通动作）
    )

    /**
     * 解析别名 → 键。输入 trim + lowercase 后查表（容错，与 M1 §1-C1 大小写不敏感精神一致）。
     * @return 键；未知别名/空白 → null（调用方静默忽略）
     */
    fun resolve(tag: String): String? = TAG_TO_KEY[tag.trim().lowercase()]
}

/**
 * idle 池抽选器：随机选 + 最近 N 个不重复（洗牌袋语义）。
 * 纯逻辑类，不依赖 renderer/GL，可直接单测。
 *
 * @param pool     可用 idle 键池（调用方构造时与 loadedMotions 求交集，剔除未加载键）
 * @param noRepeat 最近 N 个不重复（默认 3）；池内全部播过一遍后清空历史允许重复
 */
class IdleMotionSelector(
    private val pool: List<String>,
    private val noRepeat: Int = IDLE_NO_REPEAT,
) {
    companion object {
        /** 最近不重复窗口（默认 3：池 6 键时保证 ≥ 50% 新鲜度，又不至于长期饥饿） */
        const val IDLE_NO_REPEAT: Int = 3

        /** 默认 idle 池 = mao_pro 空组 6 个键（mtn_02~04 + special_01~03，M1 §5.3 可用池） */
        val DEFAULT_POOL: List<String> = listOf("_0", "_1", "_2", "_3", "_4", "_5")
    }

    /** 最近 noRepeat 次已选键（头部是最旧的） */
    private val history = ArrayDeque<String>()

    /**
     * 取下一个 idle 键。
     * @return 池中随机一个不在最近 noRepeat 历史内的键；池空 → null（调用方走 §5.4 兜底）；
     *         候选集为空（历史覆盖全池）→ 清空历史后重选
     */
    fun next(): String? {
        if (pool.isEmpty()) return null
        var candidates = pool.filter { it !in history }
        if (candidates.isEmpty()) {
            // 历史覆盖全池 → 清空历史重选（洗牌袋：池内全部播过一遍后允许重复）
            history.clear()
            candidates = pool
        }
        val chosen = candidates.random()
        history.addLast(chosen)
        while (history.size > noRepeat) {
            history.removeFirst()
        }
        return chosen
    }
}

/**
 * 文本关键词 → motionLabel 别名 映射表（§4.2 使用；关键词 → 别名 → 键 再经 MotionAliasTable）。
 * 表结构与匹配机制锁定；行的语义值 [UNSURE-1] 待模型上肉眼验证。
 */
object KeywordMotionTable {

    /**
     * 关键词 → motionLabel 别名。
     * 初始行为建议值：挥手/招呼 → wave；再见/拜拜 → special_02。
     * ⚠️ 语义依赖 [UNSURE-1]（motion3.json 无动作语义标签，special_01 是否"挥手"未验证）。
     * 保守原则：不收录"你好"（日常回复高频词，会过度触发）；扩表 = 契约修订。
     */
    val KEYWORDS: Map<String, String> = mapOf(
        "挥手" to "wave",
        "招招手" to "wave",
        "打招呼" to "wave",
        "再见" to "special_02",
        "拜拜" to "special_02",
        "掰掰" to "special_02",
    )

    /** 关键词命中：position = 在文本中的字符下标（首个命中）；alias = 目标别名 */
    data class Hit(val position: Int, val alias: String)

    /**
     * 在文本中扫描关键词（子串匹配，contains）。
     * 规则（锁死）：按 position 升序返回；同别名只保留最早命中（一次 speak 内同别名至多触发一次）；
     * 无命中 → 空列表。空文本/空白 → 空列表。
     */
    fun find(text: String): List<Hit> {
        if (text.isBlank()) return emptyList()
        // 逐关键词扫描，每个别名取其最早命中位置
        val bestByAlias = mutableMapOf<String, Int>()
        for ((keyword, alias) in KEYWORDS) {
            val pos = text.indexOf(keyword)
            if (pos >= 0) {
                val prev = bestByAlias[alias]
                if (prev == null || pos < prev) bestByAlias[alias] = pos
            }
        }
        return bestByAlias
            .map { (alias, pos) -> Hit(pos, alias) }
            .sortedBy { it.position }
    }
}
