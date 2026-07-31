package com.live2dai.plugin.memory

/**
 * 记忆关键词匹配工具——MVP 用简单关键词重叠打分，不引入向量数据库
 * （个人项目记忆条目量级不需要），见 docs/plan-task-plugin-memory.md §2.2。
 *
 * 打分公式：`score(fact, userMessage) = 关键词交集数量 / fact.keywords.size`
 *
 * 已知局限（文档 R3）：关键词检索在「语义相似但字面不同」的场景会漏检
 * （如"我养猫" vs "我的宠物"），MVP 阶段可接受，向量检索留作后续增强。
 */
internal object MemoryKeywordMatcher {

    /** 中文高频虚词/停用字，过滤后降低检索噪声（"的/了/我/你"等） */
    private val CJK_STOPWORDS = setOf(
        '的', '了', '我', '你', '他', '她', '它', '是', '在', '和', '就', '不', '有', '也',
        '都', '把', '被', '还', '很', '真', '好', '呀', '啊', '吧', '呢', '吗', '这', '那',
        '个', '们', '与', '及', '或', '但', '而', '若', '要', '会', '能', '去', '来', '对',
        '到', '从', '于', '向', '为', '以', '之', '其', '此', '各', '每', '只', '更', '最',
        '已', '将', '再', '又', '并', '且', '虽', '没', '么', '什', '怎', '请', '问',
    )

    /**
     * 从文本中提取关键词（与事实存储的关键词使用同一套规则，保证打分可比）：
     * - 中文：二字滑窗词（二字均非停用字）优先，如 "咖啡"、"豆豆"
     * - 英文/数字词（≥2 字符，转小写），如 "coffee"
     * - 中文单字（剔除停用字），如 "猫"
     *
     * 二字词优先入列（精度高，且保证 [maxKeywords] 截断不会先丢自然词）；
     * 单字保召回（"猫"能命中"猫豆豆"）。
     */
    internal fun extractKeywords(text: String, maxKeywords: Int = 24): List<String> {
        val tokens = mutableListOf<String>()
        val cjkPattern = Regex("[\\u4e00-\\u9fff]+")
        for (run in cjkPattern.findAll(text)) {
            val runText = run.value
            // 二字滑窗：仅保留二字均非停用字的自然词（先入列）
            if (runText.length >= 2) {
                for (i in 0..runText.length - 2) {
                    val bigram = runText.substring(i, i + 2)
                    if (bigram[0] !in CJK_STOPWORDS && bigram[1] !in CJK_STOPWORDS) {
                        tokens += bigram
                    }
                }
            }
        }
        // 英文/数字词
        val asciiPattern = Regex("[a-zA-Z0-9_]{2,}")
        asciiPattern.findAll(text).forEach { tokens += it.value.lowercase() }
        // 单字：剔除停用字（最后入列，保障二字词先占满配额）
        for (ch in text) {
            if (ch.code in 0x4E00..0x9FFF && ch !in CJK_STOPWORDS) {
                tokens += ch.toString()
            }
        }
        return tokens.distinct().take(maxKeywords)
    }

    /**
     * 单条打分：`关键词交集数量 / fact.keywords.size`。
     * 事实关键词为空时返回 0（避免除零，也视为不相关）。
     */
    internal fun score(fact: MemoryFact, messageKeywords: Set<String>): Double {
        val factKeywords = fact.keywords.split(',')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (factKeywords.isEmpty()) return 0.0
        val overlap = factKeywords.count { it in messageKeywords }
        return overlap.toDouble() / factKeywords.size
    }

    /**
     * 按关键词重叠打分排序，返回 score > 0 的 top-N（默认 3）。
     * 稳定排序：同分时 id 小的在前，保证运行与测试行为确定。
     */
    internal fun rankTop(facts: List<MemoryFact>, userMessage: String, limit: Int = 3): List<MemoryFact> {
        val messageKeywords = extractKeywords(userMessage, maxKeywords = 64).toSet()
        return facts.asSequence()
            .map { fact -> fact to score(fact, messageKeywords) }
            .filter { (_, s) -> s > 0.0 }
            .sortedWith(compareByDescending<Pair<MemoryFact, Double>> { it.second }.thenBy { it.first.id })
            .take(limit)
            .map { it.first }
            .toList()
    }
}
