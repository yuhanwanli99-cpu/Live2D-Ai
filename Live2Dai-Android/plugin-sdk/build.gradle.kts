plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// 纯 Kotlin/JVM module（不依赖 Android SDK）：
// - 插件与 core 之间只通过接口 + 数据类通信，不需要 Android Context 之外的任何东西
// - 未来若要复用到 Live2Dai-pc（Python 之外的 JVM 后端）也可以直接引用
java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
}
