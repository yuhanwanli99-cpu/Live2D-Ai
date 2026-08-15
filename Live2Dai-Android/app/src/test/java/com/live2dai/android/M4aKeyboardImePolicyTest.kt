package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * M4a 键盘视口固定（D6：adjustNothing + bottomBar imePadding）—— RED 测试（契约 M4-A1）。
 *
 * 依据 `.pi/spoq/contract-R1-ui.md` §2.1 / §3 M4-A1：
 *   - `<activity ... android:windowSoftInputMode="adjustNothing">`（MainActivity）
 *   - `windowSoftInputMode="adjustNothing"` **命中且唯一**
 *   - configChanges **现状保留**（orientation|screenSize|screenLayout|keyboardHidden）
 *
 * 本测试仅对 `AndroidManifest.xml` 做文本级 grep 断言（契约 §7 验收命令 `grep -n "adjustNothing"` 的
 * JVM 可运行形态）。**不读取、不依赖任何 Kotlin 实现（MainScreen.kt 等）**，符合「禁读 src/ 实现、
 * 禁对答案出题」约束。
 *
 * RED 依据（现状实测）：MainActivity 当前**无** windowSoftInputMode → 缺省 adjustResize →
 * 键盘弹出时 Scaffold bottomBar + Box(weight 1f) 缩水 → GLSurfaceView.onSurfaceChanged → 角色被压缩/裁切
 * （R1/M4a 用户可见缺陷）。
 * GREEN 条件：coder 为 MainActivity 添加 `android:windowSoftInputMode="adjustNothing"`（D6 拍板）。
 * 该 window policy 是「键盘弹出 → 窗口不缩放 → 模型区 Box(weight 1f) 尺寸不变 → 视口不压缩」的机制基石。
 *
 * 范围界定：契约 M4-A1 的「键盘弹出 → 模型区尺寸不变」与「输入行抬升到键盘上方」的**机器可断言形式**
 * 为真机 ui_dump E2E（契约 M4-A2，批次 4 联合验证；schedule §6/§7 波1 仅不含真机键盘三连）。故本类只产
 * M4-A1 的本地机械断言；键盘三连 E2E 由批次 4 处理（触发方式见交接报告 T3-R1-ui-red.md §4）。
 */
class M4aKeyboardImePolicyTest {

    private val manifestText: String by lazy {
        locateMainAndroidManifest().readText()
    }

    private fun locateMainAndroidManifest(): File {
        var dir: File? = File(System.getProperty("user.dir")).absoluteFile
        // 同时适配两种 CWD 布局：Gradle 单测工作目录（app 模块目录）与仓库根（IDE/脚本）。
        val candidates = listOf(
            "app/src/main/AndroidManifest.xml",
            "Live2Dai-Android/app/src/main/AndroidManifest.xml",
        )
        while (dir != null) {
            for (rel in candidates) {
                val candidate = File(dir, rel)
                if (candidate.isFile && candidate.readText().contains("MainActivity")) {
                    return candidate
                }
            }
            dir = dir.parentFile
        }
        throw AssertionError(
            "未定位到 AndroidManifest.xml（user.dir=" +
                System.getProperty("user.dir") + "）"
        )
    }

    // ────────────────────────────────────────────────────────────
    // RED：D6 视口固定机制未落地（当前 MainActivity 无 windowSoftInputMode）
    // ────────────────────────────────────────────────────────────

    @Test
    fun mainActivity_sets_windowSoftInputMode_adjustNothing() {
        val hasAdjustNothing =
            manifestText.contains("android:windowSoftInputMode=\"adjustNothing\"")
        assertTrue(
            "契约 M4-A1（D6）要求 MainActivity 声明 android:windowSoftInputMode=\"adjustNothing\"；" +
                "否则缺省 adjustResize 触发键盘弹出压缩模型区（当前 RED：未落地）。\nmanifest=" +
                manifestText,
            hasAdjustNothing
        )
    }

    @Test
    fun adjustNothing_occurrence_is_unique() {
        val count =
            Regex("windowSoftInputMode=\"adjustNothing\"").findAll(manifestText).count()
        assertEquals(
            "契约 M4-A1 要求 windowSoftInputMode=\"adjustNothing\" 命中且唯一（仅 MainActivity）；" +
                "当前=${count}（RED：未落地）。\nmanifest=" + manifestText,
            1,
            count
        )
    }

    // ────────────────────────────────────────────────────────────
    // GREEN 基线（回归锁）：M4a 只新增 windowSoftInputMode，不得移除 configChanges
    // ────────────────────────────────────────────────────────────

    @Test
    fun configChanges_value_preserved() {
        val hasConfigChanges =
            manifestText.contains(
                "android:configChanges=\"orientation|screenSize|screenLayout|keyboardHidden\""
            )
        assertTrue(
            "契约 M4-A1 要求 configChanges 现状保留（orientation|screenSize|screenLayout|keyboardHidden）；" +
                "M4a 只能新增 windowSoftInputMode，不得移除 configChanges。\nmanifest=" + manifestText,
            hasConfigChanges
        )
    }
}
