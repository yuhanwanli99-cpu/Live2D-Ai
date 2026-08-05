# Live2Dai — 架构规划

> 更新日期: 2026-08-05 | 当前架构: **V1 确立 — 两端独立直连**

---

## 架构演进摘要

- **Plan A (2026-06)**: Gateway + Bridge 三层架构 → 统一后端管理 AI + 路由
- **Plan B (2026-07)**: 桌面端直连 API + Bridge 保留 → 去掉 Gateway 层
- **当前 (2026-07+)**: **两端独立直连** → 桌面/Android 分别直连 LLM API，共享 `shared/persona.yaml` 配置
- **V1 确立 (2026-08-05)**: 「文本输入 → LLM → Live2D 做出相关回应（表情/动作）」双端验收 PASS（Android 真机 + PC :12393）。核心链路 = 文本 → LLM（零配置 GLM-4.7-Flash 兜底 / DeepSeek 升级路径）→ `[emotion]` 标签 → Live2D 表情切换。验收顺带修复 2 个 P0（commit `d2f1904`：表情链路接线回归 + LazyColumn key 哈希碰撞崩溃）。

---

## 当前架构

```
┌─────────────────────────────────┐     ┌─────────────────────────────────┐
│   Windows Desktop               │     │   Android (Kotlin/Native)       │
│                                 │     │                                 │
│   Open-LLM-VTuber (Electron)    │     │   ChatService (OkHttp SSE)      │
│   ├── run_server.py :12393      │     │   ├── DeepSeek API 直连         │
│   ├── DeepSeek API 直连         │     │   ├── Edge TTS 本地合成         │
│   ├── Edge TTS 本地合成         │     │   ├── Live2D GLSurfaceView      │
│   └── Live2D 模型渲染           │     │   ├── Emotion → LipSync         │
│                                 │     │   └── VisionService (GLM-4V)    │
│   配置: conf.yaml               │     │                                 │
│   └── 内联人设 + API Key        │     │   配置: persona.yaml (shared/)  │
└─────────────────────────────────┘     └─────────────────────────────────┘
                                            │
                                     ┌──────┴──────┐
                                     │  shared/     │ ← 单源真理
                                     │ persona.yaml │
                                     │ mcp_tools    │
                                     └─────────────┘
```

**关键变化**:
- ❌ 移除 `live2dai` Gateway (FastAPI :4869) 中间层
- ❌ 移除 `bridge.py` (WebSocket :8010) 桥接层
- ✅ 两端分别直连 DeepSeek API
- ✅ `shared/persona.yaml` 作为人设单源真理
- ✅ Android 端全 Kotlin 原生 (Compose + GLSurfaceView)

---

## Wave 0 完成变更

| 模块 | 变更 | 文件 |
|------|------|------|
| Android 框架 | 独立原生 Android 项目 (Kotlin/Compose) | `Live2Dai-Android/` |
| Live2D 渲染 | Cubism SDK 集成 (反射桥接，三级降级) | `Live2DRenderer.kt`, `Live2DView.kt` |
| 情感系统 | [emotion] 标签 → 表情映射 + LipSync | `EmotionController.kt` |
| 语音系统 | Edge TTS + TTS降级链 | `EdgeTtsService.kt` |
| 语音输入 | Android SpeechRecognizer 集成 | `VoiceInputService.kt` |
| 视觉服务 | GLM-4V 截图分析 | `VisionService.kt` |
| 人设配置 | SnakeYAML 解析, 跨平台共享 | `PersonaConfig.kt`, `shared/` |
| 桌面端 | 独立直连启动脚本, conf.yaml 配置 | `Live2Dai-pc/start.ps1` |
| 测试 | Android 单元测试 (5个) | `app/src/test/` |
| 文档 | 架构文档, 交接文档 | `docs/SUMMARY.md`, `HANDOVER.md` |

---

## 启动方式

### Windows 桌面端

```powershell
cd Live2Dai-pc/open-llm-vtuber
# 首次: git clone + uv sync (见 Live2Dai-pc/README.md)
.\start.ps1
# 启动 Open-LLM-VTuber.exe → 右键切换桌宠模式
```

### Android 端

```bash
# 用 Android Studio 打开 Live2Dai-Android/ 目录
# 构建运行到设备/模拟器
./gradlew installDebug
```

### 快速验证

```bash
# 桌面端: 确认 WebSocket 端口
curl http://localhost:12393/health

# Android: 直接在设备上打开应用即可
```

---

## 配置文件说明

| 文件 | 用途 | 位置 |
|------|------|------|
| `shared/persona.yaml` | 猫娘人设（单源真理） | `shared/persona.yaml` |
| `conf.yaml` | 桌面端 Open-LLM-VTuber 配置 | `Live2Dai-pc/open-llm-vtuber/conf.yaml` |
| `persona.yaml` (root) | 根目录摘要（指向 shared/） | `persona.yaml` |
| **API Key 配置** | 见下方 | |

### API Key 设置

**桌面端**: 编辑 `Live2Dai-pc/open-llm-vtuber/conf.yaml`
```yaml
llm_configs:
  openai_compatible_llm:
    base_url: 'https://api.deepseek.com/v1'
    llm_api_key: 'sk-你的Key'       # ← 在此填写
```

**Android 端**: 自动读取 `shared/persona.yaml` 中的 model 字段, OKHttp 通过代码配置 API Key
- 如需覆盖: 在 `Live2Dai-Android/app/src/main/java/com/live2dai/android/ChatService.kt` 中修改 `DEEPSEEK_API_KEY`

---

## 已知问题

1. Desktop conf.yaml 内联了人设 prompt，与 `shared/persona.yaml` 存在同步风险
2. Android Live2D SDK 需手动从官网下载 AAR 放入 `app/libs/`
3. Android TTS 降级链优先级：Edge TTS → 系统 TTS → 无语音
4. Wave 1 的 Android Live2D LipSync + Emotion 集成已完成但需端到端验证

---

*详细交接文档见 `HANDOVER.md`，项目总结见 `docs/SUMMARY.md`*
