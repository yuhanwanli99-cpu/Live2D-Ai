# 共享表情协议 emotion-protocol（双端 L1 + L3 单源真理）

> 档位：premium（M1 共享协议，双端 L1+L3 的单源真理）
>
> **锁定面**：expressionMix / expressions[].name|weight / parameterOverrides / motionLabel 字段名与层级不可改。改必须走契约修订（SOP R2 路由）。
>
> **引用方**：
> - Android M2：EmotionController.parseStructured() 解析规则
> - PC M3：live2d_model.py extract_expression_mix() 解析规则 + prompt 升级
> - M7/M8：motion 映射表（F 段）
> - M9：跨端一致性断言

---

## A. 协议目标与双端引用方

本文件是双端（Android `EmotionController` / PC `live2d_model.py`）表情（L1）+ 动作（L3）协议的**单源真理**。

- LLM 结构化回复应按 **B 段 JSON schema** 输出，三字段（expressionMix / parameterOverrides / motionLabel）均为可选。
- 旧 `[keyword]` 标签（如 `[joy]`、`[joy_3]`）保留为**降级路径**（见 E 段），双端向后兼容不变。
- JSON 解析成功 → 走结构化路径；解析失败 → 降级到旧 `[keyword]` 路径。混合场景（JSON + `[keyword]` 共存）→ JSON 优先，`[keyword]` 由现有 strip/sanitize 逻辑剥离。

---

## B. JSON schema（LLM 结构化输出格式）

以下为 LLM 回复中可嵌入的 JSON 结构（可放置于 markdown code fence 内或裸文本中）：

```jsonc
{
  // 表情混合权重（主通道，缺失 = 表情层不动作，不降级）
  "expressionMix": {
    "expressions": [
      { "name": "joy",      "weight": 0.7 },   // weight ∈ [0.0, 1.0]，总和 ≤ 1.0
      { "name": "surprise", "weight": 0.3 }    // 未指定的表情 weight=0（隐含）
    ]
  },
  // 参数直接覆盖（可选，与 expressionMix 同时生效）
  "parameterOverrides": {
    "ParamEyeLOpen": 1.2,    // key = Live2D 参数 ID（白名单见 C3），value = 乘数（1.0=默认）
    "ParamMouthForm": 0.8    // 超出参数合法范围的由渲染器 clamp
  },
  // 动作标签（可选，LLM 显式指定播放哪个 motion；null/缺失 → 关键词兜底）
  "motionLabel": "wave"      // 取值 = F 段别名表中的别名
}
```

**字段说明**：

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `expressionMix` | object | 否 | 缺失 | 包含 `expressions` 数组；缺失或空 → 表情层不动作 |
| `expressionMix.expressions` | array | 否 | `[]` | 表情权重列表，每项含 `name`（C1 枚举）和 `weight`（C2 约束） |
| `expressionMix.expressions[].name` | string | 是 | — | 8 key 枚举之一（大小写不敏感），见 C1 |
| `expressionMix.expressions[].weight` | number | 是 | — | 浮点数 ∈ [0.0, 1.0]，见 C2 |
| `parameterOverrides` | object | 否 | 缺失 | key 为 Live2D 参数 ID（白名单见 C3），value 为乘数 |
| `parameterOverrides.<key>` | number | — | — | 乘数，1.0 = 保持当前值不变，见 C3 |
| `motionLabel` | string \| null | 否 | null | F 段别名表中的别名，见 C4；未知 → 视为 null |

---

## C. 字段约束

### C1 · expressionMix.expressions[].name 枚举

取值必须为以下 **8 个 key**（大小写不敏感，实现统一 lowercase 后校验）：

| key | 中文 | niziiro_mao 表情文件 |
|---|---|---|
| `neutral` | 中性 | exp_01 |
| `fear` | 恐惧 | exp_02 |
| `sadness` | 悲伤 | exp_03 |
| `anger` | 愤怒 | exp_04 |
| `disgust` | 厌恶 | exp_05 |
| `joy` | 高兴 | exp_06 |
| `smirk` | 得意 | exp_07 |
| `surprise` | 惊讶 | exp_08 |

> 来源：Android `EmotionController.kt` `EMOTION_MAP`（L27-34），实读确认。

未知 name → 丢弃该项（保留其余合法项）。

### C2 · weight 约束与容错

- `weight` 类型为 number，合法范围 **[0.0, 1.0]**（含边界）。
- 所有 `expressions[].weight` 之和 ≤ **1.0**（LLM 输出约束）。
- **实现端容错规则**（双端一致，M9 断言同输入同输出）：
  - 单项 weight 非法（`<0` 或 `>1` 或非数字）→ 丢弃该项。
  - `expressionMix.expressions` 为空或全部被丢弃 → `expressionMix` 视为空（表情层不动作，不降级）。
  - 权重总和 > 1.0 → **按比例归一化**（每项 `weight / sum`，使总和 = 1.0）。

### C3 · parameterOverrides key 白名单

- key 白名单 = **当前模型 cdi3.json `Parameters[].Id` 全集**（niziiro_mao 128 个，均实读）。
- 实现以**运行时读取 cdi3.json 动态校验**（不硬编码，换模型自动适配）。
- 非法 key（不在当前模型 cdi3.json 白名单中）→ **丢弃该 key**（保留其余合法 key）。
- value 语义 = **乘数**：渲染时 `最终值 = 当前值 × 乘数`，clamp 到参数合法范围；无合法范围信息的参数按 [0, 1] clamp。
  - `1.0` = 保持当前值不变
  - `>1.0` = 放大（如 `ParamEyeLOpen: 1.2` → 眼睛比默认张更大 20%）
  - `<1.0` = 缩小（如 `ParamMouthForm: 0.0` → 嘴巴完全闭合）
  - ⚠️ [UNSURE-2]：value 乘数语义按 architecture.md §3.1 锁定；若渲染效果异常需回架构确认是否改绝对值。

### C4 · motionLabel 取值

- 取值必须为 **F 段别名表中的别名**（如 `idle`、`wave` 等），不是模型组名直用（niziiro_mao 含空字符串组名 `""`，不可直用）。
- 未知别名 → 视为 `null`（走关键词兜底路径，不报错）。
- `null` / 缺失 / 空字符串 → motion 层不动作（由调用方按文本关键词兜底）。

### C5 · 部分生效原则

- 顶层三个字段（`expressionMix`、`parameterOverrides`、`motionLabel`）全部**可选**。
- 至少一个字段存在且结构合法才算"JSON 解析成功"。
- `expressionMix` 缺失或空 → 表情层不动作，但 `parameterOverrides` / `motionLabel` 仍生效（部分字段合法 → 部分生效，不整体降级）。
- JSON 解析失败（无 JSON / 畸形 / 顶层结构非法）→ 整体降级到 E 段 `[keyword]` 路径。

---

## D. 示例

### 中文回复（含 code fence + 双表情 + 参数覆盖 + motionLabel）

白的 LLM 回复示例：

````
欢迎回来～今天也想你啦！

```json
{
  "expressionMix": {
    "expressions": [
      { "name": "joy",      "weight": 0.8 },
      { "name": "surprise", "weight": 0.2 }
    ]
  },
  "parameterOverrides": {
    "ParamEyeLOpen": 1.3,
    "ParamMouthForm": 0.7
  },
  "motionLabel": "wave"
}
```

今天过得怎么样？……嘿嘿，想你了呀～
````

> 解析：表情 = joy 80% + surprise 20%（归一化后），眼睛张更大（×1.3），嘴巴微收（×0.7），动作 = wave。

### 英文回复（裸 JSON，单表情，最小合法形）

LLM 回复（无 markdown fence，裸 JSON）：

```
That's great to hear! {"expressionMix":{"expressions":[{"name":"joy","weight":1.0}]}} Keep it up!
```

> 解析：表情 = joy 100%，无参数覆盖，无 motionLabel（走关键词兜底）。

---

## E. 降级规则

```
LLM 回复文本
  ├─ ① 提取 JSON（fence 优先 → 裸 JSON 兜底）
  │    ├─ ② 解析 + schema 校验成功
  │    │    ├─ expressionMix 合法 → 多表情加权混合（M2/M3 应用）
  │    │    ├─ parameterOverrides 合法 key → 参数覆盖（非法 key 丢弃）
  │    │    ├─ motionLabel 合法别名 → 动作触发（未知 → null）
  │    │    └─ 部分字段合法 → 部分生效，不整体降级（C5）
  │    └─ ② 失败（无 JSON / 畸形 / 结构非法）→ 降级 ③
  └─ ③ [keyword] 旧路径：Android parseAndApply(text) / PC extract_emotion(text)
       → 单表情（最后匹配标签，Android 带强度后缀），无参数覆盖，无 motionLabel
```

- 双端降级规则**必须一致**（M9 跨端断言基础）。
- 降级是**静默**的：不报错、不打断主链路（与现有 `[keyword]` 自然降级行为一致）。
- LLM 回复中 JSON 与 `[keyword]` 混合出现时 → JSON 优先，`[keyword]` 由现有 strip/sanitize 逻辑剥离。

---

## F. motion 映射表

> 来源：实读 `niziiro_mao.model3.json` FileReferences.Motions（双端字节级一致）+ `AnimationSystem.kt:1271-1312`（键拼接 = `${group}_$i`）。
>
> ⚠️ `niziiro_mao` 空组名 `""`（含 mtn_02~04 / special_01~03 共 6 个 motion）不可直用为 motionLabel，必须走别名。
>
> [UNSURE-1]：motion3.json 只含参数曲线、无动作语义标签，无法从文件确认 special_01 就是"挥手"。语义列为建议值，待 M7/M8 实现时在模型上肉眼验证后回填。

### niziiro_mao motion 文件清单

| 文件 | 组名 | 组内索引 | Android 键 | Duration | Loop | 备注 |
|---|---|---|---|---|---|---|
| mtn_01 | `Idle` | 0 | `Idle_0` | 5.57s | true | Idle 组（待机） |
| mtn_02 | `""`（空） | 0 | `_0` | 3.47s | true | 空组，普通动作 |
| mtn_03 | `""`（空） | 1 | `_1` | 4.40s | true | 空组，普通动作 |
| mtn_04 | `""`（空） | 2 | `_2` | 4.20s | true | 空组，普通动作 |
| special_01 | `""`（空） | 3 | `_3` | 7.80s | true | ParamAngleY 摆至 -22.9°（大动作） |
| special_02 | `""`（空） | 4 | `_4` | 9.37s | true | ParamAngleY 摆至 -22.8° |
| special_03 | `""`（空） | 5 | `_5` | 9.23s | true | ParamAngleY 摆至 -25.9° |

### 别名映射表

| motionLabel 别名 | 语义建议 | niziiro_mao 文件 | niziiro_mao Android 键 | 状态 |
|---|---|---|---|---|
| `idle` | 待机 | mtn_01 | `Idle_0` | ✅ 已确认（组名 Idle 语义明确） |
| `wave` | 挥手/打招呼 | special_01 | `_3` | [UNSURE-1] 语义未验证 |
| `special_02` | 特殊动作 | special_02 | `_4` | [UNSURE-1] |
| `special_03` | 特殊动作 | special_03 | `_5` | [UNSURE-1] |
| `mtn_02` | 普通动作 | mtn_02 | `_0` | [UNSURE-1] |
| `mtn_03` | 普通动作 | mtn_03 | `_1` | [UNSURE-1] |
| `mtn_04` | 普通动作 | mtn_04 | `_2` | [UNSURE-1] |

> shizuku 已删除（K1）：其语义化组名（FlickUp/Tap/Flick3）不再可用，相关行随实体一并移除。

**处理规则**：
- motionLabel 取值查 niziiro_mao 别名表 → 命中则用对应 Android 键。
- 均未命中 → motionLabel = null（不报错，走关键词兜底）。
- [UNSURE-1] 别名→文件语义映射：表结构/文件→键对应关系已锁定（确定面），语义列为建议值。待 M7/M8 实现时在模型上肉眼验证，或用户直接拍板后回填本表。

---

## G. 变更记录

| 版本 | 日期 | 变更 | 说明 |
|---|---|---|---|
| v1.0 | 2026-08-08 | 初始版本 | 字段锁定自 architecture.md §3.1 + contract-M1-shared-protocol.md §1；A~G 七段完整；别名映射表含 [UNSURE-1] 标注 |
| v1.1 | 2026-08-22 | 新增 §J motionTimeline | PC 端括注动作帧（播放位置触发）；Android 可后续对齐 M7 关键词兜底 |
| v1.2 | 2026-08-25 | §J1 帧级元数据 + 同轮多动作保序 | motionTimeline 帧新增可选 strength/source/priority/reason（渲染端单 active 调度器逐帧消费）；主 LLM 同轮多工具动作整队转时间线（[0.1,0.8] 保序分布），修复「只保留最后一个」 |

---

## H. 表情参数映射（emotionParamMap 8×3 + fade 规范）

> 单源真理：`shared/model-adapter/bai.adapter.json` 的 `emotionParamMap` 字段是机器可读形式；
> 本段是其文档化表达（contract-shared §S2：8 表情 × 3 强度 × 参数组）。
> 参数名均为白-免费版 cdi3.json 实读确认的标准 Cubism 参数（model-facts-bai.md §三）。
> 值语义 = **目标绝对值**（非乘数），范围 = 参数合法值域 [-1.0, 2.0]（保守取宽域）。

```yaml
# emotion → 参数组合（目标绝对值，非乘数）
# 每参数值 ∈ [-1.0, 2.0]（Cubism 参数合法值域），fade 20-1000ms
# 强度后缀：_1=弱、_2=中、_3=强
emotion_param_map:
  neutral:
    _1: {ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthForm: 0.5, ParamMouthOpenY: 0.0}
    _2: {ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.75, ParamEyeROpen: 0.75, ParamMouthForm: 0.55, ParamMouthOpenY: 0.0}
    _3: {ParamBrowLY: 0.0, ParamBrowRY: 0.0, ParamEyeLOpen: 0.80, ParamEyeROpen: 0.80, ParamMouthForm: 0.6, ParamMouthOpenY: 0.0}
  joy:
    _1: {ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.80, ParamEyeROpen: 0.80, ParamMouthForm: 0.7, ParamMouthOpenY: 0.1}
    _2: {ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.85, ParamEyeROpen: 0.85, ParamMouthForm: 0.85, ParamMouthOpenY: 0.2}
    _3: {ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.90, ParamEyeROpen: 0.90, ParamMouthForm: 0.95, ParamMouthOpenY: 0.35}
  anger:
    _1: {ParamBrowLY: -0.2, ParamBrowRY: -0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthForm: -0.3, ParamMouthOpenY: 0.05}
    _2: {ParamBrowLY: -0.35, ParamBrowRY: -0.35, ParamEyeLOpen: 0.60, ParamEyeROpen: 0.60, ParamMouthForm: -0.55, ParamMouthOpenY: 0.1}
    _3: {ParamBrowLY: -0.5, ParamBrowRY: -0.5, ParamEyeLOpen: 0.55, ParamEyeROpen: 0.55, ParamMouthForm: -0.75, ParamMouthOpenY: 0.2}
  sadness:
    _1: {ParamBrowLY: 0.15, ParamBrowRY: 0.15, ParamEyeLOpen: 0.55, ParamEyeROpen: 0.55, ParamMouthForm: -0.2, ParamMouthOpenY: 0.1}
    _2: {ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.50, ParamEyeROpen: 0.50, ParamMouthForm: -0.4, ParamMouthOpenY: 0.15}
    _3: {ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 0.45, ParamEyeROpen: 0.45, ParamMouthForm: -0.55, ParamMouthOpenY: 0.25}
  fear:
    _1: {ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 0.85, ParamEyeROpen: 0.85, ParamMouthForm: 0.3, ParamMouthOpenY: 0.2}
    _2: {ParamBrowLY: 0.35, ParamBrowRY: 0.35, ParamEyeLOpen: 1.0, ParamEyeROpen: 1.0, ParamMouthForm: 0.2, ParamMouthOpenY: 0.4}
    _3: {ParamBrowLY: 0.55, ParamBrowRY: 0.55, ParamEyeLOpen: 1.3, ParamEyeROpen: 1.3, ParamMouthForm: 0.15, ParamMouthOpenY: 0.65}
  surprise:
    _1: {ParamBrowLY: 0.25, ParamBrowRY: 0.25, ParamEyeLOpen: 0.95, ParamEyeROpen: 0.95, ParamMouthForm: 0.5, ParamMouthOpenY: 0.3}
    _2: {ParamBrowLY: 0.40, ParamBrowRY: 0.40, ParamEyeLOpen: 1.2, ParamEyeROpen: 1.2, ParamMouthForm: 0.4, ParamMouthOpenY: 0.5}
    _3: {ParamBrowLY: 0.55, ParamBrowRY: 0.55, ParamEyeLOpen: 1.5, ParamEyeROpen: 1.5, ParamMouthForm: 0.3, ParamMouthOpenY: 0.7}
  smirk:
    _1: {ParamBrowLY: 0.1, ParamBrowRY: 0.1, ParamEyeLOpen: 0.70, ParamEyeROpen: 0.70, ParamMouthForm: -0.3, ParamMouthOpenY: 0.0}
    _2: {ParamBrowLY: 0.2, ParamBrowRY: 0.2, ParamEyeLOpen: 0.65, ParamEyeROpen: 0.65, ParamMouthForm: -0.5, ParamMouthOpenY: 0.0}
    _3: {ParamBrowLY: 0.3, ParamBrowRY: 0.3, ParamEyeLOpen: 0.60, ParamEyeROpen: 0.60, ParamMouthForm: -0.7, ParamMouthOpenY: 0.05}
  disgust:
    _1: {ParamBrowLY: -0.15, ParamBrowRY: -0.15, ParamEyeLOpen: 0.60, ParamEyeROpen: 0.60, ParamMouthForm: -0.3, ParamMouthOpenY: 0.05}
    _2: {ParamBrowLY: -0.25, ParamBrowRY: -0.25, ParamEyeLOpen: 0.55, ParamEyeROpen: 0.55, ParamMouthForm: -0.5, ParamMouthOpenY: 0.1}
    _3: {ParamBrowLY: -0.35, ParamBrowRY: -0.35, ParamEyeLOpen: 0.50, ParamEyeROpen: 0.50, ParamMouthForm: -0.65, ParamMouthOpenY: 0.15}
```

### fade 规范

> fade 场景与时长（contract-shared §S2-04，值 ∈ [20, 1000]ms）：

| 场景 | 时长(ms) | 说明 |
|---|---|---|
| emotionSwitch | 300 | 表情切换过渡（emotion→emotion） |
| emotionToNeutral | 500 | 表情→neutral 回退 |
| microExpression | 200 | 单参数微调（待机微表情） |
| lipSync | 50 | 口型参数（跟音频帧率） |

---

## I. mood 协议（心情值 -100 ~ +100）

> 双端共享（architecture-shared §3.2.3 / contract-shared §S3）。
> PC 端消费方：`mood_engine.py`（流式心情值系统）。

### I1. mood 值范围

mood ∈ **[-100, +100]**（float），含义：

| 区间 | 分层 | 表现 |
|---|---|---|
| -100 ~ -60 | furious | 低落/烦躁，语气不耐烦 |
| -60 ~ -20 | upset | 有点低落，语气冷淡 |
| -20 ~ +20 | neutral | 平淡/正常 |
| +20 ~ +60 | happy | 开心/愉悦，语气活泼 |
| +60 ~ +100 | ecstatic | 高涨/兴奋，语气兴奋 |

默认初始值：mood = 0（中性）；跨 session 不持久化（重启归零）。

### I2. mood 打分事件表

触发事件 → Δmood（累加到当前值，clamp [-100, +100]）：

| 事件 | Δmood | 说明 |
|---|---|---|
| 用户主动打招呼/开启对话 | +5 | 有人理=开心 |
| 用户夸奖/表达好感 | +10 | "你好厉害""喜欢你" |
| 用户提问/被需要感 | +2 | 被需要=愉悦 |
| 用户批评/否定 | -10 | "你说错了""不好" |
| 用户长时间不理（>5min） | -3 | 每轮衰减 |
| 被触摸/tap（互动） | +3 | 互动正向 |
| 对话流畅完成（一轮完） | +2 | 完成感 |
| LLM 回复被截断/出错 | -5 | 打击 |
| LLM 回复 joy/surprise 情绪 Token | +5 | 持续情绪积累 |
| LLM 回复 anger/sadness 情绪 Token | -6 | 持续情绪积累 |

### I3. mood 衰减

- 自然衰减速率：**-2 / 分钟**（仅在无对话交互时衰减）
- 下限：**-40**（不会无限低落——"傲娇不会太丧"）
- 上限：+100
- 对话活跃期暂停衰减

### I4. mood→三端联动映射

| 联动端 | mood 影响 | 机制 |
|---|---|---|
| 表情基线 | mood<0 → 默认偏 sadness；mood>0 → 默认偏 joy | `get_expression_modifier()` |
| TTS 基调 | mood 分层 → speech_rate/pitch_rate 微调 | `get_tts_modifier()` |
| LLM prompt | mood 分层 → system prompt 注入文案 | `get_prompt_injection()` |

### I5. mood 协议接口（双端对齐）

```
// mood 状态对象（双端共享）
{
  "mood": float,      // -100 ~ +100，当前值
  "moodState": string, // "furious"|"upset"|"neutral"|"happy"|"ecstatic"
  "lastEvent": string, // 最近一次打分事件类型
  "timestamp": int    // Unix ms
}
```

---

## J. motionTimeline 括注动作帧（PC 端，播放位置触发）

> my-neuro 式「文本位置 → 动作帧」：动作不再随 audio 帧到达即整句一次性触发，
> 而是**语音播放到对应文本位置时才触发**。Android 已有同思想的 M7 关键词兜底
> （t(pos)=pos/len×totalMs 线性近似），本节把该契约固化到共享协议，供双端对齐。

### J1. 数据形态

随 `audio` 帧 `actions.motionTimeline` 下发（`Actions.to_dict()` 产出）：

```json
{
  "type": "audio",
  "actions": {
    "motionTimeline": [{"at": 0.0, "motion": "nod"}, {"at": 0.42, "motion": "happy"}]
  }
}
```

| 字段 | 类型 | 约束 |
| --- | --- | --- |
| `at` | float ∈ [0,1) | 该动作在**所在音频块**内的归一化播放进度（按文本位置线性近似） |
| `motion` | string | BodyGesture 别名（nod/tilt/sway/happy/surprise/listen/excited/idle） |
| `strength` | int ∈ {1,2,3}，可选 | 帧级动作强度（缺省 2）；主 LLM 工具/用户命令时间线逐帧携带 |
| `source` | string，可选 | 触发来源：`llm_tool` / `user_command` / `stage_direction` 等；透传给渲染端单 active 调度器 |
| `priority` | int，可选 | 后端调度优先级（用户命令=100 > llm_tool=90 > 括注=60 > 导演=50）；渲染端 ≥100 映射 Live 档、其余映射 Cue 档 |
| `reason` | string，可选 | 一句话触发原因（仅调试/面板展示，不参与调度） |

> v1.2 起（同轮多工具动作修复）：主 LLM 一轮内多次调用
> `live2d_perform_action` 时不再只保留最后一个 —— 绑定层按接受顺序把整队
> 事件转成下一句的 motionTimeline（帧序即触发顺序，[0.1, 0.8] 播放进度线性
> 分布，逐帧携带上述元数据），单动作仍走一次性 `motion` 兼容路径；中断 /
> 新 epoch 清空未触发的队列动作。

### J2. 生成链路（PC）

1. `director.stage_gesture_timeline(text)`：解析**全部**全角/半角动作括注 →
   `{position, clean_position, motion}[]`（clean_position 扣除前序括注跨度，
   即清洗后朗读文本中的字符锚点）；
2. `conversation_utils.handle_sentence_output`：句子级帧经
   `project_motion_cues_into_chunks()` 按 `split_tts_chunks` 分块切片，
   归一化为块内进度 `at`；
3. 时间轴存在时清掉一次性 `actions.motion`（避免前端双重触发）；
   投影失败/无命中 → 不下发时间轴，完整保留旧行为。

### J3. 消费语义

- **有 motionTimeline**：忽略同帧一次性 `motion` 字段；音频元素开始播放后挂
  进度调度器，`currentTime ≥ at×duration − 30ms` 时触发一次且仅一次；
  打断（stopAudio/interrupt）/播完自动清理未触发帧。
- **无 motionTimeline**：保持旧行为（audio 帧到达即播 `motion`），旧前端兼容。

### J4. 实现落点

| 端 | 文件 |
| --- | --- |
| PC 后端 | `open_llm_vtuber/director.py`（解析+投影）、`conversations/conversation_utils.py`（分块下发）、`agent/output_types.py`（字段） |
| PC 前端 | `renderer/src/motion-timeline.ts`（调度器）、`renderer/src/ws-bridge.ts`（接线） |
| Android 对齐参考 | `VoiceIoController.scheduleKeywordMotions()`（M7，匀速近似触发） |
