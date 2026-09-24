# 动作预设：展示名映射与表情可辨性调研（2026-09）

> **⚠️ 2026-09-23 起已被 v3 取代**：18 条包合并为 9 条（sad+angry → `unhappy` 按
> intensity morph 等），旧 id 变成 deprecated 别名一版。**现行契约见
> [`docs/architecture/action-packs-v0.md`](../architecture/action-packs-v0.md)**；
> 本文保留为当时的调研快照（展示名映射机制与 sad/angry 可辨性论证仍然有效）。

> 范围：动作预设（18 条 id + none 哨兵）的**展示名唯一真源**，以及 sad / angry
> 两条表情的**可辨性**改造。
>
> 产出物：
> - 映射表：`assets/actions/preset_labels.json`（唯一真源）
> - 消费方：Flutter 调试面板（`GET /actions/preset_labels.json`）、Rust 导演
>   Select（`include_str!`）、本文件
> - 参数改动：`assets/actions/presets.json` + `crates/l2d-wasm-demo/src/preset.rs`
>   内建 fallback 的 `expr_sad` / `expr_angry`
>
> 红线：Face / Head / Body 标准参数，**无手臂 / 无特效 / 无 motion3 / 不改表演层 schema**。

## 0. 结论摘要

1. **N.E.K.O 的外显情绪是固定的五类**：happy / sad / angry / surprised / neutral，
   由 LLM 按提示词判成一条 JSON（`{"emotion": ..., "confidence": ...}`），运行时
   经 `setEmotion(name)` **双通道**（expression + motion）应用，单侧缺失优雅降级。
   我们的 4 条表情预设 + none 正好一一对齐这五类。
2. **Cubism 官方只规范「参数名」，不规范「表情名」**。标准参数表给出
   `ParamBrowLY`、`ParamEyeLOpen`、`ParamMouthForm`、`ParamAngleY` 等稳定
   id；而 `.exp3.json` / `.motion3.json` 的**文件名就是约定**（Hiyori / 官方示例
   模型里常见 Smile / Angry / Sad 等）。所以「稳定 id 归我们、展示名归表」是唯一
   不会漂移的做法。
3. **VTube Studio 明确不提供固定表情词汇**：表情是模型目录里的 `.exp3.json`，
   由「Hotkeys」绑定；参数可设 overwrite / add / multiply。也就是说同类项目里
   **没有一套可照抄的中文标签**——只能自建一份并集中维护。
4. 因此本轮把 19 个展示名收进 **一张 JSON**，Rust 与 Dart 都读它；
   **禁止再在 Dart / Rust 手写第二套中文标签**（导演 Select 的旧硬编码已删除）。

## 1. N.E.K.O（猫娘计划，Project-N-E-K-O）：外显五情 + setEmotion

**五情**（原文，`config/prompts/prompts_emotion.py` 的
`OUTWARD_EMOTION_ANALYSIS_PROMPT`，中/英/日/韩/繁中五语同构）：

| 外显情绪 | 中文提示词里的语义（节选） |
| --- | --- |
| happy | 开心、兴奋、满足、轻快、宠溺、可爱、调皮、得意、热情 |
| sad | 失落、难过、委屈、沮丧、低落、遗憾、脆弱 |
| angry | 生气、不满、烦躁、攻击性、强烈指责、炸毛 |
| surprised | 惊讶、震惊、意外、被逗到、夸张感叹、强烈新奇感 |
| neutral | 平静、陈述事实、情绪很弱、难以判断 |

- 输出契约：**只回 JSON** `{"emotion": "...", "confidence": 0..1}`，且明确要求
  「优先选最强主情绪，不要轻易 neutral」。
- 来源：`https://github.com/Project-N-E-K-O/N.E.K.O`
  （`config/prompts/prompts_emotion.py`）。

**Live2D setEmotion 用法**（`static/live2d/live2d-emotion.js`）：

- 运行时维护「情感 / 表情 / 动作」三块：切换表情、设置情感、**常驻表情**管理；
- 应用表情时**记录模型初始参数**，并把 `ParamAngleX/Y/Z` 与口型参数
  （`ParamMouthOpenY` / `ParamMouthForm` / `ParamA..O`）**排除在重置之外**——
  表情重置不能把头部朝向与口型压掉；
- 抖动/淡入用毫秒级常量（软重置 220ms、软表情淡入 220ms），并区分
  `motionBaselineParameters` / `appearanceBaselineParameters`；
- 与我们项目的关系：**同样的分层职责**——表情层不抢口型与头角（我们的预设写
  `final_override`，口型仍走 input 层；见 `crates/l2d-wasm-demo/src/preset.rs`
  的 P0-2 说明）。

**N.E.K.O 运行态侧**（本仓既有调研，非本轮新验）：
- `EmotionController.kt` 把 8 个关键词（neutral/fear/sadness/anger/disgust/joy/
  smirk/surprise）映射到索引，`setEmotion` 立即硬切；来源
  `docs/research/core-fusion-research.md` §「表情映射」。
- 表情/动作走 Cubism 标准 `FileReferences.Motions/Expressions` 分组 +
  **EmotionMapping 编辑器**；运行时 `setEmotion(name)` 双通道，单侧缺失优雅降级；
  来源 `docs/research/live2d-halfbody-motion-research.md` §五。

## 2. Live2D Cubism：标准参数与 Expression 命名习惯

官方「标准参数列表」只规范**参数 id**（我们允许的通道全部来自它）：
`ParamAngleX/Y/Z`、`ParamBodyAngleX/Y/Z`、`ParamEyeLOpen/ROpen`、
`ParamEyeLSmile/RSmile`、`ParamEyeBallX/Y`、`ParamBrowLX/RX`、`ParamBrowLY/RY`、
`ParamBrowLAngle/RAngle`、`ParamBrowLForm/RForm`、`ParamMouthForm`、
`ParamMouthOpenY`、`ParamCheek`、`ParamBreath`、`ParamArmLA..`、`ParamHandL/R` 等。
来源：`https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/`。

**Expression 命名没有官方词表**：`.exp3.json` 是「一组参数目标值」的文件，
文件名由作者自取；官方示例 / 常见模型里高频出现的是 Smile、Angry、Sad、
Surprised 一类英文短名。**结论**：稳定 id 与展示名都必须由我们这边定，且只定一份。

## 3. 同类项目对照

### 3.1 VTube Studio（公开 Wiki，闭源应用）
- 表情 = 模型目录里的 `.exp3.json`（并可由 `.cdi3.json` 提供分组）；
- 通过「Hotkeys」绑定「Set/Unset Expression」触发，可手动开关、可「Clear all
  Expressions」；被标为 Model Customization Expression 的表情不会被清空；
- 每个参数可设 **Overwrite / Add / Multiply** 三种模式，Add/Multiply 栈最后结算；
- **没有固定表情名/中文名**——名字来自文件与作者。
- 来源：`https://github.com/DenchiSoft/VTubeStudio/wiki/Expressions-(a.k.a.-Stickers-or-Emotes)`。

### 3.2 Soullink Emotion SDK（开源，桌宠表情/动作引擎）
- 用 **Valence / Arousal / Dominance** 三维连续情绪 + **FACS/AU** 表情语义，
  再经 ModelProfile 映射成具体 Cubism 参数；Idle / Reaction / Speech / 情绪分层混合；
- 强调「不把角色简化成收到一句话切一个表情」，自动扫描模型参数并生成
  `soullink.profile.json` 做能力降级；
- 对我们的启发：**展示名与能力清单都属于「表/Profile」，不属于代码**。
- 来源：`https://github.com/nanlingyin/soullink-emotion-sdk`。

### 3.3 其它（本仓既有调研）
- Amica：表情名在运行时从已加载模型发现，LLM 漏标签时注入 `[neutral]`；
  来源 `docs/research/ui-design-survey-companion-2026-09.md`（§「表情」）。
- N.E.K.O：LLM 文本标签 → `emotion_mapping` JSON（`{"motions":{"happy":[...]}}`）
  → motion3/exp3；来源 `docs/research/neko-benchmark-report-2026-09-03.md`。

## 4. 映射表（稳定 id → 展示名）

真源：`assets/actions/preset_labels.json`（字段：zh / en / channel / canonical /
source / aligned / note）。`aligned=true` 表示能对齐 N.E.K.O 外显五情；
`aligned=false` 标注「本项目扩展」并给出规范英文短名。

| id | 中文 | 英文 | 通道 | canonical | 来源 |
| --- | --- | --- | --- | --- | --- |
| none | 中性 | Neutral | expression | neutral | neko:neutral（渲染面撤销哨兵） |
| expr_smile | 微笑 | Smile | expression | happy | neko:happy · vts:exp3 通用 Smile |
| expr_sad | 难过 | Sad | expression | sad | neko:sad |
| expr_angry | 生气 | Angry | expression | angry | neko:angry |
| expr_surprised | 惊讶 | Surprised | expression | surprised | neko:surprised |
| nod | 点头 | Nod | motion | — | 本项目扩展 |
| shake | 摇头 | Head Shake | motion | — | 本项目扩展 |
| look_left | 左看 | Look Left | motion | — | 本项目扩展 |
| look_right | 右看 | Look Right | motion | — | 本项目扩展 |
| bow_slight | 微鞠躬 | Slight Bow | motion | — | 本项目扩展 |
| tilt_left | 左歪头 | Tilt Left | motion | — | 本项目扩展 |
| tilt_right | 右歪头 | Tilt Right | motion | — | 本项目扩展 |
| agree_nod_double | 点头同意 | Nod Twice | motion | — | 本项目扩展 |
| deny_shake_strong | 摇头否认 | Strong Head Shake | motion | — | 本项目扩展 |
| shy_look_down | 害羞低头 | Shy Look Down | motion | — | 本项目扩展（soullink:FACS 眼睑/视线） |
| happy_bounce | 开心弹跳 | Happy Bounce | motion | happy | neko:happy（情感向短动作） |
| look_up | 抬头 | Look Up | motion | — | 本项目扩展 |
| ponder_tilt | 思索歪头 | Ponder Tilt | motion | — | 本项目扩展 |
| surprised_recoil | 惊讶后仰 | Surprised Recoil | motion | surprised | neko:surprised（情感向短动作） |

**「不能对齐」的诚实处理**：9 条短动作（nod / shake / look_* / tilt_* / bow /
look_up / ponder）在 N.E.K.O 里没有同名枚举，标 `aligned=false` 并给英文规范短名，
语义由 note 说明；我们**不**编造「某项目也叫这个名」。

## 5. 可辨性：sad ≠ angry

**问题**：改前两条几乎同构——都是「嘴负向 + 眉下压」，只有幅值差
（sad 眉 -0.6 / angry 眉 -0.9），肉眼扫一遍分不出来。

**典型通道差异**（依据 Cubism 标准参数含义 + FACS 直觉）：

| 通道 | sad（难过） | angry（生气） |
| --- | --- | --- |
| 眉 `ParamBrowLY/RY` | **上挑**（内眉抬） | **下压**（压眉瞪人） |
| 眼 `ParamEyeLOpen/ROpen` | 半闭（垂眼 0.65） | 瞪大（1.1） |
| 头 `ParamAngleY/Z/X` | 低头 -10 + 歪头 6 + 偏 -2 | 略抬 2 + 偏 3 |
| 身 `ParamBodyAngleY` | -3（微含胸） | -2 |
| 嘴 `ParamMouthForm` | -1.0（嘴角下垂） | -0.9（压平） |

**改前 → 改后**（`assets/actions/presets.json` 与内建 fallback 同步）：

| | 改前 | 改后 |
| --- | --- | --- |
| expr_sad | MouthForm -1.0；BrowLY/RY **-0.6**；EyeSmile 0 | MouthForm -1.0；BrowLY/RY **+0.4**；EyeL/ROpen 0.65；AngleY -10、AngleZ 6、AngleX -2；BodyAngleY -3 |
| expr_angry | MouthForm -0.8；BrowLY/RY **-0.9**；EyeSmile 0 | MouthForm -0.9；BrowLY/RY **-0.95**；EyeL/ROpen 1.1；AngleY +2、AngleX 3；BodyAngleY -2 |

**回归钉子**：`crates/l2d-wasm-demo/src/preset_tests.rs` 的
`sad_and_angry_differ_on_the_key_channels` —— 对内建 fallback **与外置表**分别断言：
眉形反号且差距 > 1.0、眼开合差 > 0.3、头角一低一抬、嘴都负向，并逐参数检查
「只碰允许通道 + 不超幅值分档」。

**其它易混项（本轮未改，列出观察）**：
- `look_left` / `look_right` 与 `tilt_left` / `tilt_right`：前者是水平转头
  （AngleX 为主），后者是侧倾（AngleZ 为主），参数已区分；扫一遍时靠动作方向可辨。
- `nod` / `agree_nod_double`：只有幅度差（20 vs 24），静态截图看不出，
  扫一遍时靠幅度与时长区分；若将来要求「一眼可辨」，可给后者加多周期。
- `shake` / `deny_shake_strong`：同上，靠幅度/周期区分。
- `expr_smile` / `happy_bounce`：一静一动，静态下前者是表情、后者带角度，已可辨。

## 6. 消费方与落盘位置

- **文件**：`assets/actions/preset_labels.json`（服务端静态路由 `GET /actions/preset_labels.json`；
  与 `presets.json` 同属 `/actions/*`）。`assets/` 不捆绑模型二进制，本表随仓库走。
- **Flutter**：`lib/settings/preset_labels.dart`（纯解析 + `display(id)`），
  调试面板「动作调试」的按钮显示 `中文（id）`；取不到表回落显示 id。
- **Rust**：导演 Mod 的 `preset_label` 改为 `include_str!` 读同一张表
  （进程内 OnceLock 缓存）；回归 `preset_select_labels_come_from_the_shared_table`。
- **覆盖守卫**：`preset_labels_json_covers_every_shipped_preset_id` 断言
  `presets.json` 里每条 id 都有中/英名，且标签表不得有幽灵 id（none 除外）。
- **文档**：本文件第 4 节的表是**引用**，不是第二份真源；改表只改 JSON。

## 7. 参考链接

- N.E.K.O 仓库：https://github.com/Project-N-E-K-O/N.E.K.O
- N.E.K.O 五情提示词：https://raw.githubusercontent.com/Project-N-E-K-O/N.E.K.O/main/config/prompts/prompts_emotion.py
- N.E.K.O Live2D 情感运行时：https://raw.githubusercontent.com/Project-N-E-K-O/N.E.K.O/main/static/live2d/live2d-emotion.js
- Cubism 标准参数列表（EN）：https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/
- VTube Studio Expressions Wiki：https://github.com/DenchiSoft/VTubeStudio/wiki/Expressions-(a.k.a.-Stickers-or-Emotes)
- Soullink Emotion SDK：https://github.com/nanlingyin/soullink-emotion-sdk
- 本仓既有：`docs/research/live2d-halfbody-motion-research.md`、
  `docs/research/core-fusion-research.md`、
  `docs/research/neko-benchmark-report-2026-09-03.md`、
  `docs/research/ui-design-survey-companion-2026-09.md`
