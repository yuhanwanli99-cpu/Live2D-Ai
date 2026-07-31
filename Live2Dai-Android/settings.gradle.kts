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
// Wave 2 骨架（docs/plugin-template.md）：仅搭建结构，尚未在 app 侧装载，不影响现有行为
include(":live2dai-plugin-memory")
include(":live2dai-plugin-proactive")
include(":live2dai-plugin-agent-tools")
