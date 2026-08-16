package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.Buffer
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.ByteArrayOutputStream
import java.io.File
import java.security.MessageDigest
import java.util.zip.GZIPOutputStream

/**
 * ModelDownloadManager（V1.2 模块3/4）单元测试。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → functions.ModelDownloadManager.* /
 * dataModels.DownloadState。plan §9 教训四件事全覆盖验证：
 * - 进度通知：状态流 Idle→Downloading→Verifying→Completed|Failed（终态断言）
 * - 断点续传：.part + Range 头（206 续传 / 200 重头 / 校验失败删 .part）
 * - SHA256 校验：匹配完成 / 不匹配失败
 * - 存储空间预检：可注入 SpaceChecker，空间不足快速失败（不发 HTTP）
 * 另含模型级 download(spec)：tar.bz2 下载 + 解压 + 就绪判定。
 *
 * 用 MockWebServer 模拟服务器（本地上传/下载，不触外网）。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ModelDownloadManagerTest {

    private lateinit var context: Context
    private lateinit var server: MockWebServer
    private lateinit var manager: ModelDownloadManager

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        server = MockWebServer()
        server.start()
        manager = ModelDownloadManager(
            context,
            client = OkHttpClient(),
            spaceChecker = SpaceChecker { Long.MAX_VALUE }, // 默认无限空间
        )
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    // ─────────────────────────────────────────────
    // SHA256 校验
    // ─────────────────────────────────────────────

    @Test
    fun download_completesAndVerifiesSha256() = runBlocking {
        val content = "hello sherpa onnx".toByteArray()
        server.enqueue(MockResponse().setBody(Buffer().write(content)))
        val dest = tempFile("ok.bin")

        val flow = manager.download(server.url("/model.bin").toString(), dest.absolutePath, sha256(content))

        val state = flow.value
        assertTrue("应完成: $state", state is DownloadState.Completed)
        assertEquals(dest.absolutePath, (state as DownloadState.Completed).path)
        assertTrue("目标文件应存在且内容一致", dest.readBytes().contentEquals(content))
        assertFalse("校验通过后不应残留 .part", File(dest.absolutePath + ".part").exists())
    }

    @Test
    fun download_sha256MismatchFailsAndDeletesPartial() = runBlocking {
        val content = "corrupted data".toByteArray()
        server.enqueue(MockResponse().setBody(Buffer().write(content)))
        val dest = tempFile("bad.bin")

        val flow = manager.download(
            server.url("/bad.bin").toString(), dest.absolutePath,
            "0000000000000000000000000000000000000000000000000000000000000000" // 错误的期望值
        )

        val state = flow.value
        assertTrue("应失败: $state", state is DownloadState.Failed)
        assertTrue("失败信息应含 SHA256: $state", (state as DownloadState.Failed).error.contains("SHA256"))
        assertFalse("校验失败应删除 .part（损坏数据不能续传）", File(dest.absolutePath + ".part").exists())
        assertFalse("不应生成目标文件", dest.exists())
    }

    @Test
    fun download_alreadyVerifiedFileCompletesWithoutRequest() = runBlocking {
        // 已下载且校验通过 → 幂等重入：直接 Completed，不发 HTTP
        val content = "already there".toByteArray()
        val dest = tempFile("cached.bin")
        dest.writeBytes(content)

        val flow = manager.download(server.url("/cached.bin").toString(), dest.absolutePath, sha256(content))

        assertTrue(flow.value is DownloadState.Completed)
        assertEquals("已缓存文件不应再发请求", 0, server.requestCount)
    }

    // ─────────────────────────────────────────────
    // 断点续传（Range + .part）
    // ─────────────────────────────────────────────

    @Test
    fun download_resumesFromPartialWithRangeHeader() = runBlocking {
        val full = ByteArray(4096) { (it % 251).toByte() }
        val half = 2048
        val dest = tempFile("resume.bin")
        // 预置 .part（前一半）
        File(dest.absolutePath + ".part").writeBytes(full.copyOfRange(0, half))
        // 服务器支持 Range：返回 206 + 剩余部分
        server.enqueue(
            MockResponse()
                .setResponseCode(206)
                .setHeader("Content-Range", "bytes $half-${full.size - 1}/${full.size}")
                .setBody(Buffer().write(full.copyOfRange(half, full.size)))
        )

        val flow = manager.download(server.url("/resume.bin").toString(), dest.absolutePath, sha256(full))

        assertTrue("续传应完成: ${flow.value}", flow.value is DownloadState.Completed)
        assertTrue("续传后文件内容应完整", dest.readBytes().contentEquals(full))
        // 服务器应收到 Range 请求（断点续传核心证据）
        val request = server.takeRequest()
        assertEquals("bytes=$half-", request.getHeader("Range"))
    }

    @Test
    fun download_restartsWhenServerIgnoresRange() = runBlocking {
        val full = "server ignores range, send full".toByteArray()
        val dest = tempFile("no-range.bin")
        File(dest.absolutePath + ".part").writeBytes(ByteArray(10)) // 残留部分文件
        // 服务器忽略 Range：返回 200 + 完整内容
        server.enqueue(MockResponse().setBody(Buffer().write(full)))

        val flow = manager.download(server.url("/no-range.bin").toString(), dest.absolutePath, sha256(full))

        assertTrue("应重头完成: ${flow.value}", flow.value is DownloadState.Completed)
        assertTrue("重头下载后内容应完整", dest.readBytes().contentEquals(full))
        // 客户端确实发了 Range（服务端选择忽略）
        assertEquals("bytes=10-", server.takeRequest().getHeader("Range"))
    }

    // ─────────────────────────────────────────────
    // 容错
    // ─────────────────────────────────────────────

    @Test
    fun download_httpErrorFails() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(500))
        val dest = tempFile("http500.bin")

        val flow = manager.download(server.url("/boom.bin").toString(), dest.absolutePath, null)

        val state = flow.value
        assertTrue("HTTP 错误应失败: $state", state is DownloadState.Failed)
        assertTrue((state as DownloadState.Failed).error.contains("500"))
        // 失败保留 .part 供下次续传（resume 语义）
        assertFalse("失败后不应有目标文件", dest.exists())
    }

    // ─────────────────────────────────────────────
    // 存储空间预检
    // ─────────────────────────────────────────────

    @Test
    fun download_spec_insufficientSpaceFailsBeforeHttp() = runBlocking {
        val noSpace = ModelDownloadManager(
            context,
            client = OkHttpClient(),
            spaceChecker = SpaceChecker { 0L }, // 无空间
        )
        val spec = testSpec(url = server.url("/m.bin").toString())

        val flow = noSpace.download(spec)

        val state = flow.value
        assertTrue("空间不足应失败: $state", state is DownloadState.Failed)
        assertTrue((state as DownloadState.Failed).error.contains("存储空间不足"))
        assertEquals("空间不足不应发起 HTTP 请求", 0, server.requestCount)
    }

    // ─────────────────────────────────────────────
    // 模型级下载：tar.bz2 下载 + 解压 + 就绪判定
    // ─────────────────────────────────────────────

    @Test
    fun download_spec_extractsTarAndCompletes() = runBlocking {
        // 构造 tar.bz2：顶层目录 {kokoro-test}/model.onnx + tokens.txt
        val tarBytes = TarBuilder.builder()
            .file("kokoro-test/model.onnx", byteArrayOf(1, 2, 3, 4))
            .file("kokoro-test/tokens.txt", "test-token\n".toByteArray())
            .buildGzip()
        server.enqueue(MockResponse().setBody(Buffer().write(tarBytes)))
        val spec = testSpec(url = server.url("/m.tar.bz2").toString())

        val flow = manager.download(spec)

        val state = flow.value
        assertTrue("模型下载应完成: $state", state is DownloadState.Completed)
        val dir = manager.modelDir(spec)
        assertTrue("应解压出 model.onnx", File(dir, "model.onnx").exists())
        assertTrue("应解压出 tokens.txt", File(dir, "tokens.txt").exists())
        assertEquals("顶层目录应被剥离", byteArrayOf(1, 2, 3, 4).toList(), File(dir, "model.onnx").readBytes().toList())
        assertTrue("isDownloaded 应判定为就绪", manager.isDownloaded(spec))
        assertEquals("resolvedModelFile 应指向 model.onnx", File(dir, "model.onnx"), manager.resolvedModelFile(spec))
        assertFalse("tar 包解压后应删除", File(dir, spec.fileName).exists())
    }

    @Test
    fun modelDirAndIsDownloaded_reflectModelFiles() {
        val spec = testSpec(url = "http://unused/")
        val dir = manager.modelDir(spec)
        assertFalse("初始不应就绪", manager.isDownloaded(spec))
        assertNull("无模型文件时 resolvedModelFile 应为 null", manager.resolvedModelFile(spec))

        // int8 变体文件名优先（model.int8.onnx）
        File(dir, "model.int8.onnx").writeText("x")
        File(dir, "tokens.txt").writeText("t")
        assertTrue("模型文件齐备应就绪", manager.isDownloaded(spec))
        assertEquals(File(dir, "model.int8.onnx"), manager.resolvedModelFile(spec))
    }

    // ─────────────────────────────────────────────
    // V1.2 模块5：UI 数据源（离线模型选择区遍历 CATALOG）
    // ─────────────────────────────────────────────

    @Test
    fun catalog_containsKokoroAndSenseVoiceForUi() {
        // K1 单引擎（Wave 5 清理）：CATALOG 仅含 Kokoro TTS（SenseVoice ASR 已随 ASR 链删除）
        assertEquals("CATALOG 应含一个模型（K1 单引擎）", 1, ModelDownloadManager.CATALOG.size)
        val ids = ModelDownloadManager.CATALOG.map { it.id }.toSet()
        assertEquals(
            setOf("kokoro_tts"),
            ids
        )
        val kokoro = ModelDownloadManager.CATALOG.first { it.id == "kokoro_tts" }
        assertTrue("Kokoro 模型文件候选应含 onnx", kokoro.modelFileCandidates.any { it.endsWith(".onnx") })
        assertTrue("Kokoro 需伴生 tokens.txt", kokoro.requiredCompanions.contains("tokens.txt"))
    }

    // ─────────────────────────────────────────────
    // 工具
    // ─────────────────────────────────────────────

    private fun tempFile(name: String): File = File(context.cacheDir, "mdt_$name").apply { delete() }

    private fun sha256(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    private fun testSpec(url: String = "http://unused/") = ModelSpec(
        id = "test_model_${System.nanoTime()}",
        name = "测试模型",
        fileName = "test-model.tar.bz2",
        url = url,
        expectedSha256 = null,
        sizeBytes = 1024 * 1024,
        modelFileCandidates = listOf("model.int8.onnx", "model.onnx"),
        requiredCompanions = listOf("tokens.txt"),
    )
}

/**
 * 最小 ustar tar 写入器（测试夹具）：写 header + data + 512 对齐 + 双零块，可选 gzip。
 * 与生产 [TarBz2] 解析器配套（仅验证解析路径，不校验 checksum/magic）。
 */
private class TarBuilder {
    private val entries = mutableListOf<Pair<String, ByteArray>>()

    fun file(name: String, data: ByteArray): TarBuilder = apply { entries += name to data }

    fun buildGzip(): ByteArray {
        val raw = build()
        val out = ByteArrayOutputStream()
        GZIPOutputStream(out).use { it.write(raw) }
        return out.toByteArray()
    }

    private fun build(): ByteArray {
        val out = ByteArrayOutputStream()
        for ((name, data) in entries) {
            out.write(header(name, data.size.toLong()))
            out.write(data)
            val padding = (512 - data.size % 512) % 512
            out.write(ByteArray(padding))
        }
        out.write(ByteArray(1024)) // 两个零块
        return out.toByteArray()
    }

    private fun header(name: String, size: Long): ByteArray {
        val h = ByteArray(512)
        name.toByteArray(Charsets.US_ASCII).copyInto(h, 0, 0, minOf(100, name.length))
        octal(size, 12).copyInto(h, 124)
        h[156] = '0'.code.toByte() // 普通文件
        "ustar".toByteArray().copyInto(h, 257)
        h[263] = '0'.code.toByte() // 版本
        h[264] = '0'.code.toByte()
        // checksum：全部字节求和（checksum 字段本身按空格计）
        for (i in 148 until 156) h[i] = ' '.code.toByte()
        val sum = h.sumOf { it.toInt() and 0xFF }
        "%06o\u0000 ".format(sum).toByteArray(Charsets.US_ASCII).copyInto(h, 148)
        return h
    }

    private fun octal(value: Long, width: Int): ByteArray {
        val s = "%0${width - 1}o".format(value) + "\u0000"
        return s.toByteArray(Charsets.US_ASCII)
    }

    companion object {
        fun builder() = TarBuilder()
    }
}
