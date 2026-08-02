# Live2Dai 🐱

> **Live2D AI 伴侣** — 用开源 Purism Core 替代闭源 Cubism SDK 的全栈 Android 猫娘。
>
> **Live2D AI Companion** — The first MIT full-stack open-source Android Live2D companion. Purism Core replaces the proprietary Cubism SDK.

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License MIT"/>
  <img src="https://img.shields.io/badge/platform-Android-brightgreen" alt="Platform Android"/>
  <img src="https://img.shields.io/badge/language-Kotlin-blueviolet" alt="Language Kotlin"/>
  <img src="https://img.shields.io/badge/Live2D-Purism%20Core-orange" alt="Live2D Purism Core"/>
  <img src="https://img.shields.io/badge/LLM-DeepSeek%20%7C%20GLM--4V-blue" alt="LLM"/>
  <img src="https://img.shields.io/badge/status-MVP%20%F0%9F%94%B0-success" alt="MVP ✅"/>
</p>

---

## 📖 项目简介 | Overview

**Live2Dai** 是一款运行在 Android 手机上的 **猫娘 AI 伴侣**。她使用 **Live2D 模型**（猫娘角色）与您进行 **语音对话**，通过 **大语言模型 (LLM)** 理解并回应您，同时根据对话情感实时切换表情。

> "主人好呀喵~！今天想让我做什么呢？(=^･ω･^=)" —— 小喵

**核心亮点**：本项目是**首个 MIT 全栈开源**的 Live2D AI 伴侣方案。我们用 [Purism Core](https://github.com/SakuraMotion/PurismCore)（MIT 开源许可）替代了 Live2D 官方的闭源 Cubism SDK，无需注册 Live2D 账号即可编译运行。

同时提供 **Windows 桌面端**（基于 [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber)）作为扩展平台。

---

## ✨ 特性 | Features

### 🎭 Live2D 渲染（开源核心）

- Purism Core (MIT) 替代闭源 Cubism Core，完全开源
- 完整的 OpenGL ES 2.0 渲染管线：Premultiplied Alpha、裁剪掩码、MSAA 4x
- 三线性过滤 + Mipmap，呈现光滑边缘
- 支持 Cubism 5.0 MOC3 模型格式

### 🗣️ 语音对话

- **说话即聊**：Android SpeechRecognizer 语音输入 → DeepSeek-v4-flash LLM 处理
- **Edge TTS 语音合成**（降级链：Edge TTS → 系统 TTS → 静默）
- 对话历史上下文保持

### 👁️ 视觉识别

- 猫娘能看到屏幕内容：GLM-4.6V 多模态截图分析
- 主人可以问"看我在做什么"，猫娘会描述画面

### 😊 情感联动

- LLM 回复中的 `[emotion]` 标签（`[joy]`, `[sadness]`, `[anger]` 等）
- 猫娘说话时实时切换表情和语气
- 完整的眨眼、呼吸、物理效果（辫子摆动）

### 🏠 桌面端同步

- Windows 桌面端基于 Open-LLM-VTuber，支持 120fps 渲染
- 共享 `shared/persona.yaml` 人设文件，两端人设统一
- 猫娘桌宠模式（透明穿透窗口）

### 🚀 计划中 | Roadmap

- [ ] Android 悬浮窗模式（后台 Service + 悬浮窗权限）
- [ ] 多模型切换 UI
- [ ] 触摸互动反馈
- [ ] 对话气泡 Canvas 渲染
- [ ] MCP 工具集成

---

## 📸 截图 | Screenshots

> <!-- TODO: 替换为真实截图 Replace with actual screenshots -->

| Android 端 | Windows 桌面端 |
|-----------|---------------|
| `[screenshot]` | `[screenshot]` |

| 猫娘小喵 | 对话界面 |
|---------|---------|
| `[screenshot]` | `[screenshot]` |

---

## 🚀 快速开始 | Quick Start

### 环境要求 | Prerequisites

| 工具 | 版本 | 用途 |
| ------ | ------ | ------ |
| Android Studio / Gradle | 8.7+ | Android 构建 |
| JDK | 17+ | Kotlin 编译 |
| NDK | 27.0.12077973 | Purism Core C 编译（自动下载） |
| Android SDK | API 35 (compileSdk) | 目标平台 |
| Android 手机 | API 26+ (Android 8.0+) | 运行设备 |

### 构建 APK | Build

```bash
# 克隆项目
git clone https://github.com/your-org/Live2Dai.git
cd Live2Dai/android-native

# 构建 Debug APK（NDK 会自动下载）
./gradlew assembleDebug

# APK 路径
# app/build/outputs/apk/debug/app-debug.apk
```

### 部署到手机 | Deploy

```bash
# 无线 ADB 连接（替换为你的手机 IP）
adb connect 192.168.0.x:5555

# 卸载旧版本（某些系统必须完整卸载）
adb uninstall com.live2dai.android

# 安装
adb install app/build/outputs/apk/debug/app-debug.apk

# 启动
adb shell am start -n com.live2dai.android/.MainActivity
```

> **注意**：vivo / OPPO 等系统会在安装时弹出安全验证弹窗，需在手机上手动确认。

### Windows 桌面端

```bash
cd Live2Dai-pc
.\start.ps1
# 浏览器访问 http://localhost:12393/
```

### 运行测试 | Run Tests

```bash
# Android 端测试
cd android-native && ./gradlew test

# Windows 端测试
cd .. && python -m pytest tests/
```

---

## 🏗️ 架构 | Architecture

### 整体架构

**两端独立直连 LLM API**，无中心化后端，共享 `shared/persona.yaml` 人设配置：

```
┌─────────────────────────────┐     ┌─────────────────────────────────┐
│   Android (Kotlin + Compose)│     │   Windows Desktop               │
│                             │     │                                 │
│   DeepSeek API ─────────────┼─────┼─── DeepSeek API                 │
│        (OkHttp SSE)         │     │                                 │
│                             │     │   Edge TTS                      │
│   Edge TTS ─────────────────┼─────┼───                              │
│                             │     │                                 │
│   GLM-4.6V 视觉 ────────────┼─────┼─── GLM-4.6V 视觉               │
│                             │     │                                 │
│   Purism Core ──────────────┤     │   Live2D Web (Cubism SDK)      │
│   (原生 C 渲染引擎)          │     │                                 │
│                             │     │   Open-LLM-VTuber (Python)      │
│   共享人设: persona.yaml ────┼─────┼─── 共享人设: persona.yaml        │
└─────────────────────────────┘     └─────────────────────────────────┘
```

### 渲染管道 (每帧)

```
onDrawFrame():
  1. AnimationPipeline.update(deltaTime)
     ├─ MotionManager      ← 当前播放动作
     ├─ EyeBlinkManager    ← 自然眨眼
     ├─ BreathController   ← 胸部起伏
     ├─ PhysicsManager     ← 物理效果（辫子等）
     ├─ model.update()     ← 第一次更新
     ├─ Pose.update()      ← 隐藏后臂/肢体重叠
     └─ model.update()     ← 第二次更新（Pose 生效）
  2. refreshCachedAttributes()
  3. buildDrawableList()   ← 按 renderOrder 排序
  4. drawObjectLoop()
     ├─ 裁剪掩码 FBO（需要时）
     ├─ 设置着色器参数
     └─ glDrawElements
```

### 配置流

```
shared/persona.yaml  ← 单源真理 (Single Source of Truth)
    ├──→ Android: Live2Dai-Android/app/src/main/assets/persona.yaml
    └──→ Desktop:  Live2Dai-pc/conf.yaml
```

---

## 💻 技术栈 | Tech Stack

### Android 端

| 组件 | 技术 | 版本 |
| ------ | ------ | ------ |
| UI 框架 | Jetpack Compose + Material3 | BOM 2024.12 |
| 语言 | Kotlin | 2.1 |
| LLM API | DeepSeek Chat (OpenAI 兼容) | deepseek-v4-flash |
| 视觉模型 | GLM-4V (Zhipu) | glm-4.6v |
| Live2D 原生 | **Purism Core** (MIT) | latest |
| Live2D Java | CubismJavaFramework | develop |
| 语音合成 | Edge TTS（降级链） | — |
| 语音识别 | Android SpeechRecognizer | API 26+ |
| 构建 | Gradle + Kotlin DSL + AGP | 8.7.3 |
| NDK | 27.0.12077973 | CMake |
| 最低 SDK | Android 8.0 | API 26 |

### Windows 桌面端

| 组件 | 技术 |
| ------ | ------ |
| 运行时 | Python 3.10+ (FastAPI + WebSocket) |
| LLM | DeepSeek API |
| 视觉 | Zhipu GLM-4.6V |
| TTS | Edge TTS |
| ASR | Faster Whisper (large-v3-turbo) |
| VAD | Silero VAD |
| Live2D | Cubism SDK for Web |
| 前端 | React + ChakraUI + Vite |

---

## 🎨 自定义指南 | Customization

### 换模型 | Change Model

1. 将 Live2D 模型文件夹（含 `.model3.json` + `.moc3` + 纹理图片）放入：

   ```bash
   Live2Dai-Android/app/src/main/assets/live2d/<model-name>/runtime/
   ```

2. 修改 `Live2DRenderer.kt` 中的模型加载路径：

   ```kotlin
   // 在 loadModel() 中
   private val modelPath = "live2d/<model-name>/runtime/"
   ```

3. 重新编译

   ```bash
   ./gradlew assembleDebug
   ```

### 换人设 | Change Persona

编辑 `shared/persona.yaml`（单源真理，两端自动同步）：

```yaml
name: "你的角色名"
role: "角色设定"
system_prompt: |
  你是...（角色描述）

model: "deepseek-v4-flash"
temperature: 0.8
```

同步到 Android：

```bash
cp shared/persona.yaml Live2Dai-Android/app/src/main/assets/
```

> 人设支持 `[neutral]`, `[joy]`, `[anger]`, `[sadness]`, `[fear]`, `[surprise]`, `[smirk]` 七个情感标签。

### 换 LLM | Change LLM

项目使用 OpenAI 兼容 API 协议，更换 LLM 只需修改：

- **Android**：`ChatService.kt` 中的 `DEEPSEEK_API_KEY` 和 API endpoint URL
- **Windows**：`conf.yaml` 中的 `llm_api_key`

支持任意 OpenAI 兼容的 API（DeepSeek、GLM、通义千问、Ollama 本地等）。

---

## 🤝 贡献指南 | Contributing

欢迎贡献！请遵循以下流程：

1. **Fork** 本仓库
2. **创建** 特性分支：`git checkout -b feat/your-feature`
3. **提交** 改动：`git commit -m "feat: add your feature"`
4. **推送** 分支：`git push origin feat/your-feature`
5. **发起** Pull Request

### 开发约定

- 代码风格遵循 Kotlin 官方规范
- 所有新功能应包含对应的单元测试
- 保持 `shared/persona.yaml` 作为人设单源真理
- 修改渲染管线请参见 `HANDOVER.md` 中的 Bug 修复记录
- 禁止在提交中引入闭源二进制文件

### 测试

```bash
# Android 单元测试
cd android-native && ./gradlew test

# 代码风格检查
./gradlew lint
```

---

## 🧠 项目状态 | Project Status

**当前阶段：Phase 2 — 执行 & 状态机模式**

| 阶段 | 时间 | 状态 |
| ------ | ------ | ------ |
| 初版 (Gateway + Bridge 架构) | 2026-06 | ❌ 已废弃 |
| Wave 0 (独立直连 + Cubism SDK) | 2026-07 | ✅ 基础可用 |
| **Wave 1 (Purism Core 迁移)** | **2026-07** | **✅ MVP 验收通过** |
| Wave 2+ (悬浮窗/多模型/触摸) | _规划中_ | ⏳ |

### MVP 验收数据 (2026-07-22)

| 检查项 | 结果 |
| -------- | ------ |
| 渲染稳定性 (4帧差异) | 71K~99K px/帧 → ✅ 流畅动画 |
| 4 手 Bug | PartArmLB=0.0, PartArmRB=0.0 → ✅ 已修复 |
| 视觉确认 (GLM-4.6V) | 2 只手 → ✅ 正确 |
| 启动耗时 | 1.0s → ✅ |
| 运行时异常 | 0 crashes, 0 ANR → ✅ |
| APK 大小 | 278KB (对比旧版减少 41KB) |

---

## 📚 文档 | Documentation

| 文档 | 说明 |
| ------ | ------ |
| [HANDOVER.md](./HANDOVER.md) | 🔥 项目交接文档（必读） |
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | 架构详细说明 |
| [docs/SUMMARY.md](./docs/SUMMARY.md) | 项目总结 |
| [shared/persona.yaml](./shared/persona.yaml) | 猫娘人设（单源真理） |
| [docs/renderer-algorithm.md] | 渲染算法说明 |
| [docs/mask-premake-algorithm.md] | 裁剪掩码算法 |

---

## 📄 许可证 | License

**MIT License** © 2026 Live2Dai Contributors

本项目使用 MIT 许可证，您可自由使用、修改、分发（包括商业用途）。

### 第三方许可致谢 | Third-Party Licenses

| 组件 | 许可 | 说明 |
| ------ | ------ | ------ |
| [Purism Core](https://github.com/SakuraMotion/PurismCore) | **MIT** | Live2D 原生引擎核心 |
| [CubismJavaFramework](https://github.com/Live2D/CubismJavaFramework) | Apache 2.0 | Live2D Java 框架（仅着色器文件） |
| [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) | MIT | Windows 桌面端基础 |
| Jetpack Compose / Material3 | Apache 2.0 | Android UI 框架 |
| OkHttp | Apache 2.0 | HTTP 客户端 |
| SnakeYAML | Apache 2.0 | YAML 解析 |
| GLM-4V / DeepSeek API | 各自 API 协议 | LLM 与视觉模型服务 |

### 关于 Live2D 模型

本项目包含的 `.model3.json` / `.moc3` / 纹理文件（如 `mao_pro`、`shizuku`）为 Live2D 模型文件，其使用受 Live2D 许可协议约束。建议替换为你自己的模型。

---

## 💬 相关链接 | Links

- [Purism Core (MIT)](https://github.com/SakuraMotion/PurismCore) — Live2D 开源核心
- [CubismJavaFramework](https://github.com/Live2D/CubismJavaFramework) — Live2D Java 框架
- [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) — Windows 桌面端
- [DeepSeek API](https://platform.deepseek.com/) — LLM 服务
- [Zhipu API](https://open.bigmodel.cn/) — 视觉模型服务
- [Edge TTS](https://github.com/rany2/edge-tts) — 语音合成

---

<p align="center">
  Made with ❤️ and 🐱 (lots of cat ears)
  <br/>
  <sub>
    "主人好喵~！今天也要一起开心地度过喵！(=^･ω･^=)"
  </sub>
</p>
