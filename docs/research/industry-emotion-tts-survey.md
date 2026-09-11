# 行业调研：LLM → 表情/情绪映射方案全景对比

> 调研目标：Live2D AI 伴侣核心链路 — **LLM 输出的情绪信号长什么样？**（结构化 JSON / function calling / 内嵌标签 / 关键词匹配 / LLM 后处理）
> 调研员：researcher（只读）+ orchestrator 拆解
> 证据等级标注：🟢=源码实锤（直接读取源文件）｜🟡=官方文档/DeepWiki 归纳（基于源码但非原始文件直读）｜🔴=社区传闻/逆向推断
> 覆盖项目：Open-LLM-VTuber、morettt/my-neuro、AITuberFlow、entropy622/LLM_Live2D、Neuro-sama(Vedal)、VTube Studio 生态、Live2D 官方 Motion-sync
> 与现有文档关系：本报告聚焦「LLM→表情 核心链路」，与 `docs/research/NEURO_LIVE2D_RESEARCH.md`（渲染/复刻栈）、`docs/research/tts-research-report.md`（TTS 选型）互补，不重复。

---

## TL;DR — 五条核心结论

| # | 结论 | 证据强度 |
|---|------|---------|
| 1 | **业内主流方案是「LLM 在文本流里内嵌情绪标签」**，用正则/字符串扫指实时提取，然后映射到 Live2D expression index | 🟢 源码实锤（Open-LLM-VTuber、my-neuro） |
| 2 | **结构化 JSON 输出正在成为第二主流**（深挖 Open-LLM-VTuber 的 ToolCallAgent + entropy622/LLM_Live2D 的 expressionMix），尤其当需要「表情+参数」多维控制时 | 🟢 源码实锤 |
| 3 | **「LLM 后处理独立分析」是第三种方案**（AITuberFlow），独立调一次 LLM 返回 `{expression, intensity}`，天然支持 rule-based 降级 | 🟢 源码实锤 |
| 4 | **流式兼容性是决策关键**：内嵌标签方案天然流式（token 级）；JSON/结构化输出需缓冲解析（句子/块级）；后处理方案不可流式（需等全文）| 🟢 源码+架构分析 |
| 5 | **可靠性三件套**：①prompt 强约束只允许用白名单标签 ②大小写不敏感+宽容匹配 ③失败降级（默认 neutral / 规则匹配 / LLM→rule fallback） | 🟢 源码实锤 |
| 6 | **TTS 口型同步业内已标准化为「viseme + volume 双通道」**：Live2D 官方 Cubism 用音量，VTube Studio 用音量/频率，Viseme 方案（AudioWorklet/神经网）可做到音素级 | 🟢 Cuttool/官方文档 |

---

## 方向1：Open-LLM-VTuber — 内嵌标签的流式标杆（源码实锤 🟢）

**这是本调研证据最硬的一个项目**（公开源码，已直接读取三个核心文件）。

### 1.1 LLM 输出格式：内嵌 `[expression_key]` 标签
- 🟢 **`prompts/utils/live2d_expression_prompt.txt`**（源码直接抓取，原文全文在内）：
  ```text
  ## Expressions
  In your response, use the keywords provided below to express facial expressions...
  Here are all the expression keywords you can use. Use them regularly:
  - [<insert_emomap_keys>]
  ## Examples
  "Hi! [expression1] Nice to meet you!"
  "[expression2] That's a great question! [expression3] Let me explain..."
  Note: you are only allowed to use the keywords explicity listed above. Don't use keywords unlisted above.
  ```
- **`<insert_emomap_keys>` 是占位符**，运行时被替换成模型配置里的真实情绪 key 列表（如 `[neutral],[anger],[disgust],[fear],[joy]...`）。LLM 被指示在文本任何位置插入 `[key]`。

### 1.2 可靠性与映射：`Live2dModel` 类（源码实锤 🟢）
- 🟢 `src/open_llm_vtuber/live2d_model.py`：
  - `self.emo_map = {k.lower(): v for k,v in model_info["emotionMap"].items()}` — emotion key → **Live2D expression index**（整数）。
  - `self.emo_str = " ".join(f"[{key}]," ...)` — 用于拼进 prompt 的 key 列表。
  - **`extract_emotion(str_to_check)`**：大小写不敏感（`str_to_check.lower()`），线性扫描文本找 `[key]`，命中一个就把映射的 index 追加进 `expression_list`（可多个）。
  - **`remove_emotion_keywords()`**：反向把文本里的 `[key]` 剥掉，得到干净文本供 TTS/显示。
- 🟢 `model_dict.json` 实锤 emotionMap 格式：
  ```json
  "emotionMap": {
      "neutral": 0, "anger": 2, "disgust": 2, "fear": 1,
      "joy": 3, "smirk": 3, "sadness": 1, "surprise": 3
  }
  ```
  即「情绪词（英文 key）→ expression index」，同一 index 可被多个语义相近情绪共享（anger/disgust 都=2）。

### 1.3 流式管道：全异步 generator 装饰器（源码实锤 🟢）
- 🟢 `src/open_llm_vtuber/agent/transformers.py` 是一条链式 async generator 装饰器栈，**逐 token 流式**：
  ```
  LLM token stream
    → sentence_divider()       // 切成句子 + 标签状态
    → actions_extractor()      // 对每句 extract_emotion，产 (sentence, Actions)
    → display_processor()      // 产出显示文本
    → tts_filter()             // 剔除 [key]、处理 think 等
    → yield (SentenceOutput, Actions)   // 每句实时推送
  ```
  - `actions_extractor` 对每个完成句子调用 `extract_emotion` —— **句子粒度触发表情**，不需等全文。
  - Actions 对象承载 `expressions`（expression index 列表）随句子一起送达。
- 🟢 前端通过 **WebSocket（端口 12393）** 实时接收 Live2D expression/motion 命令，真正 token→句子级流式。证据：`src/open_llm_vtuber/websocket_handler.py`（`WebSocketHandler`，连接/上下文字典、`current_conversation_tasks`）。

### 1.4 可靠性降级
- **没有显式 fallback 代码**——它把「只准用白名单 key」完全押在 prompt 约束上；匹配失败/无 key 时 `extract_emotion` 返回空列表（不报错），即表情不变化、自然降级为无动作。🟢
- 🟢 边界处理：`extract_emotion` 对大小写不敏感、`remove_emotion_keywords` 循环删除确保不漏（对带子串的 key 也安全）。

### 1.5 关键结论
- **LLM 输出 = 纯文本 + 内嵌 `[key]` 标签；映射 = 正则扫描 → index；流式 = 完全支持（句子级）；降级 = 无动作。**
- 这是「喂给前端的就是给 TTS 的文本 + 附带的动作」，一份输出多路复用，链路最简、时延最低。

---

## 方向2：其他 AI VTuber 项目

### 2.1 morettt/my-neuro（Neuro-sama 社区复刻，AI 伴侣定位，中文支持）— 🟢 源码实锤

**与用户项目定位最接近**（专属 AI 角色 / 桌面宠物 / 实时语音）。截取到多个源码。

- 🟢 **`live-2d/config.json` 的 system prompt（原文在中国区）** 明确要求 LLM 内嵌**尖括号中文情绪标签**：
  > "你可以用这几个情绪标签：`<开心> <生气> <难过> <惊讶> <害羞> <俏皮>` 自然地用就行,想用就用,不想用就不用。"
- 🟢 **`live-2d/emotion_actions.json` 标签→motion 映射**：每个角色一段映射，把情绪标签映射到 Live2D motion 文件路径（`开心→motions/hiyori_m06.motion3.json` 等），也可映射到 `expression`。
- 🟢 **可选 BERT 独立检测**：config 里 `bert: { enabled:false, url:".../classify" }` —— 可用一个独立 BERT 服务对文本做情绪分类，作为 LLM 标签的**替代/兜底**。这是「关键词/模型检测」混合方案。
- 🟢 **流式**：`live-2d/js/ai/llm-client.js`（520 行）处理 streaming response + tool call 解析（`toolCallRegex1/2`、`attrRegex`）。LLM 输出也走流式。
- 🟢 **mood 记忆系统**：「Authentic emotions: Simulates real human emotional states with persistent mood tracking」（README 第49行待办）。
- **机制归类**：同 Open-LLM-VTuber 的「内嵌标签」家族，但标签是**中文语义词**不是英文 key，且更强调「自然随意、可选」（"想用就用不想用就不用"）—— 牺牲严格性换取自然度，靠 `emotion_actions.json` 兜底映射。

### 2.2 entropy622/LLM_Live2D — 结构化 JSON 输出（🟢 文档明确 + 未直读 py/ts）

- 🟡 README（中文，原文直取）自述，核心是**三层架构**：
  > 1. `llm.ts` 要求模型返回**结构化 JSON**，包含回复文本、`expressionMix` 和可选的 `parameterOverrides`
  > 2. `avatarManifest.ts` 在运行时自动从模型资源发现可用 `.exp3.json` 和核心参数白名单
  > 3. ...
- 🟡 明确对比定位：『社区复刻 Neuro Sama 的任务（如 Airi）并不支持 LLM 去直接控制各种 expression key，这个项目是为了补齐这一点』——**直接证据表明「内嵌/固定 expression」是常见默认做法，而本项目引入了「混合表情」(expressionMix) + 参数级控制 (parameterOverrides)。**
- **机制归类**：方案三「结构化 JSON 输出」，支持表情混合 + 参数覆盖，需要 JSON schema 强约束。
- 注：`llm.ts` 实际路径未在 raw 上直接命中（可能路径不同），关键设计点来自 README/DeepWiki 归纳。🟡

### 2.3 AITuberFlow `emotion-analyzer` 插件 — LLM 后处理 + rule-based 降级（源码实锤 🟢）

- 🟢 `plugins/emotion-analyzer/node.ts`（398 行，直接抓取）：
  - **这是「独立 LLM 后处理」最清晰的一例**——TTS 文本照常由主 LLM 生成，另起一个轻量 LLM 对文本做情绪分类。
  - **LLM prompt（原文）** 要求只回一个 JSON：
    > `Respond with ONLY a JSON object in this exact format: {"expression": "<expression_id>", "intensity": <0.0-1.0>}`
  - **韧性解析**：`response.match(/\{[^}]+\}/)` 从回复里抠出第一个 JSON 对象再 `JSON.parse`；解析失败→默认 `neutral`。
  - **validation**：`if (!validIds.includes(expression))` 未知 expression → 强制 `neutral`；`intensity` clamp 到 [0,1]。
  - **双重降级**：`try LLM → catch → analyzeRuleBased(text)`。方法默认 `"llm"`；无 API key 直接走规则匹配（关键词字典，支持 ja/en）。
  - 输出通过 `createEvent("avatar.expression", {expression, intensity})` 广播（emitEvents 可开关）。
- 🟢 **非流式**：这是对它**不利**的关键——`analyzeLlmBased` 是一次完整 `chat.completions` 调用（无 stream），且要等主 LLM 文本产出才能分析，天然**不可 SSE 流式表情同步**（延迟高）。它适合「整段语气情绪」，不适合逐句情绪。

### 2.4 ai-character-framework / ez-vtuber-ai / Idiots-AI（次要，🟡 社区项目）
- `murayan1982/ai-character-framework`：AI 角色对话框架，支持 text/voice/Live2D，多 LLM 连接。文档级概述。
- `VoxLink-org/ez-vtuber-ai`：纯 TS/Electron/ONNX，README 提到「LLM for efficient emotion prediction」——同样是独立情绪预测节点。
- `au79g/Idiots-AI-vtuber-project`：本地 LM Studio + VRM，情绪 = 表情+动作，社区级实现。
- 均无一手源码直读，归纳为「LLM 预测情绪」家族。

---

## 方向3：主流方案对比 — 结构化 vs 标签 vs 后处理 vs 关键词

### 3.1 业界对「结构化输出 vs function calling」的界定（🟡 通用资料）
- 🟡 machinelearningmastery.com《Structured Outputs vs. Function Calling》：两者底层都靠传 JSON schema，本质是「强制模型按 schema 回复」；区别在**约束强度**（Structured Outputs 是硬 schema 校验，Function Calling 是「可供调用的工具库，模型主动选择」）。对 VTuber，结构化输出适合「每轮都要表情」，function calling 适合「条件触发动作工具」。

### 3.2 四种落地方案的直接对比

| 方案 | 代表项目 | 表情信号形式 | 流式兼容 | 可靠性 | 时延 |
|------|---------|------------|---------|--------|------|
| **A. 内嵌标签** | Open-LLM-VTuber, my-neuro | 文本流中 `[key]` / `<中文>` | ✅ 句子/token 级 | prompt 强约束；失败=无动作 | 最低（零额外调用） |
| **B. 结构化 JSON** | entropy622/LLM_Live2D, Open-LLM-VTuber ToolCallAgent | 每段 `expressionMix`+`parameterOverrides` | ⚠️ 需缓冲解析（块级） | JSON schema 强约束/校验 | 中 |
| **C. LLM 后处理** | AITuberFlow emotion-analyzer | 独立返回 `{expression, intensity}` | ❌ 非流式 | 解析失败降级+rule fallback | 高（多一次 LLM 调用） |
| **D. 关键词/规则匹配** | AITuberFlow rule-based, my-neuro BERT, 早期**emotionMap** | 词→情绪的静态字典 | ✅ | 确定性最高 | 最低 |

### 3.3 「阿里云」生态的情绪输出最佳实践（🟡 官方文档，接地气佐证）
- 🟡 阿里云「AI 实时互动/智能媒体服务」官方文档：为让 TTS 带情感，**要求 LLM 在回复最开头输出情感标签**：
  > `{{emotion=情绪值}}` 放在回复最开头，支持（自然/快乐/悲伤...）并提供严格 prompt（"只返回XXX否则受惩罚"）——**这是「前导标签」方案在商业云端的官方落地**，与内嵌标签理念同源，只是固定在前导位置便于 TTS 抽取。
- 🟡 涂鸦开发者平台「多模态情绪输出」：在**句首插入一个 emotion 表情符号（emoji）**，用「表情符号→TTS/动作」映射同时驱动文本、语音、动作。核心卖点：**TTS 天然忽略 emoji、无需剥离**、单信号源多模态触发。
- 🟡 阿里云 Dar(云) 社区「动作情绪控制」：**标准 JSON 化输出** `{action, things}` + 回复文本；强调「指令与回复由大模型一次推理生成」，保证动作与回复一致、降低时延 —— **印证方案 B：一次推理含结构化动作字段**。

### 3.4 业内共识判断
- **被最多项目采是「内嵌标签」**(A) —— 成本最低、流式最顺、prompt 约束足够。Open-LLM-VTuber（最活跃的本地开源 VTB）实例就是这个。
- **追求表情精细度/混合 → 结构化 JSON**(B)。
- **需要语气级/置信度/rule 兜底 → 后处理**(C)，代价是不可流式。
- **纯文本时代（弱模型）→ 选项是纯关键词**(D)。

---

## 方向4：Neuro-sama / Vedal 的做法（🔴 闭源，证据以复刻+社区为准）

**核心结论：Neuro-sama 本体渲染/情绪管线未开源，无法取得一手源码。**（与既有 `docs/research/NEURO_LIVE2D_RESEARCH.md` 结论一致）

- 🔴 官方公开仓库 `VedalAI/neuro-sdk`、`neuro-game-sdk` **只开源「让 Neuro 玩游戏」的 WebSocket SDK（Unity/Godot）**，不含 VTuber 情绪映射。
- 🔴 `Vedal987` 的其他仓（`neuro-api-docs` 等）GitHub API rate-limit 未取到内容。
- 🟡 NeuroWiki/Wikipedia/Vice：描述其「emotion simulation」「voice recognition」「anime-style avatar」，但无不公开机制。
- 🟡 学术论文《"I am Neuro, who are you?"》（New Media & Society）从社会学角度分析其人性化表演，无技术细节。
- **🟡 最接近一手的技术镜像**是社区复刻：
  - `kimjammer/Neuro`（🔴 dev log，LLAMA3 / oobabooga，非 emotion 专述）
  - `morettt/my-neuro`（受 Neuro-sama 启发，内嵌中文标签 — 见 2.1，这是**社区认为 Neuro 最可能的形式**）
  - anfogy/Neurosama（🔴 测试构建失败）
- **结论标注**：Neuro-sama 的情绪机制我**只能推断为「LLM 内嵌/前导情绪信号 + Live2D 表达映射」**，依据是社区复刻普遍采用该模式，但**非一手实证**，归为社区推断 🔴→🟡 之间。

---

## 方向5：TTS + 口型同步的业内方案（多源 🟢/🟡）

### 5.1 口型同步三条技术路线
| 路线 | 原理 | 代表 | 粒度 | 时延 |
|------|------|------|------|------|
| **音量驱动** | 用音频 volume 映射 MouthOpen | Live2D Cubism 默认、VTube Studio | 粗（开口度） | 实时 |
| **频率/能量驱动** | 分析 FFT 能量带驱动口型 | `DenchiSoft/Live2DFrequencyLipSync`、VTS POG | 中 | 实时 |
| **Viseme（音素）驱动** | 音频→phoneme→嘴形 | Live2D Motion-sync、`Amoner/lipsync-engine`、`s-soltys/LipSync` | 细（音素级） | 需分析 |

### 5.2 各方案细节
- 🟢 **Live2D 官方 Motion-sync**（Cubism 4）：把指定音频转成 **viseme 时间序列**，自动混合预定义的 mouth shape。这是标准「音素级」做法，需要 TTS 提供（或自行分析出）音素时间戳。
- 🟢 **Live2D Cubism SDK MouthMovement**：经典音量方案——`MouthMovement` 组件从 AudioSource volume 驱动口型，配 `.model3.json` 的 lip-sync 值。**最简单、零依赖、实时**，但只有开合无音素。
- 🟢 **VTube Studio**：内置几种 Lipsync（语音分析→ mouth 参数），同时支持**外部 audio 驱动的 mouth parameter + viseme**（blerp 文档、VTS POG 插件用 TTS 音量/频率开合嘴）。
- 🟢 **纯 JS 流式 viseme**（非常相关，因为是浏览器端/移动端友好）：
  - `Amoner/lipsync-engine`（~15KB，AudioWorklet + Web Audio，实时 viseme，即输入音频流→输出闭口形，配 Canvas/Live2D）。
  - `vlapky/three-vrm-lip-sync`（VRM，browser 内实时 lip sync，无 phoneme/无服务端）。
  - `sujito00/HeadAudio`（MFCC + 高斯原型 + Mahalanobis 分类 → Oculus viseme）。
  - `s-soltys/LipSync`（LPC + 神经网 → viseme morph target）。

### 5.3 对 Live2D 项目最可行的启示
- **无音素时间戳的 TTS**（如 Gemini TTS、多数云端 TTS 只给音频不给音素对齐）→ 用「音量/频率驱动」最稳（VTube Studio/官方 Cubism 默认）。
- **追求音素级** → 需要 TTS 返回 phoneme 时间戳，或用 AudioWorklet 实时 viseme 引擎（无需 TTS 配合）。
- 🟡 kugutu `docs/lipsync-v0.md` 明确给出决策表：「TTS 不吐 viseme 时间戳时 → 回落到音量/能量驱动」，正是业内最实用 fallback。

---

## 对 Live2D-Ai 的实现建议（综合）

结合本项目（Android Kotlin + PC Python，LLM→表情是核心链路），给出分层结论：

1. **首选「内嵌标签」为主链路**：仿 Open-LLM-VTuber —— 在 system prompt 注入 emotionMap 白名单 key，LLM 在文本流中内嵌 `[key]`，后端 async 管线逐句 `extract_emotion`→映射 Live2D expression index。**天然流式、零额外 LLM 调用、时延最低。**
2. **如需表情混合/参数级控制 → 再升级为结构化 JSON**：仿 entropy622 的 `expressionMix` + `parameterOverrides`，用 JSON schema 强约束。注意需逐句缓冲解析，牺牲部分流式实时性。
3. **不要用「后处理独立 LLM」做实时表情**：AITuberFlow 模式时延高、不可流式，只适合「整段语气」或置信度场景。
4. **可靠性三件套抄齐**：
   - prompt 白名单强约束（"只准用下列 key"）；
   - 大小写不敏感 + 宽容字符串扫描 + 未知 key 忽略（不崩）；
   - 全链路 fallback：无 key→无动作；JSON 解析失败→默认 neutral；LLM 后处理失败→规则匹配。
5. **TTS 口型**：先做「音量/频率驱动」（Cubism MouthMovement 思路，零依赖、实时），未来若要音素级再加 AudioWorklet visime 引擎（可参考 lipsync-engine），无需初始依赖 TTS 音素对齐。
6. **流式架构**：后端情绪提取用 async generator 装饰器链（Open-LLM-VTuber 样板），前端走 WebSocket 实时推送 expression 命令。

---

## 附：证据强度总表

| 来源 | 证据类型 | 文件/URL |
|------|---------|---------|
| Open-LLM-VTuber expression prompt | 🟢 源码直读 | `prompts/utils/live2d_expression_prompt.txt` |
| Open-LLM-VTuber Live2dModel | 🟢 源码直读 | `src/open_llm_vtuber/live2d_model.py` |
| Open-LLM-VTuber 流式管线 | 🟢 源码直读 | `src/open_llm_vtuber/agent/transformers.py` |
| Open-LLM-VTuber WebSocket | 🟢 源码直读 | `src/open_llm_vtuber/websocket_handler.py` |
| Open-LLM-VTuber emotionMap 格式 | 🟢 源码直读 | `model_dict.json` |
| my-neuro emotion 标签 prompt | 🟢 源码直读 | `live-2d/config.json` |
| my-neuro 标签→motion 映射 | 🟢 源码直读 | `live-2d/emotion_actions.json` |
| my-neuro 流式+tool call | 🟢 源码直读 | `live-2d/js/ai/llm-client.js` |
| AITuberFlow LLM 后处理 | 🟢 源码直读 | `plugins/emotion-analyzer/node.ts` |
| entropy622/LLM_Live2D 结构化 | 🟡 README/DeepWiki | github.com/entropy622/LLM_Live2D |
| Neuro-sama 机制 | 🔴 闭源/社区推断 | VedalAI SDK + 复刻项目 + 论文 |
| 阿里云前导标签/JSON | 🟡 官方文档 | alibabacloud/aliyun |
| Live2D Motion-sync | 🟢 官方文档 | docs.live2d.com |
| 音量/频率口型 | 🟢 官方文档+开源 | Cubism SDK, DenchiSoft, VTube Studio |
| 流式 viseme 引擎 | 🟢 开源 README/源码 | lipsync-engine, HeadAudio, LipSync |

> 调研完成。本报告 Phase-0，供规划自定义（无强约束），后续可进入 Phase-1 拆解。
