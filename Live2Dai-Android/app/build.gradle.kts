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

        buildConfigField("String", "DEEPSEEK_API_KEY", "\"${deepseekKey}\"")
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
        unitTests.all {
            // 383+ 单测含多个 Robolectric 测试，默认 512MB fork 堆会 OOM（verify-M9-followup P0-1）
            it.maxHeapSize = "2048m"
        }
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

// ═══════════════════════════════════════════════════════════════════
//  Pre-build: 复制 shared/model-adapter/niziiro_mao.adapter.json + bai.adapter.json → app assets
//  适配配置（contract-v3-adapter §1.1/§1.4，T1.2）是双端唯一运行时适配真相源，构建时同步到
//  src/main/assets/*.adapter.json，运行时由 ModelAdapterConfig 从 assets 读取。
//  注意：Gradle 根项目是 Live2Dai-Android/，shared/ 在其父目录（仓库根）下。
// ═══════════════════════════════════════════════════════════════════
val adapterSourceDir = rootProject.projectDir.parentFile.resolve("shared/model-adapter")
val adapterTargetDir = file("src/main/assets")
val adapterTargetFile = file("src/main/assets/niziiro_mao.adapter.json")
val baiAdapterTargetFile = file("src/main/assets/bai.adapter.json")

val copyModelAdapter = tasks.register<Copy>("copyModelAdapter") {
    group = "live2dai"
    description = "将 shared/model-adapter/niziiro_mao.adapter.json + bai.adapter.json 复制到 app/src/main/assets/（Live2D 模型适配配置）"
    from(adapterSourceDir)
    include("niziiro_mao.adapter.json", "bai.adapter.json")
    into(adapterTargetDir)
}

// ═══════════════════════════════════════════════════════════════════
//  Pre-build: 复制 shared/persona.yaml → app assets
//  persona（shared/persona.yaml，contract-v3-features §2，T4.2）是双端 persona 单源真相，
//  构建时同步到 src/main/assets/persona.yaml，运行时由 assets 读取（C23 从"校验"升级为"构建强制同步"）。
//  注意：Gradle 根项目是 Live2Dai-Android/，shared/ 在其父目录（仓库根）下。
// ═══════════════════════════════════════════════════════════════════
val personaSourceFile = rootProject.projectDir.parentFile.resolve("shared/persona.yaml")
val personaTargetFile = file("src/main/assets/persona.yaml")

val copyPersona = tasks.register<Copy>("copyPersona") {
    group = "live2dai"
    description = "将 shared/persona.yaml 复制到 app/src/main/assets/（双端 persona 单源同步）"
    from(personaSourceFile)
    into(file("src/main/assets"))
}

// ═══════════════════════════════════════════════════════════════════
//  Pre-build: 复制 shared/mcp_tools.json → app assets
//  MCP 工具定义（shared/mcp_tools.json）是双端共享源，构建时同步到 assets。
// ═══════════════════════════════════════════════════════════════════
val mcpToolsSourceFile = rootProject.projectDir.parentFile.resolve("shared/mcp_tools.json")

val copyMcpTools = tasks.register<Copy>("copyMcpTools") {
    group = "live2dai"
    description = "将 shared/mcp_tools.json 复制到 app/src/main/assets/（MCP 工具定义同步）"
    from(mcpToolsSourceFile)
    into(file("src/main/assets"))
}

// 挂到 preBuild / variant / unitTest 前置任务（contract-v3-adapter §1.4 锁死）：
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
        dependsOn(copyModelAdapter)
        dependsOn(copyPersona)   // contract-v3-features §2（T4.2）：persona 单源同步
        dependsOn(copyMcpTools)  // contract-shared-v0 SM-03：mcp_tools 单源同步
    }
}

// clean 时一并删除生成的注册表与适配配置（assets 位于 src/ 下，Gradle clean 不会自动清理）
tasks.named("clean") {
    doLast {
        delete(registryTargetFile)
        delete(adapterTargetFile)
        delete(baiAdapterTargetFile)
    }
}

dependencies {
    // Wave 5 清理：插件模块（plugin-sdk/live2dai-plugin-vision/-memory/-proactive/-agent-tools）
    // 已随删除清单移除（settings.gradle include + 本依赖 + Room 一并删除）。

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

// Live2D Cubism native 由 CMake 编译 Purism Core（MIT，app/src/main/cpp/）提供，与 libs/ AAR 无关
// （spoq-fix G7：历史“AAR 缺失→降级占位”表述已移除——liblive2dcubismcore.so 由 NDK 构建打包进 APK）
// libs/ 下仅 sherpa-onnx 引擎 AAR（V1.2 模块3/4，sherpa-onnx-1.13.4.aar）经此自动引入：
// 缺失时条件源码集不编译（见上方 sherpa-onnx 条件源码集段），构建仍通过。
implementation(fileTree("libs") { include("*.aar") })

    // Core Android
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    // Testing
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.mockito:mockito-core:5.14.0")
    testImplementation("org.mockito.kotlin:mockito-kotlin:5.4.0")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.robolectric:robolectric:4.14.1")
    // org.json 真实实现覆盖 mockable-android.jar 的 stub（EmotionController.parseStructured 依赖，verify-M9-followup P0-2）
    testImplementation("org.json:json:20231013")
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
