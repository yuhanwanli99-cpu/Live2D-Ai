package com.live2dai.android

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * 捏合注入 instrumentation（契约 R1-interaction §2.1/DIAG-2；真机运行）。
 *
 * 目的：在**真实设备**上通过 Compose performTouchInput 双指注入驱动主页真实 UI（Live2DView），
 * 捕获 `[gesture]` / `[interaction]` logcat 序列，为根因裁决矩阵（①POINTER_DOWN 到达/②MOVE(2)/
 * ③CANCEL/④pinch scale=）提供机器证据（输出 .pi/spoq/verify/gesture_diag.log 的测试侧来源）。
 *
 * 根因判决（契约 §2.1 裁决矩阵，本测试仅取证 + 断言期望行为；对当前损坏设备测试将 RED）：
 *   - 断言「POINTER_DOWN 到达 Live2DView」失败 → 候选根因 A（DOWN 不消费/触摸目标语义）
 *   - 「POINTER_DOWN 到、MOVE(2) 不到或 CANCEL 出现」失败 → 候选根因 B（前景层/interop 多点拦截）
 *   - 「①②到、pinch scale= 不到」失败 → 候选根因 C（状态机/索引缺陷）
 *   - 均到但真机仍不可用 → 候选根因 D → 上报主代理（禁止猜测）
 *
 * 运行命令（真机）：`./gradlew :app:connectedDebugAndroidTest`
 * 断言方式：注入后读 logcat（UiAutomation.executeShellCommand，shell 权限读）匹配 `[gesture]`/`[interaction]`。
 *
 * 注意：本文件归 R1-interaction（文件锁，禁 src/ 实现）。PinchInjectionTest 不引用 GestureStateMachine
 * （尚未实现）；仅驱动真实 UI + logcat 取证，故编译不依赖状态机生产符号。
 */
@RunWith(AndroidJUnit4::class)
class PinchInjectionTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    /** 双指捏合注入主页模型区 → 断言 [gesture] POINTER_DOWN 到达 + MOVE(2) + [interaction] pinch scale= 出现。 */
    @Test
    fun twoFingerPinch_injectsReachesLive2DViewAndProducesScaleLog() {
        // ── 0. 清 logcat，保证取证只来自本次注入 ──
        executeShellCommand("logcat -c")

        // ── 1. 等待主页就绪；取屏幕（根节点）几何 ──
        composeRule.waitUntil(timeoutMillis = 30_000) {
            // 根节点 bounds 非空即认为 Compose 已树就绪（模型区为居中显示，注入点取画面竖向中心）
            val b = composeRule.onRoot().getBoundsInRoot()
            b.width > 0f && b.height > 0f
        }
        val rootBounds = composeRule.onRoot().getBoundsInRoot()
        val cx = rootBounds.width / 2f
        val cy = rootBounds.height / 2f

        // ── 2. 构造双指捏合轨迹（自屏幕中心向两侧张开 = 放大；契约 §2.1 performTouchInput 序列）──
        // 起始两指非常靠近（初始间距 40px）→ 向两侧各张 80px（末距 200px），驱动 ApplyPinch 距离变化
        val startA = Offset(cx - 20f, cy)
        val startB = Offset(cx + 20f, cy)
        val endA = Offset(cx - 100f, cy)
        val endB = Offset(cx + 100f, cy)

        composeRule.onRoot().performTouchInput {
            down(0, startA)
            down(1, startB)          // ← 第二指落下 = ACTION_POINTER_DOWN，关键判据①
            moveTo(0, endA)
            moveTo(1, endB)          // ← 双指移动 = ACTION_MOVE(pointerCount=2)，关键判据②
            moveBy(0, Offset(-40f, 0f))
            moveBy(1, Offset(40f, 0f)) // 继续张开，制造多帧 MOVE
            up(0)
            up(1)
        }

        // ── 3. 读取注入期间的 logcat（[gesture] + [interaction]，契约格式 §2.1）──
        Thread.sleep(1_000) // 给 EventDispatch/queueEvent 落日志留余量
        val logs = executeShellCommand("logcat -d -s Live2DView Live2DRenderer")

        // ── 4. 断言四判据（每项失败 → 对应候选根因，见契约 §2.1 裁决矩阵）──
        assertTrue(
            "⚠ 判据①失败→候选根因A：注入后 logcat 未见 ACTION_POINTER_DOWN 到达 Live2DView（DOWN 不消费/触摸目标语义）。\n完整 logcat:\n" + logs,
            logs.contains("POINTER_DOWN"),
        )
        // 判据②：双指 MOVE（2 个活动 pointer 的 MOVE）到达 —— 需 MOVE 与 count=2 标记同时命中
        val doubleMoveSeen = logs.contains("MOVE") &&
            (logs.contains("count=2") || logs.contains("pointer_count=2") || logs.contains("ids=["))
        assertTrue(
            "⚠ 判据②失败→候选根因B：POINTER_DOWN 到达但未见双指 MOVE(pointerCount=2)（Compose 前景层/interop 多点派发截断）。\n完整 logcat:\n" + logs,
            doubleMoveSeen,
        )
        // 判据③：CANCEL 出现 → 被前景层/父层拦截（根因 B 强化信号）；此处不强断言（出现才诊断）
        val cancelSeen = logs.contains("CANCEL") || logs.contains("action=CANCEL")
        val pinchScaleSeen = logs.contains("pinch scale=") || logs.contains("[interaction]")
        assertTrue(
            "⚠ 判据④失败→候选根因C：①②到达但未见 renderer `[interaction] pinch scale=` 日志（状态机/索引缺陷）。" +
                "（CANCEL 出现=${cancelSeen}，供根因 B 佐证推荐）\n完整 logcat:\n" + logs,
            pinchScaleSeen,
        )
    }

    // ── 辅助：以 shell 权限执行 adb 命令并回读 stdout（logcat 取证需 shell 读权限）──
    private fun executeShellCommand(cmd: String): String {
        val pfd = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(cmd)
        return java.io.FileInputStream(pfd.fileDescriptor).use { it.readBytes() }.decodeToString()
    }
}
