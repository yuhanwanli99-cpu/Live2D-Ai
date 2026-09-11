# PLAN-V3：表演引擎复用（@soullink-emotion-sdk）实现计划

> 原则（用户确认）：能引 MIT 包就不自己写；维护 = 锁版本 + 评估同步上游改动；无 LICENSE/AGPL/GPL 只看思想不碰代码。
>
> **硬原则（本轮新增）**：
> 1. **永不静默崩溃**——任何失败必须可见（日志 ERROR + 前端 toast/error 帧），降级必须有明确出口并告知用户降到了什么状态；
> 2. **用户侧默认永远提供 LLM + TTS**（设置界面必配项）；表演链路建立在"LLM/TTS 必在"的前提上，不为缺席场景在核心里造兜底（离线情绪分类器 → 非核心可选插件）。
> 调研依据：docs/research/live2d-halfbody-motion-research.md

## 0. 复用清单（全部已发布 npm，MIT，锁精确版本）

| 包 | 版本 | 用途 | 吸收/替代我们的 |
| --- | --- | --- | --- |
| `@soullink-emotion/engine` | 0.1.0-beta.1 | VAD/FACS/Idle/Mixer/Speech/Profile 全套表演引擎 | param-arbiter 多源合并、idle/* 四件套、body-motion.ts、choreography.ts 内部实现 |
| `@soullink-emotion/profile-generator` | 同上 | 扫描皮套文件生成 soullink.profile.json | model_dict.json 静态 emotionMap |
| `@soullink-emotion/live2d-pixi` | 同上 | 仅参考其 setParameters 应用时机 | - |
| runtime-core / planner-openai | 同上 | P2 会话编排 / DeepSeek planner 对接（多帧说话演出规划） | ExpressionDirector 升级、motionTimeline 竞态治理 |
| SoulLink_Live2D | 无 LICENSE | 仅架构对照 | 不复制任何代码 |

## 1. 目标架构（引擎接管后）

```
后端（不变）：LLM ─► emotion 帧(8key+strength) + motionTimeline(括注位置) ── WS ──┐
                                                                                ▼
前端：SoullinkRuntime(profile) ◄── EmotionIntent 映射(8key→sdk 词表, strength→intensity)
      ◄── AudioLevelAnalyzer(RMS volumes 复用)   ◄── 括注 cue 到点 → 重音/显式 intent
      每帧 update(t,dt) ─► RuntimeSnapshot.live2dParams ─► coreModel 写入
```

- 口型单一来源归 SDK LipSyncController（我们喂 RMS）；眨眼/呼吸/微动/idle 小动作全由 engine 接管；
- 括注触发体验保留：cue 到点即触发当句演出重音/动作意图；
- choreography.ts 模板退役（SDK 手势+MotionStyle 取代），director 关键词表改为映射 SDK 语义键；
- UserTouch 相机拖拽不动（相机 ≠ 参数）。

## 1.5 bai 免费皮套参数盘点（实读 cdi3/model3/physics，2026-08-22）

**总量 128 参数；FileReferences.Motions={} 且 Expressions=[]（零动作/表情文件，纯参数驱动路线正确）。**

### 可用表演参数（按演出价值分层）

| 层 | 参数 | 演出用途 |
| --- | --- | --- |
| 头部（标准） | AngleX/Y/Z | 注视/歪头/点头/摇头 |
| 身体（标准+**扩展关节**） | BodyAngleX/Y/Z + **肩部×2(BodyAngleX3/X6) + 胯部(X4) + 迈腿(X5) + X2/Y2** | 半身姿态；**耸肩/换重心可用**（比预期多） |
| 眼部（标准+物理链） | EyeLOpen/ROpen、EyeBallX/Y + 18 个随动/睫毛物理链 | 眨眼/注视/游移；物理自动带动睫毛 |
| 眉毛 | BrowLY/BrowRY（仅上下） | 挑眉/皱眉近似（见缺失） |
| 嘴部（标准+特色） | MouthOpenY/Form + **嘟嘴(P4)/嚼(P7)/歪嘴(P6)** 及配套眉眼 | 口型+害羞嘟嘴/不服气歪嘴等特色表情 |
| 呼吸 | ParamBreath | 待机活人感 ✓ |
| **皮套专属彩蛋** | 翅膀×6、耳朵×6、光环×3 | excited 扇翅/倾听竖耳/思考光环移动——差异化演出（profile 专属通道） |

### 缺失参数与应对

| 缺失 | 影响 | 应对 |
| --- | --- | --- |
| ParamCheek（腮红） | 害羞/生气脸红不可 | 组合替代：歪头+EyeBall 躲闪+MouthForm 微笑+嘟嘴(P4)；**选型建议**：下个皮套优先含 Cheek |
| BrowLForm/RForm（眉形） | 皱眉/担心眉/开心眉不可，anger/sadness 力度打折 | BrowY 极值+头部补偿+力度靠身体；选型建议同上 |
| 笑眼（EyeLSmile 类） | 微笑眯眼不可 | EyeOpen 降至 ~0.25 近似 |
| 手臂/手参数 | 挥手只能以身代手 | 已决策不做骨骼级；肩部耸动近似 |
| 泪/汗特效 | 哭戏表现力 | 不做（特效贴图属素材侧） |

> 结论：bai 属"标准参数齐全 + 特色彩蛋丰富、缺面部细分类参数"的皮套。通用表演层不受阻；情绪细腻度上限受眉形/腮红限制——已在 P1 三态签名与 P2 台词治理中用节奏与台词补偿。

## 2. 阶段计划（重设计：表演 = 对话三态 × 语义重音的连续微行为）

目标重申：**像真人与对话本身**——不是大动作，而是"该看谁时看谁、在想时走神、说重点时有肢体重音"，且永不僵住、永不静默崩溃。

### P0 Spike（约 0.5 天，go/no-go）
1. renderer `devDependencies` 安装 engine / profile-generator / live2d-pixi（精确版本）；
2. `profile-generator` 扫描 bai 皮套 → `shared/model-profiles/bai.profile.json`；
   - **重点核对非标参数是否入档**：肩部 BodyAngleX3/X6、胯 X4、迈腿 X5、翅膀/耳朵/光环——决定"皮套专属彩蛋通道"可行性；
3. Node 冒烟：SoullinkRuntime(headless) update 输出参数 ID 集 ⊆ bai 实际参数集（128 个）；眨眼/呼吸/VAD 手势曲线数值抽查；
4. 记录 CapabilityDetector 对缺失通道（眉形/腮红/笑眼）的降级表现。
**验收**：无未知参数写入；缺通道降级可见；《bai 能力覆盖报告》产出。**回滚**：纯增量，不接管线。

### P1 特性开关双跑（约 2 天）——对话三态表演

表演框架 = **对话状态机（listening/thinking/speaking）× 语义重音** 的连续微行为：

| 状态 | 触发信号 | 表演签名 |
| --- | --- | --- |
| listening | 用户 sendText / 麦克风开启 | 眼球+头部转向用户方向、身体微前倾、小点头概率↑、眨眼频率略降 |
| thinking | text-delta 开始 ~ 首个音频就绪 | 视线游移（EyeBall 漂移）、歪头、手势冻结、"嗯…"微姿态 |
| speaking | 音频开播 ~ 队列空 | 口型(RMS)+motionTimeline 语义重音+情绪参数层+VAD 手势 |

改动清单：
1. 设置中心新增 `performanceEngine: legacy | soullink`（默认 legacy）；
2. soullink 分支：Runtime 适配器（~120 行胶水）——EmotionIntent 映射(8key→sdk 词表, strength→intensity) + RMS 喂 AudioLevelAnalyzer + snapshot.live2dParams 写 coreModel + motionTimeline cue 到点转发为该句演出重音/intent；
3. **三态信号零后端改动**：前端现有事件即可推导（sendText/mic→listening；text-delta→thinking；audio 开播→speaking）；
4. legacy 路径同步补齐三态最小行为（保证双跑可比性）。
**验收**：三态切换肉眼可辨且无抖动；冒烟清单（说话口型/情绪表情/括注动作/打断/切模型/断线重连）；vitest 适配器单测（fake clock+fake runtime）。
**回滚**：开关切回 legacy。

### P2 默认切换 + 台词/语音质量收口（1~2 天）
1. 默认值改 soullink；完整对话场景观察（多段 TTS + 打断 + VAD 插话）；
2. 退役 legacy 执行端：idle/*、body-motion.ts、choreography.ts、motion-timeline 执行器、param-arbiter（interaction 相机部分保留）；
3. director.py 关键词表 → SDK 语义键映射收敛；`_VALID_MOTIONS` 白名单同步；
4. **台词质量治理落地（借鉴 N.E.K.O SLOP 挖掘器）**：离线脚本扫 chat 历史 JSONL → 口头禅候选 → 人工策展 rules.json → 运行时 tts_filter 后正则治理；
5. **语音轮次打磨排期**：smart-turn 思路评估（打断阈值/静默自适应）——属核心 ASR/TTS 链路质量。
**验收**：PC pytest ≥401 保持；renderer vitest 迁移后 ≥30 例；bundle gzip 增量 <80KB；SLOP 完成一轮真实历史治理。
**回滚**：git revert 单 PR / 开关切换。

### P3 与未来功能池（独立排期；均为非核心/插件承载）

**皮套专属彩蛋通道**（profile 扩展字段驱动，其他皮套自动无此通道）：
- 耸肩组合进 shy/surprise 签名（肩 BodyAngleX3/X6）；
- excited 时翅膀扇动（翅膀×6）、倾听竖耳（耳朵×6）、思考光环缓移（光环×3）。

**对话表演深化**：
- Reflections 反思层（记忆五层第二层，N.E.K.O 对齐）；
- 害羞等缺失参数的组合签名固化（歪头+视线躲闪+嘟嘴 P4）；
- smart-turn 自适应打断阈值落地。

**扩展面插件**：
- skill-loader（pi-agent 式前置插件：skills/*.md 指令包注入）；
- mcp-bridge（P3 默认关；标准工具生态）；
- proactive 插件（freedom_llm 思想，自研 MIT）；
- emotion-classifier 可选插件（离线兜底，非核心）；
- knowledge 插件 v2：LightRAG 图检索。

## 3. 维护流程（上游同步 SOP）

1. package.json **锁精确版本**（不用 ^/~）；
2. 订阅仓库 Releases；升级 SOP：看 release notes → `npm view` 对比 → bump → 我们回归（vitest+冒烟）→ 合入；
3. beta 期（0.1.x）每次 bump 按 minor 级审查；
4. profile 重生成时机：换皮套 / 模型文件更新 / SDK ModelProfileSchema 升大版本；
5. 上游缺陷：优先提 issue/PR 给作者（同 MIT 生态反哺），紧急时在我们适配层 monkey-patch 并标注 TODO(upstream)。

## 4. 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| bai 免费皮套参数覆盖不足 | SDK CapabilityDetector+FallbackStrategy 自动降级；情绪与口型仍工作（官方承诺路径） |
| beta API 变动 | 锁版本 + 双跑开关随时回滚 |
| 表现风格不合预期 | MotionStyle(natural/lively/calm/shy)+spontaneity/gestureFrequency 可调 |
| ESM-only 集成 | Vite 原生 ESM ✓ |

## 5. 明确不做

手臂/手指骨骼控制、3D 化、动捕映射、视线注视/看向谁系统（用户确认取消）、插件逻辑进核心模块。

## 6. 总验收口径

- PC pytest ≥401 保持全绿；
- renderer vitest 迁移后 ≥30 例全绿；tsc strict 通过；
- bundle gzip 增量 <80KB；
- 冒烟清单（说话/打断/VAD 插话/切模型/低配皮套/设置开关往返）人工通过并留档 verification/。

---

## 7. N.E.K.O 设计借鉴与 skill / MCP 可行性评估

### 7.1 N.E.K.O 实现学习（代码侧：他们怎么实现 Live2D 表演）→ 采纳定位

> 澄清：学习的是 N.E.K.O 的**实现方式与接口设计**（live2d-emotion.js / emotion mapping / 参数编辑器 / SLOP 治理脚本等），不是调研其产品生态。最终目标 = Live2D+AI 的表演**像真人与对话本身**。

| N.E.K.O 设计 | 它解决什么 | 采纳定位 |
| --- | --- | --- |
| Smart-Turn v3 智能轮次判断 + RNNoise 降噪 + Silero VAD | 语音体验：该打断才打断、说完才接话 | **核心语音打磨（P2 排入）**——属 ASR/TTS 链路质量本身 |
| SLOP 规则挖掘器（离线统计 AI 口头禅 → 策展规则表 → 运行时正则治理） | LLM 口头禅毁"表演文本"质感 | **核心（维护期脚本 + 运行时规则表）**：零运行成本，直接提升台词质量 |
| 记忆五层 + 证据强化/反驳衰减 | 人设一致性、长期陪伴感 | 核心演进方向（facts+LLM 管家已对齐第一层） |
| Proactive chat 域模型 / deep-topic hooks / icebreaker | 直播主动找话题 | 插件承载（proactive 插件） |
| 三服务器 / 任务 HUD / MMD / galgame / workshop 等大量周边 | 功能竞赛 | 明确不做 |

学术参考：连续情绪空间（PAD/VAD 维度模型）驱动表情与动作力度——见 docs/research/live2d-halfbody-motion-research.md §四；co-speech gesture 生成以 3D 骨架为主，2D 参数皮套需降维映射，暂不做。

### 7.2 skill 系统 / MCP 接入可行性（"拓展手脚"，pi-agent 式前置插件）

**结论：都可行，分层采用——短期 Skill（轻、稳、零新协议），中期 MCP（重、全、标准生态）。**

| 方案 | 现状基础 | 成本 | 适用 |
| --- | --- | --- | --- |
| Skill 系统 | llm.before 注入钩子 + 插件注册表已在位：skill =「按触发条件注入的指令包 + 可选脚本工具」 | 低：新原生插件 skill-loader（读 skills/*.md 清单 → 条件匹配 → 注入 context；脚本经 tool 钩子执行） | 领域话术包、角色小技能、固定流程编排 |
| MCP 系统 | 上游 mcpp 管线在本分支保留为半成品（basic_memory_agent 已含组件，use_mcpp=False 关闭）；shared/mcp_tools.json 契约在位 | 中：启用开关 + 工具循环治理（每轮多一次 LLM 往返；需超时/工具白名单/结果截断） | 需要标准工具生态时的正式扩展面 |

落地形态（符合 pi-agent 式前置插件）：两者都做成原生插件——skill-loader（P2 后可选）、mcp-bridge（P3，默认关）。核心只暴露 hook 点与工具注册接口，不感知具体协议。

### 7.3 插件系统现状评估（诚实版）

现有 PluginHost 为单向 emit_event + 返回值合并。**够用**：输入/输出改写、上下文注入、动作/参数覆盖、管理端点。**不够**：异步长任务与双向流式（直播接入、定时主动消息需要宿主提供后台服务注册与生命周期句柄）。若要做 live-ingress 增强/定时 proactive，先给 PluginHost 增加 register_service（start/stop 生命周期）前置改造——独立小改动，不阻塞本计划 P0-P2。

---

## 8. 多 LLM 并行管线（正式化：从隐式三角色到显式导演编排）

> 背景单 LLM 无法同时保证「角色沉浸台词 + 动作表情导演 + 记忆沉淀」。现状盘点：已有三个 LLM 角色在跑，只是未正式化为管线——
> A 主对话脑 basic_memory_agent（主力模型，阻塞主链）✓；B 表演导演 ExpressionDirector（独立实例可配 provider/model，低温+64tok）⚠️半实现且串行占主链时延；D 记忆管家 MemoryCurator ✓ 链尾异步；C 心情/TTS 导演 ❌ 由规则版 mood_engine 充当。

### 8.1 同类项目佐证（多角色编排是生态惯例）

| 项目 | 多角色设计 |
| --- | --- |
| N.E.K.O | 三服务器架构（main/agent/memory 独立进程）；memory-server 内含专用 LLM 抽取/反思/晋升 |
| soullink-emotion-sdk | planner-openai 包=独立规划 LLM（反应/反思/主动消息/多帧说话动作），与对话模型分离 |
| SoulLink_Live2D | ExpressionGenerator=每句一次独立 LLM 参数生成 |
| 本仓库 PLAN-PC-V2 | 早已定义 A/B/C 三角色 + TurnBundle(sentenceId 对齐) + fallback 帧（未实施完） |
| 生态其他 | VT-Orchestrator、Project AIRI、Ikaros-521/AI-Vtuber（插件化编排） |

### 8.2 目标角色-模型分层

| 角色 | 职责 | 模型层级 | 输出契约 | 阻塞主链 |
| --- | --- | --- | --- | --- |
| A 主对话脑 | 角色沉浸台词（唯一说话的人） | 必配主力模型 | 流式文本 | 是 |
| B 表演导演 | 情绪+动作语义键+参数向量（喂 Runtime EmotionIntent） | flash 级低温导演模型 | per-sentence EmotionIntent JSON | 否（并行，缺帧兜底） |
| C 心情/TTS 导演 | mood tier / instruction / 语速音高 | 规则版可用；LLM 版可选实验 | MoodFrame/TtsFrame | 否 |
| D 记忆管家 | facts 增删合并 | flash 级 | add/remove 列表 | 否（链尾异步） |

- 模型配置：沿用 director_llm_provider/director_model，扩展 per-role 覆盖；默认回落主力 provider 的 flash 档。
- Token 预算：B ≈ 每句 ~200 tok；D ≈ 每轮 ~400 tok；C 规则版零成本。

### 8.3 时序与关键改造（对齐 P1/P2）

```
A 流式 ──句完成──► B.analyze(句)【async 不 await TTS】──┐
   TTS 合成照旧先行 ──► audio 帧(seq) ◄── 按 sentenceId 合并 B 结果(motionTimeline/EmotionIntent)
```

1. B 并行化（P1 内做）：_apply_director 从 await 后入队改为句产出即 create_task，结果按 sentenceId 回填；TTS 不等 B——未返回先发无动作帧，返回时未播到则补发 intent，已播过丢弃（lifecycleToken 思路防迟到）；
2. TurnBundle 轻量版：sentenceId + emotion + strength + motionTimeline + intent；fallback = 无 intent（前端无动作）并标注「表演导演降级」——符合永不静默崩溃；
3. C 保持规则版（克制：规则已满足分层与 instruction；LLM 版列 P3 实验，仅在证明不够时启动）；
4. D 维持链尾异步。

### 8.4 任务拆解

| 任务 | 阶段 | 文件 |
| --- | --- | --- |
| B 并行化 + sentenceId 回填 + 迟到丢弃 | P1 | single_conversation.py / tts_manager.py |
| per-role 模型覆盖配置 | P1 | service_context.py / config |
| fallback 可见化（前端“表演导演降级”提示） | P1 | ws-bridge.ts / toast |
| C 的 LLM 版实验门（默认关） | P3 实验 | mood_engine 旁路 |
