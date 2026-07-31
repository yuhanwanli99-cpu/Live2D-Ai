plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("com.google.devtools.ksp")
}

// 记忆插件 —— live2dai-plugin-memory
// 长期记忆（事实摘要）存储与检索：Room（SQLite）+ 关键词重叠打分。
// beforeSend 注入相关记忆，afterReceive 周期性调用 core 注入的 SummaryLlmClient 生成新事实。
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
        // Robolectric 单元测试（Room 内存数据库依赖 Android framework SQLite）
        unitTests.isIncludeAndroidResources = true
    }
}

dependencies {
    // 插件 SDK：compileOnly（接口契约，core 运行时提供实现）
    compileOnly(project(":plugin-sdk"))

    // Room 长期记忆存储（KSP 注解处理生成实现）
    implementation("androidx.room:room-runtime:2.6.1")
    implementation("androidx.room:room-ktx:2.6.1")
    ksp("androidx.room:room-compiler:2.6.1")

    // 协程（DAO suspend / hook suspend）
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")

    // 测试
    // plugin-sdk 为 compileOnly（主源码）；测试需要完整可见性，单独声明 testImplementation
    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("org.robolectric:robolectric:4.14.1")
    testImplementation("androidx.test:core:1.6.1")
}
