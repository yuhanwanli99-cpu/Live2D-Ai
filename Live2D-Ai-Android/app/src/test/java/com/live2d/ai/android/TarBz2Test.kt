package com.live2d.ai.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.GZIPOutputStream

/**
 * TarBz2 最小 ustar 解压器（V1.2 模块3/4：模型 .tar.bz2 分发）单元测试。
 *
 * 覆盖：普通文件 + 顶层目录剥离、目录条目跳过、GNU 长文件名（'L'）、
 * 路径穿越防护、gzip 解压。
 */
class TarBz2Test {

    @Test
    fun extract_stripsTopLevelAndWritesFiles() {
        val tar = buildTar(
            "kokoro-test/model.onnx" to byteArrayOf(1, 2, 3, 4),
            "kokoro-test/tokens.txt" to "t1\nt2\n".toByteArray(),
        )
        val destDir = tempDir("tar_extract")
        TarBz2.extract(File(destDir, "m.tar.bz2").apply { writeBytes(tar) }, destDir)

        assertArrayEquals("顶层目录应被剥离", byteArrayOf(1, 2, 3, 4), File(destDir, "model.onnx").readBytes())
        assertEquals("t1\nt2\n", File(destDir, "tokens.txt").readText())
        assertTrue("不应残留顶层目录", !File(destDir, "kokoro-test").exists())
    }

    @Test
    fun extract_skipsDirectoryEntries() {
        val tar = buildTar(
            "pkg/sub/" to null, // 目录条目
            "pkg/model.onnx" to byteArrayOf(9),
        )
        val destDir = tempDir("tar_dir")
        TarBz2.extract(File(destDir, "m.tar.bz2").apply { writeBytes(tar) }, destDir)

        assertEquals(byteArrayOf(9).toList(), File(destDir, "model.onnx").readBytes().toList())
    }

    @Test
    fun extract_supportsGnuLongNames() {
        // GNU 长文件名：'L' 条目（>100 字符路径），真实条目用占位短名
        val longName = "a-very-long-directory-name-that-exceeds-one-hundred-characters-for-testing-purposes-only-ok/model.onnx"
        assertTrue(longName.length > 100)
        val tar = buildGnuLongNameTar(longName, byteArrayOf(7))
        val destDir = tempDir("tar_long")
        TarBz2.extract(File(destDir, "m.tar.bz2").apply { writeBytes(tar) }, destDir)

        assertEquals(
            "长文件名条目应解压（顶层目录剥离后落在 destDir 根）",
            byteArrayOf(7).toList(),
            File(destDir, "model.onnx").readBytes().toList()
        )
    }

    @Test
    fun extract_rejectsPathTraversal() {
        // ".." 出现在剥离前的原始路径中必须被拒绝（不被顶层目录剥离掩盖）
        val tar = buildTar("../evil.txt" to byteArrayOf(1))
        val destDir = tempDir("tar_traversal")
        val file = File(destDir, "m.tar.bz2").apply { writeBytes(tar) }

        val exception = assertThrows(IllegalArgumentException::class.java) { TarBz2.extract(file, destDir) }
        assertTrue("应拒绝路径穿越: ${exception.message}", exception.message!!.contains("路径穿越"))
        assertTrue("不应写入目标目录之外", !File(destDir.parentFile, "evil.txt").exists())
    }

    // ─────────────────────────────────────────────
    // 测试工具：最小 ustar tar 写入器
    // ─────────────────────────────────────────────

    /** 构造 GNU 长文件名 tar：'L' 条目 + 真实条目（占位名）。 */
    private fun buildGnuLongNameTar(longName: String, data: ByteArray): ByteArray {
        val raw = ByteArrayOutputStream()
        // 'L' 条目：name 为空、typeflag='L'、size=路径长度、数据为完整路径
        raw.write(header("", longName.length.toLong(), typeFlag = 'L'))
        raw.write(longName.toByteArray(Charsets.US_ASCII))
        raw.write(ByteArray((512 - longName.length % 512) % 512))
        // 真实条目：占位短名（<100 字符）
        raw.write(header("placeholder", data.size.toLong(), typeFlag = '0'))
        raw.write(data)
        raw.write(ByteArray((512 - data.size % 512) % 512))
        raw.write(ByteArray(1024))
        val out = ByteArrayOutputStream()
        GZIPOutputStream(out).use { it.write(raw.toByteArray()) }
        return out.toByteArray()
    }

    private fun buildTar(vararg entries: Pair<String, ByteArray?>): ByteArray {
        val raw = ByteArrayOutputStream()
        for ((name, data) in entries) {
            if (data == null) {
                raw.write(header(name, 0L, typeFlag = '5'))
            } else {
                raw.write(header(name, data.size.toLong(), typeFlag = '0'))
                raw.write(data)
                raw.write(ByteArray((512 - data.size % 512) % 512))
            }
        }
        raw.write(ByteArray(1024))
        val out = ByteArrayOutputStream()
        GZIPOutputStream(out).use { it.write(raw.toByteArray()) }
        return out.toByteArray()
    }

    private fun header(name: String, size: Long, typeFlag: Char): ByteArray {
        val h = ByteArray(512)
        name.toByteArray(Charsets.US_ASCII).copyInto(h, 0, 0, minOf(100, name.length))
        ("%011o".format(size) + "\u0000").toByteArray(Charsets.US_ASCII).copyInto(h, 124)
        h[156] = typeFlag.code.toByte()
        return h
    }

    private fun tempDir(name: String): File {
        val dir = File(System.getProperty("java.io.tmpdir"), "tarbz2_$name")
        dir.deleteRecursively()
        dir.mkdirs()
        return dir
    }
}
