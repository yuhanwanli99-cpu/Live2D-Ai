package com.live2dai.plugin.agenttools

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * "open_app" 工具：按应用显示名称打开对应 App（如「帮我打开微信」）。
 *
 * LLM 只需要给出应用名称（「微信」「支付宝」），无需猜测包名——
 * 通过 [resolvePackageByLabel] 在已安装应用里做一次本地模糊匹配，
 * 命中后调用 `PackageManager.getLaunchIntentForPackage` 启动。
 *
 * 风险等级：仅切换前台，无数据写入风险，[requiresConfirmation] = false。
 * 匹配歧义：MVP 取第一个模糊匹配结果（见 `docs/plan-task-plugin-agent-tools.md` R3），
 * 返回结果中带上匹配到的应用名，便于用户/LLM 发现匹配错误并纠正。
 */
class OpenAppTool : ToolDefinition {

    private val resolver: AppResolver

    /** 生产路径：通过 PackageManager 解析已安装应用。 */
    constructor(context: Context) {
        resolver = RealAppResolver(context)
    }

    /**
     * 内部测试接缝：注入候选表与启动器，避免单元测试依赖真实 PackageManager。
     * 与生产路径共用同一份 execute 核心逻辑。
     */
    internal constructor(resolver: AppResolver) {
        this.resolver = resolver
    }

    override val name = "open_app"
    override val description = "打开手机上指定名称的应用。app_name 填应用名称（如「微信」「支付宝」），无需填包名。"
    override val parametersSchema = """
        {
          "type": "object",
          "properties": {
            "app_name": {
              "type": "string",
              "description": "应用名称，如「微信」「支付宝」"
            }
          },
          "required": ["app_name"]
        }
    """.trimIndent()
    override val requiresConfirmation = false  // 仅切前台，无数据风险

    override suspend fun execute(argsJson: String): String {
        val appName = parseAppName(argsJson)
            ?: return """{"error":"缺少 app_name 参数"}"""
        val packageName = resolvePackageByLabel(appName, resolver.installedApps())
            ?: return """{"error":"未找到名为「$appName」的应用"}"""
        val intent = resolver.launchIntentFor(packageName)
            ?: return """{"error":"「$appName」没有可启动的入口"}"""
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        resolver.startActivity(intent)
        return """{"result":"已打开 $appName"}"""
    }
}

/**
 * 解析 [OpenAppTool.execute] 参数中的 `app_name`（字符串、非空、去空白）。
 * 非法 JSON / 字段缺失 / 类型错误时返回 null（由调用方转为清晰错误信息，不崩溃）。
 */
internal fun parseAppName(argsJson: String): String? = try {
    val obj = Json.parseToJsonElement(argsJson).jsonObject
    obj["app_name"]?.jsonPrimitive?.takeIf { it.isString }?.content?.trim()?.takeIf { it.isNotEmpty() }
} catch (_: Exception) {
    null
}

/**
 * 按应用显示名称模糊匹配包名：
 * 1. 精确匹配（忽略大小写）优先；
 * 2. 应用名包含目标名（如目标「微信」命中「企业微信」）；
 * 3. 目标名包含应用名。
 * 取第一个命中，不做歧义消解（设计文档 R3）。候选为空或目标名为空时返回 null。
 */
internal fun resolvePackageByLabel(appName: String, apps: List<Pair<String, String>>): String? {
    val target = appName.trim()
    if (target.isEmpty()) return null
    return apps.firstOrNull { (label, _) -> label.equals(target, ignoreCase = true) }?.second
        ?: apps.firstOrNull { (label, _) -> label.contains(target, ignoreCase = true) }?.second
        ?: apps.firstOrNull { (label, _) -> target.contains(label, ignoreCase = true) }?.second
}

/**
 * OpenAppTool 依赖的"已安装应用解析"能力接缝（生产 = PackageManager，测试 = 内存数据）。
 */
internal interface AppResolver {
    /** 已安装应用的 (显示名, 包名) 候选表。 */
    fun installedApps(): List<Pair<String, String>>

    /** 取包名对应的启动 Intent；无启动入口时返回 null。 */
    fun launchIntentFor(packageName: String): Intent?

    /** 启动 Activity（要求调用方已设置 NEW_TASK flag）。 */
    fun startActivity(intent: Intent)
}

/** 生产实现：走 [android.content.pm.PackageManager]。 */
internal class RealAppResolver(private val context: Context) : AppResolver {

    override fun installedApps(): List<Pair<String, String>> {
        val pm = context.packageManager
        return pm.getInstalledApplications(0).mapNotNull { app ->
            val label = app.loadLabel(pm)?.toString()?.trim()?.takeIf { it.isNotEmpty() }
                ?: return@mapNotNull null
            label to app.packageName
        }
    }

    override fun launchIntentFor(packageName: String): Intent? =
        context.packageManager.getLaunchIntentForPackage(packageName)

    override fun startActivity(intent: Intent) {
        context.startActivity(intent)
    }
}
