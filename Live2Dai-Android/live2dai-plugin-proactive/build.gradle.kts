plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// 主动陪伴插件骨架 —— live2dai-plugin-proactive（对应 docs/plugin-template.md）
// 骨架阶段：仅搭建结构，onLoad 暂不注册任何调度器，具体逻辑留给 task-plugin-proactive。
android {
    namespace = "com.live2dai.plugin.proactive"
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

    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
}
