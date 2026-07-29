# Live2Dai — 项目总览

> 更新日期: 2026-07-22
> 项目定位: **通用 Live2D 桌宠** — Live2D 角色 + LLM 语音对话
> 当前架构: **两端独立直连**（无中心化后端）

---

## 快速导航

| 文档 | 说明 |
| ------ | ------ |
| **`HANDOVER.md`** | **★ 必读** — 完整项目交接（架构、Bug修复记录、动画系统、构建部署） |
| **`PLAN.md`** | 架构演进历史（Plan A/B/C → 当前直连架构） |
| **`AGENTS.md`** | SPOQ 编排规则（Phase 0/1/2 流程） |
| `docs/ARCHITECTURE.md` | 技术栈详表 |
| `docs/renderer-algorithm.md` | Cubism 渲染算法逆向分析 |
| `docs/mask-premake-algorithm.md` | Mask FBO 渲染算法 |
| `docs/verification-win.md` | 桌面端配置验证报告 |
| `android-native/app/libs/README.md` | Live2D SDK 依赖说明 |
| `.pi/lessons-learned.md` | 经验教训库（新手上路必读） |

---

## 当前架构

```
┌──────────────────────────────┐    ┌──────────────────────────────┐
│   Windows Desktop            │    │   Android                    │
│                              │    │                              │
│   Open-LLM-VTuber (Python)   │    │   Kotlin + Compose           │
│   ├── DeepSeek API 直连      │    │   ├── DeepSeek API (OkHttp)  │
│   ├── Edge TTS / GLM-4V     │    │   ├── Edge TTS                │
│   ├── Faster-Whisper ASR    │    │   ├── GLM-4V 视觉             │
│   └── Live2D Web 渲染       │    │   ├── Live2D (Purism Core)    │
│                              │    │   └── Emotion → LipSync      │
│   配置: conf.yaml            │    │                              │
│   人设: shared/persona.yaml  │    │   配置: assets/persona.yaml  │
└──────────────────────────────┘    └──────────────────────────────┘
                                        ↑
                                   ┌────┴────┐
                                   │ shared/ │ ← 单源真理
                                   └─────────┘
```

---

## 关键变更（相对于旧架构）

| 变更 | 旧 | 新 |
| ------ | ----- | ----- |
| 后端架构 | Gateway (FastAPI) + Bridge (WebSocket) 三层 | **两端独立直连** LLM API |
| Live2D 原生层 | Cubism Core AAR（闭源，需Live2D许可） | **Purism Core** (MIT开源) |
| Live2D Java层 | 反射调用 (~600行) | **CubismJavaFramework** (直接API) |
| 配置管理 | 分散在各端 | `shared/persona.yaml` **单源真理** |

---

## 项目结构

```
F:\Live2Dai\
├── shared/                  ← ★ 跨平台共享配置（单源真理）
│   ├── persona.yaml         ←   人设（编辑此文件影响两端）
│   ├── mcp_tools.json       ←   MCP 工具定义
│   └── model_dict.json      ←   模型路由表
│
├── android-native/          ← ★ Android 原生应用（主力平台）
│   └── app/src/main/
│       ├── cpp/             ←   Purism Core (MIT) 源码
│       ├── java/com/live2dai/android/  ← Kotlin 源码
│       └── assets/live2d/   ←   Live2D 模型
│
├── desktop/                 ← ★ Windows 桌面端
│   ├── open-llm-vtuber/     ←   Open-LLM-VTuber 后端
│   ├── start.ps1            ←   一键启动脚本
│   └── 启动桌宠.bat
│
├── docs/                    ← 文档
├── tests/                   ← 测试
├── scripts/                 ← 辅助脚本
├── archive/                 ← 旧架构文件归档
└── HANDOVER.md              ← ★ 交接文档（首读）
```

---

## 快速命令

```bash
# 编辑人设
vim shared/persona.yaml
# 同步到 Android
cp shared/persona.yaml android-native/app/src/main/assets/

# 构建 Android APK
cd android-native && ./gradlew assembleDebug

# 桌面端一键启动
cd desktop && ./start.ps1

# 运行测试
cd android-native && ./gradlew test
python -m pytest tests/
```

---

## 开发须知

1. **Purism Core** (<https://github.com/SakuraMotion/PurismCore>) MIT 许可，CMake+NDK 自动编译
2. **JNI 陷阱**: `getPartOpacities()` 返回 COPY，修改必须用 `setPartOpacity()` 写入
3. **动画更新顺序**: Motion → EyeBlink → Breath → Physics → **第一次 model.update()** → Pose → **第二次 model.update()**
4. **色器**: 7个 GLES 2.0 着色器，在 `assets/shaders/` 下
5. 详细 Bug 修复记录见 `HANDOVER.md` 第四章

---

## 经验教训

> 启动前务必阅读 `.pi/lessons-learned.md`

| 教训 | 说明 |
| ------ | ------ |
| vivo 安全拦截 | 安装 APK 需在手机上手动确认安全弹窗 |
| Purism Core 自动编译 | 无需手动下载 AAR，CMake 自动编译 |
| Pose 双 update | Pose 必须在第一次 model.update() 之后，并再跑一次 update |
