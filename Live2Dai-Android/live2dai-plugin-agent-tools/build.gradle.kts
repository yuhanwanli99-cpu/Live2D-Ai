plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// Agent 工具执行插件 —— live2dai-plugin-agent-tools（对应 docs/plugin-template.md）
// 注册 open_app / send_notification_reply / set_alarm 三个工具（task-plugin-agent-tools）。
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
    // 插件 SDK：compileOnly（接口契约，core 运行时提供实现）
    compileOnly(project(":plugin-sdk"))

    // 工具参数 JSON Schema / 结果解析
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")

    // 测试
    // plugin-sdk 为 compileOnly（主源码）；测试需要完整可见性，单独声明 testImplementation
    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    // Context/PackageManager/Intent 相关路径用 Robolectric（vision 插件已验证可行，R5）
    testImplementation("org.robolectric:robolectric:4.14.1")
    testImplementation("androidx.test:core:1.6.1")
}
