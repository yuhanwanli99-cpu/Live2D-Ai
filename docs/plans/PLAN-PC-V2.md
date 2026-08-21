# Live2D-Ai PC v2 计划（核心链 3-LLM + 插件仓库 + GitHub v1 基线）

> 范围：只谈 PC。Android 不进入本计划。
> 版本：v2（在 PLAN-PC-V1.md 基础上重构，重点解决“人设/Live2D/TTS 链路不自然”与“插件化/开源发布”）。
> 硬原则：精简代码、完全信任本机、不做防呆设计；但**不做静默崩溃**。

---

## 0. 总原则

1. **精简优先**：核心仓库只保留“能跑、能看懂、能改”的最小闭环，不堆平台适配、不堆防御。
2. **信任本机**：不做沙箱式防呆、不做多层权限确认、不做“用户可能乱点”的过度保护。
3. **不做静默崩溃**：这是与“不做防呆”并列的硬约束。任何失败都必须**可见**：前端状态灯、Toast、WS error 帧、日志事件至少出现一个；允许降级，但降级必须显示原因与当前降级档位。
4. **核心不依赖插件，插件能挂任意链路**：插件权限默认给大，但所有插件异常都必须可观测、可隔离、可停用。
5. **v1 = 能上传 GitHub 的基线**，不是功能齐全的商业产品。

---

## 1. 仓库与插件分层

### 1.1 仓库形态

- 主仓库：Live2D-Ai（核心闭环 + UI + 必要链路）。
- 子仓库 / 插件仓库：live2d-ai-plugins（git submodule 或独立仓库，由主仓库以目录形式挂载）。
- 云端主仓库下开子仓库，避免把 B 站、抖音、CLI、roleplay 等 P2 内容塞进核心。

### 1.2 链路优先级

| 层级 | 内容 | 维护策略 |
| --- | --- | --- |
| 核心链路 | TTS + AI + Live2D | 永远优先，必须稳定、低延迟、可观测 |
| 必要链路 | UI、外挂知识库、记忆系统、ASR、插件系统、背景图、Live2D 悬浮窗/桌宠、设定书、暴露对话输入端口、联网查询、模型/配置导入导出、健康状态与日志 | 随 v1 交付 |
| P2 / 暂不维护 | 多平台弹幕、CLI 操控、DeepSeek 角色扮演模式、唱歌、VRM/PNGTuber、跨设备云同步、多角色、插件市场 | 只做插件接口，不主动维护 |

### 1.3 必要链路补充（用户未列全，我补）

- 健康状态端点：/api/health 细化到 LLM/TTS/Live2D/插件/直播桥，前端有状态灯。
- 事件与错误通道：/api/events 或 WS 统一错误帧，所有模块失败都往这里报。
- 模型资产导入：Live2D zip 上传、模型列表、删除、默认模型。
- 配置包：conf.yaml + persona.yaml 导入/导出/重置。
- 语音资产：参考音频/克隆音色上传、试听、应用。
- 外部输入端口：HTTP /api/external/chat 与 WebSocket 文本输入，供第三方脚本和直播桥复用。
- 会话与历史：聊天历史持久化、清空、新建。
- 桌面壳：窗口置顶/透明/拖拽/缩放，是桌宠化和直播合成的基础。
- 状态持久化：插件启停、知识库、记忆、背景选择。

### 1.4 插件能力边界

- 插件可在任意链路注入钩子，可阻塞任意链路。
- 默认权限给大，但主循环只做一件事：**把插件异常和阻塞原因显示出来**，不做权限审批。
- 钩子位置至少覆盖：
  - 输入前/输入后
  - LLM 前/流式增量/句子完成后
  - emotion/motion 解析前后
  - TTS 前后
  - Live2D 帧更新前后
  - 背景切换
  - 记忆/知识检索
  - 工具调用
  - WebSocket 发送前
  - 直播事件处理前
- 插件接口以 Python 为主：Plugin、PluginHost、ChatHook、ToolDefinition、LiveHook、ConfigSchema。
- 插件仓库按目录分包：plugin-bilibili、plugin-douyin、plugin-cli、plugin-roleplay 等。

### 1.5 B 站弹幕转发改为插件

- 现状问题：scripts/run_bilibili_live.py 引用了不存在的 src/open_llm_vtuber/live/bilibili_live.py 与 BiliBiliLivePlatform，属于典型静默崩坏。
- 处理：v2 第一阶段把它迁出核心，做成 plugin-bilibili。
- 插件只负责：连接房间、收弹幕/礼物、去重、限流、按规则转成通用事件。
- 通用事件继续走现有 /api/live/event，核心不感知平台差异。
- Windows 启动弹幕转发.bat 只作为插件入口的启动脚本，不再承载业务逻辑。

---

## 2. 核心链路 v2：3 个 LLM 并行产出

### 2.1 为什么

当前问题不是“缺一个字段”，而是**一个 LLM 同时负责说话、动作、情绪、语气，结果每样都平庸**：

- 说话像普通 AI，人设不稳定。
- 动作数据要么没有，要么被 JSON 碎片污染。
- 心情/TTS 语气靠规则库，无法随上下文变化。

参考 Neuro 与 N.E.K.O. 的分工：Neuro 是复合系统，脑/视觉/游戏/TTS 分离；N.E.K.O. 是记忆、Agent、TTS、脑分离。我们也应把“说什么、怎么动、什么语气”拆开。

### 2.2 三个 LLM 分工

| LLM | 职责 | 输入 | 输出 | 是否阻塞主链 |
| --- | --- | --- | --- | --- |
| LLM-A 对话脑 | 生成干净、有人设、可直接朗读的回复文本 | persona + 历史 + 记忆 + 用户输入 | 流式文本 + 可选 emo token | 是，主链 |
| LLM-B 身体/表情导演 | 生成半身动作、表情混合、参数覆盖、motion 标签 | 用户输入 + A 的句子 + 模型能力目录 | BodyFrame 或 MotionFrame JSON | 否，尽力而为 |
| LLM-C 心情/TTS 导演 | 生成 mood tier、TTS instruction、语速/音高/音量风格 | 用户输入 + 历史 + A 回复摘要 | MoodFrame / TtsFrame JSON | 否，尽力而为 |

### 2.3 数据契约

TurnBundle：

- text：A 输出，去标签，直接 TTS 与显示。
- emotionEvents：流式 emo token，先于音频上屏。
- bodyFrames：B 输出，按 sentenceId 对齐；字段 expressionMix、parameterOverrides、motionLabel、bodyPose、confidence。
- mood：C 输出，mood tier 与持久 mood 更新。
- ttsStyle：C 输出，instruction、rate、pitch、volume 风格。
- status：每个子 LLM 的状态，例如 ok、timeout、fallback、disabled。

### 2.4 并行调度

- A 是热路径，启动即流式返回。
- B、C 与 A 同时启动，使用轻量模型（GLM Flash / DeepSeek Flash 等）或可配置为同模型。
- B、C 设置超时，超时不阻塞 A；结果迟到时：
  - 若对应句子还没开始播放，则应用；
  - 若已播放，则丢弃并计入 fallback。
- B、C 失败时回落到确定性规则：
  - B 回落 BodyMotionPlayer 参数动作。
  - C 回落 tts_instruction_library 与 mood_engine 规则。
- 任何回落都要发 visible fallback 事件，不在后台悄悄消失。

### 2.5 对 Live2D 的落点

- LLM-B 输出 motionLabel 时，模型有 motion3 就播 motion3；没有就转成 BodyMotionPlayer 参数动作。
- LLM-B 输出 expressionMix/parameterOverrides 时，写 EmotionConsumer 或直接写 ParamArbiter。
- 模型能力目录由后端解析 model3.json 后下发，B 只能引用存在的 motion/expression。
- 新增 Motion 优先级到 ParamArbiter，确保口型与眨眼不被动作压坏。

### 2.6 对 TTS 的落点

- LLM-C 输出按 TTS provider 能力适配：
  - CosyVoice 支持 instruction/rate/pitch/volume。
  - Edge/Azure/OpenAI 支持 rate/pitch/volume 或对应字段。
  - GPT-SoVITS/SparkTTS/x_tts 只使用参考音色，不套 instruction。
- 当前 TTS 能力由 capability registry 暴露，不支持的字段不传，并显示“该厂商不支持”。
- TTS 失败仍可降级为下一引擎，但前端必须显示降级原因与当前引擎。

---

## 3. 与 N.E.K.O. / Neuro 的对比

| 维度 | N.E.K.O. | Neuro | Live2D-Ai v2 定位 |
| --- | --- | --- | --- |
| 形态 | Docker 多服务 + 插件商城 + 多 Avatar | 直播 IP + 闭源渲染 + 社区 SDK | 单进程 PC 核心 + 插件子仓库 + 单一 Live2D |
| 记忆 | 5 层记忆 | 长期记忆回捞、人设成长 | 短期历史 + 长期事实/偏好 + 知识库检索，先做 2-3 层 |
| 动作/表情 | 多 Avatar 表情系统 | 预置动画 toggle + 表情 fade | 3-LLM 导演 + motion3/参数兜底 |
| TTS | 17 引擎 + 音色克隆 | 定制音色，多次升级不换声 | 多 provider capability + 声音克隆 + 心情导演 |
| 插件 | 插件商城/创意工坊 | 游戏 SDK | Python 运行时插件，任意链路钩子，默认大权限 |
| 部署 | 重，依赖 Docker/云 | 闭源直播栈 | 轻，WSL2/Linux 单仓 + start.sh |
| 分发 | Steam/角色卡/插件商城 | 直播平台 | GitHub v1 基线 + 角色卡/配置/模型导入 |

我们要抄 N.E.K.O. 的“模块解耦 + 插件生态”，但不抄 Docker 多进程和商城；抄 Neuro 的“复合模型分工 + 长期一致性 + 表情自然化”，但不做争议人设和纯直播演出。

---

## 4. P2 / 暂不维护清单

- 抖音、快手、虎牙、斗鱼等多平台弹幕。
- CLI 操控。
- DeepSeek 角色扮演模式。
- AI 唱歌。
- VRM / PNGTuber / MMD 多形态。
- 跨设备云同步。
- 多角色/群聊。
- 插件市场与在线安装。
- 全自动电脑操作 Agent。

这些只预留插件接口，不进主仓库。

---

## 5. 开源许可证

目标：传染性 + 非授权不可商用。

两个方案：

- 方案 A（推荐）：AGPL-3.0-only + 商业授权双许可。
  - 代码以 AGPL-3.0-only 发布，保留网络传染性。
  - 另加 COMMERCIAL-LICENSE.md，商业使用必须单独取得授权。
  - 优点：GitHub 上仍是标准开源许可证，社区接受度高；法律上能实现“未授权商用即违约”。
  - 注意：纯 AGPL 本身不禁止商用，所以必须写明双许可/附加商业授权条款。
- 方案 B（严格非商用）：PolyForm Noncommercial 1.0.0 或 BSL 1.1。
  - 优点：直接写明非商用。
  - 缺点：不是 OSI 开源许可证，社区认知弱，传染性不如 AGPL 强。

建议采用方案 A：主仓库与插件 SDK 用 AGPL-3.0-only；插件仓库默认同许可证，特殊闭源插件需商业授权。LICENSE、NOTICE、CREDITS、每个文件的 SPDX 头在 v1 上传前补齐。

---

## 6. 路线图

### P0：修静默崩溃 + 基线绿门

- [ ] 修复 scripts/run_bilibili_live.py 引用不存在的 BiliBiliLivePlatform，暂时移出核心或补上可运行插件。
- [ ] 修 pytest：free_glm_defaults、pymouth、persona 一致性。
- [ ] 前端 vitest 跑绿。
- [ ] 建立统一错误事件通道，所有异常至少有状态灯或 toast。
- [ ] 增加“禁止静默吞异常”检查：except Exception: pass 必须清零；logger.warning 后必须伴随可见信号。

### P1：3-LLM 核心链路

- [ ] 新增 LLM-B BodyDirector 与 LLM-C MoodDirector，和 LLM-A 并行。
- [ ] 定义 TurnBundle 与 BodyFrame/MoodFrame 契约。
- [ ] A 保持流式；B/C 超时与 fallback 可见。
- [ ] B 输出接入 EmotionConsumer/BodyMotionPlayer/motion3 播放。
- [ ] C 输出接入 mood_engine/TTS instruction。
- [ ] 增加核心链路状态面板：A/B/C 各自状态、耗时、fallback 原因。

### P2：插件运行时 + 子仓库

- [ ] 建立 live2d-ai-plugins 子仓库。
- [ ] 主仓库实现 Python PluginHost、钩子注册、错误隔离与停用。
- [ ] 把 B 站弹幕转发改成 plugin-bilibili。
- [ ] 插件管理页：列表、启停、配置、错误展示。
- [ ] 插件异常不会静默吞掉；阻塞行为必须显示。

### P3：必要链路补齐

- [ ] 知识库插件：上传/删除/检索/引用来源。
- [ ] 记忆插件：短期历史 + 长期事实/偏好 + 管理 UI。
- [ ] ASR 状态与失败可见。
- [ ] 背景图更换（已有基础，补管理 UI）。
- [ ] Live2D 悬浮窗/桌宠壳（窗口置顶/透明/拖拽/缩放）。
- [ ] 设定书/人设编辑，对齐 SillyTavern 角色卡导入导出。
- [ ] 暴露对话输入端口：HTTP 与 WS 文档化。
- [ ] 联网查询：MCP 工具接入并显示失败原因。
- [ ] 模型 zip 上传/删除/默认模型。
- [ ] 配置导入/导出/重置。

### P4：v1 发布收口

- [ ] 许可证与 SPDX 头补齐。
- [ ] 主仓库 README、快速开始、插件开发文档。
- [ ] 全量测试 + e2e 冒烟。
- [ ] 上传 GitHub 作为 v1 基线，打 tag v1.0.0。
- [ ] 明确 v1 不做什么：多平台弹幕、CLI、roleplay 等只保留插件接口。

---

## 7. 不做静默崩溃的落地规则

1. 所有模块异常都进入统一 EventBus，前端状态灯显示 LLM/TTS/Live2D/插件/直播桥健康度。
2. 降级不等于无声：任何降级都发送 degraded 事件，包含模块、原因、当前降级档位。
3. 插件钩子异常：捕获后标记该插件 error，继续主链，同时 Toast/状态灯提示；不静默跳过。
4. 3-LLM 的 B/C 超时/失败：发送 fallback 帧，前端显示“动作导演降级”“心情导演降级”。
5. 核心链路失败：LLM/TTS/Live2D 任一失败，界面必须变红或黄，并给用户可读错误；不允许模型静止不动且无提示。
6. 后台任务必须有心跳与超时；卡死要能看见，而不是一直转圈。
7. 测试层面：新增规则扫描，禁止空 except、禁止只 logger.warning 的吞错路径。

---

## 8. 立即开工项

1. 先修 BilibiliLivePlatform 缺失和 pytest 失败。
2. 搭统一错误事件通道，把“静默崩溃”全部翻出来。
3. 把 B 站弹幕脚本迁成插件，脱离核心。
4. 建立 3-LLM 契约与最小可跑 Demo：同一句回复，A 出文本，B 出表情/动作，C 出语气，超时回落规则。
5. 确认许可证方案并写入 LICENSE/NOTICE。
