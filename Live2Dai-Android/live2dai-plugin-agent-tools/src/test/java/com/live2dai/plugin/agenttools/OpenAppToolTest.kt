package com.live2dai.plugin.agenttools

import android.content.Intent
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

/**
 * [OpenAppTool] 测试：
 * - 纯逻辑（参数解析 + 应用名→包名匹配）不依赖 Android，直接单测；
 * - execute 三个分支（成功 / 未找到应用 / 无启动入口）通过 [AppResolver] 接缝注入内存数据，
 *   避免单元测试依赖真实 PackageManager（设计文档 R5）。
 *
 * 整个类跑在 Robolectric 下（execute 分支会构造真实 [Intent]）。
 */
@RunWith(RobolectricTestRunner::class)
class OpenAppToolTest {

    private val fakeApps = listOf(
        "微信" to "com.tencent.mm",
        "企业微信" to "com.tencent.wework",
        "支付宝" to "com.eg.android.AlipayGphone",
        "WeChat" to "com.example.wechat",
    )

    // ── 纯逻辑：参数解析 ──

    @Test
    fun parseAppName_returnsNameFromValidArgs() {
        assertEquals("微信", parseAppName("""{"app_name":"微信"}"""))
        assertEquals("微信", parseAppName("""{"app_name":"  微信  "}"""))
    }

    @Test
    fun parseAppName_returnsNullForInvalidArgs() {
        assertNull(parseAppName("not-json"))
        assertNull(parseAppName("{}"))
        assertNull(parseAppName("""{"app_name":123}"""))
        assertNull(parseAppName("""{"app_name":"   "}"""))
        assertNull(parseAppName("""{"other":"微信"}"""))
    }

    // ── 纯逻辑：应用名 → 包名匹配 ──

    @Test
    fun resolvePackageByLabel_exactMatchFirst() {
        assertEquals("com.tencent.mm", resolvePackageByLabel("微信", fakeApps))
        assertEquals("com.tencent.mm", resolvePackageByLabel(" 微信 ", fakeApps))
    }

    @Test
    fun resolvePackageByLabel_ignoreCaseMatch() {
        assertEquals("com.example.wechat", resolvePackageByLabel("wechat", fakeApps))
        assertEquals("com.example.wechat", resolvePackageByLabel("WECHAT", fakeApps))
    }

    @Test
    fun resolvePackageByLabel_containsMatch() {
        // label 包含目标名：目标「企业」命中「企业微信」
        assertEquals("com.tencent.wework", resolvePackageByLabel("企业", fakeApps))
        // 目标名包含 label：目标「微信App」命中「微信」
        assertEquals("com.tencent.mm", resolvePackageByLabel("微信App", fakeApps))
    }

    @Test
    fun resolvePackageByLabel_returnsNullWhenNotFound() {
        assertNull(resolvePackageByLabel("不存在的应用", fakeApps))
        assertNull(resolvePackageByLabel("", fakeApps))
        assertNull(resolvePackageByLabel("   ", fakeApps))
        assertNull(resolvePackageByLabel("微信", emptyList()))
    }

    // ── execute 分支（AppResolver 接缝） ──

    private fun fakeResolver(
        apps: List<Pair<String, String>> = fakeApps,
        launchable: Set<String> = setOf("com.tencent.mm"),
    ) = object : AppResolver {
        val launched = mutableListOf<Intent>()
        override fun installedApps(): List<Pair<String, String>> = apps
        override fun launchIntentFor(packageName: String): Intent? =
            if (packageName in launchable) Intent(Intent.ACTION_MAIN).setPackage(packageName) else null
        override fun startActivity(intent: Intent) { launched += intent }
    }

    @Test
    fun execute_opensAppSuccessfully() = runBlocking {
        val resolver = fakeResolver()
        val tool = OpenAppTool(resolver)

        val result = tool.execute("""{"app_name":"微信"}""")

        assertTrue("成功结果应包含匹配到的应用名（R3 便于纠正歧义）", result.contains("已打开 微信"))
        assertEquals(1, resolver.launched.size)
        assertEquals("com.tencent.mm", resolver.launched.single().`package`)
    }

    @Test
    fun execute_returnsErrorWhenAppNotFound() = runBlocking {
        val resolver = fakeResolver(apps = emptyList())
        val tool = OpenAppTool(resolver)

        val result = tool.execute("""{"app_name":"微信"}""")

        assertTrue(result.contains("未找到名为"))
        assertEquals(0, resolver.launched.size)
    }

    @Test
    fun execute_returnsErrorWhenNoLaunchEntry() = runBlocking {
        val resolver = fakeResolver(launchable = emptySet())
        val tool = OpenAppTool(resolver)

        val result = tool.execute("""{"app_name":"微信"}""")

        assertTrue(result.contains("没有可启动的入口"))
        assertEquals(0, resolver.launched.size)
    }

    @Test
    fun execute_returnsErrorWhenAppNameMissing() = runBlocking {
        val tool = OpenAppTool(fakeResolver())

        val result = tool.execute("{}")

        assertTrue(result.contains("缺少 app_name"))
    }

    // ── 元数据 ──

    @Test
    fun metadata_nameSchemaAndConfirmationFlag() {
        val tool = OpenAppTool(fakeResolver())

        assertEquals("open_app", tool.name)
        assertFalse("仅切前台，无需确认", tool.requiresConfirmation)

        // parametersSchema 必须是合法 JSON Schema（验收标准 #2）
        val schema = Json.parseToJsonElement(tool.parametersSchema).jsonObject
        assertTrue(schema.containsKey("properties"))
        assertTrue(schema["required"] != null)
    }
}
