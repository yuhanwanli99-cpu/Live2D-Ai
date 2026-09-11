# Live2D-Ai 项目架构文档

> 更新时间: 2026-08-05 | 架构版本: **v0（开发中）** | 旧"V1"验收数据见下方历史记录

---

## 历史里程碑（v0 阶段记录）

> 以下为旧 V1 验收基线，仅作 v0 阶段历史记录保留，**不代表当前 v1 达成**。

- **旧 V1 定义**：文本输入 → LLM → Live2D 做出相关回应（表情/动作），可正常对话即达成。
- **Android 真机验收 PASS**：输入文本 → LLM 回复 `[joy]` → `EmotionController: setEmotion: joy -> expressionIndex=5` → 表情切换；多轮对话零崩溃。
- **PC 验收 PASS**：WS text-input → LLM 回复 `[joy]`/`[anger]` → `audio.actions.expressions=[3]/[2]` → 前端 setExpression → 表情切换；会话闭环正常。
- **验收修复**：commit `d2f1904`（表情链路接线回归 + LazyColumn key 哈希碰撞崩溃），91 单测全过。

---

## 项目定位

**Live2D + AI 最小验证项目** — 验证最小化 AI+Live2D 闭环（文本/语音 → LLM → 表情/口型/动作），留下扩展接口供后续和社区开发 mod。Windows 桌面端 + Android 手机端，两端独立直连云端 API，无中心化后端。当前版本: v0（作者自测，不对外发布）。

---

## 目录结构

```
Live2D-Ai\
├── shared/                      ← 跨平台共享配置（单源真理）
│   ├── persona.yaml             ← 人设（name/system_prompt/model/temperature）
│   ├── mcp_tools.json           ← MCP 工具定义（time/ddg-search/weather/...）
│   └── model_dict.json          ← 模型路由表
│
├── Live2D-Ai-pc/                     ← Windows 桌面端
│   ├── 启动Live2D-Ai.bat        ← 一键启动入口
│   ├── start.ps1                ← PowerShell 启动脚本（自动检测环境）
│   └── open-llm-vtuber/         ← Open-LLM-VTuber（后端+前端）
│       ├── conf.yaml            ← 精简配置（deepseek+zhipu+edgetts+fasterwhisper）
│       ├── run_server.py        ← 入口 → FastAPI + WebSocket
│       ├── frontend/            ← 预编译 React Web 静态文件
│       └── src/open_llm_vtuber/ ← Python 后端源码
│
├── Live2D-Ai-Android/              ← Android 原生端
│   ├── app/src/main/
│   │   ├── cpp/                 ← Purism Core 原生层
│   │   ├── java/com/live2d/ai/android/  ← Kotlin 源码
│   │   ├── assets/live2d/       ← Live2D 模型文件
│   │   └── res/
│   └── build.gradle.kts
│
└── docs/                        ← 文档
```

---

## 技术栈

### Windows 桌面端

| 组件 | 技术 |
| ------ | ------ |
| 运行时 | Python 3.10+ (FastAPI + WebSocket) |
| LLM | DeepSeek API (deepseek-v4-flash) |
| 视觉 | Zhipu GLM-4.6V |
| TTS | Edge TTS |
| ASR | Faster Whisper (large-v3-turbo) |
| VAD | Silero VAD |
| Live2D | Web 前端渲染（Cubism SDK for Web） |
| 前端 | React + ChakraUI + Vite（预编译静态文件） |
| 端口 | localhost:12393 |
| 帧率 | 120fps |

### Android 端

| 组件 | 技术 |
| ------ | ------ |
| UI | Jetpack Compose + Material3 |
| 语言 | Kotlin 2.1 |
| LLM | DeepSeek API (SSE streaming, OkHttp) |
| TTS | Edge TTS (降级链: Edge→系统TTS→静默) |
| ASR | Android SpeechRecognizer |
| 视觉 | GLM-4.6V |
| Live2D 原生层 | **Purism Core** (MIT开源, Cubism Core替代) |
| Live2D Java层 | **CubismJavaFramework** (公开GitHub) |
| OpenGL | GLSurfaceView + 自定义 renderer |
| 编译 | AGP 8.7.3 + NDK 27.0.12077973 (CMake) |
| minSdk / targetSdk | 26 / 35 |

---

## Live2D 渲染方案对比

| | 原始方案 | 当前方案 |
| ------ | ---------- | ---------- |
| 原生层 | Cubism Core AAR (需Live2D许可) | **Purism Core** (MIT, GitHub) |
| Java层 | 反射调用 (~600行) | **CubismJavaFramework** (直接API) |
| 集成 | 手动下载AAR | CMake+NDK 自动编译 |
| 许可 | 需注册Live2D账号 | 无需许可 |

---

## 配置流

```
shared/persona.yaml ──→ Live2D-Ai-pc/open-llm-vtuber/conf.yaml (load_persona_from_file)
                    ──→ Live2D-Ai-Android/app/src/main/assets/persona.yaml (复制)

shared/mcp_tools.json ──→ Live2D-Ai-pc/open-llm-vtuber/mcp_servers.json
                      ──→ Live2D-Ai-Android/app/src/main/assets/mcp_tools.json

shared/model_dict.json ──→ Live2D-Ai-pc/open-llm-vtuber/model_dict.json
                       ──→ Live2D-Ai-Android/app/src/main/assets/model_dict.json
```

---

## 已知问题

1. **Android Live2D 渲染**: 模型已加载但角色显示有黑色遮挡，需修复 OpenGL 渲染管线
2. **Pi tester agent**: ~~需要 `~/.pi/agent/models.json` 配置 zhipu provider~~（zhipu provider 已废弃移除，不再需要）
3. **API Key 管理**: Android 端通过 local.properties (gitignored)，Windows 端在 conf.yaml 中
4. **MCP 工具**: 仅桌面端可用，Android 端待集成
