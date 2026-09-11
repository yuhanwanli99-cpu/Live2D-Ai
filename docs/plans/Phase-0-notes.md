# Phase 0 — 技术调研报告：AI + TTS + Live2D 组合模式开源轮子

> 调研日期: 2026-07 | 调研代理: technical-research
> 关联项目: Live2D-Ai（Windows Desktop = Open-LLM-VTuber Electron；Android = ChatService 直连 DeepSeek）
> 当前链路：文本 → LLM → `[emotion]` 标签 → Live2D 表情切换。
> 本报告聚焦「语音 TTS + 口型同步 + 表情动作」组合的成熟开源轮子，供规划 TTS/lip-sync/多平台扩展参考。

---

## 1. 调研范围与方法

针对 6 个关键词方向做了网络检索，覆盖：Open-LLM-VTuber 深度解析、Live2DViewerEX、VTube Studio 开源替代、Inochi2D/Inochi Session、LLM emotion→Live2D 映射、AI VTuber 全景。重点深入了 Open-LLM-VTuber（已知在用）的架构细节。

---

## 2. 项目全景表（成熟度 / 许可证 / 复用性）

| 项目 | Lang | Stars | 许可证 | 架构 | 成熟度 | 是否适合复用 |
|---|---|---|---|---|---|---|
| **Open-LLM-VTuber** | Python | 13k+ | Apache 2.0（另有部分附加条款，见 LICENSE，前端 Web/Electron 单独协议） | 前后端分离：FastAPI/Uvicorn 后端 + React/Electron 前端，WebSocket | ★★★★★ 生产级，社区活跃 | ✅ 主后端轮子，本项目已在用 |
| **Open-LLM-VTuber-Web** | TypeScript | 121+ | Apache 2.0(+附加) | React + Electron 前端，pixi-live2d-display 渲染 | ★★★★ 稳定 | ✅ 前端 fork 基础 |
| **guansss/pixi-live2d-display** | TypeScript | 1.5k+ | MIT | PixiJS v6 插件，Cubism2/3/4 统一 API | ★★★★★ 事实标准 Web Live2D | ✅ 前端 Live2D 渲染核心 |
| **原神 Live2DWidget (live2d-widget)** | TypeScript | 8k+ | MIT | pixi-live2d-display 封装，看板娘 | ★★★★ | ⚠️ 仅单模型 UI，非 AI 管道 |
| **AI-Vtuber (Ikaros-521)** | Python | 4.4k+ | GPL-3.0 | 多直播平台对接（B站/抖音等） | ★★★ 活跃但偏直播 | ⚠️ GPL 传染；主要是直播场景 |
| **Inochi2D + Inochi Session** | D/OpenGL | 390+ | BSD-2 / 多许可 | 自研 puppet 渲染引擎，非 Cubism | ★★★ 活跃（v0.8）但"未 ready for production" | ❌ 模型格式不兼容 Cubism，偏离 Live2D |
| **handcrafted-persona-engine** | Py/TS | 新 | MIT | Live2D + LLM + ASR + TTS + RVC | ★★ 新项目 | ⚠️ 思路可借鉴，不成熟 |
| **Live2D-LLM-Chat (suzuran0y)** | Python | 43+ | Apache-2.0 | Live2D + ASR(SenseVoice) + LLM + TTS | ★★ 小项目 | ⚠️ 语音链路参考 |
| **funnycups/petto** | Python/Qt6 | 114+ | GPL-3.0 | Live2D 桌面助手，Whisper + TTS + 表情 | ★★ | ⚠️ GPL；单模型桌面宠物 |
| **Project N.E.K.O.** | — | — | — | Live2D + emotion→expression/motion 映射 | ★★ | ✅ emotion 映射配置范式可借鉴 |
| **VTube Studio** | C#/闭源 | — | 专有(API公开) | Live2D 流媒体面捕，**非 AI** | ★★★★★ 商用 | ❌ 闭源无 AI；仅参考面捕 |
| **Live2DViewerEX** | 闭源 | — | 专有 | Live2D 桌面壁纸/看板 | ★★★ 商用 | ❌ 闭源无 AI 管道 |

**核心结论**：要搭「LLM + TTS + Live2D」组合，**没有单一项目能同时端到端复用**。事实标准组合 = **后端抄 Open-LLM-VTuber（Python/FastAPI/WebSocket），前端抄 Open-LLM-VTuber-Web + pixi-live2d-display**。这正是本项目现状。差异化 / 升级点在于：**TTS 口型同步质量和 emotion 表情映射的调优**。

---

## 3. Open-LLM-VTuber 深度架构解析（本项目主轮子）

### 3.1 整体分层（Client–Server）
```
┌─ Client 层 ─────────────────────────────┐
│  Web 前端 (React)  /  Desktop (Electron)│
│  · pixi-live2d-display 渲染 Live2D      │
│  · 音频捕捉/播放 (Web Audio API)        │
└──────────────┬─────────────────────────┘
               │ WebSocket (双向二进制+JSON)
┌──────────────▼─────────────────────────┐
│  Backend (FastAPI/Uvicorn, Python)      │
│  Server 生命周期管理 WebSocket 会话      │
│  每会话 clone 一个 ServiceContext        │
│  流水线: ASR → Agent → LLM → TTS        │
└─────────────────────────────────────────┘
```
- **通信协议**：WebSocket 双向。客户端上行音频/文本，后端下行文本 + Base64 WAV 音频 + `volumes` 音量数组（用于前端口型同步）+ Live2D 表情/动作指令。
- **多客户端**：同一后端服务多端口/多服务上下文，支持 Web 与 Unity 前端并存（Open-LLM-VTuber-Unity 在研）。

### 3.2 LLM 集成方式
- **多 Provider 工厂**：`LLMFactory` + `StatelessLLMInterface`。
- **统一"OpenAI 兼容"范式**：除 llama.cpp 和 Claude 外，**几乎所有后端都是 `openai_compatible_llm` 的换皮**（改 base_url/key/model）。
- 支持 Provider：OpenAI、OpenAI 兼容、Ollama、Gemini、Claude、DeepSeek、Zhipu、Groq、Mistral、llama.cpp、LM Studio。
- 现代版用 **Letta 做长时记忆**，**MCP 支持工具调用**。
- 关键配置字段：`llm_provider`、`temperature`、`faster_first_response`、`segment_method`(regex/pysbd)。

### 3.3 TTS 方案 & 口型同步（Lip-Sync）— 本报告重点
**TTS 引擎工厂** `TTSFactory`，支持极多后端：
- Azure TTS、Edge TTS（免费/联网/API key）、pyttsx3、Bark、CosyVoice / CosyVoice2、Melo、Coqui、Piper、Fish API、X-TTS、GPT-SoVITS、Sherpa-ONNX、MiniMax、ElevenLabs、Cartesia。

**口型同步机制（关键！两套思路）：**
1. **音量驱动法（当前默认实现）**：
   - 后端 `prepare_audio_payload` 用 `pydub` 把 TTS 音频转 WAV → Base64 打包。
   - **同时计算出声级数组 `volumes`**（分帧 RMS 音量 0–1），随音频一起经 WebSocket 下发。
   - 前端用 Web Audio API 播放音频的同时，用 `volumes` 数组驱动 Live2D 的 `ParamMouthOpenY` 参数 → 模拟嘴部开合。
   - **这是 Open-LLM-VTuber 的 lip-sync 核心，纯音量联动，非真音素级对齐**。
2. **官方 MotionSync 插件**：Live2D 提供 Cubism SDK MotionSync Plugin（Web/Unity/Native），基于 CRI Lipsync 做音素级口型（license 免费但需按 Live2D 协议），需模型含 `.motionsync3.json`。**Open-LLM-VTuber 未用 MotionSync，走的是自研 volumes 方案。**

**口型同步已知坑（重要调优经验）**：
- GitHub Issue #412：`cartesia_tts` 输出 `pcm_f32le` 编码音频，**导致嘴唇不动（声道/编码不兼容 volumes 计算）**；切回 `edge_tts` 正常。**→ 选 TTS 引擎必须验证音频编码与 lip-sync 兼容。**

**流式 / 抢占机制**：
- `TTSTaskManager` 并行生成多句 TTS，用**序号（sequence counter）保证前端音频按序播放**。
- **Voice Interruption（语音打断）**：用户说话时立刻打断后端 TTS 播放，且**被打断后不再继续生成 token**（减少浪费），agent 用 `handle_interrupt(heard_response)` 只记忆实际听到的部分。

### 3.4 Live2D 表情 / 动作控制
- **模型元数据**存于 `model_dict.json`：`name` / `url`(model3.json路径) / `kScale` 为必填；`emotionMap` / `tapMotions` / `defaultEmotion` 等可选。
- **`emotionMap` 映射**：`{ "happy": {"expression": "f01", "motion": "idle_01"}, ... }` → 每模型独立定义 emotion label → Live2D 表情/动作。
- **LLM 情绪标签注入**：系统在 persona prompt 末尾注入 `live2d_expressio` tool prompt，把 `emotionMap` 的 keys 以 `[<insert_emomap_keys>]` 占位符写入 System Prompt。LLM 输出 `[happy]` 这类标签文本。
- **后端提取**：`Live2dModel.extract_emotion(str_to_check)` 用**关键词匹配从文本中找情绪标签**（非结构化概率模型），映射到 expression index，随 WebSocket 下发前端播放。
- 前端用 `pixi-live2d-display` 的 MotionManager / ExpressionManager 播放对应 motion / expression，与音频播放同步（配合 volumes 做嘴部）。

### 3.5 参数调优经验（直接可复用）
- **降低首句延迟**：`faster_first_response: True`（第一句遇到逗号即先生成音频）。
- **句切分**：`segment_method: "pysbd"`（比 regex 更准确的句子边界 → 更好的逐句 TTS 流式）。
- **温度**：控制在 1.0 附近（默认），情绪标签才稳定；过高会乱打标签。
- **emo_key 设计**：情绪标签要收敛且映射要真对应模型的表情文件；标签太多模型会乱。
- **口型兼容**：验证 TTS 引擎输出音频格式可被 volume 分析（见 Issue #412）。
- **量化选择**：当前 OpenAI 兼容范式使模型切换零成本。

---

## 4. 关键技术轮子单独分析

### 4.1 guansss/pixi-live2d-display（前端渲染基石）
- 对官方 CubismWebFramework 的**重写/统一封装**，号称"universal Live2D framework on the web"。
- 支持**所有版本** Live2D（Cubism2/3/4）。
- 抽象层：`MotionManager`（动作）、内置 `ExpressionManager`（表情）、`FocusController`（视线）、`HitArea`。PixiJS 风格 API。
- 支持 `PIXI.RenderTexture` / `PIXI.Filter`（可加滤镜特效）。
- `Live2DExpression.setFadeIn/setFadeOut/updateParam` → 表情淡入淡出控制。
- **MIT 许可，复用门槛低**。Open-LLM-VTuber-Web 直接用 `pixi-live2d-display-lipsyncpatch`（加了 lipsync 补丁，支持 Cubism3–5，不支持 Cubism2）。

### 4.2 Live2D 官方 Cubism SDK MotionSync（口型备选）
- 音素级真口型（CRI Lipsync），免费但需遵循 Live2D 协议，需模型 `motionsync3.json`。
- 与 Open-LLM-VTuber 的"音量联动法"是**两条路线**：MotionSync 更真实但绑定官方 SDK 与模型资产；volumes 法简单、跨引擎通用、适合现有 Web 架构。
- **对本项目建议**：沿用 volumes 法（已打通），只有需要专业口型时才引入 MotionSync。

### 4.3 Inochi2D / Inochi Session —— 为何剔除
- Inochi2D 是**自研 2D puppet 格式**（mesh + 分层艺术），非 Live2D/Cubism 模型。
- Inochi Session 声明 **"under heavy development and isn't ready for production"**（0.8.7, 2024-09）。
- 面向**真人面捕直播**（VTubeStudio/MeowFace/OpenSeeFace 输入），**不含 LLM/TTS/AI 管道**。
- 本项目已投入大量 Cubism 模型资产 → **不迁移**。

### 4.4 VTube Studio & Live2DViewerEX —— 为何不选
- 都是**专有闭源**，不提供 LLM/TTS 组合管道。
- VTube Studio 有公开插件 API（面捕/hotkey/事件），仅适合作**面捕输入源**（本项目若未来加真人面捕可参考），不符"AI 组合轮子"需求。

### 4.5 周边 AI 语音链路可选组件
- **ASR**：Open-LLM-VTuber 默认 Whisper / FunASR / SenseVoice（`live2d-llm-chat` 用 SenseVoice）。
- **Voice 唤醒 / VAD**：配合 hands-free、voice interruption。
- **变声 RVC**：`AI-Vtuber` 与 `persona-engine` 集成 so-vits-svc / RVC，个性化音色。

---

## 5. 与本项目（Live2D-Ai）的映射与建议

| Live2D-Ai 现状 | 成熟轮子对照 | 建议 |
|---|---|---|
| 桌面 = Open-LLM-VTuber | 直接采纳其 TTS 工厂、volumes lip-sync、emotion 标签管道 | ✅ 主链路沿用 |
| Android = ChatService(OkHttp SSE) 直连 DeepSeek | **Android 端无现成轮子**，需自研 | ⚠️ 双平台差异化；可借鉴后端 pipeline 思路到 Kotlin |
| 当前纯文本链路 | Open-LLM-VTuber 已内置 TTS + ASR | ✅ 桌面升级语音 = 开启 TTS 配置即可 |
| emotion→表情 已打通 | emotionMap 配置范式 + LLM 标签注入 | ✅ 已对齐官方做法 |

**面向"神经桌面/Neuro-sama 风格"双平台扩展的增量关注点**：
1. **桌面端**：启用语音（ASR+TTS），复用 Open-LLM-VTuber 现成的 Edge TTS + volumes lip-sync + voice interruption。
2. **Android 端**：TTS 用本地 Edge TTS / 或云端；lip-sync 无官方轮子，需自研"volume→ParamMouthOpenY"（与官方 volumes 思路一致），或接入 Live2D MotionSync（Android 支持）。
3. **emotion 覆盖度**：确认 emotionMap 能覆盖模型所有表情，并统一 `[emo]` 标签语法在双端一致。
4. **许可证合规**：确认 Open-LLM-VTuber 的 Apache 2.0 + 附加条款边界（尤其 fork 分发时需披露），以及 Live2D 模型资产的 Cube 许可分隔。

---

## 6. 调研局限
- DeepWiki（Open-LLM-VTuber 源码级 wiki）与 github.io 在本次网络环境被 SSRF 拦截，部分源码细节依赖搜索摘要 + 官方 docs 快照，**未逐行读源码**。
- Android 端 TTS/lip-sync 开源轮子稀缺，属已知空白，需在正式规划中立项自研或调研 Live2D MotionSync Android 集成。

---

## 7. Phase 0 结束条件
- ✅ 对目标领域理解充分（核心轮子已定位 + 主项目架构已解析）。
- ✅ 进入正式结构化规划前，建议向用户澄清：**「Android 端语音 + 口型是自研 volume 方案，还是接官方 MotionSync？」** 以及 **「是否需要 RVC 变声 / 真人面捕输入」**。
