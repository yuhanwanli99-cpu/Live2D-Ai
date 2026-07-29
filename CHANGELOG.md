# Live2Dai — 变更日志

> 日期: 2026-07-14  
> 任务: SPOQ Wave 0 — 桌面端配置精简 + Android 部署探索

---

## 修改文件清单

### ✅ task-win-trim: 桌面端配置精简 + 启动优化

| # | 文件 | 操作 | 说明 |
| --- | ------ | ------ | ------ |
| 1 | `desktop/open-llm-vtuber/conf.yaml` | **重写** | 从 ~230 行精简到 ~94 行，删除 27 个不用的 provider 配置块 |
| 2 | `desktop/start.ps1` | **增强** | 新增自动检测 Python/venv/ffmpeg/Electron/API Key + 环境摘要面板（~280 行） |
| 3 | `docs/plan-task-win-trim.md` | **新增** | 软件架构师设计文档 |
| 4 | `docs/plan-task-android-deploy.md` | **新增** | 软件架构师设计文档 |
| 5 | `.pi/spoq-state.json` | **更新** | Phase=done, Wave 0 状态记录 |
| 6 | `android-native/local.properties` | **新增** | API Keys 配置（DeepSeek + Zhipu） |

### 📋 task-android-deploy: 暂停 (blocked)

| 阻断项 | 详情 |
| -------- | ------ |
| 🔴 Live2D SDK AAR | 需人工从 [Live2D 官网](https://www.live2d.com/sdk/download/native/) 下载并放入 `app/libs/` |
| 🔴 vivo 安全拦截 | 安装 APK 时需在手机上手动勾选安全确认 |
| 🟡 RECORD_AUDIO 权限 | `MainActivity.kt` 需添加运行时权限请求代码（`docs/plan-task-android-deploy.md` §4 有完整方案） |

---

## conf.yaml 精简明细

**删除项（27 个配置块）：**

| 类别 | 删除内容 |
| ------ | --------- |
| 未用 LLM (9) | ollama_llm, claude_llm, gemini_llm, groq_llm, mistral_llm, openai_llm, lmstudio_llm, llama_cpp_llm, stateless_llm_with_template |
| 未用 ASR (6) | azure_asr, whisper_cpp, whisper, fun_asr, sherpa_onnx_asr, groq_whisper_asr |
| 未用 TTS (10) | azure_tts, melo_tts, x_tts, gpt_sovits_tts, fish_api_tts, siliconflow_tts, sherpa_onnx_tts, spark_tts, openai_tts, minimax_tts |
| 翻译 (2) | deeplx, tencent |

**保留项 (10)：** system_config, character_config (含 agent_config + llm_configs×2 + asr_config×1 + tts_config×1 + vad_config×1 + tts_preprocessor), vision_config, mcp_servers

---

## start.ps1 新增功能

| 功能 | 说明 |
| ------ | ------ |
| Python 自动检测 | 检测 ≥3.10，缺失时提示 winget install |
| 虚拟环境自动创建 | 优先 uv sync，降级 python -m venv |
| 依赖自动安装 | pip install -e . 后验证 import open_llm_vtuber |
| ffmpeg 检测 | 仅提示，不阻塞启动 |
| Electron 前端检测 | 检查 3 个常见安装路径 |
| API Key 状态解析 | 正则解析 conf.yaml 中 DeepSeek/智谱 密钥状态 |
| 环境摘要面板 | 彩色 ✅/❌ 展示各项状态 |
| 新参数 | -SkipEnvCheck / -SkipFrontendCheck |

---

## 未完成项（留给下一次）

1. **Android 真机部署** — 下载 Live2D Cubism SDK AAR → 放置到 `app/libs/` → 添加 RECORD_AUDIO 权限 → 构建 → 手机手动安装
2. **端到端验证** — 两端 120fps 实测、语音对话全链路、API Key 可用性测试
3. **桌面端 Python 后端实际启动测试** — 确认精简后的 conf.yaml 通过 Pydantic 验证

---

## 经验教训

1. **vivo 安全拦截**: 安装调试 APK 到 vivo/OPPO 设备时需在手机上手动确认安全弹窗，adb 无法自动绕过
2. **Live2D SDK 手动依赖**: Cubism Core 不在公开 Maven/GitHub，必须从官网下载并接受许可
3. **Open-LLM-VTuber Pydantic 安全**: 所有 provider 字段均为 Optional，YAML 中删除不会导致验证失败
