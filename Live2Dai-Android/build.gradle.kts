// Top-level build file for Live2Dai-Android
plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.1.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.0" apply false
    // KSP（Room 注解处理，live2dai-plugin-memory 使用；版本须匹配 Kotlin 2.1.0）
    id("com.google.devtools.ksp") version "2.1.0-1.0.29" apply false
}

// ─── Wave 2 骨架插件测试隔离（加载门）──────────────────────────────────────
// settings.gradle.kts 中 live2dai-plugin-agent-tools 标注为“仅搭建结构，尚未在 app 侧装载”。
// 在全量构建（connectedDebugAndroidTest / test）时，这些模块的测试任务会把其依赖链拉入
// 执行图——在无法稳定访问 repo.maven.apache.org 的环境（TLS/证书问题）中，即便这些
// 模块并未被 app 实际使用，其依赖解析失败也会拖垮整个构建。
// 因此在模块真正装载进 app 之前，默认从任务图排除其单元测试与 connected 测试任务
// （等价于命令行 -x 排除，排除会连同其依赖链一起剪枝）。
// 接入方式：某插件正式装载进 app 后，将其从下方列表移除并同步解除
// settings.gradle.kts 中的骨架注释，其测试即恢复运行。
// 注：live2dai-plugin-memory 已于 V1.1 模块3 正式装载进 app（MainActivity loadPlugins），
// 已从本列表移除，其测试随全量构建恢复运行。
// 注：live2dai-plugin-proactive 已于 V1.2 模块6 正式装载进 app（MainActivity loadPlugins），
// 已从本列表移除，其测试随全量构建恢复运行。
val unloadedSkeletonModules = listOf(
    ":live2dai-plugin-agent-tools",
)
val excludedSkeletonTestTasks = unloadedSkeletonModules.flatMap { module ->
    listOf(
        "$module:testDebugUnitTest",
        "$module:testReleaseUnitTest",
        "$module:connectedDebugAndroidTest",
        "$module:connectedReleaseAndroidTest",
    )
}
// 用户显式请求某个骨架模块的测试任务时尊重其意图，不做排除
val userRequestedSkeletonTask = gradle.startParameter.taskNames.any { requested ->
    unloadedSkeletonModules.any { module -> requested.startsWith(module) }
}
if (!userRequestedSkeletonTask) {
    // projectsEvaluated 时各模块任务已创建，过滤掉不存在的任务（如库模块没有
    // connectedReleaseAndroidTest），避免“Cannot locate excluded tasks”警告
    gradle.projectsEvaluated {
        val existingTasks = excludedSkeletonTestTasks.filter { path ->
            val projectPath = path.substringBeforeLast(':')
            val taskName = path.substringAfterLast(':')
            gradle.rootProject.findProject(projectPath)?.tasks?.findByName(taskName) != null
        }
        if (existingTasks.isNotEmpty()) {
            gradle.startParameter.excludedTaskNames.addAll(existingTasks)
        }
    }
}
