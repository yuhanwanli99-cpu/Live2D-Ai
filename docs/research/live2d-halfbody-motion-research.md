# Live2D 半身动作 / 情绪表演 —— 开源检索汇总（nanlingyin 系 + 生态）

> 检索目标：LLM 驱动的 Live2D 半身动作与情绪表演，**重点无预设参数**（不依赖手作 keyframe 模板）、跨皮套通用。
> 决策前提（用户确认）：手臂/手指骨骼控制、3D 化、动捕映射 —— 不做；只打磨 Live2D + AI + TTS 核心，额外能力走插件。

## 一、结论摘要

1. 生态共识 = 四层叠加：①模型素材(motion3/physics/pose) ②程序化三件套(眨眼/呼吸/头部跟踪) ③音频+LLM 驱动 ④动捕。我们已有 ②③ 大半。
2. **无预设参数路线的成熟开源实现存在且 MIT**：`nanlingyin/soullink-emotion-sdk` 的 `@soullink-emotion/engine`——纯 TS ESM、零运行时依赖、框架无关；VAD 连续情绪空间 + FACS/AU 表情语义 + ModelProfile 跨皮套参数映射 + MotionMixer 分层混合 + SpeechPerformancePlanner 说话演出规划。
3. 合规警示：`SoulLink_Live2D`（同作者前作）**无 LICENSE**（GitHub license API 404）→ 代码不可复制，仅作架构对照；此前外部资料标注"MIT"有误。
4. 同作者还有主动对话（freedom_llm/MIT 思路参考、astrbot 两插件 AGPL 只可远观）、LightRAG fork（MIT，知识库 v2 候选）。

## 二、nanlingyin 名下仓库分诊（31 个 → 相关子集）

| 仓库 | 语言 | License | 复用判定 |
| --- | --- | --- | --- |
| **soullink-emotion-sdk** | TS | **MIT** | ✅ 直接依赖引入（P0）：VAD/FACS/Idle/Mixer/Speech/Profile 全套引擎 |
| SoulLink_Live2D | JS | **无（保留版权）** | ⚠️ 仅架构对照：LLM_EXPRESSION_PRINCIPLE、参数清单提示词、边界过滤、idle 挂起恢复 |
| freedom_llm | Python | MIT | ○ 主动对话思路参考（对应我们 proactive 插件方向） |
| astrbot_plugin_proactive_chat / InitiativeDialogue | Python | AGPL-3.0 | ✖ 代码不可并入宽松协议核心；设计思想可参考 |
| MoeChat | - | GPL-3.0 | ✖ GPT-SoVITS 低延迟语音交互，仅参考 |
| my-neuro (fork) | Python | MIT | ○ 已在早期调研覆盖（情绪标签位置触发同源思路） |
| LightRAG (fork) | - | MIT | ○ 图 RAG（EMNLP2025）——知识库 v2 嵌入检索候选 |
| xiaozhi-esp32-server | Vue | MIT | ○ 语音交互后端模式参考（ESP32 场景外） |
| KiraAI | Python | 无 | ✖ 模块化 AI virtual being，仅概念参考 |
| 其余（工具/竞赛/个人站等 ~20 个） | - | - | 与本链路无关 |

## 三、soullink-emotion-sdk 深读（v0.1.0-beta，264 文件 monorepo）

### 3.1 包结构与职责

| 包 | 职责 | 网络 |
| --- | --- | --- |
| `@soullink-emotion/engine` | VAD/FACS/Idle/反应时序/口型/参数混合（无 DOM、无网络、零依赖） | 否 |
| runtime-core | 会话编排：消息/Planner/TTS/Audio/Clock | 视注入 |
| planner-openai | OpenAI-compatible 反应/反思/主动消息/**多帧说话动作规划** | 是 |
| classifier-embedding | Qwen Embedding 情绪分类（1400 条中文语料+规则降级+向量缓存） | 通常需要 |
| live2d-pixi | PIXI v7 渲染集成（加载模型、写 Cubism Core） | 资源 |
| profile-generator | 扫描模型文件生成 `soullink.profile.json`（可选 LLM 精修） | 默认否 |
| devtools-vue | 模型校准面板 / privateEmotionMap | - |

### 3.2 数据流（README 权威版）

```
用户消息/事件 ──► 分类(本地规则 ∥ Embedding ∥ LLM Planner) ──► EmotionIntent(emotion/variant/
    naturalVAD/intensity/contextTags) ──► VAD 情绪状态(Valence/Arousal/Dominance 三轴连续值)
        ├──► FACS/AU 表情（模型无关语义）──┐
        └──► Idle/VAD/Speech 动作 ────────┴─► MotionMixer ──► ModelProfile 参数映射
                                                    ──► Record<CubismId,number> ──► 渲染器
```

- **VAD 三轴**替代离散标签：anger 与 anxiety 同为负效价高唤起，但 Dominance 一高一低——直接决定眉形/视线/前倾/力度差异。
- **FACS/AU** 是模型无关表情语义层；`ModelProfile` 把 AU 映射到具体模型的 Cubism 参数 ID 与范围 → 同一套情绪逻辑跨皮套复用（正是我们要的多皮套适配）。
- **SpeechPerformancePlanner**：按语义情绪+VAD+语音时长+Profile 实际能力，生成头/身/视线/表情重音时间线；**只输出语义键，ParamXXX 由 ModelProfileAdapter 解析**（planner 不写死模型参数）。缺通道时能力推导自动过滤，情绪和 LipSync 照常工作。
- 口型单一来源原则：speech performance 不写 mouthOpen/eyeOpen/眨眼通道；LipSyncController 是嘴部唯一来源，重音只叠 mouth-form/眉毛/眼周/效果通道。
- `lifecycleToken` 单调令牌拒绝迟到计划（旧请求覆盖新语音的竞态防护）。
- MotionStyle 预设：natural / lively / calm / **shy**；spontaneity/gazeStability/gestureFrequency/idleActionGain 可独立调；seeded random 可复现。

### 3.3 engine/src 模块 ↔ 我们的对应物

| SDK 模块 | 我们现状 | 差距判定 |
| --- | --- | --- |
| idle/Blink·Breathing·MicroMotion·BodySway·Gaze | idle/* 四件套已自研并接线 | 功能对齐；SDK 版更全（Gaze/BodySway/IdleBias） |
| mixer/{LayeredParameterMixer,MotionMixer,PriorityRules} | param-arbiter.ts 七源优先级仲裁 | 各有千秋：我们有 Blink 眼部特权；SDK 有 speech-accent 层测试 |
| speech/{LipSyncController,AudioLevelAnalyzer} | lip-sync.ts RMS | SDK 多 attack/release 平滑+peak+VoiceWaitingMotionController |
| emotion/VAD*(State/Gesture/MicroMotion/PrivateOverlay) + ReflectionPulse + Proactive | ❌ 无 | **核心增量**：连续情绪空间驱动，天然无预设参数 |
| expression/{FACS ActionUnitSolver,EmotionArchetype,ExpressionTimeline} | ❌（我们是固定模板库） | **核心增量**：AU 语义合成时间线 |
| profile/*（Schema/Detector/Adapter/Fallback/Coverage） | model_dict.json 静态 emotionMap | **跨皮套适配的关键件**；profile-generator 可对任意新模型一键出档 |
| reaction/{MessageReactionClassifier,PlanSequencer,RecoveryController} | 部分（motionTimeline 即时层） | 计划序列化+恢复控制值得借鉴 |

## 四、SoulLink_Live2D（前作）架构对照 —— 仅研究，代码不可用（无 LICENSE）

- LLM_EXPRESSION_PRINCIPLE：运行时枚举【模型真实参数清单】(id/name/min/max/default) 全量嵌入 system prompt；LLM 输出 {expression, parameters:{...}, duration}，必须覆盖 15 个核心参数（双眼/眼球/双眉/嘴开合/嘴形/腮红/头 XYZ/身体 XYZ）。
- 代码侧边界过滤 `_clamp_parameters()`：未知参数丢弃、clamp [min,max]、exclude_mouth（口型归音频）、eye-open 可选二值化、关节类 ×joint_motion_boost(默认 1.25) 放大。
- 前端缓动平滑 duration 毫秒；生成期间挂起 idle motion3（generatedMotionLocks），播完防抖恢复。
- 对比结论：该方案=「LLM 全量参数生成」一通道；emotion-sdk 是它的工程化升级（VAD 连续层+FACS 语义层+Profile 映射+优先级混音），且补了 SoulLink 缺的并发/竞态处理。

## 五、N.E.K.O 实现学习（代码侧）

- 表情/动作映射走 Cubism 标准 FileReferences.Motions/Expressions 分组 + EmotionMapping 编辑器；运行时 setEmotion(name) 双通道应用（expression+motion），单侧缺失优雅降级。
- 模型能力可查询：提供 /api/live2d/model_parameters/{model} 参数元数据 API 与参数编辑器——与我们 ModelProfile 方向一致。
- SLOP 规则挖掘器：离线 n-gram 统计 assistant 口头禅 → 策展规则 → 运行时治理（台词质量维护脚本范式，已列入 PLAN-V3 P2）。
- 结论：N.E.K.O 表演 =「素材(motion3/exp3)+映射表+程序化三件套」素材驱动路线；bai 无素材，走「VAD/FACS+参数清单」生成路线——两条路线在参数混音层汇合。

## 六、论文与文档（学术圈现状）

- Live2D 商业闭源格式 → 学术论文极少；集中在 talking head / 2D avatar animation：[Textoon arXiv:2501.10020](https://doi.org/10.48550/arxiv.2501.10020)、[Style Transfer for 2D Talking Head (CVPRW2024)](https://openaccess.thecvf.com/content/CVPR2024W/GCV/papers/Pham_Style_Transfer_for_2D_Talking_Head_Generation_CVPRW_2024_paper.pdf)、[数字人综述 arXiv:2507.17327](https://arxiv.org/html/2507.17327v1)。
- co-speech gesture 主流在 3D 骨架（diffusion 系）；2D 参数皮套需降维映射，成本高 → 维持不做。
- 官方机制文档：[多动作管理](https://docs.live2d.com/en/cubism-sdk-tutorials/multi-motion-management-web/)、[口型同步](https://docs.live2d.com/zh-CHS/cubism-sdk-manual/lipsync/)、[眨眼设置](https://docs.live2d.com/zh-CHS/cubism-editor-manual/eye-blink-settings/)、[CubismWebFramework](https://github.com/Live2D/CubismWebFramework)（Mouth/Breath/EyeBlink 控制器源头）。

## 七、采纳路线（映射本仓库文件；全部 MIT 合规）

| 阶段 | 内容 | 落点 |
| --- | --- | --- |
| P0 基线（已完成） | 程序化三件套+emotion 帧+括注位置触发编排+日志/记忆/知识插件 | 已交付 |
| P1 引入 engine | renderer 增加 `@soullink-emotion/engine` 依赖：SoullinkRuntime(profile) 替代自研 idle 四件套与 BodyActionPlayer 内部实现；motionTimeline/live-event 触发改为投喂 EmotionIntent | renderer/package.json、main.ts、choreography.ts 退役或转薄适配 |
| P1 ModelProfile | 用 profile-generator 对 bai 及后续皮套生成 profile.json；CapabilityDetector+FallbackStrategy 保证低配皮套自动降级 | shared/model-profiles/*.json |
| P2 Planner 对接 | planner-openai 接口用 DeepSeek 实现（或直连 ExpressionDirector 升级为多帧说话动作规划）；lifecycleToken 防竞态模式移植到 motionTimeline | director.py / motion-timeline.ts |
| P3 情绪分类离线兜底 | classifier-embedding（规则降级已内置）在无 Key 时提供 EmotionIntent | 插件：新增 emotion-classifier 原生插件 |
| P3 知识库 v2 | LightRAG 式图检索替换字符重叠匹配（保持 MIT） | knowledge 插件升级 |

## 八、引用

- https://github.com/nanlingyin/soullink-emotion-sdk （MIT）— engine/planner/profile 等
- https://github.com/nanlingyin/SoulLink_Live2D （无 LICENSE，仅对照）— LLM_EXPRESSION_PRINCIPLE.md
- https://github.com/nanlingyin?tab=repositories — 作者全量仓库分诊来源
- 官方：CubismWebSamples / CubismWebFramework / CubismWebMotionSyncComponents / docs.live2d.com 各教程
- 论文：Textoon(2501.10020)；2D Talking Head Style Transfer(CVPRW2024)；数字人综述(2507.17327)
