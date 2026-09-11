# Neuro-sama 技术架构调研报告

> 调研对象：Vedal 开发的 AI VTuber **Neuro-sama**（及双胞胎 Evil Neuro）
> 目标：理解其如何组合 **LLM + TTS + Live2D**，提炼可复用的架构思路与组合模式。
> 说明：Vedal/Neuro 的核心代码**闭源**，本报告基于公开访谈、开发日志、维基、社区复刻项目（kimjammer/Neuro、Open-LLM-VTuber、my-neuro、Airi 等）与第三方技术分析交叉印证。标注`[推测]`的为推断项。

---

## 0. 一句话总结

Neuro-sama 是**「复合系统」而非单一模型**：它把多个"专家"AI（LLM 对话、专用游戏 AI/视觉、ASR、TTS、图像视觉 VLM、Filter AI）用 Python 脚本编排成一个实时流水线，人格**通过微调写入模型权重**（而非只靠 system prompt），Live2D 表情由**结构化情感标签驱动**，口型同步交给 **VTube Studio 基于音频波形**完成。

---

## 1. 技术栈总览

### 1.1 整体分层（社区反向工程结论 [kimjammer/Neuro, NeuroWiki, Qiita 分析]）

```
┌─────────────────────────────────────────────────────────────┐
│  直播平台层：Twitch / Bilibili / YouTube（独立客户端进程）      │
└──────────────────────────┬──────────────────────────────────┘
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  协调/编排层：Python 后端（signals 总线，模块化）              │
│  · 多输入源（chat/voice/game-screen）合并                     │
│  · trigger → 调度 LLM/TTS/Vision/Filter                      │
└───────────┬────────────────────┬────────────────┬───────────┘
            ▼                    ▼                ▼
┌────────────────┐   ┌──────────────────┐  ┌──────────────────┐
│ 大脑 Brain (LLM) │   │ 视觉 VLM / GameAI │  │ Filter AI(过滤)   │
│ 人格微调权重      │   │ (识别画面/玩游戏)  │  │ (内容审查)         │
└───────┬────────┘   └──────────────────┘  └──────────────────┘
        ▼
┌──────────────────────────────┐
│  TTS (语音合成) → 音频流       │
│  + 字幕文本（供前端显示）        │
└───────┬──────────────────────┘
        ▼
┌──────────────────────────────┐
│  Live2D 渲染（VTube Studio）   │
│  情感标签 → 表情；音频波形 → 口型 │
└──────────────────────────────┘
```

### 1.2 开发语言与运行
- **语言**：C#、Python、JavaScript（跨多组件）[Virtual YouTuber Wiki]
- **GPU**：Twitch 主页标注 RTX 4090 [Teck's Treehouse — 推测本地 GPU 跑小模型/低延迟任务]
- **编排**：后端为一系列 Python 脚本，对外部服务发 API 调用 [Teck's Treehouse]
- **开源 SDK**：`VedalAI/neuro-sdk`（含 Unity、Godot 官方 SDK + WebSocket 协议），用于把 Neuro 接入游戏——证明核心通过 **WebSocket 双向通信**与外部工具交互 [VedalAI/neuro-sdk]

### 1.3 LLM（大脑）
- 明确由 **LLM 驱动对话**，早期推举 OpenAI/ChatGPT 类商用模型（成本高、Vedal 明示"运行很贵"）[Teck's Treehouse, vedal.ai/advice]
- 关键点：**人格通过微调写入权重**，而非仅靠 system prompt [LLM Agent Research 专文]
  - `[推测]` 早期可能用本地/开源基座做了自定义微调或 LoRA（开发日志里提到"避免训一个完整 LoRA 来获得风格，prompt 也能做到" 反证其考虑过两种路线 [kimjammer Neuro Dev Log 4]）
  - `[推测]` 后段转向**自研/微调模型**（Vedal 曾直播从零按 Karpathy 教程训练语言模型 [SozAI/Youtube]）
- 社区复刻用的参照物：**LLaMA 3 8B Instruct / Llama3.2-3B**（说明生态里这类大小足够）[kimjammer/Neuro, HuggingFace]
- 多模型并存的判断：人格/对话一个模型，游戏决策/视觉另外的专用模型（见 §5 组合架构）[HuggingFace 论坛讨论, NeuroWiki]

### 1.4 TTS（语音）
- **商用 TTS**生成高音女性音色，低延迟支撑快节奏对话 [Wikipedia]
- 社区复刻参照：**CoquiTTS/XTTSv2**（本地、可微调克隆音色）+ RealtimeTTS 流式引擎（音频**边生成边播**）[kimjammer/Neuro]
- 生态内其他选项：**ElevenLabs**（流式、多语）、**Fish Audio / 微软**等 [Kimjammer, Questie]
- Vedal 未公布确切产品，但"低延迟 + 人声克隆"是核心诉求 [Wikipedia, kimjammer]

### 1.5 Live2D 渲染
- 初始用 **Live2D 免费模型"日代ほのか"/Hiyori Momose**；2023/5/27 换用**定制模型**（Otozuki Teru 建模，Anny 设计）[Wikipedia]
- 渲染托管在 **VTube Studio**（社区、DenchiSoft 出品），Neuro 通过 VTube Studio 插件/WebSocket 控制 [Veal, kimjammer, NeuroWiki]

---

## 2. AI 人格 / 人设管理

### 2.1 三种人格方案（Vedal 的关注点）
社区分析归纳了三路正解，Vedal 取舍如下 [Qiita AI_shigeo 分析]：
1. **自建自定义语言模型** —— `[推测]` 后期方向
2. **既有模型微调**（权重写入人设）—— **关键人格载体**
3. 既有模型 **prompt 调优 / RAG** —— 仅用于"情境上下文"，非人格本体

> 核心洞察：Neuro 是为数不多的、**把人设刻意固化进权重**的生产系统，system prompt 只承载"当下情境"这类易变信息 [LLM Agent Research]。

### 2.2 System Prompt 用法
- **System prompt 承担"情境性"人设**（当前场景、规则、可变的指示）
- **不可变的人格内核**交给权重微调
- 双胞胎：**Evil Neuro** 与 Neuro 使用同一套系统但人格相反——Vedal 对其 prompt 的调侃 "You are evil. You suck" [Fandom]——说明**人格差异可主要通过 prompt 注入实现**（动态换人设很快）
- 前端可**运行时注入 Custom Prompts**（不改核心配置即可临时改变行为）[DeepWiki/kimjammer]

### 2.3 记忆系统（长期一致性）
Neuro 经历了：**无记忆 → 对话内记忆**，后逐步获得**长期记忆（明确为 2024 某次升级）**、情绪模拟、逻辑组织 [NeuroWiki]
社区复刻的"长期记忆"标准做法（Neuro 大概率类似）：[DeepWiki kimjammer, Stackoverflow]
- **向量数据库**（ChromaDB / Qdrant / pgvector）存"fact/长期记忆"
- 会话中**检索相关记忆注入 prompt**（RAG）
- **分层记忆**：短期（对话内）、中期（摘要）、长期（向量库持久化）
  - 例：`neuromem` 用 **Fact / Episode / Graph / Trait** 四层，`digest()` 反思引擎归纳用户特质 [MatrixDriver]
  - 例：**每 N 条消息做一次 session summary** 进中期记忆 [Stackoverflow]
- Neuro 仍会"编造事件"——即长期记忆仍不完美 [NeuroWiki]

### 2.4 一致性保持的补充手段
- **Filter AI（过滤 AI）**：内容审查；违规输出**不是静默屏蔽**，而是替换为单词 **"filtered"**——保留可读的"被过滤感"，成为人设特色 [Wikipedia, Medium Rise of vTubers, kotaku]
- **人工团队**实时监控与审核 chat [Vice/Motherboard]
- **情感/情绪模拟**作为人格的一环，被 NeuroWiki 列为后续获得的能力

---

## 3. 情绪检测与表情映射

社区对 AI VTuber 情感管线的标准实现（Neuro 思路高度吻合，且有公开复刻佐证）：

### 3.1 LLM 输出结构化情感标签
让 LLM 在生成回复文本的同时，输出**情感标签**（如 `set emotion: surprised` / `happy` / `angry`），由 LLM 自主判断每句情绪。

- **结构化输出**：LLM 返回 JSON，`{ reply, expressionMix, parameterOverrides }` [entropy622/LLM_Live2D]
- **注意力/关注建议**：DeepWiki 的 Open-LLM-VTuber 明确 LLM 需"**为了情感控制，输出结构化数据而非纯文本**"——这是当代实现的关键演进

### 3.2 情感 → Live2D 表情索引映射
- 把**"自然语言空间情绪"**解析为**"代码实体空间动画索引"** [DeepWiki Open-LLM-VTuber 10.2]
- 引擎（`Live2dModel.extract_emotion`）：基于**关键词匹配**（model_dict.json 里 emotion→expression 映射），从文本中识别情绪词匹到表情 [DeepWiki]
- LLM 直接给标签时更精准：LLM 决定情感 → 标签 → 查表 → Cubism expression [AITuberFlow emotion-analyzer, my-neuro]
- **表情混合**：不只切表情，还能做 `expressionMix`（多表情权重混合）+ `parameterOverrides`（直接控制 Live2D 参数如眼睛/眉毛）[entropy622]
- 复刻实战要求：Live2D rig **至少 5 个表情开关**，才能支撑 happy/sad/surprised/angry 等 [AnimArts]

### 3.3 表情与音频/文本同步
- 表情动画要和**音频播放**同步触发 [DeepWiki]
- 情绪标签与文本分句对齐：分句送到 TTS，同时该句对应表情下发前端播放（详见 §4、§6）

---

## 4. TTS 与口型同步方案

### 4.1 口型同步（Lip Sync）：依赖音频波形，"开箱即用"
Neuro 的口型**不是**从文本/音素做精确 viseme，而是依赖 **VTube Studio 的音频驱动口型**：
- 机制：Neuro 的 TTS 音频输出经由 **虚拟音频线缆**（VB-Audio Cable 类）送入 VTube Studio [kimjammer]
- VTube Studio **Advanced Lipsync**：分析**麦克风/音频音量 + 频率**，自动驱动 Live2D 的 `MouthOpen` 等口型参数 [VTubeStudio Wiki]
  - 根据音量开合口腔、按检测到的语音频率改变嘴型
- **精度取舍**：基于波形（volume/frequency）而非音素级 viseme——延迟低、够用；真正的音素口型非常复杂 [VTubeStudio Wiki, Blerp]

> 结论：Neuro 的"TTS→口型"是**粗粒度波形驱动**，把精确口型外包给 VTube Studio，自己专注情感表情。这是省事且稳健的组合。

### 4.2 字幕
- 回复通常**配字幕**，可独立于音轨对齐显示（文本即时，音频流式）[Wikipedia]

### 4.3 TTS 流式
- 社区复刻用 RealtimeTTS `TextToAudioStream`，**边生成边播放**（不等整个句子/回复合成完）[kimjammer/Neuro tts.py]
- 句子级切分 → 有序排队 → 并行 TTS 生成、串行播放（见 §6）

---

## 5. 管道架构（Pipeline）：文本 → LLM → emotion → TTS → Live2D

### 5.1 主流水线（单次回复回合）
```
[触发] chat/voice/比赛事件
   │
   ▼
[大脑 LLM] 基于 {system prompt + 语境 + 记忆检索 + 历史} 生成
   │  输出两部分：
   │   ① 口语文本（可含情感标签）
   │   ② 情感标签/结构化信号（emotion / expressionMix）
   ▼
[Filter AI] 内容过滤（违规→"filtered"）
   │
   ├──────────────► [前端字幕] 即时显示文本
   ▼
[TTS引擎] 分句 → 流式合成（边生成边播）→ 虚拟音频线 → VTube Studio
   │                               │
   ▼                               ▼
[Live2D前端]                  [口型：波形驱动 MouthOpen]
   表情标签→Cubism expression触发      + 语音播放

（每句循环：分句 → 该句TTS + 该句表情同步）
```

### 5.2 多"专家模型"协同（Neuro 的复合架构本质）
- **对话 LLM**：人格 + 回复
- **专用游戏 AI / 视觉模型**：2018 年起为 osu! 训练的神经网络（80×60 灰度屏幕输入），后扩展到 Minecraft/其他游戏 [Wikipedia, Fandom]
- **视觉 VLM**：读取屏幕/图像生成回复（reaction 内容）[Kotaku, NeuroWiki]
- **ASR/STT**：用于与人类 VTuber 合作语音交流（Bilibili/YouTube 双字幕后演进）[Vice, NeuroWiki]
- **Filter AI**：内容把关 [Medium]

> Vedal 本人在 Kotaku 采访明示：**"她使用先进 AI 模型与算法的组合，聊天 AI 由 LLM 驱动"**——坐实"复合系统" [Kotaku]。

### 5.3 与外部工具的通信（扩展能力）
- Via **Neuro API / WebSocket**（`neuro-sdk`），让 Neuro 能"看"游戏画面并**用工具操作游戏** [VedalAI/neuro-sdk, "How Neuro plays games" 视频]
- 游戏集成采用 SDK（Unity/Godot），说明核心与工具进程是**分离的、松耦合的 WebSocket 架构** [VedalAI]

---

## 6. 流式处理、打断机制、延迟优化

### 6.1 流式 + 并行分句（社区标准做法，Neuro 思路一致）
- **核心矛盾**：LLM 是"流式 token"，TTS 是"原子整句合成"，Live2D 是"音画同步" [DeepWiki 5.2]
- 解法（TTS Task Manager / Sentence Divider）[DeepWiki Open-LLM-VTuber]：
  - **句子切分器**：把 LLM 流式文本按句切开
  - **并行 TTS 生成**：多个句子并行合成音频
  - **有序投递**：`TTSTaskManager` 用队列保证**并行生成、按序播放**（防止乱序）
  - **双队列架构**：一边转音频、一边播上一段（pipelining）[my-neuro DeepWiki]
- 效果：实现"边想边说"，第一句话的音频在 LLM 还在生成后续时就已开始播放
- 目标延迟：**voice-to-voice ~500-700ms**（streaming pipeline 最佳实践水平）[pipecat 架构文档]

### 6.2 打断 / 抢话（Barge-in）
- 早期 Neuro 的 voice collab（2023/2 与 Miyune）**处理打断很挣扎**，后经修复 [Fandom]
- 现代标准实现：**用户开口（VAD 检测）→ 立即停止当前 TTS 与 LLM 生成 → 清空生成队列 → 处理新输入** [EdgeVox, PromptKit PR, voice-agent-starter]
  - `InterruptController`：VAD→触发 <20ms；TTS flush <100ms；LLM 中断同步 [EdgeVox]
- `[推测]` Neuro 同样依赖 **VAD + 输出队列取消/刷新**实现打断；其"低延迟"使其能快速切换话题

### 6.3 延迟优化措施
- **流式 TTS**（不等全句）[kimjammer, ElevenLabs 概念]
- **分句流水线**并行（LLM/TTS 重叠）[DeepWiki]
- **模型选速度优先**（Flash/实时模型、减少缓冲）[ElevenLabs latency 指南]
- **TTFB（首字延迟）优化**：会话复用、就近路由、流式接口 [ElevenLabs, livekit issue #1402]
- **延迟实测（Neuro 官方 2024-07 开发流）** [Fandom Dev Stream]:
  - 平均延迟从 **900ms–1s** 降到 **≤700ms**（主要场景）；大段文本/长响应可达 5s 级别
- **"VAD 跳过式发言"机制**（日本 Qiita 分析发现的系统级行为）[Qiita 分析その2]:
  - Neuro 会**故意不在第一段无音区间应答**，而在**第二次无音区间**才作答（有时一次覆盖两轮问题）
  - 推断这是**系统主动的说话轮次（turn-taking）管理**，用于避免被误判为"抢话"或等待更完整上下文——不是 bug，是设计
- **低延迟的整体观**：Neuro 被评价"低延迟支撑快节奏对话"是其体验关键 [Wikipedia]

---

## 7. 关键架构洞察 / 可复用模式总结

1. **复合系统 > 单模型**：Brain(LLM) + 专用游戏/视觉模型 + ASR + TTS + Filter，各自擅长的事交给专职小模型，用 Python 编排。
2. **人格 = 权重微调为主 + system prompt 为辅**：不可变内核进权重，可变情境进 prompt；用 prompt 即可快速切换人格（Neuro/Evil）。
3. **结构化情感输出**：让 LLM 输出 文本 + 情感标签（结构JSON），把"即时反应"走确定性查表，快且可控。
4. **情感标签驱动 Live2D 表情**：emotion → expression 映射（关键词或 LLM 标签），支持表情混合 + 参数覆盖。
5. **口型同步外包给波形**：TTS 音频→虚拟声卡→VTube Studio 音频驱动口型，省去音素级 viseme 复杂度。
6. **流式+分句+并行TTS+有序播放**：LLM 流式、句子切分、TTS 并行、排队按序播放，实现"边说边想"、低延迟。
7. **VAD + 打断 + 说话轮次管理**：主动暂停应答窗口、支持 barge-in，是"像人"体验的关键。
8. **内容安全 = Filter AI + 人工**，且"filtered"占位保持人设可读性。
9. **松耦合 WebSocket 架构**：核心与工具/游戏/前端分离，开放 SDK 让社区扩展（体现架构的模块化）。

---

## 8. 参考来源

- Wikipedia《Neuro-sama》：栈综述、封禁史、TTS/Live2D 模型
- NeuroWiki（en.neurosama.info）：能力演进、长期记忆、情绪模拟
- LLM Agent Research（lin-guanguo.github.io 专文）：微调人格、流水线图
- kimjammer/Neuro（GitHub + DeepWiki + Dev Logs）：社区复刻架构、TTS、记忆、Custom Prompts
- **Qiita《AI-Vtuber ネウロ様の分析》**系列：人格三方案、VAD 跳过式发言/turn-taking、延迟
- Open-LLM-VTuber（DeepWiki）：情感映射、TTS Task Manager、打断、流水线（当代参照实现）
- VTube Studio Wiki：Advanced Lipsync（音频驱动口型）
- Vedal 采访（Kotaku / Vice / Motherboard / vedal.ai / Wikipedia）："复合 AI 模型组合"、LLM 为主、团队审核
- Fandom Dev Stream：延迟实测（900ms→≤700ms）、记忆升级
- ElevenLabs / Pipecat / EdgeVox docs：低延迟、流式、barge-in 最佳实践

---

## 附：与本项目的关系（Phase 1 待定）

本调研为后续规划提供背景。若要做类似 AI VTuber（Windows+安卓双端），可参考的落地组合思路：
- **LLM**：OpenAI 兼容 / 本地（Llama/Qwen）微调人设
- **TTS**：流式（XTTSv2 / ElevenLabs / Fish）边生成边播
- **Live2D**：VTube Studio 或自绘 Cubism 渲染 + 音频波形口型
- **情感**：LLM 结构化标签 → 表情映射 → 表情混合/参数覆盖
- **管线**：句子切分 → 并行 TTS → 有序播放 → VAD 打断 → 轮次管理
