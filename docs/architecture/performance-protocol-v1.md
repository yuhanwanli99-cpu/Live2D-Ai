# 表演协议 v1（冻结真源，2026-09-26）

> **状态**：阶段4 契约冻结波次（**只写文档，零代码**）。分支 `mod/l1-product`，工作树 `/home/skystar/Live2D-Ai-l1`。
> **本文件 = 表演协议 v1 的唯一真源。** 与代码冲突时以**代码 + 回归**为准，并回改本文。
>
> **裁决来源**：维护者已冻结的 V1–V12 写在任务书里，事实与证据来源是
> [RESEARCH-actions-director-audit-2026-09-21.md](../plans/RESEARCH-actions-director-audit-2026-09-21.md) §8–§10.7
> （§10.7 是维护者已确认答复，§10.3/§10.4 是维护者采纳的推荐）。**本文件不得重新论证 V1–V12**；
> 有异议 → 停下问维护者。
>
> **本波红线**：零代码改动；不动 `topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs`；
> 不碰其它 worktree；密钥 / `.env` / 请求体不进文档。

---

## 0. 一句话

**主模型说原文；表演层只做两件事：把原文切分 `segments`（逐字不变），并给三族语义表演字段
`body` / `head` / `expression`。** 字段经**每皮套的映射表**（人工标定）落到模型参数；
编排锚在**音频播放时钟**上；渲染面用**事件级 ack** 回执；角色 baseline 绑会话。

v1 相对 v0 的**唯一语义退役**：`speak`（可改写说辞）→ `segments`（只切分、不改写，V1）。
**一帧都不删、不新增字段名到既有 WS 帧**（V11）。

---

## 1. 分工（V12：说话权归属不在本波裁决）

| 层 | 做什么 | 不做什么 |
| --- | --- | --- |
| **主模型**（`[llm]`，酒馆式） | 角色扮演；出剧情正文（含思考） | 不暴露工具；不写表演 JSON；不写 `ParamXxx` |
| **表演层**（`[performance]`，独立端点） | 交回**一份** v1 JSON：`segments`（切分原文）+ `cues`（三字段表演） | 不改写原文；不产 TTS 参数；不直接写模型参数 |
| **director Mod `staging_*`**（Mod） | **只产 cues**（`action_cue`），不产送 TTS 的文本 | 不产 segments / speak |
| **规则导演**（`presets::rule_cues_for_text`） | 仅失败回退：一条规则 cue | 同上 |
| **确定性清洗**（`clean_for_tts`） | 任何送 TTS 的文本都过它（只拆标记、不改句界） | 不改写说辞、不改句边界 |
| **渲染面**（`l2d-wasm-demo`） | 参数真源；字段 → 参数；发事件级 ack | 不接受每帧状态流；不被前端镜像 |

**文档事实（V12，不得再论证）**：

- **当前事实 = 表演层产 `segments` + `cues`；director staging 只产 `cues`。**
- **「说话权」归属（主模型说、导演只演；还是导演是说 + 演的演员）是 Q1 遗留问题，
  不在本波裁决、本文件不裁决**（RESEARCH §3.7 Q1 / §3.8）。v0 的 `speak` 能力**保留可解析**
  （V11），但**新计划一律写 `segments`**。

---

## 2. schema（v1）

### 2.1 顶层形状

```jsonc
{
  // 原文的切分方案：只能【切分原文】，逐字不变（V1）。
  // 注意：segments 不是上屏字符串 —— 上屏 == 送 TTS == clean_for_tts(段)（D23）
  "segments": ["嗯……", "我想到了。"],
  // 表演字段：同类可【重复出现】，按 add 合成；立即生效、不排队（V2）
  "cues": [
    { "field": "body",       "x": 0.0, "y": 0.30,  "intensity": 1, "at": "now",   "hold": true  },
    { "field": "head",       "x": 0.0, "y": -0.40, "intensity": 2, "at": "seg:2", "hold": false },
    { "field": "expression", "id": "thinking",  "intensity": 1, "at": "now",   "hold": true  },
    { "field": "expression", "id": "surprised", "intensity": 2, "at": "seg:2", "hold": false }
  ]
}
```

- `segments`：**必填**（`speak` 缺席时）。数组，元素为 string。切分不变量见 §3。
- `cues`：**必填**。数组。**`[]` = 本轮不动**（不是撤销；撤销的哨兵见 §11）。
- **未知顶层键一律丢弃**（宽容，沿用 v0 §2.1；V11「前端遇未知字段忽略」同口径）。
- 发给 provider 的 `json_schema` **必须由同一组常量拼出**（v0 的
  `schema_and_validator_share_the_same_bounds` 纪律沿用）。

### 2.2 字段表

| 键 | 类型 | 取值 | 必填 | 语义 |
| --- | --- | --- | --- | --- |
| `segments` | string[] | 见 §3 | `speak` 缺席时**必填** | 原文的切分方案；拼接 == 主模型原文 |
| `cues[].field` | enum | `body` / `head` / `expression` | 是 | 三选一（V2） |
| `cues[].x` | number | `[-1, 1]` | `body`/`head` 可选 | 轴值；多轴同给 = 按比例合成 |
| `cues[].y` | number | `[-1, 1]` | `body`/`head` 可选 | 轴值 |
| `cues[].z` | number | `[-1, 1]` | **仅 `head`** 可选 | 歪头倾斜；`body`/`expression` 给 `z` → 丢弃该键并 warn |
| `cues[].id` | string | 表情面板 id | **仅 `expression`** 必填 | 五官表情（V3）；未知 id → 丢该条 + warn |
| `cues[].intensity` | integer | `1` / `2` / `3` | 是 | 轻微 / 中 / 强（V4） |
| `cues[].at` | enum | `now` / `seg:N` / `after_prev` | 是 | 锚点（V6） |
| `cues[].hold` | bool | `true` / `false` | 是 | `true` 保持到下次指令；`false` 按 ttl 到点回（V5） |
| `cues[].ttl_ms` | number | `(0, 5000]` | 可选 | **仅 `hold=false` 有意义**；省略 = 该字段默认时长（见 §2.5） |
| `speak` | string\\|null | 任意 | 可选（**弃用**） | v0 旧语义保留（V11）；`segments` 缺席时走旧路径 |

**轴的方向由映射表定义**（§5.2）：LLM 只写归一化轴值，正负方向、量程、符号翻转都在表里。

### 2.3 合法样例

**A. 只说（纯文本切分，不动）**

```json
{ "segments": ["下午好，", "今天天气不错。"], "cues": [] }
```

**B. noop（不说也不动，合法）**

```json
{ "segments": [], "cues": [] }
```

**C. 只动（无文本，靠无声锚句）**

```json
{ "segments": [], "cues": [
  { "field": "expression", "id": "thinking", "intensity": 1, "at": "now", "hold": true }
] }
```

**D. 提线木偶（切分当锚点 + 立即生效 + hold）**

```jsonc
{
  "segments": ["嗯……", "我想到了。"],
  "cues": [
    { "field": "body",       "x": 0.0, "y": 0.30,  "intensity": 1, "at": "now",   "hold": true  },
    { "field": "expression", "id": "thinking",     "intensity": 1, "at": "now",   "hold": true  },
    { "field": "head",       "x": 0.0, "y": -0.40, "intensity": 2, "at": "seg:2", "hold": false },
    { "field": "expression", "id": "surprised",    "intensity": 2, "at": "seg:2", "hold": false }
  ]
}
```

**E. 同类相加（两条 body 按 add 合成；`after_prev` 串接）**

```json
{ "segments": ["好。"], "cues": [
  { "field": "body", "x": -0.30, "y": 0.10, "intensity": 1, "at": "now",        "hold": false },
  { "field": "body", "x":  0.45, "y": 0.20, "intensity": 2, "at": "after_prev", "hold": false }
] }
```

### 2.4 非法样例表

| # | 非法样例 | 违反 | 处置 | 错误码 |
| --- | --- | --- | --- | --- |
| 1 | 非 JSON 文本 | `NotJson` | **整份失败 → 回退** | `performance_plan_not_json` |
| 2 | `[]` / `"x"`（顶层非对象） | `NotObject` | 整份失败 | `performance_plan_not_object` |
| 3 | `{}`（无 `segments` 也无 `speak`） | `MissingSegments` | 整份失败 | `performance_plan_missing_segments` |
| 4 | `{"segments":"嗯", "cues":[]}` | `SegmentsType` | 整份失败 | `performance_plan_segments_type` |
| 5 | `{"segments":["嗯", 7], "cues":[]}` | `SegmentNotString` | 整份失败 | `performance_plan_segment_not_string` |
| 6 | **`segments.concat() != 原文`**（改写 / 缩写 / 增删 / 换序） | `SegmentsNotPartition`（V1 核心不变量） | 整份失败 | `performance_plan_segments_not_partition` |
| 7 | 缺 `cues` | `MissingCues` | 整份失败 | `performance_plan_missing_cues` |
| 8 | `cues` 不是数组 / 某条不是对象 | `CuesType` / `CueNotObject` | 整份失败 | `performance_plan_cues_type` / `_cue_not_object` |
| 9 | cue 缺 `field`/`intensity`/`at`/`hold`，或类型不对 | `CueField` | 整份失败 | `performance_plan_cue_field` |
| 10 | `"field":"tail"`（词表外） | `UnknownField` | 整份失败 | `performance_plan_unknown_field` |
| 11 | `body` 给 `z` | `AxisNotAllowed` | **丢该键 + warn**（宽容；不整份失败） | `performance_axis_not_allowed` |
| 12 | 轴值 `1.7` | 越界 | **钳位**到 `[-1,1]` | （无错误码） |
| 13 | `"intensity":5` | 越界 | **钳位**到 `1..=3` | （无错误码） |
| 14 | `"at":"seg:0"` / `"at":"in:800"` | `at` 非法 | 整份失败 | `performance_plan_bad_anchor` |
| 15 | `"at":"seg:99"`（超出 `segments` 长度） | 越界锚点 | **整份失败**（不是钳位：钳到哪一段都是猜） | `performance_plan_bad_anchor` |
| 16 | `"id":"none"` 之外的未知表情 id | 能力集外 | **丢该条 cue + warn**（沿用 v0 宽松：整份失败会连带丢语音） | `performance_expression_unknown_id` |
| 17 | cue 条数 > `MAX_CUES` | `TooManyCues` | 整份失败 | `performance_plan_too_many_cues` |
| 18 | `segments` 总字符 > `MAX_SEGMENT_CHARS` | 超长 | 整份失败（**不截断**：截断会破坏 V1 拼接不变量） | `performance_plan_segments_too_long` |
| 19 | 同时给 `segments` 与 `speak` | 二义 | **`segments` 优先；`speak` 忽略 + warn**（O1 已裁决，§12.1） | （无错误码） |
| 20 | 未知顶层键（`"reason":…`） | — | **丢弃**（宽容） | （无错误码） |

- `MAX_CUES` 沿用 v0 的 `16`。
- **整份失败 → 回退**：引擎用 `SentenceAssembler` 切 `clean_for_tts(原文)` 产出多段（**D22**）+ 规则 cue；
  原因码 `performance_plan_invalid` / 细分码见上表（沿用 v0 §5 回退矩阵）。
- **不局部抢救**：丢一条坏 cue、留其余会让「这一轮到底演了什么」变成不可解释的混合体
  （v0 §2.1 的既有理由，v1 沿用）。**例外只有两处**：`z` 轴不允许、未知表情 id——
  它们是「多给了 / 点名了一个不存在的面板项」，丢掉不影响原文与其余表演的可解释性。
- **错误码不含正文**（沿用 v0 §6：日志只打长度与 id 列表）。

### 2.5 默认时长（`ttl_ms` 省略时）

| `field` | 默认 `ttl_ms` | 依据 |
| --- | --- | --- |
| `body` | `900` | 现有手势包时长（`MOTION_MS`） |
| `head` | `900` | 同上 |
| `expression` | `2600` | 现有表情包时长（`EXPRESSION_MS`） |

`hold=true` 时 `ttl_ms` 被忽略（保持到下次指令，V5）。数值来源见
[action-packs-v0.md](action-packs-v0.md) §11；**是否把 `ttl_ms` 升为一等必填、以及默认值本身，
见 §12 待用户点头**。

### 2.6 三态（沿用 v0 §2.2，都是合法输出）

| 态 | 形状 | 行为 |
| --- | --- | --- |
| **noop** | `segments=[]` 且 `cues=[]` | 本轮不说也不动 |
| **只说** | `segments` 非空 且 `cues=[]` | 逐段送 TTS + 上屏 |
| **只动** | `segments=[]` 且 `cues` 非空 | 引擎给一条**无声锚句**，让 cue 有音频锚点 |

**`cues: []` = 本轮不动**；**`id:"none"`（或旧 `preset_id:"none"`）= 撤销哨兵**——
在**段/句边界**上撤销两个槽。空表与 `none` 是两个不同语义，不得互相替代（沿用 D10）。

---

## 3. `segments`：只能切分原文（V1）

### 3.1 不变量（唯一真源）

设主模型本轮原文为 `T`、表演层交回 `segments = [s₁, s₂, …, sₙ]`：

1. **逐字不变**：每个 `sᵢ` 是 `T` 的一段连续子串，中间**不得**插入、删除、替换任何字符；
2. **拼接恒等**：`s₁ + s₂ + … + sₙ == T`（**逐码点**比较，不是「差不多」）；
3. **保序**：切分点单调不减；不允许换序；
4. **不允许改写 / 缩写 / 增删语义**（这是 V1 的核心，也是 v0 `speak` 被退役的原因）；
5. **空原文**：`T` 为空 / 全空白 → `segments = []`（noop 的一半）。

> **`clean_for_tts` 在切分之后逐段应用**（只拆 Markdown / 舞台指示标记，不动句界）。
> 它改的是**送去 TTS 的字符串**，不改 `segments` 本身，也不破坏第 2 条不变量。
>
> **D23（上屏 == 送 TTS）**：第 i 段的**上屏文本 == 送 TTS 文本 == `clean_for_tts(sᵢ)`**。
> `segments` 是「切分方案」，**不是**上屏字符串；不得把含 Markdown / 舞台指示的原文直接上屏
> （那会破坏 2026-09-10「一句一单元 / 上屏与音频逐字同拍」契约）。回归 `displayed_text_equals_tts_text_per_segment`。
>
> **D22（段 ↔ 句 1:1）**：表演层给出的 `segments` **一对一**成为 TTS 单元与音频元素；
> `seg:N` ≡ `sentence_seq == N`；**不再过分句器二次切分**。回退路径（performance 关 / 失败）
> 由引擎用现有 `SentenceAssembler` 对 `clean_for_tts(原文)` 切句产出**多段**（v0 行为不变）——
> 第 1 条拼接不变量**只约束表演层提供的** segments，不约束引擎自产的段。回归 `segments_are_one_to_one_with_sentences_and_never_resplit`。

### 3.2 断句点天然是编排锚点

`seg:N` 锚的就是第 N 段音频**开始播放**的时刻（§6）。「嗯……」与「我想到了。」分成两段，
就是两次音频元素、两个锚点——**这正是「提线木偶」的实现基础**（RESEARCH §10.1 Q1 / §10.4）。

### 3.3 空白段

分句器把换行也算句读，会产生**纯空白段**。这类段：
**不发 TTS 请求**（上游会回 400，而 TTS 错误是 fatal），走既有静音段路径；
但 `segments` 数组本身**保持不变**（不为了 TTS 去改切分方案）。

**D24（空白段不跳号）**：纯空白段**仍产出**静音音频元素与 `start`/`end` 边界帧，
`seg` 编号**不跳过**——否则 `seg:N` 锚点整套错位（`seg:N` 是「第 N 段」，不是「第 N 个出声段」）。
回归 `blank_segment_keeps_its_seg_index`。

---

## 4. 表演字段（V2 / V3 / V4）

### 4.1 三个字段，语义正交（V2 / V3）

| 字段 | 管什么 | 不管什么 |
| --- | --- | --- |
| `body` | **半身**摆动 / 倾斜（ParamBodyAngle\\*） | 头、五官、口型 |
| `head` | **头部**点头 / 摇头 / 歪头（ParamAngle\\*） | 身、五官、口型 |
| `expression` | **只写五官**（眼 / 眉 / 嘴形） | **头身姿态完全交给 `body` / `head`**（V3） |

- **V3 的含义**：v0 表情包自带的小幅头身位移，在 v1 里**不再由 `expression` 携带**。
  要头身姿态就下 `body` / `head` cue（规则导演或表演层自己拆成两条）。
- **口型 `ParamMouthOpenY` 归 TTS**，三字段都**不得**写它（沿用既有红线）。

### 4.2 轴值与强度（V4）

- `intensity ∈ {1,2,3}`（轻微 / 中 / 强）；
- 轴值 `x` / `y`（`head` 另有 `z`）`∈ [-1, 1]`；
- **多轴同时给 = 按比例合成**（RESEARCH §10.7 可实现清单：两轴 / 三轴按比例合成）；
- 渲染面把轴值乘「**每皮套基础强度**」后，**仍受通道红线钳位**（§5.4）。

### 4.3 同类可重复出现，按 add 合成（V2）

- 同一 `field` 的多条 cue **相加**（add），不是后者覆盖前者，也不是相乘。
- add 的粒度是**归一化轴值 × intensity**（合成后再乘每皮套基础强度、再钳位一次）。
- **`expression` 的 add**：与 `body` / `head` **同一套语义**（O5 已裁决）——按**通道**累加，
  最后统一钳位；**不特设「后者补缺通道」规则**（那条规则不可测，且与 body/head 不一致）。

**D26（批内 add / 跨批次 replace）**：同一 plan 内、同一 `field` 的多条 cue 按各自 `at` 生效并
**相加**；**新的一批 cue 到达同一 `field`** 时结束该 field 当前的动画段（发 `preset_replaced`）、
并以新值为当前值；非 hold cue 到点移除自己的贡献（发 `preset_expired`），全无贡献 → 回基准；
`none` 撤销清空该 field 累加器。**这是「`preset_replaced`」与「同类相加」并存的定义**（§2.3 例 E 仍成立）。
回归 `new_batch_replaces_the_field_animation_value_adds`。

### 4.4 立即生效、不排队（V2）

- cue 一旦成立（`now` 立即；`seg:N` 到该段音频开始；`after_prev` 见 §6.4）**立即生效**，
  不为等音频而延迟动作，也不进动作队列；**导演自己就是调度器**（RESEARCH §10.7 Q9）。
- 「动作的发生」本身进日志，成为喂导演的输入（下一次决策的锚点）。

### 4.5 `hold`（V5）

| `hold` | 行为 |
| --- | --- |
| `true` | **保持到下次指令**，不自动回基准；`ttl_ms` 被忽略 |
| `false` | 按 `ttl_ms`（或 §2.5 默认）到点回基准 |

- `hold=true` 的 cue 到点**不会**产生 `preset_expired`（它没有「做完点」）；
  这直接影响唤醒条件（V9，§8）。

### 4.6 表情只写五官（V3）

- `expression.id` 指向的是**面板表情项**，其映射表只允许五官通道：
  `ParamMouthForm` / `ParamEyeL*Open` / `ParamEyeR*Open` / `ParamEyeL*Smile` /
  `ParamEyeR*Smile` / `ParamBrowLY` / `ParamBrowRY`（现有 13 参白名单的**五官子集**）。
- 现有三个表情包（`smile` / `unhappy` / `surprised`）的**五官部分**就是 `expression` 的
  首版映射；它们自带的头身分量**在 v1 的 `expression` 字段下不下发**（要头身就下 `body`/`head`）。
  见 [action-packs-v0.md](action-packs-v0.md) §1 与本文 §11。

---

## 5. 三字段 → 模型参数：映射原则

### 5.1 LLM **只写语义字段，不写 `ParamXxx`**（强制原则）

理由（RESEARCH §10.7 问题 1，四条：换皮套 / 语义 / 安全边界 / 人工测基础强度）——
**不在本文重述**，只记结论：

- 提示词里**不得**出现任何 `Param…` 参数名；
- 回归必须抓主模型**与表演层**的请求体原文，断言不含 `Param`（v0 已有同类回归
  `performance_never_leaks_into_the_main_model_request`，v1 扩到表演层提示词）。

### 5.2 映射表按皮套、人工标定

- **一个皮套一份映射表**：`field`（+ 轴）→ 一组模型参数（含符号、峰值、通道分类）。
- 表里同时存**轴的语义方向**（`x+` = 哪边、`y+` = 上/下、`z+` = 朝哪边倾斜）。
- **表是数据，不是提示词**：换皮套只换表，提示词不变。
- **非标准参数（翅膀 / 耳朵 / 光环 / 袖子 / 头发）本轮不做**；白名单保持现状
  （`crates/l2d-wasm-demo/src/preset/table.rs` 的 `ALLOWED_PARAMS` 13 个标准参数不动）。
  将来若要，用一个受白名单约束的 `extra` 逃生舱（RESEARCH §10.2 / §10.7）。
- **表的载体位置与「每皮套」的粒度**见 §12 待用户点头。

### 5.3 每皮套基础强度 = 现有 `[action]` 三倍率

- `body` 倍率 ↔ 字段 1、`head` 倍率 ↔ 字段 2、`expression` 倍率 ↔ 字段 3
  （RESEARCH §10.2 结构性改动 1，维护者已确认「恰好就是现有的 `[action]` 三个倍率」）。
- 现有 `[action]` 是**全局**用户可调旋钮（`head 0.75 / body 0.80 / expression 1.0`，
  量程 `[0.2, 2.2]`）；「**按皮套**」那一份住在映射表里（§5.2）。
  **两者的分工是否需要重新划，见 §12**。

### 5.4 公式与通道红线（沿用 action-packs §11.1，不另立一套）

```text
最终值 = clamp_to_channel( 归一化轴值 × 映射表峰值 × intensity × 每皮套基础强度 × 包络 )
```

- **通道上限是死的**：头 `ParamAngle*` = **30** / 身 `ParamBodyAngle*` = **10** /
  五官 = **4**；
- **数值真源**：`crates/l2d-wasm-demo/src/preset/scales.rs` 顶部注释（本节不复制它的数字，
  只声明「同一套公式」）；
- **钳位不是失败**：越界钳位 + 在 ack 里标 `clamped=true`（§7）。

### 5.5 字段 ↔ 现有通道对照（可实现清单，RESEARCH §10.7）

| 字段 | 可以做 | 做不到 / 限制 | 对应现有包 |
| --- | --- | --- | --- |
| `body` | 左右摆动（`ParamBodyAngleX`）、前后倾（`Y`）、侧倾（`Z`）；两轴/三轴按比例合成 | **形变拉伸**（Scale / Stretch）：Cubism 标准参数与本皮套都**没有**该通道 | 手势包的**身**分量 |
| `head` | 摇头左右（`ParamAngleX`）、点头上下（`Y`）、歪头倾斜（`Z`）；三轴合成 | 超过 ±30 被钳位；「拉伸脖子」做不到 | 手势包的**头**分量；表情包的小幅头 |
| `expression` | 五官面板选择（现有 3 条表情包的五官子集） | `ParamMouthOpenY` 归 TTS，不得写 | `smile` / `unhappy` / `surprised` 的五官分量 |

**可选叠加**（肩部 `ParamBodyAngleX3/X6`、胯部 `X4`、迈腿 `X5`；眼珠 `ParamEyeBallX/Y`）：
RESEARCH §10.7 写「**需你确认是否纳入**」——**本波不纳入，列入 §12 待用户点头**。

---

## 6. 锚点与音频时钟（V6 / V7）

### 6.1 唯一时间基准 = 音频播放时钟（V7）

- 嘴型由 TTS 音频驱动，所以编排的时间轴**必须**锚在音频播放位置；
  墙钟（`performance.now()`）、消息到达时刻**不合格**（RESEARCH §9.1 第 1 条）；
- 两套时钟现状（嘴跟音频、身体跟墙钟）是本协议要消除的对象（RESEARCH §9.2）。

### 6.2 段 A / 交接点 / 段 B（V7）

| 阶段 | 定义 | 时钟 |
| --- | --- | --- |
| **段 A** | 首个音频**之前**（如「闭眼思考」） | **墙钟**（还没有音频时钟） |
| **交接点** | **首个音频开始播放**（现有 `audio.sentenceStarts` + `first_chunk` 边界帧） | 切换点 |
| **段 B** | 首音频之后 | **全部按音频时钟** |

- **接续规则**：`hold` 的段 A cue 在交接点上**继续保留**，直到导演显式 release / 下一条指令——
  否则闭眼会在开口那一瞬被 ttl 掐掉（RESEARCH §9.1 第 3 条 / §9.3）。
  ⚠ 这是 §9.3 的**建议**，维护者未逐字确认 → 列入 §12。

### 6.3 `at` 锚点枚举（V6）

| `at` | 含义 | 时钟 |
| --- | --- | --- |
| `now` | **立即生效**，不排队（V2） | 段 A = 墙钟；段 B = 音频时钟当前位置 |
| `seg:N` | **第 N 段音频开始播放**时生效（1-based，N ≤ `segments.len()`） | 音频时钟（段边界） |
| `after_prev` | **上一条 cue 的动作做完**事件到达后生效（事件式，不是 `sleep(n)`） | 事件 |

### 6.4 `after_prev` 的定义

- **事件式**：等上一条 cue 的「动作做完」（渲染面 ack：`preset_expired` /
  `preset_replaced`，§7），**不是**写死 ms（RESEARCH §9.1 第 4 条：TTS 首包延迟不可预测 ⇒
  编排不能写死固定 ms）；
- 若上一条 cue 是 `hold=true`（永远不到「做完」点）→ `after_prev` **退化为 `now`**
  （在同锚点上立即生效）。⚠ 该退化规则是本波自定 → §12。

### 6.5 音频时钟下发（stage-clock 消息）

- 前端**已经有** 30ms 粒度的本段音频时钟
  （`shell/flutter/lib/audio/audio_player.dart`，按 `element.currentTime` 查句内包络）；
- 新增一条**轻量消息** `{seg, pos_ms, playing}`（每 30ms 或每帧），渲染面用它**替代
  `performance.now()`** 当 `now_ms`（RESEARCH §10.4 推荐 (a) 一步到位）；
- 段边界继续用现有 `sentenceStarts`，并**新增「段结束」事件**（§7）。
  ⚠ 消息**字段名**是本波自定 → §12。

### 6.6 断开 = 时钟消失，编排同步取消、不补帧（V7）

停止键 / 新用户消息 → 音频链断（epoch 推进、队列清空）→
**编排必须同步取消**，否则会在没有声音时追着播完，或在下一次音频开始时**补播旧帧**
（RESEARCH §9.1 第 5 条）。落地规则见 §9（清动作 + 表情 + TTS 待播 + 回 baseline）。

---

## 7. ack 事件表（V8）

### 7.1 事件（四条 ack + 段结束）

| 事件 | 触发 | 语义 |
| --- | --- | --- |
| `preset_applied` | cue 生效 | 「做了什么就回什么」（沿用 `stage-ack` 先例） |
| `preset_replaced` | 同槽被新 cue 顶掉 | 旧 cue 的结束点 |
| `preset_expired` | 非 hold cue 到点 | 「动作做完」的来源（喂唤醒条件，§8） |
| `preset_dropped` | 皮套**缺该参数**（`override_parameter` 返回 false 静默降级） | **最该暴露的事实**：点了没反应 = 皮套缺这条参数 |
| `segment_ended` | 某段音频播完 | 「TTS 段播完」的来源；**唤醒条件的另一半** |

- **不做每帧状态流、不做前端镜像**（V8）：前端镜像**必然撒谎**（它只知道「我发了什么」，
  不知道钳位 / 降级 / physics + idle 叠加），而且会掩盖「皮套缺参数」这个事实（RESEARCH §10.3）。

### 7.2 payload（携带「当时生效的字段 + 强度 + 是否被钳位或降级」）

| 键 | 语义 |
| --- | --- |
| `type` | 上表事件名 |
| `epoch` | 轮次代号（不透明；0 合法） |
| `ts_ms` | 事件时间 |
| `seq` | cue 序号（与 plan 内顺序一致，从 1 起） |
| `field` | `body` / `head` / `expression` |
| `id` | `expression` 才有 |
| `x` / `y` / `z` | **最终生效**的轴值（已乘基础强度 / 已钳位） |
| `intensity` | 生效强度 |
| `clamped` | bool：是否触通道红线被钳位 |
| `degraded` | bool：是否因皮套缺参数被静默降级 |
| `reason` | `degraded` 时的参数名（**不含正文**） |
| `seg` | `segment_ended` 才有：第几段 |

⚠ 事件**字段名与 WS 帧类型名**是本波自定 → §12。**帧结构只增不改**（V11）。

### 7.3 去向

渲染面 → 前端 → **汇入日志文本** → 喂导演（配合 RESEARCH §10.7 Q5 的「可以把日志喂给导演」）。
**不新增每帧状态流、不建立前端镜像**（V8）。

---

## 8. 唤醒条件（V9）

**导演 LLM 的唤醒 = 两个条件同时满足**：

1. **该段 TTS 播完**（`segment_ended`）；**且**
2. **动作做完**（`preset_expired` / `preset_replaced`，即非 hold 动作到点或被打断）。

**退化（明文保留）**：若某段只下了 `hold` 的动作，就**没有「动作做完」这个点**，
此时唤醒**退化为只有「TTS 段播完」**（RESEARCH §10.7 边界条款，维护者原话「这条我按此实现」）。

- 两者都到 → 组一条**段完成事件**喂导演（RESEARCH §10.7 Q9）。
- **`preset_dropped` 有没有「做完点」**：没有（参数没上，谈不上演完）——
  当前按「不产生动作完成信号、只进日志」处理。⚠ **列入 §12 待用户点头**。

---

## 9. 会话 baseline（V10）

### 9.1 绑定关系

- **角色 baseline 绑会话**：不同会话（不同角色）各有自己的 baseline；
  「退回」= 回到**该会话**的 baseline（V10）。
- baseline 的内容 = 「多角色扮演本身的基础状态值」（开心 / 伤心 / 愤怒 / 思考这类，
  RESEARCH §10.1 Q7）。

### 9.2 落点：复用 `session_scope` 的**兄弟字段**，不新造机制（V10）

- 现有宿主表 `crates/live2d-ai-desktop/src/session_scope.rs`
  （`SessionScopeStore`，按会话存 `system_prompt` 槽，取值入口 `prompt_for(...)`）；
- baseline 作为它的**兄弟字段**（同表、同会话键、同归一化闸），
  **不新造第二套会话表、不加新机制**；
- 会话 id 规则沿用 `sanitize_session_id`（[session-scope-l1.md](session-scope-l1.md) §3）。

### 9.3 停止 / 新消息 = 清三样 + 回 baseline（V10）

| 触发 | 动作 |
| --- | --- |
| **停止键** | 清**动作** + 清**表情** + 清 **TTS 待播** + 回该会话 baseline |
| **新用户消息** | 同上 |

- 与 V7 §6.6 是同一条纪律：时钟消失 ⇒ 编排同步取消、不补帧。
- ⚠ **过渡方式**（立即定格 / 平滑回基准 / 立刻归零）是 RESEARCH §9.4 **Q7，维护者未答** →
  列入 §12。

---

## 10. 失败 / 降级矩阵

| # | 触发 | 类别 | 行为 | 原因码 |
| --- | --- | --- | --- | --- |
| 1 | `[performance] enabled=false` | 关闸 | 不装配表演层，走既有流式路径（**不是回退**） | `performance_disabled` |
| 2 | 开了但缺端点 / 模型 | 降级 | 每轮回退：`SentenceAssembler` 切 `clean_for_tts(原文)` 产多段（D22）+ 规则 cue | `performance_disabled` |
| 3 | 超时 / 连接失败 / 非 2xx / 无正文 | 回退 | 同上 | `performance_request_failed` |
| 4 | 坏 JSON / 缺字段 / 词表外 field / `at` 非法 / 越界锚点 / 超条数 / 超长 | 回退 | 同上（整份失败） | `performance_plan_invalid` + 细分码（§2.4） |
| 5 | **`segments` 拼接 != 原文**（V1 被违反） | 回退 | 同上 | `performance_plan_segments_not_partition` |
| 6 | 主模型本轮无正文 | noop | 不说不动 | `performance_empty_assistant` |
| 7 | 轴值 / intensity 越界 | 钳位 | 继续执行 | （无） |
| 8 | `body` 给 `z` / 未知表情 id | 局部丢弃 | 丢该键 / 该条 cue，其余照演 | `performance_axis_not_allowed` / `_expression_unknown_id` |
| 9 | 皮套缺该参数 | 静默降级 | 参数不上，发 `preset_dropped` | （渲染面事件） |
| 10 | 通道红线触顶 | 钳位 | 继续执行，ack `clamped=true` | （无） |
| 11 | 停止 / 新消息 | 取消 | 清动作 + 表情 + TTS 待播 + 回 baseline；**不补帧** | （无） |
| 12 | 热重载重建运行时 | 重置 | 计数清零（v0 §9.3 既有缺口，沿用） | （无） |

- **两种提供者的不对称**（v0 D11，明文保留）：performance 开的中性轮是 **noop**（不立即撤销，
  靠 ttl 到点）；performance 关（director 规则）的中性轮给 **`none` 撤销哨兵**（立即撤销）。
  v1 **不改**这条不对称；统一与否见 §12。
- **干净文本 + 规则导演 = 仅失败回退**（v0 §5 既有口径，沿用）。

---

## 11. 与 v0 的兼容矩阵（V11：一律不删，缺省即旧语义）

| v0 元素 | v1 处置 | 说明 |
| --- | --- | --- |
| `speak` | **保留可解析（弃用）** | 缺 `segments` 时走 v0 旧语义（可改写说辞）；新计划一律写 `segments` |
| `cues[].sentence_seq` | **保留** | 与 `at:"seg:N"`（1-based）语义对应，映射由实现层保证 |
| `cues[].preset_id` | **保留** | 新计划用 `field:"expression", id:…`；**帧字段名不删** |
| `preset_id == "none"` | **保留** | 撤销哨兵，语义不变（段/句边界撤销两槽） |
| `cues[].intensity` | **保留** | `1..=3`，越界钳位 |
| `cues[].ttl_ms` | **保留** | `(0, 5000]`，越界钳位 |
| `cues: []` | **保留** | 本轮不动（不是撤销） |
| WS `action_cue` 帧 | **不删、不改结构** | 只增不改；`preset_id` 字段永在（V11） |
| `latest.preset_id` | **保留** | 面板只读展示；前端**不再**用它驱动舞台（D12） |
| director `staging_*` 提示词 | **保留** | 只产 cues（V12 文档事实） |
| 规则导演回退 | **保留** | 单一真源仍是 `presets::rule_cues_for_text` |
| 未知字段 | **忽略**（宽容） | 顶层未知键丢弃；前端遇未知字段忽略（V11） |

**纪律**：v0 的四个「一个都不能删」——`action_cue` 帧、`preset_id`、`speak`、`none` 哨兵。
v1 **只新增**（`segments`、`field`/`x`/`y`/`z`/`id`/`at`/`hold`、ack 事件、stage-clock 消息），
**不移除、不改既有字段含义**。

---

## 12. 本波自定点：提问原文（**已于 2026-09-26 裁决，见 §12.1**）

以下各点 V1–V12 未覆盖，本文件为了可落地给了**暂定规则**（上文已就地标注）。
**2026-09-26 维护者已逐条裁决（§12.1）；下表保留为提问原文，不再代表现行规则。**

| # | 待裁决点 | 本文件的暂定规则 | 为什么需要你 |
| --- | --- | --- | --- |
| O1 | **`speak` 与 `segments` 同时出现** | 整份失败（`performance_plan_ambiguous_text`） | V1 说 speak 退役、V11 说字段不删；二者同时出现没有覆盖 |
| O2 | **`segments` 条数上限** | 沿用 `MAX_CUES=16` 作 cue 上限；段数上限暂定 `64`、总字符 `4000`（沿 v0 `MAX_SPEAK_CHARS`） | V1 只说「逐字不变」，没给边界 |
| O3 | **`ttl_ms` 是否必填** | 可选；默认 body/head `900`、expression `2600` | 维护者草案 schema 无 `ttl_ms`，但 V5 说「按 ttl 到点回」 |
| O4 | **`after_prev` 遇到 hold 时** | 退化为 `now` | V6 只列枚举，未定义退化 |
| O5 | **`expression` 的 add 细则**（同 id 叠加 / 不同 id 同通道如何合成） | 「同通道相加、缺通道由后者补」 | V2 只冻结「同类 add」，没给 expression 的多 id 合成 |
| O6 | **段 A 在交接点的接续** | `hold` 的段 A cue 继续保留到显式 release | RESEARCH §9.3 是**建议**，维护者未逐字确认 |
| O7 | **停止 / 新消息的过渡方式** | 立即回基准（无过渡动画） | RESEARCH §9.4 Q7，维护者**未答** |
| O8 | **baseline 的存储与写入方** | 复用 `session_scope` 兄弟字段；写入方 = host（谁写未定） | V10 只说「落点复用兄弟字段」，没说谁写 |
| O9 | **每皮套映射表的载体 / 粒度** | 资产（每皮套一份）+ 渲染面内建 fallback | V4/RESEARCH 说「按皮套、人工标定」，没说住哪 |
| O10 | **`[action]` 全局倍率是否要按皮套分组** | 否：`[action]` 保持全局用户旋钮，按皮套那一份住映射表 | 「按皮套的基础强度」与「现有全局 `[action]`」的分工未逐字确认 |
| O11 | **可选叠加通道是否纳入**（肩 X3/X6 / 胯 X4 / 迈腿 X5 / 眼珠 EyeBallX·Y） | 本波不纳入 | RESEARCH §10.7 明写「**需你确认是否纳入**」 |
| O12 | **`expression` 面板集合是否扩**（现有 3 条） | 本波保持 3 条 | RESEARCH §10.7 写「可扩」，未说扩不扩 |
| O13 | **ack 事件与 stage-clock 消息的字段名 / WS 帧名** | 见 §7.2 / §6.5 | V8 只给事件语义，未给 wire 名 |
| O14 | **`preset_dropped` 是否也算一个「段完成」信号** | 否，只进日志 | V9 只定义了「动作做完」= 非 hold 到点 |
| O15 | **D11 不对称是否在 v1 统一** | 不统一，明文保留 | v0 明文写「统一属阶段4」；本协议是那次机会，但需你裁决 |
| O16 | **说话权归属（Q1）** | **本波不裁决**（V12） | 见 §1 |

### 12.1 维护者裁决（2026-09-26，**冻结**）

> 用户 2026-09-26 确认：**V1（`speak` 退役为 `segments`）成立**——表演层 = **只断句 + 出 cues 的导演**；
> Gate 5 的版本口径 = **`0.2.0-rc.x`**（不升 `0.3.0`）。以下 O1–O16 由维护者逐条裁决。

| # | 裁决 | 备注 |
| --- | --- | --- |
| **O1** | **`segments` 优先；`speak` 忽略 + warn**（**不整份失败**） | V1 不变量已保证原文不被改写；不因一个弃用字段丢掉本轮 cues。回归 `both_segments_and_speak_prefers_segments_and_warns` |
| **O2** | 段数上限 **64**、总字符上限 **4000**；`MAX_CUES` 沿用 **16**；超限**整份失败**（不截断——截断会破坏 V1 不变量） | 错误码沿用 §2.4 #18 |
| **O3** | `ttl_ms` **可选**；默认 `body`/`head` **900**、`expression` **2600** | schema 保持简单；V5 由默认值满足 |
| **O4** | `after_prev` 遇 `hold=true` → **退化为 `now`** | 显式回归，不得变成「永不生效」 |
| **O5** | `expression` 与 `body`/`head` **同一套 add 语义**（按通道累加、最后统一钳位）；**删掉「后者补缺通道」** | 见 §4.3 |
| **O6** | `hold` 的段 A cue 在交接点**继续保留**到显式 release / 下一条指令 | 否则「闭眼」会在开口瞬间被 ttl 掐掉 |
| **O7** | 停止 / 新消息 → **立即回基准（无过渡动画）** | ⚠ **产品口味**：过渡动画进 backlog，用户验收后可回改 |
| **O8** | baseline 存 `session_scope` **兄弟字段**；**写入方 = 宿主 API**（随会话设置写）；**缺省为空 = 待机**；本轮**不做** per-character profile 文件 | 不新造第二套会话表 |
| **O9** | 映射表 = **渲染面内建默认表 + 可选单文件覆盖 `assets/actions/field_map.json`**（按 model id 分节）；本轮**不要求**每皮套一份 | 表是数据、不进提示词 |
| **O10** | `[action]` **保持全局**用户旋钮；「按皮套的基础强度」住映射表 | 不按皮套分组 |
| **O11** | 肩 `X3/X6` / 胯 `X4` / 迈腿 `X5` / 眼珠 `EyeBallX·Y` **本波不纳入** | ⚠ **产品口味**，可回改 |
| **O12** | `expression` 面板保持现有 **3 条** | 可扩，不进本波 |
| **O13** | **wire 名冻结**：stage-clock = `{version:1,type:"stage-clock",payload:{seg,pos_ms,playing}}`（前端→渲染面）；ack 四条独立 type = `preset-applied` / `preset-replaced` / `preset-expired` / `preset-dropped`；段结束 = `segment-ended`（渲染面→前端），payload 按 §7.2 | 三轨共用，不得各自起名 |
| **O14** | `preset_dropped` **不算**段完成信号；**但**若某段所有 cue 都 dropped → 视为无动作，唤醒**退化为仅 TTS 段播完**（与 hold 退化同口径） | 否则该段永不唤醒 |
| **O15** | D11 不对称 **v1 不统一**（明文保留） | ⚠ **产品口味**，可回改；进 backlog |
| **O16** | 说话权归属（Q1）**不裁决** | V12 |

### 12.2 接口级冻结 D22–D26（本波新增，2026-09-26）

D22–D26 不在原 O 表内，是维护者复核契约时发现的**跨层接口空洞**；已就地写进正文，此处汇总索引：

| # | 冻结 | 落点 | 回归 |
| --- | --- | --- | --- |
| **D22** | **段 ↔ 句 1:1**：表演层给的 `segments` 一对一成为 TTS 单元/音频元素；`seg:N` ≡ `sentence_seq==N`；不再二次切分。回退路径仍用 `SentenceAssembler`（v0 行为） | §3.1 / §10 #2 | `segments_are_one_to_one_with_sentences_and_never_resplit` |
| **D23** | **上屏 == 送 TTS == `clean_for_tts(段)`**；`segments` 是切分方案、不是上屏字符串 | §3.1 | `displayed_text_equals_tts_text_per_segment` |
| **D24** | **空白段不跳号**：仍产静音元素与边界帧，`seg` 编号连续 | §3.3 | `blank_segment_keeps_its_seg_index` |
| **D25** | （已并入 O14）dropped 段的唤醒退化 | §8 / §10 #9 | 同 C4 / C6 |
| **D26** | **批内 add / 跨批次 replace**：同 plan 同 field 按 `at` 生效并相加；新一批 cue 到达同 field → 结束旧动画段（`preset_replaced`）并以新值为当前值；非 hold 到点移除自己贡献（`preset_expired`）；`none` 清空累加器 | §4.3 | `new_batch_replaces_the_field_animation_value_adds` |

---

## 13. 相关文件（单一真源索引）

| 内容 | 真源 |
| --- | --- |
| 本协议（v1） | **本文** |
| v0 表演层（`speak` 语义、配置、回退矩阵） | [performance-layer-v0.md](performance-layer-v0.md) |
| 动作包 / intensity / 幅度标定 | [action-packs-v0.md](action-packs-v0.md) |
| director Mod（规则层 + `staging_*`） | [director-mod-v0.md](director-mod-v0.md) |
| 会话作用域 / 会话 id 闸 | [session-scope-l1.md](session-scope-l1.md) |
| TTS 是核心链路（端点唯一权威） | [tts-is-core.md](tts-is-core.md) |
| 裁决原文（§8–§10.7） | [RESEARCH-actions-director-audit-2026-09-21.md](../plans/RESEARCH-actions-director-audit-2026-09-21.md) |
| 阶段3 收口（D14 / D20 纪律） | [STAGE3-CLOSEOUT-2026-09-26.md](../plans/STAGE3-CLOSEOUT-2026-09-26.md) |
| 阶段4 计划 / worker 提示词 | [STAGE4-plan-2026-09-26.md](../plans/STAGE4-plan-2026-09-26.md) · [STAGE4-WORKER-PROMPTS-2026-09-26.md](../plans/STAGE4-WORKER-PROMPTS-2026-09-26.md) |

**实现侧单一真源（v1 落地时更新）**：

- runtime：`crates/live2d-ai-runtime/src/performance/plan.rs`（schema + 校验 + 常量）；
- 渲染面：`crates/l2d-wasm-demo/src/preset/`（表 + 状态机 + 标定）；
- 前端：`shell/flutter/lib/live2d/`（cue 应用 + ack 消费）。
