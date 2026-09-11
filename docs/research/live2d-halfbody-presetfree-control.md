# Live2D 半身控制 · 无预设皮套路线专项（2026-08）

> 调研日期：2026-08-23
> 定界（用户确认口径）：**只做半身**（头 XYZ / 眼球 / 眉 / 呼吸 / 身体 XYZ / 发丝物理联动），**不做全身**（手臂骨骼、手指、3D 化、动捕——项目既定排除项）；重点 = **无预设**：不依赖手作 motion3/exp3、逐模型模板与 hotkey，任意皮套开箱可用。
> 与既有文档关系：`live2d-halfbody-motion-research.md` 已深读 soullink engine 全貌与本仓库差距表，本报告**不重复其 §3.2 数据流**，聚焦「无预设跨皮套」机制的源码级实锤、上游一周前的新进展、以及 Neuro 系对照。
> 纪律同前：每条附来源；未查证处标 ⚠️。

---

## 一、TL;DR

1. **无预设半身控制已有完整三层契约，且每一层都有 MIT 源码可抄作业**：L1 官方标准参数 ID（跨皮套事实契约）→ L2 FACS/AU 语义键到标准 ID 的规范映射（soullink `STANDARD_PARAM_TABLE`）→ L3 运行时枚举 + profile 自动生成 + 12 位能力检测降级。
2. **上游一周前刚落地关键件**：soullink PR#3「capability-aware speech performance planner」（2026-08-18 合并）——语义手势经具体 ModelProfile 映射过滤，带动作预算/历史连续性/低配降级，是无预设路线当前最完整的工程化形态。该 SDK 近两周仍有多笔并发竞态修复提交，活跃可信。
3. **Neuro 系对照结论：本体与全部主流复刻都停在「预设绑定」层**（exp3/motion3/VTS hotkey），身体近乎静止；以 Neuro-like 为目标做无预设半身表演，是开源生态尚未被占据的差异点。
4. **预设资产不必丢弃**：`NativeAnimationResolver` 把已有 exp3/motion3 收编进 profile 绑定并与参数层互斥防冲突——迁移可以渐进。

---

## 二、「无预设」三层契约（源码实锤）

### L1 官方标准参数 ID —— 跨皮套的事实契约

来源：<https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/>（表格实读）

| 通道 | 标准 ID | 默认范围 | 备注 |
| --- | --- | --- | --- |
| 头转 XYZ | ParamAngleX/Y/Z | ±30 | |
| 眼球 XY | ParamEyeBallX/Y | ±1 | |
| 眼开闭/笑眼 | ParamEyeL/ROpen、EyeL/RSmile | 0–1 | 开闭上限可到 1.2/1.5 |
| 眉 Y/X/Angle/Form ×左右 | ParamBrowLY/RY/LX/RX/LAngle/RAngle/LForm/RForm | ±1 | 六自由度微表情 |
| **身体转 XYZ** | **ParamBodyAngleX/Y/Z** | **±10** | 半身核心 |
| 呼吸 | ParamBreath | 0–1 | |
| 手臂 A/B ×左右 | ParamArmLA/RA/LB/RB | ±30 | **带 \* 号=非保证存在，本项目不承诺** |
| 发丝摆动 前/侧/后 | ParamHairFront/Side/Back | ±1 | **官方注明通常由 physics 驱动** |

要点：标准 ID 是绝大多数商用/社区皮套的公共子集，但**覆盖不齐**（Arm 带 \*、部分模型改自定义 ID）——这正是需要 L2/L3 的原因。

### L2 FACS/AU 语义键 → 标准 ID 规范映射（soullink `standardParamTable.ts` 实锤）

来源：<https://github.com/nanlingyin/soullink-emotion-sdk/blob/main/packages/profile-generator/src/standardParamTable.ts>

- 表结构：每个 FACS 键给出 `ids/pair/group` + `mode/scale/min/max`，注释声明这是 heuristic 的**唯一权威数据记录**（"data, not a second source of truth"）。
- 半身相关摘录：
  - `headX/Y/Z` → ParamAngleX/Y/Z，scale 30，±30
  - `bodyX/Y/Z` → ParamBodyAngleX/Y/Z，scale 12，**±12（比官方默认 ±10 放宽）**
  - `eyeOpen` 左右 pair → EyeL/ROpen，max **1.2**；`gazeX/Y` → EyeBallX/Y ±1
  - `browInnerUp`→BrowLY/RY；`browOuterUp`→BrowL/RAngle(scale 0.9)；`browDown`→BrowL/RForm
  - `mouthOpen` 归 **LipSync group**（口型单通道原则在映射表层面就隔离了）
- **设计取舍实锤**：tear/sweat/mouthFrown/mouthPucker/eyeSquint 等「仅名称可辨或需差值推导」的键**故意不进规范表**——规范表只收 ID 确定无疑的通道，其余交给 heuristic/LLM 兜底。这避免了错误映射污染跨皮套可靠性。

### L3 运行时建档 + 能力降级（`Live2DProfileAutoGenerator.ts` v3 + `CapabilityDetector.ts`）

来源：packages/profile-generator/src/ 与 packages/engine/src/profile/（api.github.com 实读）

自动建档流程（`soullink-profile-autogen-v3`）：

1. 扫描模型目录：model3.json（FileReferences：Expressions/Motions/Physics）+ **cdi3.json（参数显示名与分组）** → 得到全量参数清单 `{id,name,groupId,groupName}`
2. 双提供者：`heuristic`（按 STANDARD_PARAM_TABLE 推导）∥ `openai-compatible`（LLM 对自定义参数精修，失败自动落回 heuristic）
3. 产物 ModelProfile 含 neutralParams / parameterSmoothing / AdaptationCoverage；目录签名 hash → `current/missing/stale/forced` 增量再生
4. `SaveCalibratedProfileRequest`：devtools 人工校准结果可覆盖固化（自动→人工两段式工作流）

能力检测（12 个布尔位）：`headControl / bodyControl / eyeBlink / eyeSmile / gazeControl / mouthOpen / mouthSmile / browControl / blush / tear / sweat / breath`——缺哪位就关哪条表演通道，**这就是「无预设但可降级」的运行时保证**。

---

## 三、上游最新：PR#3 能力感知说话演出规划器（2026-08-18 合并）

来源：<https://github.com/nanlingyin/soullink-emotion-sdk/pull/3>

PR 描述要点（原文摘译）：

- 确定性、能力感知的语义说话演出规划器，改编自 "Qiling motion-planning approach"（⚠️ 该出处未能检索到公开仓库，待问作者）
- **手势与表情重音经具体 ModelProfile 映射过滤**后再下发——异构皮套只在映射层分叉，规划逻辑统一
- 有界曲线（bounded curves）、历史感知连续性（避免逐句跳变）、**动作预算**（motion budgets，防演出过载）、reduced-motion 与低配皮套降级
- 运行时生命周期 replace/append/interrupt/clear，与 LipSync 及手动层的所有权划分明确

同期提交（活跃度佐证）：runtime-core 并发消息 latest-wins、过期 reflection 失效修复（2026-07-28/29）。stars 已从上次调研记录增长至 91。

---

## 四、Neuro 系对照：无预设半身是复刻生态的空白点

### 4.1 各家实际做法（本轮新增实锤）

| 项目 | 身体/半身动作做法 | 路线判定 |
| --- | --- | --- |
| Neuro-sama 本体 | 表情标签驱动 + VTS 音频口型；身体基本静止（既有报告 §1.5/§3 结论） | 预设绑定（极简） |
| kimjammer/Neuro | VTS hotkey 触发**预设动画**（README 自述例子：麦克风滑入/滑出）；默认 Hiyori；捆绑 mao_pro（exp_01~08 + mtn_01~04 通用组名） | 预设绑定 |
| morettt/my-neuro | 情绪→动画索引（既有 NEURO_LIVE2D 调研） | 预设绑定 |
| N.E.K.O | Motions/Expressions 分组 + EmotionMapping 编辑器（既有 halfbody 调研 §五） | 预设绑定+映射工具 |
| XnneHangLab #379 | 以 "Neuro-like" 为目标提案参数级控制（上轮专项已详述） | 无预设方向提案 |
| soullink-emotion-sdk | VAD/FACS + profile + mixer（本报告 §二/三） | **无预设，唯一完整实现** |

### 4.2 两条路线对比

| 维度 | 预设绑定（exp3/motion3/hotkey） | 无预设（语义层+profile+mixer） |
| --- | --- | --- |
| 新皮套接入成本 | 需要现成素材；无素材=零表演 | 扫描即出档，低配皮套自动降级 |
| 表演粒度 | 句级离散切换 | 参数级连续（含幅度/时长曲线） |
| 跨皮套泛化 | 每模型手工映射 | 标准 ID 契约自动覆盖大多数 |
| 自然度上限 | 受素材质量限制 | 可表达"思考中/得意/吐槽"等细微状态 |
| 当前生态占有 | Neuro 系全部复刻 | soullink 一家 + #379 提案 |

**结论**：Neuro 本体的表演重心在人设与对话而非身体（身体近乎静止也不妨碍其成功——见 benchmark 报告"人设是灵魂"结论）；但作为**产品功能**，无预设半身表演恰好是所有复刻都没做的空白，且与我们"动动手指即可配置"的低门槛定位一致（免素材、免逐模型调参）。

---

## 五、资产收编：预设不是敌人（NativeAnimationResolver 实锤）

来源：packages/engine/src/profile/NativeAnimationResolver.ts

- profile 可携带 `expressionMap/motionMap` 把现有 exp3/motion3 登记为原生动画绑定；EmotionIntent 按 `<emotion>:<variant>` 复合键优先、裸 emotion 键兜底解析，支持 minIntensity 门控。
- 命中原生表情时回填 `suppressParamIds`（来自 catalog 的 params 清单），**防止参数层与原生表情双重驱动同一批参数**。
- 单调 token 只在指令真正变化时调 model.expression()/motion()；profile 无映射时输出与引入前完全一致（向后兼容承诺写在注释里）。

对迁移的含义：bai 现有 emotion 帧与 motion 资产可以先"收编"进 profile 作为过渡层，再逐通道替换为纯参数驱动——不需要一刀切。

---

## 六、对本项目的落点建议

1. **半身通道白名单（按降级优先序）**：headXYZ → gazeXY → brow → breath → bodyXYZ；Arm 明确不承诺（官方带 \*）；Hair 不直接驱动，靠头/身体参数经 physics 联动拿免费的次级运动。
2. **幅值不写死**：官方 body 默认 ±10、soullink 用 ±12——clamp 必须读各自 profile 值，禁止全局常量（对应我们 param-arbiter 的改造点）。
3. **profile-generator 进工具链**：参考其 scripts/generate-profile.mjs 先例，对 bai 出档 → devtools 校准 → SaveCalibratedProfile 固化；P1 引 engine 时一并纳入（衔接既有采纳路线表 P1 行）。
4. **口型单通道原则不动摇**：STANDARD_PARAM_TABLE 把 mouthOpen 划入 LipSync group、PR#3 显式维护 LipSync 所有权——与我们 lip-sync 单一来源约定同构。
5. **升级观察项**：PR#3 的 motion budgets / history continuity 是我们 motionTimeline 目前没有的概念，P2 Planner 对接时应移植。
6. **差异化叙事确认**：「无预设半身表演」在 Neuro 复刻生态中无人占位，可作为对外卖点写入产品文案（免素材、换皮套零成本）。

---

## 七、引用

- 官方标准参数表：<https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/>
- soullink-emotion-sdk（MIT）：standardParamTable.ts ／ Live2DProfileAutoGenerator.ts ／ CapabilityDetector.ts ／ NativeAnimationResolver.ts（api.github.com contents 实读，2026-08-23）
- PR#3 capability-aware speech performance planner：<https://github.com/nanlingyin/soullink-emotion-sdk/pull/3>
- 近期 commits（latest-wins / stale reflections）：repo commits API，2026-07-28 ~ 2026-08-18
- kimjammer/Neuro README（VTS hotkey 预设动画自述）：<https://github.com/kimjammer/Neuro>
- Open-LLM-VTuber live2d_model.py（emotionMap → [key] 机制复核）：src/open_llm_vtuber/live2d_model.py
- 交叉引用：live2d-halfbody-motion-research.md（engine 全貌）、industry-emotion-tts-survey.md（A/B 两派）、live2d-ai-control-ecosystem.md（五类通道与 #379 详述）
