pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    plugins {
        id("org.jetbrains.kotlin.jvm") version "2.1.0"
        id("org.jetbrains.kotlin.plugin.serialization") version "2.1.0"
        id("org.jetbrains.kotlin.android") version "2.1.0"
        id("org.jetbrains.kotlin.plugin.compose") version "2.1.0"
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.PREFER_PROJECT)
    repositories {
        google()
        mavenCentral()
        maven { url = uri("https://jitpack.io") }
    }
}

rootProject.name = "Live2Dai-Android"
include(":app")
include(":plugin-sdk")
include(":live2dai-plugin-vision")
// Wave 2 骨架（docs/plugin-template.md）：仅搭建结构，尚未在 app 侧装载，不影响现有行为。
// 其测试任务已被根 build.gradle.kts 的“骨架插件测试隔离”默认排除；真正装载进 app 时
// 需同步从根 build.gradle.kts 的 unloadedSkeletonModules 中移除对应模块。
// live2dai-plugin-memory 已于 V1.1 模块3 正式装载进 app（MainActivity loadPlugins），
// 已从骨架列表除名，其测试随全量构建恢复运行。
// live2dai-plugin-proactive 已于 V1.2 模块6 正式装载进 app（MainActivity loadPlugins），
// 已从骨架列表除名，其测试随全量构建恢复运行。
include(":live2dai-plugin-memory")
include(":live2dai-plugin-proactive")
include(":live2dai-plugin-agent-tools")
