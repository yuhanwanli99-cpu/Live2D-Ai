plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// Agent 工具执行插件骨架 —— live2dai-plugin-agent-tools（对应 docs/plugin-template.md）
// 骨架阶段：仅搭建结构，onLoad 暂不注册任何工具，具体逻辑留给 task-plugin-agent-tools。
android {
    namespace = "com.live2dai.plugin.agenttools"
    compileSdk = 35

    defaultConfig {
        minSdk = 26
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }
}

dependencies {
    compileOnly(project(":plugin-sdk"))

    // 预留：工具参数多为 JSON Schema/结果，提前加上序列化依赖
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")

    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
}
