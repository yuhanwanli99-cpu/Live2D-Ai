# AI 控制 Live2D 皮套 —— 控制通道与生态补充调研（2026-08）

> 调研日期：2026-08-23
> 定位：**补充轮**。本报告不重复既有四份调研的结论，只填空白：
>
> | 既有报告 | 已覆盖（本报告不再展开） |
> |---|---|
> | industry-emotion-tts-survey.md | LLM→表情映射 A/B 两派：文本内嵌标签 vs 结构化 JSON（源码实锤） |
> | live2d-halfbody-motion-research.md | soullink-emotion-sdk 深读：VAD 连续情绪空间 + FACS/AU + MotionMixer 分层混合 |
> | NEURO_LIVE2D_RESEARCH.md | Neuro-sama 复刻栈、口型两派（音量 vs 音素）、眨眼/表情过渡参数 |
> | NeuroSama-architecture-research.md / benchmark-neko-vs-neurosama.md | Neuro 架构反向工程、产品层对标 |
>
> 纪律：每条结论附来源 URL；GitHub 元数据（stars/license/pushed）为 2026-08-23 经 api.github.com 实查；未验证项明确标注「待验证」。不编造数据。

---

## 一、TL;DR

1. **控制通道可归纳为五类（A–E，见 §二）**。本项目主链路已落在 A/B/C/D 四类上；本轮新发现的是 **E 类「Agent/MCP 工具化操控」**——皮套成为编码 Agent 的"外设"，与本项目插件化定位天然契合。
2. **「参数级实时控制 + 分层 mixer + 连续情绪状态」已是社区共识架构**：中文社区独立提案（XnneHangLab issue #379，2026-03）与 nanlingyin/soullink-emotion-sdk 的 MotionMixer 设计几乎逐条对应——两个独立来源收敛于同一形态，是强信号。
3. **官方音频直驱能力已就绪**：Cubism Motion Sync（CRI LipSync 引擎，音频→viseme→口型参数自动生成）。官方组件仓库只有 **Web / Native / Unity** 三份，**没有 Android(Java) 版** → PC Web 前端可评估接入，Android 维持音量方案。
4. **三个活跃新开源项目**值得纳入观察名单：myths-labs/prometheus-avatar（MIT）、shinshin86/aituber-onair（MIT）；tegnike/aituber-kit 自 v2.0 起改自定义双许可，**代码不可复制**，仅可对标理念。
5. **上游风险提示**：Open-LLM-VTuber（PC 端底座）13.4k stars、代码 MIT，但最近一次 push 为 **2026-05-15**（截至本报告约 3 个月无动态），需留意其维护节奏。

---

## 二、控制通道全景（分类学）

| 类 | 通道 | 代表实现 | 控制粒度 | 本项目现状 |
|---|---|---|---|---|
| A | 文本流内嵌情绪标签 | Open-LLM-VTuber [key]、my-neuro <中文> | 句级离散 | PC 主链路（详见 industry-emotion-tts-survey） |
| B | 结构化 JSON / tool call | entropy622 expressionMix、OLV ToolCallAgent | 表情+参数多维 | 备选路线（同上） |
| C | 连续情绪空间 + 分层 mixer | soullink-emotion-sdk engine（VAD/FACS/AU） | 参数级连续 | PLAN-V3 选定路线 |
| D | 音频直接驱动 | 音量→ParamMouthOpenY、pymouth DTW 音素、**官方 Motion Sync（新）** | 帧级 | Android=音量；PC=pymouth；Motion Sync 待验证 |
| E | **Agent/MCP 工具化操控** | prometheus-avatar MCP server / agent skill | 工具调用级 | 无（差异化机会） |

A/B 两派的成本与可靠性对比见 industry-emotion-tts-survey.md 对比表，此处不重复。

---

## 三、2026-08 新生态盘点

### 3.1 myths-labs/prometheus-avatar —— 把皮套做成 SDK/MCP 外设（MIT 🟢）

- 元数据：MIT；15 stars（年轻项目）；pushed 2026-08-18（活跃）；npm @prometheusavatar/core v0.8.0、@prometheusavatar/mcp-server v0.3.5。
  <https://github.com/myths-labs/prometheus-avatar>
- 形态：浏览器端 SDK（基于 pixi-live2d-display，宣称支持 Cubism 2 & 4），5 行代码起一个会说话的皮套：createAvatar() → avatar.speak(text)，内部自动做 **文本情绪检测 → 表情+动作 → 口型同步**。
- 实时语音：Gemini Live API WebSocket 流式（宣称 ~200ms 延迟，内置 VAD+打断）。
- **E 类通道的两个入口**：
  1. **MCP server**：npx @prometheusavatar/mcp-server，暴露 10 个工具（create_avatar / set_avatar_state / equip_asset / speak 等），任何 MCP 客户端（Claude Desktop、Cursor 等）即获得一张"脸"；
  2. **Agent Skill**：prometheus-companion skill——编码 Agent 通过纯 HTTP 驱动皮套，把自身任务状态（thinking/acting/done）推给角色表演。
- 还带 Marketplace（皮/声/人设交易）。对本项目的启示：**渲染端可以做成"被 Agent 编排的服务"**，与 docs/architecture/plugin-sdk.md 的插件契约方向一致。

### 3.2 shinshin86/aituber-onair —— 模块化 TS 工具箱（MIT 🟢）

- 元数据：MIT；175 stars；pushed 2026-08-21（非常活跃）。
  <https://github.com/shinshin86/aituber-onair>
- 形态：@aituber-onair/core 模块包 + 托管 Web 应用 + npm create aituber-onair 脚手架；自述目标即 "Neuro-sama-style AI VTuber toolkit"。
- 皮套格式覆盖最广：PNGTuber / PuruPuru(发丝物理) / VRM / **Live2D（本地 .model3.json 文件夹加载器）** / Pet / PSD / Inochi2D(实验)。Live2D 示例刻意**不捆绑模型资产**（规避 Live2D 示例数据条款）。
- 口型统一走**实际音频输出音量**驱动（非文本估算），全皮套格式通用；另有 YouTube/Twitch 评论响应与记忆系统。

### 3.3 tegnike/aituber-kit —— 功能标杆但许可红线 ⚠️

- 元数据：1053 stars（生态最大之一）；license = **NOASSERTION（自定义双许可）**；pushed 2026-08-17。
  <https://github.com/tegnike/aituber-kit>
- LICENSE 原文实查（api.github.com）：v2.0.0 起为 Custom License——**非商业用途免费；商业用途需另行购买 Commercial License**。→ **代码不可复制进本项目**，仅作功能对标。
- 功能清单有对标价值：多模态摄像头/图片输入、RAG 长期记忆、YouTube 评论（API 或 わんコメ）、无评论时的自发续聊模式、Realtime API 低延迟对话+函数调用、**游戏实况模式（定时截屏→AI 解说）**、数字标牌模式（人脸检测迎宾/送客）、Idle 模式（定时语料/分时段问候/AI 自动生成三源）。

### 3.4 Open-LLM-VTuber 现状更新（PC 底座）

- 元数据：13,427 stars；pushed **2026-05-15**（约 3 个月无 push，放缓观察⚠️）。
  <https://github.com/Open-LLM-VTuber/Open-LLM-VTuber>
- License 实查：仓库根目录有两份文件——
  - LICENSE：**代码 MIT**（Copyright (c) 2025 Yi-Ting Chiu）；
  - LICENSE-Live2D.md：仓库内捆绑的 Live2D 官方示例模型受 Live2D 单独条款约束（原版角色/联动角色/外部授权角色三类，条款不同）。
- 对本项目：引用/复用其**代码**安全；但其**内置模型资产不可随我们分发**。此前 license-report 未记录这一区分，建议补一条。

---

## 四、参数级实时控制：社区共识与 MVP 形态

来源：XnneHangLab issue #379「支持 Neuro-like 的参数级表情/身体控制（基于官方 Cubism/LApp）」（2026-03-25 开，2026-03-26 关闭）
<https://github.com/XnneHangLab/XnneHangLab/issues/379>

### 4.1 核心论点

当前多数 AI VTuber 前端停留在「自动眨眼 + 视线跟随 + 音量张嘴 + 随机 motion」，能"会动"但难"像活着"；差距不在更强的 LLM，而在四件事：**参数级控制、多层混合(mixer)、持续状态(state)、短时事件(event gesture)**。前提是驱动层已迁到官方 Cubism Web SDK/LApp（该社区由 Open-LLM-VTuber-Web 完成），底层 update loop 天然支持逐参数写入——**"缺的不是渲染能力，而是把参数能力暴露成业务能力"**（issue 原文）。

### 4.2 提案要点

- **持续情绪状态**（非 preset 切换）：valence / arousal / focus / confusion / amusement / confidence 平滑映射到眉毛、眼睛开合、视线漂移、身体前倾后仰、头部微动 → 表达"思考中/吐槽中/得意/困惑/被夸的小反应"。
- **事件手势**：surprise/laugh/smug/embarrassed/scared/annoyed 等短时覆盖部分参数再平滑回落。
- **MVP 三步**：①暴露参数 API（set/add/blend by id，WebSocket 下发参数数组）→ ②接关键默认参数（ParamAngleX/Y/Z、ParamBodyAngleX、ParamEyeBallX/Y、ParamMouthOpenY）→ ③简单三层 mixer（idle 层 / speech 层 / backend_pose 层）。
- **长期架构**：Dialogue Brain → Performance Brain →（Speech Driver / Idle Driver / Gesture Sequencer）→ Mixer → Model Driver 逐帧写参。

### 4.3 与 soullink-emotion-sdk 的收敛对照（两个独立来源）

| #379 提案概念 | soullink engine 对应物（见 halfbody 调研 §3） |
|---|---|
| 持续状态 valence/arousal… | VAD 连续情绪空间 |
| 状态→面部映射 | FACS/AU 表情语义 + ModelProfile 跨皮套映射 |
| 三层 mixer | MotionMixer 分层混合 |
| Gesture Sequencer | SpeechPerformancePlanner 说话演出规划 |

**结论**：PLAN-V3 选定的 soullink 路线与社区最新提案同构，方向无需调整；#379 的「WebSocket 参数数组接口 + idle/speech/backend_pose 三层命名」可作为我们 mixer 接口设计的参考命名法。

---

## 五、官方音频直驱：Cubism Motion Sync

### 5.1 能力与原理（官方手册实读）

来源：<https://docs.live2d.com/en/cubism-editor-manual/motion-sync/>

- 输入 WAV（建议 16bit/44100Hz/单声道），经音频分析库 **Lip-sync (CRI LipSync)** 转成 viseme 时间序列（Silence + A/I/U/E/O 五元音），对每个 viseme 预定义的形状做加权混合，**自动生成口型动作**——即官方提供的"零 keyframe 音频→参数"路线。
- 可调项：sample rate 15–100（越大越细腻但抖动大，快语速建议约 60）、blending ratio 0–1（主元音主导↔均匀混合）、smoothing 1–100、每 viseme 的 scale/threshold。
- 设置随模型导出 .motionsync3.json；官方提供示例模型 "Kay"。默认预设两种：「Basic」（变形/开合两轴）与「仅元音 blend shape」。

### 5.2 运行时支持矩阵（2026-08-23 GitHub org 实查）

| 运行时 | 官方组件仓库 | 最近 push | 双端可用性 |
|---|---|---|---|
| Web | Live2D/CubismWebMotionSyncComponents | 2025-03-27 | ✅ PC Web 前端（Open-LLM-VTuber-Web 一系）可评估 |
| Native (Win/Linux/macOS) | Live2D/CubismNativeMotionSyncComponents | 2025-03-27 | ➖ 本项目暂无 Native 渲染端 |
| Unity | Live2D/CubismUnityMotionSyncComponents | 2024-11-28 | ➖ 不适用 |
| **Android (Java/Core)** | **无** | — | ❌ 维持音量→ParamMouthOpenY 方案 |

仓库链接：<https://github.com/Live2D/CubismWebMotionSyncComponents> ・ <https://github.com/Live2D/CubismNativeMotionSyncComponents> ・ <https://github.com/Live2D/CubismUnityMotionSyncComponents>

### 5.3 含义

- PC 端若未来从 pymouth（DTW 音素）切换或叠加 Motion Sync：编辑器侧需为皮套制作 viseme 映射并随模型分发 .motionsync3.json，运行时引入 Web 组件即可；**待验证**项为 Cubism Web SDK 运行时与 pixi 封装的实际集成成本。
- Android 端维持现状（Phase-0 既定降级路径成立），Motion Sync 不作为 Android 目标。

---

## 六、对本项目的落点建议

1. **不动摇的部分**：A/B 标签链路（industry survey 结论①⑥）+ soullink C 路线（PLAN-V3）保持；#379 收敛信号进一步确认 C 路线正确性。
2. **接口命名借鉴**：mixer 分层采用 idle/speech/backend_pose 三层词汇；对外控制接口按「参数数组 over WebSocket」设计，便于未来接 E 类通道。
3. **E 类通道作为差异化实验**：把 Live2D-Ai 渲染端封装成 MCP server 或最小 HTTP 控制面（对齐 prometheus-avatar 的 10 工具面），契合 plugin-sdk 契约与"低门槛"定位；优先级排在 V3 之后。
4. **口型升级候选**：PC 端评估 Motion Sync（先做 pixi/Web 组件集成 spike）；Android 明确排除。
5. **许可红线更新**：aituber-kit 代码禁抄（自定义双许可）；Open-LLM-VTuber 代码 MIT 但其内置 Live2D 官方示例模型受单独条款约束——补入 license-report。
6. **上游风险**：关注 Open-LLM-VTuber 是否恢复活跃；若长期停滞，评估 fork 维护或迁移成本（其 MIT 许可使 fork 无障碍）。

---

## 七、引用清单

- prometheus-avatar README（api.github.com raw，2026-08-23）：<https://github.com/myths-labs/prometheus-avatar>
- aituber-onair README：<https://github.com/shinshin86/aituber-onair>
- aituber-kit README 与 LICENSE 原文：<https://github.com/tegnike/aituber-kit>
- Open-LLM-VTuber LICENSE / LICENSE-Live2D.md 原文：<https://github.com/Open-LLM-VTuber/Open-LLM-VTuber>
- XnneHangLab issue #379 全文（api.github.com）：<https://github.com/XnneHangLab/XnneHangLab/issues/379>
- Cubism Editor Manual – Motion-sync：<https://docs.live2d.com/en/cubism-editor-manual/motion-sync/>
- Live2D 官方 MotionSync 组件仓库 ×3（org 实查，见 §5.2）
- 各仓库 stars/license/pushed 元数据：api.github.com，检索日 2026-08-23
