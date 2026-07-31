plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// 记忆插件骨架 —— live2dai-plugin-memory（对应 docs/plugin-template.md）
// 骨架阶段：仅搭建结构，onLoad 暂不注册任何 hook/tool，具体逻辑留给 task-plugin-memory。
android {
    namespace = "com.live2dai.plugin.memory"
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
