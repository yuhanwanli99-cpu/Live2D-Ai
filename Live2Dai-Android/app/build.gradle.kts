import java.util.Properties
import java.io.FileInputStream

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android") version "2.1.0"
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.0"
    id("org.jetbrains.kotlin.plugin.serialization") version "2.1.0"
}

// Local AAR for Live2D Cubism SDK (place in app/libs/)
repositories {
    google()
    mavenCentral()
    maven { url = uri("https://jitpack.io") }
    flatDir { dirs("libs") }
}

android {
    namespace = "com.live2dai.android"
    compileSdk = 35

    // CMake external native build for PurismCore
    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    ndkVersion = "27.0.12077973"

    defaultConfig {
        applicationId = "com.live2dai.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables {
            useSupportLibrary = true
        }

        // ── API Keys: Read from local.properties (primary), then env vars (fallback) ──
        // local.properties is .gitignored — safe for secrets
        // Note: project.findProperty() does NOT read custom properties from local.properties
        // so we must load local.properties explicitly via java.util.Properties.
        // OS env vars are used when local.properties is absent (e.g. CI)
        val localProps = Properties()
        val localPropsFile = rootProject.file("local.properties")
        if (localPropsFile.exists()) {
            localProps.load(FileInputStream(localPropsFile))
        }

        val deepseekKey = localProps.getProperty("DEEPSEEK_API_KEY")?.ifBlank { null }
            ?: (project.findProperty("DEEPSEEK_API_KEY") as? String)?.ifBlank { null }
            ?: System.getenv("DEEPSEEK_API_KEY")?.ifBlank { null }
            ?: ""
        val cosyvoiceKey = localProps.getProperty("COSYVOICE_API_KEY")?.ifBlank { null }
            ?: (project.findProperty("COSYVOICE_API_KEY") as? String)?.ifBlank { null }
            ?: System.getenv("COSYVOICE_API_KEY")?.ifBlank { null }
            ?: ""
        val cosyvoiceWsUrl = localProps.getProperty("COSYVOICE_WS_URL")?.ifBlank { null }
            ?: (project.findProperty("COSYVOICE_WS_URL") as? String)?.ifBlank { null }
            ?: System.getenv("COSYVOICE_WS_URL")?.ifBlank { null }
            ?: ""
        val zhipuKey = localProps.getProperty("ZHIPU_API_KEY")?.ifBlank { null }
            ?: (project.findProperty("ZHIPU_API_KEY") as? String)?.ifBlank { null }
            ?: System.getenv("ZHIPU_API_KEY")?.ifBlank { null }
            ?: ""

        buildConfigField("String", "DEEPSEEK_API_KEY", "\"${deepseekKey}\"")
        buildConfigField("String", "ZHIPU_API_KEY", "\"${zhipuKey}\"")
        buildConfigField("String", "COSYVOICE_API_KEY", "\"${cosyvoiceKey}\"")
        buildConfigField("String", "COSYVOICE_WS_URL", "\"${cosyvoiceWsUrl}\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  sherpa-onnx 离线引擎（V1.2 模块3/4）— 条件源码集
//
//  sherpa-onnx 官方不在 Maven Central 发布 Android AAR（plan §3.3.3 的坐标
//  `com.k2fsa.sherpa:onnx-tts-engine:1.12.0` 实测不存在），引擎以 GitHub Releases 的
//  `.aar` 分发。下载到 app/libs/ 后由下方 fileTree 自动引入（与 Live2D SDK 同机制）：
//
//    curl -L -o app/libs/sherpa-onnx-1.13.4.aar \
//      https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.4/sherpa-onnx-1.13.4.aar
//    （中国大陆网络需镜像，如 https://gh-proxy.com/ 前缀；AAR 已被根 .gitignore 排除）
//
//  AAR 缺失时构建仍通过：src/sherpaOnnxEngine/ 条件源码集不参与编译，sherpa 引用
//  不进入 classpath，SherpaOnnxTtsProvider/AsrProvider 运行时 health=UNAVAILABLE 降级
//  （SherpaOnnxEngineRegistry.tryBootstrap 的 Class.forName 静默失败）。
// ═══════════════════════════════════════════════════════════════════
if (file("libs/sherpa-onnx-1.13.4.aar").exists()) {
    // AAR 存在 → 编译真实引擎实现（JNI 包装，类型安全）
    android.sourceSets.getByName("main").java.srcDir("src/sherpaOnnxEngine/kotlin")
}

// ═══════════════════════════════════════════════════════════════════
//  Pre-build: 复制 shared/model_registry.json → app assets
//  注册表（shared/model_registry.json）是跨平台唯一真相源，构建时同步到
//  src/main/assets/model_registry.json，运行时由 ModelRegistry 从 assets 读取。
//  注意：Gradle 根项目是 Live2Dai-Android/，shared/ 在其父目录（仓库根）下。
// ═══════════════════════════════════════════════════════════════════
val registrySourceFile = rootProject.projectDir.parentFile.resolve("shared/model_registry.json")
val registryTargetFile = file("src/main/assets/model_registry.json")

val copyModelRegistry = tasks.register<Copy>("copyModelRegistry") {
    group = "live2dai"
    description = "将 shared/model_registry.json 复制到 app/src/main/assets/（Live2D 模型注册表）"
    from(registrySourceFile)
    into(file("src/main/assets"))
}

// 挂到 preBuild / variant / unitTest 前置任务：
// - preBuild：命令行 assemble 的必经节点
// - preDebugBuild / preReleaseBuild：IDE 增量编译可能跳过 preBuild，直接执行变体任务
// - preDebugUnitTestBuild / preReleaseUnitTestBuild：单测（Robolectric 读 assets）需要文件就绪
// tasks.matching+configureEach 惰性匹配，任务不存在时静默跳过，不会报错。
listOf(
    "preBuild",
    "preDebugBuild",
    "preReleaseBuild",
    "preDebugUnitTestBuild",
    "preReleaseUnitTestBuild",
).forEach { taskName ->
    tasks.matching { it.name == taskName }.configureEach {
        dependsOn(copyModelRegistry)
    }
}

// clean 时一并删除生成的注册表（assets 位于 src/ 下，Gradle clean 不会自动清理）
tasks.named("clean") {
    doLast {
        delete(registryTargetFile)
    }
}

dependencies {
    // 插件 SDK：core 与插件之间的最小契约（Live2DaiPlugin/PluginHost/ChatHook/ToolDefinition）
    implementation(project(":plugin-sdk"))

    // 视觉插件：截图描述 hook + capture_screen 工具（从 VisionService.kt 迁出）
    implementation(project(":live2dai-plugin-vision"))

    // 记忆插件：长期记忆（Room 事实存储 + 关键词检索注入），V1.1 模块3 正式装载。
    // core 注入 SummaryLlmClient 实现（LlmSummaryClient，复用用户配置的 LLM Provider）；
    // 失败静默：记忆插件任何异常不影响对话核心链路（架构约束 §8.2-2）。
    implementation(project(":live2dai-plugin-memory"))

    // Agent 工具插件：open_app / send_notification_reply / set_alarm（Wave 2）
    // 仅编译依赖（实现其 NotificationReplyBridge 接缝）；运行时插件未装载，行为不受影响。
    // 排期接入时在 MainActivity 的 loadPlugins 中装载（docs/plan-task-plugin-agent-tools.md）
    implementation(project(":live2dai-plugin-agent-tools"))

    // 主动陪伴插件（V1.2 模块6，docs/plan-v12-plan.md §5.6）：
    // WorkManager 周期检查「距上次对话 + 静默时段 + 冷却」→ 主动话题（LLM + 记忆）→
    // host.sendAssistantMessage（经 onAssistantMessage → voiceIo.speak 播报）+ 通知栏通知。
    // core 注入 GreetingLlmClient 实现（ProactiveGreetingClient，复用当前 LLM Provider）
    // 与 memoriesProvider（ProactiveMemorySource，读记忆插件 Room 库）。
    implementation(project(":live2dai-plugin-proactive"))

    // Room runtime：ProactiveMemorySource 直接读取记忆插件（live2dai-plugin-memory）的
    // MemoryDatabase（RoomDatabase 子类）；memory 模块以 implementation 引入 Room，
    // 不对外暴露编译类路径，此处需显式声明（版本与 memory 模块一致）
    implementation("androidx.room:room-runtime:2.6.1")

    // Kotlin Serialization (for API JSON parsing)
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")

    // Jetpack Compose (Material3)
    implementation(platform("androidx.compose:compose-bom:2024.12.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")

    // Kotlin Coroutines
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")

    // OkHttp for HTTP/SSE streaming
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.squareup.okhttp3:logging-interceptor:4.12.0")

    // SnakeYAML for parsing persona.yaml
    implementation("org.yaml:snakeyaml:2.0")

    // Live2D Cubism SDK (local AAR — 放在 app/libs/ 目录下)
    // 从 Live2D 官网下载 Cubism SDK for Android，放入 libs/ 目录即可自动生效
    // 如果未下载 SDK，此行不会报错（fileTree 在目录为空时自动跳过）
    // 代码在运行时会检测 native 库是否可用，不可用时自动降级到占位渲染
    // sherpa-onnx 引擎 AAR（V1.2 模块3/4，sherpa-onnx-1.13.4.aar）同样经此自动引入：
    // 缺失时条件源码集不编译（见上方 sherpa-onnx 条件源码集段），构建仍通过。
    implementation(fileTree("libs") { include("*.aar") })

    // Core Android
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")

    // Testing
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.mockito:mockito-core:5.14.0")
    testImplementation("org.mockito.kotlin:mockito-kotlin:5.4.0")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.robolectric:robolectric:4.14.1")
    testImplementation("androidx.test:core:1.6.1")
    // Compose UI 测试（Robolectric 本地运行 ModelSelectorTest）
    testImplementation(platform("androidx.compose:compose-bom:2024.12.01"))
    testImplementation("androidx.compose.ui:ui-test-junit4")
    testImplementation("androidx.compose.ui:ui-test-manifest")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation(platform("androidx.compose:compose-bom:2024.12.01"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-tooling")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
