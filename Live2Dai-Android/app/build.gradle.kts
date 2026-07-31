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
        val zhipuKey = localProps.getProperty("ZHIPU_API_KEY")?.ifBlank { null }
            ?: (project.findProperty("ZHIPU_API_KEY") as? String)?.ifBlank { null }
            ?: System.getenv("ZHIPU_API_KEY")?.ifBlank { null }
            ?: ""

        buildConfigField("String", "DEEPSEEK_API_KEY", "\"${deepseekKey}\"")
        buildConfigField("String", "ZHIPU_API_KEY", "\"${zhipuKey}\"")
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

dependencies {
    // 插件 SDK：core 与插件之间的最小契约（Live2DaiPlugin/PluginHost/ChatHook/ToolDefinition）
    implementation(project(":plugin-sdk"))

    // 视觉插件：截图描述 hook + capture_screen 工具（从 VisionService.kt 迁出）
    implementation(project(":live2dai-plugin-vision"))

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
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation(platform("androidx.compose:compose-bom:2024.12.01"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-tooling")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
