plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// 视觉插件 —— live2dai-plugin-vision
// 从 core 的 VisionService.kt 迁出，通过 plugin-sdk 接口（Live2DaiPlugin/ChatHook/ToolDefinition）
// 注册「截图描述」hook + 「capture_screen」工具。截图能力由 core 注入（ScreenshotProvider 接口）。
android {
    namespace = "com.live2dai.plugin.vision"
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
        // Robolectric 单元测试（Bitmap 等 Android 类）
        unitTests.isIncludeAndroidResources = true
    }
}

dependencies {
    // 插件 SDK：compileOnly（接口契约，core 运行时提供实现）
    compileOnly(project(":plugin-sdk"))

    // HTTP 请求（GLM-4V API）
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    // JSON 解析（kotlinx-serialization）
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    // 协程（Dispatchers.IO）
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")

    // 测试
    // plugin-sdk 为 compileOnly（主源码）；测试需要完整可见性，单独声明 testImplementation
    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.robolectric:robolectric:4.14.1")
    testImplementation("androidx.test:core:1.6.1")
}
