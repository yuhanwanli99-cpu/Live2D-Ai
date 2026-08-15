package com.live2dai.android

import android.os.StatFs
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit

/**
 * 下载状态（契约见 docs/plan-v12-plan.schema.json → dataModels.DownloadState）。
 *
 * 状态机：Idle → Downloading(progress) → Verifying → Completed | Failed。
 * - 已下载且校验通过的文件直接进入 Completed（幂等重入）。
 * - 下载中断/失败保留 .part 文件，重试时 Range 续传。
 * - SHA256 不匹配会删除 .part 重新下载（数据损坏不能续传）。
 */
sealed class DownloadState {
    /** 初始状态（尚未开始） */
    object Idle : DownloadState()

    /** 下载中（progress 0.0-1.0） */
    data class Downloading(
        val progress: Float,
        val downloadedBytes: Long,
        val totalBytes: Long,
    ) : DownloadState()

    /** 下载完成，正在 SHA256 校验 */
    object Verifying : DownloadState()

    /** 下载 + 校验完成 */
    data class Completed(val path: String) : DownloadState()

    /** 失败（error 为人类可读原因） */
    data class Failed(val error: String) : DownloadState()
}

/**
 * 离线模型清单条目（plan §3.3.3 / §4.4.2 模型管理）。
 *
 * 模型不打包进 APK，按需下载到 `externalFilesDir/models/{id}/`（plan §3.3.3）。
 * URL 与 SHA256：plan 未给具体值 → 标注"待验证"（实测 URL 存在，SHA256 需下载后回填）。
 */
data class ModelSpec(
    /** 模型唯一标识（"kokoro_tts" / "sensevoice_asr"，供 UI 状态查询） */
    val id: String,
    /** 人类可读名称 */
    val name: String,
    /** 远端文件名（GitHub Releases 资产名） */
    val fileName: String,
    /** 下载 URL */
    val url: String,
    /** 期望 SHA256（null = 跳过校验；待验证时先置 null） */
    val expectedSha256: String?,
    /** 解压前体积（用于存储空间预检，字节） */
    val sizeBytes: Long,
    /** 模型文件候选名（tar 解压后按任一存在判定模型就绪） */
    val modelFileCandidates: List<String>,
    /** 必须同时存在的伴生文件（tokens 等） */
    val requiredCompanions: List<String> = emptyList(),
)

/**
 * 磁盘可用空间探测（可注入以便单测；默认 StatFs）。
 *
 * plan §3.3.5 风险缓解：模型下载前预检存储空间，不足时快速失败（不发起 HTTP）。
 */
fun interface SpaceChecker {
    fun availableBytes(dir: File): Long
}

/**
 * 离线模型下载管理（V1.2 模块3/4，plan §3.3.3 + §9 教训：进度通知、断点续传、
 * SHA256 校验、存储空间预检四件事全覆盖）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.ModelDownloadManager.*：
 * - [download] 挂起直到下载完成/失败；返回 StateFlow 供 UI 观察进度
 * - 断点续传：`.part` 临时文件 + `Range: bytes=<start>-`（服务端不支持 Range 时
 *   自动重头；416 时重头；校验失败时删除 .part 防止损坏续传）
 * - SHA256：流式校验（不整体载入内存）；null 期望值跳过
 * - 空间预检：StatFs 可用空间 vs 期望体积（可注入 [SpaceChecker] 便于测试）
 * - 模型级 [download(ModelSpec)]：下载 tar.bz2 → [TarBz2] 解压到
 *   `models/{spec.id}/` → 校验模型文件就绪 → Completed(模型目录)
 *
 * 日志规范（plan §5.7.4）：关键节点 Log.i（国产 ROM 抑制 Log.d 教训），TAG="ModelDownload"。
 */
class ModelDownloadManager(
    private val context: android.content.Context,
    private val client: OkHttpClient = defaultOkHttpClient(),
    private val spaceChecker: SpaceChecker = SpaceChecker { dir ->
        try {
            StatFs(dir.absolutePath).availableBytes
        } catch (e: Exception) {
            // StatFs 失败（目录不存在等）→ 保守返回 0（视为空间不足）
            Log.w(TAG, "StatFs 失败: ${e.message}")
            0L
        }
    },
) {

    companion object {
        private const val TAG = "ModelDownload"

        /** 单次写缓冲（16KB） */
        private const val BUFFER_SIZE = 16 * 1024

        /** 进度发射节流（StateFlow 无背压，避免每 16KB 一发的洪泛） */
        private const val PROGRESS_EMIT_INTERVAL_MS = 80L

        /** 空间预检冗余系数（下载解压双份空间：tar + 解压文件） */
        private const val SPACE_FACTOR = 1.2

        private fun defaultOkHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(120, TimeUnit.SECONDS)
            .writeTimeout(30, TimeUnit.SECONDS)
            .build()

        /** 模型根目录：externalFilesDir/models/（plan §3.3.3） */
        fun modelsRootDir(context: android.content.Context): File =
            File(context.getExternalFilesDir(null), "models").apply { mkdirs() }

        // ─── V1.2 模型清单（URL 已实测存在；SHA256 待验证——plan 未提供具体值） ───

        /** Kokoro 离线 TTS（v1.1-zh：103 说话人 zh+en，int8 档 147MB，用户裁定 B-9=升级 v1.1-zh） */
        val KOKORO_TTS = ModelSpec(
            id = "kokoro_tts",
            name = "Kokoro 离线 TTS",
            fileName = "kokoro-int8-multi-lang-v1_1.tar.bz2",
            url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_1.tar.bz2",
            expectedSha256 = null, // TODO(sha256-SUSPENDED-2026-08-12): GitHub 下载中断历史遗留（v1.0 下载至 50MB 断连）；v1.1-zh 包需真机下载实测后回填 sha256（禁止猜 hash，架构 B U2 闭环项）
            sizeBytes = 147_031_220L, // HEAD 实测 Content-Length（2026-08-15，kokoro-int8-multi-lang-v1_1.tar.bz2）
            modelFileCandidates = listOf("model.onnx", "model.int8.onnx"),
            requiredCompanions = listOf("tokens.txt"),
        )

        /** 全部模型清单（供 UI 遍历）——K1 单引擎：仅 Kokoro（ASR 链已随 Wave 5 删除） */
        val CATALOG: List<ModelSpec> = listOf(KOKORO_TTS)
    }

    /** 各下载目标的状态流（destPath / specId → 状态），供 UI 查询 */
    private val states = ConcurrentHashMap<String, MutableStateFlow<DownloadState>>()

    /** 各目标是否正在下载（去重：同一目标不并发双开） */
    private val activeDownloads = ConcurrentHashMap.newKeySet<String>()

    // ─── 原始契约（schema functions.ModelDownloadManager.download） ───

    /**
     * 下载单个文件到 [destPath]（可断点续传 + SHA256 校验 + 空间预检）。
     *
     * @param url 下载地址
     * @param destPath 目标本地文件路径
     * @param expectedSha256 期望 SHA256（null 跳过校验）
     * @return 下载状态流（Idle→Downloading→Verifying→Completed|Failed）
     */
    suspend fun download(
        url: String,
        destPath: String,
        expectedSha256: String? = null,
    ): StateFlow<DownloadState> {
        val key = destPath
        val flow = states.getOrPut(key) { MutableStateFlow(DownloadState.Idle) }
        val partial = File("$destPath.part")
        val finalFile = File(destPath)

        // 去重：同一目标已在下载 → 直接返回现有流（不并发双开）
        if (!activeDownloads.add(key)) return flow

        return withContext(Dispatchers.IO) {
            try {
                // 1) 已下载且（无需校验或）校验通过 → 直接 Completed（幂等重入）
                if (finalFile.exists() && fileMatches(finalFile, expectedSha256)) {
                    flow.value = DownloadState.Completed(destPath)
                } else {
                    if (finalFile.exists()) {
                        Log.i(TAG, "已存在但校验不通过，删除重下: $destPath")
                        finalFile.delete()
                    }
                    // 2) 存储空间预检（模型级在 download(spec) 已做；此处对已存在的 .part 兜底）
                    val remaining = if (partial.exists()) partial.length() else 0L
                    val available = spaceChecker.availableBytes(File(destPath).parentFile ?: finalFile)
                    if (available < (remaining * SPACE_FACTOR).toLong()) {
                        flow.value = DownloadState.Failed(
                            "存储空间不足：需要 ${formatBytes((remaining * SPACE_FACTOR).toLong())}，可用 ${formatBytes(available)}"
                        )
                        return@withContext flow
                    }
                    // 3) 断点续传下载
                    flow.value = DownloadState.Idle
                    downloadWithResume(url, partial, finalFile, flow, expectedSha256)
                }
            } catch (e: CancellationException) {
                // 协程取消：保留 .part 供下次续传（resume 语义），状态留在 Downloading
                throw e
            } catch (e: Exception) {
                Log.e(TAG, "下载失败（${url.take(80)}）: ${e.message}")
                flow.value = DownloadState.Failed(e.message ?: "下载失败")
            } finally {
                activeDownloads.remove(key)
            }
            flow
        }
    }

    // ─── 模型级下载（tar.bz2 → 解压 → 模型目录） ───

    /**
     * 按模型清单下载并解压模型到 `models/{spec.id}/`。
     *
     * @return 状态流；[DownloadState.Completed] 的 path 为模型目录（模型文件已就绪）
     */
    suspend fun download(spec: ModelSpec): StateFlow<DownloadState> {
        val specFlow = states.getOrPut("spec:${spec.id}") { MutableStateFlow(DownloadState.Idle) }
        val dir = modelDir(spec)
        if (isDownloaded(spec)) {
            specFlow.value = DownloadState.Completed(dir.absolutePath)
            return specFlow
        }
        // 空间预检（模型级：spec.sizeBytes 更准确，先于 HTTP 快速失败）
        val available = spaceChecker.availableBytes(dir)
        if (available < (spec.sizeBytes * SPACE_FACTOR).toLong()) {
            specFlow.value = DownloadState.Failed(
                "存储空间不足：需要 ${formatBytes((spec.sizeBytes * SPACE_FACTOR).toLong())}，可用 ${formatBytes(available)}"
            )
            return specFlow
        }
        val tarFile = File(dir, spec.fileName)
        val flow = download(spec.url, tarFile.absolutePath, spec.expectedSha256)
        // 等待原始下载终态（已下载→Completed 立即返回；并发去重路径则等待既有下载结束）
        val terminal = flow.first { it is DownloadState.Completed || it is DownloadState.Failed }
        when (terminal) {
            is DownloadState.Completed -> {
                try {
                    specFlow.value = DownloadState.Verifying
                    Log.i(TAG, "解压模型 ${spec.fileName} → ${dir.name}（${spec.name}）")
                    TarBz2.extract(tarFile, dir)
                    tarFile.delete()
                    if (isDownloaded(spec)) {
                        specFlow.value = DownloadState.Completed(dir.absolutePath)
                    } else {
                        specFlow.value = DownloadState.Failed("解压后模型文件缺失（${spec.name}）")
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "模型解压失败: ${e.message}")
                    specFlow.value = DownloadState.Failed("模型解压失败: ${e.message}")
                }
            }
            is DownloadState.Failed -> specFlow.value = terminal
            else -> specFlow.value = DownloadState.Failed("下载未完成: $terminal")
        }
        return specFlow
    }

    // ─── 状态查询（供 UI / Provider 健康判断） ───

    /** 查询某模型的状态流（UI 进度条/状态展示；未开始过则返回 null）。 */
    fun stateOf(modelId: String): StateFlow<DownloadState>? = states["spec:$modelId"]

    /** 模型目录：externalFilesDir/models/{spec.id}/ */
    fun modelDir(spec: ModelSpec): File = File(modelsRootDir(context), spec.id).apply { mkdirs() }

    /** 模型是否已就绪（任一模型文件存在 + 伴生文件齐全）。 */
    fun isDownloaded(spec: ModelSpec): Boolean {
        val dir = modelDir(spec)
        if (!dir.exists()) return false
        val modelOk = spec.modelFileCandidates.any { File(dir, it).exists() }
        val companionsOk = spec.requiredCompanions.all { File(dir, it).exists() }
        return modelOk && companionsOk
    }

    /** 已就绪模型的实际模型文件（供引擎加载；未就绪返回 null）。 */
    fun resolvedModelFile(spec: ModelSpec): File? {
        val dir = modelDir(spec)
        return spec.modelFileCandidates.map { File(dir, it) }.firstOrNull { it.exists() }
    }

    // ─── 内部实现 ───

    /** 断点续传核心：Range 请求 → 追加写 .part → 校验 → 改名。 */
    private fun downloadWithResume(
        url: String,
        partial: File,
        finalFile: File,
        flow: MutableStateFlow<DownloadState>,
        expectedSha256: String?,
    ) {
        val start = if (partial.exists()) partial.length() else 0L
        if (start > 0) {
            Log.i(TAG, "断点续传：从 ${formatBytes(start)} 继续（${partial.name}）")
        }
        val requestBuilder = Request.Builder().url(url)
        if (start > 0) requestBuilder.header("Range", "bytes=$start-")
        val response = client.newCall(requestBuilder.build()).execute()
        response.use {
            if (!response.isSuccessful) {
                throw RuntimeException("HTTP ${response.code}: ${response.message}")
            }
            val body = response.body ?: throw RuntimeException("空响应体")
            val totalBytes: Long = when {
                response.code == 206 -> (body.contentLength().takeIf { it >= 0 } ?: 0L) + start
                response.code == 416 -> {
                    // Range 不可满足：服务端可能已存完整文件——重头下载
                    partial.delete()
                    body.contentLength().takeIf { it >= 0 } ?: 0L
                }
                else -> body.contentLength().takeIf { it >= 0 } ?: 0L
            }
            // 200 且带 Range 头：服务端忽略 Range → 重头（截断 .part）
            if (response.code == 200 && start > 0) {
                Log.i(TAG, "服务端不支持 Range，重头下载")
                partial.delete()
            }
            // 存储空间预检（响应头已知 Content-Length 后，空间不足快速失败不写盘）
            val spaceNeeded = (totalBytes * SPACE_FACTOR).toLong()
            val spaceAvailable = spaceChecker.availableBytes(partial.parentFile ?: finalFile)
            if (spaceAvailable < spaceNeeded) {
                throw RuntimeException(
                    "存储空间不足：需要 ${formatBytes(spaceNeeded)}，可用 ${formatBytes(spaceAvailable)}"
                )
            }
            var written = if (partial.exists()) partial.length() else 0L
            var lastEmit = 0L
            FileOutputStream(partial, /* append = */ written > 0).use { out ->
                val buffer = ByteArray(BUFFER_SIZE)
                while (true) {
                    val read = body.byteStream().read(buffer)
                    if (read < 0) break
                    out.write(buffer, 0, read)
                    written += read
                    val now = System.currentTimeMillis()
                    if (now - lastEmit >= PROGRESS_EMIT_INTERVAL_MS || totalBytes > 0 && written >= totalBytes) {
                        lastEmit = now
                        val progress = if (totalBytes > 0) (written.toFloat() / totalBytes).coerceIn(0f, 1f) else 0f
                        flow.value = DownloadState.Downloading(progress, written, totalBytes)
                    }
                }
            }
        }
        // 4) SHA256 校验
        if (expectedSha256 != null) {
            flow.value = DownloadState.Verifying
            val actual = sha256Hex(partial)
            if (!actual.equals(expectedSha256, ignoreCase = true)) {
                // 损坏数据不能续传：删除 .part（下次重头）
                partial.delete()
                throw RuntimeException("SHA256 校验失败：期望 $expectedSha256，实际 $actual")
            }
        }
        // 校验通过 → .part 改名为最终文件（原子可见）
        if (finalFile.exists()) finalFile.delete()
        if (!partial.renameTo(finalFile)) {
            throw RuntimeException("无法移动临时文件到 $finalFile")
        }
        flow.value = DownloadState.Completed(finalFile.absolutePath)
        Log.i(TAG, "下载完成: ${finalFile.name}（${formatBytes(finalFile.length())}）")
    }

    /** 文件 SHA256（流式，不整体载入内存）。 */
    private fun sha256Hex(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(BUFFER_SIZE)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    /** 文件是否匹配期望 SHA256（期望为 null 时仅要求存在）。 */
    private fun fileMatches(file: File, expectedSha256: String?): Boolean {
        if (!file.exists() || file.length() == 0L) return false
        if (expectedSha256 == null) return true
        return sha256Hex(file).equals(expectedSha256, ignoreCase = true)
    }

    /** 期望体积：空间预检以响应头 Content-Length 为准（downloadWithResume 内），此处无预知。 */
    private fun expectedSize(url: String, expectedSha256: String?): Long = 0L

    private fun formatBytes(bytes: Long): String = when {
        bytes >= 1024L * 1024 * 1024 -> "%.1fGB".format(bytes / 1024.0 / 1024 / 1024)
        bytes >= 1024L * 1024 -> "%.1fMB".format(bytes / 1024.0 / 1024)
        bytes >= 1024L -> "%.1fKB".format(bytes / 1024.0)
        else -> "${bytes}B"
    }
}
