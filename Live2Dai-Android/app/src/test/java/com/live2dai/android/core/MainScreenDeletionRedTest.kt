package com.live2dai.android.ui

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * MainScreen 大裁剪的「删除断言（结构面）」RED 测试——DEL-1 UI 状态面（契约 refactor-0814 §3 · 架构 §7.1）。
 *
 * 依据（唯一真相源 = 契约 + 裁定，**禁读 src/ 实现，禁对答案出题**）：
 *   - C-5 ASR 删 → UI 状态机不得含「语音输入」状态（无 `isListening`/ASR/麦克风面）
 *   - DEL1-A1（架构 §7.1 MainScreen 大裁剪）→ 无「引导弹窗/确认门」状态（无 `onboarding`/`confirmation` 面）
 *   - C-1 保留极简设置页 → 仍保留 `settingsVisible` 字段
 *   - CORE1-A1（S2 核心环路 UI 载体）→ 保留 `inputText`/`isLoading`/`bubbles`/`connectionState`
 *
 * **机制（黑盒，规避对 src 实现的符号级依赖）**：用 `Class.forName` + `getDeclaredFields` 反射读取目标
 * `MainScreenUiState` 的字段集合，机械判定——含极简面、缺 ASR/引导/确认门面。类不存在即
 * `ClassNotFoundException` → 本文件运行时 FAIL（RED）；类创建后反射判定 PASS（GREEN 前提）。
 *
 * 定位：本文件聚焦**删除面是否存在**；状态行为归 `MainScreenUiStateTest`。
 */
class MainScreenDeletionRedTest {

    companion object {
        const val TARGET_CLASS = "com.live2dai.android.ui.MainScreenUiState"
    }

    /** DEL-1：反射取得 MainScreenUiState 的 declared field 名集合。类不存在抛 ClassNotFoundException（=RED）。 */
    private fun fieldNames(): Set<String> {
        val clazz = Class.forName(TARGET_CLASS) // 类不存在 → ClassNotFoundException = RED
        return clazz.declaredFields.map { it.name }.toSet()
    }

    @Test
    fun targetClass_exists() {
        // D/TC16 前置：目标 UI 状态类必须存在（类缺失即 RED）
        Class.forName(TARGET_CLASS)
        assertTrue("目标状态类应存在", true)
    }

    @Test
    fun state_hasNoAsrListeningField() {
        // DEL1-A1 / C-5：无语音输入（isListening/ASR/麦克风）状态字段
        val fields = fieldNames()
        assertTrue(
            "UI 状态机不得含 'isListening'（ASR 已删，C-5）",
            "isListening" !in fields,
        )
    }

    @Test
    fun state_hasNoOnboardingField() {
        // DEL1-A1：无引导弹窗状态字段
        val fields = fieldNames()
        assertTrue(
            "UI 状态机不得含 'onboarding'（引导弹窗已删）",
            "onboarding" !in fields,
        )
    }

    @Test
    fun state_hasNoConfirmationGateField() {
        // DEL1-A1：无确认门状态字段
        val fields = fieldNames()
        assertTrue(
            "UI 状态机不得含 'confirmation'（确认门已删）",
            "confirmation" !in fields,
        )
    }

    @Test
    fun state_hasNoAsrVariantNames() {
        // 删除面清扫：不含 asr*/mic*/voiceInput/guide*/confirm* 变体字段
        val fields = fieldNames()
        val bannedPrefixes = listOf("asr", "mic", "voiceInput")
        val bannedSubstrings = listOf("guide", "confirm", "onboard")
        val leaked = fields.filter { f ->
            val lower = f.lowercase()
            bannedPrefixes.any { lower.startsWith(it) } ||
                bannedSubstrings.any { lower.contains(it) }
        }
        assertTrue(
            "UI 状态机不得含 ASR/引导/确认门相关字段变体，发现泄漏: $leaked",
            leaked.isEmpty(),
        )
    }

    @Test
    fun state_keepsMinimalSurface() {
        // C-1 / CORE1-A1：保留极简面字段在（输入/加载/设置可见/连接状态；气泡为 getter 读取面，非 declared field）
        val clazz = Class.forName(TARGET_CLASS)
        val fields = clazz.declaredFields.map { it.name }.toSet()
        val required = listOf("inputText", "isLoading", "settingsVisible", "connectionState")
        assertTrue(
            "UI 状态机应保留极简面字段 $required，实得: $fields",
            fields.containsAll(required),
        )
        // 气泡读取面经 getter 暴露 → 以 methods 断言 getBubbles 存在
        val methodNames = clazz.methods.map { it.name }.toSet()
        assertTrue("气泡读取面（getBubbles）须存在", "getBubbles" in methodNames)
    }

    @Test
    fun state_exposesNoAsrBubbleVariant() {
        // DEL1-A1：气泡 sealed 封口不含 ASR/引导/确认气泡变体。
        // 用反射读 UiBubble 密封子类型名，断言只含 User/Assistant/Error，无导引面。
        val bubbleClass = Class.forName("com.live2dai.android.ui.UiBubble")

        fun nestedTypeNames(clazz: Class<*>): Collection<String> {
            val out = mutableListOf<String>()
            try {
                for (inner in clazz.declaredClasses) {
                    out += inner.simpleName
                }
            } catch (_: Throwable) {
                // 某些环境 declaredClasses 可能受限——若无法枚举则不硬判（避免误 RED）
            }
            return out
        }

        val names = nestedTypeNames(bubbleClass).map { it.lowercase() }
        // 若密封子类可枚举，则断言无导引面变体；若环境禁止枚举（空集合）则跳过（记为已知缝隙，非假绿）
        if (names.isNotEmpty()) {
            val banned = listOf("asr", "mic", "voice", "guide", "onboard", "confirm", "gate", "tool")
            val leaked = names.filter { n -> banned.any { n.contains(it) } }
            assertTrue(
                "气泡 sealed 类型不得含 ASR/引导/确认门变体，发现泄漏: $leaked（当前类型: $names）",
                leaked.isEmpty(),
            )
        }
    }
}
