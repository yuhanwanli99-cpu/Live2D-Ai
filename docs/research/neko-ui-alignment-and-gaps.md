# Live2D-Ai · UI 对标（N.E.K.O）与「最小 Live2D+AI+TTS+高级感」能力缺口调研

> 调研员：产品 + 前端架构调研员（只读）
> 日期：2026-08-20
> 范围：仅 PC 端渲染层（`Live2D-Ai-pc/open-llm-vtuber/renderer/`）现状核对 + 现网 UI 对标 + 缺口清单。
> 纪律：本项目现状断言一律附 `文件:行`；网络信息附 markdown 链接；明确区分「✅ 已验证事实」与「🔶 推测」。

---

## 结论先行（最关键的 5 个差距）

1. **没有桌面壳**：PC 端现在只是「浏览器打开 `http://localhost:12393/`」，透明置顶悬浮窗 / 点击穿透 / 系统托盘 / 开机自启 / 全局快捷键**全部未落地**——`background: transparent` 只存在于 CSS 注释里（`renderer/index.html:12`、`renderer/src/main.ts:115`），`package.json` 无 Electron/Tauri 依赖。**这是「算不算桌宠」的身份级缺口，排第一。**
2. **两套割裂的视觉系统**：聊天面板是 index.html 内联 CSS（靛蓝 `#6366f1` + 320px 半透明），设置中心是 `settings.css`（品牌蓝 `#4176e6` + 13px 深色）——主题色、字号阶梯、字体栈、阴影层级各写各的，没有统一 design token。
3. **几乎没有动效与过渡体系**：面板开关是 `display:none ↔ flex` 硬切、消息上屏无打字/渐入、tab 切换无过渡、无空状态 / 无「AI 思考中」指示 / 无 onboarding——只有按钮 120ms 的 hover 微过渡和 spinner。
4. **聊天历史不持久化、不可回看**：消息只存内存 DOM（`chat-ui.ts:67-76`），刷新即丢；后端无聊天历史 API（`routes.py` 无 `/api/chat-history`）。
5. **语音 / VAD 打断没有用户可见反馈**：后端已实现 Silero VAD + barge-in 打断（`websocket_handler.py:379-397,502-509`），但前端 mic 按钮走的是浏览器 Web Speech API（`chat-ui.ts:90-131`），「聆听中 / 说话中 / 打断」没有任何视觉指示，两条语音链路还互相脱节。

---

## 目录

1. [调研方法与证据纪律](#1-调研方法与证据纪律)
2. [Part A：N.E.K.O 及同类产品 UI 对标](#2-part-aneiko-及同类产品-ui-对标)
   - 2.1 [同类 UI 结构拆解](#21-同类-ui-结构拆解)
   - 2.2 [逐项对比表](#22-逐项对比表)
   - 2.3 [「看起来廉价」的具体成因](#23-看起来廉价的具体成因)
3. [Part B：能力缺口清单](#3-part-b能力缺口清单)
   - 3.1 [P0 必须有](#31-p0-必须有)
   - 3.2 [P1 应该有](#32-p1-应该有)
   - 3.3 [P2 加分](#33-p2-加分)
   - 3.4 [明确不做](#34-明确不做)
   - 3.5 [16 项常见缺口逐一核查](#35-16-项常见缺口逐一核查)
4. [参考链接索引](#4-参考链接索引)

---

## 1. 调研方法与证据纪律

### 已读本项目文件（现状依据）

- `Live2D-Ai-pc/open-llm-vtuber/renderer/src/main.ts`（584 行）、`chat-ui.ts`（151）、`settings-ui.ts`（1350）、`settings-logic.ts`（339）、`ws-bridge.ts`（304）、`settings.css`（615）、`index.html`（187 内联 CSS）、`package.json`、`vite.config.ts`
- `Live2D-Ai-pc/open-llm-vtuber/server.py`、`routes.py`、`websocket_handler.py`、`vad/silero.py`（部分 grep）
- 前期调研：`benchmark-neko-vs-neurosama.md`、`NEURO_LIVE2D_RESEARCH.md`、`core-fusion-research.md`、`tts-research-report.md`、`ux-benchmark.md`、`competitive-analysis-report.md`
- 架构/规划：`ARCHITECTURE.md`、`core-contracts.md`、`Phase-0-architecture.md`、`README.md`、`PLAN.md`、`CHANGELOG.md`

### 事实 / 推测区分约定

- ✅ **已验证事实**：直接来自本项目文件（含行号）或官方文档原文摘录。
- 🔶 **推测**：来自二手聚合（README/社区）或对同类的合理外推，未逐字核到官方截图。
- 图标列中 `⬜` = 无实现；`🔶` = 部分实现；`✅` = 已实现。

### 关键架构澄清（影响对标口径）

- PC 端存在**两套前端**：旧的预编译 React SPA 在 `frontend/`，新的 TS+PixiJS 渲染层在 `renderer/`。**当前由 FastAPI 挂在 `/` 的是 `frontend/`（`server.py:174-177`），而 `renderer/` 经 Vite 构建后 `outDir: '../frontend'` 覆盖它（`vite.config.ts`）**——即「renderer 就是当前 PC 前端」。本报告以 `renderer/` 为准。
- `backgrounds/` 目录与 `/backgrounds` 静态挂载（`server.py:151-154`）、`fetch-backgrounds` WS 处理器（`websocket_handler.py:94,560-566`）是 **Open-LLM-VTuber 上游遗留**，新 renderer 并未接线——背景图切换在本项目**未实现**。

---

## 2. Part A：N.E.K.O 及同类产品 UI 对标

### 2.1 同类 UI 结构拆解

#### 2.1.1 N.E.K.O（猫娘计划，Project-N-E-K-O）

> 来源：`docs/research/tts-research-report.md:10-107`（GitHub 结构树 + README 原文）、`docs/research/ux-benchmark.md:94-101`、[GitHub 仓库](https://github.com/Project-N-E-K-O/N.E.K.O)、[Steam 商店 / v0.8.x 更新](https://steamdb.info/patchnotes/23581477/)。

**结构（✅ 已验证，基于 README 结构树与官方配置文档）**：

| 区域 | N.E.K.O 形态 |
|---|---|
| 主画面 / 角色区 | 多 Avatar 形态（Live2D / VRM / MMD / PNGTuber / 猫桌宠 5 种），桌面常驻主动贴贴 |
| 对话区 | 主聊天页 `/`（文本 + 实时语音 Realtime API 对话） |
| 输入区 | 文字输入 + 语音实时对话；「主动找你玩」的主动话题推荐（`topic/`） |
| 设置区 | **按用途分页**：`/api_key`（独立密钥页）、`/character_card_manager`（角色卡：名字/性别/年龄/性格/自定义 Live2D/VRM 模型/克隆声音/系统提示词）、`/config`（多任务分模型） |
| 状态指示 | 环境变量 `NEKO_*` > 用户配置 > API 配置 > 默认值 的分层配置可见性 |
| 悬浮 / 置顶 / 透明 / 托盘 | 🔶 未逐字核到官方截图；但作为「桌面猫娘 + Steam 发行」产品，桌面常驻壳是其产品前提（此项归为「同类通用做法」，见 2.1.2） |

**可借鉴点**：① 角色卡中心化（一个地方改名字/性格/声音/模型/提示词，非散落多处）；② 独立 API Key 页；③ 声音克隆（上传 ~15 秒干净音频）；④ Free provider 零门槛起步；⑤ 主动陪伴（非只被动响应）。

#### 2.1.2 桌面壳通用模式（Open-LLM-VTuber / Live2DPet / Meuxe / Nexus / Soul of Waifu）

> 这是「AI 桌宠」区别于「AI 聊天网页」的决定性 UI 层，本项目恰恰缺这一层。

- **Open-LLM-VTuber**（本项目 PC 底座）：官方文档有 [窗口 & 桌宠模式（Electron）](https://docs.llmvtuber.com/en/docs/user-guide/frontend/electron/#desktop-pet-mode) —— 透明穿透桌宠模式，与普通窗口模式切换。✅
- **Live2DPet**（Electron，⭐73）：三窗口渲染（设置窗 / 宠物窗 / 聊天气泡窗），「设置界面底部点『启动宠物』→ 透明角色窗口出现在桌面右下角」。✅（来源 `ux-benchmark.md:124-128`）
- **Meuxe**（Tauri 2 + Rust + React）、**Nexus**（Electron + React）——透明置顶常驻。✅（来源 `competitive-analysis-report.md:146-159`）
- **Amadeus System**（PyQt，Steins;Gate 风格 AI 伴侣）：🔶 见 [Amadeus System 文档](https://docs.amadeus-web.top/en/)、[贴吧更新帖](https://tieba.baidu.com/p/8345668436)、[Trae 论坛介绍](https://forum.trae.cn/t/topic/23118)——确为「可陪伴、可执行的个人 AI 记忆体 + Live2D」桌面系统，但详细 UI 结构未逐字核到截图，本报告不逐项引用其 UI。

**桌面壳通用能力清单（✅ 同类共识）**：透明背景 + 无边框 + 置顶（always-on-top）+ 点击穿透（click-through，角色可透传鼠标）+ 系统托盘（托盘菜单：显示/隐藏/退出）+ 全局快捷键唤起 + 开机自启。

#### 2.1.3 Neuro-sama 直播叠层（对照）

> 来源 `benchmark-neko-vs-neurosama.md:18-71`、`NEURO_LIVE2D_RESEARCH.md`。

Neuro-sama 是「直播演出」而非桌宠产品：主画面 = OBS 叠层里的 Live2D 形象 + 字幕条；对话区 = Twitch/YT 聊天；无独立设置面板面向普通用户。**对标价值在于「自然感配方」而非 UI 布局**：自动眨眼（4s 间隔）+ 注视跟随 + 空闲动画 + 音量→口型 + 表情 fade，全部是「状态机 + 参数切换」而非真人动捕。这是本项目 `renderer` 已经在做的方向（`main.ts` 已接 blink/breath/microExpr/idleMotion/lipSync/emotion，见 `main.ts:334-342`）。

---

### 2.2 逐项对比表

> 图例：优先级 P0 = 缺了不算完成品；P1 = 明显影响体验；P2 = 高级感来源。

| 能力项 | N.E.K.O / 同类表现 | 本项目现状（文件:行证据） | 差距 | 优先级 |
|---|---|---|---|---|
| **主画面 / 角色区** | 多形态 Avatar 常驻桌面，主动贴贴；Live2DPet「透明角色窗口在桌面右下角」 | PixiJS 透明画布渲染 Live2D（`main.ts:117-124` `backgroundAlpha:0`），默认模型 `bai` 兜底（`main.ts:53,324-332`），可拖拽+滚轮缩放（`main.ts:349-421`） | 有角色渲染 + 位姿交互，但**无桌面常驻壳**、无主动行为 | P1 |
| **对话区** | 主聊天页（文本+语音）；Neuro 叠层字幕条 | 右侧 320px 半透明面板，消息流 + 气泡（`index.html:23-38`、`chat-ui.ts:44-57`）；字幕走 `onSubtitle`（`main.ts:450-452`） | 有基础对话面板；**无空状态/无打字指示/无流式逐字/无历史** | P1 |
| **输入区** | 文字输入 + Realtime 语音 | 文本框 + 发送 + 麦克风（`chat-ui.ts:52-56`）；语音走**浏览器 Web Speech API**（`chat-ui.ts:90-131`），非后端 ASR | 文字可用；语音链路与后端脱节、无 VAD 反馈 | P1 |
| **设置区** | 分页（api_key / character_card_manager / config） | 6 分区左导航 Modal（`settings-ui.ts:212-227`），含 LLM/TTS/密钥/人设/形象/系统 | 结构已接近 NEKO 分页思路，但**风格与聊天面板割裂**（见 2.3） | P1 |
| **状态指示** | 分层配置可见性（环境变量>用户>API>默认） | 聊天头部「未连接/已连接 + TTS 状态」（`chat-ui.ts:44-57,138-149`）；设置面板导航状态简报（`settings-ui.ts:221-226,442-459`） | 有基础连接/TTS 指示；**无语音/VAD/打断/思考中指示** | P1 |
| **悬浮与置顶** | ✅ 同类通用（always-on-top 桌宠） | ❌ 无（仅 CSS 注释「配合 Electron/Tauri 桌面壳」`index.html:12`、`main.ts:115`；`package.json:13-16` 无壳依赖） | **完全缺失** | **P0** |
| **透明穿透** | ✅ Open-LLM-VTuber 桌宠模式 click-through | ❌ 无（同上） | **完全缺失** | **P0** |
| **托盘 / 快捷键** | ✅ 同类通用（托盘菜单 + 全局唤起） | ❌ 无（grep `Tray/globalShortcut/registerHotkey` 无命中） | **完全缺失** | **P0** |
| **开机自启** | 🔶 同类部分有 | ❌ 无 | 缺失 | P1 |
| **空状态 / 加载态** | 🔶 同类普遍有欢迎态 | 设置面板有「加载模型列表…/暂无 Provider」（`settings-ui.ts:476,563,739`）；**聊天区完全无**（欢迎语、空态、thinking 指示皆无） | 半缺失 | P1 |
| **动效与过渡** | 🔶 Soul of Waifu 宣称「顺滑动画/高级感」 | 仅按钮 120ms hover + spinner（`settings.css:292,333-346`）；面板开关 `display:none↔flex` 硬切（`settings-ui.ts:404,832`）；消息无动画（`chat-ui.ts:67-76`） | 几乎无 | **P1** |
| **主题色系统** | 同类多为统一 design token | 聊天靛蓝 `#6366f1`（`index.html:153`）vs 设置品牌蓝 `#4176e6`（`settings.css:7,304`），两套并立 | 无统一主题 | **P1** |
| **字体与字号阶梯** | 同类有清晰 heading/body/caption 阶梯 | 设置面板字号散落 10/10.5/11/11.5/12/12.5/13/14px（`settings.css:39,45,67,103,120,162,199,234,289,331,332,350,439,499,541,578,597`）无 4px 阶梯；聊天面板另起一套 11/12/13/14px（`index.html:48,53,88,105,111,127,142,155`） | 阶梯混乱 | P2 |
| **背景图更换** | 🔶 Open-LLM-VTuber 上游支持（`backgrounds/`） | ❌ 新 renderer 未接线（`backgrounds/` 静态挂载 `server.py:151-154` 是遗留，`main.ts` 透明背景未用） | 缺失 | P2 |

---

### 2.3 「看起来廉价」的具体成因

> 每一条都落到具体文件与 CSS 现象；按「对观感伤害」从高到低排。

1. **两套主题色打架（最刺眼）**：聊天面板主色是靛蓝 `#6366f1`（发送键 `index.html:153`、用户气泡 `index.html:93`），设置面板主色是品牌蓝 `#4176e6`（`settings.css:7,304`）。同一应用里「发送」是靛蓝、「保存并应用」是品牌蓝，观感立刻分裂为两个产品。根因：聊天 UI 内联在 `index.html` 的 `<style>`（`index.html:6-178`），设置 UI 抽到 `settings.css`，各自手写了一套色板，没有共享 CSS 变量。

2. **无「面板级」动效**：设置面板打开是 `overlay.style.display='flex'`（`settings-ui.ts:832`）、关闭是 `display='none'`（`settings-ui.ts:404`），中间没有任何 opacity/scale/translate 过渡（`settings.css:15-24` 的 `.settings-overlay` 没有 transition）。消息上屏直接 `appendChild` + `scrollTop=scrollHeight`（`chat-ui.ts:67-76`），无渐入。tab 切换 `classList.toggle('active')`（`settings-ui.ts:396-397`）对应 `display:none/flex`（`settings.css:188-196`），硬切。→ 所有「变化」都是瞬断，没有现代 UI 的连续感。

3. **字号阶梯是「随机分布」而非「比例系统」**：设置面板出现 10 / 10.5 / 11 / 11.5 / 12 / 12.5 / 13 / 14 px 共 8 档（见 2.2 表），差 0.5px 的档位肉眼难分，又缺真正的标题层级（最大的 section 标题才 12.5px `settings.css:199`，比正文 13px `settings.css:39` 还小——**标题比正文小的反层级**）。聊天面板又另起 11/12/13/14 一套。→ 没有「一眼看出信息层级」的排版节奏。

4. **聊天面板缺「内容态」设计**：打开后是空消息列表，没有欢迎语 / 角色空状态 / 引导（`chat-ui.ts:44-57` 只有 header + 空 messages + 输入行）；没有「AI 正在思考」的 typing 指示（回复是整句 `onSubtitle` 一次性 `addMessage`，`main.ts:450-452`）；错误只是红色小字居中（`chat-ui.ts:81-83`、`index.html:108-113`）。→ 像「调试工具」而非「产品界面」。

5. **控件尺寸/圆角/间距不统一**：按钮圆角有 6/7/8/9/10px 多档（`settings.css:284,332,77,362,33`），输入框 `7px 10px`（`settings.css:229`）vs 聊天输入 `8px 12px`（`index.html:122`）；聊天面板 320px 固定宽 + 圆角 14px（`index.html:28,33`）vs 设置弹窗圆角 10px（`settings.css:33`）。→ 两个面板的「设计语言」不一致。

6. **聊天面板是「伪毛玻璃」**：`rgba(20,20,28,0.72)` + `backdrop-filter: blur(10px)`（`index.html:31-34`）算有毛玻璃；但设置弹窗 `#0f0f0f` 是不透明实底（`settings.css:31`），覆盖层才 `rgba(7,7,8,0.72)+blur(10px)`（`settings.css:22-23`）。两层「玻璃」一个真一个假，叠加后层次感混乱。

7. **无阴影层级体系**：聊天面板 `0 8px 32px rgba(0,0,0,0.35)`（`index.html:35`）vs 设置弹窗 `0 30px 80px rgba(0,0,0,0.65)`（`settings.css:34`），两套阴影无阶梯；卡片/按钮基本无阴影（`settings.css:283` `.btn` 无 box-shadow），「悬浮感」不足。

8. **emoji 当图标**：设置齿轮 `⚙`（`chat-ui.ts:49`）、麦克风 `🎤`（`chat-ui.ts:54`）、TTS 状态 `🔊`（`ws-bridge.ts:140`）——直接塞 emoji，没有统一图标集（SVG/icon font），在不同平台渲染不一致，廉价感直接来源之一。

---

## 3. Part B：能力缺口清单

> 标准：**「能交付给普通用户、动动手指即可配置」**。工作量口径 S < 1 天 / M 1–3 天 / L 3–5 天+（沿用 `core-fusion-research.md` 口径）。

### 3.1 P0 必须有（缺了不算完成品）

| # | 缺口 | 现状证据 | 目标形态 | 实现路径概要 | 工作量 | 依赖 |
|---|---|---|---|---|---|---|
| P0-1 | **桌面壳：透明置顶悬浮窗 + 点击穿透 + 托盘 + 开机自启 + 全局快捷键** | `index.html:12`、`main.ts:115` 仅注释「配合 Electron/Tauri」；`package.json:13-16` 无壳依赖；grep Tray/globalShortcut 无命中 | 桌面右下角常驻透明角色；托盘菜单（显示/隐藏/退出）；可置顶、可穿透；快捷键唤起 | 选 Electron（已有 Open-LLM-VTuber 桌宠模式先例）或 Tauri；壳内加载 `http://localhost:12393/`，开透明/无边框/置顶/穿透/托盘/快捷键五件套 | L | 后端 `run_server.py` 保持常驻；需要跨平台打包 |
| P0-2 | **语音/VAD/打断的用户可见反馈** | 后端有 Silero VAD + barge-in（`websocket_handler.py:379-397,502-509`、`vad/silero.py`），但前端 mic 走浏览器 Web Speech（`chat-ui.ts:90-131`），无「聆听/说话/打断」指示 | 对话区常驻状态：聆听中（麦克风波纹）/ 说话中（口型+波形）/ 用户打断（视觉打断动画） | WS 上行语音帧 → 后端 VAD；前端接 `audio-play-start`/interrupt 事件渲染状态灯与波纹；统一语音链路（弃浏览器 ASR） | M | P0-1 之后（语音采集需桌面壳权限）；后端已有事件 |
| P0-3 | **聊天历史持久化 + 回看** | 消息只存内存 DOM（`chat-ui.ts:67-76`）；`routes.py` 无 `/api/chat-history`；`chat_history/` 目录未随仓库存在 | 会话落盘（本地 SQLite/JSONL），重启可回看历史、可清空 | 后端加历史存储 + `GET/POST/DELETE /api/chat-history`；前端启动拉取历史渲染 + 「清空聊天」按钮 | M | 无 |
| P0-4 | **统一设计 token（主题色/字号/间距/圆角/阴影）** | 两套色板：`index.html:93,153` vs `settings.css:7,304`；字号散落 8 档（见 2.3-3） | 单一 `theme.css`（CSS 变量：`--accent/--bg-*/--text-*/--radius-*/--shadow-*/--font-*`），两面板共用 | 抽 CSS 变量到共享文件；`index.html` 内联样式迁出与 `settings.css` 统一；定 4px 字号阶梯（12/14/16/20） | M | 无（纯前端） |
| P0-5 | **首次启动引导（onboarding）** | 无实现（grep onboarding/引导/首次 无命中） | 首启三步引导：填 API Key → 选 LLM/TTS → 选模型/音色 → 完成；有「跳过」 | 前端加 onboarding 层，读 `/api/config` 判断是否缺 Key/模型，逐项引导并直达设置对应分区 | M | P0-4（需统一 token 才好看） |

### 3.2 P1 应该有（明显影响体验）

| # | 缺口 | 现状证据 | 目标形态 | 实现路径概要 | 工作量 | 依赖 |
|---|---|---|---|---|---|---|
| P1-1 | **动效与过渡体系** | 面板 `display:none↔flex` 硬切（`settings-ui.ts:404,832`、`settings.css:188-196`）；消息无动画（`chat-ui.ts:67-76`） | 面板淡入缩放、消息渐入、tab 内容交叉淡入、气泡上滑 | 统一 transition/animation 约定（150–200ms ease-out）；消息挂 CSS animation；`prefers-reduced-motion` 降级 | S | P0-4 |
| P1-2 | **聊天「内容态」：空态 + thinking 指示 + 流式** | 空消息列表无欢迎语（`chat-ui.ts:44-57`）；回复整句上屏无流式（`main.ts:450-452`） | 欢迎空态 + 「正在思考…」三点动画 + 逐 token 流式上屏 | 后端把流式 token 透传到 WS；前端打字机渲染 + 思考态 | M | 后端需透传流式事件 |
| P1-3 | **统一图标集（替换 emoji）** | `⚙`（`chat-ui.ts:49`）、`🎤`（`chat-ui.ts:54`）、`🔊`（`ws-bridge.ts:140`） | 内联 SVG 图标（设置/麦克风/发送/关闭/音量…），一致线宽 | 引入一套轻量 SVG 图标组件/雪碧图，替换所有 emoji | S | P0-4 |
| P1-4 | **模型热切换 UI 直连到主画面** | 设置面板有模型列表/扫描/选中（`settings-ui.ts:738-764,1252-1282`），但「已切换，重启后生效」（`settings-ui.ts:1277`）；主画面无模型切换入口 | 形象区右上角悬浮「换装/换模型」入口，切后即时热加载（WS `set-model-and-conf`） | 前端主画面加模型切换下拉，触发 `/api/models/select` 并让后端下发 `set-model-and-conf` 即时切换（后端已支持热切换 `main.ts:305-322`） | M | 后端 select 需即时应用（当前是重启生效） |
| P1-5 | **多角色/人设切换 UI** | 人设只能编辑单一 persona（`settings-ui.ts:275-292`），无多套人设保存/切换 | 多个人设卡（角色名+头像+音色+模型），一键切换 | 后端支持多 persona 槽；前端人设 Tab 改「卡片 + 切换」；对齐 NEKO 角色卡思路 | L | 后端多 persona 存储 |
| P1-6 | **错误与断线的用户可见提示（Toast）** | 断线仅聊天面板顶部小字「未连接/重连中」（`main.ts:453-459`、`chat-ui.ts:140-143`）；LLM/TTS 失败无主画面提示 | 全局 Toast + 角色头顶气泡提示「连不上/重试中/Key 无效」 | 加 Toast 组件；把 WS 断线、LLM 失败、TTS 降级等事件统一上抛渲染 | S | 无 |
| P1-7 | **背景图更换** | `backgrounds/` 静态挂载 + `fetch-backgrounds` 是上游遗留（`server.py:151-154`、`websocket_handler.py:94,560-566`），新 renderer 未接 | 设置里选背景图 / 纯透明 / 自定义壁纸 | 前端接线 `fetch-backgrounds`，在设置形象区加背景选择；或保留透明交给桌面壳 | S | P0-1（透明场景下背景图意义不同） |
| P1-8 | **无障碍基础** | 无实现：纯 `div`/`button` 无 aria（`chat-ui.ts:44-57`、`settings-ui.ts:202-330` 无 aria-label/role），键盘焦点无管理 | 语义化 role + aria-label + 焦点环 + 键盘可操作 | 给面板/输入/按钮补 aria；设置 Modal 补 `role=dialog` + Esc 关闭 + 焦点陷阱 | S | 无 |

### 3.3 P2 加分（高级感来源）

| # | 缺口 | 现状证据 | 目标形态 | 实现路径概要 | 工作量 | 依赖 |
|---|---|---|---|---|---|---|
| P2-1 | **情绪/动作调试面板（开发者/进阶）** | 无（表情经 `emotionConsumer` 直接消费，`ws-bridge.ts:245-268`，无可视化调试） | 侧边栏「调试」：手动触发表情/动作、查看 FPS/延迟/参数曲线 | 复用 `param-arbiter`/`emotion-consumer` 暴露当前参数；加只读调试面板（对应 Phase-0 T10） | M | 无 |
| P2-2 | **字幕/气泡样式升级（毛玻璃 + 头像 + 时间戳）** | 气泡是纯色 rgba（`index.html:92-113`），无头像/时间戳 | 角色头像 + 时间戳 + 毛玻璃气泡 + 引用/思考态区分 | 聊天面板样式重构，复用 P0-4 token | S | P0-4 |
| P2-3 | **i18n（多语言）** | 前端硬编码中文（`settings-ui.ts` 全中文、`index.html:2` `lang=zh-CN`）；后端配置有 `I18nMixin` en/zh（`config_manager/vad.py:7` 等） | 前端文案抽字典，中/英可切换 | 建 i18n 字典；`settings-ui.ts`/`chat-ui.ts` 文案走 key | M | 无 |
| P2-4 | **微交互（hover 抬升、按下反馈、焦点动效）** | 按钮仅 `background/color` 过渡（`settings.css:292`），无 translate/scale | 按钮 hover 轻微抬升、按压下沉、输入框 focus 光晕统一 | 在 token 层加统一交互类 | S | P0-4 |

### 3.4 明确不做（避免范围膨胀）

| 缺口 | 为什么不做 |
|---|---|
| **多 Avatar 形态（VRM/MMD/PNGTuber）** | 项目哲学「只打磨一个最小最优秀的核心」（`README.md:20-37`）；N.E.K.O 的 5 形态是广度取胜，我们保 Live2D 做精。已有调研明确「先保住 Live2D，VRM/图片桌宠作为可选 P2 差异化」（`benchmark-neko-vs-neurosama.md:121`）。 |
| **声音克隆 / 自定义音色训练（GPT-SoVITS）** | 依赖重模型与训练链路，与「动动手指即可配置」的轻目标冲突；N.E.K.O 的 15s 克隆依赖云端 Realtime 能力。列为远期 P2，不进最小闭环（`tts-research-report.md:296-297`）。 |
| **主动陪伴 / 长期记忆 / 屏幕感知闭环** | 是「AI 伴侣」灵魂（Neuro/N.E.K.O 核心），但属**内容/引擎**能力而非「UI 对标」范畴；且是独立大模块（`competitive-analysis-report.md:273-276` 列为核心短板，需单独立项），本 UI 调研不展开。UI 侧只为其预留状态位（如「主动搭话」气泡样式）。 |
| **跨设备记忆同步 / 账号系统** | 项目坚持「两端独立直连、无中心化后端、隐私优先」（`ARCHITECTURE.md:20`、`benchmark-neko-vs-neurosama.md:93`）；N.E.K.O 的同步是牺牲隐私的取舍，我们不做强制中心化。 |
| **直播平台对接 / 唱歌 / 游戏陪玩** | 偏离「私密陪伴桌宠」定位（`benchmark-neko-vs-neurosama.md:118-119`）；Neuro 的演出/娱乐形态不可复制，风险高、收益边缘。 |
| **i18n 全量多语言** | 已列入 P2-3；但不做「全部竞品文案级」翻译（含后端 20+ 引擎描述），首期只做前端中/英两语，避免摊薄核心。 |

### 3.5 16 项常见缺口逐一核查

> 逐项给出「已实现 / 部分 / 无实现」的**文件:行证据**。

| 缺口 | 结论 | 证据 |
|---|---|---|
| 开机自启 / 托盘 | ⬜ 无 | 无壳依赖（`package.json:13-16`）；grep `Tray/autostart` 无命中 |
| 窗口透明穿透与置顶（桌宠壳） | ⬜ 无（仅注释意图） | `index.html:12`、`main.ts:115` 注释「配合 Electron/Tauri」；无实际壳 |
| 快捷键唤起 | ⬜ 无 | grep `globalShortcut/registerHotkey` 无命中 |
| 模型热切换 UI | 🔶 部分（设置页有，主画面无、且需重启） | `settings-ui.ts:1252-1282`；`selectModel` 返回「已切换，重启后生效」`settings-ui.ts:1277` |
| 背景图更换 | ⬜ 无（上游遗留未接） | `server.py:151-154` + `websocket_handler.py:560-566` 是遗留；新 renderer 未用 |
| 语音输入按钮 | ✅ 有（浏览器 Web Speech） | `chat-ui.ts:90-131` |
| VAD 打断的可视反馈 | ⬜ 无 | 后端有 VAD/barge-in（`websocket_handler.py:379-397,502-509`），前端无对应 UI |
| 字幕/气泡样式 | 🔶 有但基础 | `index.html:92-113` 纯色气泡；无头像/时间戳/毛玻璃层级 |
| 情绪与动作调试面板 | ⬜ 无 | 表情直接消费无可视化（`ws-bridge.ts:245-268`）；对应 Phase-0 T10 未做 |
| 多角色/人设切换 | ⬜ 无（单 persona 编辑） | `settings-ui.ts:275-292` 单一 persona 字段 |
| 聊天历史持久化与回看 | ⬜ 无 | 消息内存 DOM（`chat-ui.ts:67-76`）；`routes.py` 无历史端点 |
| 首次启动引导（onboarding） | ⬜ 无 | grep onboarding/引导/首次 无命中 |
| 错误与断线的用户可见提示 | 🔶 部分（仅连接状态） | 断线小字提示（`main.ts:453-459`）；LLM/TTS 失败无主画面 Toast |
| i18n | ⬜ 前端无（后端配置层有 en/zh） | 前端硬编码中文（`settings-ui.ts`、`index.html:2`）；后端 `config_manager/vad.py:7` 等 `I18nMixin` |
| 无障碍 | ⬜ 无 | `chat-ui.ts:44-57`、`settings-ui.ts:202-330` 无 aria/role/焦点管理 |
| 打包分发方式 | 🔶 半缺失 | 有 `start.sh` 一键启动（`CHANGELOG.md:7-9`、README `快速开始`）；但**用户拿到的是源码 + 浏览器访问 localhost，无安装包/无桌面壳**；Android 有 `gradlew assembleDebug` 出 APK（`README.md:130-134`） |

---

## 4. 参考链接索引

**本项目（现状依据）**
- `Live2D-Ai-pc/open-llm-vtuber/renderer/src/main.ts` / `chat-ui.ts` / `settings-ui.ts` / `settings-logic.ts` / `ws-bridge.ts` / `settings.css` / `index.html` / `package.json`
- `Live2D-Ai-pc/open-llm-vtuber/server.py` / `routes.py` / `websocket_handler.py` / `vad/silero.py`
- `docs/research/benchmark-neko-vs-neurosama.md` / `NEURO_LIVE2D_RESEARCH.md` / `core-fusion-research.md` / `tts-research-report.md` / `ux-benchmark.md` / `competitive-analysis-report.md`
- `docs/architecture/ARCHITECTURE.md` / `core-contracts.md` / `Phase-0-architecture.md` / `README.md` / `PLAN.md` / `CHANGELOG.md`

**现网对标**
- N.E.K.O 仓库：https://github.com/Project-N-E-K-O/N.E.K.O
- N.E.K.O 官网配置/快速上手：https://project-neko.online/config/ · https://project-neko.online/guide/quick-start
- N.E.K.O Steam v0.8.x 更新：https://steamdb.info/patchnotes/23581477/
- N.E.K.O 中文介绍：https://www.acgsq.com/4969.html · https://global.v2ex.co/t/1220005
- Open-LLM-VTuber 窗口 & 桌宠模式（Electron）：https://docs.llmvtuber.com/en/docs/user-guide/frontend/electron/#desktop-pet-mode
- Open-LLM-VTuber 仓库：https://github.com/Open-LLM-VTuber/Open-LLM-VTuber
- Live2DPet：https://github.com/x380kkm/Live2DPet
- Soul of Waifu：https://github.com/jofizcd/Soul-of-Waifu
- Meuxe（Tauri 2）：https://github.com/meet447/Meuxe
- Nexus（Electron）：https://github.com/FanyinLiu/Nexus
- my-neuro：https://github.com/morettt/my-neuro
- Amadeus System：https://docs.amadeus-web.top/en/ · https://tieba.baidu.com/p/8345668436 · https://forum.trae.cn/t/topic/23118
- 同类导航（awesome-agentic-ai-waifus）：https://github.com/yuri-os/awesome-agentic-ai-waifus

---

> **完整性声明**：本项目现状全部基于 `renderer/` 与后端源文件实测（`文件:行`）；N.E.K.O 的「多服务 / 角色卡中心 / API Key 页 / 声音克隆 / Free provider / Steam 发行」来自官方 README 结构树与配置文档（✅），其「桌面壳 / 托盘 / 快捷键」具体截图未逐字核到，本报告以「Open-LLM-VTuber / Live2DPet / Meuxe / Nexus 等同类通用做法」作为桌面壳基准（✅），并在 N.E.K.O 对应单元格标注 🔶。Amadeus 仅作产品存在性引用，UI 细节未逐项核对。

---

## 附：缺口落地状态（2026-08-20 由实现侧回填）

> 本报告主体是**只读调研**产出；下表是同日实现侧的真实进度，供后续轮次对账。
> 详细改动见 `CHANGELOG.md` 前两节（第二轮 / 第三轮）。

| 编号 | 缺口 | 状态 | 落地要点 / 未做原因 |
| --- | --- | --- | --- |
| P0-1 | 桌面壳（透明置顶 / 穿透 / 托盘 / 自启 / 快捷键） | ⏸ 未做 | L 量级，需 Electron/Tauri 独立立项 |
| P0-2 | 语音 / VAD / 打断的可见反馈 | ✅ 已做（真机麦克风待验） | 第四轮：新增 `voice-input.ts` 把音频送回后端 ASR（16kHz 重采样 + 2048 分片 + 尾帧 flush），前端补齐 `control` 消息处理（此前**完全忽略**→ 打断永远不可见）与真正的 `stopAudio()`；聆听/识别/已打断三态可见 + 打断 Toast；不支持 getUserMedia 时回落 Web Speech。**未验**：真实麦克风采集与识别质量；VAD 发送端需在 conf.yaml 启用 `silero_vad` |
| P0-3 | 聊天历史持久化 + 回看 | ✅ 已做 | 后端连接即绑定历史（此前 `history_uid` 恒空 → **一条都没落过盘**）+ 续接最近一段 + LLM 记忆接上；前端历史抽屉（切换/新建/删除）；真机 E2E 12/12 |
| P0-4 | 统一 design token | ✅ 已做 | 新增 `theme.css`；`settings.css` 零裸色值零裸字号；删除聊天面板私有靛蓝；修正标题反层级 |
| P0-5 | 首次启动引导 | ✅ 已做 | `onboarding.ts`：缺什么说什么 + 一键跳分区；只在阻塞项（Provider/Key）缺失时弹；支持不再提示 |
| P1-1 | 动效与过渡体系 | ✅ 已做 | 覆盖层淡入 / 弹窗上浮 / tab 交叉淡入 / 消息渐入 / 胶囊脉冲 / 波纹；遵守 `prefers-reduced-motion` |
| P1-2 | 空态 + thinking + 流式 | ✅ 已做 | 第四轮：后端在 `emo_interceptor` 旁听一路 `text_delta`（**主链/分段/TTS 一字未改**）→ WS `text-delta`；前端流式气泡 + 句子级字幕作权威文本收尾，不重复上屏 |
| P1-3 | 统一图标集 | ✅ 已做 | `icons.ts` 15 个统一线宽 SVG，替换全部 emoji |
| P1-4 | 模型热切换即时生效 | ✅ 已做 | `broadcast_model_change()` 主动推 `set-model-and-conf`；`select` 不再回 `requires_restart`；真机 E2E 5/5 |
| P1-5 | 多角色 / 人设切换 | ⏸ 未做 | L 量级，需后端多 persona 存储 |
| P1-6 | 错误与断线可见（Toast） | ✅ 已做 | `toast.ts`；并修掉「后端错误被 transformers 吞成 warning、前端全无提示」的真实缺陷 |
| P1-7 | 背景图更换 | ✅ 已做 | `backgrounds.py` + 设置页缩略图网格；铺在画布之下，即时生效 |
| P1-8 | 无障碍基础 | ✅ 已做 | 模态 `role=dialog`+`aria-modal`+Esc；聊天 `role=log`+`aria-live`；aria-label；统一焦点环 |
| P2-1 | 情绪 / 动作调试面板 | ⏸ 未做 | M 量级，可与真机渲染轮次一起做 |
| P2-2 | 气泡升级（头像 / 时间戳 / 毛玻璃） | ✅ 已做 | 头像 + 时间戳 + 毛玻璃气泡 + 外部注入角标 |
| P2-3 | i18n | ⏸ 未做 | M 量级 |
| P2-4 | 微交互（hover 抬升等） | ✅ 已做 | `theme.css` 的 `.ui-lift` 统一类 + 焦点环 |

**结论口径修正（截至第四轮）**：报告「结论先行」的 5 个最关键差距中，第 2（两套视觉系统）、
第 3（几乎无动效）、第 4（历史不持久化）、第 5（语音/VAD 反馈，真机麦克风待验）**均已解决**；
**只剩第 1（无桌面壳）** —— 这也是当前唯一的身份级缺口（L 量级，需 Electron/Tauri 立项）。
