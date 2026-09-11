# N.E.K.O.（猫娘计划） vs Neuro‑sama —— 产品级对标分析

> 调研日期: 2026-08-05
> 调研员: researcher agent（只读）
> 定位：**产品设计 / 功能 / 体验 / 架构理念 层面**的对标报告，**不深入源码**。代码级参考见 `docs/research/NEURO_LIVE2D_RESEARCH.md` 与 `docs/research/tts-research-report.md`。
> 三条纪律：每条结论附来源（URL / 本项目文件:行号）；抓取失败标注「未找到，待补充」；不编造数据、版本号、价格。

---

## 0. 一句话总结（TL;DR）

- **Neuro‑sama 不是「产品」，是一场成功的「直播演出 / IP」**：她的核心竞争力是**人设的社区共创**（粉丝喂料 + 记忆回捞形成"会成长的真人感"），而**不是技术有多先进**。技术栈只是普通的"LLM + TTS + Live2D + 游戏AI"拼装（与本项目同构），但**产品包装、直播场景、社区运营**把它推成了 Twitch 顶流。
- **N.E.K.O. 是最接近"平民版全能 AI 伴侣平台"的产品**：靠"多服务 + 多 Avatar + 多 Provider + 插件商城 + 5层记忆 + 主动陪伴"的**广度**取胜，是"什么都给你搭好"的一站式方案。
- **对 Live2D-Ai 的启示**：Neuro‑sama 证明了**"人设与陪伴感"是灵魂**（非技术），N.E.K.O. 证明了**"多形态 + 可扩展 + 低门槛上手"是底座**。Live2D-Ai 应优先补**"持续人设 + 记忆成长 + 主动陪伴"**（抄 Neuro 的理念），再补**"多 Avatar / 多 Provider / 一键换装"**（抄 N.E.K.O. 的广度）。

---

## 1. 两个项目的产品定位对比

| 维度 | **N.E.K.O.（猫娘计划）** | **Neuro‑sama（VedalAI）** | **Live2D-Ai（我们）** |
|---|---|---|---|
| 本质 | 开源 **AI 伴侣平台 / 桌面伴侣产品** | 顶流 **AI VTuber 直播 IP / 演出**（+ 外围开源 SDK） | 开源 **AI 桌宠 / 语音伴侣** |
| 目标用户 | 想自建/定制个性化 AI 猫娘的**开发者 + 二次元用户** | **Twitch/观众/粉丝**（围观"AI 能否像真人主播"） | 想要**手机+桌面陪伴**的普通用户 |
| 使用场景 | 部署在自己机器上的**主动陪伴桌宠**（Agent 干活、闲聊、记忆成长） | **定时直播**（玩 Minecraft/osu + 读聊天 + 唱歌） | 手机随身 / 桌面常驻的**语音陪伴** |
| 核心卖点（一句话） | **"一只会主动找你玩、能干活、能换皮、能装插件的全栈 AI 猫娘"** | **"完全由 AI 控制、看起来像真人主播还会读聊天、玩游戏、翻车的 AI VTuber"** | **"首个 MIT 全栈开源、免许可自编译的 Android+PC Live2D AI 伴侣"** |
| 平台 | 自部署 Docker 多服务（桌面为主） | Twitch + YouTube + Bilibili 直播 | Android + Windows 桌面 |
| 收入/分发 | 插件商城 + Steam 创意工坊 + 角色卡导出 | 订阅/打赏（SuperChat）+ 周边 + 官方周边 & 音乐单曲 | 开源 MIT，无商业化 |

来源:
- N.E.K.O. 定位：`docs/research/tts-research-report.md:12-16`（⭐2,382 / Apache 2.0 / 描述"一只会主动找你玩的 AI 猫娘"）；仓库/Docker 多服务与插件商城、Steam 创意工坊、角色卡由任务书给定 + `docs/research/tts-research-report.md:43-53`（多服务树）。
- Neuro‑sama 定位：NeuroWiki https://en.neurosama.info/wiki/Neuro-sama（"does not have a pre-defined 'persona'… exists purely as a streamer"）；https://en.wikipedia.org/wiki/Neuro-sama（"speech and personality powered by LLM + avatar + TTS"）；Vice https://www.vice.com/en/article/this-virtual-twitch-streamer-is-controlled-entirely-by-ai/（"controlled entirely by AI"）；Fandom https://neurosama.fandom.com/wiki/Neuro-sama（14 岁 anime girl、Hiyori 模型、osu 80×60、游戏AI Python/VR功能 C#）。
- Live2D-Ai 定位：`README.md:5-10`、`docs/architecture/ARCHITECTURE.md:21`（通用 Live2D 桌宠、两端独立直连无中心后端）。

---

## 2. 功能矩阵对比

> 图例：✅ 完整 / 🔶 部分或已实现未实测 / ❌ 缺失或空壳 / ❓ 未找到证据（标灰项标注待补充）

| 维度 | N.E.K.O. | Neuro‑sama | Live2D-Ai 现状 | 差距（Live2D-Ai 视角） |
|---|---|---|---|---|
| **主动陪伴** | ✅ 主动话题（`topic/` 主动话题推荐 + activity 状态跟踪） | ✅（直播本身就是主动输出，读 TTS/聊天、漫谈） | 🔶 主动插件已实现未实测（任务书 + 竞品表 my-neuro/OpenLLM-VTuber 主动对话） | 需把"主动话题源 + 时机判断"真正跑通 |
| **记忆系统** | ✅ 5 层记忆（事实/反思/人设/近远期/事件） | ❓官方记忆机制未公开；但"人设成长靠长期记忆回捞"是社区公认（社区制造的事件被其选入长期记忆） | 🔶 Room + 关键词（无反思共识层） | 缺"反思/人设固化"层——Neuro 的人设成长即源于此 |
| **多模态对话** | ✅ 语音/文字/视觉 Agent（browser/computer_use） | ✅ 读聊天文本 + 屏幕截图（vision）理解画面/游戏 | 🔶 语音通 / 视觉空壳（GLM-4.6V 能看但截屏链路弱） | 视觉→主动回应的闭环还没接 |
| **TTS 个性化** | ✅ 多 Provider 适配器（17 引擎）+ 音色克隆/自定义语音包 | ✅ 定制 TTS 音色（V1→V3 数次升级）；唱歌用独立 AI 唱歌 + 调音师 | ❌ CosyVoice 标准音（`docs/research/tts-research-report.md` §四） | 至少做到"可换引擎 + 可选音色"；远期为声音克隆 |
| **表情自然度** | ✅（多 Avatar 各有表情系统，细节未逐个核到） | ❓/✅ 官方未开源渲染；社区总结 = **预置表情/动作 toggle + 随时间/对话切换**，且明确"表情主要是视觉表现而非真情绪模拟" | ❌ 表情闪烁（`docs/research/NEURO_LIVE2D_RESEARCH.md:172` 已知） | "表情是给用户看的情绪+反馈"，不是真要模拟情绪——先解决闪烁、再做 fade 过渡 |
| **Avatar 形态** | ✅ 5 种（Live2D/VRM/MMD/PNGTuber/猫桌宠） | 1 种（Live2D，基于官方免费 Hiyori Momose 改模） | 1 种（Live2D/MaoPro/Shizuku） | 多形态是差异化（N.E.K.O. 最强）；Neuro 反而只靠 1 张脸 |
| **多平台/分发** | ✅ Steam + 桌面 + 插件商城 + 跨设备记忆同步 | Twitch/YT/Bili 直播（单一"演出"平台） | 🔶 Android + PC（两端共享 persona.yaml，无实时同步、无分发渠道） | 缺分发/分享机制（N.E.K.O. 的 Workshop/角色卡思路） |
| **游戏/干杂活** | ✅ Agent 工具执行（浏览器/文件/电脑控制） | ✅ 游戏 AI（osu 80×60 灰度视觉 + Minecraft）+ neuro-game-sdk 开放给第三方游戏 | 🔶 MCP 工具（time/search/weather/filesystem） | "看一眼画面/屏幕"→"主动做点事"这一环最像 Neuro，值得做 |
| **唱歌/娱乐** | ❓（未在既有调研中确认） | ✅ AI 唱歌插件（V1/V2/V3 三次重做）+ 官方单曲《Colorful Array》 | ❌ | Neuro 的唱歌是独立娱乐模块，可选 P2 |

来源:
- 主动陪伴：`docs/research/tts-research-report.md:54,57`（`topic/` 主动话题、`activity/` 状态跟踪）；Neuro‑sama NeuroWiki（"rambling much like a regular Chill stream"）；`docs/research/competitive-analysis-report.md:68,80`（OpenLLM/IS 主动说话、Live2D-Ai 主动消息 ⬜/部分）。
- 记忆：`docs/research/tts-research-report.md:49-52`（5 层记忆：facts/reflection/persona）；NeuroWiki（"fabricated events…later chosen by Neuro for long-term memory"）。
- 视觉：NeuroWiki（"watching the game via Vision-Language Models…analyzes screenshots…near real-time"）；zmescience（"analyzes screenshots of her desktop to react"）。
- TTS：`docs/research/tts-research-report.md:78-84`（17 引擎/克隆/语音包）；NeuroVoice Reddit（V1/V3 演进）、YouTube Vedal talks V3 voice；NeuroWiki（karacke 用 AI 唱歌 + tuning specialist queenpb）。
- 表情：`docs/research/NEURO_LIVE2D_RESEARCH.md:76-96,151-158`（自然感=眨眼+口型+表情 fade+多轨；Live2D-Ai 表情闪烁问题见 ARCHITECTURE 已知问题）；NeuroWiki（"expressions…toggling pre-configured animations…primarily a visual representation"）。
- Avatar：`docs/research/tts-research-report.md:56`（5 形态）；NeuroWiki Fandom（Hiyori Momose 模型）。
- 分发：任务书（N.E.K.O. 插件商城/Workshop/角色卡导出）。

**Neuro‑sama 的 6 个问题直接回答**：

1. **交互方式**：**「直播演出」——文本+语音混合**。主输入是 **Twitch 聊天文本**（观众打字 + SuperChat 打赏消息）+ 环境里的语音（用 Discord 音频源区分说话者方向）→ LLM 生成回复 → TTS 说出来。画面感知靠 **截图 → vision 模型**主动理解（看粉丝图、看游戏、连线直播），但平常低功耗模式会关掉 vision 省钱。游戏时是**专门游戏 AI**（osu 用 80×60 灰度截图 + 游戏AI；Minecraft 用视觉识别），不是靠通用 vision。来源：NeuroWiki；zmescience；Fandom；VedalAI/neuro-game-sdk。
2. **人设/性格**：**没有预设人设**。Neuro"没有预定义 persona，纯粹作为一个主播存在"，人设是**社区共创**——粉丝通过聊天/梗/催泪事件持续"投喂"，被 AI 选入长期记忆，逐渐固化出"腹黑但有成长"的个性。观众觉得"像真人"的心理学解释：**"一致性（consistency）而非人性化（humanness）锚定真实感"**—粉丝不是因为她像人，而是因为她**记得梗、记得过去、前后一致**，所以产生「她跟我共同成长」的陪伴感。来源：NeuroWiki；CHI'26 论文 https://arxiv.org/html/2509.10427 ；Annenberg https://www.asc.upenn.edu/news-events/news/what-makes-ai-livestreamer-seem-real 。
3. **表情/动作自然度**：**不是实时动捕，是"预置动画切换"**。Live2D 表情/动作是 `pre-configured face and body animations`，按对话内容、别人说的话、听到的歌声、放的音乐来 toggle。官方渲染未开源，社区复刻（my‑neuro / airi）证明社区共识 = **自动眨眼 + 注视跟随 + 空闲动画 + 声音→口型（音量/元音） + 表情 fade 过渡**——全部是"状态机+参数切换"，没有真人动捕参数。来源：NeuroWiki；`docs/research/NEURO_LIVE2D_RESEARCH.md:20-72`。
4. **技术栈（产品层）**：**LLM 大脑（生成要说的话）→ TTS 语音 → Live2D 脸 → 流媒体层（OBS + Twitch/YT chat 输入 + 音频路由）** + **独立游戏 AI**。动画能力用 C#（VTuber 功能），游戏 AI 用 Python。**Vedal 官方只开源了让 Neuro 玩游戏的外围 SDK（neuro-game-sdk，Unity/Godot WebSocket 协议）**，VTuber 渲染本体闭源。TTS 是定制音色、经数次升级以保住同一声音不换脸。来源：AnimArts blog https://animarts.studio/blog/how-to-build-ai-vtuber-bot-2026 ；Fandom；VedalAI/neuro-sdk；Vedal talks V3 Voice (YouTube)。
5. **社区生态**：社区复刻（**morettt/my‑neuro ⭐1.3k**、kimjammer/Neuro、AIRI 等）与 Vedal 官方是**"受启发/蹭热度但独立"的关系**。my‑neuro README 明说"受 neuro-sama 启发，名字是社区建议取的，一半是蹭热度"。**社区提供的价值**：官方不开源 VTuber 渲染 → 社区**用开源栈把"自然感"配方喂给了大家**（1 秒延迟、语音克隆、换脸换声、vision、长期记忆、主动对话、Bilibili 直播），并维护了 neuro-frontend、神经游戏 SDK 的社区版等。**Vedal 官方则反过来开放"神经游戏 SDK"，让第三方游戏接入 Neuro 来"一起玩"**——这是官方主动拥抱生态的分发策略。来源：morettt/my-neuro README/DeepWiki；VedalAI/neuro-sdk；`docs/research/competitive-analysis-report.md:125-131`。
6. **优点/缺点（产品级判断）**：
   - **做得好、值得抄**：① **陪伴感/人设成长**（社区共创 + 长期记忆回捞）——这是 IP 的灵魂；② **直播"演出感"**——把 AI 放在"边打游戏边聊梗"的真人主播场景，掩盖了技术糙点（观众注意力在"玩梗/翻车"上）；③ **记忆一致性**带来的"懂事感"；④ **开放神经游戏 SDK** 让生态来做内容；⑤ **同一声线持久经营**（V3 升级却不换音色）。
   - **不适合我们（或我们不做）**：① **"翻车/争议人设"**（Holocaust 否认、女性权利争议导致的 Twitch 封禁）——这是为了"有趣"故意做差的 edge 人设带来的风险，**对"陪伴"产品是致命的信任破坏**，我们绝不抄；② **纯直播/IP 演出形态**——我们是私密陪伴，不是公开演出；③ **为省资源关掉 vision**——直播场景的取舍，不是陪伴产品的取舍；④ 依赖大额打赏经济做内容，不可复制。

（优点/缺点综合来源：ksadov postmortem https://www.ksadov.com/posts/2024-05-16-aituber.html ；Kotaku/ScreenRant 封禁报道；NeuroWiki；Agent research https://lin-guanguo.github.io/llm-memory-research/neuro-sama.research/ 。）

---

## 3. 我们能直接借鉴的（产品设计/架构理念，不碰源码）

### 3.1 产品设计层（直接"抄理念"）

| 借鉴点 | 从谁抄 | 抄什么（产品层，不是代码） |
|---|---|---|
| **"陪伴感 > 技术"的核心理念** | Neuro‑sama | 用户要的是"**记得我、跟我一起成长**"，不是"响应快、答得准"。所有产品决策优先服务"长期一致性与成长感"。 |
| **记忆=人设成长引擎** | Neuro‑sama | 不只存聊天记录，而是**把"反反复复出现的话题/用户的梗/用户痛点"沉淀成长期人设**，下次主动用上——这是"懂事感"的来源。 |
| **主动输出机制** | Neuro + N.E.K.O. | 有一个"**什么时机主动说一句**"的触发源（N.E.K.O. 是 topic 推荐；Neuro 是直播漫谈）。把"主动找话题"从"插件未实测"升级成日常能力。 |
| **表情是"情绪+反馈"，不是真情绪** | Neuro + 社区 | 表情/动作用于**表达回应态度、让用户感到被关注**（笑、惊喜、沮丧），不必追求"模拟真情绪"。"自然"= 过渡平滑 + 随机不重复 + 眨眼/口型/注视做足。 |
| **一套脸 + 持久声线** | Neuro‑sama | Neuro 坚持"V3 升级不换音色"。**先固化一个专属音色/形象**形成品牌，再谈个性化迁移，用户黏性更强。 |
| **主动看画面、做点事** | Neuro + N.E.K.O. | "**看一眼用户屏幕 → 主动回应/主动一句话**"是差异点；N.E.K.O./Neuro 都靠它封神，我们已有视觉底座却"空壳"，把它闭环最值钱。 |

### 3.2 架构理念层（多服务 vs 单体 / 插件 vs 内置）

- **N.E.K.O. 的多服务解耦**（main / agent / memory / tts / brain / plugin 各自独立）印证：**把"记忆、Agent、TTS、核心对话"拆成可替换模块**，比塞进单体更利于"多形态 + 多 Provider + 插件"长期演进。Live2D-Ai 已是"LLM/TtsProvider 抽象"思路（`docs/research/tts-research-report.md:87-90` 建议的 TtsProvider 接口正好对齐），继续坚持抽象层即可，**不必照搬 Docker 多进程**（对我们 Android 端太重）。
- **插件 vs 内置**：**用插件承接"高频/通用"能力（记忆、视觉、主动、TTS provider），内置保住"分钟级可用"的核心链路**。这与 N.E.K.O."插件 SDK + 内置 Agent"、Live2D-Ai 现有 "ChatHook 插件 + 核心直驱" 的理念一致（`docs/research/competitive-analysis-report.md:67` 插件机制 ✅）。**教训**：插件不能让"表情/对话核心链路"断裂（Live2D-Ai 曾因此回归表情断线，见 `README.md` V1 验收）。
- **单服务直连 vs 中心后端**：Live2D-Ai 的"两端独立直连 + persona.yaml 单源真理"是优点（隐私、离线、无服务器成本），继续保留；**跨设备记忆同步**是 N.E.K.O. 亮点但属"牺牲隐私"取舍，应做成**用户可选的本地/私有云同步**，而非强制中心化。

### 3.3 社区 / UGC 策略

- **N.E.K.O. 的"角色卡导出 + Steam 创意工坊 + 插件商城"**是强分发杠杆：**"一键分享你的人设/换装/插件"，把每个用户变成内容创作者**。Live2D-Ai 已有人设 persona.yaml（`docs/architecture/ARCHITECTURE.md` 配置流），**可加一层"角色卡打包/导入/分享"**（对齐常见的 Character Cards 格式），成本低、生态价值高。
- **Neuro‑sama 的"开放神经游戏 SDK"**：Vedal 官方主动开放"让 AI 接入游戏"的接口、社区造内容。可作为**长期愿景**：开放"MCP 工具 + 屏幕感知"给开发者/用户自造玩法（我们已有 MCP 底座）。
- **社区复刻的商业价值**：my‑neuro 证明"受大 IP 启发、独立做本地化平价版"是可行的（换脸/换声/1 秒延迟/Bilibili），N.E.K.O. 证明 "一站式 + 主动 + 多形态" 是完整产品。可对标 my‑neuro 的"全本地 + 低延迟 + 换脸换声+Bilibili 直播"作为 P1 目标。

### 3.4 不自然问题的产品层解法（不碰渲染代码）

> 渲染层细节见 `docs/research/NEURO_LIVE2D_RESEARCH.md:20-72`，这里只讲产品层怎么调。

1. **表情闪烁**：产品上不要"一收到 `[emotion]` 标签就硬切"，而是**设定"情绪渐变不改跳 + 最频繁情绪维持模式"**（社区叫"表情 fade + 多轨分离"）。产品层落点 = **规则：不打断正在说的句子、只在句子间隙切、用淡入淡出**。
2. **口型/眨眼"假"**：产品层定义"**自然节奏**"（眨眼间隔 ~4 秒、口型随音量开合、说话时身体有 idle 摆动），把这些做成"默认手感"，不是逐参数去调。
3. **动作重复感**：给动作/表情加**随机化与频率抑制**（同一个动作不连续重复出现），让观感更"活"。
4. **"像真人在说话"**：最有效的产品层手段不是表情，而是**延迟节奏 + 语气一致 + 记忆回捞**（Neuro 的秘诀），表情只是锦上添花。

---

## 4. 我们不能/不适合借鉴的

| 不借鉴点 | 来源项目 | 为什么不适合我们 |
|---|---|---|
| **"翻车/争议/edge 人设"与擦边梗** | Neuro‑sama | 是因"蹭争议流量"翻车（Holocaust 否认导致 Twitch 封禁）的产物；对"陪伴/信任"产品是**致命信任破坏**，且国内监管风险更高。**坚决不学**。来源：Kotaku/ScreenRant/AIAAIC 报道。 |
| **纯直播 IP / 演出形态** | Neuro‑sama | 我们是**私密陪伴 + 手机随身**，不是"公开直播赚钱"。直播的"省钱关 vision、打赏驱动内容、定时演出"都不适用。 |
| **依赖单一主播 IP 的粉丝经济** | Neuro‑sama | 我们无 Vedal 那样的"人设领航员 + 调音师 queenpb + 长期运营"，无法复刻那套内容/打赏循环；我们只能做"工具/平台"层面的事。 |
| **Docker 多服务重部署** | N.E.K.O. | 完全照搬会让 Android 端无法嵌入、用户部署门槛过高（`docs/research/tts-research-report.md:96-97` 已指出部署复杂度 + 依赖云端）。我们保留抽象层、放弃多进程。 |
| **全部依赖云端 Realtime API** | N.E.K.O. | 我们硬性要求**离线可用 + 中国大陆直连**（`docs/research/tts-research-report.md` §四约束），不能把核心体验挂在单一云厂商上。N.E.K.O. 云端为主不适用我们。 |
| **"5 形态 Avatar"全量** | N.E.K.O. | 全做会摊薄投入。先保住 Live2D 做精，VRM/图片桌宠作为可选 P2 差异化即可。 |
| **"全自动 Agent 干电脑活"** | N.E.K.O. | 是爽点但**高风险**（权限/安全/用户的信任），且我们极简优先。宜做成**用户显式授权的可选 MCP 工具**，默认不越权。 |

---

## 5. 落地优先级

### P0 —— 马上做（成本低、命中两大 IP 的共同灵魂）

1. **"人设成长"记忆闭环**：把"反复出现的话题/用户的梗/用户的点"沉淀成长期人设并主动复用（抄 Neuro 的记忆=人设引擎）。现 Room+关键词 → 加**反思/固化层**。
2. **主动陪伴跑通**：把"主动话题插件"从"已实现未实测"变成日常（N.E.K.O. 的 `topic/` 思路）。
3. **视觉→主动回应闭环**：GLM 已能看，补"看一眼屏幕→主动一句/触发动作"（Neuro 的核心杀招）。
4. **表情"自然化"产品规则**：fade 过渡 + 句子间隙切换 + 随机抑制，消"闪烁感"（不碰渲染核，只加产品层规则）。

### P1 —— 值钱但需投入

5. **TtsProvider 抽象 + 可换引擎/可选音色**（`docs/research/tts-research-report.md` 四/五推荐的 P0/P1 项）——补"个性化音色"这一行缺口，向 Neuro 的"专属声线"靠拢。
6. **角色卡打包/导入/导出**（对齐 Character Cards + N.E.K.O. 角色卡导出）——低成本高生态价值的分发。
7. **多模型切换 UI + 本地/云端双轨**（对齐 my‑neuro 双轨、`docs/research/competitive-analysis-report.md:68` 多模型切换缺失项）。
8. **一套专属默认音色/形象品牌化**（Neuro 的"脸+声持久经营"）。

### P2 —— 长期愿景

9. **VRM/图片桌宠等多形态**（N.E.K.O. 差异化，择一可选做）。
10. **开放"AI 接游戏/玩法"接口**（Neuro‑sama 神经游戏 SDK + 我们 MCP 底座）——让社区造内容。
11. **跨设备记忆可选同步**（N.E.K.O. 亮点，做成可选私有云/本地）。
12. **AI 唱歌 / 娱乐模块**（Neuro 娱乐加法，非刚需）。

---

## 6. 调研证据索引（本报告引用）

| 主题 | URL/文档 |
|---|---|
| N.E.K.O. 架构/TTS/5层记忆/多服务 | `docs/research/tts-research-report.md:43-96` |
| Neuro‑sama 概览/人设无预设 | NeuroWiki https://en.neurosama.info/wiki/Neuro-sama ；https://en.wikipedia.org/wiki/Neuro-sama |
| Neuro‑sama 视觉/游戏理解 | NeuroWiki；https://www.zmescience.com/other/offbeat-other/an-ai-anime-girl-has-become-the-most-popular-streamer-on-twitch/ ；Fandom |
| Neuro 技术栈（产品层） | https://animarts.studio/blog/how-to-build-ai-vtuber-bot-2026 ；Fandom；VedalAI/neuro-game-sdk |
| Neuro SDK 开放 | https://github.com/VedalAI/neuro-sdk ；https://vedal.ai/ |
| 人设/社区共创学术 | https://arxiv.org/html/2509.10427 （My Favorite Streamer is an LLM, CHI'26）；https://www.asc.upenn.edu/news-events/news/what-makes-ai-livestreamer-seem-real |
| 表情/动作自然（复刻实证） | `docs/research/NEURO_LIVE2D_RESEARCH.md:20-72` |
| 唱歌/声线 | NeuroWiki（queenpb）；Reddit r/NeuroSama 声线目录；MusicBrainz《Colorful Array》 |
| 争议/封禁（不借鉴） | Kotaku https://kotaku.com/neuro-sama-twitch-vtuber-ban-holocaust-minecraft-ai-1849977269 ；ScreenRant；AIAAIC |
| 社区复刻 my‑neuro | morettt/my‑neuro README（https://github.com/morettt/my-neuro）+ DeepWiki；`docs/research/competitive-analysis-report.md:125-131` |
| Live2D-Ai 现状特征 | `README.md:5-75`；`docs/architecture/ARCHITECTURE.md`；`docs/research/competitive-analysis-report.md:15-100` |

---

## 7. 完整性声明 / 未找到项

- ✅ 两个项目产品定位/功能矩阵都已基于**可核证来源**（NeuroWiki、Wikipedia、Vice/zmescience/Kotaku、官方神经游戏 SDK、morettt/my‑neuro README 深度、本项目两份调研 + README/ARCHI）。
- ⚠️ **Neuro‑sama 官方 VTuber 渲染本体、精确记忆/系统提示实现闭源**→ 此类细节标"未找到，待补充"，本报告按要求不深挖代码，仅从产品理念与社区实证归纳。
- ⚠️ NeuroWiki / Wikipedia 等二级聚合源经搜索引擎引用，个别如"play the game using 80×60"多源（zmescience/Fandom/技术博客）交叉一致。
- ⚠️ N.E.K.O. 部分体验层细节（如唱歌、表情近实时质感）未逐个二源核到，已在功能矩阵中该列用 ❓/✅ 区分。
- ⚠️ 所有 star 数 / 数据为调研时点（2026-08-05）参考，可能变动。
