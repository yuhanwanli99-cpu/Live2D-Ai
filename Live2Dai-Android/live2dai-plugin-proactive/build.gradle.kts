plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// 主动陪伴插件 —— live2dai-plugin-proactive（docs/plan-task-plugin-proactive.md）
// 能力：WorkManager 周期检查「距上次对话时长 + 静默时段 + 冷却期」→ 触发 host.sendAssistantMessage
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
        // Robolectric 单元测试（SharedPreferences / WorkManager 需要 Android 环境）
        unitTests.isIncludeAndroidResources = true
    }
}

dependencies {
    // 插件 SDK：compileOnly（接口契约，core 运行时提供实现）
    compileOnly(project(":plugin-sdk"))

    // WorkManager：周期性后台检查（进程被杀后由系统恢复调度、Doze 兼容），
    // PeriodicWorkRequest 最小间隔 15 分钟是 Android 平台硬约束
    implementation("androidx.work:work-runtime-ktx:2.9.1")

    // androidx.core：主动陪伴通知（NotificationCompat / NotificationManagerCompat / ContextCompat 权限检查）
    implementation("androidx.core:core-ktx:1.15.0")

    // 测试
    // plugin-sdk 为 compileOnly（主源码）；测试需要完整可见性，单独声明 testImplementation
    testImplementation(project(":plugin-sdk"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("org.robolectric:robolectric:4.14.1")
    testImplementation("androidx.test:core:1.6.1")
    // WorkManager 官方测试工具：TestListenableWorkerBuilder（构造 Worker）/ WorkManagerTestInitHelper（测试 WorkManager 实例）
    testImplementation("androidx.work:work-testing:2.9.1")
}
