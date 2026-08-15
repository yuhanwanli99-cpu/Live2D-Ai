package com.live2dai.android

import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.util.zip.GZIPInputStream

/**
 * 最小 tar.bz2 解压器（V1.2 模块3/4：sherpa-onnx 模型以 .tar.bz2 分发）。
 *
 * 仅实现 ustar 子集（sherpa-onnx 模型资产所需）：
 * - 普通文件（typeflag '0' / '\0'）与目录（'5'）
 * - GNU 长文件名（typeflag 'L'）
 * - 顶层目录剥离：资产形如 `{top-dir}/model.onnx`，解压时去掉第一段
 * - 路径穿越防护：拒绝绝对路径 / ".." 段
 *
 * 不做权限/符号链接处理（模型文件不需要）。整体文件校验由
 * [ModelDownloadManager] 的 SHA256 负责（下载校验在解压之前）。
 */
internal object TarBz2 {

    private const val TAG = "ModelDownload"

    private const val BLOCK_SIZE = 512
    private const val HEADER_END = 2 * BLOCK_SIZE // 连续两个空块 = 归档结束

    /**
     * 解压 tar.bz2 到 [destDir]（顶层目录自动剥离，见类注释）。
     * @throws IllegalArgumentException 归档损坏 / 路径穿越 / 解压失败
     */
    fun extract(tarBz2: File, destDir: File) {
        if (!destDir.exists() && !destDir.mkdirs()) {
            throw IllegalArgumentException("无法创建解压目录: ${destDir.absolutePath}")
        }
        GZIPInputStream(tarBz2.inputStream()).use { input ->
            var pendingName: String? = null
            while (true) {
                val header = readFully(input, BLOCK_SIZE) ?: break
                if (isEndBlock(header)) break

                val name = parseString(header, 0, 100)
                val size = parseOctal(header, 124, 12)
                val typeFlag = header[156].toInt().toChar()

                when (typeFlag) {
                    'L' -> {
                        // GNU 长文件名：下一条目的数据就是完整路径
                        val data = readFully(input, size.toInt())
                            ?: throw IllegalArgumentException("归档损坏：长文件名截断")
                        pendingName = parseString(data, 0, data.size)
                        skipPadding(input, size)
                    }
                    '5' -> { /* 目录：无需创建（文件写入时父目录自动创建） */ }
                    '0', '\u0000' -> {
                        val pending = pendingName?.takeIf { it.isNotEmpty() }
                        val rawName = pending ?: name
                        pendingName = null
                        val relative = stripTopLevel(rawName)
                        val target = safeResolve(destDir, rawName, relative)
                        target.parentFile?.mkdirs()
                        if (size > 0) {
                            FileOutputStream(target).use { out ->
                                copyExactly(input, out, size)
                            }
                        } else {
                            target.createNewFile()
                        }
                        skipPadding(input, size)
                    }
                    else -> {
                        // 未知类型（链接等）：跳过数据，不处理
                        if (size > 0) {
                            skipFully(input, size)
                            skipPadding(input, size)
                        }
                    }
                }
            }
        }
    }

    /** 去掉顶层目录段：`kokoro-xx/model.onnx` → `model.onnx`。 */
    private fun stripTopLevel(name: String): String {
        val trimmed = name.trimStart('.', '/')
        val slash = trimmed.indexOf('/')
        return if (slash >= 0) trimmed.substring(slash + 1) else trimmed
    }

    /**
     * 路径穿越防护 + 相对目标解析。
     * 校验在 [rawName]（剥离前）上执行，防止 ".." 被顶层目录剥离逻辑掩盖。
     */
    private fun safeResolve(destDir: File, rawName: String, relative: String): File {
        val normalized = rawName.replace('\\', '/')
        if (normalized.isEmpty()) {
            throw IllegalArgumentException("归档内出现空路径")
        }
        val segments = normalized.split('/').filter { it.isNotEmpty() && it != "." }
        if (segments.any { it == ".." } || normalized.startsWith("/")) {
            throw IllegalArgumentException("路径穿越拒绝: $rawName")
        }
        if (relative.isEmpty()) {
            throw IllegalArgumentException("归档内出现空路径: $rawName")
        }
        var current = destDir
        for (segment in relative.split('/').filter { it.isNotEmpty() && it != "." }) {
            current = File(current, segment)
        }
        return current
    }

    private fun isEndBlock(block: ByteArray): Boolean = block.all { it == 0.toByte() }

    /** 读满 n 字节；EOF 提前返回 null。 */
    private fun readFully(input: InputStream, n: Int): ByteArray? {
        val buffer = ByteArray(n)
        var offset = 0
        while (offset < n) {
            val read = input.read(buffer, offset, n - offset)
            if (read < 0) return if (offset == 0) null else buffer.copyOf(offset)
            offset += read
        }
        return buffer
    }

    private fun copyExactly(input: InputStream, out: FileOutputStream, size: Long) {
        val buffer = ByteArray(BUFFER)
        var remaining = size
        while (remaining > 0) {
            val read = input.read(buffer, 0, minOf(BUFFER.toLong(), remaining).toInt())
            if (read < 0) throw IllegalArgumentException("归档损坏：文件提前结束")
            out.write(buffer, 0, read)
            remaining -= read
        }
    }

    /** 跳过不足一块的填充字节（数据按 512 对齐）。 */
    private fun skipPadding(input: InputStream, size: Long) {
        val padding = (BLOCK_SIZE - (size % BLOCK_SIZE)) % BLOCK_SIZE
        if (padding > 0) skipFully(input, padding)
    }

    private fun skipFully(input: InputStream, n: Long) {
        var remaining = n
        while (remaining > 0) {
            val skipped = input.skip(remaining)
            if (skipped <= 0) throw IllegalArgumentException("归档损坏：skip 失败")
            remaining -= skipped
        }
    }

    /** ustar 八进制数值解析（同时支持 base-256 大数）。 */
    private fun parseOctal(block: ByteArray, offset: Int, length: Int): Long {
        if (block[offset].toInt() and 0x80 != 0) {
            // base-256 编码（GNU tar 大文件，>8GB）；首位只取 7 bit（去掉标志位）
            var value = 0L
            for (i in offset until offset + length) {
                val b = block[i].toLong() and 0xFF
                value = if (i == offset) b and 0x7F else (value shl 8) or b
            }
            return value
        }
        val raw = String(block, offset, length, Charsets.US_ASCII).trim { it == ' ' || it == '\u0000' }
        if (raw.isEmpty()) return 0L
        return raw.toLongOrNull(8) ?: throw IllegalArgumentException("非法的八进制长度字段: '$raw'")
    }

    private fun parseString(block: ByteArray, offset: Int, length: Int): String {
        var end = offset
        while (end < offset + length && block[end] != 0.toByte()) end++
        return String(block, offset, end - offset, Charsets.US_ASCII)
    }

    private const val BUFFER = 16 * 1024
}
