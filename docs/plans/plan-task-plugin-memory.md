# plan-task-plugin-memory — 记忆插件设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 把骨架 `live2d-ai-plugin-memory`（`docs/plans/test-task-plugin-skeletons.md` 已验收）落地为具备真实记忆能力的插件
> 前置依赖: `task-plugin-memory-skeleton`（已完成）、Wave 0（plugin-sdk + ChatService hook 点）已完成
> 不做的事: 不接入 core（`MainActivity.kt`），功能验收通过后再排期接入

---

## 1. 需求概述

### 1.1 背景

对照 N.E.K.O. 的"多层记忆"理念，MVP 阶段做**两层**即可，不做五层：

| 层级 | 说明 | 存储 |
| ------ | ------ | ------ |
| 短期记忆（近期对话） | 最近 N 轮原始对话 | 内存 + `ChatContext.recentMessages`（已由 core 提供，无需插件重复存储） |
| 长期记忆（事实摘要） | 用户的稳定信息（如"用户叫小明，养了一只猫"），由对话内容定期总结生成 | Room（SQLite） |

短期记忆已经由 `PluginHost`/`ChatContext` 提供只读快照，**插件本身只需要做长期记忆**（事实摘要的存储、检索、注入），避免重复造轮子。

### 1.2 能力形态

| 能力 | 注册方式 | 触发时机 |
| ------ | --------- | --------- |
| 记忆检索注入 | `ChatHook.beforeSend` | 每次用户发消息前，检索相关记忆摘要注入上下文 |
| 记忆写入 | `ChatHook.afterReceive` | 每轮对话结束后，判断是否触发摘要生成（如每 10 轮或检测到"记住"类关键词） |

本 MVP **不**注册 `ToolDefinition`（不做"主动查询记忆"工具），因为 `beforeSend` 自动注入已覆盖主要场景；"记住这件事"工具可作为 P2 后续增强。

### 1.3 验收标准

1. 插件独立编译通过（Android Library + Room 依赖）
2. 单元测试覆盖：写入→检索往返正确；相关性检索能命中语义相关的摘要（关键词或简单向量均可，见 3.3）
3. 无网络请求失败/数据库异常导致对话流程中断（hook 内部异常需被吞掉，不上抛）
4. 骨架阶段的 `MemoryPluginConfig` 需要按本设计扩展新增字段，且保持向后兼容（`placeholder` 字段可删除）

---

## 2. 系统架构

```
┌───────────────────────────────────────────────┐
│                  Core (app)                    │
│  MainActivity                                  │
│    └─ PluginManager.loadPlugins([              │
│         MemoryPlugin(                          │
│           MemoryPluginConfig(                  │
│             context = applicationContext,      │  ← Room 需要 Context
│             summaryTriggerEveryNTurns = 10,     │
│           )                                     │
│         )                                       │
│       ])                                        │
└───────────────────────┬─────────────────────────┘
                         │ onLoad(host)
                         ▼
┌───────────────────────────────────────────────┐
│           live2d-ai-plugin-memory               │
│                                                 │
│  MemoryPlugin                                  │
│    ├─ registerHook(MemoryRecallHook)           │
│    │     beforeSend  → 检索 MemoryDao          │
│    │     afterReceive → 写入 + 判断是否摘要      │
│    ├─ MemoryDatabase (Room)                    │
│    │     Entity: MemoryFact(id, content,       │
│    │       keywords, createdAt, lastUsedAt)    │
│    ├─ MemoryDao (CRUD + 简单检索)               │
│    └─ SummaryGenerator                         │
│          调用 core 传入的 LLM 摘要能力           │
│          （见 2.1 关键决策）                     │
└─────────────────────────────────────────────────┘
```

### 2.1 关键设计决策：摘要生成用谁的 LLM？

**问题**：生成记忆摘要需要调用一次 LLM（把近期对话浓缩成一句事实），插件要不要自己再接一个 LLM 客户端？

**决策**：**不**在插件内重新实现 LLM 客户端。`MemoryPluginConfig` 注入一个函数类型依赖：

```kotlin
fun interface SummaryLlmClient {
    suspend fun summarize(recentTurns: List<ChatTurn>, personaName: String): String?
}
```

由 core 提供实现（内部复用现有 `LLMProvider.kt` 已有的 HTTP 客户端逻辑），插件不关心是 GLM/OpenAI/本地模型。理由与 vision 插件的 `ScreenshotProvider` 接缝完全一致——插件只定义"需要什么能力"的接口，具体实现留给 core 注入，保持插件与厂商 API 解耦。

### 2.2 检索策略（MVP：关键词匹配，不做向量检索）

MVP 阶段用简单的关键词重叠打分，**不引入向量数据库**（过度设计，个人项目记忆条目量级不需要）：

```
score(fact, userMessage) = 关键词交集数量 / fact.keywords.size
取 score 最高的 top-3（score > 0 才纳入），按 lastUsedAt 更新
```

后续如果记忆条目膨胀（如 > 500 条）出现检索质量问题，再评估引入本地向量方案（如 `ObjectBox` 或 SQLite FTS5），不在本次范围内。

---

## 3. 数据模型

### 3.1 MemoryFact（Room Entity）

```kotlin
@Entity(tableName = "memory_facts")
data class MemoryFact(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val content: String,          // 事实摘要文本，如"用户养了一只叫豆豆的猫"
    val keywords: String,         // 逗号分隔的关键词，用于检索打分
    val createdAt: Long,          // epoch millis
    val lastUsedAt: Long,         // 最近一次被检索命中的时间，用于遗忘策略（MVP 暂不做主动遗忘）
)
```

### 3.2 MemoryPluginConfig（扩展骨架版）

```kotlin
data class MemoryPluginConfig(
    val context: Context,                       // 用于构建 Room database
    val summaryLlmClient: SummaryLlmClient,      // 见 2.1
    val summaryTriggerEveryNTurns: Int = 10,     // 每 N 轮触发一次摘要生成
    val databaseName: String = "live2dai_memory.db",
)
```

### 3.3 MemoryDao

```kotlin
@Dao
interface MemoryDao {
    @Insert suspend fun insert(fact: MemoryFact): Long
    @Query("SELECT * FROM memory_facts") suspend fun getAll(): List<MemoryFact>
    @Update suspend fun update(fact: MemoryFact)
}
```

检索逻辑（打分）在 `MemoryDao.getAll()` 之上用 Kotlin 代码实现，不下沉到 SQL（MVP 数据量小，全表扫描+内存打分足够简单可靠）。

---

## 4. 接口定义

### 4.1 MemoryRecallHook

```kotlin
class MemoryRecallHook(
    private val dao: MemoryDao,
    private val summaryLlmClient: SummaryLlmClient,
    private val triggerEveryNTurns: Int,
) : ChatHook {

    private var turnCounter = 0

    override suspend fun beforeSend(userMessage: String, context: ChatContext): String? {
        val facts = dao.getAll()
        val relevant = rankByKeywordOverlap(facts, userMessage).take(3)
        if (relevant.isEmpty()) return null
        return "[系统: 相关记忆]\n" + relevant.joinToString("\n") { "- ${it.content}" }
    }

    override suspend fun afterReceive(assistantMessage: String, emotion: String?) {
        turnCounter++
        if (turnCounter % triggerEveryNTurns != 0) return
        // 触发摘要：由 core 侧的 ChatContext 提供近期对话（afterReceive 签名当前不带 context，
        // 需要评估是否扩展 ChatHook.afterReceive 增加 context 参数——见 §6 风险 R1）
    }
}
```

### 4.2 PluginHost / ChatHook 接口是否需要扩展？

`afterReceive(assistantMessage: String, emotion: String?)` 当前**不带** `ChatContext`，但摘要生成需要"近期几轮对话"。两个方案：

- **方案 A（推荐）**：扩展 `ChatHook.afterReceive` 签名为 `afterReceive(assistantMessage, emotion, context: ChatContext)`，因为 `beforeSend` 已经证明 `ChatContext` 是安全的只读快照，afterReceive 同理不会破坏封装
- 方案 B：插件自己维护一份本地滑动窗口（在 `beforeSend`/`afterReceive` 分别喂入 user/assistant 消息拼出历史），不改 SDK 接口，但会与 core 的 `recentMessages` 产生重复状态，容易不一致

**决策：采用方案 A**，作为本任务对 `plugin-sdk` 的唯一一处改动（需同步更新 vision 插件的 `VisionDescribeHook.afterReceive` 签名，因为它也实现了 `ChatHook`——但 vision 的 afterReceive 是空实现，改动零风险）。

---

## 5. 文件清单

| # | 文件路径 | 变更类型 |
| --- | --------- | --------- |
| 1 | `plugin-sdk/src/main/java/com/live2d/ai/pluginsdk/ChatHook.kt` | 修改：`afterReceive` 增加 `context: ChatContext` 参数 |
| 2 | `live2d-ai-plugin-vision/.../VisionDescribeHook.kt` | 修改：同步新签名（空实现，加个参数即可） |
| 3 | `live2d-ai-plugin-memory/build.gradle.kts` | 修改：新增 Room 依赖（`androidx.room:room-runtime` + `room-ktx` + ksp/kapt `room-compiler`） |
| 4 | `live2d-ai-plugin-memory/.../MemoryFact.kt` | 新增：Room Entity |
| 5 | `live2d-ai-plugin-memory/.../MemoryDao.kt` | 新增：Room Dao |
| 6 | `live2d-ai-plugin-memory/.../MemoryDatabase.kt` | 新增：Room Database 单例构建 |
| 7 | `live2d-ai-plugin-memory/.../SummaryLlmClient.kt` | 新增：接口定义（fun interface） |
| 8 | `live2d-ai-plugin-memory/.../MemoryRecallHook.kt` | 新增：ChatHook 实现 |
| 9 | `live2d-ai-plugin-memory/.../MemoryPlugin.kt` | 修改：`onLoad` 里真正 `registerHook` |
| 10 | `live2d-ai-plugin-memory/.../MemoryPluginConfig.kt` | 修改：按 3.2 补充真实字段 |
| 11 | `live2d-ai-plugin-memory/src/test/.../MemoryDaoTest.kt` | 新增：Room 内存数据库测试（写入→查询） |
| 12 | `live2d-ai-plugin-memory/src/test/.../MemoryRecallHookTest.kt` | 新增：mock Dao + mock SummaryLlmClient，验证 beforeSend 注入格式、afterReceive 触发频率 |
| 13 | `live2d-ai-plugin-memory/src/test/.../MemoryPluginTest.kt` | 修改：骨架版断言（"零注册"）替换为"确实注册了 1 个 hook" |
| — | `app/**` | **不修改**（本任务不接入 core，见文档头部） |

---

## 6. 风险与注意事项

| # | 风险 | 缓解措施 |
| --- | ------ | --------- |
| R1 | `ChatHook.afterReceive` 签名变更是对 `plugin-sdk` 的破坏性改动，理论上任何未来插件都要跟进 | 当前只有 vision 插件实现了该接口，改动影响面小且是编译期可发现（Kotlin 接口方法签名不匹配会直接编译失败，不会静默出错） |
| R2 | Room + KSP 在这个沙盒环境同样受本地 Android SDK 限制，无法本地验证 assemble | 沿用 Wave 1/2 的做法：交给团队在正常 CI/WSL 环境跑 `assembleDebug` + `testDebugUnitTest`，本地做人工代码审查兜底 |
| R3 | 关键词打分检索在语义相似但字面不同的场景会漏检（如"我养猫" vs "我的宠物"） | MVP 阶段可接受，标注为已知局限，向量检索留作后续增强，不阻塞本任务 |
| R4 | 摘要生成失败（LLM 超时/异常）不应影响写入近期记忆的能力 | `afterReceive` 内部 try-catch 包裹摘要调用，摘要失败只跳过本轮摘要，不影响下一轮触发 |

---

## 7. 验收标准（供后续 tester 使用）

1. `MemoryDaoTest`：插入 3 条记忆 → 全表检索能取回 3 条，字段无损
2. `MemoryRecallHookTest`：mock 2 条相关记忆 + 1 条不相关记忆 → `beforeSend` 返回值只包含相关的 2 条，格式符合 `[系统: 相关记忆]` 前缀
3. `MemoryRecallHookTest`：`afterReceive` 调用 9 次不触发摘要，第 10 次触发（`summaryTriggerEveryNTurns=10`）
4. 插件独立编译通过（`:live2d-ai-plugin-memory:assembleDebug`）
5. `plugin-sdk` 签名变更后，`live2d-ai-plugin-vision` 模块同步更新并仍然编译通过（回归验证，防止破坏 Wave 1 成果）
