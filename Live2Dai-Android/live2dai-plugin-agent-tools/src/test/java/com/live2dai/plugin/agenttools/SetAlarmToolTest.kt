package com.live2dai.plugin.agenttools

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.ResolveInfo
import android.provider.AlarmClock
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows

/**
 * [SetAlarmTool] 测试：
 * - 纯逻辑（参数解析 + Intent 构造）不依赖 Android 环境；
 * - execute 的 resolveActivity / startActivity 路径用 Robolectric
 *   （`ShadowPackageManager.addResolveInfoForIntent` 控制是否存在可处理闹钟的 App）。
 */
@RunWith(RobolectricTestRunner::class)
class SetAlarmToolTest {

    // ── 纯逻辑：参数解析 ──

    @Test
    fun parseSetAlarmArgs_valid() {
        assertEquals(Triple(7, 30, null), parseSetAlarmArgs("""{"hour":7,"minute":30}"""))
        assertEquals(Triple(7, 30, "起床"), parseSetAlarmArgs("""{"hour":7,"minute":30,"label":"起床"}"""))
        assertEquals("label 会 trim 后返回", Triple(7, 30, "起床"), parseSetAlarmArgs("""{"hour":7,"minute":30,"label":"  起床  "}"""))
    }

    @Test
    fun parseSetAlarmArgs_boundaryValues() {
        assertEquals(Triple(0, 0, null), parseSetAlarmArgs("""{"hour":0,"minute":0}"""))
        assertEquals(Triple(23, 59, null), parseSetAlarmArgs("""{"hour":23,"minute":59}"""))
    }

    @Test
    fun parseSetAlarmArgs_invalidReturnsNull() {
        assertNull("hour 越界", parseSetAlarmArgs("""{"hour":25,"minute":30}"""))
        assertNull("minute 越界", parseSetAlarmArgs("""{"hour":7,"minute":60}"""))
        assertNull("hour 为负", parseSetAlarmArgs("""{"hour":-1,"minute":30}"""))
        assertNull("缺 minute", parseSetAlarmArgs("""{"hour":7}"""))
        assertNull("缺 hour", parseSetAlarmArgs("""{"minute":30}"""))
        assertNull("空对象", parseSetAlarmArgs("{}"))
        assertNull("非法 JSON", parseSetAlarmArgs("not-json"))
        assertNull("非数字 hour", parseSetAlarmArgs("""{"hour":"七","minute":30}"""))
    }

    // ── 纯逻辑：Intent 构造 ──

    @Test
    fun buildSetAlarmIntent_carriesExpectedExtras() {
        val intent = buildSetAlarmIntent(7, 30, "起床")

        assertEquals(AlarmClock.ACTION_SET_ALARM, intent.action)
        assertEquals(7, intent.getIntExtra(AlarmClock.EXTRA_HOUR, -1))
        assertEquals(30, intent.getIntExtra(AlarmClock.EXTRA_MINUTES, -1))
        assertEquals("起床", intent.getStringExtra(AlarmClock.EXTRA_MESSAGE))
        assertTrue(intent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
    }

    @Test
    fun buildSetAlarmIntent_omitsLabelWhenNull() {
        val intent = buildSetAlarmIntent(7, 30, null)
        assertNull(intent.getStringExtra(AlarmClock.EXTRA_MESSAGE))
    }

    // ── 元数据 ──

    @Test
    fun metadata_nameSchemaAndConfirmationFlag() {
        val tool = SetAlarmTool(ApplicationProvider.getApplicationContext())

        assertEquals("set_alarm", tool.name)
        assertFalse("系统闹钟页天然二次确认，无需插件层确认", tool.requiresConfirmation)

        val schema = Json.parseToJsonElement(tool.parametersSchema).jsonObject
        assertTrue(schema.containsKey("properties"))
    }

    // ── execute：resolveActivity / startActivity 路径（Robolectric） ──

    @Test
    fun execute_returnsErrorWhenNoAlarmAppOnDevice() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val tool = SetAlarmTool(context)

        val result = tool.execute("""{"hour":7,"minute":30}""")

        assertTrue("无闹钟 App 时应返回明确错误而非崩溃", result.contains("没有可处理闹钟设置的应用"))
    }

    @Test
    fun execute_launchesAlarmIntentWhenResolvable() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        // 注册一个可处理 ACTION_SET_ALARM 的闹钟 App
        val resolveInfo = ResolveInfo().apply {
            activityInfo = ActivityInfo().apply {
                packageName = "com.android.deskclock"
                name = "com.android.deskclock.AlarmClock"
            }
        }
        val alarmIntent = buildSetAlarmIntent(7, 30, null)
        // Robolectric 4.14 已废弃 resolveInfoForIntent 机制，但它是 4.14 中唯一能 stub 隐式
        // Intent 解析的方式（addOrUpdateActivity+intentFilter / addActivityIfNotPresent 实测均不生效），
        // 仅测试用，加 Suppress 抑制废弃告警。
        @Suppress("DEPRECATION")
        Shadows.shadowOf(context.packageManager).addResolveInfoForIntent(alarmIntent, resolveInfo)

        val tool = SetAlarmTool(context)
        val result = tool.execute("""{"hour":7,"minute":30,"label":"起床"}""")

        assertTrue("成功应返回跳转结果", result.contains("已跳转到闹钟设置 07:30"))
    }

    @Test
    fun execute_returnsErrorForInvalidTimeInsteadOfCrashing() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val tool = SetAlarmTool(context)

        val result = tool.execute("""{"hour":25,"minute":30}""")

        assertTrue(result.contains("参数不合法"))
    }
}
