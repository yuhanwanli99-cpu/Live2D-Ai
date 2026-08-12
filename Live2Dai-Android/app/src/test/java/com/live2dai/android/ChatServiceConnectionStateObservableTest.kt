package com.live2dai.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * M4b 连接状态指示 —— 数据源暴露断言（契约 `.pi/spoq/contract-R1-ui.md` §2.2 / §3 **M4-B1**）。
 *
 * M4-B1：`grep：ChatService/ChatViewModel 出现连接状态可观察对象（StateFlow/Flow<ConnectionState> 或等价）`
 * — 这是契约自己定义的本条唯一机械验证方式（同款 grep 亦见于契约 §7 验收命令索引）。
 *
 * 本测试将契约 M4-B1 的 grep 判据固化为 JVM 可运行断言：对 `ChatService.kt` / `ChatViewModel.kt`
 * 源码**文本**扫描，断言…… 出现 `ConnectionState` 连接状态可观察对象（`StateFlow<ConnectionState>` 或等价
 * `Flow<ConnectionState>`）。
 *
 * 说明（对「禁读 src/ 实现」边界的交代）：M4-B1 的断言目标（ChatService/ChatViewModel 须暴露连接状态
 * 可观察对象）**完全取自契约文本**，不是从 src 实现反推；测试仅把契约 §7 的 grep 命令机械化为单测，
 * 与波1 `M4aKeyboardImePolicyTest` 对 AndroidManifest.xml 的 grep 断言同构（契约 M4-A1 亦是 grep）。
 * 这里扫描的 ChatService.kt/ChatViewModel.kt 是**数据（被 grep 的检索面）**，非用于推导测试逻辑的实现细节。
 *
 * **RED 依据**：契约 U7 实证「ChatService 现无任何连接状态可观察对象」→ 现状扫描零命中 → RED。
 * GREEN 条件：C3 为 ChatService/ChatViewModel 新增 `StateFlow<ConnectionState>`（或等价 Flow）连接状态
 * 可观察对象（最小暴露，契约 §2.2）。
 */
class ChatServiceConnectionStateObservableTest {

    private fun sourcePatch(pathEnd: String): File {
        var dir: File? = File(System.getProperty("user.dir")).absoluteFile
        // 同款 M4aKeyboardImePolicyTest 定位 AndroidManifest 的处理（Gradle 单测工作目录 = app 模块根，
        // IDE/脚本下 CWD = 仓库根）：两种相对布局都要试，自 user.dir 逐级上溯。
        // 目标文件位于 com/live2dai/android/ 包根（ChatViewModel.kt 与 ChatService.kt 同级，均在 package 根，不在 ui/）。
        val baseRel = "app/src/main/java/com/live2dai/android"
        val baseRoot = "Live2Dai-Android/app/src/main/java/com/live2dai/android"
        while (dir != null) {
            for (base in listOf(baseRel, baseRoot)) {
                val candidate = File(dir, "$base/$pathEnd")
                if (candidate.isFile) return candidate
            }
            dir = dir.parentFile
        }
        throw AssertionError(
            "未定位到 src 文件 $pathEnd（user.dir=" + System.getProperty("user.dir") + "）"
        )
    }

    private val chatServiceText: String by lazy {
        sourcePatch("ChatService.kt").readText()
    }

    private val chatViewModelText: String by lazy {
        sourcePatch("ChatViewModel.kt").readText()
    }

    /** 连接状态可观察对象的命中判据：类型为 StateFlow/Flow 且元素类型为 ConnectionState（或子串含该名）。 */
    private fun containsConnectionStateObservable(text: String): Boolean {
        val hasObservable = Regex("StateFlow\\s*<\\s*ConnectionState\\s*>|" +
            "Flow\\s*<\\s*ConnectionState\\s*>").containsMatchIn(text)
        val hasElement = text.contains("ConnectionState") &&
            (text.contains("StateFlow") || text.contains("<Flow"))
        return hasObservable || hasElement
    }

    // ────────────────────────────────────────────────────────────
    // RED：M4-B1 —— ChatService 暴露连接状态可观察对象（U7 实证现无）
    // ────────────────────────────────────────────────────────────

    @Test
    fun chatService_exposes_connectionState_observable() {
        assertTrue(
            "契约 M4-B1：ChatService 应暴露连接状态可观察对象（StateFlow/Flow<ConnectionState> 或等价）；" +
                "U7 实证现无 → 当前 RED。\nChatService.kt 命中判定=" +
                containsConnectionStateObservable(chatServiceText) +
                "\ntext sample=" + chatServiceText.take(1200),
            containsConnectionStateObservable(chatServiceText)
        )
    }

    // ────────────────────────────────────────────────────────────
    // RED：M4-B1 —— ChatViewModel 暴露连接状态可观察对象
    //    （契约 §2.2 注「ChatViewModel 或 ChatService 暴露」；任一暴露即满足，二者都扫）
    // ────────────────────────────────────────────────────────────

    @Test
    fun chatViewModel_exposes_connectionState_observable_or_absent_DUAL() {
        // 契约 M4-B1 允许连接状态可观察对象由 ChatService **或** ChatViewModel 暴露（§2.2 机制 coder 定）。
        // 只要二者之一已有可观察对象即算满足；但为守卫「新状态暴露」确实落地，至少一方必须命中，
        // 否则连接状态指示无从渲染（M4-B3 依赖）。
        val serviceHit = containsConnectionStateObservable(chatServiceText)
        val vmHit = containsConnectionStateObservable(chatViewModelText)
        assertTrue(
            "契约 M4-B1：ChatService **或** ChatViewModel 必须至少一方暴露连接状态可观察对象" +
                "（StateFlow/Flow<ConnectionState> 或等价）；现状 service=$serviceHit, viewModel=$vmHit " +
                "→ U7 实证两者皆无 → 当前 RED。\nChatViewModel.kt sample=" + chatViewModelText.take(1500),
            serviceHit || vmHit
        )
    }
}
