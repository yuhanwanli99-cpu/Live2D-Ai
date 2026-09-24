# 下一轮范围：动作系统 / 主页语音 / 单会话记忆 —— 选型与实施规格（2026-09-15）

> **来源**：用户 2026-09-15 三条要求 —— ①记忆方向＝**单会话隔离 + 上下文自己管 + 纯对话**（不做客服/工单模型）；
> ②动作系统参考 **Live2D 标准皮套（底层）** 与 **N.E.K.O**（但 N.E.K.O 很重且绑单角色），需要**具体的按钮调试**
> 与**导演额外开一条异步 LLM 链路**管理 TTS 输入内容与动作的同步（要**选型**）；
> ③开启语音 Mod 后主页加**语音对话开关**、**按住说话**、内置**关键词唤醒（"小可爱"，类似 Hey Siri）**。
>
> **本文是设计/选型，不是实现**。现状依据：`mod/l1-product` worktree（只读实测，行号基于该快照）。
> 安全项按用户口径继续降权。

---

## 0. 记忆方向确认（先说结论）

**确认不做**：客服/工单模型（Chatwoot、春松式「一访客一工单流」）、群聊/多渠道揉成一条队列。

**要做**：单会话隔离（一个会话一套上下文）、上下文自己管（组装 + 预算 + 压缩）、纯对话（不进工作流/坐席）。

**同类对照**（官方文档/源码核实；📌 事实、🧠 推断，来源见 §6）：

| 项目 | 隔离单元 | 两层? | 上下文组装（有序） | 压缩 | 卡/世界书 | 对我们的含义 |
|---|---|---|---|---|---|---|
| **SillyTavern** | chat（群聊**共享历史**，只换卡） | ❌ | Main → WI(before) → Persona → Description/Personality → Scenario → WI(after) → Chat Examples → 历史 → Post-History（WI/作者注/摘要可 **@depth**） | Summarize 写进 chat metadata，**可回滚/暂停** | 最强 | 上下文栈的**参考上限**；我们不做世界书/作者注/Regex |
| **Agnai** | chat（多租户） | ❌ | Scenario/Persona/Sample + Memory Book 命中 + 历史 | 无自动摘要 | Memory Book≈lorebook | 多用户隔离对我们无意义（本地单人） |
| **RisuAI** | chat | ❌ | 自定义顺序 + lorebook + regex | Hypa/SupaMemory | 有 | 轻量替代的取舍参考 |
| **AstrBot** | **session(UMO) + conversation(uuid)** | ✅ | system/persona + 扁平消息 | **窗口 82% 触发**；默认丢 1 轮；LLM 摘要留最近 4 轮；仍超则对半砍 | ❌ | **最值得抄概念**：窗口与历史分离 + 阈值 + 留最近 K 轮 |
| **LibreChat** | conversation（+user） | ❌ | system/预设 + 历史 + RAG；fork 派生新 conversation | ⚠️ 未见官方自动摘要 | persona | 「一条对话一条线程」的干净模型 |
| **Open WebUI** | chat（+user） | ❌ | system prompt 三层级 + 历史 + RAG + Memory | 超窗截断，无自动摘要 | ❌ | memory 作为独立层 |
| **LobeChat** | **assistant(人格) + topic(历史)** | 部分 | assistant system role + topic 历史 + Memory | ⚠️ topic 内全量 | ❌ | 人格与历史解耦 |
| **Chronicler** | session + character + world | ✅ | 分层检索 canon→heuristic→reflex | 三层记忆替代滚动摘要 | v2/v3 ✅ | 记忆带 `session_id` + 命名空间 + provenance |
| **Serene Pub** | chat（多账号） | ❌ | Handlebars Context Config：system + 人设 + scenario + lorebook + 历史 + reminder | **手动**摘要→lorebook，**不删原消息** | ST 全导入 | 模板化组装 + 「摘要不删原文」 |

**选型结论（对 Live2D-Ai）**：

1. **两层「概念」要，两层「实现」不要**：`Session` = 活动窗口（唯一，持 persona / 记忆桶绑定）；
   `Conversation` = 一条 JSONL 历史（可多条、可清空/fork）。本地实现只需 `conversation_id` + `active` 标志。
   好处很实在：现在「**清空聊天记录**」和「**删记忆**」是同一个含糊动作，分开之后各自有明确语义。
   UI / API 措辞统一用 conversation（`session_id` 作为兼容别名保留一版）。
2. **上下文组装固定五段**（有序、可预算）：
   `system 匿名基线` → `persona 槽（本 conversation）` → `memory 槽（本 conversation）` → `对话历史（引擎 max_history_pairs）` → `摘要（末尾）`。
   不做世界书 / 作者注 / Regex / Instruct（重度 RP 才需要，违背「最小」）。
3. **压缩阈值挂在「注入预算」上，不挂在模型窗口上**（AstrBot 的 82% 是模型上下文窗口比例；
   我们的瓶颈是 persona+memory 槽的注入预算）：
   - 平时**按轮截断**（O(1)、确定、零额外 LLM 调用）；
   - memory 注入 **> 预算 60%** → 按分数裁 top-k（条数不是预算）；
   - **> 预算 75%** → 触发摘要（**带冷却**：每 N 轮最多一次），且**保留最近 K=4 轮原文**；
   - 摘要**可回滚、带版本**（ST「Restore Previous」/ Serene「不删原消息」）：写旁车文件并记录
     「本摘要覆盖到哪个消息位点」，回滚 = 丢弃摘要、用回原文。
4. **删掉全局降级**：无会话不再写 `persona.system_prompt`（那是「两份真相」的来源，还会污染所有会话）；
   改为只投本轮匿名槽，UI 明示「还没有 conversation，本条只在本次生效」。
5. **每条记忆带 provenance 且可编辑删除**（来源 turn / 时间 / 为什么被检索到），
   跨 conversation 记忆**默认关闭**，要开就用独立命名空间，不混进按会话分桶。
6. **memory 保持单文件会话桶**（`memory/<conversation_id>.jsonl`），只写自己的 owner 槽，
   **不整份改写** persona。

> 完整单会话记忆设计（数据模型 / worker 化 / 尾部读 / 缓存 / 相似度 / 分阶段）见
> `docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md` §4。

---

## 1. 动作系统选型（标准皮套 / N.E.K.O 参考）

### 1.1 现状与硬约束（实测）

- `l2d` 的姿态分层栈：**base < idle < input < physics < final_override**（`pose_stack.rs`）；纯逻辑可测。
- **没有 motion3/exp3 加载器**：`crates/l2d/src/` 只有 asset/format/model/pose_stack/renderer，没有动作/表情文件解析。
  所以 v1 动作只能是**参数直写**。
- 现有 preset 走 **input 层**的 `set_parameter`（`preset.rs` → `input.rs:199-206`）；
  `final_override` 层**当前没有任何写入方**（`input.rs:215-217` 明文）。
- **实测冲突（本轮抓到）**：idle 微表情写 `ParamBrowLY/ParamBrowRY/ParamMouthForm/ParamEyeBallX/Y`
  （`idle.rs:226-230`），而 `expr_smile` 也写 `ParamMouthForm/ParamBrowLY/ParamBrowRY`；
  同一 **input 层**内，`apply_idle_life` 在 preset 之后执行（`input.rs:196` vs `:266`）
  → **idle 会盖掉表情的嘴形/眉毛**（微表情触发那一小段会「表情闪一下又变平」）。
- 口型走 `ParamMouthOpenY`，动作表刻意不碰（这条保持，别动）。

### 1.2 方案选型

| 方案 | 实现成本 | 可移植性（换模型） | 表现力上限 | 与 idle/物理/口型 | 需模型自带文件 | 结论 |
|---|---|---|---|---|---|---|
| **A 纯参数预设**（现状 + 修层级） | 低 | 最好（有参数就演，没有就 no-op） | 表情 + 头/身角度、视线、嘴形 | 可完全掌控（改层级/仲裁） | 否 | **基线，必做** |
| **B 参数 + exp3**（需给 l2d 加 exp3 加载器） | 中 | 中（要模型导出 exp3） | 更精细表情（多参数组合/Blend/Fade） | exp3 是覆盖式，仲裁要自己写 | 是 | **P2 可选** |
| **C 参数 + motion3**（NEKO 路线） | 高（加载器 + 曲线 + 口型曲线清洗） | 差（每个模型自带 motion，绑单角色） | 手臂/手指/复杂编排 | 与口型/物理冲突面大（NEKO 要清 motion 里的口型曲线） | 是 | **不做** |
| **D 混合（A 为主 + B 可选）** | 中 | 好（无 exp3 时自动降级到 A） | 上限高于 A | 同一套仲裁 | 可选 | **推荐** |

**推荐：D-lite = A 为唯一必需路径，B 作为 P2 可选增强；明确不学 N.E.K.O 的 motion3 路线。**

本轮**拿到了一手证据**（浅克隆 N.E.K.O 全文检索 + 官方 docs），补充三点：
- 它的 `emotion_mapping` **写在该模型的 `.model3.json`**（`FileReferences.Motions/Expressions` +
  兼容字段 `EmotionMapping`）→ 天然**按模型绑定**，换皮套要重配，正是我们不要的形态；
- 它自己也有一条「无素材」回退 `playSimpleMotion`：**happy/sad/surprised 写 `ParamAngleY` ±5~8、
  angry 写 `ParamAngleX` ±5** —— 这几乎就是我们现在的 `nod/shake` 参数路线，说明 A 不是「凑合」，
  而是**无素材时的正解**；
- 它解决口型冲突的办法是**上传时改写用户的 `motion3.json`**（把命中官方嘴部白名单 ∪
  `Groups`→`Name=="LipSync"` 的曲线的 `Segments` 置空后原子写回）。**我们不抄**：
  动用户模型文件是重风险，参数直写天然不碰口型就绕开了整个问题。

**明确不抄 N.E.K.O 的**：① 为情绪判断**再打一次 LLM**（前端 5s 超时）；② 手写双槽互斥管抢占；
③ 上传时改写用户模型文件；④ 三套 FastAPI + 三条 ZeroMQ + Electron + Steam Workshop 的拓扑
（`requirements` 1377 行、32 个直接依赖）——「很重」指的就是这些。

### 1.3 层级与仲裁选型（修掉 §1.1 的冲突）

| 通道 | 写哪一层 | 理由 |
|---|---|---|
| 口型（`ParamMouthOpenY`） | `input` | 现状；必须**永不被动作盖** |
| 待机生命体征（呼吸/眨眼/微表情） | `input` | 现状；保留 |
| **动作/表情预设** | **`final_override`**（本轮新增写入方） | 最高优先级 → 不被 idle 微表情盖、不被物理盖；到点 `clear_override_parameter` 整批撤销 |
| 物理 | `physics` | 不动 |

- 备选方案（更自然但更复杂）：**参数 ownership** —— idle 在写微表情前查「本帧该参数是否被动作声明」，
  声明了就跳过（soullink 的 ownership 思路）。**本轮不选**：`final_override` 已经能修掉当前缺陷，改动小。
- **优先级/打断只在「同层内」生效**（跨层仍由 `base<idle<input<physics<final_override` 决定）：
  `reaction（按键/点击） > speech（伴随一句说话的动作） > emotion（表情） > idle`；
  进行中的短动作默认**不可被同级打断**，由 `priority` + `preempt` 决定。
- **`persistent` ≠ 高优先级**：N.E.K.O 的「常驻」实现是「**任何清空/重置后立刻重放一次** +
  参数 ID 进保护白名单」，不是抢占锁。我们照这个语义：`persistent=true` 的表情在被 clear / 换轮后自动重放，
  且它的参数不许 idle 擦掉。
- **冷却**：同 `(preset_id)` 在 `cooldown_ms` 内不重放（防止连续同情绪刷屏）。
- **缺参数：先探测再写，别只靠静默 no-op**。Cubism Web 的 `getParameterIndex()` 对不存在的 ID 返回
  「非存在参数」索引，值进 `_notExistParameterValues` 边表、**永不落核心模型**（官方语义即静默 no-op、不抛错）。
  本仓已有 `ModelHandle::has_parameter()`、`PoseStack::set_parameter()` 返回 `false`、`idle.rs` 自动跳过——
  动作层应**按 `has_parameter` 过滤后再写**，做**通道级降级**（缺哪条只关哪条），
  并让「参数探针」（§2）把这份能力集显示给用户。soullink 把它做成 12 位能力位，可参考。

### 1.4 数据模型（**外置**，不硬编码进 Rust）

```json
{
  "id": "expr_smile",
  "kind": "expression",
  "params": [{ "id": "ParamMouthForm", "value": 1.0, "blend": "overwrite" }],
  "duration_ms": 2600,
  "fade_in_ms": 120,
  "fade_out_ms": 200,
  "priority": 20,
  "cooldown_ms": 1500,
  "persistent": false
}
```

- 存 `assets/actions/presets.json`（或缺省内建 + 用户覆盖）；渲染面启动时加载并校验。
- **保留红线回归**：只允许面部/头角度参数（`ParamMouthForm/Eye*/Brow*/Angle*`）、幅值 ≤30 —— 现状
  `presets_stay_inside_the_allowed_channel` 就是这条，改成读外置表后继续保留。
- 缺参数静默 no-op（现有 `set_parameter` 语义）；「参数探针」（§2）用来告诉用户缺哪些。

> 淡入淡出的简化：VTube Studio 的 `ExpressionActivationRequest` 只能设**淡入**（0–2s 钳制），
> **淡出永远复用淡入值**——可以照抄：数据模型里一个 `fade_ms` 双向复用，少一个容易配错的参数。

### 1.5 用「语义通道」而不是裸参数 ID

直接写 `ParamBrowLY` 会把动作绑死在 Live2D 命名上，也做不了通道级降级。建议表里写**语义键**，
下发时再按 `has_parameter` 映射到实际参数：

```rust
struct ActionPreset {
  id: &'static str,        // 稳定契约：expr_smile / nod / look_left
  kind: ActionKind,        // Expression | Gesture | Persistent
  channels: Vec<Channel>,  // 语义键，不是参数 ID
  duration_ms: u32,
  fade_ms: u16,            // 双向复用
  envelope: Envelope,      // Hold | Sine | Linear
  layer: Layer,            // Idle | Input | FinalOverride
  priority: u8,            // 仅同层内抢占
  preempt: bool,
  cooldown_ms: u32,
  persistent: bool,        // 常驻：被清后自动重放
  lip_safe: bool,          // 恒 true：通道白名单不含口型
}
struct Channel { semantic: SemanticKey, scale: f32, min: f32, max: f32, default: f32 }
// SemanticKey = HeadX/Y/Z | GazeX/Y | BrowY | BrowAngle | BrowForm | Breath | BodyX/Y/Z | Cheek
```

- 外置 JSON 表仍是**用户可改**的那份；语义键在加载时校验，未知键 → 丢该通道 + warn（不失败）。
- 这样换皮套 / 缺参数时**动作定义不用改**，只是可用通道变少。

---

## 2. 按钮调试（具体清单）

### 2.1 放哪儿

设置 → **开发工具/诊断** 里新增「**动作调试**」分区（`dev_mode` 可见），**主界面不放调试按钮**。
所有按钮都走**前端直发** `preset` 帧（`Live2DStage.applyPreset(id)` 已存在），**不经过后端、不经过 LLM** ——
这样调试链路与产品链路解耦，离线也能点。

### 2.2 按钮清单（P0）

| 按钮 | 行为 | 走哪条链路 | 期望结果 |
|---|---|---|---|
| `expr_smile` / `expr_sad` / `expr_angry` / `expr_surprised` | 触发对应表情 | 前端 → bridge `preset` → 渲染面 | 表情保持 `duration_ms` 后整批撤销 |
| `nod` / `shake` / `look_left` / `look_right` | 触发短动作 | 同上 | 正弦包络一次，首末为 0（不跳变） |
| **全部扫一遍** | 每 600ms 依次触发 8 条 | 同上 | 肉眼验收「每条都能演、都能撤销」 |
| **归零 / 停止** | 立即撤销当前预设 | `{id:"none"}`（**新语义**） | 参数回到 idle/默认；HUD `preset:` 变 `-` |
| **参数探针** | 渲染面回报模型有哪些允许通道参数 | 渲染面 → 新 ack 帧 | 列出「本皮套能演 / 不能演」的参数，解释 no-op |
| **当前状态** | 显示 active preset、来源（debug/director）、剩余 ms | 前端本地 + HUD | 一眼看出「谁在演、还剩多久」 |
| **模拟导演 cue** | 手填 `sentence_seq` + 选 preset → 走 cue 队列 | 前端本地 cue 队列 | 验证「按句同步」而不需要 LLM/TTS |

**P1/P2 追加**：强度滑条、时长滑条、`fade` 开关、`persistent` 勾选、「导出当前参数快照」。

### 2.3 需要的最小改动

1. `preset` 消息扩展：`{id, intensity?, ttl_ms?, priority?, source?}`，并让 `id:"none"` = 立即撤销全部本通道参数
   （现状 unknown 一律忽略——`preset.rs:124-127`——所以 `none` 今天不会撤销，**要新增**）。
2. 调试面板 UI（`settings/sections/dev_tools_section.dart` 里加一块）。
3. 「参数探针」需要渲染面回一条 ack（新协议消息 `preset-ack`，或复用现有 stage-ack 加字段）。
4. **无后端改动**即可完成 P0。

### 2.4 前端接线草图（P0）

```dart
// settings/sections/dev_tools_section.dart 的 DeveloperSection 里新增「动作调试」块
// 注意：该 Section 拿不到舞台 State，所以由 main.dart（经 shell 回调）注入一个 onApplyPreset。
const kDebugPresets = <String>[
  'expr_smile','expr_sad','expr_angry','expr_surprised',
  'nod','shake','look_left','look_right',
];

Wrap(spacing: 8, children: [
  for (final id in kDebugPresets)
    OutlinedButton(onPressed: saving ? null : () => widget.onApplyPreset?.call(id), child: Text(id)),
  TextButton(onPressed: () => _sweep(), child: const Text('全部扫一遍')),
  TextButton(onPressed: () => widget.onApplyPreset?.call('none'), child: const Text('归零')),
]);

// main.dart 注入：
DevToolsSection(
  // …
  onApplyPreset: (id) => unawaited(_stageKey.currentState?.applyPreset(id) ?? Future.value()),
);
```

- 「全部扫一遍」用 `Timer.periodic(600ms)` 逐条调用，最后一条后自动 `none` 归零。
- **状态显示**：`live2d_stage.dart` 可暴露一个 `ValueNotifier<PresetStatus>`（active id + 来源 + 剩余 ms），
  调试块读它即可；渲染面 HUD 的 `preset:` 字段已经是现成的第二证据。
- **参数探针**：渲染面加载模型后把「哪些允许通道参数存在」记下来，随 `stage-ack` 一起回一次；
  调试块显示三列（模型有 / 模型无 / 本表用到），用来解释「为什么点了没反应」。

---

## 3. 导演异步 LLM 链路（TTS 输入 ↔ 动作同步）

> ⚠️ **这是对现有范围的一次明确反转**：director 现在明文写「**不做第二路 LLM 分类**」
> （`presets.rs:11`、`director-mod-v0.md`），红线是「零投递」。本轮新增异步第二路 LLM 与真正的动作下行，
> **必须同步改**：`AGENTS.md` 的 director 台账、`core-chain-baseline.md`、`director-mod-v0.md`、
> 以及 `presets_stay_inside_the_allowed_channel` 之类回归的措辞。

### 3.1 为什么「异步旁路 + 规则兜底」是唯一不伤延迟的形态

调研三条硬事实：
- **第二路 LLM 必须小/快且异步**：一个「Dual-LLM 呈现层」先例里，一次请求 38s 中有 **25s 花在呈现生成**；
  而「隐形路由」先例用 GPT-4o 每次多 **400–1200ms**，换 Groq llama-3.3-70b 后中位 **~55ms**、在专家模型首 token 前返回。
- **第二路失败必须回退到第一路已有产物**（上例：presentation LLM 失败就直接回吐主链内容）。
- **同类已有人这么做**：N.E.K.O 的 LLM→Live2D **不是内联标签**，而是独立情绪层 ——
  整段回复 → `POST /api/emotion/analysis`（前端 **5s 超时**）→ 归一到 5 档 → `setEmotion`；
  无 motion 素材时回退 `simpleMotion`（参数直写）。这既是「异步旁路」的先例，也说明
  **超时必须短于用户感知**、且**必须有参数回退**。
- **驱动层必须自备「没有数据也能动」的兜底**（openhuman 的 viseme 三级降级）。

结论：**规则（同步、零延迟、永不失败）是主干；异步二路 LLM 是覆盖；两者都失效时等于没有导演，主链一字不改。**

### 3.2 同步机制选型（含优先级）

| 方案 | 延迟 | 污染主 LLM | 契合分句 TTS | 失败降级 | 成本 | 优先级 |
|---|---|---|---|---|---|---|
| 1 内联标签（主 LLM 出 `[joy]`） | 0 | **高**（改输出契约、标签表要进提示词、易被念出） | 好（随句传递） | 无标签=无表情 | 低 | 80（**仅预留解析器，默认关**） |
| **2 异步二路 LLM** | 后台 55ms–1.2s | **无** | 需按 `sentence_seq` 对齐 | 回退规则 | 每轮 ≤3 次调用 | 40（**默认关，配了端点才开**） |
| **3 规则/词表（现状 `derive`）** | 0 | 无 | 同帧同句 | 本身即兜底 | 0 | **10（唯一不可失败路径，常开）** |
| 4 TTS 音频驱动（viseme/韵律） | 依赖整段/时间轴 | 无 | **冲突**（一句一单元拿不到全音频） | 无时间轴即失效 | 高 | **不做** |
| 5 混合（= 3 + 2 + 1 预留） | 规则 0 + 二路后台 | 无 | 好 | 三级 | 低 | **选它** |

- **方案 4 明确不做**：本机 TTS 只回 PCM（无 viseme/word-boundary；OpenAI TTS 也没有），
  Live2D Motion Sync 是**整段/离线**方案，与「一句一单元」冲突；Azure 那条路还明确警告
  「viseme 事件快于播放，调用方必须自己按 `audio.currentTime` 重排」。**口型维持现状（RMS 包络 + 播放时钟）**。
- **方案 1 只预留**：本仓库刚拆掉 LLM 工具层，不宜再改主 LLM 输出契约。

### 3.3 触发与输入

- **触发点**：不在每句都调 LLM。**首句被分句器提交时发一次**，之后仅当
  「已生成句数 > 上份 plan 的 `covers_upto_seq`」且距上次 ≥ `min_interval_ms`（建议 **1200ms**）再发，
  **每轮上限 3 次**。这样既有「边生成边演」，又不会每句一次往返。
- **输入**：用户正文 + 助手正文（**已过确定性清洗的 TTS 文本**）+ 本模型能力集（`preset_allowlist`）；
  **绝不含 `reasoning_content`**（仓库纪律：思考不进句子装配器、不进 TTS）。
- **需要的宿主能力（当前缺）**：`DialogueEvent::SentenceReady` 目前在引擎内部被直接消费去 push TTS job
  （`conversation/engine.rs:276-287`），**从不离开引擎**。所以要加
  `EngineEvent::SentenceReady { epoch, ts_ms, sentence_seq, text }`（发在 `push_job` 之前）
  → `supervisor/handlers.rs` → Mod 事件桥 → director。
  **为什么这个锚点最好**：它发在 TTS 合成之前，而该句 TTS 合成（0.5–2s）就是二路的天然预算窗口；
  等到 `SentenceVoiced` 再发就晚了半拍（那时音频已可播）。

### 3.4 输出契约（**plan 而不是逐句 cue**）

```json
{ "epoch": 42, "covers_upto_seq": 3,
  "cues": [ { "sentence_seq": 1, "preset_id": "nod", "intensity": 2, "ttl_ms": 1800 } ] }
```

- **`epoch` 不等于当前轮 → 整份丢弃**（`/stop`、新一轮、换会话都会换 epoch）；
- `preset_id` 不在能力集 → 丢该条（不报错）；`intensity` 钳 1–3；`ttl_ms` 钳 ≤5000；
- **`priority` 由应用层写死（规则 10 / 异步 40 / 标签 80），LLM 不得自报**；
- `sentence_seq` 与 `epoch` **完全复用引擎既有字段**（与 `AudioChunk.sentence_seq`、WS 音频帧同源）——
  **导演不产生新序号，也绝不产生句子边界**；`covers_upto_seq` 是这份 plan 的效力终点，
  超出部分交回规则（这就是两层的交接面）。

### 3.5 「管理 TTS 输入内容」的边界：**导演是备注，不是誊写员**

| 能做（确定性、不阻塞） | 不做 |
|---|---|
| 剥括号动作描述（`（…）` / `*…*`）与 Markdown 标记 | **改写 / 缩写 / 重排句子**（会「说了用户没看到的话」） |
| 标点归一与停顿（换行 → `…`） | 增删语义 |
| 数字 / 单位读音表 | 改句读边界（句边界是引擎契约） |
| 多音字**词表覆盖**（给读音提示，不改字面） | **让 TTS 等导演**——送 TTS 的文本永远由确定性清洗产出 |

一句话：**拔掉导演，TTS 照说原话。**

### 3.6 时序与降级

**锚点选「该句音频 `first_chunk`（WS `start` 帧）」**，而不是句子提交时刻——否则慢 TTS 会让动作先于声音几秒
（Azure 那条明确警告「事件快于播放」）。可选再加一个前置锚点 `SentenceQueued`（带 `lead_ms`）供长动作预起手。

| 情况 | 处置 |
|---|---|
| cue 在 `start` 前到达 | 入持有队列，等 `start` 应用（带淡入） |
| 迟到、该句仍在播 | `priority` 高且剩余 > `ttl_ms` → **补演**（淡入，不重播）；否则丢弃 |
| 迟到、该句已结束 | **丢弃** + 计数 `cue_dropped_late`（不回头、不追帧） |
| 二路超时（≤1500ms）/ 失败 / JSON 坏 | 静默回退规则，置 `director_degraded` |
| `epoch` 变化 / `/stop` | 取消在飞请求，清持有队列 |
| TTS mock / 离线 / 无端点 | 规则独立跑；仲裁层是纯函数，单测不触网 |
| 未配置导演端点 | 功能整体关闭（**本地优先默认关**） |

### 3.7 实现骨架（建议）

```text
crates/live2d-ai-mod-director/src/
  decision.rs   现状：规则 derive（**保持纯函数**，兜底路径）
  presets.rs    现状：emotion|intent → preset_id（改为读外置表，见 §1.4）
  plan.rs       新：DirectorPlan / Cue / parse_plan（严格校验 + 钳位 + 丢未知）
  staging.rs    新：异步 LLM 客户端（OpenAI 兼容，独立 base_url/model/timeout；复用 secrets::lookup）
  arbiter.rs    新：纯仲裁（按 seq 持有、epoch 校验、规则/plan/标签三级优先）
  lib.rs        on_scoped_event(SentenceReady) → 触发/节流 → 后台 worker → 产出 plan → WS `action_cue`
crates/live2d-ai-mod-system/src/topics.rs    新：ModEventTopic::SentenceReady
crates/live2d-ai-desktop/src/supervisor.rs   新：把 EngineEvent::SentenceReady 投影给 Mod
crates/live2d-ai-desktop/src/web_api/ws/     新：WS 帧 action_cue（版本化，缺省忽略=向后兼容）
shell/flutter/lib/main.dart                  前端：按 seq 收 cue、在 AudioEvent{start} 时 applyPreset
```

> 仲裁放在 **director crate 内（纯函数）**而不是 core：既保持「Mod 边界」，又能用现有
> `cargo test` 直接测（不需要起 wasm/GPU）。网络只在 `staging.rs`，`arbiter`/`plan` 零 IO。
>
> 备注：仓库里 `live2d-ai-core/src/action/rules.rs` 的确定性规则与 `performance` 子系统仍**休眠保留**，
> 可作为规则层底座**参考**；若复用，只当它是一个纯函数库，**不复活 core 的动作仲裁通道**（那条线按 rc.2 已整体拆除）。

**回归三条**（必须能离线跑）：
1. 导演 100% 失败时，规则 cue 与「没有导演」逐字段相同；
2. `epoch` 不匹配的 plan **不产生任何状态变更**；
3. TTS 全 503 时动作仍沿规则路径推进，且送 TTS 的文本**不含任何导演产物**。

**trace 计时点**（进 `state_json`，复用引擎的 `ts_ms` 单调基准）：
`director.fire_ms` / `first_byte_ms` / `done_ms` / `cue.arrived_before_start_ms` /
`cue.applied_seq` / `cue_dropped_late` / `director_degraded`。

## 4. 主页语音 UI（开关 + 按住说话 + 唤醒词开关）

### 4.1 现状（WIP，在 `mod/l1-product` 的**未提交**文件里）

- `lib/voice/{voice_listen_controller,speech_recognizer,speech_recognizer_stub,speech_recognizer_web}.dart`、
  `lib/api/voice_api.dart`、`lib/settings/mods/voice_input_panel.dart` + 3 个 Dart 测试（含
  `voice_wake_default_consistency_test.dart` 与 Rust `DEFAULT_WAKE_PHRASE` 对账）。
- 主界面已有「**听**」按钮 + 状态行 + 可读错误；控制器只支持**常驻唤醒**（`toggle()`），**没有 PTT、没有模式开关**。
- **闸门事实（本轮最关键的一条）**：`gate::evaluate` 的顺序是
  `manual_enabled → wake_phrase 非空 → 文本必须**包含**唤醒词 → 剥词`；
  否则回 `400 wake_phrase_required`。**所以 PTT 若发裸正文会被拒**——
  WIP 的做法是发「**唤醒词+正文**」原文（`_sendNow('\$_wakePhrase\$_body')`），由服务端剥词。
- Web Speech 事实：**Chrome 默认把音频送云端**（MDN 原文，离线不可用；`processLocally` 仍是实验特性）；
  `http://127.0.0.1:18080` 属 secure context **免 HTTPS**；Firefox 仅 142+ 开偏好后支持 →
  文案写「桌面 Chrome/Edge」是对的。

### 4.2 交互规格（三态，一个按钮）

| 操作 | 行为 |
|---|---|
| **点按**（<150ms 松手） | 常驻唤醒开/关（与现有 WIP「听/停」同义，零学习成本） |
| **按住**（≥150ms） | PTT：按下开始、松手提交；**不要求唤醒词**（见 §4.3 的两种接法） |
| 设置「语音默认行为」 | 关 / 按住说话 / 常驻唤醒 —— 决定进入页面时的初始态 |

- 状态用文字 + 颜色：关=灰；按住=「聆听中…」；常驻=呼吸态 +「在听：说「小可爱……」」。
- **唤醒词加固**：建议「唤醒词须在**句首/独立成句**」才进入 armed（现状是**任意位置子串命中**，
  「我昨天说小可爱好看」会误触发）；**播报期间暂停唤醒**（AI 自己的声音/环境人声会误触发，回声消除不够）。
- **busy 协同**（`say` 通道 cap 1，满了回 200+`ok:false`）：**不排队**（排队只会让语音越来越滞后）；
  提示「角色还在说话」，并把识别结果**落到输入框**让用户改字重发——比静默丢弃好。
- **诚实性**：Web Speech 模式下 UI 必须明说「**此模式需联网、音频会出本机**」（P1 的本地 ASR 才离线）。

### 4.3 PTT 与唤醒闸的两种接法（要选一个）

| 接法 | 改动 | 优点 | 缺点 |
|---|---|---|---|
| **(a) 零后端改动**：PTT 也发「唤醒词+正文」 | 无 | 今天就能用，闸门仍是唯一真源 | 语义别扭；日志/输入框里出现用户没说的唤醒词 |
| **(b) 推荐（P1）**：端点加显式 `ptt: true`（或 Mod config `ptt_bypass_wake`，默认开），gate 对该请求**跳过唤醒匹配** | voice_routes + gate + Mod | 语义诚实；manual 闸与 token 校验不变 | 一次契约小改 + 回归 |

> 无论哪种：**manual 闸、token、长度、清洗都不变**；变的只有「是否要求文本包含唤醒词」。

### 4.4 选型（ASR × 唤醒 × PTT）

| 阶段 | ASR 通道 | 唤醒词 | PTT |
|---|---|---|---|
| **P0 最小可用** | Web Speech（**云**） | 文本级：在 Web Speech 定稿里找「小可爱」（现有 WIP） | Web Speech `start/stop`，发含唤醒词原文 (a) |
| **P1 离线化** | 新增「上传音频→本地 ASR」端点（sherpa-onnx / whisper.cpp / Vosk） | 仍文本级 | `MediaRecorder`→Blob 或 `AudioWorklet`→16k WAV（与项目 TTS 同款零依赖 WAV） |
| **P2 唤醒词本地化** | 同 P1 | **openWakeWord（Python sidecar，最省事）或 sherpa-onnx KWS（有 Rust/WASM/Dart 绑定）** | 同 P1 |

- **不选 Porcupine**（免费层已关停、要 AccessKey、需联网校验）；**不选 TF.js speech-commands**
  （默认英文词表，中文「小可爱」要自训）；Vosk ~50MB 可作低配备选。
- 要「中文唤醒 + 离线 + 无密钥」，实际只剩 **openWakeWord** 或 **sherpa-onnx KWS**。
- 红线复核：不引外部 CDN ✅（模型随包/随 sidecar 本地加载）；但 **P0 达不到「离线也能用」**，
  离线路径是 P1/P2 —— UI 必须如实说，别让用户以为本地可用。

### 4.5 状态机与接线草图（P0）

```text
PTT:      idle →(按下,权限OK) listening →(松手) recognizing →(非空) submitted → idle
失败：permissionDenied(终止,不自动重试) / noDevice / tooShort(<300ms或RMS≈0,丢弃+提示) /
      timeout(5s) / HTTP 非 200 按 code 显示 / ok:false busy(不排队,落输入框)
唤醒:     listening →(定稿含「小可爱」) armed(窗口8s,只收定稿) →(有正文) submit → listening
```

```dart
// voice_listen_controller.dart：复用同一个 recognizer
Future<void> pressStart()   async { /* 不要求唤醒词；开始识别 */ }
Future<void> pressRelease() async { /* 停 → 等 ~300ms 定稿 → 提交 */ }

// 提交文本：P0 = '$_wakePhrase$body'（服务端剥词）；P1 = body + {ptt:true}
```

```dart
// chat_panel.dart 的麦克风按钮
GestureDetector(
  onTap:            () => onToggle(),            // 常驻开/关
  onLongPressStart: (_) => onPressStart(),       // PTT
  onLongPressEnd:   (_) => onPressRelease(),
  child: MicButton(listening: listening, armed: armed),
);
```

- **松手判定**：`onLongPressStart` 到 `onLongPressEnd` <150ms 视为点按（走 toggle）；
  实现上可用 `onTapDown/onTapUp` + 计时器更可控。
- **Web Speech 的 `stop()` 语义坑**：Chrome 上 `stop()` 尝试交出最后一次定稿但不保证立刻到；
  `abort()` 会**丢弃**结果——`pressRelease()` 必须等一个短窗口（~300ms）拿定稿，
  超时用最后的 interim（UI 标注「可能不完整」），**不要用 `abort()`**。
- 单测：`shell/flutter/test/voice_listen_controller_test.dart`（现有）+ PTT 迁移表/唤醒窗口/各 code→文案。

## 5. 任务清单（建议顺序）

| 优先级 | 任务 | 文件 | 量 |
|---|---|---|---|
| **P0-1** | preset 支持 `none` 撤销 + `intensity/ttl/source` | `l2d-wasm-demo/src/main.rs`、`preset.rs` | S |
| **P0-2** | 动作改走 `final_override`，修 idle 盖表情 | `web/surface/input.rs` | S |
| **P0-3** | 动作调试面板（8 按钮 + 扫一遍 + 归零 + 状态） | `dev_tools_section.dart`、`live2d_stage.dart` | M |
| **P0-4** | 语音 PTT + 模式开关 | `voice_listen_controller.dart`、`chat_panel.dart`、`main.dart` | M |
| **P1-1** | 外置 preset 表（JSON）+ 校验/回归 | `assets/actions/`、`preset.rs` | M |
| **P1-2** | 新事件 `SentenceReady`（topics + supervisor 投影） | `topics.rs`、`supervisor.rs` | M |
| **P1-3** | 异步 staging LLM + cue 队列 + WS `action_cue` | director crate、`ws/` | L |
| **P1-4** | 前端按句应用 cue + 模拟 cue 调试按钮 | `main.dart`、`live2d_bridge.dart` | M |
| **P1-5** | 单会话记忆 P0（worker 化 + 尾部读 + 缓存 + 删全局降级） | memory crate | M |
| **P2-1** | 本地 ASR 端点（离线） | voice routes / sidecar | L |
| **P2-2** | exp3 加载器（可选增强） | `l2d` | L |
| **P2-3** | 本地唤醒词（sherpa KWS/openWakeWord） | voice 链路 | L |
| **P2-4** | 记忆 P1（后台抽取 + 摘要层） | memory crate | M |

---

## 6. 同类实现调研来源（已核实部分）

**记忆 / 会话模型**（详见 §0）：
[SillyTavern 上下文模板](https://docs.sillytavern.app/usage/prompts/context-template/) ·
[Prompt Manager](https://docs.sillytavern.app/usage/prompts/prompt-manager/) ·
[World Info](https://docs.sillytavern.app/usage/core-concepts/worldinfo/) ·
[Summarize](https://docs.sillytavern.app/extensions/summarize/) ·
[Group Chats](https://docs.sillytavern.app/usage/core-concepts/groupchats/) ·
[Agnai Memory Books](https://agnai.guide/docs/memory/memory-books) ·
[RisuAI](https://github.com/kwaroran/RisuAI) ·
[AstrBot 上下文压缩](https://docs.astrbot.app/use/context-compress.html) ·
[AstrBot conversation_mgr.py](https://github.com/AstrBotDevs/AstrBot/blob/master/astrbot/core/conversation_mgr.py) ·
[AstrBot compressor.py](https://github.com/AstrBotDevs/AstrBot/blob/master/astrbot/core/agent/context/compressor.py) ·
[LibreChat Fork](https://www.librechat.ai/docs/features/fork) ·
[Open WebUI Memory](https://docs.openwebui.com/features/chat-conversations/memory/) ·
[LobeChat Topics](https://lobehub.com/docs/usage/agent/topic) ·
[Chronicler ADR-002](https://github.com/yantrikos/chronicler/blob/main/docs/ADR-002-memory-conventions.md) ·
[Serene Pub context-configs](https://github.com/doolijb/serene-pub/blob/main/docs/context-configs.md)

**TTS ↔ 动作同步 / 异步导演**（详见 §3）：
[Open-LLM-VTuber live2d_model.py](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/blob/main/src/open_llm_vtuber/live2d_model.py) ·
[sentence_divider.py](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/blob/main/src/open_llm_vtuber/utils/sentence_divider.py) ·
[VTube Studio API](https://github.com/DenchiSoft/VTubeStudio) ·
[Live2D Motion Sync 手册](https://docs.live2d.com/zh-CHS/cubism-editor-manual/motion-sync/) ·
[Azure TTS viseme/word boundary](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/how-to-speech-synthesis) ·
[pipecat #1516（viseme 落后于播放）](https://github.com/pipecat-ai/pipecat/issues/1516) ·
[llm-agent Dual-LLM Presentation Stage](https://github.com/fr0ster/llm-agent/blob/main/docs/superpowers/specs/2026-03-28-dual-llm-presentation-design.md) ·
[Invisible Orchestrator（55ms vs 400–1200ms）](https://dev.to/pavelbuild/the-invisible-orchestrator-cheap-routing-expensive-reasoning-in-multi-agent-apps-51h0) ·
[openhuman viseme 三级降级](https://github.com/tinyhumansai/openhuman/pull/1160) ·
[LIA-Assistant sentence_streamer.py](https://github.com/jgouviergmail/LIA-Assistant/blob/main/apps/api/src/domains/voice/sentence_streamer.py) ·
[hermes-agent #96927（首句被缓冲）](https://github.com/NousResearch/hermes-agent/issues/96927)

**主页语音 / PTT / 唤醒词**（详见 §4）：
[MDN SpeechRecognition](https://developer.mozilla.org/en-US/docs/Web/API/SpeechRecognition) ·
[MDN Using Web Speech API（Chrome 送云端）](https://developer.mozilla.org/en-US/docs/Web/API/Web_Speech_API/Using_the_Web_Speech_API) ·
[MDN processLocally（实验性离线）](https://developer.mozilla.org/en-US/docs/Web/API/SpeechRecognition/processLocally) ·
[web-speech-api #137（continuous 仍 onend）](https://github.com/WebAudio/web-speech-api/issues/137) ·
[MDN Secure Contexts（localhost 免 HTTPS）](https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Secure_Contexts) ·
[Permissions-Policy: microphone](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Permissions-Policy/microphone) ·
[MDN MediaRecorder](https://developer.mozilla.org/en-US/docs/Web/API/MediaRecorder) ·
[MDN AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/AudioWorklet) ·
[record 平台矩阵](https://github.com/llfbandit/record/blob/master/record/README.md) ·
[openWakeWord](https://github.com/dscripka/openWakeWord) ·
[sherpa-onnx KWS](https://k2-fsa.github.io/sherpa/onnx/kws/index.html) ·
[Porcupine 免费层关停](https://community.home-assistant.io/t/porcupine-free-tier-shutdown-alternatives-for-home-assistant-voice-users/1012382) ·
[TF.js speech-commands](https://github.com/tensorflow/tfjs-models/tree/master/speech-commands) ·
[Vosk](https://github.com/alphacep/vosk-api) ·
[Open-LLM-VTuber（改送音频到后端 ASR + VAD）](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) ·
[SillyTavern Speech Recognition 扩展](https://github.com/SillyTavern/Extension-Speech-Recognition) ·
[Live2DPet（Electron + 本地 Whisper）](https://github.com/dwgx/Live2DPet)

**动作系统 / Live2D 标准皮套**（详见 §1）：
[N.E.K.O](https://github.com/Project-N-E-K-O/N.E.K.O)（`docs/frontend/live2d.md`、`architecture/tts-pipeline.md`、
`main_routers/live2d_router.py`、`static/live2d/live2d-emotion.js`） ·
[Cubism 标准参数表](https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/) ·
[motion3/exp3/physics3 规范](https://github.com/Live2D/CubismSpecs/tree/master/FileFormats) ·
[缺参数静默 no-op（cubismmodel.ts）](https://github.com/Live2D/CubismWebFramework/blob/develop/src/model/cubismmodel.ts) ·
[Motion Sync（需授权插件，非标准参数能力）](https://docs.live2d.com/en/cubism-sdk-manual/use-on-scene-motion-sync-unity/) ·
[soullink-emotion-sdk](https://github.com/nanlingyin/soullink-emotion-sdk) ·
仓库内既有研究 `docs/research/live2d-halfbody-presetfree-control.md`（在 `mod/pg-director` 分支）

> 诚实标注：LibreChat 的自动摘要策略、LobeChat topic 内历史是否全量注入，均**未找到官方明确文档**；
> N.E.K.O 的 `emotion_mapping` 一手证据未能取得（见 §1 的证据说明）。

