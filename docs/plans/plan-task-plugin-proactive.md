# plan-task-plugin-proactive — 主动陪伴插件设计

> 版本: 1.0.0
> 日期: 2026-07-31
> 目标: 把骨架 `live2d-ai-plugin-proactive`（`docs/plans/test-task-plugin-skeletons.md` 已验收）落地为具备"主动找用户说话"能力的插件
> 前置依赖: `task-plugin-proactive-skeleton`（已完成）、Wave 0（`PluginHost.sendAssistantMessage` 已提供接口，尚无实现调用方）
> 不做的事: 不接入 core，功能验收通过后再排期接入

---

## 1. 需求概述

### 1.1 背景

对照 N.E.K.O. 的"主动陪伴"能力：猫娘会不经用户输入主动发起对话（如"好久没理我了""该喝水啦"）。MVP 阶段只做**基于时间的调度**，不做"检测用户情绪/行为"等复杂触发（那属于更高阶能力，后续再评估）。

### 1.2 能力形态

| 触发类型 | 说明 | 实现方式 |
| ------ | ------ | ------ |
| 定时问候 | 距离上次对话超过 N 小时后触发一句关心的话 | `WorkManager` 周期性任务 |
| 时段问候 | 每天固定时间段（如早安/晚安）触发 | `WorkManager` + 时间窗口判断 |

不做的：基于地理位置/传感器/使用行为的触发（超出 MVP 范围，且涉及额外权限，风险大）。

### 1.3 验收标准

1. App 处于前台或后台（进程存活）时，达到触发条件会调用 `PluginHost.sendAssistantMessage()`
2. 触发文案由 LLM 即时生成（结合人设+当前时间），而非固定死板文案池（更贴近"猫娘"人设，但保留静态文案池作为 LLM 调用失败时的兜底）
3. 不会在用户正在对话时突兀打断（需检查"最近是否有进行中的对话"）
4. 调度逻辑有单元测试覆盖（不依赖真实 WorkManager 定时器，用可注入的 Clock/Scheduler 接口做确定性测试）

---

## 2. 系统架构

```
┌───────────────────────────────────────────────┐
│                  Core (app)                    │
│  MainActivity                                  │
│    └─ PluginManager.loadPlugins([              │
│         ProactivePlugin(                       │
│           ProactivePluginConfig(               │
│             context = applicationContext,      │  ← WorkManager 需要 Context
│             greetingLlmClient = ...,           │  ← 复用 core 的 LLM 客户端（同 memory 插件模式）
│             idleThresholdMinutes = 180,        │
│             quietHours = 23..7,                │  ← 静默时段，不打扰睡眠
│           )                                     │
│       )])                                       │
└───────────────────────┬─────────────────────────┘
                         │ onLoad(host)
                         ▼
┌───────────────────────────────────────────────┐
│          live2d-ai-plugin-proactive             │
│                                                 │
│  ProactivePlugin                               │
│    ├─ 注册 WorkManager PeriodicWorkRequest      │
│    │     (ProactiveCheckWorker, 每 15 分钟检查一次) │
│    ├─ ProactiveCheckWorker (CoroutineWorker)   │
│    │     检查: 距上次对话时间 / 是否在静默时段    │
│    │     → 触发 → host.sendAssistantMessage()   │
│    └─ LastInteractionTracker                    │
│          记录"上次对话时间"（通过 ChatHook.afterReceive 更新）│
└─────────────────────────────────────────────────┘
```

### 2.1 关键设计决策：为什么用 WorkManager 而不是自己起 Handler/Timer？

- **WorkManager 是 AndroidX 官方组件**，天然处理"进程被杀后恢复调度""Doze 模式下延迟执行"等 Android 系统级坑，个人开发不需要重新踩一遍
- 15 分钟是 `PeriodicWorkRequest` 允许的**最小间隔**（Android 系统限制，硬约束，不可更短），因此"定时问候"的精度只能是 15 分钟级别，不能做到分钟级精确触发——这是 Android 平台限制，非设计缺陷
- `ProactivePlugin.onLoad()` 里通过 `WorkManager.getInstance(context).enqueueUniquePeriodicWork(...)` 注册，`onUnload()` 里 `cancelUniqueWork(...)`，保证插件卸载后不留后台任务

### 2.2 关键设计决策：如何避免"用户正在对话时突然插话"

`ProactiveCheckWorker` 触发前必须满足：

```
距离上次用户消息发送时间 > idleThresholdMinutes（默认 180 分钟 = 3 小时）
且不在 quietHours 静默时段内
且距离上一次主动消息发送 > 最小冷却时间（如 6 小时，避免频繁刷屏）
```

"上次对话时间"由插件自己的 `ChatHook.afterReceive` 更新（每轮真实对话都会刷新这个时间戳），而不是猜测 App 是否在前台——这样即使用户在别的插件/别的 App 但没跟猫娘说话，也不会被误判为"活跃中"。

---

## 3. 数据模型

### 3.1 ProactivePluginConfig（扩展骨架版）

```kotlin
data class ProactivePluginConfig(
    val context: Context,
    val greetingLlmClient: GreetingLlmClient,       // 见 3.2，复用 memory 插件同款接缝模式
    val idleThresholdMinutes: Long = 180,
    val cooldownMinutes: Long = 360,
    val quietHoursStart: Int = 23,                   // 24 小时制，23 表示 23:00
    val quietHoursEnd: Int = 7,                      // 7 表示 07:00（跨天区间）
    val fallbackGreetings: List<String> = listOf(
        "在忙什么呀？好久没理我了~",
        "主人，该喝水啦！",
        "今天也要元气满满哦！",
    ),
)
```

### 3.2 GreetingLlmClient（沿用 memory 插件的接缝模式）

```kotlin
fun interface GreetingLlmClient {
    suspend fun generateGreeting(personaName: String, systemPrompt: String, idleMinutes: Long): String?
}
```

由 core 注入实现，插件不关心具体 LLM 厂商；返回 `null` 或调用异常时，`ProactiveCheckWorker` 回退到 `fallbackGreetings` 随机取一条，保证"主动陪伴"功能不会因为网络问题而完全失效。

### 3.3 LastInteractionTracker（持久化，进程重启不丢失）

用 `SharedPreferences`（而非 Room，数据极简，一个时间戳而已，Room 是过度设计）：

```kotlin
class LastInteractionTracker(context: Context) {
    private val prefs = context.getSharedPreferences("live2dai_proactive", Context.MODE_PRIVATE)
    fun recordInteraction(atMillis: Long = System.currentTimeMillis()) =
        prefs.edit().putLong(KEY_LAST_INTERACTION, atMillis).apply()
    fun recordProactiveMessage(atMillis: Long = System.currentTimeMillis()) =
        prefs.edit().putLong(KEY_LAST_PROACTIVE, atMillis).apply()
    fun minutesSinceLastInteraction(nowMillis: Long = System.currentTimeMillis()): Long { /* ... */ }
    fun minutesSinceLastProactive(nowMillis: Long = System.currentTimeMillis()): Long { /* ... */ }

    companion object {
        private const val KEY_LAST_INTERACTION = "last_interaction_at"
        private const val KEY_LAST_PROACTIVE = "last_proactive_at"
    }
}
```

`nowMillis` 参数化（而非内部直接调用 `System.currentTimeMillis()`）是为了让单元测试可以注入固定时间，避免"时间相关的 flaky test"。

---

## 4. 接口定义

### 4.1 ProactivePlugin

```kotlin
class ProactivePlugin(private val config: ProactivePluginConfig) : Live2DAiPlugin {
    override val id: String = "proactive"
    override val version: String = "1.0.0"

    private lateinit var tracker: LastInteractionTracker

    override fun onLoad(host: PluginHost) {
        tracker = LastInteractionTracker(config.context)
        host.registerHook(ProactiveTrackingHook(tracker))

        val request = PeriodicWorkRequestBuilder<ProactiveCheckWorker>(15, TimeUnit.MINUTES)
            .setInputData(workDataOf(/* config 序列化后的必要字段 */))
            .build()
        WorkManager.getInstance(config.context)
            .enqueueUniquePeriodicWork("live2dai_proactive_check", ExistingPeriodicWorkPolicy.KEEP, request)
    }

    override fun onUnload() {
        WorkManager.getInstance(config.context).cancelUniqueWork("live2dai_proactive_check")
    }
}
```

### 4.2 ProactiveTrackingHook（只更新时间戳，不注入上下文）

```kotlin
class ProactiveTrackingHook(private val tracker: LastInteractionTracker) : ChatHook {
    override suspend fun beforeSend(userMessage: String, context: ChatContext): String? = null
    override suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext) {
        tracker.recordInteraction()
    }
}
```

> 注：`afterReceive` 签名已按 `plan-task-plugin-memory.md` §4.2 的方案 A 扩展为带 `context` 参数——两个 Wave 2 功能任务都依赖这一处 SDK 改动，需协调好谁先落地（建议 `task-plugin-memory` 先做这处 SDK 改动，`task-plugin-proactive` 直接复用，避免冲突）。

### 4.3 ProactiveCheckWorker（真正决策 + 触发的地方）

```kotlin
class ProactiveCheckWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val tracker = LastInteractionTracker(applicationContext)
        val now = System.currentTimeMillis()

        if (isWithinQuietHours(now)) return Result.success()
        if (tracker.minutesSinceLastInteraction(now) < idleThresholdMinutes) return Result.success()
        if (tracker.minutesSinceLastProactive(now) < cooldownMinutes) return Result.success()

        val greeting = runCatching {
            greetingLlmClient.generateGreeting(personaName, systemPrompt, tracker.minutesSinceLastInteraction(now))
        }.getOrNull() ?: fallbackGreetings.random()

        host.sendAssistantMessage(greeting)  // 见 §5 风险 R1：Worker 如何拿到 host 引用
        tracker.recordProactiveMessage(now)
        return Result.success()
    }
}
```

---

## 5. 文件清单

| # | 文件路径 | 变更类型 |
| --- | --------- | --------- |
| 1 | `live2d-ai-plugin-proactive/build.gradle.kts` | 修改：新增 `androidx.work:work-runtime-ktx` 依赖 |
| 2 | `live2d-ai-plugin-proactive/.../LastInteractionTracker.kt` | 新增 |
| 3 | `live2d-ai-plugin-proactive/.../GreetingLlmClient.kt` | 新增：接口定义 |
| 4 | `live2d-ai-plugin-proactive/.../ProactiveTrackingHook.kt` | 新增：ChatHook 实现 |
| 5 | `live2d-ai-plugin-proactive/.../ProactiveCheckWorker.kt` | 新增：CoroutineWorker |
| 6 | `live2d-ai-plugin-proactive/.../ProactivePlugin.kt` | 修改：`onLoad` 里真正注册 hook + 排期 WorkManager |
| 7 | `live2d-ai-plugin-proactive/.../ProactivePluginConfig.kt` | 修改：按 3.1 补充真实字段 |
| 8 | `live2d-ai-plugin-proactive/src/test/.../LastInteractionTrackerTest.kt` | 新增：注入固定时间验证冷却/静默逻辑 |
| 9 | `live2d-ai-plugin-proactive/src/test/.../ProactiveCheckWorkerTest.kt` | 新增：用 `TestListenableWorkerBuilder`（AndroidX Work 官方测试工具）验证决策分支 |
| 10 | `live2d-ai-plugin-proactive/src/test/.../ProactivePluginTest.kt` | 修改：骨架版"零注册"断言 → "确实注册了 hook + WorkManager 任务" |
| — | `app/**` | **不修改** |

---

## 6. 风险与注意事项

| # | 风险 | 缓解措施 |
| --- | ------ | --------- |
| R1 | `CoroutineWorker` 由系统调度创建，**拿不到** `onLoad(host)` 传入的 `PluginHost` 实例引用（Worker 是独立生命周期对象，不是插件持有的对象） | 需要一个进程内单例桥接（如插件内部持有 `object ProactiveHostBridge { var host: PluginHost? = null }`，`onLoad` 时赋值，`onUnload` 时清空，Worker 内部读取），这是本任务与骨架阶段最大的架构补充点，需要在实现前明确写好，避免遗漏 |
| R2 | Doze 模式/电池优化可能进一步延迟 `PeriodicWorkRequest` 执行（不仅是 15 分钟粒度，系统可能整体延后） | 属于 Android 平台已知限制，MVP 阶段接受"触发时间是软性的、尽力而为"，不承诺精确送达 |
| R3 | 用户可能觉得主动消息打扰 | `quietHours` + `cooldownMinutes` 双重限流；后续可加"设置里一键关闭主动陪伴"开关（core 侧 UI，超出本插件范围，记录为 core 侧后续任务） |
| R4 | 时间相关测试容易 flaky | `LastInteractionTracker` 所有方法都参数化 `nowMillis`，测试用固定时间戳，不依赖真实系统时钟 |

---

## 7. 验收标准（供后续 tester 使用）

1. `LastInteractionTrackerTest`：模拟"3 小时前对话 + 现在" → `minutesSinceLastInteraction` 返回 180 左右，误差在测试容忍范围内
2. `ProactiveCheckWorkerTest`：`idleThresholdMinutes` 未到 → `doWork()` 不调用 `sendAssistantMessage`
3. `ProactiveCheckWorkerTest`：满足所有条件 → 调用 `sendAssistantMessage` 且传入的文案来自 mock 的 `GreetingLlmClient` 返回值
4. `ProactiveCheckWorkerTest`：`GreetingLlmClient` 抛异常 → 回退到 `fallbackGreetings`，测试流程不中断
5. `ProactiveCheckWorkerTest`：静默时段内 → 不触发
6. 插件独立编译通过（`:live2d-ai-plugin-proactive:assembleDebug`）
