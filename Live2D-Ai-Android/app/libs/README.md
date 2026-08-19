# Live2D Cubism SDK for Android

将 Cubism 4 SDK for Android 的 AAR 文件放入此目录即可自动生效。

## 下载方式

1. 访问 [Live2D 官方网站](https://www.live2d.com/sdk/download/native/)
2. 下载 Cubism 4 SDK for Android (R1 或更新版本)
3. 解压后将 `live2d-cubism-core-release.aar` 复制到此目录

## 降级行为

如果 SDK 未安装，应用不会崩溃。Live2D 渲染区域会显示一个动态渐变色背景，
指示 Live2D 模型区域的位置。

## 文件要求

- 文件名: `live2d-cubism-core-release.aar` (或其他 .aar 文件)
- 包含: Java/Kotlin 包装类 + 各架构的 .so 文件 (arm64-v8a, armeabi-v7a, x86_64)
