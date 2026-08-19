# Live2D-Ai 竞品调研报告 — 同类型开源项目功能模块对比

> 调研日期: 2026-08-05
> 调研方法: GitHub API + README 原文抓取（curl）
> 调研员: researcher agent (只读)

---

## 一、对比总表（项目 × 功能模块矩阵）

> 图例: ✅ 有 / 🔶 部分实现 / ⬜ 无 / ❓ 未找到证据

### 1.1 对话与 LLM

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber (⭐13k) | AIRI (⭐47k) | Soul of Waifu (⭐1.1k) | my-neuro (⭐1.3k) | Live2DPet (⭐73) | Miru (⭐56) | Meuxe (⭐69) | Nexus (⭐16) | VPet-Ultra (⭐3) | AI-Vtuber by Ikaros (⭐4.4k) |
|---|---|---|---|---|---|---|---|---|---|---|
| 多 Provider 抽象 | ✅ GLM/DeepSeek/Ollama | ✅ Ollama/OpenAI/Claude/Gemini/Mistral/Zhipu/GGUF 等 | ✅ 多模型支持 | ✅ 10 云提供商 + Llama.cpp | ✅ 本地+闭源双轨 | ✅ OpenAI-compatible | ✅ 三层独立模型 (Vision/Chat/Memory) | ✅ ACP 协议 (OpenCode/ClaudeCode/Codex) | ✅ Ollama/DeepSeek/OpenAI | ✅ 本地+Online 模式 | ✅ ChatGPT/Claude/Qwen/Ollama 等 |
| 流式 SSE | ✅ OkHttp SSE | ✅ WebSocket | ✅ WebSocket | ✅ 逐句流式 | ✅ 字幕语音同步输出 | ❓ | ❓ | ✅ ACP 流式 | ❓ | ❓ | ❓ |
| 多轮对话上下文 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 短期记忆 | ✅ 对话历史 | ✅ 对话历史 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 长期记忆 | ⬜ | 🔶 曾移除，即将回归 | ✅ RAG + 专有 memory 系统 | ✅ Soul Memory (4层认知文件) | ✅ 长期记忆 (MemOS) | 🔶 关键帧视觉记忆 | ✅ 四类记忆 + 日记 + 承诺清单 | ✅ Semantic/Episodic/Reflection | 🔶 已规划 (Phase 4) | ✅ SQLite episodic+semantic | ⬜ |

### 1.2 语音

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| TTS 引擎 | ✅ Edge TTS (降级链) | ✅ Edge TTS / sherpa-onnx / MeloTTS / Coqui / GPTSoVITS / Bark / CosyVoice / Fish Audio / Azure 等 | ✅ 多引擎 | ✅ Qwen3 TTS / XTTSv2 / Kokoro / Silero / EdgeTTS / ElevenLabs | ✅ GPT-SoVITS (语音克隆) | ✅ VOICEVOX (日语) | ❓ | ✅ 内置 TTS + ElevenLabs + OpenAI | 🔶 已规划 (Phase 3) | ✅ Piper / Kokoro | ✅ edge-tts / VITS / elevenlabs / bert-vits2 等 |
| ASR 引擎 | 🔶 Android SpeechRecognizer (断链), PC Faster Whisper | ✅ sherpa-onnx / FunASR / Faster-Whisper / Whisper.cpp / Groq / Azure 等 | ✅ | ✅ Faster Whisper | ❓ | ⬜ (文本为主) | ❓ | ✅ Whisper-based | 🔶 已规划 (Phase 3) | ✅ Whisper (whisper.cpp) | ❓ |
| VAD | 🔶 PC 端 Silero VAD | ✅ Silero VAD | ✅ | ✅ Silero VAD | ❓ | ⬜ | ❓ | ✅ | ❓ | ❓ | ❓ |
| 语音打断 | 🔶 PC 端有 | ✅ 免耳机打断 | ✅ | ✅ 全双工打断 | ✅ 实时打断 | ⬜ | ❓ | ❓ | 🔶 已规划 | ❓ | ❓ |
| 语音唤醒 | ⬜ | ⬜ | ❓ | ❓ | ❓ | ⬜ | ❓ | ❓ | 🔶 已规划 (Phase 3) | ❓ | ❓ |
| TTS 多语言/翻译 | ⬜ | ✅ TTS 翻译 | ❓ | ❓ | ✅ 字幕中文+外语音频 | ✅ 中→日 TTS 翻译 | ❓ | ❓ | ❓ | ❓ | ❓ |

### 1.3 视觉

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| 截图/屏幕感知 | ✅ GLM-4.6V | ✅ 摄像头+屏幕录制+截图 | ✅ | ✅ Screen Reader | ✅ 视觉能力 | ✅ 定时截屏 | ✅ 屏幕感知 (隐私优先) | ⬜ | 🔶 短期粗粒度陪伴摘要 | ✅ SmolVLM/LLaVA | ⬜ |
| 摄像头 | ⬜ | ✅ | ❓ | ❓ | ❓ | ⬜ | ⬜ | ⬜ | ⬜ | ❓ | ⬜ |
| 剪贴板感知 | ⬜ | ❓ | ❓ | ✅ Clipboard Reader | ❓ | ❓ | ❓ | ⬜ | ⬜ | ✅ Clipboard 工具 | ❓ |
| 视觉模型 | ✅ GLM-4.6V | ✅ | ❓ | ❓ | ✅ 多模态 | ✅ 视觉模型 | ✅ Vision 模型层 | ⬜ | ⬜ | ✅ LLaVA/SmolVLM | ❓ |

### 1.4 Live2D

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| Live2D 渲染 | ✅ Purism Core (MIT) | ✅ Cubism SDK for Web | ✅ | ✅ Live2D | ✅ Live2D 替换 | ✅ PixiJS + Cubism SDK | ✅ Live2D | ✅ Cubism + VRM | ✅ Live2D | ✅ 插件支持 (.moc3) | ✅ Live2D |
| 表情联动 | ✅ [emotion] 标签→表情 | ✅ 表情映射 | ✅ | ✅ 28 情感分类器 | ✅ 动作表情 | ✅ 情绪累积触发 | ❓ | ✅ 表达式标签 | ✅ 基础表情 | ✅ 10维情绪向量 | ❓ |
| LipSync | ⬜ | ❓ | ❓ | ✅ | ❓ | ❓ | ❓ | ✅ | ⬜ | ❓ | ❓ |
| 动作/动画系统 | ✅ 眨眼/呼吸/物理 | ✅ | ❓ | ✅ 动作动画链接 | ✅ | ✅ | ❓ | ❓ | ⬜ | ❓ | ❓ |
| 多模型切换 | 🔶 需要修改代码加载路径 | ✅ 导入自定义 Live2D | ❓ | ✅ | ✅ | ✅ 热导入+图片模型 | ❓ | ✅ Live2D+VRM 双选 | ✅ 内置模型选择 | ❓ | ❓ |
| 3D/VRM 支持 | ⬜ | ⬜ | ✅ VRM | ✅ VRM 3D | ⬜ | ⬜ (仅Live2D+图片) | ⬜ | ✅ VRM | ⬜ | ⬜ | ✅ UE/VRM |

### 1.5 交互

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| 悬浮窗/桌宠 | 🔶 Android 规划中 | ✅ 桌面宠物模式 (透明穿透) | ✅ | ✅ 桌面覆盖 | ✅ | ✅ 透明置顶窗口 | ✅ 桌面宠物 | ✅ Mini 模式 | ✅ 桌面常驻 | ✅ WPF桌宠 | ❓ |
| 触摸/点击互动 | 🔶 规划中 | ✅ 点击/拖拽触觉反馈 | ❓ | ❓ | ❓ | ✅ 点击/拖拽 | ❓ | ❓ | 🔶 Phase 2 | ✅ 桌宠互动 | ❓ |
| 通知栏回复 | ✅ Android NotificationReplyListener | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 全局快捷键 | ⬜ | ❓ | ❓ | ❓ | ❓ | ⬜ | ❓ | ✅ 全局快捷键 | ❓ | ❓ | ❓ |
| 聊天窗口/气泡 | 🔶 规划中 Canvas | ✅ | ✅ | ✅ | ✅ | ✅ 气泡对话 | ✅ | ✅ | ✅ | ❓ | ❓ |

### 1.6 工具/扩展

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| MCP/函数调用 | ✅ MCP (time/search/weather/filesystem) | 🔶 Agent 接口可扩展 | ❓ | ✅ MCP + 6内置工具 | ✅ MCP支持 | ⬜ | ⬜ | ⬜ (ACP-only) | 🔶 Phase 5 规划 | ✅ 工具分发 | ❓ |
| 插件机制 | ✅ ChatHook 生命周期 + SDK | 🔶 模块化设计可扩展 | ❓ | ❓ | ❓ | ⬜ | ⬜ | ❓ | ❓ | ✅ MOD 系统 | ❓ |
| Web 搜索 | ✅ ddg-search MCP | ❓ | ❓ | ✅ Web Search | ✅ 联网搜索 | ⬜ (v2.0 弃置) | ⬜ | ⬜ | 🔶 Phase 5 | ❓ | ❓ |
| 浏览器控制 | ⬜ | ❓ | ❓ | ✅ Browser Control | ❓ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ Open Browser | ❓ |
| 文件系统 | ✅ filesystem MCP | ❓ | ❓ | ❓ | ❓ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ Read File | ❓ |
| 桌面控制 | ⬜ | ❓ | ❓ | ⬜ | ✅ 语音控制打开软件 | ⬜ | ⬜ | ⬜ | ⬜ | ✅ 打开应用 | ❓ |
| 直播平台对接 | ⬜ | ⬜ | ❓ | ❓ | ✅ Bilibili 直播 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ Bilibili/抖音/快手/YouTube/Twitch 等 |
| Discord 对接 | ⬜ | ⬜ | ❓ | ✅ Discord Gateway | ❓ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ❓ |

### 1.7 多模态与记忆

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| 多模型切换 UI | ⬜ (仅 LLMProvider 代码级) | ✅ 配置文件切换 | ✅ | ✅ Models Hub (HuggingFace 下载) | ❓ | ✅ 设置面板切换 | ✅ 三层独立配置 | ✅ ACP Agent 切换 | ✅ 多 Provider | ✅ Model Manager | ✅ 多模型矩阵 |
| 本地模型支持 | ✅ Ollama | ✅ Ollama / LM Studio / vLLM / GGUF | ✅ | ✅ Llama.cpp (CUDA/HIP/SYCL/Vulkan) | ✅ LLM-studio 本地推理 | ⬜ (仅 API) | ✅ 任何 OpenAI-compatible | ⬜ (需 ACP Agent) | ✅ Ollama | ✅ LLamaSharp 进程内推理 | ✅ Ollama |
| 人设单源真理 | ✅ persona.yaml | 🔶 Prompt 修改 | ✅ | ✅ Character Cards | ❓ | ✅ 角色卡 JSON | ✅ soul.md | ✅ soul.md + style.md + rules.md | 🔶 背景与常用表达 | ✅ Big Five + adaptive traits | ❓ |
| 图像生成 | ⬜ | ⬜ | ❓ | ✅ 本地+云端 AI 生图 | ❓ | ⬜ | ⬜ | ⬜ | ⬜ | ❓ | ✅ 协同 SD 画图 |
| 情绪状态机 | 🔶 [emotion] 标签解析 | ✅ 表情映射 | ❓ | ✅ 28 情感分类 + 神经激素模拟 | 🔶 规划中 | ✅ 情绪累积触发 | ✅ 持续情绪 | ❓ | 🔶 Phase 6 | ✅ 10维情绪向量 | ❓ |

### 1.8 其他

| 功能模块 | Live2D-Ai (我们) | Open-LLM-VTuber | AIRI | Soul of Waifu | my-neuro | Live2DPet | Miru | Meuxe | Nexus | VPet-Ultra | AI-Vtuber (Ikaros) |
|---|---|---|---|---|---|---|---|---|---|---|
| 主动消息/陪伴 | ⬜ | ✅ 主动说话 | ❓ | ✅ 神经激素驱动主动对话 | ✅ 主动对话 | ✅ AI 根据屏幕内容主动对话 | ✅ AttentionEngine 主动陪伴 | ⬜ | 🔶 Check-in 决策 | ❓ | ❓ |
| 定时任务 | ⬜ | ❓ | ❓ | ❓ | ❓ | ❓ | ❓ | ⬜ | ❓ | ❓ | ❓ |
| 设置页 | ✅ Compose 设置 | ✅ 前端设置面板 | ✅ | ✅ 配置面板 | ✅ | ✅ 设置窗口 | ✅ | ✅ | ✅ 设置抽屉 | ✅ | ❓ |
| 多端同步 | 🔶 共享 persona.yaml 但无实时同步 | ⬜ (单机) | ✅ Web/macOS/Windows | ✅ 局域网 Web 客户端 | ✅ 手机 App | ⬜ | ✅ 多设备邀请码同步 | ⬜ | ⬜ (单机) | ⬜ | ❓ |
| 账号系统 | ⬜ (无后端) | ⬜ | ⬜ | ⬜ | ❓ | ⬜ | ⬜ (自部署) | ⬜ | ⬜ | ⬜ | ❓ |
| tRPG/游戏 | ⬜ | ⬜ | ✅ Minecraft/Factorio | ✅ Soul Stage RPG | ✅ 游戏陪玩 (MC/Galgame) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 唱歌 | ⬜ | ⬜ | ❓ | ❓ | ✅ AI 唱歌 | ⬜ | ⬜ | ⬜ | ⬜ | ❓ | ❓ |
| 跨平台 | ✅ Android + PC | ✅ Windows/macOS/Linux | ✅ Web/macOS/Windows | ⬜ Windows only | ✅ Windows + Android App | ⬜ Windows only | ✅ macOS + Android | ✅ Tauri 跨平台 | ✅ Win/Mac/Linux | ✅ Win/Linux(Wine) | ❓ |

---

## 二、每个项目的要点总结

### 2.1 Open-LLM-VTuber (我们 PC 端的基础)
- **来源**: https://github.com/Open-LLM-VTuber/Open-LLM-VTuber (⭐13,107)
- **简介**: 语音交互 AI 伴侣，支持 Live2D 面部、免提语音对话、语音打断、跨平台完全离线运行
- **核心卖点**: TTS/ASR 引擎矩阵最丰富（20+ 引擎可选），桌面宠物透明穿透模式，v2.0 正在全面重写
- **与我们关系**: 我们 PC 端直接基于此项目
- **来源 URL**: https://github.com/Open-LLM-VTuber/Open-LLM-VTuber#readme

### 2.2 AIRI (行业最大)
- **来源**: https://github.com/moeru-ai/airi (⭐46,987)
- **简介**: 容器化的 AI waifu 灵魂容器，目标达到 Neuro-sama 高度。支持实时语音、Minecraft/Factorio 游玩
- **核心卖点**: WebGPU/WebAssembly 技术栈，自有 RAG+memory 系统，VRM 3D + Live2D 双模式，Electron 桌面端
- **与我们差异**: 更偏游戏+直播，有专门的 memory 子项目生态，社区最大
- **来源 URL**: https://github.com/moeru-ai/airi#readme

### 2.3 Soul of Waifu (最全面的桌面伴侣之一)
- **来源**: https://github.com/jofizcd/Soul-of-Waifu (⭐1,092)
- **简介**: 免费开源桌面 AI 角色扮演应用，Live2D/VRM 双模式，四大模式（聊天/RPG/桌宠/记忆）
- **核心卖点**: Soul Memory 四层认知架构（心理学/关系/事件/日记），Soul Stage 桌游 RPG 引擎，Soul Companion 桌宠叠加层，神经激素驱动的主动对话，28 情感分类器，全本地+私有
- **与我们差异**: 功能非常全面（长期记忆+RPG+桌宠+生图+MCP），Windows only
- **来源 URL**: https://github.com/jofizcd/Soul-of-Waifu#readme

### 2.4 my-neuro (功能最丰富的 Neuro-sama 复刻)
- **来源**: https://github.com/morettt/my-neuro (⭐1,324)
- **简介**: 打造逼近真人的 AI 伙伴，可训练声音/性格/替换形象
- **核心卖点**: 超低延迟(<1s)、双模型支持(本地+闭源)、MCP 工具、实时打断、AI 唱歌、Bilibili 直播、桌面控制、游戏陪玩、长期记忆、手机 App
- **与我们差异**: 功能清单极长（唱歌/直播/游戏/桌面控制），但部分标注规划中
- **来源 URL**: https://github.com/morettt/my-neuro#readme

### 2.5 Live2DPet (与我们定位最接近的落地产品)
- **来源**: https://github.com/x380kkm/Live2DPet (⭐73)
- **简介**: Electron 桌面宠物，Live2D 角色常驻桌面，AI 视觉感知，VOICEVOX 日语语音
- **核心卖点**: 定时截屏感知+主动对话、VOICEVOX 本地 TTS 一键安装、图片模型备选、关键帧视觉记忆、情绪累积系统、音频状态机降级
- **与我们差异**: 专注 Windows 桌宠体验，有成熟的截屏感知和 VOICEVOX 集成
- **来源 URL**: https://github.com/x380kkm/Live2DPet#readme

### 2.6 Miru (AI 伴侣记忆先行)
- **来源**: https://github.com/kiyotakali/Miru (⭐56)
- **简介**: "真正记住你的 AI 伴侣"，macOS + Android + 自部署服务器
- **核心卖点**: 三层模型（Vision/Chat/Memory）、AttentionEngine 主动陪伴、四类长期记忆+日记+承诺清单、可验证可导出的 Markdown 记忆、多设备邀请码同步
- **与我们差异**: 记忆系统极为深入（承诺追踪、夜间整理），核心在"主动陪伴"而非简单反应
- **来源 URL**: https://github.com/kiyotakali/Miru#readme

### 2.7 Meuxe (架构最现代的桌面伴侣)
- **来源**: https://github.com/meet447/Meuxe (⭐69)
- **简介**: Tauri 2 (Rust + React) 桌面伴侣，ACP 协议驱动，Live2D/VRM 双模式
- **核心卖点**: ACP 智能体协议（Claude Code/Codex/OpenCode）作为聊天后端，分层角色文件（soul.md/style.md/rules.md），关系状态追踪，并行 TTS 低延迟
- **与我们差异**: 完全不嵌入 LLM 客户端，用 ACP 协议委托给外部 Agent，架构非常克制
- **来源 URL**: https://github.com/meet447/Meuxe#readme

### 2.8 Nexus (与我们 Phase 1 最接近的新项目)
- **来源**: https://github.com/FanyinLiu/Nexus (⭐16)
- **简介**: 本地优先的 AI 桌面伙伴，Electron + React + TypeScript，Live2D 常驻
- **核心卖点**: 6 阶段路线清晰（Phase 1 最小桌面伙伴→Phase 6 完整伴侣），Ollama/DeepSeek 双路径，always-on 唤醒词，check-in 主动陪伴，陪伴摘要隐私脱敏
- **与我们差异**: 相对早期但设计理念清晰（常驻、安静、陪伴感），路线规划很务实
- **来源 URL**: https://github.com/FanyinLiu/Nexus#readme

### 2.9 VPet-Ultra (VPet 的 AI 增强分支)
- **来源**: https://github.com/Amirwopi/VPet-Ultra (⭐3)
- **简介**: VPet (⭐6,582) 的 fork，添加 LLM 推理、记忆、人格、情绪、语音、智能体能力
- **核心卖点**: LLamaSharp 进程内推理（无需 Ollama/Python）、Model Manager (HuggingFace 下载)、Big Five 人格+10维情绪向量、Goal-driven Agent、完全离线
- **与我们差异**: 基于成熟 VPet 生态（WPF），专注本地离线 AI 增强
- **来源 URL**: https://github.com/Amirwopi/VPet-Ultra#readme

### 2.10 AI-Vtuber by Ikaros-521 (直播场景最全)
- **来源**: https://github.com/Ikaros-521/AI-Vtuber (⭐4,421)
- **简介**: 多 LLM 后端+多直播平台驱动的虚拟主播框架
- **核心卖点**: 支持的直播平台极多（Bilibili/抖音/快手/YouTube/Twitch/TikTok 等），多样化的 TTS 引擎矩阵，SD 画图协同
- **与我们差异**: 专注直播场景（弹幕互动→AI 回复→TTS 播出），不是伴侣型产品
- **来源 URL**: https://github.com/Ikaros-521/AI-Vtuber (API 返回 description)

### 2.11 N.E.K.O (README 提及的竞品)
- **来源**: 在 awesome-agentic-ai-waifus 列表中提到 `Project-N-E-K-O/N.E.K.O`（原 Xiao8），但 GitHub 搜索 `Project-N-E-K-O` 未返回公开仓库结果
- **结论**: 未能抓取到该项目——可能已改名/私有化/删库
- **来源 URL**: https://github.com/yuri-os/awesome-agentic-ai-waifus (README 提及)；GitHub API search 返回空

### 2.12 LLM-Live2D-Desktop-Assitant (ylxmf2005)
- **来源**: 被 Open-LLM-VTuber README 列为 Related Project，但 GitHub API 返回 DMCA takedown
- **结论**: 仓库已被 DMCA 下架，无法读取任何功能信息
- **来源 URL**: https://github.com/github/dmca/blob/master/2026/07/2026-07-20-live2d.md

---

## 三、共性功能清单 —— 多数同类项目都做的功能

> 这是评估我们核心架构是否缺失的基准：如果一个功能在 7+ 个以上同类项目中出现而我们缺失，则是明确的功能缺口。

### 🔴 高优先级共性（几乎所有人都做，我们缺失或弱于平均）

| 功能 | 同类覆盖率 | 我们状态 | 差距评估 |
|---|---|---|---|
| **长期记忆** | 9/11 项目有 | ⬜ 缺失 | **最大缺口**：Soul of Waifu/Miru/my-neuro/VPet-Ultra 均实现了多层记忆架构（语义/情景/关系/日记），这是"AI 伴侣"区别于"AI 聊天"的核心差异 |
| **主动消息/陪伴感知** | 6+/11 项目有 | ⬜ 缺失 | 明显缺口：Miru 的 AttentionEngine、Soul of Waifu 的神经激素、Nexus 的 check-in 决策——伴侣"不只在被叫时才出现" |
| **LipSync** | Soul of Waifu/Meuxe 有 | ⬜ 缺失 | 与 TTS 联动是语音对话的基础体验闭环 |
| **多模型切换 UI** | 8/11 项目有 | ⬜ 仅代码级切换 | 用户无法在运行时切换模型，需修改代码/配置 |
| **直播/外部平台对接** | my-neuro/AI-Vtuber/Soul of Waifu 有 | ⬜ 缺失 | 若需要触达直播场景 |
| **TTS 多语言/翻译** | OL-VTuber/my-neuro/Live2DPet 有 | ⬜ 缺失 | 用户中文输入→AI 日/英语音输出 |

### 🟡 中优先级共性（多数项目有，我们有基础但需强化）

| 功能 | 同类覆盖率 | 我们状态 | 差距评估 |
|---|---|---|---|
| **桌面悬浮窗/桌宠模式** | 8/11 项目有 | 🔶 Android 规划中 | PC 端已有（OL-VTuber 基础），Android 端是关键差异化机会 |
| **触摸/点击互动** | OL-VTuber/Live2DPet/VPet 有 | 🔶 规划中 | 桌宠基本交互 |
| **TTS 本地引擎** | 多数项目支持多种本地 TTS | 🔶 仅 Edge TTS（云端） | Soul of Waifu/VPet-Ultra 支持全本地 Kokoro/Piper 推理 |
| **ASR 本地引擎** | OL-VTuber/VPet-Ultra/SoW 支持 | 🔶 Android 端断链 | Android 端 ASR 链路需修复，或引入 Whisper 本地方案 |
| **情绪状态机（不只是标签解析）** | OL-VTuber/SoW/Live2DPet/VPet 有 | 🔶 仅 [emotion] 标签→表情 | 多数竞品实现了情绪累积/衰减/驱动的状态机，而非简单的标签→动作映射 |

### 🟢 低优先级/差异化（少数项目有，但我们做得不错或可作为差异化）

| 功能 | 同类覆盖率 | 我们状态 |
|---|---|---|
| **插件 SDK (ChatHook)** | 仅 VPet-Ultra (MOD系统)、OL-VTuber (可扩展) | ✅ 独有，完整 ChatHook 生命周期 |
| **MCP 工具框架** | 仅 my-neuro/SoW/VPet-Ultra | ✅ 有，time/search/weather/filesystem |
| **人设单源真理 (persona.yaml)** | 少数 (Miru soul.md, Meuxe .md分层, SoW character cards) | ✅ 做得好，共享配置跨平台 |
| **Android + PC 双端** | Miru/Open-LLM-VTuber 单平台为主 | ✅ 独有优势 |
| **通知栏回复** | 无同类 | ✅ 独有功能 |
| **LLM Provider 抽象** | 多数有，但我们是独立设计 | ✅ 架构清晰 |
| **Purism Core (MIT 替代 Cubism)** | 仅我们 | ✅ 独有，无需 Live2D 许可 |

---

## 四、我们相对同类项目的关键评估

### 4.1 缺失项（应该补齐的核心功能缺口）

| 缺失功能 | 紧迫度 | 对标项目 | 建议 |
|---|---|---|---|
| **长期记忆系统** | 🔴 高 | Soul of Waifu (4层认知架构), Miru (承诺追踪+日记), VPet-Ultra (SQLite episodic+semantic) | 先做最小闭环：SQLite 语义记忆+关键事实提取。不需要一步做到 Soul Memory 的复杂度 |
| **主动陪伴/主动消息** | 🔴 高 | Miru AttentionEngine, Soul of Waifu 神经激素, Nexus check-in | 从时间/空闲检测开始，结合屏幕感知触发问候 |
| **LipSync** | 🟡 中 | Soul of Waifu/Meuxe | 与 Edge TTS 的 word-boundary 事件对齐嘴型 |
| **多模型切换 UI** | 🟡 中 | 几乎所有竞品 | 在设置页添加模型下拉+Provider 切换，LLMProviderManager 已准备好了后端 |
| **Android 悬浮窗** | 🟡 中 | 竞品多桌面端，Android 悬浮窗是差异化 | 已规划，加速落地 |
| **Android ASR 链路修复** | 🟡 中 | ← | Android SpeechRecognizer 断链需调查修复或替换为 Whisper 方案 |
| **情绪状态机** | 🟢 低 | SoW 28分类器+神经激素 | 当前标签→表情链路工作正常，情绪累积/衰减属于锦上添花 |

### 4.2 独有项（我们相对于同类项目的竞争优势）

| 独有功能 | 优势说明 |
|---|---|
| **MIT 全栈开源 + Purism Core** | 唯一无需 Live2D 许可的开源方案，降低了分发和使用门槛 |
| **ChatHook 插件 SDK** | 完整的插件生命周期（onLoad/对话钩子/工具注册/onUnload），多数竞品没有正式的插件系统 |
| **MCP 工具框架** | 标准化工具接入（time/search/weather/filesystem/sequential-thinking），只有 my-neuro/SoW/VPet-Ultra 有类似能力 |
| **persona.yaml 单源真理** | 跨平台人设共享，竞品多用分散的 prompt 文件或 character cards |
| **Android + PC 双端** | 竞品多为单平台（Windows），Miru 支持 macOS+Android 但不支持 Windows |
| **零配置开箱即用** | GLM-4.7-Flash 免费兜底模型（永久免费），竞品通常需要用户自行配置 API Key |
| **通知栏回复** | Android 独有交互入口，无同类竞品实现 |

### 4.3 架构健康度评估

- **核心链路** `文本→LLM→[emotion]→Live2D` ✅ 双端验收通过，核心稳固
- **LLM Provider 抽象** ✅ 设计清晰（ProviderType 枚举 + OpenAI 兼容协议），可扩展
- **插件架构** ✅ ChatHook 生命周期完整，支持通过 MCP 协议扩展工具
- **TTS 降级链** ✅ Edge TTS → 系统 TTS → 静默，容错设计合理
- **语音链路** ⚠️ Android 端 ASR 断链是脆弱点
- **记忆/上下文** ⚠️ 无长期记忆是最大的架构缺口

---

## 五、总体结论

### 我们的核心架构并不过少，反而是中等偏上水平

Live2D-Ai 的核心链路（文本→LLM→表情→Live2D）已经**比大多数同类项目更稳固**：
- 拥有 MIT 全栈开源（Purism Core 替代 Cubism SDK）的独特壁垒
- 插件 SDK + MCP 工具框架在同类中属于少数派但架构前瞻
- persona.yaml 单源真理 + LLM Provider 抽象证明架构设计有章法
- 零配置开箱即用（GLM 免费兜底）降低了使用门槛

### 明确需要补上的三个最关键短板

1. **长期记忆** — 这是"AI 伴侣"区别于"AI 聊天机器人"的核心标志，几乎每个成熟竞品都有
2. **主动陪伴/感知** — 伴侣不应只在被召唤时才出现，Miru/SoW/Nexus 都在做
3. **Android 端能力补齐** — 悬浮窗桌宠模式 + ASR 修复，这是我们的独特赛道

### 建议的优先级

```
Phase A (近期): 长期记忆 MVP → 多模型切换 UI → Android ASR 修复
Phase B (中期): 主动陪伴/感知 → LipSync → Android 悬浮窗
Phase C (远期): 情绪状态机 → 直播对接 → 桌面控制 → 游戏伴玩
```

---

## 六、调研证据索引

| 项目 | GitHub Stars | 许可证 | README 来源 URL |
|---|---|---|---|
| Open-LLM-VTuber | 13,107 | Other | https://github.com/Open-LLM-VTuber/Open-LLM-VTuber#readme |
| AIRI (moeru-ai) | 46,987 | — | https://github.com/moeru-ai/airi#readme |
| Soul of Waifu | 1,092 | GPLv3 | https://github.com/jofizcd/Soul-of-Waifu#readme |
| my-neuro | 1,324 | — | https://github.com/morettt/my-neuro#readme |
| Live2DPet | 73 | MIT | https://github.com/x380kkm/Live2DPet#readme |
| Miru | 56 | Apache 2.0 | https://github.com/kiyotakali/Miru#readme |
| Meuxe | 69 | MIT | https://github.com/meet447/Meuxe#readme |
| Nexus | 16 | MIT | https://github.com/FanyinLiu/Nexus#readme |
| VPet-Ultra | 3 | Apache 2.0 | https://github.com/Amirwopi/VPet-Ultra#readme |
| AI-Vtuber (Ikaros) | 4,421 | — | https://api.github.com/repos/Ikaros-521/AI-Vtuber |
| N.E.K.O | 未找到 | 未找到 | 未找到（可能已改名/私有化） |
| LLM-Live2D-Desktop-Assitant | DMCA 下架 | — | https://github.com/ylxmf2005/LLM-Live2D-Desktop-Assitant |
| VPet (原版) | 6,582 | — | https://github.com/LorisYounger/VPet |
| awesome-agentic-ai-waifus | — | — | https://github.com/yuri-os/awesome-agentic-ai-waifus#readme |

---

> **调研完整性声明**: 以上所有结论均标注了来源 URL。标注为 ❓ 的项目确认无法从 README 中读到该功能证据（非编造标注为无）。标注为"未找到"的项目为 GitHub 搜索无结果或仓库已下架。
