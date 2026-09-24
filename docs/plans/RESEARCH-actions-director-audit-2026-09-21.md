# 动作 / 表情 / 导演链路调研（4 项症状 → 根因 → 下一轮任务）

> 调研对象：工作树 `/home/skystar/Live2D-Ai-l1`，分支 `mod/l1-product`（178 个未提交改动）。
> 调研日期 2026-09-21（本机时钟）。注意分支内部分文档自带 `2026-09-23` 的日期，
> 比本机时钟**靠后**——引用时以文件名 + 章节为准，不要拿日期做先后判断。
> **本文件只做调研与范围界定，不含代码改动**；每一条结论都给出 `file:line` 或命令输出。

## 0. 一句话结论

| 症状 | 根因（一句话） | 性质 |
| --- | --- | --- |
| ① 动作测试附带旧表情 | 调试面板「叠加基础表情」**默认开**且 `_expressionId` **永久粘住**上次点的表情，每次点手势都会把那条表情**重新续期 2.6s** | 前端默认值 / UX 缺陷（状态机本身正确） |
| ② 幅度像「编译死了」、临时覆盖不生效 | ①出厂倍率（head 0.75 / body 1.4）把 shipped 手势的 `ParamBodyAngle*` 直接顶到 ±10 上限，body 滑条在 1.43 以上**完全无效**；②`Live2DStage.actionScales` 是**从未传过的死参数**；③临时覆盖与防抖下发器是**两个写者共用一个通道、各自去重**，任何 `DisplayPrefs` 变更（换主题等）都会**无条件冲掉**临时值 | 参数标定 + 接线缺陷（可量化、可复现） |
| ③ 导演系统脱轨 | ① 与项目**自己的** `director-rfc.md` 红线**正面冲突**：主链（runtime/引擎）内建了第二 LLM + 独立端点与密钥（RFC 明令禁止），并且**改写了主模型要说的话**（`speak`）；② 一套系统里并存**两份契约相反**的二路 LLM；③ 前端有**两条未协调的驱动通道**（见 §3.6 的裁决：**输入=用户输入是刻意设计，不是偏差**） | 架构脱轨（文档 ↔ 代码不一致 + 场景能力错放在底座里） |
| ④ 其它工程小错 | 见 §4（共 11 类，含 `join_endpoint` 4 份实现、多处注释漂移、Dart 侧又抄了一份 id 列表等） | 卫生问题 |

---

## 1. 症状①：动作测试会附带「之前的」老表情

### 1.1 实际行为（读了什么、发了什么）

`shell/flutter/lib/settings/sections/dev_tools_section.dart`：

| 位置 | 内容 |
| --- | --- |
| `1337` | `String _expressionId = 'none';` —— 「当前基础表情」是**面板自身的 State**，不跟渲染面同步 |
| `1346` | `bool _overlayFace = true;` —— 「叠加基础表情」**默认开** |
| `1400-1406` | `_applyGesture()`：`if (_overlayFace && _expressionId != 'none') onApplyPreset(_expressionId, _faceIntensity);` 然后才发手势 |
| `1483-1494` | 点表情按钮 → `setState(() => _expressionId = id)` —— **只写不清** |
| `1571-1573` | 唯一能清掉它的入口是「归零（none，两槽同清）」 |

所以：**在「表情调试」里点过一次 `smile` 之后**，此后每一次「动作调试」的手势点击都会先发一条 `smile`，把它的 2.6s ttl **重新计时**；手势本身只有 0.9s。用户看到的就是「点头/摇头总带着那张老脸」。

`shell/flutter/test/developer_section_test.dart:122-154` **明确把这条行为钉成了回归**（`动作调试：叠加基础表情开着时，先发 Face 再发 Gesture`）——所以它不是状态机 bug，是**默认值 + 粘滞 UI 状态**的产品判断问题。

### 1.2 渲染面状态机本身是对的（不要改这里）

`crates/l2d-wasm-demo/src/preset/mod.rs`：
- `handle()` `808-839`：同槽换包只清**该槽上一条**的通道（分槽撤销），`Revoke` 才两槽同清；
- `apply_frame()` `845-870`：先按 ttl 逐槽到点撤销，再按 `Face → Gesture` 顺序重写，所以「清掉共享通道后同帧又被另一槽写回」，**单帧抖动 ≤1 帧**。
- morph 包的 low 极解析正确（`table.rs:174-210` 把 `morph.low_params` 映射成 `spec.params`），`spec_channels()` 覆盖两极 → 撤销不会漏通道。

### 1.3 与 N.E.K.O 的口径差（这条是「老表情」的一半来源）

N.E.K.O 的**表情是设计上跨轮保持的**：`live2d-emotion.js:1412-1448` `clearEmotionEffects()` 明确「清除 motion 参数，**保留 expression**」；motion 到点只清 motion（`1249-1296`），expression 一直留到下一次 `setEmotion` 替换它（`1554-1572` 是唯一会清上一套表情的正常路径）。

也就是说：**「动作不该带表情」这件事，N.E.K.O 的答案是反过来的**——动作带表情是正常的，它只要求 motion 到点后表情**还在**。我们现在同时有两种语义（表情 2.6s 自动撤 + 面板叠加），才让「老表情」看起来像 bug。

### 1.4 产品路径上还有两个「清不掉」的洞

1. **`none` 不会撤销**：`main.dart:482-483` `if (preset is! String || preset.isEmpty || preset == 'none') return;` —— 中性/无预设时**什么都不做**；而 `presets.rs:166-176` 的 `resolve_slots` 也会把 `PRESET_NONE` 过滤掉。**当前没有任何产品路径会把「撤销」发到渲染面**，只能等 ttl 自然到点。
2. **空 `action_cue` 也不撤销**：`main.dart:421-425` 收到 `ActionCueEvent` 时 `_directorCues = {每条 cue}`；`cues=[]` → 空表 → `_applyDirectorCueForSeq` 直接 return。而 `performance-layer-v0.md` §2.2 写的是「noop … 发一份空 action_cue（**清上一轮残留**）」——**这句与代码不符**，空表什么也清不掉。

---

## 2. 症状②：幅度「像编译死了」+ 临时覆盖不生效

### 2.1 出厂倍率把 body 顶到上限，滑条大部分行程无效（**这是「编译死了」的主因**）

最终值公式（`preset/scales.rs:143-154`）：

```
最终值 = clamp_to_channel(id, 表值 × 包络 × 通道倍率)
上限：ParamAngle* = 30、ParamBodyAngle* = 10、其余（五官）= 4   （scales.rs:13-25）
出厂倍率：head 0.75 / body 1.4 / expression 1.0       （scales.rs:89-93、live2d-ai.toml [action]）
```

按 `assets/actions/presets.json` 实测（复刻 Rust 公式逐包计算，含 `shake` 的多周期峰值 0.9285）：

> **口径（2026-09-21 与编排者复算后钉死）**：本表的「死区起点」= `上限 / (表值 × 峰值)`，是**钳死点**（**未留余量**）；
> **验收线**用 W2 的 T1 定义 `上限 × 0.95 / (表值 × 峰值)`（**留 5% 余量**）。两者相差 5% 是刻意的，核对时**按同一口径对同一口径**。

| 包 | 通道 | 出厂值 | 上限 | **倍率死区起点**（超过就钳死，再拖无效果） |
| --- | --- | --- | --- | --- |
| `nod` | ParamBodyAngleY | −9.80 | 10 | **1.43** |
| `nod` | ParamAngleY | −15.00 | 30 | 1.50 |
| `shake` | ParamBodyAngleX | 9.75 | 10 | **1.44** |
| `shake` | ParamAngleX | 15.32 | 30 | 1.47 |
| `look_left` / `look_right` | ParamBodyAngleX | ±9.80 | 10 | **1.43** |
| `look_left` / `look_right` | ParamAngleX | ±16.50 | 30 | **1.36** |
| `tilt_left` / `tilt_right` | ParamBodyAngleZ | ±5.60 | 10 | 2.50（刚好等于滑条上限） |
| `tilt_left` / `tilt_right` | ParamAngleZ | ±12.00 | 30 | 1.88 |

滑条量程是 `0.2 … 2.5`（`settings_models.dart:236-239`）。于是：
- **body 滑条在 1.43 以上对 `nod` / `shake` / `look_*` 完全无效**，而**出厂值就是 1.4**（已经吃掉 96–98% 的行程）；
- head 滑条对 `look_*` 在 **1.36** 以上、对 `nod` 在 1.50 以上、对 `tilt_*` 在 1.88 以上无效 ⇒ 所有手势的 head 滑条**上半段（1.4→2.5）都有死区**。

### 2.2 出厂倍率还破坏了表内设计的「身约头的 1/3」

| 包 | 表内 身/头 | 出厂倍率后 身/头 |
| --- | --- | --- |
| `nod` | 0.35 | **0.65** |
| `shake` | 0.34 | **0.64** |
| `look_left` / `look_right` | 0.32 | **0.59** |
| `tilt_*` | 0.25 | **0.47** |

`action-packs-v0.md` §1 写的是「身约头的 1/3」，`preset/mod.rs:54-59` 写的是「身约头的 1/3~1/2」。**出厂配置（head 收 0.75、body 放 1.4）把身/头比翻了一倍多**——这正好解释「上下左右一按，动的是**上半身**」：躯干摆幅 ≈ 头的 2/3，而头本身还被收窄到 75%。

### 2.3 `Live2DStage.actionScales` 是**从未传过**的死参数

- 声明：`live2d/live2d_stage.dart:96` `final Map<String, double>? actionScales;`
- 构造点：`main.dart:709-717` 的 `Live2DStage(...)` **没有** `actionScales:`（注释明说「这里不再传静态快照」）；
- 于是 `_attach()` `live2d_stage.dart:287-294` 与 `didUpdateWidget` `205-207` 这两条「iframe 重建后补发」的通路**永远拿到 null**；
- 全仓 `shell/flutter/test/**` 对 `actionScales` **零引用**。

实际唯一通路是 `ActionScalesSyncer`（`main.dart:288-291` → `live2d_stage.dart:413-417 applyActionScales`）。也就是说：**文档里写的「产品设置是真源、重建会补发」在代码里只靠 host 的 `onReady: _applyPrefs` 兜着**，舞台自己那条防线是空的。

### 2.4 临时幅度覆盖被**两个写者 + 各自去重**冲掉（可复现）

通道只有一个（`sync.actionScales`），但有两个互不知情的写者：

| 写者 | 代码 | 去重状态 |
| --- | --- | --- |
| 产品/草稿值（防抖 150ms） | `shell_prefs.dart:38-45` → `action_scales_sync.dart:55-67` | 自己的 `_sent` 快照 |
| 调试面板的临时覆盖（直发） | `shell_settings.dart:254-261` → `live2d_stage.sync()` | **不经过 syncer，不更新 `_sent`** |

两条确定性缺陷：

1. **任何 `DisplayPrefs` 变更都会无条件冲掉临时值**：`shell_prefs.dart:99-114 _updatePrefs()` → `_applyPrefs(next)` → `91-92 _syncActionScalesNow(force: true)`；而 `force: true` **绕过 `_sent` 去重**。所以「临时 head=2.0 → 应用 → 换个主题 / 开关待机小动作 / 调音量」⇒ head 静默回到 0.75。这**不在**面板文案承认的范围内（文案只说「未保存的草稿」会冲掉它，`dev_tools_section.dart:1596-1601`）。
2. **结果依赖 `_sent` 恰好等于哪个键**：`_settings.addListener(_scheduleActionScalesSync)`（`main.dart:367`）在**任何一次 `edit()`**（`settings_controller.dart:357-360` 无条件 `notifyListeners()`）后 150ms 走 `syncNow(effective)`；若 `effective` 的键**恰好等于** `_sent`（临时覆盖前刚发过产品值），则**不发送**——临时值意外**粘住**；键不等则发送——临时值被冲掉。同一操作序列在不同历史下结果不同。

### 2.5 这条链路上唯一「设计上说得过去」的部分

`final_override` 层写入是**对的**：`web/surface/input.rs:18-26` 把 `PresetSink` 接到 `override_parameter`/`clear_override_parameter`，`l2d/src/pose_stack.rs:158-169、212-220` 确认优先级 `base < idle < input < physics < final_override`，所以预设压得住同帧更晚写的 idle（input 层）。**不要动这层。**

---

## 3. 症状③：导演系统「理解和实现脱轨」——NEKO 对照

### 3.1 现状：这套系统现在有**三个**情绪/表演决策者

| # | 决策者 | 输入文本 | 输出 | 位置 | 当前开关 |
| --- | --- | --- | --- | --- | --- |
| A | `live2d-ai-mod-director` 规则层（词表打分） | **用户输入**（`TurnPrompt`） | `latest.preset_id`（emotion→1 条）+ `action_cue`（intent→手势、emotion→表情） | Mod | **`mods.json` 里 `enabled: true`** |
| B | `live2d-ai-mod-director` 二路 LLM（`staging_*`，提示词写「助手…逐字勿改」） | 用户 + 助手 | 只有 cues | Mod | 默认 `false` |
| C | `live2d-ai-runtime/src/performance/` **表演层** | 用户 + 助手 | **`speak`（改写说辞）** + cues | **主链（引擎内）** | `live2d-ai.toml [performance] enabled = false` |

再叠加**两个前端应用通道**：`main.dart:421-425`（按句 `action_cue`）与 `main.dart:466-492`（拉 `GET /api/v1/mods/director/state` 的 `latest.preset_id`）。

**A 的三条证据**（规则层判的是**用户**的情绪）：
- `crates/live2d-ai-mod-system/src/topics.rs:50`：`TurnPrompt` 只给 Mod **用户侧**正文；
- `supervisor.rs:684-716`：`say_rx.recv()` 取出 `SayRequest { text, session }`，紧接着 `f(ModEventTopic::TurnPrompt, &text, …)` —— payload 就是**用户输入**；`mod-director/src/lib.rs:694-708` 拿它直接喂 `ledger.record_prompt(payload, …)` → `derive(text)`；
- `presets.rs:236-248` `rule_cues_for_text(text)` 与 `presets.rs:142-178 resolve/resolve_slots` 用的是同一个 `Decision`。

### 3.2 与项目**自己的** `docs/architecture/director-rfc.md` 正面冲突

| RFC 原文 | 位置 | 现行实现 |
| --- | --- | --- |
| 「**红线**：主链**不新增**情绪字段、不新增情绪事件、不为导演改 `ConversationConfig` / `PersonaSettings` / **引擎**；导演**不得**起第二个 LLM 客户端」 | `director-rfc.md:125-130` | C 就在**引擎**里（`conversation/engine.rs:404-417` 调 `perf.resolve(...)`），且是**主链自己的**第二个 LLM 客户端 + 独立 `[performance] base_url/api_key_env` |
| 「第二 LLM 调用做情绪分类 … **不做**」（理由：复制核心网络层、多一份端点与密钥路径、一轮多一次延迟与费用） | `director-rfc.md:169` | 实现里现在有**两份**第二 LLM（B 和 C） |
| 长期非目标：「第二 LLM 调用」「把导演做成**第二个 LLM/TTS 端点**的权威（端点权威永远只有 `[tts]` / `[llm]`）」 | `director-rfc.md:425-426` | `[performance]` 就是第三个端点 + 第三份 `api_key_env` |
| 导演的**回复侧**情绪证据是「**补充**证据」（用户问「今天很累」vs 模型回「那早点休息」）；骨架「**不订阅** `TextDelta`」；」回复侧情绪证据因此是**不完整**的」 | `director-rfc.md:119`、`419`、`502-503` | A **只**看用户侧，回复侧从未接上；缺口没补，反而在 C 里另起一条 |
| 「core 动作 / 表演状态 = **无驱动方**（休眠）；导演**禁止**」 | `director-rfc.md:389` | 动作包（表情+手势）经 `preset` 协议直接驱动渲染面参数，**绕过 core 仲裁** |

### 3.3 与 N.E.K.O 的实际口径**方向相反**

N.E.K.O 源码级事实（抓取 commit `a3c82b5a`，`git clone --depth 1` + raw 逐文件核对）：

1. **触发点**：`static/app/app-websocket.js:1483-1539` `finalizeAssistantTurn()` —— 「一轮 AI 文本**说完后**的统一收尾：… + 情感分析 + …」，`fullText` 是 `_geminiTurnFullText`（**AI 回复**的流式累积，`app-chat-adapter.js:775-776`）；分析完只调 `applyEmotion()`。
2. **项目自己写明两条管线分工**（`main_logic/activity/master_emotion.py:29-35`）：
   > *This is NOT the `OUTWARD_EMOTION_ANALYSIS` pipeline that drives lanlan's avatar face — that analyzes the **character's** reply. This analyzes the **user's** own utterance and **never touches the avatar channel**.*
   —— N.E.K.O 里分析**用户**的那条叫 `MasterEmotionTracker`（valence/arousal，喂 FocusScorer/前置门），**明确不驱动 Live2D**。
3. **机制**：主路径是**再发一次 LLM**（`system = OUTWARD_EMOTION_ANALYSIS_PROMPT`、`user = text`，`main_routers/system_router/emotion.py:883-961`），关键词启发式只是**兜底/覆盖层**（`984-1031`：LLM 坏 JSON 才降级；启发式分数 ≥4 且置信 <0.8 才强覆盖）。
4. **输出契约**：只有 `{"emotion": 5 选 1, "confidence": 0..1}`（`_EMOTION_CANONICAL_LABELS = ("happy","sad","angry","surprised","neutral")`，`emotion.py:66`）。
5. **不改写说辞**：情绪层拿不到 TTS、也不回写文本（§5 五条证据：只 `applyEmotion` + 一条 WS 帧 + 一个静态主题气泡；`tts_runtime.py`、`main_logic/tts_client/` 里 `emotion` **零命中**）。唯一与文本有关的是**剥离** `<...>` 标记（`main_logic/core/turn.py:1694`），与情绪结果无关。
6. **expression 与 motion 是同一 emotion 的两个并列子表**（`EmotionMapping.motions` / `.expressions`，各查各的、单侧缺失另一侧照常、多候选随机；样本见 `static/mao_pro/mao_pro.model3.json`——`angry` 的 motion 是**空数组**，所以生气只有表情没有动作）。**不是**我们这个「emotion→表情 / intent→手势」的错位配对。
7. **expression 跨轮保持、无自动过期**；motion 到点只清 motion、**保留 expression**。
8. **主聊天没有 director**。全仓唯一的 Director 是 `YuiGuideDirector`（七日教程场景导演，`static/tutorial/yui-guide/director/director-core.js:46`），台词写死在 `days/dayN-*.js`，director 只做时间线调度。
9. **expression 必须避开头部朝向与口型**：`live2d-emotion.js` `recordInitialParameters()` 的 `skipParams = ['ParamAngleX','ParamAngleY','ParamAngleZ', ...LIPSYNC_PARAMS]` —— 表情重置**不碰**头角与嘴。我们的表情包却**故意**写 `ParamAngleY/Z/X + ParamBodyAngleY`（`presets.json` 的 `smile`/`unhappy`/`surprised`），正是 N.E.K.O 刻意排除的那类通道。

### 3.4 脱轨清单（按严重度排序）

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

### 3.5 现状一句话

用户当前**实际在跑**的是 A（规则层 + 用户输入 + `mods.json` 里 director 开着），C 是关的。所以「导演理解和实现脱轨」的直接观感来自 A：**角色在模仿用户的情绪，而不是在表演自己的情绪**；而 C 一旦打开，它会**改写角色要说的话**——两件事都与 N.E.K.O 相反。

---
### 3.6 口径裁决（维护者 2026-09-21）

> ⚠ **本节已被后续澄清部分推翻，先读 §3.7。** R1（输入=用户输入）仍然成立；
> **R2 已被维护者否定**——导演**不是**「场景 Mod」，它是**一个 AI**，属产品的角色扮演本体。
> 在 §3.7 的产品形态被确认之前，**不要**按 R2 动任何代码或文档。

**产品目标（维护者原话）**：「本项目的目标是 live2d 皮套让 ai 接入的底座和 mod 分化各场景」。

由此得到两条**裁决**，后续任何人不得按别的项目翻案：

| # | 裁决 | 含义 |
| --- | --- | --- |
| **R1** | 情绪/表演决策的**输入 = 用户输入** | 「用户说了什么 → 皮套怎么反应」。这不是偏差，也不是「待对齐 N.E.K.O」的缺口；N.E.K.O 分析角色回复属于**另一种产品形态**（角色有自己心理），不是本项目底座要表达的东西 |
| ~~**R2**~~ **已被否定** | ~~导演/表演属**场景能力**，不属**底座**能力~~ | **这是我的推论，不是维护者的裁决，已被明确否定**：导演**不是一个「场景 Mod」**，它**是一个 AI**（见 §3.7）。因此「主链里不该有第二 LLM」这条推断**作废** |

**影响面（本节落地后本文其它部分的口径以本节为准）**：
- §3.4 的 **D1 撤回**（它的建议「改判角色回复」与 R1 相反）；
- ~~§3.4 的 D2/D4 因 R2 而加强~~ **作废**：D2/D4 是否算问题，取决于 §3.7 里「导演是演员还是表演辅助」的答案，**未确认前不要当缺陷处理**；
- §3.4 的 **D3/D5 仍然成立**（两份契约相反的二路 LLM；两条未协调的前端驱动通道）；
- §3.3 的 N.E.K.O 事实**仍然有效且有用**——它现在是「另一种产品形态的参照」，用来解释我们**为什么刻意不同**，
  而不再是「要对齐的目标」。其中 §3.3.9（N.E.K.O 表情刻意避开 `ParamAngle*`）与 §3.3.7（表情跨轮保持）两条
  仍与本项目现状存在**设计差异**，属于待裁决的设计问题（见 `action-packs-v0.md`），与 R1/R2 无关。
- 已落进任务：`IMPL-PROMPTS-actions-performance-round.md` 的 **W8**（把 R1/R2 写进契约）与 **W9**（单一驱动者 + 表演权归位，
  其代码部分卡在裁决点 B2）。

**给下一轮执行者的一句话**：你可以引用 N.E.K.O 的实现细节，但**不得**用它作为把输入改成角色回复的理由；
同时**也不得**再引用 §3.6 的 R2 去论证「导演该住 Mod / 主链不该有第二 LLM」——R2 已作废，先读 §3.7。

### 3.7 产品形态（维护者 2026-09-21 追加澄清，**权威原话**）

> 「从产品形态来看是可以通过**类酒馆稳定人设**、**mod 实现各功能**的 **ai 人**，
>  通过 **live2d 皮套做情绪表达**。**导演不是辅助用户更好的操控皮套，而是作为一个 ai**。
>  可以说是『**角色扮演皮肤**』。」

**从这段话能确定的事（不再有歧义）**：

| # | 确定的事实 | 对我此前理解的修正 |
| --- | --- | --- |
| P1 | 产品是「**角色扮演皮肤**」：一个 **AI 人** | 皮套**不是**用户操控的木偶，而是这个 AI 的**身体**；情绪表达是**这个 AI 的**表达 |
| P2 | 人设靠**类酒馆**方式稳定 | 主模型（`[llm]` + persona）承担角色扮演，这一点没变 |
| P3 | **导演是一个 AI** | 导演**不是**「辅助用户操控皮套」的工具，也**不是**「可选的场景 Mod」。它是**产品本体的一部分** |
| P4 | 功能由 **mod** 分化 | mod 承担的是**各功能**（记忆 / 外部输入 / 语音 / 壁纸…），**导演不在此列** |
| P5 | Live2D 皮套承担**情绪表达** | 情绪表达是产品目标本身，不是附加装饰 |

**因此我此前两个推断的处置**：
- `R1`（输入=用户输入）**保留**——这是维护者对「情绪判定的输入是什么」的直接回答。
  在「角色扮演皮肤」下它自洽：情绪的**起因**是用户说的话，AI 人据此产生情绪并表达。
- `R2`（导演属场景 Mod、主链不该有第二 LLM）**已被否定**，作废。
  推论链路的错误在第一步：我把「底座 + mod 分化」读成「凡是非主链皮肤的能力都该住 Mod」，
  而维护者的意思是「**AI 人本身**（含角色扮演与情绪表达）就是底座的那一面，mod 只分化**功能**」。

**仍未确认、且决定了 W8/W9 怎么写的两件事**（问维护者，不要自行假设）：

| Q | 问题 | 影响 |
| --- | --- | --- |
| **Q1** | 「AI 人」是**一个**模型还是**两个**？即：主模型（类酒馆人设）**说**、导演**演**（只出表情/动作）；还是主模型写剧情、**导演是「说 + 演」的演员**（`speak` 由导演产出）？ | 决定 `[performance].speak` 是产品能力还是越界（D2 是否成立） |
| **Q2** | `[performance]`（主链第二 LLM，含 speak）与 director 的 `staging_*`（Mod 第二 LLM，只出 cues）是**同一个东西的两代实现**吗？该保留哪一个？ | 决定 W9 是「二选一合并」还是「保留两条各司其职」 |

**在这两问有答案之前**：`IMPL-PROMPTS` 的 **W8 与 W9 都不要派**（W8 的块内口径建立在被否定的 R2 上）。
Wave 0 的其它块（W1/W2/W3/W5/W6）与 Wave 1/2（W4/W7）不受影响。

### 3.8 产品形态再澄清（维护者 2026-09-21 之二，**权威原话**）

> 「也可以理解成**酒馆 套 live2d 皮套壳子**。本身目前的 **mod 矩阵不就是在实现相关功能**吗。
>  而且**文档相关漂移可以先治理一下**。」

**由此收敛的三条结论**（本节之后，导演相关的争议按此处理）：

| # | 结论 | 含义 |
| --- | --- | --- |
| S1 | 产品 = **「酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子」** | 人设由酒馆式内核稳定；皮套是壳；**不需要为「导演该住哪」做架构搬迁** |
| S2 | **功能分化已经由现有 mod 矩阵承担** | 记忆 / 外部输入 / 语音 / 人设 / 导演都已在矩阵里。因此 §3.7 的 Q2（[performance] 与 staging 谁搬家）**不再是架构问题**，只是「两份职责重叠的可选实现」这个**文档要说清的事实** |
| S3 | **先治理文档漂移** | 多处仍把导演写成「遗留并行实现 / 不建议开启 / 零投递旁路 / 不驱动动作」，与代码事实和产品定位都不符。这是**当前最高优先级**，且**不需要等任何裁决** |

**§3.7 那两个问题的最终处置**：
- **Q2**：按 S2 关闭——**不做搬迁**；由 W8 在文档里把「两个可选提供者都默认关、职责重叠」写成事实。
- **Q1**（主模型说、导演只演；还是导演是说+演的演员）：**降级为开放的设计问题，不再阻塞任何任务**。
  当前代码的事实是：`[performance]` 会产出 `speak`（主链第二 LLM 改写说辞），`staging_*` 只出 cues。
  W8 只需把这个**事实**写进文档并标注「谁是演员**未定**」，**不裁决**；等产品需要时再定。
- **因此 W8 / W9 都不再需要裁决**：W8 = 文档治理；W9 = **单一驱动者**（把「拉 latest.preset_id」那条通道退役、统一到 `action_cue`），
  这是**纯缺陷修复**（两条通道取到的集合不同，见 §3.5/D5），与 Q1 无关。

**给下一轮执行者的一句话**：「导演是产品本体的表演者、mod 矩阵已承担功能分化」是既定事实——
不要再提出「把导演搬进/搬出 Mod」「主链不该有第二 LLM」这类架构改造建议；只做文档治理与单一驱动者修复。


---

## 4. 症状④：其它工程小错误（编入下一轮）

| # | 问题 | 证据 | 建议 |
| --- | --- | --- | --- |
| E1 | `join_endpoint` 现在有 **4 份实现**，其中一份是**同一个 crate 内**的私有重复 | `runtime/src/lib.rs:162`（pub，文档自称「唯一实现」）、`runtime/src/performance/client.rs:136`（同 crate 重复，还被 `mod.rs:48` 对外 re-export）、`mod-director/src/staging_http.rs:52`、`mod-memory/src/summary_http.rs:41` | 收敛到 `runtime::join_endpoint`；改掉「唯一实现」这句不实文档 |
| E2 | 注释与代码相反：input.rs 说「动作 override 层已删除，idle 之上不再有 `final_override` 写入方」，但同文件 `202` 每帧都在写 | `l2d-wasm-demo/src/web/surface/input.rs:237-242` vs `:200-203` | 改注释 |
| E3 | `settings.rs` 文档仍写「由 `resolve_with(|n| env::var(n).ok())` 从环境读出」，与「密钥真源 = `.env`（`secrets::lookup`）」矛盾 | `runtime/src/settings.rs:594` | 改文档（P1 碰过这个文件但没顺手改） |
| E4 | 4 处注释/文档仍引用**已不存在的** `preset.rs`（现在是 `preset/` 目录） | `l2d-wasm-demo/src/main.rs:56`、`preset/table.rs:1`、`shell/flutter/lib/live2d/live2d_stage.dart:371`、`preset_labels.dart` 头注 | 全部改成 `preset/` |
| E5 | 3 处本地 token 读取绕过 `secrets::lookup`（会绕过 `.env`，出现「界面上刚写了 key、链路还说没配置」） | `mod-external-input/src/lib.rs:155`（`EXTERNAL_INPUT_TOKEN`）、`web_api/external_routes.rs:248`、`web_api/voice_routes.rs:226` | 统一走 `secrets::lookup`（上轮已记录，本轮未做） |
| E6 | Mod 读取面仍用**弱口径** `settings_to_view`（`has_api_key` = 仅「声明了变量名」），与 `GET/PUT /env` 的强口径不一致 | `web_api/cli_entry.rs:272`；对照 `settings/view.rs:7-10` 的两种语义说明 | 注入 lookup，或在代码里显式标注「Mod 面例外」 |
| E7 | **Dart 又抄了一份预设 id 列表**，与「单一真源 = `PRESET_IDS`」纪律冲突 | `settings/preset_labels.dart:22` `kExpressionPresetIds`、`dev_tools_section.dart:1273-1287` `kDebugExpressionPresets` / `kDebugGesturePresets` | 从 `/actions/preset_labels.json` 的 `channel` 字段派生 |
| E8 | **38 个 Rust 文件超行数纪律**（源码 ≤500 / 测试 ≤800，`AGENTS.md:227`），动作/导演区最集中 | `mod-director/src/lib.rs` 892、`mod-director/src/tests.rs` 839、`l2d-wasm-demo/src/main.rs` 833、`mod_registry.rs` 1328、`web_api/tests_ws.rs` 1048 … | 下一轮只拆动作/导演那 4 个，其余记 backlog |
| E9 | 调试面板单文件 1649 行（Dart 无行数纪律，但已难维护） | `settings/sections/dev_tools_section.dart` | 拆成 `debug_panels/` 三个文件 |
| E10 | `director_panel.dart` 与文档口径可能不一致（`director-mod-v0.md:305` 说面板顶部写「不驱动动作、delivered=false、channel=none」，而实现 `channel` 恒 `"preset"` 且**确实**驱动动作）；面板里已无此文案，但 `lib.rs:41-82` 的模块头注仍保留旧 JSON 示例 | `mod-director/src/lib.rs:41`、`109-110`；`shell/flutter/lib/settings/mods/director_panel.dart` | 统一到「channel=preset、会驱动动作」 |
| E11 | `Live2DStage.actionScales` 死参数（症状②的一部分，也属工程债） | 见 §2.3 | 要么接上、要么删掉 |

### 4.1 文档 ↔ 配置 ↔ 代码三方漂移（这一类最危险）

| 文档 | 它说什么 | 分支现实 |
| --- | --- | --- |
| `AGENTS.md`（`-l1`，`:112`） | 「LLM 工具层与**动作系统已整体拆除**——不要再以『动作系统』为前提写代码」 | 分支里有 `[action]` 段、9 条动作包、`preset` 协议、调试面板「表情/动作调试」 |
| `AGENTS.md`（`:256-268`） | 「动作在产品路径上**不存在**」；core action/performance **休眠无驱动方** | 动作包经 `preset` 直接驱动渲染面参数 |
| `docs/plans/PRODUCT-L1-GOALS-2026-09-15.md:39` | 非目标：「**不复活 Action**」；director「**本轮不做**」 | director 在 `mods.json` 里 `enabled: true` |
| `director-rfc.md:417` | 骨架「**不投递**任何动作 / TTS 参数」 | 已投递 `latest.preset_id` + `action_cue` |
| `AGENTS.md`（`:119/134`） | 「现行 5 个注册 Mod … `mod_count_is_five`」 | `main.rs:471` 仍是 `mod_count_is_five` ✔（这条一致） |

**建议**：下一轮开头先做一次「文档对齐」小任务（只改文档，不改代码），把动作/导演的现行状态写清；否则下一轮的执行者会继续被 `AGENTS.md` 误导（我自己这轮就被误导过一次——先按 AGENTS 以为动作系统不存在）。

---

## 5. 下一轮任务清单（建议）

### 波次 0：文档对齐（无代码风险，先做）
- **P0-doc**：给 `AGENTS.md`（`-l1`）/`PRODUCT-L1-GOALS`/`director-rfc`/`action-packs-v0` 加一节「动作与表演现行状态（2026-09）」，逐条列出：动作包存在、`[action]` 存在、director 缺省启用、`[performance]` 存在但缺省关；并作废那些已不成立的「非目标」字句。**只改文档。**
- 验收：`grep -n '动作系统已整体拆除\|不复活 Action' docs AGENTS.md` 不再命中无条件的断言（或每条后面都跟一句现状更正）。

### 波次 1：症状①②（渲染面标定 + 前端接线，可并行）
- **P1-scales**（后端/资产）：重标 `[action]` 出厂倍率与 `presets.json` 表值，使
  `final = 表值 × 倍率` 在 `0.2…2.5` 全行程内**都不触上限**；至少保证出厂值不落在死区起点 6% 以内。
  验收：一条新回归按「包 × 通道 × 倍率 ∈ {0.2,0.5,1.0,1.4,2.0,2.5}」断言 `|final| < 上限`（留 5% 余量），并打印死区起点表。
- **P2-debugpanel**（Flutter）：①「叠加基础表情」默认改 `false`；②`_expressionId` 加 ttl 计时/状态同步（渲染面到点后 UI 不再假称「当前基础表情：X」）；③手势按钮不再隐式续期表情。
  验收：`developer_section_test.dart` 增一条「默认不叠加」+「表情到点后开关描述不再声称在演」；`flutter analyze && flutter test` 全绿。
- **P3-revoke**（前端+协议）：给 `none` 一条真正的撤销路径（`_applyDirectorPreset` 遇到 `none` 发 `applyPreset('none')` 而不是 return；或明确删掉「空 action_cue 清残留」这句文档）。
  验收：一条回归证明「收到空 `action_cue` / `none` 后渲染面两槽都被清」。
- **P4-scales-wiring**（Flutter）：`Live2DStage.actionScales` 要么接上（把有效倍率作为 prop 传下去，重建时自愈），要么删掉；临时覆盖要么进 syncer 的 `_sent`，要么给 syncer 一个「临时层优先」的显式状态。
  验收：一条 widget 测试证明「设临时值 → 改主题（`DisplayPrefs` 变更）→ 临时值仍在」；再一条证明「恢复产品设置」一定生效。

### 波次 2：症状③（导演，需要先裁决再动代码）
- **P5-decision（先出裁决，不写代码）**：在一页纸里回答三件事——
  1. 情绪分析的对象是**用户输入**还是**角色回复**？（建议对齐 N.E.K.O ⇒ 角色回复）
  2. 保留 `[performance]`（主链第二 LLM）还是退掉？（若保留，按 `director-rfc.md:354-372` §5.2 清单逐条交付；若退掉，`speak` 一律等于 `clean_for_tts(原文)`）
  3. `staging_*` 与 `[performance]` 二选一（不允许两份契约相反的「二路导演」）。
- **P6-director-input**（代码，取决于 P5）：把 A 的情绪/意图判定输入从 `TurnPrompt` 换成「本轮助手正文」（`TurnReport::assistant_text`；Mod 侧已有 `AssistantReplied` 主题可用），并把 `TurnPrompt` 降为「答案长度/称呼」这类非情绪用途。
  验收：一条回归证明同一句用户输入 + 不同助手回复 ⇒ 得到不同 preset；并保留「回复侧缺证据时 fail-safe 到 neutral」。
- **P7-single-driver**：闸掉 `latest.preset_id` 那条前端拉取（或让它在 `[performance]` 开时让位），兑现「同轮只有一个 cue 产者」。
  验收：一条回归证明 performance 开时前端只消费 `action_cue`。

### 波次 3：症状④（卫生 + 收敛，可穿插）
- **P8-endpoint**：收敛 4 份 `join_endpoint`（E1）；改掉「唯一实现」文档（E1/E2/E3/E4/E10）。
- **P9-secrets**：3 处 token 走 `secrets::lookup`（E5）；`cli_entry` 的 Mod 面口径统一或显式标注（E6）。
- **P10-dart-sot**：删掉 Dart 的两份 id 列表，改从 `preset_labels.json` 的 `channel` 派生（E7）。
- **P11-split**：拆 `mod-director/src/lib.rs`（892）、`tests.rs`（839）、`l2d-wasm-demo/src/main.rs`（833）、`dev_tools_section.dart`（1649）（E8/E9），其余 34 个超限文件记 backlog。

### 全程红线（沿用既有纪律）
- **不碰** `/home/skystar/Live2D-Ai`（主工作树）；只在 `/home/skystar/Live2D-Ai-l1` 动。
- **禁止**任何 `git worktree / checkout / switch / stash / reset / commit / clean`（178 个未提交改动）。
- **禁止**裸 `cargo fmt --all`（只能 `--check`）。
- 改了 `shell/flutter/**` **必须** `flutter build web --release --base-href /app/ --no-web-resources-cdn`（否则 `/app/` 上看不到改动——上轮就是这样漏的）。
- 门禁：`cargo test --workspace --all-targets` / `--doc` / `fmt --check` / `clippy -D warnings` / `rust-ratio` + `flutter analyze && flutter test` + `flutter build web` + `./scripts/ignite.sh --check`。

---

## 6. 证据索引（按症状）

**症状①**：`dev_tools_section.dart:1337,1346,1400-1406,1483-1494,1571-1573`；`preset/mod.rs:808-839,845-870`；`preset/table.rs:174-210`；`main.dart:421-425,482-483`；`developer_section_test.dart:122-154`；`performance-layer-v0.md:82-86`。

**症状②**：`preset/scales.rs:13-25,89-93,143-154`；`preset/mod.rs:157-159,469-501`；`presets.json`；`settings_models.dart:236-239`；`live2d_stage.dart:96,205-207,287-294,413-417`；`main.dart:288-291,367,709-717`；`shell_prefs.dart:14-45,91-92,99-114`；`shell_settings.dart:254-261`；`action_scales_sync.dart:55-67`；`dev_tools_section.dart:1596-1601`；`web/surface/input.rs:18-26,200-203`。

**症状③**：`docs/architecture/director-rfc.md:119,125-130,162-169,389,393-397,417-428,494-516`；`mod-director/src/lib.rs:41-110,540-546,694-759`；`mod-director/src/presets.rs:142-178,236-248`；`mod-director/src/staging.rs:90-100`；`runtime/src/performance/prompt.rs:15-39`；`runtime/src/performance/mod.rs:269-358`；`runtime/src/conversation/engine.rs:398-490`；`live2d-ai-desktop/src/supervisor.rs:694-732`；`supervisor/handlers.rs:47-52,80-125`；`supervisor/turn.rs:195-198`；`main.dart:417-425,466-492`；`live2d-ai.toml:71-81`；`mods.json`；N.E.K.O 证据见 §3.3 各条 URL。

**症状④**：见 §4 表格内逐行 `file:line`。

---

## 7. 本轮调研未做的事（如实标注）

- **没有起活服务做肉眼验收**（本 worktree 上一次服务已停；这轮是纯代码调研 + 离线计算）。症状①②的**最终观感**仍需要在浏览器里点一遍确认（`flutter build web` 后）。
- **没有跑门禁**（不涉及代码改动）；§4 的行数统计是脚本扫 `git ls-files` 得出的。
- **N.E.K.O 的 `emotion` 模型 tier 默认端点**未定位（由用户配置读出，仓库无硬编码）；MMD/VRM/PNGTuber 侧情绪取值未展开核对。
- §5 只给**范围与验收口径**，未给逐 worker 的可复制提示词；需要时再出（沿用 `IMPL-PROMPTS-…` 的形式）。

---

## 8. 目标链路规格（维护者 2026-09-21 · **已确认**）

> 本节是**目标设计**，不是现状（现状见 §3.5 与全文各处 file:line）。
> 维护者确认后本节升格为契约，并据此改写 W8/W9 + 新增任务。

### 8.1 链路（维护者描述，逐字保留结构）

```text
用户消息
 → ① 表演/前端 LLM（接酒馆人设）→ 出回答（含思考）              【第 1 次 LLM 调用】
 → ② 路由 = 大脑/导演 LLM（独立端点；不与①共用上下文）           【第 2 次 LLM 调用】
      产出 3 个字段：
        1) Live2D 表情
        2) Live2D 上半身动作
        3) 给 TTS 的消息（**过滤层**）
 → ③ TTS 产出首个音频并播放（**嘴型由 TTS 驱动**）
      同时前端展示模型各小句回复（与音频同拍），并展示**连续的**模型动画
 → 链路可**整体断开**并回初始：① 用户按停止  ② 再收到用户消息
      断开时清空：动作 / 表情 / TTS 待播放
```

### 8.2 导演是**异步状态机**（不是「一轮一次」）

维护者给的触发例 —— **上一轮动作结束**时给导演输入：
- 模型回复；用户原消息；本次模型回复**已上屏了哪些部分**；**上一个动作字段**（例：闭眼、抬头（上半身向上保持？）、强度**轻微**）。

期望输出：**惊讶 / 开心** + **强度中** + **是否复原**。

具体场景：「用户：**好好想想**」→ 期望展示：**闭眼思考一段时间** → TTS 合成之后 →
**陆续播放第二次的动作编排释放**（维护者原话：**类似提线木偶**）。

### 8.3 与现状的差异（逐条带证据）

| # | 目标 | 现状 | 差距 |
| --- | --- | --- | --- |
| ① 第一个 LLM 出回答（含思考） | 酒馆人设流式出正文；思考单列 | `[llm]` 流式；`reasoning_delta` 独立帧、不进 TTS（`handlers.rs:68-79`） | **已对上** |
| ② 第二个 LLM 独立、不共用上下文 | 导演专属端点 | `[performance]` 有独立 `base_url`/`model`/`api_key_env`；提示词只有 system+user 两条（`performance/prompt.rs:16-39`） | **已对上** |
| ② 输出 **3 字段** | 表情 / 上半身动作 / 给 TTS 的文本 | 单 `preset_id`（由 `kind` 决定落哪个槽）+ 单 `speak`（语义是「整理成这一轮**真正要说的话**」） | 协议要改：三字段；`speak` 语义从「改写」改为「**过滤**」；「上半身动作」要独立字段且需要**保持/复原**语义 |
| ③ 嘴型由 TTS 驱动 | 导演不碰口型 | 口型走 `input` 层 RMS→dB 包络（`web/surface/input.rs:180-189`） | **已对上** |
| ③ **连续**动画（提线木偶、陆续释放） | 动作可排队、保持、按后续指令释放 | 渲染面 = **单个包 + ttl 到点自动撤**（`preset/mod.rs:845-870`）；**没有队列/时间线** | 需要编排层（`live2d-ai-core` 的 `performance/` 子系统正是编舞，现休眠、无驱动方） |
| ③ 小句回复与音频同拍 | — | `SentenceVoiced` → WS `text_delta`；cue 锚 `audio.sentenceStarts` | **已对上** |
| ④ 断开 = 停止键 | 清 动作/表情/TTS 待播 | Stop → `core::reducer::stop()` 推进 epoch（`reducer.rs:257-258`），音频按 epoch 丢弃；**但渲染面的活动预设不会被清** | 需要：Stop 时显式下发 `none` + 清前端 cue/队列 |
| ④ 断开 = 再收到用户消息 | 回初始 | 新 `TurnPrompt` 只 `arbiter.reset`；前端整份替换 cue；**渲染面旧预设仍演到 ttl** | 同上 |
| 导演的异步输入 | 已上屏进度 / 上一个动作字段 / 动作结束 | **三样都没有**：渲染面只发 `ready`/`loaded`/`error`/`fps`/`stage-ack`（`l2d-wasm-demo/src/main.rs` 的 `emit_event`），**没有 preset 生命周期 ack**；前端 `presetStatus` 是按 2600/900ms **猜的定时器**（`live2d_stage.dart:373-375`）；HUD 的 `preset:` 行只画进 DOM、不上报 | **最大的结构性缺口**：需要新链路「渲染面 → 前端 → 后端 → 导演」 |

### 8.4 由本节引出的待确认点

**Q1** 「给 TTS 的消息（过滤层）」是**只净化**（剥舞台指示 / markdown / 括号动作，不改语义、不改句界——即现有 `clean_for_tts` 的职责），还是**允许改写表达**？

**Q2** 「上半身动作」是**独立通道族**（只动 `ParamBodyAngle*` + 少量头），还是就是现在的手势包（头身同动）？「抬头（上半身向上**保持**？）」里的**保持**是不是一等语义（保持到下次指令，而不是 0.9s 到点回 0）？

**Q3** 「闭眼思考」这类**持续状态**与「惊讶/开心」这类**情绪表情**是同一层还是两层？（N.E.K.O 是 expression 层 + 常驻表情层）眼睛是否要有独立的第四字段？

**Q4** 导演的**输入事件清单**是否就是：① 首音频开始 ② 每句播放完 ③ 上一个动作结束（需渲染面 ack）④ 用户停止 ⑤ 新用户消息？导演输出是**立刻生效**还是**排到当前动作之后**？

**Q5** 「当前在演什么 / 是否结束」的**真源**放哪：渲染面（新增 ack 帧）还是前端镜像？（现状是前端按固定 ms **猜**的，与项目「不猜」纪律冲突。）

---

## 9. 时间轴对齐（维护者 2026-09-21 之三，**硬约束**）

> 「由于 **tts 操控嘴型**，需要对**导演层的编排**和**上 live2d 模型**进行**时间轴对齐**。」

### 9.1 由此得到的五条硬结论

1. **唯一时间基准 = 音频播放时钟**。嘴型是由 TTS 音频驱动的，所以导演编排的时间轴必须锚在**音频播放位置**上；墙钟（`performance.now()`）、消息到达时刻都不合格。
2. **锚点粒度要细到句内**。现状只有句级锚点（`sentence_seq` → 该句音频**开始**时应用一次）；「闭眼 300ms 后抬头」「说这句时第 800ms 才摇头」这类编排**表达不出来**。
3. **存在两段时钟 + 一个交接点**：
   - **段 A（首个音频之前）**：如「闭眼思考」——此时**还没有音频时钟**，只能用「等 TTS」的墙钟；
   - **交接点 = 首个音频开始播放**（现有 `audio.sentenceStarts` + `first_chunk` 边界帧）；
   - **段 B（首音频之后）**：全部按音频时钟。
   ⇒ 必须显式定义「段 A 的动画在交接点如何转段/是否被切断」，否则闭眼会在开口那一瞬被 ttl 掐掉。
4. **TTS 首包延迟不可预测 ⇒ 编排不能写死固定 ms**。「闭眼想 2 秒」里的 2 秒其实是「音频还没来的等待」，真实时长等于 TTS 时延。
   ⇒ 需要**事件式编排**（「等首个音频到了再释放下一段」），而不是 `sleep(n)`。这正是维护者说的「TTS 合成之后陆续播放第二次的动作编排释放」。
5. **断开时时钟消失，编排必须一起停且不补帧**。停止键 / 新用户消息 → 音频链断（epoch 推进、队列清空）→ 编排必须同步取消；否则会在「已经没有声音」的情况下追着播完，或在下一次音频开始时补播旧帧。

### 9.2 现状：**两套时钟并存**（这就是要对齐的东西）

| 通道 | 用的时钟 | 证据 |
| --- | --- | --- |
| **口型**（嘴） | **音频播放时钟** | 前端用 `audio.currentTime` 查服务端 `volume` 包络逐帧驱动 |
| **表情 / 动作**（脸、上半身） | **墙钟** | 渲染面 `apply_frame(now_ms, core)` 的 `now_ms = performance.now()`，进度 = `(now - started_ms)/ttl`（`preset/mod.rs:845-870`、`input.rs:150-153`） |
| **cue 的触发** | 句级锚点 | `_applyDirectorCueForSeq(seq)` 在 `sentenceStarts` 触发（`main.dart:391-392`、`443-456`） |

⇒ 结果：**嘴跟着声音、身体跟着墙钟**。帧率抖动、标签页切后台、暂停/继续、音频解码延迟，都会让两者错开。

### 9.3 建议的契约方向（草案，待确认）

- cue 的时间字段升级：`anchor`（`immediate` / `audio_start` / `after_prev`）+ `at_ms`（相对锚点的偏移）+ `duration_ms` + `hold`（保持到下次指令 / 到点回基准）+ `release`（是否复原）;
- 渲染面接受**时间轴基准**：两个可选做法——
  (a)【推荐】前端**每帧**把音频时钟（`audio.currentTime`，或「本轮已播 ms」）下发给渲染面（新增一条轻量消息），渲染面用它当 `now_ms`；
  (b) 前端把音频时刻换算成「从现在起 delay_ms」一次性下发，渲染面仍用墙钟近似——实现小，但有漂移、且暂停/切后台必错。
- 段 A 的动画需要一个**不依赖音频时钟**的驱动源（墙钟），并在交接点上按规则转段（建议：`hold` 的段 A 动画在首音频到达时**继续保留**，直到导演显式 release）。

### 9.4 新增待确认点

**Q6** 时间轴基准取 §9.3 的 (a) 每帧下发音频时钟，还是 (b) 一次性换算 delay？

**Q7** 断开（停止 / 新用户消息）时，正在保持的表情/动作是**立即定格**、**平滑回基准姿态**、还是**立刻归零**？「模型复原」这个输出字段的语义就是它吗？

---

## 10. 导演输出契约与字段分解（维护者 2026-09-21 之四 · **已确认**）

### 10.1 已确认（维护者答复，逐条）

| # | 维护者答复 | 我的解读（请核对） |
| --- | --- | --- |
| **Q1** 过滤层 | **不允许改写**原 LLM 转 TTS 的文本；**但允许断句**（例：「嗯。。。。我想到了」→「嗯」与「我想到了」是**两部分**，分开交 TTS 合成） | 字段 = **切分方案**：字**逐字不变**，只决定在哪里切开、分几段送 TTS。⇒ **断句点天然就是动作编排的时间锚点**（每段一个独立 audio 元素，currentTime 相对该段起点） |
| **Q2** 三个表演字段 | 「不管目前实现，具体更加分开」：**字段1 = 半身 4 向移动**（可按比例合成）、**字段2 = 头部 4 向移动**（点头上下 + 摇头左右，可按比例合成）、**字段3 = 面板表情选择**（开心/伤心…）；每个字段带**强度**；**同类字段可重复输出以合成**；基础动作强度由**人工测试**确定以适配不同皮套；并给导演 AI 一份**使用手册（基础提示词）** | 输出 = **3 个表演字段 + 1 个文本切分项**；表演字段可多次出现、同类**相加**；每个字段到模型参数的**映射按皮套**确定（即「模型选择的相关字段」） |
| **Q3** 眼睛/持续状态 | **同 Q2**（按同样原则分开；不理解再问） | 已由 §10.7 答复（Q8）：持续状态靠**重复输出**维持，而非独立 hold 字段 |
| **Q4** 输入事件 + 立即/排队 | **同 Q2** | 已由 §10.7 答复（Q9）：输出**立即生效且可叠加**，**不需要动作队列**（导演自己就是调度器） |
| **Q5** 「动作结束/上一个字段」真源 | 可以把**日志**喂给导演（原话：天然就有相关记忆了）；反问：渲染面 ack 帧 vs 前端镜像，**哪个在模型表现层面更好？** | 见 §10.3：**事件级 ack 帧** |
| **Q6** 时间轴基准 | 反问「你的推荐是？」 | 见 §10.4：**先事件级锚点 + 一步到位下发音频时钟** |
| **Q7** 断开时收尾 | **退回默认待机态**，或退回**多角色扮演本身的基础状态值**（开心/伤心/愤怒/思考这类） | ⇒ 需要「**角色基础状态**」概念（per-character baseline）；归属待确认 Q10 |

### 10.2 必须先说清的**模型侧物理限制**（直接影响字段定义）

assets/models/bai/runtime/bai.cdi3.json 我全量列过：共 **128 个参数**。与三个字段相关的**现有通道**：

| 字段 | 可用参数（实际存在于该皮套） | 语义 |
| --- | --- | --- |
| **1 半身** | ParamBodyAngleX/Y/Z（身体旋转 X/Y/Z）、ParamBodyAngleX2(XZ)、ParamBodyAngleY2(Yy)、ParamBodyAngleX3/X6(肩部 x/y)、ParamBodyAngleX4(胯部)、ParamBodyAngleX5(迈腿) | **旋转/摆动** |
| **2 头部** | ParamAngleX/Y/Z（角度 X/Y/Z） | 左右 = X（摇头）、上下 = Y（点头）、倾斜 = Z |
| **3 表情** | ParamEyeLOpen/ROpen（开闭）、ParamEyeBallX/Y（眼珠）、ParamBrowLY/RY（眉）、ParamMouthForm（嘴形）、Param4(嘟嘴)、Param6(歪嘴)… —— **必须排除 ParamMouthOpenY**（口型归 TTS） | 面板可选集合 |
| （额外玩法） | 翅膀 / 耳朵 / 光环 / 袖子 / 围裙 / 头发各 3-4 级参数，共 **80+ 个非标准参数** | **现在被硬白名单挡死**：crates/l2d-wasm-demo/src/preset/table.rs:20-34 的 ALLOWED_PARAMS 只允许 13 个标准参数，出现其它参数就丢通道 + warn |

⚠ **不存在拉伸/形变参数**：Cubism 标准参数集与这个皮套里都没有 Scale/Stretch 类通道。
⇒ 「4 向**拉升**」若指**形变拉伸**，本皮套做不到（要模型作者加参数，属模型侧工作）；若指**摆动/倾斜**，现有 ParamBodyAngleX/Y（半身）与 ParamAngleX/Y（头）已经够用。**见 Q11**。

### 由 Q2 直接推出的两项结构性改动（记录用；排在 10.3 前是写入顺序，不是优先级）

1. ~~白名单要扩展到非标准参数~~ **已按维护者答复收窄（2026-09-21）**：**非标准参数目前不做设计与要求**；对不同皮套的支持**只**通过「**用户手动调整表演强度的基础参数**」实现。⇒ 白名单**保持现状**（`preset/table.rs` 的 13 个标准参数不动，不开 80+ 非标准参数）；需要新增的只是「每皮套一组可手调的基础强度」，而它**恰好就是现有的 `[action]` 三个倍率**（head / body / expression）——并与 Q2 的三个字段一一对应：**body 倍率 ↔ 字段 1、head 倍率 ↔ 字段 2、expression 倍率 ↔ 字段 3**。**W2 正在修的就是这条链路**（让它真正有效），所以 W2 就是「不同皮套支持」的地基。
2. **导演 AI 的「使用手册」必须与字段表/schema 同源生成**，不能在提示词里手抄一份——否则会重演本仓「同一个数写 3 遍」的老毛病（preset_labels.json 存在的理由就是它）。
### 10.3 Q5 推荐：**渲染面「事件级」ack 帧**（不是每帧状态流，也不是前端镜像）

1. **渲染面才是参数真源**。前端只知道「我发了什么」，实际生效受三件事影响：
   ① 皮套缺该参数时 override_parameter 返回 false **静默降级**（crates/l2d-wasm-demo/src/web/surface/input.rs:19-26）；
   ② 越界被 clamp_to_channel **钳位**；③ 同帧还有 **physics + idle** 叠加。
   ⇒ 前端镜像**必然撒谎**，而且会掩盖「点了没反应 = 皮套缺这条参数」这个最该暴露的事实。
2. **成本不对等**：不需要每帧流，只要**事件**——preset_applied / preset_replaced / preset_expired / preset_dropped（缺参数）各一条，携带「当时生效的字段 + 强度 + 是否被钳位或降级」。这正好就是导演要的「上一个动作字段是什么 / 结束了没有」。
3. **与项目纪律一致**：stage-ack 已是「做了什么就回什么」的先例（crates/l2d-wasm-demo/src/main.rs:560-568）；而 HUD 的 preset: 行**只画进 DOM、不上报**，正是那条「看得见但没人看得见」的坑。

⇒ 落地：渲染面新增 4 条**事件**帧（非状态流）→ 前端 → 汇入日志文本 → 喂导演（配合 Q5 的喂日志）。

### 10.4 Q6 推荐：**先做事件级锚点，句内偏移一步到位**

1. **主锚点 = 段边界**，而它由 Q1 的**断句**天然给出（每段是独立 audio 元素，currentTime 相对**该段**起点）。所以 audio_start / after_prev 两个锚点就足以支撑「提线木偶」：「嗯……」→（段间：抬头或睁眼）→「我想到了。」
2. **句内偏移**（同一段音频里第 800ms 做动作）才需要**音频时钟下发**。前端**已经有** 30ms 粒度的该段音频时钟（shell/flutter/lib/audio/audio_player.dart:381-422，按 element.currentTime 查句内包络），所以再加一条轻量消息成本很低。
3. ⇒ 推荐：**一步到位**加 stage-clock 消息（{seg, pos_ms, playing}，每 30ms 或每帧），渲染面用它替代 performance.now() 当 now_ms；段边界继续用现有 sentenceStarts，并**新增「段结束」事件**（这是 Q4 的「上一个动作结束」的另一半）。

### 10.5 剩余待确认（Q8–Q11；**已由 §10.7 答复**，本表保留为提问原文）

| # | 问题 | 为什么影响实现 |
| --- | --- | --- |
| **Q8** | 「持续状态」（闭眼思考）是靠**重复输出**同一字段维持，而不是独立的 hold 字段？ | 决定要不要在协议里加 hold / release，还是靠导演按事件重发 |
| **Q9** | 导演输出**立即生效 + 同类相加**，所以**不需要动作队列**（导演自己按事件节奏输出）？合成语义是**相加**，还是也允许**覆盖或相乘**？ | 决定渲染面要不要新增 add_override 语义（现在是 set） |
| **Q10** | Q7 的「**角色基础状态值**」（开心/伤心/愤怒/思考…）住哪：**人设（persona）** / **Mod 配置** / assets 下**每角色一份 profile**？ | 决定「退回基础状态」的实现位置 |
| **Q11** | 「半身 4 向**拉升**」是**摆动/倾斜**（用现有 ParamBodyAngleX/Y），还是**形变拉伸**（本皮套**没有**该通道，需模型侧加参数）？ | 直接决定字段 1 能不能落地 |

### 10.7 答复落地（维护者 2026-09-21 之五）

**问题 1：模型 / Cubism 的映射字段能不能直接当导演的输出控制参数（即 LLM 直接写 ParamAngleX=… ）？**

我的建议：**不要直接写参数名**。保留「**语义字段 → 模型参数**」这一层映射表，四条理由：

| 维度 | LLM 直接写参数名 | 语义字段 + 映射表（建议） |
| --- | --- | --- |
| 换皮套 | 提示词要跟着换（128 个名字 + 量程 + 正负方向都变），每换一个模型就要重写提示词并重测 | 提示词**不变**；换皮套只换映射表（人工测试确定） |
| 语义 | LLM 得知道「点头是 AngleY 的负方向」这类**模型特定常识**，极易搞反 | LLM 只说「点头 / 轻微」，方向与量程由表定 |
| 安全边界 | 口型 ParamMouthOpenY、呼吸 ParamBreath 与头身在同一命名空间，靠提示词挡不可靠 | 白名单与排除项写在**代码 / 表**里，可测 |
| 「人工测基础强度」 | 无处可放（只能塞进提示词，且每模型一份） | 表的**系数与量程**就是它，与 Q2 的要求完全对齐 |

⇒ 折中：字段是**小词表**（`body` / `head` / `expression`），每个字段在**每皮套的表**里可以映射到**多个参数**
（例如「半身左右」同时驱动 `ParamBodyAngleX` 与 `ParamBodyAngleX3`（肩部 x））——这才是「适应不同皮套」的正确位置。
**非标准参数（翅膀 / 耳朵 / 光环 / 袖子 / 头发）按你的答复本轮不做**；将来若要，用一个受白名单约束的 `extra` 逃生舱即可。

**可实现清单（请你按这个对齐；这就是「可实现的部分」）**

| 字段 | 可以做 | 做不到 / 限制 |
| --- | --- | --- |
| **1 半身** | 左右摆动（`ParamBodyAngleX`）、前后倾（`ParamBodyAngleY`）、侧倾（`ParamBodyAngleZ`）；两轴**按比例合成**（同时给值）；可选叠加肩部 / 胯部 / 迈腿（`ParamBodyAngleX3` / `X6` / `X4` / `X5`，**需你确认是否纳入**） | **形变拉伸**（Scale / Stretch）：Cubism 标准参数与本皮套**都没有**该通道 ⇒ 做不到（要模型作者加参数） |
| **2 头部** | 摇头左右（`ParamAngleX`）、点头上下（`ParamAngleY`）、歪头倾斜（`ParamAngleZ`）；三轴按比例合成；可选叠加眼珠（`ParamEyeBallX/Y`）做视线跟随 | 超过 ±30 会被上限钳位（`preset/scales.rs:13-25`）；「拉伸脖子」做不到 |
| **3 表情** | 面板选择（现有 3 条：微笑 / 不悦 / 惊讶，可扩）；可用五官通道 = `ParamEyeLOpen/ROpen`、`ParamEyeLSmile/RSmile`、`ParamBrowLY/RY`、`ParamMouthForm`、`Param4`(嘟嘴)、`Param6`(歪嘴) | **`ParamMouthOpenY` 归 TTS 口型**，导演不得写 |
| **保持** | `hold` **可加**（你已确认）：保持到下次指令 | 「退回」需要「角色基础状态」接住（见下，绑会话） |

⚠ 一个需要你点头的取舍：**表情字段是否只写五官**。新字段分解下头 / 身已由字段 1 / 2 负责，
表情若继续带头身位移就会与字段 1 / 2 **抢同一参数**（现在是靠「手势赢」的规则收场）。
建议：**表情字段只写五官**，头身完全交给字段 1 / 2。→ **S1**
**Q8 答复**：**hold 可加**。带 hold 的字段**不自动回基准**；不带 hold 的按 ttl 到点回。

**Q9 答复**：**立即生效**（不排队、不为等音频而延迟动作）；**忽略 TTS 首 token 延迟**；
而且**动作的发生本身可以作为喂给导演的输入段点**（「某时刻做了什么」进日志，成为下一次决策的锚点）。⇒ **不需要动作队列**。

**唤醒规则（你给的）**：导演 LLM 的唤醒有**两个条件**——① TTS 播放完成 ② 动作做完；
你更倾向**两者同时满足**时唤醒（对齐「表演完成 + TTS 结束」）。
⇒ 落地：前端已有「该段音频 `ended`」；再补「动作做完」的事件（非 hold 动作 ttl 到点 → 渲染面事件 ack，见 §10.3），两者都到 → 组一条**段完成事件**喂导演。
（边界：若某段只下了 `hold` 的动作，就没有「动作做完」这个点——此时唤醒退化为只有「TTS 段播完」。这条我按此实现，若不妥请纠。）

**Q10 答复**：**角色基础状态值绑定会话**。⇒ 「退回」= 回到**该会话**的基础状态；不同会话（不同角色）各有自己的 baseline。
「断开（停止键 / 新用户消息）」= 清「动作 + 表情 + TTS 待播」+ 退回**该会话的 baseline**。
（本仓已有 per-session 作用域：`session_scope.rs` / `session_scopes.prompt_for(...)`，baseline 可作为它的兄弟字段，不用新造机制。）

**Q11**：你暂不回应 ⇒ 以上「可实现清单」就是对齐口径：**摆动 / 倾斜全部可做，形变拉伸不可做**。

**草案 schema（待你点头）**

```jsonc
{
  // 给 TTS 的文本：只能**切分原文**，逐字不变（Q1）
  "segments": [ "嗯……", "我想到了。" ],
  // 表演字段：同类可**重复出现**，按 add 合成；立即生效（Q9）
  "cues": [
    { "field": "body",       "x": 0.0, "y": 0.30,  "intensity": 1, "at": "now",   "hold": true  },
    { "field": "head",       "x": 0.0, "y": -0.40, "intensity": 2, "at": "seg:2", "hold": false },
    { "field": "expression", "id": "thinking",  "intensity": 1, "at": "now",   "hold": true  },
    { "field": "expression", "id": "surprised", "intensity": 2, "at": "seg:2", "hold": false }
  ]
}
```

| 键 | 取值 | 说明 |
| --- | --- | --- |
| `field` | `body` / `head` / `expression` | 小词表三选一（问题 1 的结论） |
| 轴值 | `x` / `y`（`head` 还可 `z`）∈ `[-1, 1]` | 多轴可同时给 = **按比例合成** |
| `intensity` | `1` = 轻微 / `2` = 中 / `3` = 强 | 与现有 cue 的 1..3 同域；渲染面乘**每皮套的基础强度**后 clamp |
| `at` | `now` / `seg:N` / `after_prev` | `seg:N` 就是你说的「**断句当锚点**」 |
| `hold` | `true` / `false` | 保持到下次指令；否则按 ttl 到点回 |
| `expression.id` | 表情面板的 id | 现有 `assets/actions/presets.json` 的 `kind=expression` 就是它的第一版 |

**下面两条建议已按此采纳**（维护者 2026-09-21 回复「你的理解已经完善」，未提出异议）：

- **S1**：表情字段**只写五官**（头 / 身完全交给字段 1 / 2）？建议「是」。
- **S2**：`at` 是否就用 `now` / `seg:N` / `after_prev` 这三个？
