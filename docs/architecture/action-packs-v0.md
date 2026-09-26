# 动作包 v0（包 = 五官 + 小幅头身；intensity 是一等公民）

> ## ⚠ v1 已冻结：现有 preset 包 = `expression` 字段的**首版**；`body` / `head` 由字段化通道承担
>
> **v1 唯一真源 = [performance-protocol-v1.md](performance-protocol-v1.md)**（2026-09-26 契约冻结）。
> 对本文的三条定位（**不删现有包、不改现有 JSON**）：
>
> 1. **现有 `kind=expression` 包（`smile` / `unhappy` / `surprised`）= v1 `expression` 字段的首版映射**
>    （§1 的五官通道子集）；v1 的 `expression.id` 指向的就是它们。
> 2. **v1 的 `expression` 字段只写五官（V3）**——现有表情包自带的**小幅头身分量**
>    在 v1 字段化通道下**不由 `expression` 下发**；头身姿态改由 `body` / `head` 字段承担。
> 3. **`body` / `head` 由字段化通道承担**（V2）：本文 §1 的「小幅 Angle/Body 位移」
>    与 §5 的双槽规则，是字段化通道落地前的**现有实现**；v1 三字段 ↔ 通道对照见
>    [performance-protocol-v1.md](performance-protocol-v1.md) §5.5。
>
> **本文其余内容（intensity 语义、幅度标定、单一真源、旧 id 删除）继续有效**；
> 与 v1 冲突处**以 v1 为准**。

> 状态：2026-09-23，分支 `mod/l1-product`。本文是**动作包**（旧称「预设」）的范围与语义
> 真源对齐页；参数真源是 `assets/actions/presets.json`，展示名真源是
> `assets/actions/preset_labels.json`，运行时校验在 `crates/l2d-wasm-demo/src/preset/`。
> **不 bump / 不 push / 不打 tag。**

## 1. 什么是一个「包」

一个包 = **一段可点名的表演**，由两部分组成：

1. **五官通道**：`ParamMouthForm` / `ParamEyeLSmile` / `ParamEyeRSmile` /
   `ParamEyeLOpen` / `ParamEyeROpen` / `ParamBrowLY` / `ParamBrowRY`；
2. **小幅 Angle/Body 位移**：`ParamAngleX/Y/Z` + `ParamBodyAngleX/Y/Z`。

按 `kind` 分两族（`kind` 同时决定**槽位**，见 `5）：

| kind | 名称 | 时长 | 头 / 身量级 | 语义 |
| --- | --- | --- | --- | --- |
| `expression` | 表情包 | 约 2.6s 静态保持 | 头 ≤12、身 ≤4（身约头的 1/3） | 情绪表情 + 一点点头身姿态 |
| `motion` | 手势包 | 约 0.9~1.0s 包络起落 | 头 ≤30、身 ≤10 | 点头 / 摇头 / 看 / 歪头 |

**红线**（解析期由 `pack_limit` 执行 + 回归钉住）：

- 表情包**必须有**五官 + 小幅头 + 小幅身（缺任一 → 加载告警）；
- 表情包**不得**是「±24 的抽帧大姿态」——那是手势包的量级；
- 手势包**不得**改眉毛（表情归表情，动作归动作）；
- 两族都**不得**碰手臂 / 手指 / 特效 / 口型（`ParamMouthOpenY` 归 TTS 口型）/ motion3 / exp3。

## 2. 合并表（v2 → v3）

v3 把「同一个表情的静态版与动作版」合成一条包，主 allowlist 由 19 条收到 10 条
（含 `none`）。**v2 的 12 条旧 id 已在 2026-09-23 整体删除**（见 `8`）——不保留别名，
本文也不再列出它们。

- sad / angry 收进 `unhappy` 的 intensity morph（1 = 难过相、3 = 生气相）；
- happy 的静态版与动作版收进 `smile`；surprised 的两版收进 `surprised`；
- 点头 / 摇头的强弱档收进 `nod` / `shake` 的 intensity；
- 左右看（`look_left` / `look_right`）与左右歪头（`tilt_left` / `tilt_right`）方向与
  intensity 正交，各自保留、**不合并**。

新包共 9 条 + `none`：`smile` / `unhappy` / `surprised` / `nod` / `shake` /
`look_left` / `look_right` / `tilt_left` / `tilt_right`。

## 3. intensity 语义（0..3，一等公民）

- **普通包**：`最终值 = 表值 × intensity × 通道倍率 × 包络`（表情包络恒 1）。
  五官与头身**一起**缩放；intensity=1 就是表值本身。
- **morph 包**（目前只有 `unhappy`）：表值是 **low 极**，`morph.high_params` 是
  **high 极**，intensity 在两极之间**线性插值**（不再二次相乘）：
  - `intensity ≤ low(=1)`：从中性（0）线性升到 low 极；
  - `1 < intensity < 3`：low ↔ high 线性过渡；
  - `intensity ≥ high(=3)`：取 high 极。
- 表演层 `PerformanceCue.intensity` 的合法区间是 `1..=3`，与两极锚点对齐；
  调试面板两条滑条都是 `0.5..3.0`（「基础表情强度」缺省 1.0、「动作强度」缺省 1.2）。

### unhappy 三档

| intensity | 相 | 眉 `ParamBrowLY` | 眼 `ParamEyeLOpen` | 头 `ParamAngleY` | 身 `ParamBodyAngleY` |
| --- | --- | --- | --- | --- | --- |
| 1 | 难过 | +0.4（上挑） | 0.65（垂眼） | -8（低头） | -3 |
| 2 | 过渡 | -0.275 | 0.875 | -3 | -1.15 |
| 3 | 生气 | -0.95（下压） | 1.1（瞪眼） | +2（略抬） | +0.7 |

**眉与头角在两端异号、眼开合异档**——这是「@1 与 @3 可辨」的机器证据
（回归 `unhappy_morphs_sad_low_to_angry_high`）。

## 4. 与 N.E.K.O 外显五情的对照

N.E.K.O 的 `OUTWARD_EMOTION_ANALYSIS_PROMPT` 只分 5 情
（happy / sad / angry / surprised / neutral），本项目用 **unhappy 一条包 + intensity**
覆盖它的 sad 与 angry 两情：

| N.E.K.O 五情 | 本项目的包 | 说明 |
| --- | --- | --- |
| happy | `smile` | 五官微笑 + 小幅抬头 |
| sad | `unhappy`（intensity 1） | 眉上挑 / 垂眼 / 低头 |
| angry | `unhappy`（intensity 3） | 眉下压 / 瞪眼 / 略抬 |
| surprised | `surprised` | 瞪眼 + 挑眉 + 小幅后仰 |
| neutral | `none`（撤销哨兵） | 不投递任何包 |

源出处见 `preset_labels.json` 的 `sources`（N.E.K.O / Cubism / VTube Studio / soullink）。

## 5. 双槽：表情与手势可同轮

> **v1 对照（2026-09-26）**：双槽是 `preset_id` 时代的实现。v1 的三字段里，
> `expression` 对应**五官**（≈ Face 槽的五官部分），`body` / `head` 对应**头身**
> （≈ Gesture 槽）。v1 的 `expression` **不再携带头身**，因此「手势赢共享通道」这条
> 规则在 v1 下只对 `body` 与 `head` 的互斥通道有意义——细则见
> [performance-protocol-v1.md](performance-protocol-v1.md) §4 / §5.5。**现有双槽实现不删**。

运行时（`PresetRuntime`）有**两个槽**：

- `PresetSlot::Face`：`kind=expression`；
- `PresetSlot::Gesture`：`kind=motion`。

规则：

1. **同轮并存**：`smile` + `nod` 可以同时活（微笑 + 点头）——HUD 行
   `preset: face=smile gesture=nod face_intensity=1.00` 两栏同时有值；
2. **分槽撤销**：同槽换包只清**该槽上一条**的参数，另一槽不动；`none` 两槽一起清；
3. **共享通道由手势赢**：同帧写入顺序是 Face → Gesture，所以 `ParamAngleY` /
   `ParamBodyAngleY` 这类两边都写的通道，以手势包为准（否则点头会被表情的小幅抬头盖掉）；
4. **到点按槽撤销**：表情槽与手势槽各自按 `ttl_ms` 撤销，互不牵连。

表演层/规则回退可以一轮给两条 cue（`PresetTable::resolve_slots`）：intent → 手势，
emotion → 表情，同一槽只保留一条。

## 6. 单一真源

| 内容 | 真源 | 消费者 |
| --- | --- | --- |
| 包的参数 / morph 两极 | `assets/actions/presets.json` | 渲染面（`/actions/presets.json`）、Rust 回归 |
| id → 展示名（中/英）/ 通道 | `assets/actions/preset_labels.json` | Flutter 调试面板、导演 Select、Rust include_str |
| id 契约（主 allowlist） | `live2d-ai-mod-director::presets::PRESET_IDS` | 面板 Select、二路能力集、表演层可见集合 |
| 运行时校验 / 红线 | `crates/l2d-wasm-demo/src/preset/table.rs` | 加载期（未知通道丢弃 + 告警，幅值分档钳位） |

**不要在 Dart / Rust 再手写一套中文标签，也不要再抄一份 id 列表。**

## 7. 怎么验

- **机器**：`cargo test -p l2d-wasm-demo`（预设回归）、
  `cargo test -p live2d-ai-mod-director`（包表 + 双槽规则）、
  `cargo test -p live2d-ai-desktop`（表演层能力集 = accepted_preset_ids）；
- **表情调试**（开发模式；**只走 Face 槽**）：只列 `smile` / `unhappy` / `surprised`
  三个表情包，不混手势；滑条「**基础表情强度**」缺省 **1.0**（= 表值本身）。选
  `unhappy`，把强度分别放 **1 / 2 / 3**：眉从**上挑**变**下压**、眼从**半闭**变
  **瞪大**、头从**低垂**变**略抬**。「**表情扫一遍**」逐条演一遍。
- **动作调试**（开发模式；**只走 Gesture 槽**）：只列 `nod` / `shake` / `look_left` /
  `look_right` / `tilt_left` / `tilt_right`；滑条「**动作强度**」缺省 **1.2**；另有
  「**叠加基础表情**」开关——播放手势时先把当前基础表情包 + 基础强度写进 Face 槽，
  再写 Gesture 槽（Face → Gesture，与渲染面同帧写入顺序一致，共享通道由手势赢）。
  「**动作扫一遍**」会带上当前基础表情。
- **HUD 权威行**：`preset: face=<id> gesture=<id> face_intensity=<倍率> src=<来源> <剩余ms>ms | scale h../b../e..`；
  没有活动预设时是 `preset: face=- gesture=- face_intensity=- | …`。这一行是「动作到
  底有没有驱动到模型」最直接的肉眼证据（`face_intensity` 专门用来确认「基础表情强度」
  滑条真的走到了渲染面）。
- **同轮双槽**：先点 `smile` 再点 `nod`，两条应同时演（HUD 两栏都有值），
  `归零（none）`一次清两个槽。

## 8. 旧 id：已删除

2026-09-23 起，`presets.json` 顶层的 `deprecated`、渲染面内建别名表、导演的旧 id
表与相关一致性回归**已整体删除**。v2 的旧 id 现在是**未知 id**：

- **渲染面**：未知 id → `PresetCommand::Ignore`——**不动任何参数**、不回执；
- **导演**：配置解析（`is_known_preset`）只认主 allowlist，手填旧 id **回落缺省**；
- **表演层**：能力集 = `accepted_preset_ids()` = 主 allowlist，旧 id **过不了校验**；
- **展示名**：`preset_labels.json` 不再有旧 id 条目（旧 id 原样显示稳定码，不谎报
  中文名）。

## 9. 明确不做

- 不再堆**平行的情绪短剧包**（同义包一律合并，见 `2）；
- 表情包不做**手势包量级**的大姿态；
- 不以 **motion3** 为主路径（本表只做 `ParamAngle*` / `ParamBodyAngle*` / 五官参数）。

## 10. 与实现的偏差（2026-09 实测）

> 本节**原样引用**调研报告 `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`
> 的 §3.2（与项目自己的 `director-rfc.md` 的 RFC 冲突表）与 §3.4（脱轨清单），
> **未改写一行**。引用只作索引，**不在本文下架构裁决**。
> ⚠ §3.4 的 **D1 那一行已被维护者 2026-09-21 裁决撤回**（下表该行自带 `~~D1~~ **已撤回**`
> 标记与撤回理由）——**不得只引 D1 的结论**；同一份报告 §3.6 还撤回了由 R2 推出的
> 「导演属场景 Mod / 主链不该有第二 LLM」，见其 §3.7 的追加澄清。引用时三处（§3.2 / §3.4 / §3.6–3.7）要一起读。

### 10.1 RFC 冲突（RESEARCH §3.2，原样引用）

| RFC 原文 | 位置 | 现行实现 |
| --- | --- | --- |
| 「**红线**：主链**不新增**情绪字段、不新增情绪事件、不为导演改 `ConversationConfig` / `PersonaSettings` / **引擎**；导演**不得**起第二个 LLM 客户端」 | `director-rfc.md:125-130` | C 就在**引擎**里（`conversation/engine.rs:404-417` 调 `perf.resolve(...)`），且是**主链自己的**第二个 LLM 客户端 + 独立 `[performance] base_url/api_key_env` |
| 「第二 LLM 调用做情绪分类 … **不做**」（理由：复制核心网络层、多一份端点与密钥路径、一轮多一次延迟与费用） | `director-rfc.md:169` | 实现里现在有**两份**第二 LLM（B 和 C） |
| 长期非目标：「第二 LLM 调用」「把导演做成**第二个 LLM/TTS 端点**的权威（端点权威永远只有 `[tts]` / `[llm]`）」 | `director-rfc.md:425-426` | `[performance]` 就是第三个端点 + 第三份 `api_key_env` |
| 导演的**回复侧**情绪证据是「**补充**证据」（用户问「今天很累」vs 模型回「那早点休息」）；骨架「**不订阅** `TextDelta`」；」回复侧情绪证据因此是**不完整**的」 | `director-rfc.md:119`、`419`、`502-503` | A **只**看用户侧，回复侧从未接上；缺口没补，反而在 C 里另起一条 |
| 「core 动作 / 表演状态 = **无驱动方**（休眠）；导演**禁止**」 | `director-rfc.md:389` | 动作包（表情+手势）经 `preset` 协议直接驱动渲染面参数，**绕过 core 仲裁** |

### 10.2 脱轨清单（RESEARCH §3.4，原样引用）

| # | 脱轨点 | 证据 | 建议方向 |
| --- | --- | --- | --- |
| ~~D1~~ **已撤回** | ~~「分析对象反了」：N.E.K.O 用角色回复，我们判的是用户输入~~ | 见 §3.6 | **撤回**。维护者 2026-09-21 裁决：本项目是「皮套 + AI 接入的**底座**，由 Mod 分化场景」，输入=**用户输入**是刻意设计。此行的原建议（改判角色回复）作废 |
| D2 | **主链第二 LLM 改写说辞**（C 的 `speak`）：N.E.K.O 只用 LLM 贴标签，说辞一字不动 | `performance/prompt.rs:16-29`、`engine.rs:398-490` | 若保留 C：让它**只出 cues**，`speak` 必须被丢弃（与 B 的「逐字勿改」对齐） |
| D3 | **一套系统两个第二 LLM**（B 与 C），契约互相矛盾（B「逐字勿改 cue-only」vs C「整理成真正要说的话」） | `mod-director/src/staging.rs:90-100` vs `performance/prompt.rs:16-29` | 二选一，另一个降级为纯规则 |
| D4 | **端点权威从 2 个变 3 个**（`[llm]` / `[tts]` / `[performance]`），RFC §7.2 明令禁止 | `live2d-ai.toml:76-81` | 若确需第二个模型，先过 RFC §5.2 的核心评审清单 |
| D5 | **两条前端应用通道同时驱动**，且「同轮只有一个 cue 产者」的保证**只覆盖了 `action_cue` 一条** | `main.dart:417-425`（按句 cue）与 `466-492`（状态面拉取）；`supervisor/turn.rs:195-198` 只闸 `SentenceReady`；`text_delta` 帧由 `handlers.rs:111-125` **无条件**发出 | 状态面拉取这条也要按「谁主谁退」闸掉 |
| D6 | **表情与手势错位配对**（emotion→表情、**intent**→手势），N.E.K.O 是同一个 emotion 名查两张表 | `presets.rs:159-178` | 手势也应从 emotion（或明确的 performance plan）来 |
| D7 | **表情包抢占头角通道**，N.E.K.O 明确把 `ParamAngle*` 排除在表情之外 | `presets.json` vs `live2d-emotion.js.recordInitialParameters` | 要么把表情包的头身位移去掉，要么在文档里承认这是与 N.E.K.O 的有意分歧 |
| D8 | **表情 2.6s 自动撤 vs N.E.K.O 跨轮保持**——两套语义混用，用户看到「表情自己消失 / 老表情赖着」 | `preset/mod.rs:157` vs N.E.K.O §3.3.7 | 定一条，写进 `action-packs-v0.md` |

---

## 11. 幅度标定（2026-09-24 重标定）

> **数值真源**：`crates/l2d-wasm-demo/src/preset/scales.rs` 顶部注释（本节每个数字都与它逐字一致，不另立一套）。
> 出厂倍率另有一份住在 `live2d-ai-runtime::settings` 与 `live2d-ai.toml` 的 `[action]`；渲染面这份是**兜底**。

### 11.1 公式 / 上限 / 倍率

```
最终值 = clamp_to_channel(表值 × 峰值系数 × intensity × 通道倍率)
```

- **上限是死的（未变）**：头 `ParamAngle*` = **30** / 身 `ParamBodyAngle*` = **10** /
  五官（口 / 眉 / 眼 / 其它面部）= **4**。（表情包的 12 / 4 是**解析期** `pack_limit`，不是运行期上限。）
- **有两个乘法旋钮**：`intensity`（调试面板 0.5–3.0；表演层 cue 合法区间 `1..=3`）与
  **通道倍率**（滑条 `[0.2, MAX_SCALE]`）。
- **出厂倍率**：`head 0.75 / body 0.80 / expression 1.0`。
- **倍率量程**：`[0.2, 2.2]`——`MAX_ACTION_SCALE` 由 **2.5 → 2.2**（旧上限下 body 滑条对
  `nod` / `shake` / `look_*` 在 1.43 以上完全无效、出厂 1.4 已吃掉 96–98% 行程，见 RESEARCH §2.1）。
- **峰值系数**：Single = `1.0`；`shake` 的 Oscillate(`cycles=2`) = **0.9285**（回归实测）。

### 11.2 口径：每个旋钮**单独**走满都不触上限

**不可能**要求「两个旋钮同时拉满也不触上限」：那要求 `表值 ≤ 上限 / (3 × 2.2) ≈ 4`，
默认摆幅会小到看不见。本轮验收口径是**每个旋钮独立走满都不触上限**（留 5% 余量）：

- ① **出厂组合**：`|表值 × 峰值 × 出厂倍率| ≤ 上限 × 0.60`；
- ② **scale 旋钮走满**：`|表值 × 峰值 × MAX_SCALE| ≤ 上限 × 0.95`；
- ③ **intensity 旋钮走满（方案 (i)：下调表值）**：`|表值 × 峰值 × 3 × 出厂倍率| ≤ 上限 × 0.95`。

（morph 包 `intensity=3` 取 high 极、**不乘 3**；③ 里一律乘 3 是**保守**口径。回归：
`factory_scales_stay_below_sixty_percent_of_every_channel_limit` /
`max_scale_knob_alone_never_reaches_the_limit` / `intensity_knob_alone_never_reaches_the_limit`。）

### 11.3 两个旋钮**同时**拉满：会钳位的组合（T3）

`intensity = 3` 且 `scale = 2.2` 时，**普通包会钳位，morph 包不会**——这是刻意接受的代价
（口径是 §11.2 的「独立走满」，不是「整个二维网格都不触上限」）。下面这张组合表**原样**
来自 `scales.rs` 顶部注释（回归 `both_knobs_at_max_clamp_only_the_documented_combinations` 钉住）：

```
| 包 | 通道 | 拉满需要 | 上限 |
| --- | --- | --- | --- |
| nod / shake（主轴 ×0.9285） | ParamAngleX / ParamAngleY | 73.5 | 30 |
| look_left/right / tilt_left/right（主轴 ×1.0） | ParamAngleX / ParamAngleZ | 79.2 | 30 |
| 同上四条主轴的身侧 | ParamBodyAngleX / ParamBodyAngleZ | 23.9(shake) / 25.7 | 10 |
| smile | ParamMouthForm / ParamEyeLSmile / ParamEyeRSmile | 6.6 | 4 |
| smile | ParamAngleY / ParamBodyAngleY | 39.6 / 13.2 | 30 / 10 |
| surprised | ParamEyeLOpen / ParamEyeROpen / ParamBrowLY / ParamBrowRY | 8.316 / 5.28 | 4 |
| surprised | ParamAngleY / ParamBodyAngleY | 33.0 / 13.2 | 30 / 10 |
```

按包展开，共 **23 组** `(包, 通道)`（顺序与回归的期望集一致；`拉满需要` 与上表同源，
nod 主轴属 Single ×1.0 档、值与「look/tilt」行的 79.2 相同，回归实测 79.200）：

| # | 包 | 通道 | 拉满需要 | 上限 |
| --- | --- | --- | --- | --- |
| 1 | `nod` | `ParamAngleY` | 79.2 | 30 |
| 2 | `nod` | `ParamBodyAngleY` | 25.7 | 10 |
| 3 | `shake` | `ParamAngleX` | 73.5 | 30 |
| 4 | `shake` | `ParamBodyAngleX` | 23.9 | 10 |
| 5 | `look_left` | `ParamAngleX` | 79.2 | 30 |
| 6 | `look_left` | `ParamBodyAngleX` | 25.7 | 10 |
| 7 | `look_right` | `ParamAngleX` | 79.2 | 30 |
| 8 | `look_right` | `ParamBodyAngleX` | 25.7 | 10 |
| 9 | `tilt_left` | `ParamAngleZ` | 79.2 | 30 |
| 10 | `tilt_left` | `ParamBodyAngleZ` | 25.7 | 10 |
| 11 | `tilt_right` | `ParamAngleZ` | 79.2 | 30 |
| 12 | `tilt_right` | `ParamBodyAngleZ` | 25.7 | 10 |
| 13 | `smile` | `ParamMouthForm` | 6.6 | 4 |
| 14 | `smile` | `ParamEyeLSmile` | 6.6 | 4 |
| 15 | `smile` | `ParamEyeRSmile` | 6.6 | 4 |
| 16 | `smile` | `ParamAngleY` | 39.6 | 30 |
| 17 | `smile` | `ParamBodyAngleY` | 13.2 | 10 |
| 18 | `surprised` | `ParamEyeLOpen` | 8.316 | 4 |
| 19 | `surprised` | `ParamEyeROpen` | 8.316 | 4 |
| 20 | `surprised` | `ParamBrowLY` | 5.28 | 4 |
| 21 | `surprised` | `ParamBrowRY` | 5.28 | 4 |
| 22 | `surprised` | `ParamAngleY` | 33.0 | 30 |
| 23 | `surprised` | `ParamBodyAngleY` | 13.2 | 10 |

**不钳位**（同一口径下的反向读数）：全部手势次轴（`shake` Z 13.5、`look` Z 17.8、`tilt` X 19.8、
身次轴 ≤5.3）；`smile` 的 Brow（3.3）与 AngleZ（13.2）；`surprised` 的 MouthForm（1.32）、
AngleX（19.8）、BodyAngleX（6.6）；**`unhappy` 全部通道**（最大 6.6）。

### 11.4 主轴死区起点：全部 > 2.2 ⇒ 滑条全行程有效

`scales.rs` 顶部注释原文（口径 = 上限 × 0.95 / (表值 × 峰值)）：

```
峰值在手势主轴上给出的**死区起点**（= 上限 × 0.95 / (表值 × 峰值)，再拖就钳死）：
  head 主轴 `ParamAngleX/Y/Z` = 12.0 → 2.375（look/tilt）/ 2.558（nod/shake，含 0.9285）
  body 主轴 `ParamBodyAngleX/Y/Z` = 3.9 → 2.436（Single）/ 2.623（shake）
  五官 `ParamEyeLOpen/ROpen`（surprised）= 1.26 → 3.016
全部 > MAX_SCALE(2.2)：**滑条全行程有效**。
```

按「钳死点（**未留余量**）= 上限 / (表值 × 峰值)」与「验收线（留 5%）= 上限 × 0.95 / (表值 × 峰值)」
两种口径并列（RESEARCH §2.1 的口径说明：两者相差 5% 是刻意的，核对时按同一口径对同一口径）：

| 主轴（示例参数） | 峰值 | 钳死点（超过它就钳死） | 验收线（scales.rs 原文口径） |
| --- | --- | --- | --- |
| head 12.0（`nod`/AngleY、`look`/AngleX、`tilt`/AngleZ） | 1.0（Single） | **2.500** | 2.375 |
| head 12.0（`shake`/AngleX） | 0.9285（Oscillate） | **2.693** | 2.558 |
| body 3.9（`nod`/BodyY、`look`/BodyX、`tilt`/BodyZ） | 1.0（Single） | **2.564** | 2.436 |
| body 3.9（`shake`/BodyX） | 0.9285（Oscillate） | **2.762** | 2.623 |
| 五官 1.26（`surprised` EyeOpen） | 1.0（Single） | **3.175** | 3.016 |

**全部 > `MAX_SCALE`(2.2)**：倍率滑条**全行程有效**，没有「拖到一半就没效果」的上半段。
回归 `dead_zone_starts_are_printed_for_every_channel` 逐通道打印并断言 ≥ `MAX_SCALE`。

### 11.5 身 / 头比（六条手势包主轴）

主轴配对：`nod` = AngleY/BodyY、`shake` = AngleX/BodyX、`look_*` = AngleX/BodyX、
`tilt_*` = AngleZ/BodyZ；表值 = 头 12.0 / 身 3.9。

- **表内 0.3250**（六条手势包一致）；
- **出厂倍率后 0.3467**（= 0.3250 × 0.80/0.75），落在设计口径 `[0.30, 0.50]` 内；
- `tilt_*` 的**表内旧值 0.25（4/16）本轮已修到 0.3250**（旧值本来就不达标，不是倍率造成的）。

回归 `gesture_main_axis_body_head_ratio_returns_to_the_design_window`（同时覆盖内建 fallback 表与外置 JSON）。

### 11.6 `morph` 包不参与 intensity 二次相乘（一次说清）

`morph` 包（目前只有 `unhappy`）的 `intensity=3` **直接取 high 极、不再乘 3**。因此
**两个旋钮同时拉满时 `unhappy` 一个通道都不钳位**（该包拉满后的最大通道值 6.6 < 上限）。
这与 §11.3「普通包会钳位」并存，不是矛盾：普通包与 morph 包走的是两条 intensity 语义
（详见 §3）；回归对 `unhappy` 在 T3 里单独断言「不该钳位」。

### 11.7 每皮套覆盖的落点（阶段5 D40，2026-09-26）

**用户旋钮住 `live2d-ai.toml`，不住映射表**：

```toml
[action]                 # 全局默认（三键）
head_scale = 0.75
body_scale = 0.80
expression_scale = 1.0

[action.models.bai]      # 可选：某皮套的覆盖；三键各自可选
head_scale = 1.2         # 只有这一键被覆盖
                         # body / expression 缺省 → 逐键回落全局
```

- **生效值 = 本模型覆盖 > 全局（逐键）**：`<model_id>` 用当前 `active_model_id`（`GET /api/v1/settings` 的 `active_model_id` 字段）；
- **量程与钳位**沿用 §11.1 的 `[0.2, 2.2]`（服务端 `clamp_action_scale` 逐键钳）；
- **写回**走 `merge_into_toml` 的**就地改值**（保留注释）+ 热重载；删除某模型覆盖 = 把该键置 `null` / 从表里移除；
- **渲染面零改动**：前端按 `active_model_id` 算好三项有效值，仍经既有 `ActionScalesSyncer` → `sync.actionScales` 下发；
- **映射表分工不变**：`crates/l2d-wasm-demo/src/preset/scales.rs`（数值真源）与可选 `assets/actions/field_map.json` 只管**每通道峰值 / 方向**——它们**不是**用户旋钮，不得往里写用户设置。



