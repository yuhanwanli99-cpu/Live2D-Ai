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

| key | 中文 | mao_pro 表情文件 |
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

- key 白名单 = **当前模型 cdi3.json `Parameters[].Id` 全集**（mao_pro 128 个 / shizuku 45 个，均实读）。
- 实现以**运行时读取 cdi3.json 动态校验**（不硬编码，换模型自动适配）。
- 非法 key（不在当前模型 cdi3.json 白名单中）→ **丢弃该 key**（保留其余合法 key）。
- value 语义 = **乘数**：渲染时 `最终值 = 当前值 × 乘数`，clamp 到参数合法范围；无合法范围信息的参数按 [0, 1] clamp。
  - `1.0` = 保持当前值不变
  - `>1.0` = 放大（如 `ParamEyeLOpen: 1.2` → 眼睛比默认张更大 20%）
  - `<1.0` = 缩小（如 `ParamMouthForm: 0.0` → 嘴巴完全闭合）
  - ⚠️ [UNSURE-2]：value 乘数语义按 architecture.md §3.1 锁定；若渲染效果异常需回架构确认是否改绝对值。

### C4 · motionLabel 取值

- 取值必须为 **F 段别名表中的别名**（如 `idle`、`wave` 等），不是模型组名直用（mao_pro 含空字符串组名 `""`，不可直用）。
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

小喵的 LLM 回复示例：

````
主人回来啦喵~！小喵好想你！(=^･ω･^=)

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

今天过得怎么样喵~？
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

> 来源：实读 `mao_pro.model3.json` FileReferences.Motions（双端字节级一致）+ `AnimationSystem.kt:1271-1312`（键拼接 = `${group}_$i`）。
>
> ⚠️ `mao_pro` 空组名 `""`（含 mtn_02~04 / special_01~03 共 6 个 motion）不可直用为 motionLabel，必须走别名。
>
> [UNSURE-1]：motion3.json 只含参数曲线、无动作语义标签，无法从文件确认 special_01 就是"挥手"。语义列为建议值，待 M7/M8 实现时在模型上肉眼验证后回填。

### mao_pro motion 文件清单

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

| motionLabel 别名 | 语义建议 | mao_pro 文件 | mao_pro Android 键 | shizuku 对应 | 状态 |
|---|---|---|---|---|---|
| `idle` | 待机 | mtn_01 | `Idle_0` | 04.motion3.json（Idle 组） | ✅ 已确认（组名 Idle 语义明确） |
| `wave` | 挥手/打招呼 | special_01 | `_3` | —（shizuku 无对应） | [UNSURE-1] 语义未验证 |
| `special_02` | 特殊动作 | special_02 | `_4` | — | [UNSURE-1] |
| `special_03` | 特殊动作 | special_03 | `_5` | — | [UNSURE-1] |
| `mtn_02` | 普通动作 | mtn_02 | `_0` | — | [UNSURE-1] |
| `mtn_03` | 普通动作 | mtn_03 | `_1` | — | [UNSURE-1] |
| `mtn_04` | 普通动作 | mtn_04 | `_2` | — | [UNSURE-1] |

### shizuku motion 组（语义化组名，可直接引用）

| 组名 | 组内索引 | 说明 |
|---|---|---|
| `FlickUp` | 0 | 上滑 |
| `Tap` | 0 | 点击 |
| `Flick3` | 0 | 滑动 |
| `Idle` | 0 | 待机 |

> shizuku 组名是语义化的（无空组问题），可直接用组名作为 motionLabel 取值。具体消费方式由 M7/M8 实现时确定。

**处理规则**：
- motionLabel 取值首先查 mao_pro 别名表 → 命中则用对应 Android 键；未命中则查 shizuku 组名 → 命中则用组名。
- 均未命中 → motionLabel = null（不报错，走关键词兜底）。
- [UNSURE-1] 别名→文件语义映射：表结构/文件→键对应关系已锁定（确定面），语义列为建议值。待 M7/M8 实现时在模型上肉眼验证，或用户直接拍板后回填本表。

---

## G. 变更记录

| 版本 | 日期 | 变更 | 说明 |
|---|---|---|---|
| v1.0 | 2026-08-08 | 初始版本 | 字段锁定自 architecture.md §3.1 + contract-M1-shared-protocol.md §1；A~G 七段完整；别名映射表含 [UNSURE-1] 标注 |
