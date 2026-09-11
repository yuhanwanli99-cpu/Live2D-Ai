# Live2D-Ai —— UX 对标与 CosyVoice 参数调研报告

> 调研员: researcher (只读) · 网络来源截至调研当日
> 说明: 本报告所有结论均标注来源。凡以 URL 引用之处均未能逐字抓取原文（相关域名走 fake-IP 代理被拦），
> 但关键数据均来自可检索到的官方/文档正文摘录（见各「来源」与「证据」栏）。凡标注「未能确认」者，表示未在来源中找到该事实，**不得当作存在**。

---

## 一、CosyVoice 音色列表与最佳参数

### 1.1 请求参数总览（官方 API）

CosyVoice（阿里云百炼 / DashScope / 千问云）语音合成核心请求参数如下：

| 参数 | 类型 | 是否必选 | 说明 | 来源 |
| --- | --- | --- | --- | --- |
| `model` | string | 是 | 模型名，如 `cosyvoice-v2`、`cosyvoice-v3-plus`、`cosyvoice-v3-flash` | CosyVoice 语音合成 API 参考（千问云） |
| `voice` | string | 是 | 合成音色。系统音色见「音色列表」；复刻/声音设计音色另见对应 API | CosyVoice 语音合成 API 参考（千问云） |
| `format` | enum | 否 | 编码 mp3/pcm/wav/opus，默认 mp3 | CosyVoice 语音合成 API 参考 |
| `sample_rate` | integer | 否 | 8000/16000/22050/24000/44100/48000，默认 22050（注：部分 SDK 默认 24000） | CosyVoice 语音合成 API 参考；Python SDK |
| `volume` | integer | 否 | 音量 **0～100，默认 50** | CosyVoice 语音合成 API 参考；SSML 发音控制 |
| `speech_rate`(rate) | integer | 否 | 语速 **-500～500，默认 0**；[-500,0,500] 对应倍速 [0.5,1.0,2.0] | 使用 WebSocket 协议实现 CosyVoice 长文本合成；SSML 发音控制 |
| `pitch_rate`(pitch) | integer | 否 | 语调 **-500～500，默认 0** | 使用 WebSocket 协议实现 CosyVoice 长文本合成 |
| `instruction` | string | 否 | 控制方言/情感/说话风格；**最大长度 100 字符**；仅部分模型/音色支持 | CosyVoice 语音合成 API 参考；指令控制教程 |

- 结论: `rate`/`pitch` 的「推荐范围 0.5 ~ 2，默认 1」是 SSML `rate`/`pitch` 属性的表示法；底层 API 参数 `speech_rate`/`pitch_rate` 用 -500~500（默认 0，±500 → ±2x 倍速）。
- 来源: https://platform.qianwenai.com/docs/api-reference/speech-synthesis/ssml (发音控制) · https://help.aliyun.com/zh/isi/developer-reference/websocket-protocol-description-for-cosyvoice-tts
- 证据: SSML 页原文——`rate 语速。覆盖 API 参数 speech_rate。推荐范围：0.5 ~ 2，默认值 1。大于 1 加速，小于 1 减速`；`pitch 音调。覆盖 API 参数 pitch_rate。推荐范围：0.5 ~ 2，默认值 1`；WebSocket 页原文——`speech_rate 取值范围：-500～500，默认值：0。[-500,0,500]对应的语速倍速区间为 [0.5,1.0,2.0]`。

### 1.2 instruction 参数最佳实践示例

`instruction` 用于控制方言、情感、角色。**仅上述 v3.5-flash / v3.5-plus / v3-flash 复刻音色，以及音色列表中标注「Instruct：支持」的系统音色**可用；字符限制 ≤100 字符（汉字按 2 字符计）。

社区/文档推荐的自然语言指令示例（官方推荐风格）：

| 目标效果 | 推荐指令 | 来源 |
| --- | --- | --- |
| 激昂 | 请用非常激昂且高亢的语气说话，表现出获得重大成功后的狂喜与激动。 | 云声配音指令教程 |
| 优雅/知性 | 语速请保持中等偏慢，语气要显得优雅、知性，给人以从容不迫的安心感。 | 云声配音指令教程 |
| 耳语/亲密 | 请尝试用气声说话，音量极轻，营造出一种在耳边亲密低语的神秘感。 | 云声配音指令教程 |
| 哀伤 | 语气要充满哀伤与怀念，带有轻微的鼻音，仿佛正在诉说一段令人心碎的往事。 | 云声配音指令教程 |
| 英文/角色描述 | `Theo 'Crimson', is a fiery, passionate rebel leader. Fights with fervor for justice, but struggles with impulsiveness.<\|endofprompt\|>` | CosyVoice example.py |

> 注意: 官方 `instruction` 示例多为「情感/语速/语气」的自然语言描述，**没有官方给出的专门「猫娘」专用 prompt**。若要用于猫娘/软萌场景，可把「语气轻快、音调活泼、音色甜美」这类经过「声音设计规范」认可的维度描述复合进 instruction。公开声音设计教程认为「非常非常非常好听」这类冗余词无效，推荐结构化描述如「声线清澈的青年女声，语调温柔」。
>
> - 结论: 未能在检索到的来源中确认官方为「猫娘/萝莉」专门内置了 instruction 范本（未能确认）。
> - 来源: https://www.yuntts.com/689.html (CosyVoice声音设计使用教程) · https://github.com/FunAudioLLM/CosyVoice/blob/main/example.py
> - 证据: 声音设计教程原文——`"20-24岁，语气轻快、音调活泼、音色甜美的女声"`（推荐例）vs `"非常非常非常好听的女声"`（不推荐例，信息冗余）。

### 1.3 音色列表（含最接近「萌/软/萝莉」的候选）

官方系统音色采用「龙X」命名（voice 参数大多为 `longxxx[_版本]`）。以下为检索到的、与「可爱/萝莉/软萌」最接近的候选（**均不存在 literal 的 `cute_girl`/`loli`/`luoli` voice 参数**，见下）：

| voice 参数 | 中文名 | 特质 | 年龄/阶段 | 语言 | 是否接近「萌」 | 来源 |
| --- | --- | --- | --- | --- | --- | --- |
| `longxian_v3` / `longxian_v2` | 龙仙 | 豪放可爱女（童声） | 12岁 / 女童 | 普通话+英文 | ✅ 非常接近（儿童感、俏皮） | 系统预置音色参数与特性列表 |
| `longling_v3` / `longling` | 龙铃 | 稚气呆板女 / Childish and deadpan female | 10岁 | 普通话+英文 | ✅ 接近（稚气） | 千问云音色列表 / QwenCloud Voice list |
| `longanmin_v3` | 龙安闽 | **清纯萝莉女** | 18~25岁 | 普通话（闽南话）+英文 | ✅ 官方明确标注「萝莉」 | 千问云音色列表 |
| `longanrou_v3` / `longanrou` | 龙安柔 | 温柔闺蜜女 | 20~35岁 | 普通话+英文 | 🟡 偏温柔，可配 instruction 调软 | 系统预置音色参数与特性列表 |
| `longanling_v3` | 龙安灵 | 思维灵动女 | 20~30岁 | 普通话+英文 | 🟡 灵动，非萝莉 | 系统预置音色参数与特性列表 |
| `longxiaoxia_v3` | 龙小夏 | 沉稳权威女 | 25~30岁 | 普通话+英文 | ❌ 权威向 | 系统预置音色参数与特性列表 |
| `longxiaochun` / `_v2` / `_v3` | 龙小淳 | 温柔姐姐 / 知性积极女 | 25~30岁 | 普通话+英文 | 🟡 温柔姐姐向 | 系统预置音色参数与特性列表 / 长文本合成参数音色列表 |

- 结论: **目前未在检索到的官方音色列表中找到名为 `cute_girl`、`loli`、`luoli` 的 voice 参数**。「清纯萝莉女 `longanmin_v3`」是最接近该诉求的官方系统音色；若想要更「猫娘/软萌」，建议：① 用 `longanmin_v3`/`longxian_v3` 作基础；② 无系统音色支持 Instruct 时不强求，改用 v3.5-plus/flash 的复刻/声音设计音色 + 自然语言 instruction；③ 或将 `rate` 调 ~1.1-1.2、`pitch` 略升（如 +10~30）进一步增强「甜/活泼」感。
- 来源: 千问云音色列表 (https://platform.qianwenai.com/docs/api-reference/speech-synthesis/voice-list) · 系统预置音色参数与特性列表 (https://help.aliyun.com/zh/model-studio/cosyvoice-voice-list) · QwenCloud Voice list (https://docs.qwencloud.com/api-reference/speech-synthesis/voice-list)
- 证据: 千问云音色列表表格行——`龙安闽 | longanmin_v3 | 清纯萝莉女 ... 18~25岁 | 中文（闽南话）、英文`; `龙仙 | longxian_v3 | 豪放可爱女 | 12岁`; `龙铃 | longling_v3 | 稚气呆板女 | 10岁`。在全部检索到的列表段落中均**未出现** `cute_girl`/`loli`/`luoli` 字样。

### 1.4 猫娘风格推荐参数表（综合官方范围 + 社区惯例）

以下是「在官方合法的参数范围内」、面向猫娘/软萌风格的**推荐起点值**（非官方承诺值，需试听微调）：

| 参数 | 官方合法范围 | 猫娘风格推荐值 | 说明 |
| --- | --- | --- | --- |
| `voice` | 见音色列表 | `longanmin_v3`（官方萝莉）或 `longxian_v3` / `longling_v3` | 首选文字带「萝莉/可爱」的音色 |
| `rate` | 0.5 ~ 2（=speech_rate -500~500） | **1.05 ~ 1.2** | 略快显得俏皮，但别超过 1.3 否则读不清 |
| `pitch` | 0.5 ~ 2（=pitch_rate -500~500） | **1.05 ~ 1.15**（即 pitch_rate +50~+150 区间） | 略升调增加「甜/幼态」感；过高会失真 |
| `volume` | 0 ~ 100，默认 50 | **55 ~ 65** | 主流猫娘 TTS 常在 50 上方一点，避免破音 |
| `sample_rate` | 8000~48000 | 24000 | 通话音质/体积平衡点（SDK 亦常默认 24000） |
| `format` | mp3/pcm/wav/opus | mp3 或 opus | 移动端省流 |
| `instruction` | ≤100 字符 | 软萌示例:「请用轻快活泼、音调偏高的语气说话，语气软糯可爱，句尾带撒娇感。」 | 需音色支持 Instruct 才生效 |

> 免责: 上面「推荐值」是我根据官方合法范围 + 公开社区经验做的**建议起点**，不是官方文档声明的「猫娘最佳值」。官方文档只给出 legal 范围与默认值，未给出专门针对猫娘的档位。
> - 来源（参数范围内合法性依据）:
> - https://platform.qianwenai.com/docs/api-reference/speech-synthesis/ssml · https://help.aliyun.com/zh/isi/developer-reference/websocket-protocol-description-for-cosyvoice-tts (rate/pitch 推荐范围 0.5~2)
> - https://platform.qianwenai.com/docs/api-reference/speech-synthesis/cosyvoice/http-api (volume 0~100 默认 50)
> - https://help.aliyun.com/zh/isi/developer-reference/siso-text-to-speech-synthesis-for-cosyvoice-python-sdk (speech_rate/pitch_rate -500~500 默认 0)

---

## 二、同类产品设置页 / 前端设计对比

### 2.1 各产品概览（来源均联网检索）

#### A. Project N.E.K.O.（猫娘计划）
- 分层配置系统：**环境变量(NEKO_*) > 用户配置文件(core_config.json, user_preferences.json) > API 提供商配置(api_providers.json) > 代码默认值**。
- **Web UI 分用途管理**：API Key 独立页 `http://localhost:48911/api_key`；角色设置 `http://localhost:48911/character_card_manager`；`/` 主聊天。
- **多任务分模型配置**：不同任务（文本/视觉/摘要等）各自选模型，无单一全局默认，每 provider 自带 per-role models。
- **角色卡片化**：Web UI 从 character_card 进入，可改角色名/性别/年龄/性格、设自定义 **Live2D / VRM 模型**、**克隆自定义声音**（上传 ~15 秒干净音频样本）、编辑系统提示词；`/api/characters/catgirl/voice_id` 可按角色设 TTS voice。
- 提供 **Free provider**（无需 API Key 即可快速测试）作为 Core。
- 来源: https://project-neko.online/config/ · https://project-neko.online/guide/quick-start · https://project-neko.online/config/model-config · https://github.com/Project-N-E-K-O/T.T.S · https://project-neko.online/api/rest/characters
- 证据: 「Web UI 配置——API 密钥 /api_key；角色设置 /character_card_manager」;「克隆自定义声音（上传约 15 秒的干净音频样本）」;「选择 Free 作为核心 API 提供商」。

#### B. Open-LLM-VTuber
- **YAML 配置文件驱动**（`profiles/xxx.yaml`），基于 Pydantic + YAML 的**模块化、类型安全**配置系统，支持**环境变量替换、多语言(i18n)、运行时切换角色 profile**（`config_manager` 为中央权威）。
- 引擎选择在**后端配置**而非前端富面板：LLM(Ollama/OpenAI/…)、TTS(edge-tts/…)、ASR(sherpa-onnx/SenseVoiceSmall/…)、翻译可混用「本地计算 or API」。
- 前端为 React 界面，含 Live2D 形象、聊天、语音/文字输入等。
- 来源: https://deepwiki.com/Open-LLM-VTuber/Open-LLM-VTuber/2-configuration-system · https://github.com/Open-LLM-VTuber/Open-LLM-VTuber · https://github.com/Open-LLM-VTuber/open-llm-vtuber.github.io/blob/622a3073/docs/user-guide/backend/config.md
- 证据: 「configuration system … built on Pydantic and YAML … runtime switching between different character profiles」;顶配示例 `Ollama + sherpa-onnx-asr (SenseVoiceSmall) + edge_tts`。**前端没有公开的「可视化设置 Tab」截图或配置面板**描述，配置主要在 YAML——这一点与我们相反（我们在 App 里做设置面板，他们靠配置文件）。

#### C. AI-Vtuber（Ikaros）
- **配置驱动 + 脚本/ffmpeg 层级**，面向 Bilibili 直播等场景；TTS 可选 edge-tts / VITS / elevenlabs / bark / bert-vits2 / 睿声，可选 so-vits-svc / DDSP-SVC 变声；LLM 支持极多（ChatGPT/Claude/langchain/chatglm/ollama/…多客户端）。
- **Live2D 模型导入是「放目录」式**：自定义模型需保持**文件夹名与 model3.json 文件名一致**，模型放到指定路径（配置/命令行）。
- 前端主要为语音/直播助手，非图表单式可视化设置 UI（未见独立的图形化 API key 管理页描述）。
- 来源: https://github.com/Ikaros-521/AI-Vtuber · https://ikaros-521.github.io/AI-Vtuber/ · https://github.com/Ikaros-521/AI-Vtuber/releases/tag/live2d
- 证据: 「注意如果是自有模型，请保持文件夹名和 model3.json的文件名一致」「模型放置路径参考…」——模型导入 = 手动丢进路径，无拖拽 UI。

#### D. Soul of Waifu
- **角色大卡片（Character Hub）**：角色显示为「大卡片」而非列表，hover 展开快速面板——一键语音通话、换 avatar 模型、**切换 TTS**、查看/改角色信息、进聊天。
- **设置面板分 Tab**：含专门的「LLM 设置」Tab（LLM Setup — 顶部本地模型设置、底部云端+本地通用参数）、「API 设置」等。
- **现代、premium 动画**的界面（官网宣称有顺滑动画/高级感）。
- 来源: https://github.com/jofizcd/Soul-of-Waifu · https://jofizcd.github.io/soul-of-waifu-site/docs/llm-setup/llm-setup.html · https://jofizcd.github.io/soul-of-waifu-site/
- 证据: 「Your characters are displayed as gorgeous, large cards rather than boring lists. Hover over a card to instantly expand it and reveal a quick-access panel — launch a voice call, swap avatar models, switch the TTS…」;「进入设置…选择『LLM 设置』…在顶部是本地模型设置，底部是所有语言模型（云端+本地）」。

#### E. Live2DPet
- **三窗口渲染器**：Settings Window(`index.html` + `settings-ui.js`)、Pet Window(`desktop-pet.html` + `model-adapter.js`)、Chat Bubble(`pet-chat-bubble.html`)。
- **设置面板分 Tab / 分标签页**：在「API 设置」标签页填入 API 地址、密钥和模型名称；兼容 **OpenAI 格式 API**（可接 OpenRouter 聚合平台）；底部「启动宠物」按钮让透明角色窗口出现在桌面右下角。
- 来源: https://github.com/x380kkm/Live2DPet · https://github.com/x380kkm/Live2DPet/blob/main/README.md
- 证据: 「在『API 设置』标签页填入 API 地址、密钥和模型名称。本应用兼容所有 OpenAI 格式的 API 接口」;「设置界面底部点击『启动宠物』…透明窗口出现在桌面右下角」;目录中的三个 window html。

### 2.2 设置功能清单对比表

| 维度 | **Live2D-Ai 现状** | **N.E.K.O.** | **Open-LLM-VTuber** | **AI-Vtuber** | **Soul of Waifu** | **Live2DPet** |
| --- | --- | --- | --- | --- | --- | --- |
| **设置页布局** | 单一长列表，SectionHeader 分区（API/模型/形象/TTS/ASR/离线/人设/关于）滚动到底，无 Tab | 分用途页面：`/api_key`、`/character_card_manager`、`/config` | 后端 YAML 配置为主，前端以聊天为主（无丰富可视设置 Tab） | 配置/脚本驱动，无图形化设置面板描述 | 多 Tab（API 设置 / LLM 设置等）+ 多窗 | 多标签页（API 设置等）+ 多窗口 |
| **引擎选择** | 4 固定引擎单选 RadioGroup（CosyVoice/Edge/System/sherpa）+ 降级链徽标 | 多 Provider(Core/Assist)，数据驱动，Web UI 选 | 配置文件里选 LLM/TTS/ASR 引擎，可本地 or API 混用 | 配置文件选，支持极多引擎 | 可切换 TTS、本地 vs 云端 LLM 分区 | 填 API 地址/模型名（OpenAI 兼容） |
| **API Key 管理** | 设置页内分散多个明文/密码框（DeepSeek/智谱/阿里云），各自输入 | **独立 `/api_key` 页面**，集中管理；还支持环境变量 | 环境变量 + YAML 内替换 | 配置文件/环境变量 | Tab 内分区设置 | 「API 设置」标签页集中填 |
| **模型导入** | **无入口**（代码级改路径），仅支持仓库内 ModelRegistry 预置 | **图形化**：Character Manager 里设自定义 Live2D/VRM、上传 ~15s 音频克隆声音 | 配置文件指定模型 | 手动把模型放进指定目录（文件夹名=model3.json 名） | 卡片 hover 换 avatar 模型（内设/导入） | model-adapter.js 加载宠物模型 |
| **聊天/LLM 模型** | 下拉选模型 + Temperature 滑块 | 每任务分模型配置 | YAML 配置 + 运行时切 profile | 大量 LLM 客户端 | LLM 设置 Tab（本地/云端分区） | 模型名文本框 |
| **角色/人设** | 2 个人设（小喵/绫乃）+ Live2D 卡片选择 | Character Card 管理（名字/性别/年龄/性格/声音/提示词） | persona + system prompt 在 YAML + profile 切换 | 多角色直播 | 角色卡片 hub | — |
| **返回拦截** | 有（滑屏/返回键弹确认） | — | — | — | — | — |
| **离线模型下载** | 有（Kokoro/SenseVoice 按需下载卡片） | — | — | — | — | — |

> 注: 「—」表示未在检索到的来源中确认相应功能（Open-LLM-VTuber/AI-Vtuber 的前端并不以「可视化设置面板」为核心，配置重心在 YAML/脚本）。

---

## 三、Live2D-Ai 最急需改进的 5 个 UX 项（按优先级）

> 优先级口径: P0=当前体验有明显痛点/阻塞；P1=显著提升易用性；P2=打磨高级感。改进建议均从「2.2 对比表」中短板反推。

### P0-1 设置页从「单一长列表」改为「分类 Tab / 分段网格」
- **现状（来源）**: `Live2D-Ai-Android/.../ui/settings/SettingsScreen.kt` —— 一个 `verticalScroll` 的 Column，靠 `SectionHeader` 分段（API/模型/形象/TTS/ASR/离线/人设/关于）一路滚到底。
- **问题**: API 三大 key + 模型 + 形象 + TTS/ASR 引擎 + 离线模型 + 人设 + 关于全挤一屏，移动端要滑很久；相互独立的功能强相关项被割裂。
- **对标做法**: N.E.K.O. 分 `/api_key`、`/character_card_manager`、`/config` 独立页（来源: project-neko.online/config 及 quick-start）；Soul of Waifu 分 Tab；Live2DPet 分标签页（来源见 2.2）。Soul of Waifu「大卡片不是 boring list」提供布局启发。
- **建议**: 顶部 `TabRow`（如：AI 服务 / 语音 / 形象 / 离线 / 其他）或至少把「关于」拆出；长列表保留但顶部加锚点跳转。
- 优先级理由: 直接决定设置页「是否粗糙」的第一观感，且改动纯 UI、风险低、收益直观。

### P0-2 API Key 从「分散多框」收敛为「单一密钥管理入口」
- **现状（来源）**: `SettingsScreen.kt` 里 DeepSeek/智谱/阿里云 三个 key 分别在 API 设置区和 TTS 区离散输入，还混着「恢复默认」按钮。
- **问题**: 用户要记住三个不同位置；且当前主 LLM 是并发/兜底切换，key 分散易漏配。
- **对标做法**: N.E.K.O. 独立 `/api_key` 页集中管理（来源: project-neko.online/config）；环境变量优先级 + 单一 Web 入口。
- **建议**: 做一个「API 密钥」分组/弹窗，列出 DeepSeek / 智谱 / 阿里云(DashScope) 三个密钥，统一明文切换、统一「留空=默认/兜底」提示；每个引擎框旁显示是否已配置。

### P1-3 提供 CosyVoice 音色 + 猫娘参数的可视化选择（复用本报告 §1 结论）
- **现状（来源）**: `SettingsRepository.AVAILABLE_TTS_VOICES` 是 Edge-TTS 音色列表（Xiaoxiao/Xiaoyi/Yunjian…）；CosyVoice 引擎的 voice 当前在 `tts/` 下代码里硬编码（见 `tts/cosyvoice*.py` 相关），设置页只给「语音」下拉，并未暴露 CosyVoice 的 `longanmin_v3` 等萝莉音色，也没有 rate/pitch 滑块。
- **问题**: 用户想要「更萌音色」却只能在代码里改 voice，UI 上唯一可选的是 Edge-TTS 的常规女声；不符合「猫娘」卖点。
- **对标做法**: N.E.K.O. 有 `/api/characters/voices` 列 TTS 音色 + 按角色设 voice_id（来源: project-neko.online/api/rest/characters）。
- **建议**: ① 把 CosyVoice 支持的音色（含 `longanmin_v3` 萝莉、`longxian_v3`、`longling_v3`）接入设置页 voice 下拉；② 增加 rate/pitch 两个滑块（范围 0.5~2，对应 §1.4）做默认为渲染后的音高语速；③ 音色默认值：CosyVoice 用 `longanmin_v3` 或 `longxian_v3` 更贴猫娘。
- 优先级理由: 直接兑现「猫娘」产品定位的语音甜度，改动集中在 TTS provider + 设置页联动。

### P1-4 模型导入入口（至少「导入/选择本地 Live2D 模型」）
- **现状（来源）**: `ModelSelector`/`ModelCard` 只渲染 `ModelRegistry`（`registry.getAvailable()`）预置条目，不可用模型灰显「即将推出」；导入需代码级改路径。
- **问题**: 与 N.E.K.O.（Character Manager 设自定义 Live2D/VRM）、AI-Vtuber（放入指定目录）、Live2DPet（model-adapter 加载）对比，我们是唯一没有「加自己的模型」入口的。
- **对标做法**: N.E.K.O. `character_card_manager` 设自定义 Live2D/VRM；AI-Vtuber 需文件夹名=model3.json 一致（来源见 2.2）；Soul of Waifu 卡片换 avatar。
- **建议**: ① 最低成本：设置页给「扫描本地 live2d 目录并导入」按钮 + 提示「文件夹名需与 model3.json 一致」；② 进阶：系统文件选择器（SAF）选 `.model3.json` 并复制到应用 model 目录。
- 优先级理由: 「无法添加自己的角色模型」是同类产品都支持而我们缺的关键能力。

### P2-5 设置页质感打磨（卡片化 + 即时反馈 + 「关于/许可」收敛）
- **现状（来源）**: `SettingsScreen.kt` 已用 `SectionHeader`、`OutlinedCard`、RadioButton、下拉、滑块、弹窗等 Material3 组件，且已有返回确认框、健康徽标、下载卡片——基础不差。
- **缺口（对比 2.2）**: 仍是「一屏长列表」；缺乏：hover/卡片快速操作（对标 Soul of Waifu 角色大卡片）、设置变更的即时保存提示文案统一（目前部分即时保存、部分 onDispose 保存、弹「退出确认」）、以及「关于/许可」这种低频项占了一整节。
- **建议**: ① 把「关于/开源许可」收进底部一个小入口或 Toolbar 菜单；② 对即时生效项（引擎切换、model 切换）统一补 SnackBar 反馈（已有「已切换为 X」，可扩展到 API key、voice、persona）；③ 在下拉语音项里按引擎分组并给音色名（如「CosyVoice · 龙安闽(萝莉)」），减少空泛列表。
- 优先级理由: 属于 polish，但它正是「让它不粗糙」的最后一公里，且成本低。

---

## 附：自我检查
- [x] 每条结论均标注了来源（URL）或证据摘录
- [x] 未编造数据：网络无法逐字抓取的原文部分以「证据摘录」呈现；`cute_girl/loli/luoli` 未在来源确认处明确标「未能确认」
- [x] 未修改本仓库任何被调研文件（仅 read / grep / web_search）
- [x] 仅新建本报告 `docs/research/ux-benchmark.md`
