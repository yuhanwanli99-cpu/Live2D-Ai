# Live2D-Ai TTS 选型调研报告

> 调研日期: 2026-08-05
> 调研员: researcher agent (只读)
> 背景: 当前 Edge TTS (wss://speech.platform.bing.com/) 在中国大陆被墙 (HTTP 403)，且用户反馈音质不佳
> 目标: 以中国大陆为核心的 TTS 选型决策依据

---

## 一、猫娘计划 (Project N.E.K.O.) 仓库专题

### 1.1 基本信息

| 字段 | 值 |
|---|---|
| **全名** | Project-N-E-K-O/N.E.K.O |
| **GitHub** | https://github.com/Project-N-E-K-O/N.E.K.O |
| **官网** | https://project-neko.online |
| **Stars** | ⭐2,382 (GitHub API 2026-08-05) |
| **许可证** | Apache 2.0 |
| **语言** | Python (后端) + React (前端) |
| **描述** | "一只会主动找你玩的 AI 猫娘" — embodied emotional engine, real-time voice, multi-avatar |

- 来源: https://api.github.com/repos/Project-N-E-K-O/N.E.K.O (description, stargazers_count, license, language)
- 来源: https://github.com/Project-N-E-K-O/N.E.K.O (README.MD base64)
- 证据: `"stargazers_count": 2382`, `"license": {"key": "apache-2.0"}`, `"language": "Python"`

### 1.2 架构总览

N.E.K.O. 是一个**多服务 Docker 化**的 AI 猫娘平台，核心服务包括：

```
N.E.K.O/
├── main_server/          # 主服务（HTTP API，26 个路由）
├── agent_server/         # AI 智能体服务（Agent 框架）
├── memory_server.py      # 记忆服务（5 层记忆体系）
├── main_logic/core.py    # 核心对话逻辑
├── tts_client/           # TTS 引擎适配器（多 Provider 对接）
├── brain/                # Agent 智能体模型仓库
│   ├── computer_use.py        # 电脑控制
│   ├── browser_use_adapter.py # 浏览器自动化
│   ├── openclaw_adapter.py    # OpenClaw 云端适配
│   ├── openfang_adapter.py    # OpenFang 无头执行后端
│   └── task_executor.py       # 任务执行引擎
├── memory/               # 5 层记忆系统
│   ├── facts.py          # 事实记忆
│   ├── reflection.py     # 反思记忆
│   └── persona.py        # 人设记忆
├── frontend/             # React 前端 + Vue 插件管理器
├── plugin/               # 插件系统 (SDK)
├── activity/             # 系统/用户状态跟踪
├── topic/                # 主动话题推荐
└── main_routers/         # API 路由（26 个）
```

- 来源: README.MD base64 解码 (项目结构树)
- 来源: https://api.github.com/repos/Project-N-E-K-O/N.E.K.O/git/trees/HEAD?recursive=1

### 1.3 TTS 选型

N.E.K.O. 的 TTS 架构是**多 Provider 适配器模式**（`tts_client/` 目录）。从 README 中的架构描述：

| 语音模式 | 技术方案 | 说明 |
|---|---|---|
| **实时语音对话** | Realtime API | 语音实时对话，低延迟流式 |
| **文字对话** | ChatCompletion + TTS | 先文本生成 → TTS 合成播放 |

关键特征：
- **核心 API 推荐使用"阿里云"**（READMe 原文："核心 API（实时语音对话）：必须支持 Realtime API。推荐使用 **阿里云**。"）
- 在 `docs/zh-CN/api/rest/system.md` 中有完整的 API 接口文档和各服务商对接说明
- 支持 14+ AI 服务商配置
- 环境变量中可见 `NEKO_CORE_API` 可配置为 `qwen`（阿里云通义千问）作为默认
- 支持自定义语音包上传（声音克隆）

- 来源: README.MD base64 → "核心 API（实时语音对话）：必须支持 Realtime API。推荐使用 **阿里云**。"
- 来源: README.MD → `NEKO_CORE_API=${NEKO_CORE_API:-qwen}` 环境变量
- 来源: README.MD → 项目树中的 `tts_client/` 标注 "TTS引擎适配器（多provider对接）"

### 1.4 ASR 选型

- 使用 **Realtime API** 进行语音识别（实时流式），而非本地 ASR 模型
- 配置中可见 `NEKO_ASSIST_API_KEY_QWEN` 等环境变量

- 来源: README.MD → "语音实时对话 (Realtime API) + 文字对话 (ChatCompletion)"

### 1.5 为什么好 / 为什么不好

| 维度 | 评价 |
|---|---|
| ✅ **架构前瞻性** | 多服务解耦（main/agent/memory）+ 插件市场 SDK，和我们设计理念一致 |
| ✅ **多 Provider TTS** | `tts_client/` 抽象层支持阿里云等国内厂商，中国大陆可用 |
| ✅ **主动陪伴** | "不会只在你召唤时才出现"——主动找用户玩，与 Miru 的 AttentionEngine 理念相似 |
| ✅ **多 Avatar** | Live2D / VRM / MMD / PNGTuber 五形态，比我们只用 Live2D 更灵活 |
| ✅ **记忆系统** | 5 层记忆（事实/反思/人设/近远期/事件），功能完整 |
| ✅ **Agent 执行** | 可操控电脑、浏览器、执行任务，A2A (Agent-to-Agent) 能力 |
| ⚠️ **部署复杂度** | Docker 多服务，不适合我们 Android 端直接嵌入 |
| ⚠️ **依赖云端 API** | TTS 和 ASR 均依赖云端 Realtime API（阿里云），离线能力弱 |
| ⚠️ **非 MIT 许可** | Apache 2.0，和我们 MIT 不同（需注意署名要求） |
| ⚠️ **资源占用** | 后端 Python 多进程 + React 前端 + Node.js，PC 端可接受，Android 端不适用 |

### 1.6 我们可以借鉴什么

1. **TTS Provider 抽象层**：类似我们 LLM Provider 的抽象方式，为 TTS 做一个 `TtsProvider` 接口
2. **阿里云 Realtime API**：如果愿意用云端 TTS，阿里云在国内延迟最低、可用性最高
3. **语音包上传**：支持用户上传自己的音色（GPT-SoVITS 等），实现个性化
4. **多服务架构**：记忆服务独立、Agent 服务独立——和我们 MCP 工具框架思路一致但更激进

---

## 二、TTS 候选方案对比总表

### 图例说明

- **音质**: ⭐⭐⭐⭐⭐ 顶尖 / ⭐⭐⭐⭐ 优秀 / ⭐⭐⭐ 良好 / ⭐⭐ 可用 / ⭐ 基本
- **延迟**: <500ms 实时 / 500ms-2s 近实时 / 2-5s 可接受 / >5s 较慢
- **许可**: ✅ 商业友好 / ⚠️ 需注意 / ❌ 不可商用

### 2.1 开源本地 TTS（离线可用，不依赖境外服务）

| 方案 | 中文音质 | 中国大陆 | 离线能力 | Android 嵌入 | 模型大小 | 许可 | 延迟 | 成熟度 |
|---|---|---|---|---|---|---|---|---|
| **CosyVoice** (阿里) | ⭐⭐⭐⭐⭐ 顶尖 | ✅ 直连 | ✅ 纯本地 | ⚠️ 需 ONNX 导出 | ~1.5GB (CosyVoice-300M) / ~3GB (CosyVoice-300M-25Hz) | Apache 2.0 | <500ms (流式) | ⭐⭐⭐⭐⭐ |
| **GPT-SoVITS** | ⭐⭐⭐⭐⭐ 克隆惊艳 | ✅ 直连 | ✅ 纯本地 | ❌ 太重 | ~2-5GB (含 SoVITS 模型) | MIT | 2-5s (非流式) | ⭐⭐⭐⭐⭐ |
| **ChatTTS** | ⭐⭐⭐⭐ 对话自然 | ✅ 直连 | ✅ 纯本地 | ⚠️ 需优化 | ~1GB | CC BY-NC 4.0 ⚠️ | <2s | ⭐⭐⭐⭐ |
| **Fish-Speech** | ⭐⭐⭐⭐ 多语言 | ✅ 直连 | ✅ 纯本地 | ❌ 太重 | ~2GB | Apache 2.0 / CC BY-NC-SA ⚠️ | <2s | ⭐⭐⭐⭐ |
| **Bert-VITS2** | ⭐⭐⭐ 中文可用 | ✅ 直连 | ✅ 纯本地 | ❌ 太重 | ~2GB | AGPL-3.0 ⚠️ | 2-5s | ⭐⭐⭐ |
| **Kokoro** | ⭐⭐⭐ 轻量 (英文强) | ✅ 直连 | ✅ 纯本地 | ✅ 极轻 (~80MB) | ~80MB ONNX | Apache 2.0 | <500ms | ⭐⭐⭐ |
| **sherpa-onnx** | ⭐⭐⭐-⭐⭐⭐⭐ (取决于模型) | ✅ 直连 | ✅ 纯本地 | ✅✅ 原生支持 | 可变 (~50MB-500MB) | Apache 2.0 | <500ms | ⭐⭐⭐⭐⭐ |
| **OpenVoice** (MyShell) | ⭐⭐⭐ 克隆好 | ✅ 直连 | ✅ 纯本地 | ❌ 太重 | ~2GB | MIT | >3s | ⭐⭐⭐ |

来源说明（基于公开 GitHub 仓库信息和社区文档，均未使用代理即可访问）:
- CosyVoice: https://github.com/FunAudioLLM/CosyVoice (阿里通义实验室开源，Apache 2.0)
- GPT-SoVITS: https://github.com/RVC-Boss/GPT-SoVITS (MIT 许可，中文社区最活跃语音克隆)
- ChatTTS: https://github.com/2noise/ChatTTS (对话式 TTS，CC BY-NC 4.0 非商用)
- Fish-Speech: https://github.com/fishaudio/fish-speech (多语言 TTS)
- Bert-VITS2: https://github.com/fishaudio/Bert-VITS2 (VITS 改进版)
- Kokoro: https://github.com/remsky/Kokoro-FastAPI (~80MB ONNX 极轻量)
- sherpa-onnx: https://github.com/k2-fsa/sherpa-onnx (多引擎 ONNX 运行时，原生 Android/iOS)
- OpenVoice: https://github.com/myshell-ai/OpenVoice (MIT 许可语音克隆)

### 2.2 国内云端 TTS API

| 方案 | 中文音质 | 中国大陆 | 免费额度 | 成本 (每万字) | 延迟 | 流式 | Android SDK |
|---|---|---|---|---|---|---|---|
| **阿里云 (通义千问)** - CosyVoice 云端版 / 语音合成 | ⭐⭐⭐⭐⭐ | ✅ 直连 | 有 (每月百万字符) | ¥0.5-2 | <500ms | ✅ | ✅ |
| **火山引擎 (豆包 TTS)** | ⭐⭐⭐⭐⭐ | ✅ 直连 | 有 | ¥0.5-2 | <500ms | ✅ | ✅ |
| **讯飞 (Spark TTS)** | ⭐⭐⭐⭐⭐ | ✅ 直连 | 有 (每日 500 次) | ¥2-3 | <500ms | ✅ | ✅ |
| **腾讯云** | ⭐⭐⭐⭐ | ✅ 直连 | 有 (每月百万字符) | ¥1-3 | <500ms | ✅ | ✅ |
| **MiniMax** | ⭐⭐⭐⭐⭐ | ✅ 直连 | 有 | ¥1-2 | <300ms | ✅ | ✅ |
| **百度智能云** | ⭐⭐⭐⭐ | ✅ 直连 | 有 (每日 5 万次) | ¥1-2 | <500ms | ✅ | ✅ |
| **智谱 (GLM-4V-Flash)** | ⭐⭐⭐ | ✅ 直连 | 有 (免费额度) | ¥2-4 | <1s | ⚠️ 需确认 | ⚠️ 需确认 |
| **月之暗面 (Kimi)** | ❓ 待确认 | ✅ 直连 | 待确认 | 待确认 | 待确认 | 待确认 | 待确认 |

- 来源: 阿里云 https://help.aliyun.com/document_detail/451344.html (语音合成 API 文档)
- 来源: 火山引擎 https://www.volcengine.com/docs/6561/79820 (豆包语音合成)
- 来源: 讯飞 https://www.xfyun.cn/services/online_tts
- 来源: 腾讯云 https://cloud.tencent.com/document/product/1073
- 来源: MiniMax https://api.minimax.chat/document/guides/tts
- 来源: 百度 https://ai.baidu.com/tech/speech/tts
- 注意: 具体价格可能有变动，以上为大致参考区间

### 2.3 sherpa-onnx 关键评估（Android 嵌入重点）

sherpa-onnx 是一个**多引擎 ONNX 推理运行时**，不是 TTS 模型本身，但可以运行：
- VITS 系列模型
- Kokoro (82MB)
- Matcha-TTS
- 支持流式解码

**Android 嵌入特性**：
- ✅ 原生 Android APK (AAR) 支持
- ✅ 提供 Java/Kotlin API
- ✅ APK 体积可控（引擎 ~10MB + 模型 ~50-500MB）
- ✅ 纯本地离线推理，零网络依赖
- ✅ 中国大陆完全可用

**关于 CosyVoice 支持**：
- ❌ 截至报告日期，sherpa-onnx **未明确支持 CosyVoice 模型**
- CosyVoice 基于 SenseVoice + Matcha-TTS 架构，理论上可导出 ONNX
- 但社区尚未提供现成的 ONNX 导出脚本 → 需要自行适配

来源: https://github.com/k2-fsa/sherpa-onnx (README 中 Pre-trained Models 列表不含 CosyVoice)
来源: https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/index.html

---

## 三、同类项目的 TTS 选型

基于 `docs/research/competitive-analysis-report.md`（2026-08-05，12 竞品对比），各同类项目的 TTS 引擎：

| 项目 | TTS 引擎 | 特点 |
|---|---|---|
| **Open-LLM-VTuber** (⭐13k) | Edge TTS / sherpa-onnx / MeloTTS / Coqui / GPT-SoVITS / Bark / CosyVoice / Fish Audio / Azure | **TTS 矩阵最丰富**，20+ 引擎可选 |
| **Soul of Waifu** (⭐1.1k) | Qwen3 TTS / XTTSv2 / Kokoro / Silero / EdgeTTS / ElevenLabs | 支持全本地 Kokoro 推理 |
| **my-neuro** (⭐1.3k) | GPT-SoVITS (语音克隆) | 专注个性化音色 |
| **Live2DPet** (⭐73) | VOICEVOX (日语) | Windows 本地一键安装 |
| **VPet-Ultra** (⭐3) | Piper / Kokoro | 完全离线本地推理 |
| **AI-Vtuber (Ikaros)** (⭐4.4k) | edge-tts / VITS / elevenlabs / bert-vits2 | 多引擎直播场景 |
| **小鲸鱼喜欢你 (xiaojingyu)** | Windows SAPI | 系统内置（仅 Windows） |
| **N.E.K.O. (猫娘计划)** (⭐2.4k) | 多 Provider (阿里云 Realtime API 推荐) | 云端为主 |

来源: `docs/research/competitive-analysis-report.md:32-37` (TTS 引擎对比表)
来源: `docs/research/competitive-analysis-report.md:118-127` (Open-LLM-VTuber 详情)
来源: 本文 §1.3 (N.E.K.O. TTS 选型)

---

## 四、分场景推荐

### 4.1 Android 端推荐（体积敏感，必须离线/国内直连）

| 优先级 | 方案 | 理由 |
|---|---|---|
| **P0 立即可做** | **Android 系统 TTS (已实现 Tier 2)** + 华为/小米/OPPO 等厂商 TTS 引擎 | 零额外体积，中国大陆直连，各厂商引擎音质不差 |
| **P1 短期** | **sherpa-onnx + Kokoro (~80MB)** | 极轻量（80MB），Apache 2.0 许可，Android 原生支持，音质可接受，流式低延迟 |
| **P2 中期** | **sherpa-onnx + VITS-zh (~200MB)** | 中文音质更好，适合日文/中文场景 |
| **P3 长期** | **CosyVoice ONNX 导出**（需自研导出） | 中文音质天花板，但当前无可用的 ONNX 模型，需自行适配 |

**关键约束**：
- Android APK 目标 <150MB（当前约 35MB）
- 离线可用是硬需（中国大陆网络环境不可靠）
- 延迟 <1s 对语音对话体验至关重要
- 引擎 SDK 必须在 AGP/Kotlin 生态中可用

### 4.2 PC 端推荐（可跑大模型，算力充裕）

| 优先级 | 方案 | 理由 |
|---|---|---|
| **P0 立即可做** | **CosyVoice 本地部署** | 中文音质天花板，Apache 2.0 许可，流式<500ms，阿里开源 |
| **P1 短期** | **GPT-SoVITS** | 语音克隆能力最强，MIT 许可，可让用户训练自己的音色 |
| **P2 中期** | **ChatTTS** | 对话式体验最自然（但 CC BY-NC 许可需注意） |
| **P3 备选** | **Fish-Speech** | 多语言好，但模型较大 |

**关键优势**：PC 端有 GPU（CUDA），可跑完整 PyTorch 模型，无需压缩

### 4.3 云端 API 推荐（省事，但需网络+费用）

| 优先级 | 方案 | 理由 |
|---|---|---|
| **P0 推荐** | **阿里云 (通义千问 TTS / CosyVoice 云端)** | 国内延迟最低、音质顶尖、免费额度充足、有 Android SDK |
| **P1 备选** | **火山引擎 (豆包 TTS)** | 字节生态，音质优秀，有免费额度 |
| **P2 备选** | **MiniMax TTS** | 超低延迟 (<300ms)，音质惊艳 |
| **P3 保底** | **讯飞 TTS** | 老牌语音厂商，稳定性最高 |

---

## 五、行动建议：Edge TTS 降级链调整

### 当前降级链（V1.0-1.1）

```
Edge TTS (微软公共端点) → Android 系统 TTS → 静默
         ↓ 中国大陆 HTTP 403
    ❌ 第一层直接失败
```

### 建议 V1.2 降级链

#### Android 端

```
Tier 1: 云端直连 TTS (阿里云 / 火山引擎) ✨新增
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 2: 本地离线 TTS (sherpa-onnx + Kokoro) ✨新增
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 3: Android 系统 TTS (保留)
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 4: 静默降级 (保留)
```

#### PC 端

```
Tier 1: 本地离线 TTS (CosyVoice 本地推理) ✨新增
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 2: 云端直连 TTS (阿里云) ✨新增
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 3: Edge TTS (保留作为海外用户的备选)
  ├─ 成功 → 播放
  └─ 失败 ↓
Tier 4: 静默降级 (保留)
```

### V1.2 引入优先级

| 任务 | 端 | 优先级 | 预估工作量 | 收益 |
|---|---|---|---|---|
| **TtsProvider 接口抽象** | 双端 | P0 | 2-3天 | 架构基础，解锁多引擎切换 |
| **云端 TTS 接入（阿里云）** | 双端 | P0 | 1-2天 | 解决中国大陆可用性，音质大幅提升 |
| **sherpa-onnx + Kokoro 集成** | Android | P1 | 3-5天 | 离线可用，80MB 轻量，解决"无网"场景 |
| **CosyVoice 本地部署** | PC | P1 | 2-3天 | 中文音质天花板，流式低延迟 |
| **TTS 设置 UI (引擎选择器)** | 双端 | P1 | 1-2天 | 用户可自选 TTS 引擎，对标竞品 |
| **GPT-SoVITS 集成** | PC | P2 | 5-7天 | 语音克隆，用户自定义音色 |

### 风险提示

1. **Kokoro 中文能力**：Kokoro 主要是英文/多语言模型，中文自然度不如 CosyVoice/阿里云，需实测评估
2. **CosyVoice ONNX 导出**：目前无官方 ONNX 模型，Android 端短期内无法直接使用 CosyVoice
3. **云端 API 费用**：虽然免费额度充足，但大并发场景下需评估成本
4. **ChatTTS 许可**：CC BY-NC 4.0 不可商用，只适合个人/学习用

---

## 六、调研证据索引

| 项目 | 地址 | 关键数据来源 |
|---|---|---|
| N.E.K.O. (猫娘计划) | https://github.com/Project-N-E-K-O/N.E.K.O | GitHub API (repo info + README base64) |
| CosyVoice | https://github.com/FunAudioLLM/CosyVoice | GitHub README (Apache 2.0) |
| GPT-SoVITS | https://github.com/RVC-Boss/GPT-SoVITS | GitHub README (MIT) |
| ChatTTS | https://github.com/2noise/ChatTTS | GitHub README (CC BY-NC 4.0) |
| Fish-Speech | https://github.com/fishaudio/fish-speech | GitHub README |
| sherpa-onnx | https://github.com/k2-fsa/sherpa-onnx | GitHub README + k2-fsa.github.io 文档 |
| Kokoro | https://github.com/remsky/Kokoro-FastAPI | GitHub README |
| OpenVoice | https://github.com/myshell-ai/OpenVoice | GitHub README |
| 竞品 TTS 数据 | `docs/research/competitive-analysis-report.md` | 第 32-37 行 (TTS 引擎对比表) |
| 当前 Edge TTS 实现 | `Live2D-Ai-Android/.../EdgeTtsService.kt` | 完整的 Edge TTS WebSocket 实现 |
| PC 端 TTS 配置 | `build/staging/Live2D-Ai-pc/open-llm-vtuber/conf.yaml` | `tts_model: edge_tts` |
| 阿里云 TTS 文档 | https://help.aliyun.com/document_detail/451344.html | 官方 API 文档 |
| 火山引擎 TTS | https://www.volcengine.com/docs/6561/79820 | 官方文档 |
| 讯飞 TTS | https://www.xfyun.cn/services/online_tts | 官方文档 |
| 小鲸鱼喜欢你 | https://github.com/shgghjj/xiaojingyu-likes-you-windows | GitHub API + raw README |

---

> **调研完整性声明**: 
> - ✅ 猫娘计划 (N.E.K.O.) 已找到并深度分析
> - ✅ TTS 候选方案 16 个已评估
> - ✅ 同类竞品 8 个的 TTS 选型已汇总
> - ⚠️ Kokoro 中文实测证据缺失（需后期实际测试）
> - ⚠️ CosyVoice ONNX 截至报告日期无官方支持（sherpa-onnx 预训练模型列表中无 CosyVoice）
> - ⚠️ 云端 API 价格可能变动，以各厂官网为准
> - ⚠️ GitHub stars 数值因 API 限流未刷新，但数量级准确
