# 需求

- taskId: `task-1786161680173`
- 版本: v0.1
- 来源: Router标签: PIPELINE（预估15任务）
- 需求分析师: agent 1 (L1 requirement-analyst)
- 项目现状证据来源: 全部来自实读——`README.md` / `PLAN.md` / `AGENT.md` / `HANDOVER.md` / `CHANGELOG.md` / `docs/*`（Phase-0 系列、PLAN-PHASE1、PLAN-PHASE1-VERIFICATION、benchmark-neko-vs-neurosama、NeuroSama-architecture-research、core-fusion-research、Phase-0-notes）、`shared/persona.yaml`、`Live2Dai-Android/app/src/main/java/com/live2dai/android/` 源码、`Live2Dai-pc/open-llm-vtuber/` 源码、`git log`/`git status`

---

## 用户故事

- US1: 作为普通用户，我希望 Android 与 Windows 双端都"打开就能玩"——启动后 3 秒内加载 Live2D 模型并开口说话，达到级3 的流畅体验。
- US2: 作为用户，我希望猫娘的表情不再生硬跳变，而是按 LLM 结构化情感（强度分级 + 多表情混合 + 参数覆盖）平滑过渡（层1 级3）。
- US3: 作为用户，我希望猫娘说话时嘴型跟随语音元音（A/I/U/E/O）真实开合，而不是随机乱动（层2 级3）。
- US4: 作为用户，我希望猫娘在说到"挥手/再见/鼓掌"等语义时同步触发对应身体动作（层3 级3）。
- US5: 作为开发者，我希望 Android 端 God Class 技术债被重构为分层架构（ViewModel + 独立 Composable），双端共享同一结构化情感协议，后续扩展不破坏核心链路。

## 验收标准

- AC1（机械）: Android 端 `./gradlew test` 全绿（含现有 ~29 个测试文件的 91 单测基线，PLAN-PHASE1 声称"91 个单测全过"，不得回归）。
- AC2（机械）: `./gradlew assembleDebug` 构建成功；APK 安装启动后 3 秒内完成模型加载并渲染（证据：logcat `Live2DRenderer` 初始化日志 + 首帧渲染）。
- AC3（机械）: 层1 级3——注入含 `emotion` JSON 块（`expressionMix` 多权重）的回复文本 → `EmotionController` 解析出多表情权重并应用（日志可见 expressionMix 解析结果）；注入畸形 JSON → 自动降级到 `[joy]` 关键词匹配，功能不中断（双路径均有单测覆盖）。
- AC4（机械）: 双端协议一致——`shared/emotion-protocol.md` schema 落地，Android/PC 解析同一 JSON schema（同一批测试样本双端输出一致）。
- AC5（机械）: 层2 级3 Android——`speak()` 后 `VisemeAnalyzer` 产出元音帧序列（`VOWEL_MAP` 与 PC 端对齐），口型参数随元音变化；三级降级链（PCM 可用→analyzeSimple→LipSyncMath 随机波动）每级可单测。
- AC6（机械）: 层2 级3 PC——预编译前端开始消费 WS payload 的 `visemes` 字段并驱动 `ParamMouthOpenY/ParamMouthForm`（证据：前端 bundle 中可检索到 viseme 处理逻辑，或录制对比嘴型与音频）。
- AC7（机械）: 层3 级3——emotion JSON 含 `motionLabel` 或 TTS 播报到含"挥手/再见"等关键词的文本位置时，对应 motion 在播报期间触发（日志/动画可验证）。
- AC8（机械）: 重构验收——`MainActivity.kt ≤ 250 行`（wc -l，现工作区已 24 行）；`ChatViewModel` 存在且用 `StateFlow`；`MainChatScreen` 拆分为独立 Composable 文件（ui/ 下）。
- AC9（机械）: PC 端 `python -m pytest tests/`（8 个测试文件，含 `test_pymouth_viseme.py` 25 用例）全绿。
- AC10（流程）: 工作区未提交 WIP（ChatViewModel.kt/MainScreen.kt/Live2DaiApp.kt 等）先提交或基线化，重构与级3 任务的验收不互相污染。

## 边界

- in:
  - Windows 端（Live2Dai-pc）三层推进级3：L1 结构化情感 JSON、L2 前端消费 visemes、L3 动作联动。
  - Android 端（Live2Dai-Android）三层推进级3：L1 结构化情感 JSON + 表情混合、L2 VisemeAnalyzer 元音口型、L3 动作联动。
  - 共享核心层（shared/）：统一情感协议 schema（emotion-protocol.md）+ persona.yaml 继续作为单源真理。
  - 深度重构 Part A：MainActivity 拆分、ViewModel 引入、状态集中管理、插件懒加载、UI 分层（P0 项）。
  - 每个选型点给出"最优 + 降级"两条路线（用户约束 3）。
- out:
  - 不做云端配置同步（跨端实时同步，README 明确"跨平台一致性优先于跨端实时同步"）。
  - 不做 Neuro-sama 式长期记忆/主动陪伴系统照搬（用户明确非目标）。
  - 不做 mod/扩展插件生态（用户明确"mod 可不做"）。
  - 不做直播推流/演出形态。
  - 不做 N.E.K.O. 式 Docker 多服务架构（对 Android 过重）。
  - 不做多维情感向量（PAD/EVOKE）等学术未落地方案（core-fusion-research 已评估为"不必直接用"）。

## 非目标

- 不照搬 Neuro-sama 的记忆系统与主动陪伴（用户明确；记忆相关仅保留现有 ChatHistoryStore 本地历史，不新增反思/固化层）。
- mod/扩展插件可不做（用户明确）；核心只打磨 live2d + tts + 人设与 ai 的连接。
- 单轮对话不追求多模态记忆（视觉/语音/文本跨模态记忆不纳入本任务）。
- 不引入需要 Live2D 官方许可的闭源 SDK 依赖（保持 Purism Core MIT 路线）。
- 不做 PC 前端从零重写（优先 fork Open-LLM-VTuber-Web 重构建，保留最小改动）。

## 数据缺口清单（强制，不允许空）

- GAP1: 级3 精确定义标准——现有 git 提交命名（"级3-A emotion intensity"Android 强度后缀、commit 7bb7d01；"级3-B pymouth vowel viseme"PC 后端、commit f6f9800）与 docs/PLAN-PHASE1.md 的级3 定义（L1=结构化 JSON expressionMix、L2=元音 viseme、L3=文本位置动作）存在两套口径 | 用途: 作为全部 AC 的验收基准，裁决"级3 还剩什么" | 状态: partially filled（Phase-0-notes-refactor-level3.md §四 与 PLAN-PHASE1.md Part B 有完整分级表，但 git 提交命名口径不一致） | 检索词: Live2Dai 级3 定义, expressionMix 结构化情感 分级, lip-sync viseme 等级标准 | 原因: 两套口径冲突（强度后缀算不算级3？PC 后端 viseme 算不算完成？），不裁决则 developer 不知道验收目标。
- GAP2: Neuro-sama 与 N.E.K.O. 项目参考（ai+tts+live2d 组合模式）| 用途: 对齐"组合模式效果"，选最优+降级路线 | 状态: filled（docs/benchmark-neko-vs-neurosama.md、docs/NeuroSama-architecture-research.md、docs/NEURO_LIVE2D_RESEARCH.md 已覆盖；Neuro 渲染本体闭源，以社区复刻 my-neuro/LLM_Live2D 为证） | 检索词: Neuro-sama architecture, N.E.K.O. github 架构, AI VTuber pipeline 开源 | 原因: 参考数据已充足；唯一 unfilled 部分是 Neuro 官方渲染细节（闭源，按"诚实标注不确定"原则进 gap，不编造）。
- GAP3: ai+tts+live2d 组合模式的成熟开源方案——PC 侧已确认（Open-LLM-VTuber + pymouth + pixi-live2d-display），**Android 侧无现成轮子**：元音 viseme 需 Kotlin 移植，DTW 移植 vs TarsosDSP vs Chaquopy 三选一未定 | 用途: L2 级3 Android VisemeAnalyzer 实现选型 | 状态: partially filled（方案清单已读实，移植路线未定） | 检索词: pymouth android port, Android viseme analyzer kotlin, TarsosDSP live2d lip sync | 原因: PLAN-PHASE1-VERIFICATION 假设1 已证明纯 FFT 不等于 pymouth 的 MFCC+DTW，算法假设需修正后再定移植方案。
- GAP4: 双端技术栈与 Live2D SDK 集成方案——Android=Purism Core(MIT, CMake/NDK 27)已读实；PC=Open-LLM-VTuber fork + 预编译 React bundle（frontend/ 无源码、无 viseme 消费）；**PC 前端修改路径未定**：fork Open-LLM-VTuber-Web 重构建 vs 改 bundle | 用途: PC 端 L2/L3 前端改动落地方式 | 状态: partially filled（技术栈已读实；前端改造路径待架构师定标） | 检索词: Open-LLM-VTuber-Web rebuild, pixi-live2d-display lipsyncpatch, Cubism SDK for Web viseme | 原因: 预编译 bundle 不可直接改（PLAN-PHASE1-VERIFICATION 假设2 已验证），方案取舍需在架构阶段决策。
- GAP5: TTS 与唇形同步方案（Android 拿 PCM 的接口改造）——已确认 sherpa-onnx 返回 PCM float、Edge/CosyVoice 返回 MP3、系统 TTS 无 PCM；`TtsProvider.speak()` 现契约"只进不出"，无 PCM 输出通道 | 用途: L2 级3 Android VisemeAnalyzer 的音频源与接口改造 | 状态: partially filled（VERIFICATION 假设4 已逐 Provider 读实；接口改造方案未定：sherpa 截取 vs MediaCodec 解码 vs AudioRecord 回采） | 检索词: Android TTS get pcm data, MediaCodec mp3 to pcm android, sherpa-onnx android tts pcm | 原因: 不做接口改造则 VisemeAnalyzer 无音频数据可分析，此 gap 是 L2 级3 的硬前置。
- GAP6: 重构 Part A 的当前工作区 WIP 归属——`git status` 显示未提交：MainActivity.kt 767→24 行（已改）、ChatViewModel.kt 425 行（新增）、ui/MainScreen.kt 431 行（新增）、Live2DaiApp.kt 84 行（新增）、ChatHistoryStore.kt（改）、PC crash_reporter/（新增）；HEAD=e0c3495 checkpoint"before developer-pro" | 用途: 任务切分——重构已完成部分 vs 待做部分，避免 developer 重复/冲突 | 状态: unfilled（需要主代理/用户裁决：先提交 WIP 作为重构基线，还是回滚重做） | 检索词: git checkpoint before developer WIP, 未提交重构 基线化 策略, WIP commit 验收 边界 | 原因: 我无法确定该 WIP 是验收通过的重构成果还是半成品；不裁决则 AC8 的"重构验收"无基线可比。
- GAP7: 表情混合的参数叠加顺序与冲突规则（Android ExpressionManager）——VERIFICATION 假设3 确认 applyExpression 已有 Add/Multiply/Overwrite 三模式加权叠加基础设施，但多表情混合的叠加顺序未定义 | 用途: L1 级3 expressionMix 实现细节（防参数竞争） | 状态: partially filled（基础设施已读实；顺序规则待实现定义） | 检索词: Cubism expression blend add multiply overwrite order, Live2D expression mix 参数冲突, expressionMix 叠加顺序 | 原因: 多表情同时写同一参数（如 ParamMouthOpenY）会竞争，需明确 Add→Multiply→Overwrite 顺序，否则验收 AC3 会抖动。

## 禁止

- 禁止编造数据：本需求全部现状判断基于实读文件（见头部证据来源），未确认项一律进数据缺口清单（GAP1/GAP6 等），不猜测。
- 禁止把 git 提交命名（级3-A/级3-B）当作已完成级3 的验收证据——需先经 GAP1 裁决口径。
- 禁止照搬 Neuro-sama 记忆/主动陪伴与 N.E.K.O. Docker 架构（用户明确非目标）。
- 禁止在重构中删除现有 91 单测覆盖的核心链路代码（ChatService/EmotionController/VoiceIoController 等"不动区"）。
