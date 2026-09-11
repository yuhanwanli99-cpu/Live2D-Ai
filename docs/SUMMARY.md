# Live2D-Ai — 项目总结文档

> 更新日期: 2026-07-22
> 版本: v1.1.0 (MVP ✅)

---

## 项目目标

打造一款跨平台的 **Live2D + AI + TTS 最小验证应用**，同时运行在 Windows 桌面和 Android 手机上，验证「文本/语音 → LLM → 表情/口型/动作」闭环，提供：

- **实时语音对话**：说话即聊，语音回复
- **Live2D 模型渲染**：模型在屏幕上活起来
- **表情联动**：根据对话情感自动切换表情 + 嘴型同步
- **视觉识别**：模型能看到屏幕内容
- **Always-on**：Live2D-Ai 应用模式（Windows 透明穿透）/ 手机悬浮窗

---

## 架构概览

**两端独立直连** — 两端分别直连 DeepSeek API，共享 `shared/persona.yaml` 人设。

```
┌──────────────────────┐     ┌──────────────────────────┐
│ Windows Desktop      │     │ Android                  │
│                      │     │                          │
│ Open-LLM-VTuber      │     │ Kotlin + Compose         │
│ ├── DeepSeek API     │     │ ├── DeepSeek API (OkHttp)│
│ ├── Edge TTS         │     │ ├── Edge TTS             │
│ ├── GLM-4V 视觉      │     │ ├── GLM-4V 视觉          │
│ └── Live2D Web       │     │ └── Live2D (Purism Core) │
│                      │     │                          │
│ 配置: conf.yaml      │     │ 配置: assets/persona.yaml│
│ 人设: shared/        │     │ 人设: shared/            │
└──────────────────────┘     └──────────────────────────┘
```

### 关键里程碑

| 阶段 | 时间 | 架构 | 状态 |
| ------ | ------ | ------ | ------ |
| 初版 | 06/2026 | Gateway (FastAPI) + Bridge (WebSocket) + 两个前端 | ❌ 已废弃 |
| Wave 0 | 07/2026 | 两端独立直连 LLM API + Cubism SDK AAR | ✅ 基础可用 |
| **Purism Core 迁移** | **07/2026** | **Purism Core (MIT) + CubismJavaFramework** | **✅ MVP 验收** |

---

## 子任务完成状态

### ✅ Wave 0 — 基础设施 + Android 原生开发

| 任务 | 状态 | 说明 |
| ------ | ------ | ------ |
| Android 项目框架 | ✅ 完成 | Kotlin + Compose + Gradle 8.7 |
| Live2D 渲染（Cubism SDK） | ✅ 完成 | 反射桥接，三级降级 |
| 情感系统 | ✅ 完成 | [emotion] 标签解析 → 表情映射 |
| Edge TTS | ✅ 完成 | 降级链: Edge TTS → 系统 TTS → 无语音 |
| 语音输入 | ✅ 完成 | Android SpeechRecognizer |
| 视觉识别 | ✅ 完成 | GLM-4V 截图分析 |
| 人设配置 | ✅ 完成 | SnakeYAML 解析 shared/persona.yaml |
| Desktop 独立直连 | ✅ 完成 | start.ps1 + conf.yaml 配置 |
| 共享配置 | ✅ 完成 | shared/ 目录作为单源真理 |
| 单元测试 | ✅ 完成 | 4个测试类覆盖核心模块 |
| 文档整理 | ✅ 完成 | PLAN.md, HANDOVER.md, SUMMARY.md |

### ✅ Wave 1 — 渲染管线重写（Purism Core 迁移）

| 任务 | 状态 | 说明 |
| ------ | ------ | ------ |
| 拉取官方着色器 | ✅ 完成 | 从 CubismJavaFramework 拉取着色器（Live2D Open Software License，不可改授为 Apache/MIT） |
| 提取 Mask Premake 算法 | ✅ 完成 | 输出 docs/architecture/mask-premake-algorithm.md |
| 重写 Live2DRenderer | ✅ 完成 | Purism Core + CubismJavaFramework + 正确 FBO mask |
| 编译部署+视觉验证 | ✅ 完成 | 真机截图验证，修复 7 个 Bug |

### Wave 2+ — 增强

| 任务 | 优先级 | 说明 |
| ------ | -------- | ------ |
| Android 悬浮窗 | 中 | 后台 Service + 悬浮窗权限 |
| 多模型切换 UI | 中 | Compose UI 模型选择器 |
| 触摸互动 | 低 | 触碰反馈、待机动画 |
| MCP 工具集成 | 低 | Android 端调用 MCP 工具 |
| 对话气泡 | 低 | Canvas overlay 文本渲染 |
| 发布打包 | 中 | APK 签名、Windows 安装包 |

---

## Purism Core 迁移（渲染管线重写）

### 为什么

| | 旧方案（Cubism SDK AAR） | 新方案（Purism Core） |
| --- | -------------------------- | ---------------------- |
| 原生层 | Cubism Core AAR（闭源） | **Purism Core** (MIT, GitHub) |
| Java层 | 反射调用 (~600行) | **CubismJavaFramework** (直接API) |
| 许可 | 需注册 Live2D 账号 | 无需许可 |
| 集成 | 手动下载 AAR 放 libs/ | CMake + NDK 自动编译 |

### 修复的 Bug（7个，详见 HANDOVER.md §4）

| # | Bug | 触发场景 |
| --- | ----- | --------- |
| 1 | `buildPartIndex()` 晚于动画系统初始化 | 角色永远 4 只手 |
| 2 | Pose 在 `model.update()` 之前执行 | Pose 效果被覆盖 |
| 3 | `initParameters()` 强行覆盖所有 part opacity | 后肢/尾部不该显示的部分全开 |
| 4 | Pose JSON 解析错误 | Pose 初始化永远失败 |
| 5 | Pose 全开不隐藏 | 组内所有 part 都被设 1.0 |
| 6 | Pose 修改 Java 副本而非原生内存 | setPartOpacity 不生效 |
| 7 | `modelPtr()` 返回 0L | 动画系统全部走空指针 |

---

## MVP 验收数据 (2026-07-22)

| 检查项 | 结果 |
| -------- | ------ |
| 渲染稳定性 (4帧差异) | 71K~99K px/帧 → ✅ 动画流畅 |
| 4 手 bug | PartArmLB=0.0, PartArmRB=0.0 → ✅ 已修复 |
| GLM-4.6V 视觉确认 (2次) | 2 只手, 2 只手 → ✅ |
| 启动耗时 | 1.0s → ✅ |
| 运行时异常 | 0 crashes, 0 ANR → ✅ |
| 旧版 vs 新版像素 | 22,633 减少 → ✅ 后臂隐藏 |
| APK 大小 | 278KB vs 320KB (旧版) → ✅ 减少41KB |

---

## 快速启动命令

### Windows 桌面端

```powershell
cd Live2D-Ai\desktop
.\start.ps1
# 浏览器访问 http://localhost:12393/
```

### Android 端

```bash
cd Live2D-Ai\Live2D-Ai-Android
./gradlew assembleDebug
# 部署: adb install app/build/outputs/apk/debug/app-debug.apk
```

### 运行测试

```bash
cd Live2D-Ai-Android && ./gradlew test
cd Live2D-Ai && python -m pytest tests/
```

---

## 文件结构

```
Live2D-Ai\
├── AGENT.md              ← AI 辅助开发指南
├── AGENTS.md             ← SPOQ 编排规则
├── PLAN.md               ← 架构规划
├── HANDOVER.md            ← ★ 交接文档（首读）
├── shared/                ← ★ 跨平台共享配置
│   ├── persona.yaml       ← 人设（单源真理）
│   ├── mcp_tools.json     ← MCP 工具定义
│   └── model_dict.json    ← 模型路由表
├── Live2D-Ai-Android/        ← ★ Android 原生应用
│   ├── app/src/main/java/.../  ← Kotlin 源码 (14 个文件)
│   └── app/src/main/cpp/      ← Purism Core C 源码
├── Live2D-Ai-pc/               ← ★ Windows 桌面端
│   ├── start.ps1
│   └── open-llm-vtuber/   ← 上游项目
├── docs/                  ← 文档
├── tests/                 ← 测试
├── scripts/               ← 辅助脚本
├── tts/                   ← 独立 TTS 模块
└── archive/               ← 旧架构文件归档
```

---

## 技术栈

| 组件 | 技术 | 版本 |
| ------ | ------ | ------ |
| Android UI | Jetpack Compose + Material3 | BOM 2024.12 |
| LLM API | DeepSeek Chat (OpenAI 兼容) | deepseek-v4-flash |
| 视觉模型 | GLM-4V (Zhipu) | glm-4.6v |
| Live2D 原生 | Purism Core (MIT) | latest |
| Live2D Java | CubismJavaFramework | develop |
| TTS | Edge TTS (本地) | — |
| 桌面端 | Open-LLM-VTuber (Python + React) | latest |
| 构建 | Gradle + Kotlin DSL + AGP 8.7.3 | 8.7 |
| NDK | 27.0.12077973 | CMake |
| 最低 SDK | Android 8.0 (API 26) | — |
| 源码总计 | ~5,782 行 Kotlin/C++ | (不含上游) |

---

*详细配置见 `HANDOVER.md`，架构说明见 `PLAN.md`，渲染算法见 `docs/architecture/renderer-algorithm.md`。*
