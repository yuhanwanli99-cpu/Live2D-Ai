# Live2D-Ai Web 前端设计规格 v3（Flutter Web）

> 状态：**可执行设计规格**，面向照此写 Dart 代码的实现者。
> 范围：`shell/flutter/`（Flutter Web 外壳，由 Rust 在 `/app/` 同源托管）。
> 产物：本文件。**不要求实现者读 `docs/design/web-ui-spec-v2.md` / `web-ui-redo-spec-st1-2.md` /
> `web-ui-polish-spec.md` / `ui-preview-v2.html`** —— 那几份是已废弃的原生 JS 前端时代的产物，
> 与本规格无继承关系，**不作为设计依据**。
>
> 输入（本规格的全部依据，均已实读）：
> 1. `docs/research/ui-design-survey-companion-2026-09.md`（1447 行，同类产品视觉语言与 IA）
> 2. `docs/research/ui-design-flutter-architecture-2026-09.md`（855 行，Flutter 落地）
> 3. `docs/research/ui-design-oss-adoption-2026-09.md`（493 行，依赖 adopt/reject 与许可）
> 4. `docs/research/ui-design-control-surface-2026-09.md`（1084 行，控制台交互模式）
> 5. 现有代码基线：`shell/flutter/lib/**`（16 文件）、`shell/flutter/test/**`（6 文件 / 57 用例）、
>    `crates/live2d-ai-desktop/src/web_api/**`、`shell/README.md` §5–§8
>
> **证据等级**（沿用调研惯例，本文每条决策后标注）：
> `【已验证·本仓库】` 读本仓库源码确认 · `【已验证·官方一手】` 调研文档中已核验的官方证据 ·
> `【调研】` 指向上列调研文档的结论 · `【推断】` 本人判断 · `【裁决】` 四份调研冲突处由本规格裁定。

---

## 0. 如何使用本规格（实现者先读这一节）

### 0.1 三条不可推翻的约束（不要重新论证）

| # | 约束 | 落地检查 |
|---|---|---|
| C1 | **离线优先**：不依赖任何 Google CDN。构建必须 `--no-web-resources-cdn`；中文字体自托管（`assets/fonts/` Noto Sans SC 子集，家族名 `NotoSansSC`，400/700） | `test/theme_test.dart`（已有） |
| C2 | **新增运行时依赖 = 0**：只用 `flutter` / `http` / `web` + 内建 Material 3 | `pubspec.yaml` 只允许 3 个 dependencies |
| C3 | **渐进披露最多 2 层**：普通区 + `dev_mode` 高级区 | §4 每行的「层」列只能是「普通」或「dev」 |
| C4 | **`GET` / `PATCH /api/v1/settings`**（不是 PUT） | `match_route` 只认 `Method::Patch`【已验证·本仓库 `web_api/mod.rs:196-202`】 |
| C5 | **舞台必须保活**：切分区绝不能让 `Live2DStage` 离开 Widget 树 | §3.4 + §11 钉子 4 |
| C6 | **前端不复制核心逻辑**：状态机/动作仲裁/LLM/TTS 协议都在 Rust | §8.2（手动通道只走 HTTP） |
| C7 | **两个「静音」必须用两个词**（本机 / 服务端） | §7.2 + §11 钉子 6 |

### 0.2 基线事实（写代码前必须接受）

- 现有 `lib/` 16 个 Dart 文件、2982 行；`main.dart` 473 行内含 AppShell + 聊天面板 + 4 个私有组件。
- 现有 `test/` 6 个文件、**57 个用例全绿**（本次实读计数：`action_source_test` 12 / `audio_gain_test` 11 /
  `audio_schedule_test` 7 / `display_prefs_test` 17 / `live2d_bridge_test` 3 / `theme_test` 7）。
  **这 57 条是重写的回归基线，一条都不许删。**
- `action_state` 接线缺陷**已修复**（见 §0.3）：新增纯模块 `lib/api/action_source.dart` +
  12 条回归测试，`ws_client.dart` 已改为调用它。
- 门禁 = `cd shell/flutter && flutter analyze && flutter test`。**没有**其它钩子；
  自定义 lint 插件（`custom_lint` / `analyzer_plugin`）在 `pubspec.lock` 零命中，
  引入即违反 C2【调研 §1.2】。**所以本规格的全部守卫都写成 `test/` 里的测试。**

### 0.3 已落地的成果（迁移计划中一律写「保留」，不要重做）

1. **`lib/api/action_source.dart`**（纯逻辑，无 web 依赖）：`ActionSource` 枚举
   （`userCommand('user_command')` / `llmTool('llm_tool')` / `ruleFallback('rule_fallback')` / `unknown('unknown')`）
   + `ActionDetail.fromData(Object? raw)` 宽容解析 + `isAiInitiated` + `label`（`你`/`AI`/`规则`/`未知`）。
2. **`test/action_source_test.dart`**：含两条 **v0.4.5 本机实抓夹具** `kRealActionFrames`，
   把「服务端 `data.action` 是对象 `{action,strength,source}`」钉死。
3. `lib/api/ws_client.dart` 的 `ActionStateEvent` 现在带 `action / strength / source` 三个字段
   （`ActionDetail.fromData(data['action'])`）。**协议解析正确性这一环已完成。**
4. **仍然缺的只剩界面表达**：`chat/chat_controller.dart` 对 `ActionStateEvent` 仍是 `break`（完全忽略），
   所以「这个动作是你点的、还是 AI 自己做的」在界面上依然看不见。**§8.3 与 §10-P2 就是补这一段。**
5. **设置 API 层已在同一轮落地**（与本规格并行，尚未提交；实现者**不要重造**）：
   - `lib/api/settings_models.dart`（431 行，纯逻辑、无 web 依赖）：`sealed class Tri<T>`
     （`Tri.keep()` / `Tri.clear()` / `Tri.set(v)` —— 把服务端 `Option<Option<T>>` 的三态
     **在类型上**分开，头注解释了「把『不修改』也序列化成 `null` 会导致静默丢配置」）+
     `LlmSettingsView` / `TtsSettingsView` / `PersonaSettingsView` / `SettingsView` +
     `LlmSettingsPatch` / `TtsSettingsPatch` / `PersonaSettingsPatch` / `SettingsPatch` +
     `SettingsTestOutcome` + `SettingsPatchResult`。
   - `lib/api/api_client.dart`：**已可注入** `http.Client`（`ApiClient({String? base, http.Client? client})`），
     并新增 `fetchSettings()` / `patchSettings()` / `testLlm()` / `testTts()`。
   - `test/settings_api_test.dart`（315 行）用 `package:http/testing.dart` 的 `MockClient` 在 VM 上
     覆盖请求/响应契约（**零新依赖**，`http` 本就是运行时依赖）。
   → 因此本规格 §4.3 的三态构造与 §10-P4 的「patch 构造」**已由 `settings_models.dart` 承担**，
   P4 只需补 `SettingsController`（草稿/dirty/拦截）与 UI；**文件命名以已落地的为准**。

### 0.4 一个已知的文档与代码不一致（顺手修，别照旧文档写代码）

`shell/README.md` §8.1 声称「渲染端读 `payload.value` 而前端发 `payload.level`，口型不会生效」。
**该说明已过期**：`crates/l2d-wasm-demo/src/main.rs:361-368` 现在 **优先读 `payload.level`**，
仅在缺失时回退 `payload.value`【已验证·本仓库】。实现者按 v1 发 `level` 是正确的，
不必改发 `value`。**P0 阶段顺手把该条 README 说明改掉**（列出为待裁决 10）。

---

## 1. 设计原则（≤6 条，用来裁决后续分歧）

| # | 原则 | 一句话判据 | 依据 |
|---|---|---|---|
| P1 | **舞台是主角，UI 都是客人** | 任何新增常驻 UI 都要回答「它吃掉了多少舞台面积」；能浮不占、能收不钉 | 10 个开源样本里 7/10 用浮层而非分栏；VTS 桌宠形态零常驻 UI【调研 survey §1.1、§5】 |
| P2 | **一个品牌色，三个语义色，其余全派生** | 出现第二种「强调色」即为设计缺陷；颜色只能来自 `ColorScheme` 槽位或 `AppColors` | 8 个项目栽在多个并列强调色上（N.E.K.O 五个近似蓝、SoW/Nexus 各 6 套）【调研 survey §8 缺陷 1】 |
| P3 | **状态只有一个契约** | 「色 + 形 + 字 + 节奏」四通道里至少占两个；禁止任何页面自造状态样式 | 6 个项目的状态指示缺失/只有文字/连接态被隐藏；只有 Meuxe 与 Soul of Waifu 做对【调研 survey §8 缺陷 3、§9】 |
| P4 | **静默失效是最危险的失败模式** | 任何「引用不存在的令牌」「加了字段忘了 `copyWith`」「控件被 `display:none` 吃掉」都必须让测试变红 | 4 个独立项目复现：Amica 丢 preset（≈14 处类名零样式）、Meuxe 的 `clay-600`、SAP 的 `--primary-color` 用了 9 次从未定义、Live2DPet 空 type 撞 `display:none` 导致状态永久隐身【调研 survey §8 缺陷 4】 |
| P5 | **一个词只命名一个轴** | 「听不听得到」与「有没有声音被生产出来」必须是两个词 | OBS 因同类歧义把 `Monitor and Output` 改名 `Monitoring Enabled`、`Upload Last Log File` 改 `Upload Previous Log File`（官方自认旧标签有歧义）【调研 control §8-14、§6】 |
| P6 | **高频信号永不进入 Widget 树** | 口型电平（30 Hz）走 `GlobalKey → Bridge → postMessage`，不得经 `ChangeNotifier` 触发 rebuild | 现有实现已正确（`main.dart:104-106`）【调研 flutter §3.2、§5.9】；这是本项目最值得保护的性质 |

> **冲突裁决**：调研 survey 的 P1-7 建议「默认形态 = 舞台满幅 + 聊天浮层」，而 flutter-arch §2.4 建议
> 「宽屏三栏（rail + 舞台 + 聊天）」。本规格取三者交集：**舞台满幅 + 聊天常驻侧栏 + 设置浮层**
> （见 §3）。依据：① 任务书已定「聊天是常驻面板」；② 聊天是文字面，浮在角色上会遮挡口型（本产品的核心观感）；
> ③ 设置是低频面，用浮层最符合 P1。

---

## 2. 设计 token

### 2.1 落地载体：`ColorScheme` + `ThemeExtension` + `const`（三处分工，不要混）

**结论（采纳 flutter-arch §1.1 的折中建议）**：

| 维度 | 载体 | 为什么 |
|---|---|---|
| 语义槽位色（`primary`/`surfaceContainerHighest`/`error`/`outline`…） | `ColorScheme.fromSeed(seedColor: kSeedColor, brightness: Brightness.dark)`，**逐槽位覆写**已知不符的槽 | Material 已经定义了这些语义，重造会双份真相 |
| `hairline` / `hoverWash` / `glassScrim` / `glassBarrier` / `stageBackdrop` / `bubbleAssistant` / `dangerSurface` / `dangerBorder` / 三个语义色 | **`ThemeExtension<AppColors>`** | 这些必须**随明暗/主题插值**，`lerp` 是 `ThemeExtension` 相对常量类的唯一实质优势【调研 flutter §1.1】 |
| 间距 / 圆角 / 动效时长 / 动效曲线 / 断点阈值 / 字号 | **`const` 常量类**（`Space`/`Radius`/`Durations`/`Motion`/`Breakpoints`/`AppFontSizes`） | dark-only 应用（现有 `main.dart:53-67` 只有 `Brightness.dark`，无亮色主题）**没有主题动画需求**，`lerp` 价值≈0；`const` 能进 `const` 构造、零运行时开销、消除一层间接 |

**明令禁止**：
- ❌ 业务代码写 `Color(0x…)` 或 `Colors.<name>`（唯一豁免：`lib/design/` 下的 2 个文件，见 §2.8）。
- ❌ 业务代码写 `fontSize:` 或 `textTheme.x.copyWith(fontSize:)`（现有 `display_panel.dart:83,104` 就是反例。
  11 px 这一级必须成为正式槽位 `labelSmall`，而不是就地加字号）。
- ❌ 业务代码写 `BorderRadius.circular(8)` 这样的裸数字。
- ❌ 用字符串查表取令牌（`tokens['md']`）。**令牌引用必须是编译期常量**——这样「引用不存在的令牌」
  直接是编译错误，把 P4 的一半风险提前到编译期【调研 survey §10-P0-2】。
- ❌ 组件层覆盖字体族（Nexus 教训：V2 两个核心 CSS 丢掉中文字体）【调研 survey §7.8.6.10】。
- ❌ 亮色主题（`Brightness.light` 分支一律不写；写死 dark 是刻意的，不是遗漏）。

### 2.2 颜色 token（全部具体值）

**品牌与底**（`const`，进 `lib/design/tokens.dart`）

| token | 值 | 用途 |
|---|---|---|
| `kSeedColor` | `#7C5CFF` | `ColorScheme.fromSeed` 的唯一种子（沿用现有产品视觉基因，不换） |
| `stageBackdrop` | `#0B0D12` | `scaffoldBackgroundColor` + 舞台底（合并现有 `main.dart:64` 与 `live2d_stage.dart:158` 两处硬编码） |
| `chatPanelSurface` | `#10131A` | 聊天面板底（现 `main.dart:302`） |
| `bubbleAssistant` | `#1B2030` | 助手气泡底（现 `main.dart:461`） |
| `dangerSurface` | `#3A1D1D` | 失败气泡/错误条底（现 `main.dart:420,460`、`live2d_stage.dart:245`） |
| `dangerBorder` | `#8A3B3B` | 错误条描边（现 `live2d_stage.dart:247`） |

**语义色**（`const`，放 `AppSemanticColors`，随 `AppColors` 一起进 `ThemeExtension` 以便将来插值）

| token | 值 | 用途 |
|---|---|---|
| `success` | `#3DD68C` | `WsStatus.connected`、测试通过、Mod running |
| `warning` | `#F5A524` | `WsStatus.connecting/disconnected`、`apply_status = RestartRequired`、服务端静音徽标 |
| `danger` | `#FF6B6B` | 失败、`WsStatus.closed`、除零/解析失败 |

**玻璃/叠色**（`ThemeExtension`，因为依赖 `onSurface`，必须能随主题插值）

| token | 值 | 用途 |
|---|---|---|
| `hairline` | `onSurface` @ **0.12** | 1 px 分隔线 / 卡片描边（dark 下**用描边表达层级而非投影**） |
| `hoverWash` | `onSurface` @ **0.06** | 悬停底色 |
| `glassScrim` | `#000000` @ **0.60** | 舞台徽标/浮动字幕底（现 `live2d_stage.dart:205`） |
| `glassBarrier` | `#000000` @ **0.32** | 弹层遮罩 |
| `contentMuted` | `onSurface` @ **0.74** | **承载文字信息**的次要文本（≥ 12 px） |
| `contentFaint` | `onSurface` @ **0.60** | **仅装饰/图标**，不得承载文字信息 |
| `focusRing` | `colorScheme.primary` @ **1.0** | 键盘焦点环（**必须不透明**，见 §9.2） |
| `serverMutedBadge` | `warning` @ 0.18 底 + `warning` 描边 | 服务端静音只读徽标（§7） |

> **不做毛玻璃**：`BackdropFilter` 模糊**不到 iframe platform view**（官方 issue #184996 / #89889：
> PlatformView 内容「passes through completely un-blurred… no workaround」），却照付离屏渲染代价
> 【调研 oss §7】。所以 AIRI 的「4 px 描边 + backdrop-blur」配方**只取前一半**：
> **半透明纯色 + 1 px 高光描边 + `hairline`**。`GlassPanel` 组件名保留，实现不含模糊。
> 全仓库 `BackdropFilter` 命中数必须为 **0**，由 §11 钉子 10 守住。

### 2.3 字号阶梯（8 级，映射到 `TextTheme` 槽位；业务只读槽位）

**唯一字体族**：`kAppFontFamily = 'NotoSansSC'`（`ui/theme.dart` 已定义，`theme_test.dart` 守着）。

| # | 槽位 | size / weight / lineHeight | 用途 | 现网对应 |
|---|---|---|---|---|
| 1 | `labelSmall` | 11 / w500 / 16 | 角色名、来源徽标、时间戳、字段单位 | `main.dart:470`、`display_panel.dart:83,104` |
| 2 | `bodySmall` | 12 / w400 / 18 | 辅助说明、错误条正文、日志行 | `main.dart:269,420` |
| 3 | `labelLarge` | 13 / w500 / 18 | 按钮文字、字段标签 | `live2d_stage.dart:260` |
| 4 | `bodyMedium` | 14 / w400 / 22 | **正文**：聊天气泡、字段值、设置说明 | 全局 |
| 5 | `titleSmall` | 14 / w600 / 20 | 分组小标题（「舞台与口型」） | — |
| 6 | `titleMedium` | 16 / w600 / 24 | 分区标题、面板标题 | `display_panel.dart:41` |
| 7 | `titleLarge` | 20 / w600 / 28 | 应用名 / 大屏空态标题 | `main.dart:312` |
| 8 | `headlineSmall` | 24 / w600 / 32 | 首次引导页标题（唯一可选级） | — |

**两条硬规则**：
- **16 px 只有 w600 一个版本**（现状 `main.dart:312` 用 16/w700 而 `display_panel.dart:41` 用 16/w600，
  同为 16 两个字重 = 没有阶梯的症状）【调研 flutter §1.3】。
- **中文界面字号下限 11 px**；正文不低于 12 px，`bodySmall` 只用于辅助信息。
  依据：Nexus 中文界面出现 9–11 px 共 59 处，低于可读门槛【调研 survey §7.8.6.10】。

### 2.4 间距（4 px 基准网格，9 档）

| token | px | 典型用途 |
|---|---|---|
| `Space.s0` | 0 | — |
| `Space.s1` | 4 | 图标与文字、气泡内标签与正文 |
| `Space.s2` | 8 | 相邻控件、卡片内边距（紧凑） |
| `Space.s3` | 12 | 卡片内边距、列表行距 |
| `Space.s4` | 16 | 面板内边距、区块之间 |
| `Space.s5` | 20 | 面板外边距（弹层） |
| `Space.s6` | 24 | 大区块之间、空态内边距 |
| `Space.s7` | 32 | 设置分区之间 |
| `Space.s8` | 48 | 引导页大留白 |

**禁止**：`Space.s1` 与 `Space.s2` 之外的奇数间距；禁止 `EdgeInsets.all(13)` 这类裸值。

### 2.5 圆角（6 档，**档位两两不同值**）

| token | px | 用途 |
|---|---|---|
| `Radius.none` | 0 | 全宽分区头、`Divider` |
| `Radius.xs` | 4 | 密集列表内元素、进度条 |
| `Radius.sm` | 8 | 输入框、按钮、行内徽标、`ErrorBanner` |
| `Radius.md` | 12 | **聊天气泡**、卡片、动作格（默认档） |
| `Radius.lg` | 16 | 设置面板、舞台浮层、`GlassPanel` |
| `Radius.xl` | 20 | 底部弹层（sheet）、模态 |
| `Radius.pill` | 999 | 状态 Pill、连接徽标、圆形图标按钮 |

> **裁决（F）**：flutter-arch §1.3 草案给 4/6/8/10/12，survey §9 推荐 Meuxe 的 12/12/16/20/24。
> 本规格取 **4/8/12/16/20/999**：删掉 6 与 10 这种「一像素一档」（Nexus 的 `--radius-*` 四档同值是
> token 无信息量的极端反例，Meuxe 则是 5 档成体系），同时采 2025–26 收敛的大圆角趋势
> （N.E.K.O 16–20 px、Meuxe 12–24 px），保留 4 px 给密集列表内元素。

### 2.6 阴影（2 档，默认无阴影）

| token | 值 | 用途 |
|---|---|---|
| `shadowNone` | — | **默认**：dark 下用 1 px `hairline` 描边表达层级 |
| `shadowOverlay` | `#000000` @ 0.45, blur 16, offset (0,4) | **仅**模态/sheet/浮动字幕；**禁止**用于静态卡片 |
| `shadowFocus` | — | 不存在：焦点用 1 px 不透明 `focusRing` 边框，保证对比度可测 |

依据：Meuxe 的设计原则原文 “**Flat, not floating. Nothing casts a drop shadow.**”，
其阴影本质是 `0 0 0 1px rgba(20,20,25,.07)` 的描边【调研 survey §9】。dark 下大面积 blur 在 Web 上昂贵
【调研 flutter §5.11】。

### 2.7 动效（**2 条曲线 + 4 档时长**，全部具名）

| token | 值 | 用途 |
|---|---|---|
| `Motion.enter` | `Cubic(0.16, 1.0, 0.3, 1.0)` | 入场/出场（快出慢收）。AIRI 与 Nexus **独立收敛到同一条**曲线，属跨项目验证过的值【调研 survey §9】 |
| `Motion.state` | `Curves.easeInOutCubic` | 状态过渡、颜色/尺寸变化、开关 |
| `Durations.fast` | 120 ms | hover / pressed / 开关 / 徽标切换 |
| `Durations.base` | 200 ms | 内容切换、消息渐入、横幅滑入、Pill 切换 |
| `Durations.slow` | 320 ms | 模态 / sheet 入场（`scale .96→1` + fade） |
| `Durations.reveal` | 600 ms | **仅此一处**长动画：启动揭示 |

**规则**：
- 一个组件内不得出现第二种曲线；不得写裸 `Duration(milliseconds: 250)`（`api/`、`audio/` 的协议时长除外，
  那是协议常量不是 UI 动效，见 §2.8 白名单）。
- **禁止**：逐项入场 stagger（N 项 = N 次重绘）、长时间 shimmer 骨架屏（且 `CircularProgressIndicator`
  与 `repeat()` 动画**免疫** `disableAnimations`，见 §9.4）、自定义 fragment shader/粒子
  【调研 flutter §5.11】。
- 主题切换需要整页过渡（AIRI 的 `html { transition: all .3s }` 是极低成本的高级感细节【调研 survey §2.7-5】），
  但本应用 dark-only、无换肤，**故不做**。

### 2.8 零裸色值 / 零裸字号如何被 `flutter test` 守住（**枚举式双向校验**，零新依赖）

**为什么不能只做扫描**（这是本条的关键）：扫描只能覆盖「引用侧的一类写错方式」，覆盖不了
「声明了但没人用」「档位同值」「`ThemeExtension` 加了字段忘了 `copyWith`/`lerp`」
「主题里根本没注册这个 extension」——而这四类**都是静默失效**，正是 4 个项目栽的坑（P4）。
所以守卫 = **扫描（引用侧）+ 登记表（声明侧）+ 结构枚举（字段侧）**，三层互补。

#### 2.8.1 声明侧：每个 token 家族带一份**登记表**（只服务测试）

```dart
// lib/design/tokens.dart —— 唯一允许出现裸色值/裸字号的两个文件之一
abstract final class Radius {
  static const double none = 0, xs = 4, sm = 8, md = 12, lg = 16, xl = 20, pill = 999;

  /// 令牌登记表。**生产代码不得用本表取值**（禁止字符串查表）；
  /// 它的唯一用途是让 `design_tokens_test.dart` 能枚举声明侧。
  static const Map<String, double> registry = <String, double>{
    'none': none, 'xs': xs, 'sm': sm, 'md': md, 'lg': lg, 'xl': xl, 'pill': pill,
  };
}
```

`Space` / `Durations` / `Motion`（曲线没有可枚举的数值，改登记 `List<String> names`）/
`AppFontSizes`（登记 `Map<String, ({double size, FontWeight weight})>`）同理。

#### 2.8.2 测试一 `test/design_tokens_test.dart`：**声明侧 ↔ 引用侧双向遍历**

```dart
// ① 声明侧自洽：数量锁 + 档位互异（防 Nexus 的「四档同值」）
test('每个 token 家族的登记表长度与预期一致', () {
  expect(Radius.registry.length, 7);      // 硬编码期望：新增档位必须显式改测试 = 强制复核
  expect(Space.registry.length, 9);
  expect(Durations.registry.length, 4);
  expect(Motion.names.length, 2);
  expect(AppFontSizes.registry.length, 8);
});

test('同一家族内不得有重复取值（否则档位没有信息量）', () {
  // `AppFontSizes.sizesOf` 是 `registry`（槽位 → (size, weight)）的派生视图
  // （槽位 → size）：两个字重可以共用 w600，但**两个字阶不得同为 16 px**
  // ——这正是现状 `main.dart:312`(16/w700) 与 `display_panel.dart:41`(16/w600) 的病。
  for (final Map<String, Object> reg in <Map<String, Object>>[
    Radius.registry, Space.registry, Durations.registry, AppFontSizes.sizesOf,
  ]) {
    final List<Object> values = reg.values.toList();
    expect(values.toSet().length, values.length, reason: '存在同值档位');
  }
});

// ② 引用侧枚举：扫 lib/**，对每个 token 名统计引用次数
//    - 每个声明过的名字必须至少被引用 1 次（防死令牌：Nexus 的 var(--shadow-*) 0 引用）
//    - 每个被引用的名字必须能在登记表里找到（编译期已保证，这里把约定写死，
//      并对将来可能引入的字符串式间接引用报警）
test('所有声明的 token 都被引用，且所有引用都能解析', () {
  final Map<String, int> refs = countTokenReferences('lib', <String>[
    'Radius.', 'Space.', 'Durations.', 'Motion.', 'AppFontSizes.',
  ]); // 逐名统计 `Radius.md` 这类出现次数；见下方 helper 说明
  for (final String name in Radius.registry.keys) {
    expect(refs['Radius.$name'], greaterThan(0), reason: 'Radius.$name 声明后无人引用');
  }
  for (final String key in refs.keys) {
    expect(_knownTokenNames, contains(key), reason: '$key 被引用但登记表里没有');
  }
});

// ③ 字段侧枚举：ThemeExtension 的新增字段必须被 copyWith / lerp / values 覆盖
test('AppColors 的每个字段都被 copyWith 与 lerp 处理（防「加了字段忘了 copyWith」）', () {
  final AppColors base = AppColors.of(sampleScheme);
  final List<Object?> copyWithResults = AppColors.sentinelCopyWithProbes(base);
  // 逐个字段改成哨兵值后必须与原对象不等；全部相等 = copyWith 漏了字段（静默失效）
  expect(copyWithResults.every((Object? r) => r != base), isTrue);
  expect(AppColors.of(sampleScheme).toValuesMap().length, AppColors.fieldCount);
  expect(AppColors.lerp(base, other, 0.0), base);
  expect(AppColors.lerp(base, other, 1.0), other);
  expect(AppColors.lerp(base, other, 0.5), isNot(base)); // 防 lerp 直接 return this
});

// ④ 注册侧：extension 真的在主题里（`theme.extension<AppColors>()` 为 null 就是 SAP 那类静默失效）
test('AppColors 已在主题中注册，且 TextTheme 8 个槽位与登记表一致', () {
  final ThemeData theme = buildAppTheme();
  expect(theme.extension<AppColors>(), isNotNull);
  expect(theme.extensions!.whereType<AppColors>().length, 1);
  for (final String slot in AppFontSizes.registry.keys) {
    expect(AppFontSizes.sizeOfSlot(theme, slot), AppFontSizes.registry[slot]!.size);
  }
});
```

`countTokenReferences` 是实现约 30 行的 helper：遍历 `lib/**/*.dart`，**先剥离注释与字符串字面量**，
再用 `RegExp(r'\b(Radius|Space|Durations|Motion|AppFontSizes)\.([A-Za-z0-9_]+)')` 收集引用
（与下面 2.8.3 共用同一个剥离函数）。

#### 2.8.3 测试二 `test/design_tokens_lint_test.dart`：**裸值扫描（引用侧的兜底）**

| 规则 | 模式 | 豁免（唯一） |
|---|---|---|
| 裸色字面量 | `Color(0x` / `Colors.<name>` | `lib/design/tokens.dart`、`lib/design/typography.dart` |
| 裸字号 | `fontSize:` | 同上两个文件 |
| 裸圆角 | `BorderRadius.circular(<数字字面量>)`（`Radius.md` 变量合法） | 同上 |
| 裸断点 | `maxWidth >=` / `maxWidth <` | `lib/design/breakpoints.dart` |
| 毛玻璃 | `BackdropFilter` | **无豁免**（§2.2） |
| 协议时长混用 | `Duration(milliseconds:`（允许 `lib/api/`、`lib/audio/`） | 那两个目录 |

实现约定与已知边界（诚实记录，沿用 flutter-arch §1.2）：
- 文本扫描**不理解 AST**；剥离注释/字符串后仍可能被极端写法绕过（如拼接出的 `Color(`），
  也对 `style: someVar` 这类**我们想要的**间接引用不误报。
- 对 `EdgeInsets` / `SizedBox` 的间距规则**误报率高**（`EdgeInsets.zero` / `SizedBox.shrink()` 合法），
  故 v1 **不上**间距扫描，靠 §2.8.2 的引用侧枚举 + review 兜住。
- **豁免面越小，规则越难被侵蚀**：豁免只有 2 个文件、1 个目录。

#### 2.8.4 为什么不用 `custom_lint`

能拿到真实 AST、误报极低，但 (a) 引入 dev 依赖需过许可审查（违反 C2 的口径）、
(b) 必须挂在 `flutter analyze` 之外的命令上，要么改门禁、要么没人跑【调研 flutter §1.2】。
**若将来门禁允许扩展命令，这是升级路径，届时再评估。**

---

## 3. 信息架构

### 3.1 三者的关系（已确认结论，不要重新设计）

```
                   ┌─────────────────────────────────────────────┐
   常驻，永不销毁   │                 舞台 Stage                  │  ← iframe /render（Rust/wasm）
   （C5）          │   IndexedStack 的 index 0，任何情况下都在树里  │     不占 Flutter 渲染管线
                   └─────────────────────────────────────────────┘
   常驻面板         ┌─────────────────────────────────────────────┐
   （不是导航目的地）│                 聊天 Chat                   │  ← 消息流 + 输入区 + 音频条
                   └─────────────────────────────────────────────┘
   导航只承载这 8 项 ┌─────────────────────────────────────────────┐
                   │ 角色卡 / 模型库 / LLM / 语音合成 /            │  ← NavigationRail（≥900）
                   │ 动作与互动 / Mod / 诊断 / 开发模式            │     抽屉（<900）
                   └─────────────────────────────────────────────┘
```

- **聊天不是导航目的地**：宽/中屏它是常驻侧栏，窄屏它是常驻下半屏（带 ≥24 px 命中区的折叠把手）。
- **导航项 = `SettingsSection` 枚举**（8 个），由 §4 的声明式清单驱动，**不是手写导航**。
- **舞台保活**（C5）：`AppShell` 的 body 用 `IndexedStack`/`Stack` 承载舞台与聊天；
  设置内容**永远以浮层或并列页出现，绝不替换 body**。理由：`Live2DStage` 由
  `ValueKey<int>(_generation)` 标识 iframe（`live2d_stage.dart:160`），Widget 被卸载即 iframe 重建
  → 模型重新加载（`retry()` 的语义就是重建）【调研 flutter §2.5】【已验证·本仓库】。

### 3.2 断点（纯函数，`lib/design/breakpoints.dart`）

```dart
enum SizeClass { compact, medium, expanded }   // <900 / 900–1279 / ≥1280

/// 纯函数、零依赖，可 VM 单测。
SizeClass sizeClassOf(double width) =>
    width >= 1280 ? SizeClass.expanded
  : width >= 900  ? SizeClass.medium
  :                 SizeClass.compact;
```

现状 `main.dart:208` 的 `constraints.maxWidth >= 900` 是反例：**不可单测**，且魔法数字散落。
抽成纯函数是本次重构在可测试性上最大的净收益【调研 flutter §4.2 L1 缺口】。

### 3.3 三档布局（具体形态）

**≥1280（expanded）——三栏 + 设置侧板浮在舞台列之上**

```
┌──────────┬────────────────────────────────────────┬───────────┐
│ Rail     │  Stage（flex，min 480，常驻不销毁）      │  Chat     │
│ extended │  ┌──────────────────────────────┐      │  340px    │
│ 232px    │  │  SettingsPane 400px          │      │  常驻      │
│ 带文字    │  │  （overlay，贴舞台列右缘，     │      │           │
│          │  │    Esc / 点空白关闭）          │      │  ┌──────┐ │
│ 角色卡    │  │                              │      │  │音频条 │ │
│ 模型库    │  │  舞台仍在跑（iframe 未重建）    │      │  └──────┘ │
│ LLM      │  └──────────────────────────────┘      │           │
│ 语音合成  │                                        │  [输入框]  │
│ 动作与互动│                                        │           │
│ Mod      │                                        │           │
│ 诊断      │                                        │           │
│ 开发模式  │                                        │           │
└──────────┴────────────────────────────────────────┴───────────┘
   1280 时：232 + 340 = 572，舞台列 708；设置侧板 400 只覆盖右半，左侧约 300px 舞台仍可见
   → 满足「滑杆与舞台同屏」（`display_panel.dart:11-12` 明确要求的手感）
```

- Rail：`NavigationRail(extended: true, minWidth: 232)`。M3 对 expanded 的取向就是带标签的 rail
  【调研 flutter §2.4】。
- 设置内容**内联**（不是弹层）：`DisplayPrefs` 的价值在「拖动滑杆立刻看到舞台反应」，
  内联让滑杆与舞台同屏是这个产品**唯一必须保住的手感**【调研 flutter §2.4】。
  但为保住舞台宽度，它**浮在舞台列的右侧**而不是新增一列——**这是对 flutter-arch §2.4 的修正**（见附录 A-2）。

**900–1279（medium）——icon rail + 底部 sheet**

```
┌────┬──────────────────────────────┬───────────┐
│Rail│   Stage（flex）               │  Chat     │
│ 80 │                              │  320px    │
│ 图 │                              │           │
│ 标 │                              │           │
│ 8  │                              │           │
│ 项 │                              │           │
└────┴──────────────────────────────┴───────────┘
        选中分区 → showModalBottomSheet
        （isScrollControlled, maxWidth 560, maxHeight 0.78 × 视口高）
        → 舞台顶部仍可见，overlay 不卸载下层 ⇒ iframe 不重建
```

- Rail：`NavigationRail(extended: false)`，80 px icon-only，`labelType: all` 走 tooltip 补标签。
  900 时 80 + 320 = 400，舞台 500，仍足够。
- 弹层用 Material 的 `showModalBottomSheet`：**自带焦点陷阱 + Esc**，且不卸载下层
  【调研 flutter §6.3、§6.4】。

**<900（compact）——纵向 stage/chat + 抽屉**

```
┌───────────────────────────────────┐
│  Stage（flex 3，常驻不销毁）        │
│                                   │
├───────────────────────────────────┤   ← 可拖拽把手（命中区 ≥ 24px）
│  Chat（flex 2）                    │
│  ┌─────────────────────────────┐  │
│  │ 音频条（主音量/本机静音/服务端）│  │
│  └─────────────────────────────┘  │
│  [输入框]                          │
└───────────────────────────────────┘
  设置入口 = 聊天面板头部一个 IconButton（⚙）+ Ctrl/Cmd+,
  → NavigationDrawer 列 8 分区（第 1 层）
  → 选中后 showModalBottomSheet(maxWidth 520, maxHeight 0.72 × 视口高)（第 2 层）
```

> **裁决（A）**：flutter-arch §2.4 建议 compact 用底部 `NavigationBar` + 底部弹层。
> 本规格**不采用 NavigationBar**，改用抽屉。理由：
> ① M3 的 `NavigationBar` 是 3–5 个 destination，而我们有 **8 个分区**；
> ② 用「5 项 + 更多」会变成「设置 → 更多列表 → 分区内容」= **3 层**，
>    直接违反 NN/g 的官方硬约束「**designs that go beyond 2 disclosure levels typically have low usability**」
>    【调研 control §3.2、§8-15】；
> ③ 抽屉 + 弹层恰好是 2 层，且与 medium/expanded 共用同一份分区清单与同一个内容 Widget。
> **代价（诚实记录）**：compact 下弹层必然遮住大部分舞台（390 px 视口 + 0.72 高弹层）。
> 保底是限制弹层高度让舞台顶部露出一条（flutter-arch §2.4 的诚实回答）。

### 3.4 舞台保活的实现约束（C5 的落地写法）

| 做法 | 判定 |
|---|---|
| `IndexedStack(index: …)` 承载舞台与聊天 | ✅ |
| `Stack` + `Offstage`/`Visibility(maintainState: true)` | ✅ |
| `showModalBottomSheet` / `showDialog` 承载设置 | ✅（overlay 不卸载下层） |
| 用 `if (wide) stage else chat` 条件表达式 | ❌ iframe 重建 |
| 用 `Navigator.push` 全屏设置页 | ❌ 舞台离开树 |
| 每次 `setState` 重建 `Live2DStage(...)` 的新实例但 `key` 不变 | ✅（同 key 同类型 → 状态保留） |

**代价（必须知道）**：`IndexedStack` 中非选中子项仍被布局；iframe 平台视图在浮层覆盖下会继续运行。
对 Live2D 而言这是**期望行为**（口型动画不该停）【调研 flutter §2.5】。

---

## 4. 设置分区清单（对齐后端真实能力）

### 4.0 分区声明（单点真相）

新增一个分区 = 改**一处**枚举 + 加一个 pane builder。**不要**像 AIRI 那样把 `order` 分散到 8 个文件
（改一次顺序要动 8 个文件，且重号不报错）【调研 survey §2.6.2】。

```dart
// lib/settings/settings_sections.dart（纯声明，无 web 依赖）
enum SettingsSection {
  persona   ('角色卡',   '人设、开场白与历史轮数',       Icons.badge_outlined),
  models    ('模型库',   '导入、激活与舞台显示配置',     Icons.view_in_ar_outlined),
  llm       ('LLM',     '对话模型的服务地址、模型名与密钥', Icons.hub_outlined),
  tts       ('语音合成', 'TTS 服务、音色与采样参数',      Icons.record_voice_over_outlined),
  actions   ('动作与互动', '舞台口型、动作试演与拖动缩放',  Icons.touch_app_outlined),
  mods      ('Mod',     '扩展模块的启停与配置',          Icons.extension_outlined),
  diagnostics('诊断',    '连接、延迟、帧率与日志',        Icons.monitor_heart_outlined),
  developer ('开发模式', '高级参数与开发者工具',          Icons.terminal);
  const SettingsSection(this.label, this.description, this.icon);
  final String label; final String description; final IconData icon;
}
```

顺序 = 声明顺序（枚举天然有序）：**角色（人设 + 模型）→ 能力（LLM + TTS）→ 行为（动作与互动）→
扩展/诊断/开发**。每项**强制带 `description`**（AIRI 的每张卡片都有一句人话说明，
用户不需要先学会产品黑话【调研 survey §2.3】），由 §11 钉子 12 断言非空。

**「外观与语音」去哪了**：任务书的分区清单里没有独立的「外观与语音」，而 flutter-arch §2.1 的 7 分区里有。
本规格的处置（**裁决 B**）：
- **主音量 / 本机静音 / 服务端静音徽标** → **升为常驻控件**，放在聊天面板底部的**音频条**里（§7.4）。
  依据：NN/g「the very fact that something appears on the initial display tells users that it's important」，
  且音量/静音是**一分钟内会调第二次**的参数（§8.4 四问之一）；6 个同类项目都栽在「音量只在设置里或干脆没有」
  【调研 survey §8 缺陷 6、§10-P1-17】。
- **缩放 / 口型灵敏度 / 口型同步 / 待机小动作 / 允许拖动与缩放** → 归入「动作与互动」分区的
  **「舞台与口型」分组**（这些正是"互动参数"）。
- 收益：分区数保持 8（AIRI 的实测卡片菜单上限也是 8），且高频控件不埋在设置里。

### 4.1 字段级清单（逐区）

图例：**控件** `T`=文本输入 `N`=数字输入 `S`=滑杆 `SW`=开关 `SEG`=分段按钮 `DD`=下拉 `BTN`=按钮/动作 `RO`=只读展示。
**层**：`普通` / `dev`（只有 `dev_mode=true` 时才渲染）。

#### 4.1.1 角色卡（persona）

| 字段 | 控件 | 路由 | 层 |
|---|---|---|---|
| `persona.name` | T | `GET`/`PATCH /api/v1/settings` 的 `persona` 段 | 普通 |
| `persona.description` | T（多行） | 同上 | 普通 |
| `persona.first`（开场白） | T（多行） | 同上 | 普通 |
| `persona.personality` | T（多行） | 同上 | 普通 |
| `persona.scenario` | T（多行） | 同上 | 普通 |
| `persona.system_prompt` | T（多行） | 同上 | **dev** |
| `persona.max_history_pairs` | N | 同上 | **dev** |

来源：`settings/view.rs:54-61`（`PersonaView`）、`patch.rs:185-232`（`PersonaPatch` 七个字段）【已验证·本仓库】。
`max_history_pairs` 与 `system_prompt` 进 dev 区的理由：前者改错会显著影响延迟与费用，
后者与「人设」四项语义重叠、普通用户改它容易把角色说崩（§8.4 四问：不满足「错了能一键回」之外的可见后果一条）。

#### 4.1.2 模型库（models）

| 字段 / 动作 | 控件 | 路由 | 层 |
|---|---|---|---|
| 模型列表（`id` / `display_name` / `version` / `active` / `has_physics` / `size_bytes` / `texture_files.length` / `imported_at`） | 列表 | `GET /api/v1/models` → `{models:[...]}` | 普通 |
| 激活 | BTN（每行） | `POST /api/v1/models/{id}/activate` → `{active_id, prev_active_id, requires_restart, model_url}` | 普通 |
| 激活需重启提示 | RO 横幅 | 同上 `requires_restart` | 普通 |
| 导入本地模型 | T（输入 `id`）+ BTN | `POST /api/v1/models/import`，body `{"id":"<assets/models/ 下的目录名>"}` | **dev** |
| 删除 | BTN + 二次确认 | `DELETE /api/v1/models/{id}`（**激活中的模型会 409 `model_active`**） | **dev** |
| 单模型详情 | RO | `GET /api/v1/models/{id}` | dev |
| 舞台显示：`model.scale` / `offset_x` / `offset_y` / `rotation` | S / N | `PATCH /api/v1/models/{id}/display` 的 `model` 段 | **dev** |
| 舞台显示：`model.fit_mode` | SEG | 同上 | **dev** |
| 舞台显示：`stage.width` / `height` / `background_color` / `background_image` / `background_opacity` | N / T / S | 同上 `stage` 段 | **dev** |
| 舞台显示：`stage.background_type` | SEG | 同上 | **dev** |

来源：`models_routes/mod.rs:26-33`（端点表）、`dto.rs:11-24`（`ModelListItem`）、`dto.rs:97-141`（display patch）【已验证·本仓库】。

**不要做**：ZIP 上传（后端 D3.2 后置，v1 只接受受控本地 id）、缩略图（**后端没有缩略图端点**，
前端不要用 `Image.network` 去拉贴图当缩略图——Web 平台 `cacheWidth` 行为与原生不同
【调研 flutter §5.6】；用文字列表 + `texture_files.length` + `has_physics` 徽标代替）。

**激活/删除的反馈三件套**（最高风险操作必须有「进度 + 失败原因 + 重试」）：
ChatVRM 完全无加载/错误态、Amica 的进度回调是 TODO、Nexus 用 `<Suspense fallback={null}>` 导致舞台全空白，
是本调研里最一致的缺陷【调研 survey §8 缺陷 16、§10-P1-20】。

#### 4.1.3 LLM

| 字段 | 控件 | 路由 | 层 |
|---|---|---|---|
| `llm.base_url` | T | `GET`/`PATCH /api/v1/settings` 的 `llm` 段 | 普通 |
| `llm.model` | T | 同上 | 普通 |
| `llm.has_api_key` | RO 徽标（「已配置」/「未配置」） | 同上（**只回布尔**） | 普通 |
| `llm.api_key_env` | T（**环境变量名**，不是密钥）+「设为空以清除」 | 同上；清除用段级 `clear_api_key: true` | **dev** |
| 连通性自检 | BTN → 结果行（`ok` / `latency_ms` / `model_echo` / `error`） | `POST /api/v1/settings/test/llm` | 普通 |

**密钥红线的严格结论（实现者最容易写错的一处）**：
`LlmPatch.api_key_env` 的类型是 `Option<Option<String>>`，语义是
「不修改 / 清除 env 绑定 / **设置新的 env 变量名**」【已验证·本仓库 `patch.rs:120-126`】；
视图侧**永远不返回变量名**，只回 `has_api_key: bool`【已验证·本仓库 `view.rs:15-19`】。
因此前端**根本没有「输入 API key 明文」这条路**：密文本体不在 `AppSettings` 里，只在进程环境变量里。
UI 文案必须是「**密钥通过环境变量提供**：填入存放密钥的变量名（默认 `LIVE2D_AI_LLM_API_KEY`）」。
**绝不**回显、缓存到 localStorage、写进日志或 `debugPrint`（`AGENTS.md` §密钥安全）。

#### 4.1.4 语音合成（TTS）

| 字段 | 控件 | 路由 | 层 |
|---|---|---|---|
| `tts.base_url` | T | `settings` 的 `tts` 段 | 普通 |
| `tts.voice` | T（可换成 `DD`，但账上没有音色清单端点 → v1 用 T） | 同上 | 普通 |
| `tts.model` | T（可空 = 请求体不带 `model`） | 同上 | 普通 |
| `tts.has_api_key` | RO 徽标 | 同上 | 普通 |
| 连通性自检 | BTN → `ok` / `latency_ms` / `error` | `POST /api/v1/settings/test/tts` | 普通 |
| 服务端静音状态 | RO 徽标（**不是开关**） | WS `audio.muted` | 普通（也常驻于音频条） |
| `tts.sample_rate` / `channels` | RO（只显示当前值） | `GET /api/v1/settings` | 普通 |
| `tts.api_key_env` | T + 清除 | PATCH `tts.api_key_env` / `clear_api_key` | **dev** |
| 改 `tts.sample_rate` / `channels` | N | PATCH 支持，但**后果严重**（改错 = 全是噪声） | **dev** |
| `response_format` | — | **不可改**：`TtsPatch` 没有这个字段【已验证·本仓库】 | **不做** |

**给实现者的三个提醒**：
1. 「服务端静音」是**只读状态**，绝不能做成第二个开关（§7）。
2. `response_format`（`pcm` / `wav`）在 `AppSettings` 里存在但**不在 view 也不在 patch 里**，
   所以 UI 上**不存在**这个控件——不要"顺手加一个"。
3. 测试端点不会真正合成音频（它只做连通性），所以**不要**把它宣传成「试听」。

#### 4.1.5 动作与互动

分三组：

**A. 舞台与口型**（`DisplayPrefs`，纯本地，`localStorage`，**无后端**）

| 字段 | 控件 | 层 | 说明 |
|---|---|---|---|
| `scale`（0.5–2.0） | S，实时下发 | 普通 | 模型缩放；拖动中不断 `sync`（现有手感，保留） |
| `mouthSensitivity`（0.2–3.0） | S + 一行标定说明 | 普通 | 「1.00 为标定值；觉得嘴动得太小就调大」（现有 `display_panel.dart:79-85` 保留） |
| `lipSync` | SW | 普通 | 口型总开关 |
| `idleEnabled` | SW | 普通 | 呼吸/眨眼/微表情 |
| `allowDragZoom`（新字段） | SW | 普通 | 下发 `sync.clickEnabled`。**文案必须是「允许拖动与缩放」**，现状 `clickEnabled` 禁的是拖拽/滚轮/双击复位，**不是**点击互动【调研 control §9-7】 |
| `tier`（4096/8192/16384） | SEG | **dev** | 性能/画质权衡需要档位知识 |
| 头身角度上限 / 参数曲线 / 物理档 | — | **dev**（后续） | 需模型知识；wasm 侧目前硬编码 ±30/±10，暴露未标定滑杆会放大失真 |
| 单参数滑杆（`ParamAngleX/Y/Z`、`ParamMouthOpenY`…） | S + 显式「释放/复位」 | **dev**（后续） | 形态借 Stream Deck 的 dial stack：**选一个参数 → 一个滑杆 → 显式释放** |

**B. 动作试演**（走 core 仲裁，不绕过）

| 内容 | 控件 | 路由 | 层 |
|---|---|---|---|
| 6 个基础动作（`nod` / `shake_no` / `tilt` / `look_around` / `listen` / `surprise`） | 常驻工具条 3 个高频 + 面板卡片网格 6 个 | `GET /api/v1/commands`（清单） | 普通 |
| 强度 1/2/3 | **SEG**（离散三档用分段按钮，不是滑杆） | invoke body `{"strength":N}` | 普通 |
| 触发 | BTN | `POST /api/v1/commands/{id}/invoke` | 普通 |
| 动作来源历史（时间戳 / 动作 / 强度 / 来源 / epoch） | 列表 | WS `action_state` | 普通 |
| `capabilities.actions` 之外的动作 | — | 按钮由 `capabilities` 驱动显隐/置灰，**不要硬编码 6 个** | 普通 |

**屏幕槽位按 8 设计**（VRChat「up to 8 controls per menu」与 VTS `onScreenButtonID` 1–8 双源收敛），
先放 6 个，留 2 个给未来的 Mod 动作【调研 control §1.3-2】。

**C. 互动**

| 内容 | 控件 | 层 |
|---|---|---|
| 允许拖动与缩放 | SW（同 A 组，单点真相） | 普通 |
| 舞台缩放 +/−/复位三键 | 舞台角标 3 个小按钮，复用已实现的 `stage-zoom`（`dir ∈ in/out/reset`） | 普通 |
| 「点击角色有反应」 | **不做**：wasm 侧**没有命中测试**，拖拽在整个 canvas 上生效【调研 control §4.1】 | 不做 |
| 舞台背景图 | 走 `PATCH /api/v1/models/{id}/display` 的 `stage` 段；`stage-bg` 消息已实现（payload `dataUrl`） | dev |

#### 4.1.6 Mod 管理

| 内容 | 控件 | 路由 | 层 |
|---|---|---|---|
| Mod 列表（`id` / `name` / `version` / `api_version` / `status` / `enabled`） | 列表 + SW | `GET /api/v1/mods` → `{mods:[...]}` | 普通（列表）/ **dev**（启停） |
| 启用 / 禁用 / 重启 | BTN | `POST /api/v1/mods/{id}/enable` `/disable` `/restart` → `{ok, enabled}` / `{ok, restarted}` | **dev** |
| 配置 | T（JSON 文本域） | `POST /api/v1/mods/{id}/config`，body **必须**含 `{"config":{...}}`（`enable` 时可选带 `config`） | **dev** |

来源：`mods_routes.rs:129-216`（三个 action + `config`）、`:242-258`（列表形态）【已验证·本仓库】。
**注意**：`/api/v1/mods*` **不在 `match_route` 里**，它在 `start_server` 循环中**前置于** `dispatch`
单独处理（`web_api/mod.rs:523`）——所以它不受 settings 那套 method 分派约束，但 `POST` 仍是 mutating，
需要 loopback Origin + `application/json`【已验证·本仓库】。
配置编辑器用 `TextField` 多行 + 「保存」按钮，**提交前用 `jsonDecode` 本地校验**（错了就在原地提示，别发出去）。
理由：`{ok:false}` 的失败是 409 `mod_error`，且 restart 失败会返回「reload 成功但 restart 失败」这类半成功，
必须是可见的人话文案。

#### 4.1.7 诊断

**第 1 层（所有用户）「连接与延迟」**

| 内容 | 来源 | 说明 |
|---|---|---|
| WS 状态（5 态）+ 最近一次 `heartbeat` 的 `seq` / `ts` | `WsStatus` + WS 帧 | 常驻徽标另在 AppBar（§6.4） |
| **事件滞后**（不是「网络延迟」） | `ts` 与本地时钟之差 | WS 是**单向事件流**，服务端不读客户端帧 → **无法 ping/pong 测 RTT**。文案必须写「事件滞后」，避免假指标【调研 control §7.3-1】 |
| 渲染面 `progress` / `fps` / 桥 `phase` | `Live2DBridge` | 从舞台角标移到这里（FPS 徽标不占舞台主视线） |
| 协议版本 | `capabilities.ws_protocol_version` | |
| 后端 `version` / `schema_version` / `uptime_s` / `active_model_id` / `current_epoch` / `dev_mode` / `config_path` | `GET /api/v1/app/status`、`GET /api/v1/app/capabilities` | |
| 能力快照（`actions` / `action_sources` / `strength_levels` / `model_upload_supported`） | `capabilities` | |
| **一键复制诊断信息** | 拼接本地快照 → `Clipboard.setData` | 内容见下 |

**复制快照的内容**（成本低、收益高）：`version` + `schema_version` + `uptime_s` + `epoch` + WS 状态与最近
`seq`/`ts` + FPS + 桥 phase + 协议版本 + 最近 N 条 `action_state`（**含 source**）+ 日志级别统计。
依据：VTS 的 `requestID` 可追溯性先例（一次操作一个可复制追踪 ID）；OBS 的 `Copy Log URL` +
`Analyze Log File` 三件套——但 OBS 的 **Stats 窗口只有 `Reset Stats`、没有复制**，
所以本项目要把**统计与日志放进同一个可复制快照**，别重蹈覆辙【调研 control §7.2、§8-17】。

**第 2 层（dev_mode）「日志」**

| 内容 | 路由 |
|---|---|
| 日志尾部 ≤200 行（`log_dir` / `file` / `lines` / `truncated`） | `GET /api/v1/logs` |
| 级别列表（`trace`/`debug`/`info`/`warn`/`error`） | `GET /api/v1/logs/levels` |
| 级别过滤 | 客户端本地过滤（行级：选 warn 则保留 warn+error；无级别行始终保留——旧 JS 的 `filterLogLines` 语义是对的） |
| 复制筛选后 / 复制全部 | `Clipboard.setData` |

**`GET /api/v1/logs` 在 `dev_mode=false` 时被 `dispatch` 拦成 403**（`log_routes.rs:9-10`）【已验证·本仓库】。
所以 UI 必须把 403 渲染成「**打开开发模式后可用**」的说明行 + 一个跳到「开发模式」分区的按钮，
**不是**错误横幅。

**不要做**：把 `status.audio`（`backend: "none"` / `available: false` 是**硬编码占位常量**）
当真展示——会显示假信息【调研 control §7.1】。

#### 4.1.8 开发模式

| 字段 | 控件 | 路由 | 层 |
|---|---|---|---|
| `dev_mode` | SW（**这一层的总闸**） | `PATCH /api/v1/settings` 顶层 `{"dev_mode": true}` | 普通（这是它的入口） |
| 高级字段的说明 | RO 文本 | — | 普通 |
| `dev_mode` 生效范围说明 | RO | 「打开后：其他分区的『高级』分组出现；`/api/v1/logs` 放行」 | 普通 |
| 原始 `GET /api/v1/settings` JSON | RO 折叠 | 同端点 | dev |
| 动作仲裁观测（`dropped` 原因） | RO | 需要 Rust 侧新增 `action_state{kind:"dropped"}` 帧（**前端无法自行补齐**） | dev（待裁决 5） |
| 单参数滑杆与覆盖释放 | S + BTN | 需要 core 暴露（`ActionCommand::Release` 已有，**没有任何 HTTP 端点暴露它**） | dev（后置） |

**`dev_mode` 的三态语义**（实现者容易搞混）：PATCH 的 `dev_mode` 是 `Option<Option<bool>>`：
缺省 = 不修改 · `null` = 显式关闭 · `true|false` = 显式设置【已验证·本仓库 `settings_routes/mod.rs:78-86`】。
而 `dev_mode` 的**来源优先级是 CLI flag > settings 文件 > 默认 false**（`settings.rs:98-102`）——
所以 CLI 用 `--dev-mode` 拉起时，UI 上的开关可能是"改不动的"（PATCH 能写盘但被 CLI 覆盖）。
UI 处置：开关旁边显示一行「当前由启动参数强制开启」当 `status.dev_mode == true` 且 PATCH 后仍为 `true`
而请求体是 `false` 时（**检测到即提示，不谎报成功**）。

### 4.2 分区的宿主形态（三种断点、同一个内容）

| 断点 | 宿主 | 说明 |
|---|---|---|
| ≥1280 | 内联 `SettingsPane`（400 px，overlay 在舞台列右缘），可 Esc / 点空白关闭 | 滑杆与舞台同屏 |
| 900–1279 | `showModalBottomSheet(isScrollControlled, maxWidth 560, maxHeight 0.78h)` | 自带焦点陷阱 + Esc |
| <900 | 抽屉（列 8 项）→ `showModalBottomSheet(maxWidth 520, maxHeight 0.72h)` | 严格 2 层 |

**同一个 `SettingsScaffold` widget**，只换宿主。理由：现有代码已经证明这条路可行
（`main.dart:141-163` 宽屏/窄屏共用同一个 `DisplayPanel`），而 OLV-Web 的教训是「同一个数写 3 遍」
（440px/24px 出现在 3 个文件）【调研 survey §4.1】。

### 4.3 草稿与未保存改动（`SettingsController` 的双源）

```dart
class SettingsController extends ChangeNotifier {
  SettingsView? _remote;       // 上次 GET/PATCH 成功后的服务端真相
  SettingsPatchBuilder? _draft; // 用户正在编辑的**改动集**（不是完整副本）
  bool get dirty => _draft != null && !_draft!.isEmpty;

  /// 保存：把改动集编译成 `SettingsPatch`（三态由 `Tri` 保证，见 §0.3-5）。
  Future<void> save() async { … api.patchSettings(_draft!.toPatch()) … }
}
```

**为什么草稿是「改动集」而不是「完整副本」**：服务端的三态补丁语义下，
「把整份 view 回传」会把用户没碰的字段也变成显式赋值（并可能把 `has_api_key` 这类只读派生字段
误当成可写字段）。改动集模型 + `Tri` 的三态是**唯一**能安全表达「我只改了 `tts.voice`」的形状——
这正是已落地的 `lib/api/settings_models.dart` 的 `Tri<T>` 所解决的问题（其头注明确：
「一旦把『不修改』也序列化成 `null`，用户只改一个字段就会把同段其它字段**清空**」）。

**三处拦截**（离开分区 / 关闭弹层 / 刷新页面）。刷新页面用 `web.window.onBeforeUnload`（`main.dart`
才允许 `package:web`）。**「未保存改动必须有可见提示」**：分区头部显示一个 `warning` 色的
「未保存」Pill + 一个「保存 / 放弃」操作条。现状完全缺失（前端只有 `fetchStatus`，
`api_client.dart:90`）【调研 flutter §3.3】。

**PATCH 的三态构造**（这是「草稿 → patch」唯一容易写错的地方，必须纯函数化并单测）：

| 用户意图 | JSON |
|---|---|
| 这个字段没动 | **省略该键**（`Option::None`，`skip_serializing_if` 会跳过） |
| 这个字段要清空 | `null`（`Option::Some(None)`） |
| 这个字段要设成 X | `X`（`Option::Some(Some(X))`） |
| 整段清空 | `{"persona": null}` |
| 清除密钥绑定 | `{"llm": {"clear_api_key": true}}` |

### 4.4 保存反馈（`apply_status` 必须消费）

`PATCH /api/v1/settings` 的响应是 `{persisted, settings, apply_status}`，
`apply_status ∈ Applied | RestartRequired | Queued | NoSupervisor`【已验证·本仓库 `dto.rs:171-185`】。
文案必须按它分流，否则会出现「提示已热重载但实际 503」的错位：

| `apply_status` | toast |
|---|---|
| `Applied` | 「已保存并生效」 |
| `RestartRequired` | 「**已保存，需重启生效**」（`warning` 色） |
| `Queued` | 「已保存，正在生效」 |
| `NoSupervisor` | 「已保存，配置将在下次启动后生效」 |

---

## 5. 组件清单

### 5.1 内建即可（**不要自建**）

`Scaffold` / `AppBar` / `NavigationRail`（`extended`、`minWidth`）/ `NavigationDrawer` /
`SwitchListTile` / `Slider` + `SliderTheme` / `TextField` + `InputDecoration` /
`FilledButton`·`OutlinedButton`·`TextButton`·`IconButton`（含 `.tonalIcon`）/ `SegmentedButton` /
`DropdownMenu` / `SearchAnchor` / `ListTile` / `ExpansionTile` / `Divider`·`VerticalDivider`·`Card` /
`Tooltip` / `CircularProgressIndicator`·`LinearProgressIndicator` / `AlertDialog` +
`showModalBottomSheet`（**自带焦点陷阱 + Esc**）/ `SnackBar` + `ScaffoldMessenger` /
`ThemeData` + `ColorScheme.fromSeed` + `ThemeExtension`。

**边界原则**：凡是 Material 已表达「行为」的（按钮、输入、开关、导航）一律内建；
凡是表达「本产品语义」的（舞台、分区、字段行、状态徽标）一律自建。
自建组件**内部**尽量由内建组件拼装，不重造 `InkWell` / 焦点 / 语义【调研 flutter §7.2】。

**看似需自建但其实内建已够**：卡片、遮罩弹窗、抽屉、Tab、折叠面板、Tooltip、Toast、颜色选择器
（用预设色板 + `SegmentedButton`，不引 `flex_color_picker`）、旋钮（**本规格不需要旋钮**）。
全仓库现有代码里没有自建这些，重写时**也不要**。

### 5.2 必须自建（每个一句话职责）

| 组件 | 职责 | 关键 props | 层 |
|---|---|---|---|
| `lib/design/tokens.dart` | 全应用唯一的颜色/间距/圆角/时长常量 + `AppColors` extension + 各家族 `registry` | `const`s, `AppColors` | L1（唯一裸值豁免） |
| `lib/design/breakpoints.dart` | `SizeClass sizeClassOf(double width)` | — | L1 纯函数 |
| `lib/design/typography.dart` | 8 级字号 → `TextTheme` 的**唯一**构造点 | `TextTheme buildTextTheme()` | L1（裸值豁免） |
| `lib/settings/settings_sections.dart` | 8 个分区的声明式清单（label/description/icon） | `enum SettingsSection` | L1 |
| `lib/state/ui_phase.dart` | WS 信号 → `UiPhase` 的**纯派生函数** | `UiPhase deriveUiPhase(UiSignals)` | L1 |
| `AppShell` | 按 `SizeClass` 选导航形态 + `IndexedStack` 保活舞台 | `SizeClass` | L3 |
| `StageHost` | 包 `Live2DStage`：保活、`RepaintBoundary`、加载/错误覆盖层、语义标签、角标工具条 | `onRetry`, `children` | L3 |
| `ChatPanel` | 消息列表 + 输入区 + 音频条 + 设置入口 | `messages`, `phase`, `onSend`, `onStop` | L3 |
| `MessageBubble` | 单条消息：角色标签、流式光标、来源徽标、失败态 | `ChatMessage` | L3 |
| `StreamingIndicator` | 流式中的节流提示（**不用 `CircularProgressIndicator` 做装饰**） | `UiPhase` | L3 |
| `StatePill` | **唯一**的状态呈现：色 + 形 + 字 + 节奏 | `UiPhase` | L3 |
| `ConnectionBadge` | WS 连接状态（文字 + 语义，**不靠颜色**） | `WsStatus`, `lastSeq`, `lastTs` | L3 |
| `ErrorBanner` | 可关闭的行内错误提示（`dangerSurface` + `dangerBorder`） | `message`, `onDismiss` | L3 |
| `AudioBar` | 主音量滑杆 + 本机静音开关 + 服务端静音只读徽标 + autoplay 解锁提示 | `prefs`, `onChanged`, `serverMuted`, `audioUnlocked` | L3 |
| `SettingsScaffold` | 分区导航容器 + 草稿生命周期 + 未保存提示 + 保存/放弃操作条 | `section`, `controller` | L3 |
| `FieldRow` | 统一「图标 + 标签 + 控件 + 帮助文本 + **语义值**」行，支持滑杆/开关/文本/数字/下拉/分段 | `FieldRowKind`, `semanticFormatterCallback` | L3 |
| `SectionHeader` | 分区标题 + 分区说明（**唯一**写字号字重的地方） | `title`, `description` | L3 |
| `GlassPanel` | 半透明纯色 + 1 px 高光描边 + `hairline` 的面板容器（**无模糊**） | `child`, `padding` | L3 |
| `ActionToolbar` | 舞台上的 ≤8 槽常驻动作浮标（先 3 个高频） | `actions`, `busy`, `onInvoke` | L3 |
| `ActionGrid` | 面板内的动作卡片网格（6 动作 × 强度 SEG） | `actions`, `strength`, `onInvoke` | L3 |
| `ActionHistory` | 动作日志：时间戳 + 动作 + 强度 + **来源徽标** + epoch | `entries` | L3 |
| `ActionSourceBadge` | 来源徽标（你 / AI / 规则 / 未知）：色槽 + 图标 + 文字三通道 | `ActionSource` | L3 |
| `ModelPicker` | 模型列表 / 激活 / 导入 / 显示配置（6 个 models 子路由） | `models`, `onActivate`, … | L3 |
| `ModList` | Mod 开关 + JSON 配置表单 | `mods`, `onAction`, `onConfig` | L3 |
| `LogViewer` | 日志列表 + 级别过滤 + 复制（**必须虚拟化**，用 `ListView.builder` + `itemExtent`） | `lines`, `level` | L3 |
| `DiagnosticsPanel` | 连接/延迟/帧率/能力快照 + 一键复制诊断 | `snapshot` | L3 |
| `TestConnectionButton` | 触发 `settings/test/llm|tts` 并展示延迟/错误 | `section` | L3 |

> `ActionHistory` 的来源徽标是本规格的**差异化点**：VTS 在下发事件里直接给 `hotkeyTriggeredByAPI`，
> Warudo 用连线上滚动的球表示"控制流正在被触发"，Stream Deck 用键的两态图标——同类产品都把
> 「谁触发的」当一等公民信号【调研 control §2.2、§8-9】。本项目 core 早已下发 `source`，
> 前端**现在**才把它显示出来（§0.3）。

---

## 6. 状态与反馈

### 6.1 状态源（**只有这些，不要新造**）

| 信号 | 取值 | 来源 |
|---|---|---|
| `WsStatus` | `idle` / `connecting` / `connected` / `disconnected` / `closed` | `ws_client.dart` |
| `Live2DBridgePhase` | `loading` / `ready` / `error` / `destroyed`（+ `progress`、`fps`、`modelLoaded`） | `live2d_bridge.dart` |
| `runtime_status.event` | `voice_started` / `voice_ended` / `new_epoch` / `shutdown_ready` | WS |
| `turn_state.status` | `completed` / `failed` | WS |
| `text_delta` | `{text}` 或 `{completed}` | WS |
| `action_state.kind` | `perform` / `cease`（`data.action` 为对象，含 `source`/`strength`） | WS |
| `epoch` | 自 `chat` 受理响应与各帧 | HTTP / WS |
| `error` 帧 | `{code, message}` | WS |

> **裁决（D）**：调研 control §5.1 把 core 的 `Phase`（`Idle`/`Thinking`/`Speaking`）也列为
> 「已在 WS 上的状态源」。**这不准确**：`web_api/ws/*.rs` 全文**没有** `phase` 投影
> 【已验证·本仓库 `grep -rn phase web_api/ws/` 零命中】。所以 UI 状态必须
> **从 `runtime_status` + `text_delta` + `turn_state` + `epoch` 派生**，不得等待一个不存在的 `phase` 帧。

### 6.2 派生函数（纯逻辑，L1，可 VM 单测）

```dart
// lib/state/ui_phase.dart —— 无 web 依赖、无 BuildContext
enum UiPhase { offline, idle, thinking, speaking, interrupted, error }

class UiSignals {
  final WsStatus wsStatus;        // 由 api/ 层提供
  final bool bridgeError;         // Live2DBridgePhase == error
  final bool turnActive;          // POST 已受理且本轮未收口
  final bool voiceActive;         // 收到 voice_started、未收到 voice_ended
  final bool interrupted;         // 收到 new_epoch 后 1.2s 窗口内
  final bool errorActive;         // error 帧 / turn_state failed，未被用户关闭
}

UiPhase deriveUiPhase(UiSignals s);
```

**判定顺序（自上而下第一个命中即返回）**——顺序本身是契约，必须有表驱动测试：

```
errorActive            → error
wsStatus != connected  → offline
bridgeError            → offline
interrupted            → interrupted
voiceActive            → speaking
turnActive             → thinking
otherwise              → idle
```

### 6.3 状态 → 视觉映射（**色 + 形 + 字 + 节奏**，单点表）

| `UiPhase` | 色槽 | 形（图标/形状） | 字 | 节奏 | 舞台表现 |
|---|---|---|---|---|---|
| `offline` | `danger` | `Icons.cloud_off` + **实心点** | 「后端未连接 · 点此重试」 | 静态，**不闪**（避免"坏了"的错觉） | 顶部窄横幅滑入（`Durations.base`）+ 角色**降饱和**（`opacity` 不单独承担语义——Nexus 的「离线仅 `opacity:.72`」是反例【调研 survey §7.8.7-8】） |
| `idle` | `success`（弱化：`contentFaint`） | `Icons.circle_outlined` | 「空闲」 | 静态 | 只有待机小动作 |
| `thinking` | `warning` | `Icons.psychology_outlined` + **脉冲环** | 「思考中」 | **1.4 s** 呼吸（`Durations` 之外的单点节拍，见下） | 舞台**边缘呼吸光**（低振幅、无声） |
| `speaking` | `primary` | `Icons.graphic_eq` + **实心点** | 「说话中」 | 脉冲与 `volume` 同源（**不要独立节拍器**） | 口型由 PCM 驱动（已有） |
| `interrupted` | `contentMuted` | `Icons.stop_circle_outlined` | 「已打断」 | 一次性 200 ms 淡出后回落 | **角色一次性回正**（头/身回中），**不做**"震惊"表情——那是 AI 语义，不是系统语义 |
| `error` | `danger` | `Icons.error_outline` | 「出错 · 详情」 | 静态，可关闭 | 舞台底部错误卡 + 「重试」 |

**三条硬规则**：
1. **每个状态至少占「色/形/字」三个通道里的两个**，且**字号/形状/文案必须不同**——
   现状 OLV-Web 的 6 个 AI 状态共用 1 种视觉（恒 `#7C5CFF` 胶囊、只有文字变）
   且"思考/说话"在状态机层面本就是同一个值【调研 survey §8 缺陷 3】。
2. **连接态常驻可见**：`ConnectionBadge` 永远在 AppBar，**不得**像 N.E.K.O 那样被
   `display:none !important` 主动隐藏【调研 survey §3.4】。
3. **思考态不出提示音**。Google 官方判据：earcon「impose **cognitive load**」「**not intuitive**」，
   且「If you feel like you have to **teach users** what an earcon means, **don't use an earcon**」
   【调研 control §5.3-2】。延迟反馈优先视觉与文案。

**关于「节奏」与 `prefers-reduced-motion`**：`thinking` 的呼吸光是**唯一**的持续动画。
它必须：
- 用 `AnimationController(behavior: AnimationBehavior.normal)`（默认的 `preserve` 对重复动画**刻意免疫**
  `disableAnimations`，见 §9.4）；
- 在 `MediaQuery.disableAnimationsOf(context) == true` 时**不启动**，改为静态描边。

### 6.4 常驻状态元件

| 元件 | 位置 | 内容 |
|---|---|---|
| `ConnectionBadge` | AppBar 右侧（所有断点） | 图标 + **文字** + 语义标签。5 态文案：`idle`「未连接」/`connecting`「连接中」/`connected`「已连接」/`disconnected`「重连中」/`closed`「已关闭」。**不靠颜色单独表意**（色盲用户不可辨【调研 flutter §6.2】） |
| `StatePill` | 聊天面板头 + 舞台角标（二选一，按断点） | §6.3 的映射 |
| `AudioBar` | 聊天面板底部、输入框上方 | §7.4 |
| 服务端静音徽标 | `AudioBar` 内（另在「语音合成」分区复现一次） | §7.3 |

### 6.5 WS 断线的可见提示

1. **退避重连是既有行为，必须保留**：1s→2s→4s→8s→16s→30s 上限；
   `shutdown_ready` **不再永久停连**（服务端会被反复重启）【已验证·本仓库 `ws_client.dart:227-235,277-286`】。
2. `WsStatus.disconnected`/`closed` → `offline` → 舞台顶部横幅 + `ConnectionBadge` 变 `danger`。
3. **用户动作即保活**：发送消息前调用 `ws.ensureConnected()`（既有行为，保留）；
   动作/设置保存成功后若状态仍非 `connected`，提示「已提交，但实时通道尚未恢复」。
4. 横幅必须是**可点的重试**（`ensureConnected`），不是纯文本。

### 6.6 LLM / TTS 失败的可见提示

| 失败 | 呈现 |
|---|---|
| `POST /api/v1/chat` 400 `invalid_payload` | `ErrorBanner`：「消息格式不被接受」（这类是前端 bug，文案要写清） |
| 429 `busy` | `ErrorBanner`：「上一轮还没结束」+「打断并重发」按钮 |
| 503 `no_supervisor` | `ErrorBanner`：「后端对话引擎未就绪」+「去 LLM 设置」按钮（跳分区） |
| `network_error` | `ErrorBanner`：「无法连接后端」+「重试」 |
| WS `error` 帧 | `ErrorBanner`：`code` + `message`，可关闭；同时把 `code` 记进诊断快照 |
| `turn_state.status == failed` | 助手气泡转失败态（`dangerSurface` + 「（生成失败）」）+ 重试按钮 |
| 本轮无文字输出 | 助手气泡写「（本轮没有文字输出——通常是模型只调用了动作工具）」（**现有行为，保留**，`chat_controller.dart:196-202`） |
| `settings/test/llm` / `test/tts` 失败 | 结果行内联显示 `error.code` + `message` + 建议（base_url 是否可达 / 模型名是否存在），**不用 toast**（避免"一闪而过"） |
| `PATCH` 400 `url_invalid` / `invalid_env_name` | 对应字段下方内联错误（`FieldRow` 的错误槽），**不用 toast** |
| `PATCH` 返回 `RestartRequired` | `warning` 色 toast（§4.4） |
| 渲染面 `error` 帧 / 12 s 未 `ready` | 舞台底部错误卡 + 「重试」（现有 `_StageErrorOverlay`，保留） |

**禁止**：把失败渲染成「只有降低不透明度」或「红色小字」（Nexus / OLV-Web 的坑）
【调研 survey §8 缺陷 17】。

### 6.7 「聆听」态：当前不存在（本规格的诚实结论）

任务书的 §6 要求映射「聆听」，但**本项目当前没有输入侧**：

- core `Phase` 只有 `Idle` / `Thinking` / `Speaking`（`state.rs:15-24`）【已验证·本仓库】，
  **没有** `Listening`；
- 全仓库没有 ASR/麦克风端点，`capabilities` 里也没有 `asr`【已验证·本仓库】；
- 链路是「**用户文本** → LLM 流式 → …」（`AGENTS.md` §项目背景）。

**处置**：`UiPhase` 枚举里**不设** `listening`；`UiState` 映射表里**不设**该行。
`listen` 作为**动作**（`ActionId::Listen`）仍可手动/LLM 触发，它由 `action_state` 呈现（§8）。
若将来接入 ASR，最小落点是给 core 增一个 `Listening` 相 or 让外部输入 Mod 发一个
`runtime_status{event:"listening_started|listening_ended"}`，届时在 §6.2 的派生链里插入一行即可。
（**列入待裁决 4。**）

---

## 7. 音频与音量

### 7.1 三个轴（**必须三套词，不能两套**）

| 轴 | 谁控制 | 效果 | 数据来源 | UI 形态 |
|---|---|---|---|---|
| **主音量** `volume` | 用户 | 本机播放响度（`GainNode` 增益，非线性压缩 `v²`） | `DisplayPrefs.volume` | **滑杆**（常驻音频条 + 「动作与互动」分区两处同源） |
| **本机静音** `muted` | 用户 | 本机听不到；**口型照常**；**不断开音频图** | `DisplayPrefs.muted`（默认 `false`） | **图标开关**（常驻音频条） |
| **服务端静音** | 用户**不可控**（启动参数 `LIVE2D_AI_MUTE_AUDIO=1`） | 源头下发全零 PCM，但 `volume` 仍是**真实 RMS** ⇒ 不出声、口型照动；任何客户端都绕不过 | WS `audio.muted`（只读观测） | **只读徽标** |

### 7.2 命名契约（P5 的落地，逐字照抄）

| 控件 | 标签（逐字） | 说明文案（逐字） |
|---|---|---|
| 本机开关 | **「本机静音」** | 「**你这台设备听不到**（口型保留）」 |
| 服务端徽标 | **「服务端静音中（`LIVE2D_AI_MUTE_AUDIO=1`）」** | 「**服务端没有发出声音**（任何客户端都听不到，需在启动参数里关闭）」 |

**规则：同一个词不能同时命名「我能听到」与「有没有声音被生产出来」这两个轴。**
这正是 OBS 把 `Monitor and Output` 改名 `Monitoring Enabled` 的教训——大厂也在反复改这类歧义标签
【调研 control §6.3-3、§8-14】。

**为什么必须这样（用户视角的失败场景）**：用户按了「本机静音」却发现根本没声音过（因为服务端在静音），
会以为 UI 坏了。两个同名开关指向两个不同真相 → 必须两个名字【调研 control §8-3】。

### 7.3 六项目坑位对照（确认本章覆盖）

| 项目 | 它栽的坑 | 本规格的对策 |
|---|---|---|
| OLV-Web | **完全没有音量/静音**（裸 `HTMLAudioElement`，无 `GainNode`） | 主音量 + 本机静音**一级常驻**（音频条） |
| Meuxe | 没有音量/静音 UI | 同上 |
| my-neuro | 音量/静音**只存在于配置字段**，无 UI | 同上 |
| Live2DPet | 只有一个音量滑杆 + `Audio Mode` 三选一（含 Silent）——**把两个轴揉进一个模式枚举** | 静音是**独立布尔**，不是模式枚举；音量与静音正交 |
| Nexus | 只有 TTS 音量滑杆、**无 mute**；文案 `'This only adjusts TTS output loudness — system volume is unchanged.'` | 采纳该文案思路：主音量副标题写「只改变本机播放响度，不影响服务端是否发声」 |
| super-agent-party | 「静音」是 `audio.volume = 0.0000001` 的 hack，且主 UI 无音量滑杆 | **禁止 epsilon 静音**：静音必须 `gain == 0`（`playbackGain` 已保证）；且音频图保持活着 |
| Ghost Vessel（正例） | `toggleVoice()` 直接 `killAudio()` 并**持久化** | 静音是**可持久化的显式状态**（`DisplayPrefs.muted` 落 `localStorage` 已有） |

来源：【调研 survey §8 缺陷 6（6 个项目都有缺口）、§7.9.1、§7.7.5】。
另：**默认出声**——出厂即有声，「不出声」必须显式要求（用户 2026-09-10 裁决；
`display_prefs.dart:37-39` 已实现）。**UI 上不得默认勾选任何静音。**

### 7.4 音频条（`AudioBar`）的具体形态

```
┌────────────────────────────────────────────────────────────┐
│ 🔊 ──────●─────── 80%   [🔇 本机静音]  ⚠ 服务端静音中 ⓘ     │
│                            ↑ 图标开关        ↑ 只读徽标     │
│ （本机静音时滑杆不置灰、不归零——保留用户原值）              │
└────────────────────────────────────────────────────────────┘
   静音时滑杆下方追加一行：「当前已静音——取消静音后按此音量播放」
   autoplay 未解锁时追加一行：「点击任意位置或按任意键以启用声音」+ 一个按钮
```

- 位置：聊天面板底部、输入框**上方**（常驻，三级断点都在）。
- 徽标只在 `lastAudioFrame.muted == true` 时出现；下一次 `muted == false` 或 `new_epoch` 时清除
  （它是**观测值**，不是配置）。
- 滑杆要 `semanticFormatterCallback`（读屏要能念出百分比，见 §9.1）。

### 7.5 `AudioPlayer` 的三条不变量（**必须原样保留**）

| # | 不变量 | 为什么 | 保留方式 |
|---|---|---|---|
| 1 | **时间轴只前进** | WebAudio 对同时发声的音源**逐样本相加** → 重叠即削波/梳状滤波。实测：一轮 10.68 s 音频被压进 13.99 s 时间轴、最多 **17 个音源同时响**、峰值 1.109 = 听感「很吵的杂音」 | `scheduleSlice` 只做「`nextStart < now ? now : nextStart`」，**绝不用 `audio.start` 重置时间轴**（`schedule.dart:48-55` 的长注释保留） |
| 2 | **口型按播放时刻释放** | 分片 1 ms 级突发到达；按到达时刻推电平会让嘴唇比声音早最多 3.36 s | `feed()` 只**计算**包络并 `LevelTimeline.push(when, level)`，由 20 ms ticker 的 `drain(currentTime)` 释放 |
| 3 | **降级不破链** | 无 WebAudio 或单帧调度失败只影响该帧，口型照常驱动 | `_ensureContext()` 返回 null 时 `_emitLevel(level)` 直通；`_schedule` 抛错时同样直通 |

**与静音的关系（最容易写错的一处）**：静音走 `GainNode` 增益置 0，**不断开音频图**
（`audio_player.dart:149-154` 的长注释）。理由：音频图必须保持活着，`AudioContext.currentTime`
与排片时刻才与真实播放一致，`LevelTimeline` 的到期释放才不会漂移。
**不要**为了省事在静音时断开 destination 或停掉口型——那会同时破坏同步与手感
【调研 flutter §5.9】。

**回归钉子**（已有，全部保留）：`audio_schedule_test.dart`（时间轴单调、排空锚定、
「旧的『分片起始重置』规则必然重叠」）、`audio_gain_test.dart`（`v²` 感知压缩、边界与 NaN 兜底、
静音恒 0）、`display_prefs_test.dart`（静音与音量正交、四象限可表达、旧存档按新默认读）。

### 7.6 一个必须补的真实缺口：键盘解锁音频

**事实**：语音约定是「默认出声」，而 Web 的 autoplay 限制要求**先有一次用户手势**才能 `resume()`
`AudioContext`。现有实现的解锁入口只挂在 `Listener.onPointerDown` 上（`main.dart:167-170`）。
**后果**：纯键盘用户（Tab + Enter 发送）**永远不会解锁音频**——消息发得出去、模型嘴在动、
**但没有声音**，且界面不给任何解释。这与「默认出声，否则用户会以为 TTS 坏了」的裁决意图**直接冲突**
【调研 flutter §9.1 D11】【已验证·本仓库】。

**要求**（P3 阶段实现，§11 钉子 9）：
1. 保留 `onPointerDown`；
2. **键盘路径也解锁**：在根 `Shortcuts`/`Focus` 的任意键命中时调用 `audio.unlock()`；
3. `AudioContext.state == 'suspended'`（或从未 unlock）时，`AudioBar` 显示
   「点击任意位置或按任意键以启用声音」+ 一个显式的「启用声音」按钮，把失败**显式化**；
4. 进 `docs/verification/` 人工自测清单：**仅用键盘发一条消息，确认有声音**（`flutter test` 覆盖不到）。

### 7.7 不做（v1）

- **波形/频谱可视化**：`package:web` 已含 `AnalyserNode`，自建 `CustomPainter` 可做到 0 新依赖
  【调研 oss §3】，但它属于"锦上添花"，且必须在「说话中」状态才重绘、`RepaintBoundary` 包裹、
  不得用 `BackdropFilter` 做发光底。**列入"明确不做"（§12），保留为将来的 0 依赖升级路径。**
- 麦克风电平（涉及 `getUserMedia` 与输入侧能力，与 §6.7 的「聆听」缺失同源）。

---

## 8. 动作与互动

### 8.1 触发形态（**不做径向菜单**）

**结论**：**常驻工具条（3 个高频）+ 面板卡片网格（6 动作 × 强度 1/2/3）**。

- 6 个动作 + 3 档强度**不需要径向菜单**。径向的价值在「小集合 + 位置肌肉记忆 + 手柄/VR 指针」；
  本项目是 Flutter Web + 鼠标/触屏，径向要自绘命中区、与 iframe 舞台叠层协调、触屏收益进一步下降。
- **径向不是行业共识**：Warudo 官方文档完全没有径向/屏幕操作菜单（触发一律是节点图），
  Live2DViewerEX 用「浮球 + 两级面板」，VRChat 用径向的官方理由是「**one handed** and in a
  **less obvious** manner」——是"单手 + 旁人不易察觉"，**不是"效率更高"**
  【调研 control §1.3-6、§8-13】。
- **槽位按 8 设计**（VRChat 8 控件/页 + VTS `onScreenButtonID` 1–8 双源收敛），先放 6、留 2 给 Mod。
- **离散给按钮、连续给滑杆**：强度 1/2/3 是**离散三档** → `SegmentedButton`（本项目旧 JS 已是 `.seg-btn`）；
  音量/缩放/口型灵敏度是连续量 → 滑杆。这条分工在硬件侧已被验证（Stream Deck + 的
  `Rotate: Adjust Volume` 连续 vs `Push: Play/Pause` 离散）【调研 control §1.3-7】。
- **每个触发项必须自带"停止/释放"或明确的"正在演"**：VTS 官方原话——「任何能被自动化打开的状态，
  必须配一个用户可触达的关闭入口」【调研 control §1.1】。

### 8.2 手动通道：只走 HTTP 命令端点（C6）

| 动作 | 端点 | 语义 |
|---|---|---|
| 列动作 | `GET /api/v1/commands` → `{commands:[{id,name,description,params_schema,allowed_sources,kind,target_action,step_count}]}` | `kind` 恒 `"single"`；`params_schema` 目前只有 `strength` 1..3 默认 2；`allowed_sources` 恒 `["user_command"]` |
| 触发 | `POST /api/v1/commands/{id}/invoke`，body `{"strength":1\|2\|3}` → `{accepted, epoch, command_id, kind, steps, action}` | 内部经 `SupervisorHandle::make_user_action` → `ActionSource::UserCommand`（优先级 100） |

来源：`command_registry.rs:71-96,126-178`、`commands_routes.rs:62-95,154-235`【已验证·本仓库】。

**禁止**：
- ❌ 前端直发 `action-state` 给 iframe 来"绕过 core 仲裁"（旧 JS 的 `postToStage({type:"action-state"})`
  就是这条路，迁移时**不要再抄**）。UI → 渲染面的 `action-state` **只能**由 core 的
  `action_state{perform}` WS 帧转发而来（§8.3）。
- ❌ 前端自己发明"谁优先"的规则。core 已有 `user 100 > llm 90 > rule 50`，且「完全相同幂等拒绝、
  低优先级不可抢占、同优先级后来者胜」【已验证·本仓库 `action/mod.rs:110-130,264-317`】。
  **UI 只需要把它显示出来。**

### 8.3 来源表达（本次接线缺陷的界面收口）

**已完成的解析**（§0.3）：`ActionDetail.fromData` 给出 `action` / `strength` / `source`。

**界面如何表达来源**：

```dart
// lib/ui/action_source_badge.dart（L3，允许 import material）
// 映射集中在**一处**（吸收 6 个项目状态样式各造一套的教训）
({String label, IconData icon, Color tone}) actionSourceView(ActionSource s, ColorScheme cs) =>
  switch (s) {
    ActionSource.userCommand   => (label: '你',   icon: Icons.touch_app_outlined,     tone: cs.primaryContainer),
    ActionSource.llmTool       => (label: 'AI',   icon: Icons.auto_awesome_outlined,  tone: cs.tertiaryContainer),
    ActionSource.ruleFallback  => (label: '规则', icon: Icons.rule_outlined,          tone: cs.surfaceContainerHighest),
    ActionSource.unknown       => (label: '未知', icon: Icons.help_outline,           tone: cs.surfaceContainerHighest),
  };
```

**三通道**（满足 P3 的「至少两个」）：

| 来源 | 色 | 形 | 字 |
|---|---|---|---|
| 用户 | `primaryContainer` | `touch_app` 描边图标 | 「你」 |
| AI（LLM 工具） | `tertiaryContainer` | `auto_awesome` 描边图标 | 「AI」 |
| 规则兜底 | `surfaceContainerHighest` | `rule` 描边图标 | 「规则」 |
| 未知 | `surfaceContainerHighest` | `help_outline` | 「未知」 |

> **裁决（G）**：这是全规格中**唯一**允许使用 `ColorScheme.tertiary` 的地方。理由：
> ① 来源区分需要「不同色相 + 小图标」（调研 control §2.3-2 的明确建议）；
> ② `tertiary` 是 M3 `fromSeed` **派生**出来的槽位，不是新造色 → 不违反 P2 的「一个品牌色」；
> ③ `unknown` **绝不能**归到 AI 一侧（`action_source.dart:134-140` 已有测试钉死：协议一变
> 就会把用户的动作标成 AI 做的）。

**三处呈现**：

1. **`ActionHistory` 每一行**：`[时间戳] [动作中文名] [强度 Ⅰ/Ⅱ/Ⅲ] [来源徽标] [epoch]`。
   动作名用**中文**而不是内部 id（VTS 的教训：日志里出现的是人能读懂的语义名，
   「You can give hotkeys names that are also shown in the logs」）【调研 control §7.2】。
2. **动作卡片上的即时徽标**：`perform{source}` 收到时，在对应动作卡上打一个来源徽标 +
   「正在演」进度环（进度用 wasm 已知的 `choreography_total_ms` 驱动，或后端不提供时用
   一个不带百分比的 indeterminate 环）。
3. **舞台一次性提示**：`perform` 到达时在舞台右下角滑入一条 `GlassPanel` 小卡
   （「AI 触发了 点头 Ⅱ」），`Durations.base` 入场、3 s 后淡出。**只在来源为 AI/规则时出现**——
   用户自己点的动作不需要再告诉他一遍。

**「AI 想动但被压住」目前不可观测**（诚实记录）：`RootEffect::Action(ActionEffect::Dropped{reason})`
只走 `tracing::debug!`，**不上 WS**；而且 `RootFact::Dropped` 是**单元变体、不带 reason**
【已验证·本仓库 `supervisor/handlers.rs:244-246`、`app_event.rs:55`】。
所以「AI 想动但被用户抢占了」在界面上**做不到**——这需要 Rust 侧新增一条
`action_state{kind:"dropped", reason, incoming, active}` 帧（**列待裁决 5**）。

**前端能做的本地缓解**（P5 阶段，独立于后端）：
- 记住最近一次 invoke 的 `(action, strength, 时间)`；若在 2 s 内重复点击**同一动作**，
  在按钮下方提示「同一动作正在播放，重复点击不会重播」——
  因为 core 对「动作+强度+来源完全相同」返回 `Dropped(AlreadyActive)`（幂等），
  而用户看到的是"点了没反应且没有任何解释"【调研 control §8-7】。
- 按钮在收到自己的 `perform` 且 `source == userCommand` 时进入「正在演」态（禁用 + 进度环），
  在 `cease` 或动作时长到期后复位。

### 8.4 参数暴露准则（**四问全「是」才进普通区**）

来自 NN/g 渐进披露的判据【调研 control §3.3】：

1. **拖动后 1 秒内能用肉眼看出差异吗？**
2. **不需要知道参数名/量程/物理吗？**
3. **有确定默认值且能一键复位吗？**
4. **一分钟内会调第二次吗？**

| 参数 | 1 可见 | 2 免知识 | 3 可复位 | 4 高频 | 位置 |
|---|---|---|---|---|---|
| 主音量 | ✅ | ✅ | ✅ | ✅ | **常驻音频条** |
| 本机静音 | ✅ | ✅ | ✅ | ✅ | **常驻音频条** |
| 模型缩放 | ✅ | ✅ | ✅ | ✅ | 普通（动作与互动 → 舞台与口型） |
| 口型灵敏度 | ✅ | ✅ | ✅ | 🔶 | 普通（带一行标定说明） |
| 口型同步 / 待机小动作 / 允许拖动缩放 | ✅ | ✅ | ✅ | 🔶 | 普通（开关） |
| 动作试演 + 强度 | ✅ | ✅ | ✅ | ✅ | 普通 |
| 显示档位 `tier` | ❌ | ❌ | ✅ | ❌ | **dev** |
| 头身角度上限 / 参数曲线 / 物理档 | ❌ | ❌ | ❌ | ❌ | **dev** |
| 单参数滑杆（`ParamAngleX`…） | ❌ | ❌ | 需显式释放 | ❌ | **dev** |
| `persona.system_prompt` / `max_history_pairs` | ❌ | ❌ | ✅ | ❌ | **dev** |
| `model.scale/offset/rotation`（后端显示配置） | ✅ | ❌ | ✅ | ❌ | **dev** |
| `tts.sample_rate` / `channels` | ❌ | ❌ | ✅ | ❌ | **dev** |
| `dev_mode` | — | — | — | — | 普通（它是高级区的入口） |

**两层，不造第三层**。NN/g 官方硬约束：「**designs that go beyond 2 disclosure levels
typically have low usability**」，且「必须 obvious how users progress」
【调研 control §3.2、§9-6】。所以：
- **不设**「快捷入口」或「简易模式」这第三档（Live2DViewerEX 的「快捷菜单 = 精简版控制面板」
  **恰好是两层**，不要读成"可以再加一个入口"）；
- 高频项直接留在普通层首屏（这是「initial display 即重要」的正确读法）。

**参数面板要给单位与量程**，不要只给 0–1 归一化：`param_scale.rs` 目前硬编码 ±30°，
面板若显示「±30°」会与「±45° 的模型」矛盾——所以 v1 **不暴露**角度上限，等标定数据齐了再说。

### 8.5 舞台交互开关的文案（不许混）

| 层 | 开关 | 文案 | 实现 |
|---|---|---|---|
| 画布层 | `sync.clickEnabled` | **「允许拖动与缩放」**（不是「点击互动」！它禁的是拖拽/滚轮/双击复位） | `DisplayPrefs.allowDragZoom` → `sync.clickEnabled` |
| 画布层 | 舞台角标 +/−/复位 | 「放大 / 缩小 / 复位」+ tooltip「也可双击复位、滚轮缩放」 | 复用已实现的 `stage-zoom`（`dir ∈ in/out/reset`）——**目前 Flutter 桥没有暴露它**，需补 |
| 窗口层 | 置顶 / 点击穿透 | 「窗口置顶」/「鼠标穿透」 | **属 Rust 桌面壳职责**，前端不做（列 §12） |

依据：`clickEnabled=false` **同时**禁用拖拽、滚轮、双击复位四项【已验证·本仓库
`l2d-wasm-demo/src/main.rs:602-603,636-637,691-692,716-717`】；
穿透是**整窗布尔**且 **Web 永远不支持**（winit `set_cursor_hittest` 平台差异原文、
Electron `setIgnoreMouseEvents`），所以它不属于前端能力【调研 control §4.2】。

### 8.6 必须新增的桥能力（实现者照此写）

`live2d_bridge.dart` 目前只实现 4 类下行（`sync`/`mouth`/`stage`/`destroy`），
而渲染面**已经实现**了 `action-state` / `stage-zoom` / `stage-bg` / `clickEnabled`
【已验证·本仓库 `main.rs:334,346-360,371-406`】。需要补：

```dart
// 1) 动作表演（由 WS action_state 转发而来，禁止 UI 直接调用）
Future<void> sendActionState({
  required String action,       // 'nod' | 'shake_no' | 'tilt' | 'look_around' | 'listen' | 'surprise'
  required String state,        // 'start' | 'end'
  int? strength,                // 1 | 2 | 3（缺失时渲染面按 2 处理）
});

// 2) 缩放控制（舞台角标三键）
Future<void> sendStageZoom(String dir);   // 'in' | 'out' | 'reset'

// 3) 自定义背景（dev 区）
Future<void> sendStageBg(String? dataUrl); // null / 空串 = 清除
```

**⚠️ `action-state` 的载荷形状有一个反直觉的怪癖，必须照做**：
渲染面读动作名是 `v.get("action")`——**在信封根上**，而 `state` / `strength` 在 `payload` 里
【已验证·本仓库 `main.rs:379-407`】。所以帧必须长这样：

```json
{"version":1,"type":"action-state","action":"nod","payload":{"state":"start","strength":3}}
```

至于 `sync.clickEnabled`：它**已经在** `stage-config` 分支里被读取（`main.rs:334-336`），
所以直接加进 `sendSync` 的 payload 即可，不需要新方法。

**`stage-ack` 要读**：渲染面在处理成功后回
`{"type":"stage-ack","applied":true,"msg_type":...,"scale":...,"offset_x":...,"offset_y":...}`，
并附注释「未收 ACK 前父页不得提示'已生效'」【已验证·本仓库 `main.rs:503-517`】。
现在 `Live2DBridge._onRaw` 的 `default:` 分支把它**忽略**了。要求：
- 解析 `stage-ack` 为 `StageAckEvent`（`applied` / `msgType` / `scale` / `offsetX` / `offsetY`）；
- **容忍无 `version`**（历史遗留帧）——现有测试已覆盖这条容错，保留；
- 舞台角标的缩放按钮在收到对应 `stage-ack` 之前显示 pending 态（不谎报已生效）。

---

## 9. 无障碍与键盘

> 现状：全仓库 `Semantics` / `FocusTraversalGroup` / `Shortcuts` / `Actions(` / `prefers-reduced-motion`
> **零命中**。无障碍目前完全不存在，属于"从零补"而不是"改进"【调研 flutter §6.1】。

### 9.1 `Semantics`（4 处必须手写 + 1 处免费补齐）

| 位置 | 做法 | 理由 |
|---|---|---|
| **舞台 iframe** | `Semantics(label: 'Live2D 舞台，正在显示 <模型名>', container: true)` 包一层 | 平台视图对读屏是黑盒；现在用户完全不知道舞台存在 |
| **流式回复** | `Semantics(liveRegion: true)` + **节流播报**（≥1.5 s，或按"整句到达"播报） | 逐 delta 播报会淹没读屏（一个 `text_delta` 可能几毫秒一个） |
| **状态徽标 / 连接徽标** | 补语义标签，且**文字本身要表意**（不能只靠颜色） | 现状 `_WsBadge` 的五色编码只靠颜色传达状态，色盲用户不可辨 |
| **`Slider` 的当前值** | `semanticFormatterCallback`（主音量 →「主音量 80%」、口型灵敏度 →「口型灵敏度 1.20 倍」、缩放 →「缩放 1.00」） | 自建「字段行」最易漏的一处：视觉上靠右侧 `Text` 显示数值，但读屏用户拖滑杆时听不到变化。`display_panel.dart` 的 `_SliderRow` 正是这个雏形 |
| 装饰性元素（小圆点等） | `ExcludeSemantics` | 避免"一坨无意义节点" |

Material 内建组件自带语义：`IconButton` 带 `tooltip` 会变成语义标签、`SwitchListTile` / `ListTile` /
`TextField` 都自带【调研 flutter §6.2】。

### 9.2 焦点管理

1. **每个面板/弹层包一个 `FocusTraversalGroup`** + `OrderedTraversalPolicy` 固定顺序。
   否则顺序跟随 Widget 树，**重构就会静默改变 Tab 顺序**。
2. **焦点陷阱**：优先用 `showModalBottomSheet` / `showDialog`（自带陷阱）——这是优先用它们而不是
   自绘 overlay 的又一个理由。
3. **焦点可见性**：不用阴影，用 **1 px 不透明 `focusRing` 边框**（保证对比度可测）。
   Flutter 已内建「键盘导航时显示焦点高亮、鼠标点击时不显示」的行为（`FocusManager.highlightMode`），
   **不要覆盖它**。
4. **焦点返回**：弹层关闭后焦点回到**触发控件**（Material 的 `Navigator` 会处理，但要写测试验证）。
5. **发送后焦点留在输入框**（现在隐式如此，重写时容易丢）。

### 9.3 键盘

用 `CallbackShortcuts` 挂在根（最省样板）：

| 键 | 动作 | 备注 |
|---|---|---|
| `Ctrl/Cmd + Enter` | 发送消息 | `TextField` 即使有 `TextInputAction.send`，在 `maxLines > 1` 时 Enter 是换行，所以必须补显式快捷键 |
| `Esc` | 关闭当前非模态覆盖层（内联设置侧板 / 抽屉）→ 否则「停止本轮」 | **顺序：模态优先**（模态自带 Esc）。不要重复处理导致模态被关两次，也不要让内联面板抢掉模态的 Esc |
| `Ctrl/Cmd + ,` | 打开设置（默认分区 = 上次访问） | 桌面惯例 |
| `Ctrl/Cmd + /` | 快捷键帮助 | 可后置 |

### 9.4 `prefers-reduced-motion`（**引擎已自动接入，不要手写 JS**）

**链路核实**（flutter-arch §6.6 的引擎源码证据）：引擎监听 `(prefers-reduced-motion: reduce)`
（`media_query_manager.dart:26`）→ 写入 `AccessibilityFeatures{reduceMotion, disableAnimations}`
（`platform_dispatcher.dart:1287-1302`，注释说明 Web 上两者无区分）→ 框架暴露
`MediaQuery.disableAnimationsOf(context)` → **`AnimationController` 默认就尊重它**
（`normal` 行为会缩短时长）。

所以：
- ✅ **不需要**任何手写 `matchMedia` 监听；用 `MediaQuery.disableAnimationsOf(context)` 做条件渲染；
- ✅ Material 的 `AnimatedContainer` 等隐式动画自动变快；
- ✅ 多帧图片（GIF）也会被暂停。

**但有两个"免疫"的漏网之鱼，必须手动处理**：

| 漏网 | 原因 | 应对 |
|---|---|---|
| `AnimationController.repeat()` 类动画 | 重复动画的默认 `AnimationBehavior.preserve` **刻意免疫**（防止高频闪烁） | 本项目唯一的持续动画（`thinking` 呼吸光）必须显式 `AnimationBehavior.normal`，或在 `disableAnimationsOf == true` 时**不启动** |
| `CircularProgressIndicator` | 靠 `Ticker` 驱动而非 `AnimationController` 的时长缩短 | **不得用于装饰性加载**；只允许表意场景（流式中的 `StreamingIndicator`、渲染面加载中、测试按钮 pending）。装饰性场景改用静态占位 |

> **对调研的裁决（C）**：survey 提到 my-neuro 用 CSS `@media (prefers-reduced-motion: reduce)`
> 兜底、Nexus 也有全局兜底——那是 **Web/CSS 项目**的做法。Flutter Web 的自绘渲染不吃 CSS 媒体查询，
> 但引擎**已把该偏好接进 `AccessibilityFeatures`**。所以本规格以 flutter-arch 的引擎链路为准，
> **不写 JS**，只处理上面两个漏网。

### 9.5 对比度与字号的无障碍底线

- 承载文字信息用 `contentMuted`（`onSurface` @ 0.74）或更高；`contentFaint`（@ 0.60）
  **只允许用于装饰/图标**，不得承载文字（现状 `main.dart:463` 用 0.6 显示 11 px 的角色名 =
  低于门槛，重写时改为 `contentMuted`）。
- 中文正文下限 12 px；绝对下限 11 px（`labelSmall`）。
- §11 钉子 11 断言：`lib/ui/**` 里没有把 `contentFaint` 传给任何 `Text`。

---

## 10. 迁移与实施顺序

### 10.1 三类处置（点名文件）

#### A. 保留（**不要动**，或只做机械的 import 调整）

| 文件 | 理由 |
|---|---|
| `lib/settings/display_prefs.dart` | **L1 纯逻辑的活样本**：文件级 `library;` + 头注声明「纯逻辑无 web 依赖」；反序列化永不抛异常；被 17 条测试逐条钉死。它是「用户意图与设备实际输出正交」的正确设计范例 |
| `lib/audio/gain.dart` | 把「滑杆位置 → 线性增益」的算术从 `package:web` 后面抽出来的范式；`playbackGain(muted)` 是静音语义的单点真源 |
| `lib/audio/schedule.dart` | `scheduleSlice` + `LevelTimeline` 是两条不变量的全部算术核心，注释里有实测数据（17 音源同时响、峰值 1.109），**一个字都不要改** |
| `lib/audio/audio_player.dart` | 三条不变量的实现（§7.5）。**保留核心，只做接口扩展**（新增只读的 `unlocked` / `contextState` 供 `AudioBar` 展示）；**不改调度与增益行为** |
| `lib/api/action_source.dart` | **本轮已落地的缺陷修复**（§0.3）。`ActionSource` + `ActionDetail.fromData` 是解析的唯一入口 |
| `lib/live2d/live2d_transport.dart` | 传输抽象正确（只搬 JSON 字符串，不理解协议语义） |
| `lib/live2d/live2d_host_web.dart` | iframe 平台视图 + 严格 `source`/`origin` 校验 + `postMessage(jsonString, location.origin)`，**安全边界正确** |
| `lib/live2d/live2d_host_stub.dart` | 非 web 平台占位（36 行），测试依赖它 |
| `lib/ui/theme.dart` | `kAppFontFamily` 与 CJK 本地化守卫的落点。**扩展**为 tokens 的接线点（`buildAppTheme()` 里加 `extensions: [AppColors...]` 与 `textTheme: buildTextTheme()`），保留 `fontFamily` |
| `shell/flutter/pubspec.yaml` | 依赖维持 `flutter` + `http` + `web`（C2） |
| `shell/flutter/analysis_options.yaml` | 保持 `include: package:flutter_lints/flutter.yaml`（不引入 `custom_lint`，见 §2.8.4） |
| `web/index.html` | 保持自托管字体与本地 CanvasKit（C1） |
| `test/**` 全部 57 条 | **回归基线，一条都不删** |

#### B. 重构（保留语义，拆解结构）

| 现状 | 问题 | 处置 |
|---|---|---|
| `lib/main.dart`（473 行：AppShell + 聊天面板 + `_WsBadge` + `_EmptyHint` + `_ErrorBanner` + `_MessageBubble`） | 一个文件承担 4 个职责；`_ChatPanel` 内联在 AppShell 的 `LayoutBuilder` 里；布局决策不可测（`>= 900` 硬编码） | 拆为 `app/app_shell.dart`（装配 + `IndexedStack`）· `ui/chat_panel.dart` · `ui/message_bubble.dart` · `ui/connection_badge.dart` · `ui/error_banner.dart` · `ui/audio_bar.dart`。**`main.dart` 只留 `main()` + 两个 localStorage helper + `Live2DShellApp`** |
| `lib/chat/chat_controller.dart` | 混了「事件投影」与「状态派生」；`ActionStateEvent` 完全忽略 | 保留事件投影与 epoch→打断逻辑（那是**展示层调度决策**，不是核心逻辑复制）；把状态派生抽到 `lib/state/ui_phase.dart`；新增 `actions/action_history.dart`（纯列表模型）消费 `ActionEvent` |
| `lib/api/ws_client.dart` | 协议解析埋在 `package:web` 后面 ⇒ **在 VM 上根本跑不到**（这正是 `action_state` 缺陷能长期存活的根因） | 抽 `lib/api/ws_frame.dart`（见 §10.2）；`ws_client.dart` 只留 WebSocket 生命周期 / 退避 / 状态流 |
| `lib/api/api_client.dart` | ~~具体类 + 构造里 `new http.Client()` ⇒ 不可注入~~ **注：本轮已改为可注入**（`ApiClient({String? base, http.Client? client})`，见 §0.3-5） | 保留已落地的可注入设计；后续新增 models/mods/logs/commands 端点时沿用同一模式（`client` 参数 + `_guard` + `_errorFrom`）。**不要再引入 `SettingsApi` 接口层**——`MockClient` 已在 VM 门禁里覆盖契约，多一层接口是净成本 |
| `lib/ui/display_panel.dart`（205 行，`_SliderRow` 只支持滑杆且缺 `semanticFormatterCallback`） | 只覆盖一种控件、无语义值 | 泛化为 `ui/field_row.dart`（滑杆/开关/文本/数字/下拉/分段 + 语义值 + 错误槽），`display_panel.dart` 的内容迁入「动作与互动」分区的「舞台与口型」分组。**保留**：实时下发手感、标定说明文案、「恢复默认」按钮、静音时滑杆不置零 |
| `lib/live2d/live2d_stage.dart` | 混了「桥管理」与「覆盖层 UI」；FPS 徽标常驻占主视线 | 拆出 `ui/stage_host.dart`（保活 + `RepaintBoundary` + 加载/错误覆盖层 + 语义标签 + 舞台角标工具条）；FPS 徽标**移到诊断分区**，舞台上只在 `dev_mode` 时以 hover 形式出现 |
| `lib/live2d/live2d_bridge.dart` | 只实现 4 类下行；忽略 `stage-ack` | **保留**状态机 / ready 超时 / mouth 30 Hz 节流 / 入队上限 32 / 容错；**新增** `sendActionState` / `sendStageZoom` / `sendStageBg` + `StageAckEvent` 解析 + `sync.clickEnabled`（§8.6） |

#### C. 新增（点名）

```
lib/design/tokens.dart                  lib/design/breakpoints.dart
lib/design/typography.dart              lib/state/ui_phase.dart
lib/api/ws_frame.dart                   lib/api/models_api.dart
lib/api/mods_api.dart                   lib/api/diagnostics_api.dart
lib/api/commands_api.dart               lib/settings/settings_sections.dart
lib/settings/settings_controller.dart   lib/actions/action_event.dart
lib/actions/action_history.dart         lib/app/app_shell.dart
lib/app/nav_host.dart                   lib/ui/settings_scaffold.dart
lib/ui/field_row.dart                   lib/ui/section_header.dart
lib/ui/glass_panel.dart                 lib/ui/state_pill.dart
lib/ui/streaming_indicator.dart         lib/ui/audio_bar.dart
lib/ui/chat_panel.dart                  lib/ui/message_bubble.dart
lib/ui/connection_badge.dart            lib/ui/error_banner.dart
lib/ui/stage_host.dart                  lib/ui/action_toolbar.dart
lib/ui/action_grid.dart                 lib/ui/action_history_view.dart
lib/ui/action_source_badge.dart         lib/ui/model_picker.dart
lib/ui/mod_list.dart                    lib/ui/log_viewer.dart
lib/ui/diagnostics_panel.dart           lib/ui/test_connection_button.dart
lib/settings/sections/{persona,models,llm,tts,actions,mods,diagnostics,developer}_section.dart
test/design_tokens_test.dart            test/design_tokens_lint_test.dart
test/breakpoints_test.dart              test/ui_phase_test.dart
lib/actions/action_log_controller.dart  lib/api/commands_api.dart（P2 提前）
lib/api/ws_status.dart                  lib/chat/chat_message.dart
lib/state/ui_state_tracker.dart         lib/app/nav_host.dart
lib/ui/settings_scaffold.dart           lib/ui/glass_panel.dart
lib/ui/section_header.dart              lib/ui/state_pill.dart
lib/ui/streaming_indicator.dart         lib/ui/audio_bar.dart
lib/ui/chat_panel.dart                  lib/ui/message_bubble.dart
lib/ui/connection_badge.dart            lib/ui/error_banner.dart
lib/ui/stage_host.dart                  lib/ui/field_row.dart
lib/settings/settings_controller.dart   lib/settings/persona_card.dart
lib/settings/persona_import.dart        lib/api/models_api.dart
lib/api/mods_api.dart                   lib/api/diagnostics_api.dart
lib/settings/sections/llm_section.dart  lib/settings/sections/tts_section.dart
lib/settings/sections/persona_section.dart
lib/settings/sections/actions_section.dart
lib/settings/sections/dev_tools_section.dart
lib/settings/sections/pane_helpers.dart
test/ws_frame_test.dart                 test/action_history_test.dart
test/action_event_test.dart             test/action_log_controller_test.dart
test/action_ui_test.dart                test/commands_api_test.dart
test/ui_phase_test.dart                 test/ui_state_tracker_test.dart
test/app_shell_layout_test.dart         test/stage_keepalive_test.dart
test/audio_bar_test.dart                test/field_row_test.dart
test/settings_controller_test.dart      test/settings_sections_test.dart
test/settings_scaffold_test.dart        test/persona_import_test.dart
test/admin_api_test.dart                test/settings_api_test.dart
test/semantics_test.dart                test/no_backdrop_filter_test.dart
```

> **已在同一轮落地、本列表刻意不再重复**（§0.3-5）：`lib/api/settings_models.dart`
> （= 原计划的 `settings_draft.dart`，含 `Tri` 三态）、`lib/api/api_client.dart` 的设置端点与可注入
> `http.Client`、`test/settings_api_test.dart`（`MockClient`）。**实现者按已落地的文件名写，不要另造一份。**

### 10.2 协议层可测性的取舍与分阶段做法（**不是「应该抽」，是「怎么抽、抽到什么程度」**）

**问题陈述**：`api/ws_client.dart` 要 `import 'package:web'`（`WebSocket`），于是**整帧 decode
（type 分发 + 各字段解析）在 `flutter test` 里跑不到**——只能在浏览器里跑。
`action_source.dart` 的头注已经把这件事写明：「这正是下面这个缺陷能长期存活的原因」。
现状是**靠"把纯逻辑挪出去"逐个补救**（已经成功两次：`gain.dart`、`action_source.dart`），
但**帧级分发本身仍在门外**：`_handleText` 的 `switch (type)` 有 9 个分支、每分支多个字段，
一个都不在门禁里。

**结论：值得抽。抽的对象是「帧字符串 → 领域事件」这一个纯函数，不抽 WebSocket 生命周期。**

**分三个阶段做，每阶段独立可提交**：

**阶段 1（P1，本次必做）——抽 `lib/api/ws_frame.dart`**

```dart
// lib/api/ws_frame.dart —— 无 web 依赖（只用 dart:convert / dart:typed_data）
sealed class WsEvent { const WsEvent({this.seq, this.ts}); final int? seq; final String? ts; }
// …以下 9 个事件类从 ws_client.dart 原样搬来，字段不变：
// SubscribeAckEvent / HeartbeatEvent / TurnStateEvent / RuntimeStatusEvent / TextDeltaEvent
// / ActionStateEvent / AudioEvent / WsErrorEvent / UnknownWsEvent

/// 帧字符串 → 领域事件。**永不抛**：坏 JSON / 非 Map / 缺 type 都返回 null。
WsEvent? parseWsFrame(String raw);

/// 退避算术（单独抽出来是为了让「1s→2s→4s→8s→16s→30s 上限」可测）
Duration backoffForAttempt(int attemptMs);
```

- `ws_client.dart` 只保留：`WebSocket` 连接/关闭、`onmessage → parseWsFrame → _events.add`、
  退避定时器、`_statuses` 流、`dispose`。**行为零变化。**
- 收益：`test/ws_frame_test.dart` 覆盖 **9 类帧各一条 + 坏帧/未知 type/坏 base64 + 真实抓包夹具**
  （直接复用 `kRealActionFrames`，把"协议形状"钉在整帧解码这一层而不仅是 `ActionDetail` 这一层）。
- 代价（诚实）：多一个文件与一次函数调用。**性能可忽略**——每帧一次字符串 `switch`，
  相对 base64 解码与 JSON 解析是噪声。
- 风险：`sealed class` 与事件类搬家 → `chat_controller.dart` 与既有测试改 import（纯机械）。

**阶段 2（P3，可选）——让"连接策略"也可测**

把退避与重连判定抽成纯函数（`backoffForAttempt` 已在阶段 1，再加
`bool shouldReconnectAfter(WsEvent e)`——`shutdown_ready` 不再是永久停连这条语义就有钉子）。
**仍然不注入 WebSocket 工厂**：`package:web` 的 `WebSocket` 是 extension type，
为它造接口会把连接生命周期也抽象掉，成本远超收益。

**阶段 3（仅在出现第三个同类缺陷时）——注入连接工厂**

```dart
WsClient({WsSocketFactory? socketFactory, @visibleForTesting bool Function()? clock});
```
触发条件（任一）：① 又出现一个只能靠人工在浏览器里复现的协议缺陷；② 需要断言
「`onclose` 后 1s/2s/4s 的定时器确实被调度」。**在此之前不做**——门禁里多一层替身基础设施
本身就是维护成本。

### 10.3 分阶段路线（每阶段都必须 `flutter analyze && flutter test` 全绿、可独立提交）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0**（地基，无布局变化） | `design/tokens.dart` + `design/typography.dart` + `design/breakpoints.dart`；`theme.dart` 接线（`extensions` + `textTheme`，保留 `fontFamily`）；上 `test/design_tokens_test.dart` + `test/design_tokens_lint_test.dart` + `test/breakpoints_test.dart`；**顺手修 `shell/README.md` §8.1 的过期 mouth 说明** | 3 个新测试文件全绿；既有 57 条不变；`flutter analyze` 零 warning；布局与现状一致（**字号阶梯与圆角取值会变**：16/w700→16/w600、圆角收敛到 6 档——这是预期的，不是回归） |
| **P1**（协议层进门禁） | 抽 `api/ws_frame.dart`（§10.2 阶段 1）+ `test/ws_frame_test.dart`；`ws_client.dart` 瘦身；新增 `backoffForAttempt` | 9 类帧 + 2 条真实夹具 + 坏帧全绿；`ws_client.dart` 行数显著下降；行为零变化 |
| **P2**（来源可见 —— 兑现 §0.3 的缺口） | `actions/action_event.dart` + `action_history.dart`；`chat_controller` 消费 `ActionStateEvent` 不再 `break`；`ui/action_source_badge.dart` + `ui/action_history_view.dart`（先放进「动作与互动」分区的空壳里） | 收到 `perform{source:llmTool}` 时历史里出现「AI」徽标；`cease` 收口；`test/action_history_test.dart` 全绿。**这一阶段不依赖任何新后端能力，可独立交付价值** |
| **P3**（外壳骨架 + 常驻元件） | `app/app_shell.dart` + `app/nav_host.dart` 三档布局；`IndexedStack` 保活；拆 `chat_panel`/`message_bubble`/`connection_badge`/`error_banner`；`state_pill` + `state/ui_phase.dart`；`audio_bar`（三级控件 + 服务端徽标 + autoplay 解锁提示**含键盘路径**） | `test/app_shell_layout_test.dart`（1280/1279/900/899/390 五档）+ `test/stage_keepalive_test.dart`（切分区后 stage generation 不变）+ `test/ui_phase_test.dart` + `test/audio_bar_test.dart` 全绿；人工自测：**仅用键盘发消息，有声音** |
| **P4**（设置全量） | `settings_controller`（草稿=改动集/dirty/三处拦截）+ `settings_scaffold` + `field_row` + 8 个 section。**patch 三态构造已由已落地的 `api/settings_models.dart` + `test/settings_api_test.dart` 承担（§0.3-5），本阶段只补控制器与 UI**；顺序：**动作与互动（纯本地，先验证字段行与实时联动）→ LLM/TTS（消费已落地的 `fetchSettings/patchSettings/testLlm/testTts`）→ 角色卡/模型库 → Mod/诊断/开发模式** | `test/settings_controller_test.dart`（`MockClient`：超时/400 `url_invalid`/`invalid_env_name`/缺字段/`apply_status=RestartRequired`/GET-PATCH 乱序）+ `test/settings_sections_test.dart`（每项有非空 label+description，无重号）+ 已落地的 `test/settings_api_test.dart` 全绿 |
| **P5**（动作通道 + 桥扩展） | `api/commands_api.dart`；`action_toolbar` + `action_grid`（强度 SEG）+ 幂等连点本地提示；`live2d_bridge.sendActionState/sendStageZoom/sendStageBg` + `StageAckEvent`；`sync.clickEnabled`；舞台角标三键；`DisplayPrefs.allowDragZoom` 新字段 + 单元测试 | 手动点动作 → core 受理 → WS `perform{user_command}` → 桥转发 → 模型动；`test/live2d_bridge_test.dart` 新增 3 条（action-state 帧形状含根级 `action`、stage-zoom、stage-ack 解析） |
| **P6**（无障碍 + 性能收口） | `Semantics` 5 处 + `FocusTraversalGroup` + `Esc`/快捷键 + `disableAnimationsOf` 分支（呼吸光）+ `RepaintBoundary`；聊天列表上限 500 条；诊断分区一键复制 | `test/semantics_test.dart` + `test/no_backdrop_filter_test.dart` 全绿；`--profile` 构建下帧时间实测（人工）；`reduce motion` 开着时无持续动画 |

**阶段之间的依赖**：P0 与 P1 相互无强依赖（P1 只用 `dart:convert`，不碰 tokens），同批做只是为了共享一次 review；
P3 依赖 P0（断点）与 P1（`ws_frame`）；P4/P5 可并行；P6 收口。
**P2 完全不依赖 P0/P1，可随时插入**（这是把已修的缺陷变成可见价值的捷径）。

---

## 11. 测试计划

三层分工（沿用 flutter-arch §4.2 的 L1/L2/L3，判定标准按本项目收紧）：

> **L1 判定标准（可写成测试）**：一个文件属于 L1，当且仅当它
> **① 不 `import 'package:web'`；② 不依赖 `BuildContext`**。
> `display_prefs.dart` / `gain.dart` / `schedule.dart` / `breakpoints.dart` 还满足更强的一条：
> **完全不 import flutter**（只用 `dart:*`）。

### 11.1 L1 纯逻辑（VM 单测，禁 web / 禁 BuildContext）

| 模块 | 测什么 |
|---|---|
| `design/tokens.dart` + `design/typography.dart` | §2.8.2 的 4 组断言：登记表长度锁、档位互异、双面引用枚举、`copyWith`/`lerp`/`values` 字段枚举、extension 已注册 |
| `design/breakpoints.dart` | 表驱动：`1280→expanded` / `1279→medium` / `900→medium` / `899→compact` / `390→compact` / `0→compact` / 负值与 `double.infinity` 不抛 |
| `state/ui_phase.dart` | 判定顺序即契约：每个组合一条（`errorActive` 压过 `offline`、`offline` 压过 `speaking`…）；**边界**：`interrupted` 窗口到期后回落 |
| `api/ws_frame.dart` | **9 类帧各一条** + 2 条真实抓包夹具（整帧级）+ 坏 JSON / 非 Map / 缺 `type` / 未知 `type→UnknownWsEvent` / 坏 base64 丢弃 / `shutdown_ready` 语义 |
| `api/action_source.dart`（已有 12 条，保留） | 真实帧夹具、宽容解析、来源映射 |
| `actions/action_event.dart` + `action_history.dart` | `perform/cease` 投影；历史条目上限；来源中文名映射 |
| `api/settings_models.dart`（**已落地**，其测试 `test/settings_api_test.dart` 已覆盖） | **PATCH 三态构造**：未动→`Tri.keep()` 序列化为**省略键** / 清空→`Tri.clear()` 序列化为 `null` / 设值→`Tri.set(v)`；段级清空 `{"persona":null}`；`clear_api_key`；**以及「只改一个字段不得清空同段其它字段」这条最危险的回归**（已落地测试里应有；P4 需补 `SettingsController.dirty` 的等价性：键序无关、`Tri.keep()` 不计入 dirty） |
| `settings/display_prefs.dart`（已有 17 条，保留） | 含新增 `allowDragZoom` 的默认与旧存档回落 |
| `audio/gain.dart` / `audio/schedule.dart`（已有 18 条，保留） | 时间轴单调、排空锚定、`v²` 曲线、NaN 兜底、静音恒 0 |
| **导入守卫** | 扫 `lib/` 断言：`settings/display_prefs.dart`、`audio/gain.dart`、`audio/schedule.dart`、`design/breakpoints.dart` **不含任何 `import 'package:`**；`design/**` 与 `state/**`、`actions/**` **不含 `package:web`** |

### 11.2 L2 控制器（`flutter test` + 注入的接口，不测真实 HTTP/WS）

| 模块 | 替身 | 测什么 |
|---|---|---|
| `WsClient` 的纯部分 | `parseWsFrame` / `backoffForAttempt`（无替身，已是纯函数） | 见 11.1；退避序列 1000→2000→…→30000 封顶 |
| `SettingsController` | `MockClient`（`package:http/testing.dart`，零新依赖；`ApiClient` 已可注入 `http.Client`，见 §0.3-5）+ 直接构造的 `SettingsView` 夹具 | 初始 GET 失败 → 可见错误；PATCH 成功后 `_remote` 更新 + 改动集清空；dirty 判定（`Tri.keep()` 不计入）；**离开分区的拦截回调被调用**；`apply_status` 分流文案；`dispose` 无泄漏（`addListener` 计数归零）；**并发**：GET 在 PATCH 之后返回的乱序不得覆盖新值 |
| `ChatController` | Fake api + fake ws stream + fake audio | 每类 WS 帧各一条：`text_delta` 拼接、`completed` 收口、`voice_started` 置 speaking、`new_epoch` **立即打断音频**（这条是真实缺陷的修复，`chat_controller.dart:139-142`，必须有钉子）、`error` 帧写可见错误、发送失败建失败气泡、`422/429/503` 分流 |
| `ModelsController` / `ModsController` | Fake api | 激活 → `requires_restart` 提示；删除激活中的模型 → 409 `model_active` 的人话文案；Mod `config` 的半成功（`{"ok":true,"restarted":true}` vs reload 成功 restart 失败）分别渲染 |
| `Live2DBridge`（已有 3 条，扩展） | 内存 transport（已有） | ready 前入队/ready 后 flush、容忍无 `version` 的 `stage-ack`、error 帧；**新增**：`sendActionState` 的帧形状（`action` 在根、`state`/`strength` 在 `payload`）、`sendStageZoom('reset')`、`stage-ack` 解析进 `StageAckEvent` |

**关键手法（**裁决并修正调研 flutter §4.2 的建议**）**：调研建议「把网络抽象成接口，而不是 mock
`http.Client`」。但本轮已落地的 `ApiClient` 把 `http.Client` 做成**可注入**，而 `MockClient` 来自
`http` 包自带的 `package:http/testing.dart`（**零新依赖**，`http` 本就是运行时依赖，且有 315 行
`test/settings_api_test.dart` 在用）。所以：
- **HTTP 层**用 `MockClient` 覆盖请求/响应契约（含超时、409、`error.code` 未知、返回体缺字段）；
- **不引入 `SettingsApi` 接口层**——它现在只是净成本；
- **控制器依赖的仍是可注入的 `ApiClient`（或 `MockClient` 包出的实例）**，
  这样 `SettingsController` 在 VM 上可测，且不必造第二套替身体系。

### 11.3 L3 Widget（`flutter_test`）

| 测什么 | 做法 |
|---|---|
| 三档断点各有正确导航形态 | `tester.view.physicalSize = Size(1280, 900) * dpr; addTearDown(tester.view.reset);` 然后断言 `NavigationRail` 存在 / `NavigationBar` 不存在 / 抽屉入口存在 |
| **舞台保活（钉子 4）** | 注入一个计数 builder 作为 stage 替身（构造参数注入，**不要为测试给 stage 加分支判断**），切 3 个分区后断言 builder 调用次数与 generation 未变 |
| 用户可见契约 | 文案、空态、错误态、`Semantics` 标签、`Slider` 的 `semanticFormatterCallback` |
| `Esc` 与焦点 | 打开内联设置侧板 → `sendKeyEvent(LogicalKeyboardKey.escape)` → 断言关闭且焦点回到触发按钮 |
| 键盘解锁音频（钉子 9） | 注入 unlock 回调 hook，`sendKeyEvent` 后断言被调用 |

**三个必须提前知道的坑**（实测已知）：

| 坑 | 原因 | 应对 |
|---|---|---|
| `Live2DStage` 在测试里跑不起来 | `live2d_host_stub.dart` 是非 web 占位；`HtmlElementView` 在 VM 无实现 | 用**可注入的 stage 占位**，或只测不含 stage 的子树。**不要**为测试给 stage 加分支 |
| `Image.network` / iframe 不发真实请求 | `flutter_test` 的 `HttpClient` 被替换为永远返回 400 的桩 | v1 本就不拉网络图片（无缩略图），不受影响 |
| `pumpAndSettle` 在无限动画上死锁 | `CircularProgressIndicator` 永不 settle | 用 `pump(Duration(...))` 推进固定时长，**不要** `pumpAndSettle` |

**L3 不该测的**：Material 组件的内部行为、精确像素与颜色值（改 token 就会碎一地——**这正说明它不该被测试**，
token 的守卫交给 §2.8）。

### 11.4 必须加的回归钉子（点名，缺一不可）

| # | 钉子 | 为什么必须有 |
|---|---|---|
| 1 | `action_state` 的 `data.action` 为**对象**时解析出 `action`+`strength`+`source` | **已有的真实缺陷**（`action_source_test.dart` 12 条，含 2 条实抓夹具）+ **新增整帧级**（`ws_frame_test.dart`） |
| 2 | 旧稿形状（`data.action` 是字符串）**降级为空明细而非抛异常** | 把老路彻底堵死（已有测试，保留） |
| 3 | 未知 `source` → `ActionSource.unknown`，且 `unknown.isAiInitiated == false` | 协议新增来源时旧前端不崩、**且不把用户的动作标成 AI 做的**（已有，保留） |
| 4 | **舞台保活**：切分区后 `Live2DStage` 的 generation / builder 调用不变 | C5 是硬约束，且回归成本极低、破坏后果极重（模型重载） |
| 5 | `DisplayPrefs` 旧存档（无 `volume`/`muted`/`allowDragZoom`）按**新默认**读 | `muted` 默认刚翻过（`true`→`false`），老存档里的 `true` 是旧默认、不该继承——**每次默认值语义变更都要有这种回归，否则老用户升级后静默变哑巴**（已有，保留） |
| 6 | **两个静音两个名字**（文案契约）：本机控件文案含「本机」，服务端徽标文案含「服务端」 | P5 的落地；防止将来有人"统一文案"又把两个轴混成一个词 |
| 7 | 断点表驱动（1280/1279/900/899/390） | 布局决策的全部依据 |
| 8 | 令牌双向枚举（§2.8.2 的 4 组断言） | 原则 P4：「静默失效」在 4 个独立项目复现，这是**唯一**能自动抓住它的手段 |
| 9 | 键盘路径也解锁音频 | 「默认出声」与 autoplay 冲突的真实缺口（D11）；纯键盘用户现在**没有声音** |
| 10 | `lib/**` 中 `BackdropFilter` 命中数为 **0** | 性能红线 + iframe 模糊不到（#184996）；防止有人"为了好看"把它加回来 |
| 11 | `contentFaint` 不得传给任何 `Text` | §9.5 的对比度底线，且是容易被顺手写出的低对比文案 |
| 12 | 每个 `SettingsSection` 的 `label` / `description` 非空；且 8 项**恰好各出现一次**（等价于「无重复、无遗漏」） | AIRI 的 `order` 分散在 8 个文件、**重号不报错**是反例；同族缺陷还有「有翻译无控件」（Live2DPet 的 `tab.enhance` 三语都翻译了却没有对应 tab）与「可见 Tab 点进去空白」（OLV-Web 的 TTS Tab）【调研 survey §7.5.7-8、§4.8.2】 |
| 13 | `new_epoch` 到达时**立即** `audio.interrupt()` | 这是真实缺陷的修复（`chat_controller.dart:139-142`）：TTS 分片 1 ms 级突发、时间轴最多排到 3 s 之后，若只靠"下一帧带新 epoch"触发，停止按钮等于失灵 |
| 14 | 日志区在 `dev_mode=false` 收到 403 时渲染「打开开发模式后可用」而**不是**错误横幅 | 后端确实会 403（`log_routes.rs:9-10`），把用户态当异常是最常见的 UX 错误 |

### 11.5 人工自测（`flutter test` 覆盖不到，进 `docs/verification/`）

1. **仅用键盘**发一条消息，确认有声音（钉子 9 的验收）。
2. 断网（卸载 Google CDN 可达性 / 断网）后打开页面：中文正常、无白屏（C1）。
3. 开着 `LIVE2D_AI_MUTE_AUDIO=1` 启动：**听到无声、口型照动**、音频条出现服务端静音徽标。
4. 设置里打开 `dev_mode` → 日志区从 403 变为可读。
5. `--profile` 构建下测帧时间（P6）。
6. 说话中按「停止」：音频**立即**停（不是等下一帧）。
7. 说话中切换设置分区：**模型不重载**（钉子 4 的人工版）。

---

## 12. 明确不做（避免范围膨胀）

| # | 不做 | 理由 |
|---|---|---|
| 1 | Markdown 渲染 / 代码高亮 / 逐字打字机 | 产品形态是口语短句（`AGENTS.md` 语音约定「一句一单元」）；`flutter_markdown` **已 discontinued**（官方未接手，指向第三方 `flutter_markdown_plus`）；打字机与「整句合成完成→开始播放」的时序冲突【调研 oss §2】。**整句到达即整句上屏 + 轻量渐入** |
| 2 | 波形 / 频谱可视化、麦克风电平 | 属装饰性重绘；`AnalyserNode` 路径已核（0 新依赖）但收益不抵成本。**保留为将来的 0 依赖升级路径** |
| 3 | 麦克风 / ASR / 「聆听」态 | 当前链路是「用户文本 → LLM」，core 无 `Listening` 相、无 ASR 端点（§6.7） |
| 4 | 径向菜单 / 表情轮盘 | 6 个动作不需要；径向不是行业共识（三源证据） |
| 5 | 「自动/手动」总开关 | 与 core 的优先级仲裁语义重复，且会制造「关了自动但 AI 仍影响」的困惑。**做「手动优先 + 显式展示归属」** |
| 6 | 亮色主题 / 主题换色器 | dark-only 是桌宠形态的刻意选择；换色器会把 token 体系变成字符串查表（违反 §2.1） |
| 7 | `BackdropFilter` 毛玻璃 | iframe platform view **模糊不到**，代价照付（#184996 / #89889） |
| 8 | 自绘 Knob / 颜色选择器 / SVG 图标 / 第三方图标包 | 内建 `Slider` + `SegmentedButton` + 预设色板 + `Icons.*_outlined` 已覆盖；非 const `IconData` 会让 `flutter build web --release` **硬失败** |
| 9 | 窗口能力（置顶 / 点击穿透 / 托盘 / 窗口尺寸预设 / 无边框透明） | 属 Rust 桌面壳职责；`winit::set_cursor_hittest` 在 **Web 永远返回 NotSupported** |
| 10 | 在 Flutter 侧复制状态机 / 动作仲裁 / LLM/TTS 协议 | 治理红线（C6） |
| 11 | `action_state` 的 `dropped` 原因帧 | **前端做不到**（`RootFact::Dropped` 是无字段单元变体、只进 tracing）→ 待裁决 5 |
| 12 | `sync.paused` 控件 | Flutter 桥会发该字段，但 **wasm 全文没有 `paused` 分支** ⇒ 会静默失效的字段不该做成 UI 控件 |
| 13 | 展示 `status.audio`（`backend` / `available`） | 是**硬编码占位常量**（`app_routes.rs:179-184`），展示假信息 |
| 14 | 编辑 `tts.response_format` | `TtsPatch` 没有这个字段 |
| 15 | 模型的 ZIP/拖放上传 | 后端 D3.2 后置；v1 只接受受控本地 id |
| 16 | 模型缩略图 | 后端无缩略图端点；Web 平台 `cacheWidth`/`cacheHeight` 行为与原生不同 |
| 17 | 「点击角色有反应」 | wasm 侧没有命中测试（ArtMesh/包围盒），拖拽在整个 canvas 上生效 |
| 18 | 聊天历史落 localStorage | v1 纯内存 + **上限 500 条滚动丢弃**（现状是无界 `List` 且不持久化 = 真实风险）→ 待裁决 3 |
| 19 | 诊断日志的「上传 → 复制 URL / 一键分析」两步式 | v1 只做「复制诊断快照到剪贴板」 |
| 20 | Service Worker / 缓存策略 | Flutter 3.47 生成的 SW 是**自注销 + 强制刷新**的，且 Rust 侧已对 `/app/*` 加 `Cache-Control: no-cache`——框架已处理，**不要自己写** |
| 21 | 引入 `provider` / `riverpod` / `bloc` | `ChangeNotifier` + `InheritedNotifier` 够用；现有 `ChatController`/`Live2DBridge` 都已是这个形态且已在跑测试。触发条件（flutter-arch §3.2 的 T1–T5）出现前**不引入** |
| 22 | 引入 `google_fonts` / `flutter_animate` / `rive` / `lottie` / `flutter_markdown` / `flutter_svg` / `material_symbols_icons` | 全部有明确的 reject/HOLD 结论（C2 + 许可与维护状态）【调研 oss §11】 |
| 23 | 迁移字体到「霞鹜文楷 Lite」 | **裁决（H）**：oss-adoption 推荐 LXGW（改名成本低），但现状**已经落地 Noto Sans SC 子集 + 3 条测试守卫 + `AGENTS.md` 硬约束**，迁移是纯成本。合规性也已满足：OFL.txt 声明的 RFN 是 `'Source'`（不是 `Noto Sans SC`），所以我们的家族名 `NotoSansSC` 不触碰 RFN；OFL 文本随产物分发（`test/theme_test.dart` 有守卫） |

---

## 13. 用户裁决记录（2026-09-10，**已定，按此实现**）

规格初稿列了 12 条待裁决项。用户裁决如下；**下面每条都是最终口径**，
实现时不要再回头问，也不要按「我的倾向」列去猜。

### 13.1 已裁决（9 条：直接采纳规格倾向）

| # | 决策点 | 裁决 |
|---|---|---|
| 1 | compact（<900）导航形态 | **抽屉 + 底部弹层**（严格 2 层）。不用 `NavigationBar` 5 项 + 「更多」（3 层，违反 NN/g） |
| 2 | 「外观与语音」独立成第 9 分区？ | **不独立**。音量/静音升为常驻音频条，其余并入「动作与互动」 |
| 4 | 「聆听」态 | **不做**（当前无输入侧、无 ASR 端点；core 无 `Listening` 相） |
| 6 | AI 来源徽标用 M3 `tertiary` | **允许，且仅此一处** |
| 8 | 音频条常驻位置 | **聊天面板底部、输入框上方** |
| 9 | 诊断导出 | **v1 只做复制快照到剪贴板**，不做「上传 → 复制 URL」 |
| 10 | 修 `shell/README.md` §8.1 过期 mouth 说明 | **已修**（随本规格同笔提交） |
| 11 | `--dev-mode` 覆盖 UI 开关 | 显示「当前由启动参数强制开启」，**不谎报成功** |
| 12 | FPS 徽标 | **移出舞台**，进诊断分区；`dev_mode` 时以 hover 形式出现 |

### 13.2 用户另有明确指示（3 条，**推翻了规格原倾向**）

#### ① emoji / 生僻字不走 gstatic 兜底：**不做**

> 用户原话：「emoji 算了，以后会做表情和表情包相关，现在只做文字回复即可」

- **不扩字体子集、不加彩色 emoji 字体、不拦截 gstatic**。
- 现阶段**只做文字回复**；emoji 与「表情/表情包」是**以后**的独立话题。
- 因此 §2.8 / §11 不需要为 emoji 加任何钉子；
  但**保留**「子集体量下限」守卫（防有人把子集缩小到只留 UI 用字）。
- 已知局限如实记录：断网时彩色 emoji 与 CJK 扩展区可能缺字。**不假装已解决。**

#### ② 聊天历史**不落盘** —— 并且这是更大范围的一条口径

> 用户原话：「本身就做核心链路和 live2d 口型 + llm + tts + ui 啊，特意不做类似功能堆砌啊，
> 堆砌就往 mod 方向仅提供接口即可。目前就核心工程最简最易维护管理即可」

- **聊天历史 v1 不落 localStorage**：纯内存、上限 500 条，刷新即丢。
- **更重要**：这是**项目范围的裁决**，不只是这一条 UI 决策——
  本项目只做「核心链路 + Live2D 口型 + LLM + TTS + UI」。
  **刻意不做功能堆砌**；要堆砌的功能一律走 **Mod 方向，且核心只提供接口**。
  **当前目标是核心工程「最简、最易维护管理」。**
- **对本规格的约束**：§5 组件清单与 §12「明确不做」应以「能不能不加」为准绳复查；
  任何「顺手加一个」的功能都要先问「这是不是堆砌」。发现堆砌倾向应写进 `docs/plans/` 的 Mod 待办，
  而不是塞进核心。

#### ③ 不在 Rust 侧新增 `dropped` 帧：**只做前端**

> 用户原话：「只做前端相关。动相关我核心链路确认通过走导演层编排」

- **本轮不动 Rust**。不加 `action_state{kind:"dropped"}`。
- 动作编排（choreography）**等核心链路确认后走导演层**（`live2d-ai-mod-director`）。
- 因此 §8.3 只实现「正在演」+ 本地连点提示；
  **「AI 想动但被仲裁压住」在当前前端不可观测**，如实写进文档，不要用假状态凑。

### 13.3 待裁决项 ③⑤ 的**新口径**（影响 §4.1.3 与 §4.1.1，实现时注意）

#### 默认提示词：**不注入任何角色/格式提示词**

> 用户原话：「本身 llm 是输入输出一问一答形式，**我们主动做** 句号、问号、感叹号
> 的标记分割成多个句子，**通过限定 token 输出来控制**」

这条推翻了 v0.4.2 写进 `live2d-ai.toml` / `.example` 的「微信式短对话」`system_prompt`。裁决：

1. **默认 `system_prompt` 为空**（不预设任何角色卡、不做提示词注入）。
   规则如「1–5 句、每句以真实句读结尾」**不写进默认提示词**——
   那是**由我们在代码侧保证**的，不靠提示词求 LLM 配合。
2. **分句由前端/核心代码主动做**：按 `。！？` 标记切分成多个句子
   （已有实现：`crates/live2d-ai-runtime/src/dialogue/sentence.rs`）。
   这与 AGENTS.md 的「一句一单元、不断句」契约一致，且**不再依赖模型守规矩**。
3. **长度由 token 上限控制**（用户明确指定）——**已完成（v0.4.7，2026-09-10）**。
   落地内容：`LlmConfig.max_tokens`、`LlmSettings.max_tokens` + `DEFAULT_MAX_TOKENS = 512`、
   `LlmView.max_tokens`（回生效值）、`LlmPatch.max_tokens`（三态）、
   `ChatRequestBody.max_tokens`（**`0` 时整个字段省略**，因为部分 OpenAI 兼容
   实现把 `0` 读成「拒绝生成」）。前端镜像 `LlmSettingsView.maxTokens` +
   `LlmSettingsPatch.maxTokens: Tri<int>?`。
   > 上一版这里写的是「全仓库 grep `max_tokens` 零命中」——**那已过期**。
   > 本轮后端**只**加了这一个字段（用户口径：「这一轮后端只多改一个 maxtoken」）。
4. 设置里提供**酒馆式角色卡字段 + 导入**，让用户自己填/导入（见 13.4）。

#### 角色卡导入：**JSON + PNG 内嵌卡**（用户选定 B）

- 支持 **SillyTavern 角色卡 JSON**（`name` / `description` / `personality` /
  `scenario` / `first_mes`）。
- 支持 **PNG 内嵌角色卡**：读 PNG 的 `tEXt` chunk 里 key 为 `chara` 的
  **base64 编码 JSON**（酒馆卡常见形态）。
- **纯前端解析、零新依赖**（PNG chunk 解析手写即可，不需要图片库）。
- **不内置任何预设角色卡**（用户明确要求）。

### 13.4 ~~需要用户在下一轮确认的一件事~~ —— **已裁决（2026-09-10）**

「限定 token 输出」必须动 Rust（见 13.3-3），而 13.2-③ 的「只做前端相关」语境是
**动作 / `dropped` 帧**，不是 LLM 配置。

**用户裁决**：「这一轮后端只多改一个 maxtoken」——
即 `max_tokens` **批准作为本轮唯一的 Rust 改动**，其余仍然只做前端。
已落地并提交（v0.4.7）。

### 13.5 P2 落地时新增的两条工程裁决（2026-09-10）

#### ① `api/commands_api.dart` 从 P5 **提前到 P2**

规格的 P5 才列 `commands_api.dart`。但 P2 要求动作日志显示**中文名**，而中文名的
权威来源是服务端 `GET /api/v1/commands`（`target_action` + `name`）。

在 Flutter 侧内建一张「`nod` → 点头」映射表有两个问题：服务端加/改动作时前端
**静默**显示内部 id；并违反 AGENTS.md「不得在 Flutter 侧复制核心逻辑」。

⇒ 把 `commands_api.dart` 提到 P2（它本来就是必须做的，只是提前）。
**P5 不再需要新建它**，只补 `invoke`（`POST /api/v1/commands/{id}/invoke`）。
缺省降级：取不到目录就显示**协议名**（`tilt`）——看得懂，且不撒谎。

#### ② 新增 `actions/action_log_controller.dart`（原文件清单里没有）

`chat_controller.dart` 必须 `import 'package:web'`，**整体在 `flutter test` 里加载不了**。
若接线只有它一条路，P2 的全部价值（「`source` 终于看得见」）就永远测不到——
这正是 §0.3 那个缺陷能长期存活的原因。

⇒ 把「WS 帧 → 动作历史 + 未读 AI 动作计数」抽成一个**不碰 `package:web`** 的接线层，
于是测试可以**直接喂真实抓包的帧原文**、断言历史里的中文名与来源徽标。
`ChatController` 只持有它并转发 `action_state`（不再自己读字段）。

顺带落地的**未读角标**：`perform{source: llm_tool|rule_fallback}` 会让入口按钮出现
未读计数。「AI 自己触发了动作」是个转瞬即逝的信号（演完就没了），日志静静躺在
弹层里等于没提示——这是 §8.3「三处呈现」第 3 处（舞台小卡）的**最小可用替代**；
舞台小卡本身仍属 P5。

---

### 13.6 P3 落地时的新增裁决与偏离（2026-09-10）

#### ① 三个纯模型被从 web 依赖后面搬出来

`WsStatus`（原在 `ws_client.dart`）、`ChatMessage`/`ChatRole`（原在
`chat_controller.dart`）都必须和 `package:web` 待在一起，于是**整个状态呈现层
在 `flutter test` 里加载不了**——包括 §6.3 那张「色 + 形 + 字 + 节奏」映射表。

→ 抽出 `api/ws_status.dart` 与 `chat/chat_message.dart`（都无 web 依赖），
原文件 `export` 它们以保持既有 import 不变。**P1 的纪律在 P3 继续执行。**

#### ② 新增 `state/ui_state_tracker.dart`（原清单里没有）

`ui_phase.dart` 是纯函数，但「信号从哪来、什么时候清」同样是逻辑，而且更容易错。
→ 抽成不碰 `package:web` 的 `UiStateTracker`（只消费已解析的 `WsEvent`/`WsStatus`），
定时器可注入，于是「打断保持期」可确定性地单测。

**它当场抓出两个真缺陷**：
- `_cancelInterrupt()` 顺手清了 `_interrupted` 标志，导致 `new_epoch` 分支里刚设上的
  标志被自己抹掉——「已打断」**一次都显示不出来**；
- `error` 帧缺 `message` 时，`ws_frame.dart` 用 `?? raw` 退回整帧 JSON，而
  `message` 是**直接上屏**的字段 → 用户会看到一坨 JSON。
  现改为 `message` 为空串 + 新增 `raw` 承载整帧原文。

#### ③ `StatePill` 只出现在 AppBar（§6.4「二选一」的落地口径）

§6.4 写「聊天面板头 + 舞台角标（二选一，按断点）」。P3 实现时一度在 AppBar 与
聊天面板头**各放一个**，测试立刻抓到「同一个相位有两份」——那正是 §6.3 硬规则 1
要治的病。**统一为 AppBar 一处**（三种断点都在，与 `ConnectionBadge` 并列），
`ChatPanel` 头部只留设置入口。

#### ④ 内联侧板**不做**「点空白关闭」（诚实记录）

§4.2 写「可 Esc / 点空白关闭」。内联侧板浮在**舞台列**之上，而舞台是 iframe
平台视图——**平台视图先吃掉指针事件**，铺在下层的点击捕获区拿不到舞台区域的点击。
能拿到的只有少量 Flutter 绘制的空隙，不足以称为「点空白关闭」。
实际提供三条关闭路径：**Esc / 侧板右上角 ✕ / 再点一次 rail 上同一个分区**。

#### ⑤ 设置浮层需要一个内部 `ValueNotifier` 才不显示旧内容

`showModalBottomSheet` 的 `builder` **只在路由入栈时构建一次**，之后外壳
`setState` 不会重建浮层内容。于是「在浮层里点另一个分区」或「刚点 rail 就打开浮层」
都会显示**旧分区**——用户看到点了没反应。
→ 外壳内部用 `ValueNotifier<SettingsSection>` 作为导航单点真相（对外仍是
`section` + `onSectionChanged` 的受控接口，`didUpdateWidget` 同步）。

#### ⑥ P3 只交付**一个**真正可用的分区

8 个分区里只有「动作与互动」有真内容（迁入既有 `DisplayPanel` + P2 的
`ActionHistoryView`）；其余 7 个用 `SettingsPendingPane` **如实写明「还没接上」**。
理由：给一个看起来能用但点了没反应的面板，比说「还没做」更糟（同类项目最一致的
缺陷就是「静默失效」）。7 个分区的 pane 属 P4。

#### ⑦ 新增令牌家族 `AppRhythms`（语义节拍）

思考呼吸（1.4 s）与打断保持（1.2 s）不属于 `AppDurations` 的「UI 过渡 4 档」，
但**更不该**以裸 `Duration(milliseconds:)` 写在组件里——设计令牌门禁会拦，
而且两个指示器各写一份就会跑不同步。
→ 新增 `AppRhythms`（`thinkingBreath` / `interruptedHold`，**登记表 2 项**）。
`StreamingIndicator` 与 `StatePill` **共用** `thinkingBreath`：两个「思考中」
指示器同频，看起来是有意为之。（曾有第三个 `streamingDots`，与
`interruptedHold` 同值 1200 ms，被「同值档位没有信息量」那条令牌规则正确地拦掉了。）

---

### 13.7 P4 落地时的发现与裁决（2026-09-10）

#### ① 两个**能力缺口**：`apply_status` 与 `clear_api_key` 前端根本表达不出来

规格 §4.4 要求「必须消费 `apply_status`」，但 `SettingsPatchResult` **没有这个字段**；
§4.3 的表格里有「清除密钥绑定 → `{"llm":{"clear_api_key": true}}`」，
但 `LlmSettingsPatch` / `TtsSettingsPatch` **都没有 `clearApiKey`**。

⇒ 两处都补上了。第二条不是纸面问题：**「清除密钥绑定」这个能力此前在前端完全不可达**
（就算硬编码也构造不出那个 JSON）。补上后实测走通：

```
PATCH {"llm":{"clear_api_key":true}} → persisted:true, apply_status:applied
GET  → "has_api_key": false，且配置文件里的 api_key_env 行消失
```

（验证后已用快照逐字节还原，md5 `b13317da193dd7038e032b33d245418f`。）

**只发 `clear_api_key: true`，不额外塞 `api_key_env: null`**：服务端的
`inject_clear_key_flag` 规则 1 已经负责「段在场 + clear + 没给值 → 注入字段级清除」。
两套机制表达同一件事，将来语义漂移时没人知道该信哪个。

#### ② `FieldRow` 要拆成一组小 widget，**不要**写成泛型大 widget

第一版是 `FieldRow<T>` 带 6 个具名构造（`.slider`/`.toggle`/…）。编译器报了
**19 个错**：`T` 不能用在常量列表里、每个构造要把用不到的字段显式初始化、
回调类型还得 `as dynamic` 强转。读起来也比分开写更费劲。
⇒ 改为 `_FieldShell`（共用外壳）+ `SliderField`/`ToggleField`/`TextFieldRow`/
`NumberField`/`SegmentedField`/`DropdownField`/`ReadonlyField`/`FieldActionRow`。
**类型是真类型，测试也能按 widget 类型定位。**

顺带定了一条：**只读值用文本，不用禁用输入框**——禁用输入框会让用户以为
「本来能改，现在不能」，只读文本才诚实地表达「这不是可配置项」。

#### ③ `allowDragZoom` / `tier` 进 `DisplayPrefs`，**必须同时进 `==` 与 `hashCode`**

`AppShell` 的 `_updatePrefs` 用 `if (next == _prefs) return;` 做短路。漏字段
= 「改了不生效」，而且**界面上完全看不出来**（滑杆能动、值却不落盘也不下发）。
⇒ 两个字段都进 `==`/`hashCode`/`toString`，并加一条专门的钉子测试
（`display_prefs_test.dart`：`base == base.copyWith(allowDragZoom: false)` 必须为假）。

`allowDragZoom` 下发到渲染面 `sync.clickEnabled`（`l2d-wasm-demo/src/main.rs:334`），
它管的是**拖动 / 滚轮 / 双击复位**。所以文案是「允许拖动与缩放」，
**不是**「点击互动」——后者会让用户以为点角色会没反应（调研 control §9-7）。

#### ④ 「动作试演」留到 P5，**不占位假装有**

§4.1.5 的 B 组（动作触发按钮）需要 `POST /api/v1/commands/{id}/invoke` 与
「正在演」状态，是一条独立通道。P4 的「动作与互动」只交付 A 组（舞台与口型，
纯本地）+ C 组（动作日志，P2 成果），B 组**如实写明属于下一阶段**。

#### ⑤ 角色卡导入：PNG 的 `iTXt` **压缩**分支不支持（诚实记录）

只读 `tEXt`（未压缩）与 `iTXt` 中 `compression_flag == 0` 的块。
`iTXt` 压缩需要 zlib 解压：Web 上 `dart:io` 的 `ZLibCodec` 不可用，
为这一个分支引 `package:archive` 不值得。**如实报错**，不返回半个卡。

顺带定了一条判据：一张 JSON 是不是角色卡，看**有没有出现过任何已映射的键**，
而不是「值是否非空」——`{"foo":1}` 该被拒（否则界面显示「已导入」而四项全空），
而 `{"name":123}` 该被接受（那是一张真的卡里的一处坏字段）。

#### ⑥ 前端比裸 HTTP **更保守**：`pruneAgainstRemote`

`max_tokens` 的「省略」与「显式 512」在服务端是**不同的存储状态**
（`None` vs `Some(512)`），虽然生效值相同。实测：手工发
`{"llm":{"max_tokens":512}}` 会 `persisted:true` 并**改写配置文件**。

前端走 `edit()` 时不会发生这件事——`pruneAgainstRemote` 会把「与远端生效值相同」
的字段**撤回**（等价于没改）。即：用户在输入框里把 512 敲成 512，不会写盘。
这条是刻意的：**能不动用户的文件就不动。**

#### ⑦ 服务端缺陷（**已确认、本轮不修**）：外部改配置后 HTTP 快照不刷新

实测（本机 v0.4.8）：

```
1. 手工把 live2d-ai.toml 的 model 改成 "stale-probe-model"
2. 等文件监听触发 → 日志确认 "配置文件外部修改 detected，触发 supervisor reload"
3. GET /api/v1/settings      → llm.model = "deepseek-flash"   ← 旧值
   GET /api/v1/app/status    → llm.model = "deepseek-flash"   ← 旧值
4. 随便发一个 NoChange PATCH → 之后 GET 才变成新值
```

原因（读源码确认）：`file_watcher.rs` 只调 `supervisor.reload()`；
`StatusContext.settings`（`app_routes.rs:51`）**只被 PATCH 路径的 `swap_settings`
更新**（`dispatch.rs:116`）。

**影响**：手工编辑配置文件后，supervisor 用的是新配置，而界面显示的是旧配置——
**两者不一致且界面在说谎**。前端**做不到**修复（不能靠「偷偷发一个 PATCH」去
刷新，那会改写用户的文件）。

**对 P4 的影响**：设置面板的「加载」只对**通过界面改过**的设置可靠。
诊断分区如实标注日志需要 dev_mode；不对设置面承诺「反映手工编辑」。

**修法**（单独一轮）：给 `StatusContext` 加一个「从磁盘重读」的入口，
让 `file_watcher` 在 reload 成功后调用它（顺带把 `to_toml_string` 换成
`toml_edit` 以保住注释——那是另一个已确认缺陷，见 v0.4.7 的 CHANGELOG）。

---

## 附录 A：四份调研的冲突裁决记录

| # | 冲突 | 各自依据 | 裁决与理由 |
|---|---|---|---|
| A-1 | **静音的两个轴** | control §6 要求两套词（OBS 先例）；survey 无相反意见 | 采纳 control。**两个轴、两个名字**，逐字文案见 §7.2 |
| A-2 | **≥1280 的设置宿主** | flutter-arch §2.4：`NavigationRail(extended)` + 分区内容**内联第三栏**；survey §1.1：10 个样本里 7 个用浮层而非分栏 | **修正 flutter-arch**：rail 仍 extended，但设置内容**浮在舞台列右缘**而非新增一列。理由：1280 下「232 rail + 400 设置 + 380 聊天」会把舞台压到 268 px，而舞台是产品核心（P1）；浮层同时保住「滑杆与舞台同屏」 |
| A-3 | **compact 的导航** | flutter-arch §2.4：底部 `NavigationBar` + 弹层；control §3.2 + NN/g：>2 层即低可用 | 采纳 **NN/g 硬约束**：8 分区 > M3 的 5 项上限，「5+更多」= 3 层 → 改用抽屉（2 层） |
| A-4 | **`prefers-reduced-motion` 的实现方式** | flutter-arch §6.6：引擎**已自动接入**（`media_query_manager` → `AccessibilityFeatures` → `AnimationController` 默认尊重）；survey（my-neuro / Nexus）用 CSS `@media` 兜底 | 采纳 flutter-arch（有引擎源码链路）。Flutter Web 自绘不吃 CSS 媒体查询；另补两个漏网（`repeat()` 的 `preserve` 免疫、`CircularProgressIndicator` 靠 Ticker） |
| A-5 | **UI 状态源是否包含 core `Phase`** | control §5.1 把 `Phase` 列为「已在 WS 上的状态源」；flutter-arch §2.1 亦暗示状态齐全 | **实测否决**：`web_api/ws/*.rs` **没有** `phase` 投影（grep 零命中）。UI 状态必须从 `runtime_status` + `text_delta` + `turn_state` + `epoch` **派生**。裁决 D |
| A-6 | **毛玻璃卡片** | survey §9 把 AIRI 的「4 px 描边 + backdrop-blur」列为「值得直接抄的配方」；oss §7 证明 `BackdropFilter` **模糊不到 iframe platform view**（#184996）却照付代价 | 采纳 **oss**（有 issue 证据）。`GlassPanel` = 半透明纯色 + 1 px 高光描边，**零 `BackdropFilter`**（钉子 10） |
| A-7 | **圆角档位** | flutter-arch §1.3：4/6/8/10/12；survey §9：Meuxe 的 12/12/16/20/24 | 取 **4/8/12/16/20/999**：删「一像素一档」（Nexus 四档同值的教训），采大圆角趋势，保留 4 px 给密集元素 |
| A-8 | **字体选型** | oss §5.4：推荐 LXGW WenKai Lite（附加许可允许保留字体名）；现状与 `AGENTS.md`：已落地 Noto Sans SC 子集 | **保持 Noto Sans SC**（已落地 + 有测试守卫 + AGENTS.md 硬约束）。合规已满足：RFN 是 `'Source'`，家族名 `NotoSansSC` 不触碰它 |
| A-9 | **打字机 / 逐字揭示** | survey §9 收录 Soul of Waifu 的逐词参数（45 ms/字）与 Nexus 的按内容估时；oss §2.3：整句到达即整句上屏（与「一句一单元」对齐） | 采纳 **oss**：整句渐入。逐字会与「整句合成完成→开始播放」的时序抢跑 |
| A-10 | **状态动画节奏** | survey §9（Nexus）按状态换动画速度；flutter-arch §5.11 禁止持续/大面积昂贵效果 + reduced-motion | 折中：颜色/形状优先；**只允许一处**持续动画（`thinking` 呼吸光），且必须尊重 `disableAnimations` |
| A-11 | **圆角/间距是否进 `ThemeExtension`** | flutter-arch §1.1 记录了正反两方（「为统一而统一」vs 需要插值） | 采纳其**折中建议**：尺寸/时长用 `const`，颜色族/玻璃用 `ThemeExtension`。理由：dark-only 无主题动画，`lerp` 价值≈0 |

## 附录 B：后端接口真相速查（实现者抄这里，不要去猜）

| 方法 + 路径 | 响应要点 | 备注 |
|---|---|---|
| `GET /api/v1/settings` | `{llm:{base_url,model,has_api_key}, tts:{base_url,model,voice,has_api_key,sample_rate,channels}, persona:{system_prompt,max_history_pairs,name,description,personality,scenario,first}, dev_mode}` | **密钥只回布尔，永不回变量名** |
| `PATCH /api/v1/settings` | `{persisted, settings:<同上>, apply_status}` | 字段级三态：**省略=不改 / `null`=清空 / 值=设置**；段级 `null`=整段清空；段内 `clear_api_key:true`=清除密钥绑定 |
| `POST /api/v1/settings/test/llm` | `{ok, latency_ms?, model_echo?, error?}` | `ok=true` 时 `latency_ms`+`model_echo` 必有 |
| `POST /api/v1/settings/test/tts` | 同上（`voice` 回显） | |
| `POST /api/v1/chat` | `{accepted, epoch, pending_cleared}` | 400 `invalid_payload` / 429 `busy` / 503 `no_supervisor` |
| `POST /api/v1/chat/stop` | `{accepted:true}` | 幂等 |
| `GET /api/v1/app/status` | `{started_at, uptime_s, config_path, active_model_id, audio:{backend,available,sample_rate,channels}, llm:{...}, tts:{...}, dev_mode, current_epoch}` | `audio` 是**占位常量**，不要展示 |
| `GET /api/v1/app/capabilities` | `{app, version, schema_version, actions[6], action_sources[3], strength_levels[1,2,3], model_upload_supported, script_invoke_supported, runtime_ws, state_ws, ws_protocol_version}` | 按钮由它驱动显隐 |
| `GET /api/v1/commands` | `{commands:[{id,name,description,params_schema,allowed_sources,kind,target_action,step_count}]}` | `kind` 恒 `single` |
| `POST /api/v1/commands/{id}/invoke` | `{accepted, epoch, command_id, kind, steps, action}` | body `{"strength":1\|2\|3}`；恒 `user_command`（优先级 100） |
| `GET /api/v1/models` | `{models:[{id,display_name,version,layout,moc3_file,texture_files,has_physics,has_display_info,active,imported_at,size_bytes}]}` | |
| `POST /api/v1/models/import` | `{id, model3_json_path, applied}` | body `{"id":"<本地目录名>"}` |
| `GET /api/v1/models/{id}` | 单模型详情 | |
| `POST /api/v1/models/{id}/activate` | `{active_id, prev_active_id, requires_restart, model_url}` | |
| `PATCH /api/v1/models/{id}/display` | `model:{scale,offset_x,offset_y,rotation,fit_mode}` · `stage:{width,height,background_type,background_color,background_image,background_opacity}` | `deny_unknown_fields`——**键名写错会 400** |
| `DELETE /api/v1/models/{id}` | 激活中的模型 → **409 `model_active`** | |
| `GET /api/v1/mods` | `{mods:[{id,name,version,api_version,status,enabled}]}` | 前置在 dispatch 之前，**不在 `match_route`** |
| `POST /api/v1/mods/{id}/{enable\|disable\|restart\|config}` | `{ok, enabled}` / `{ok, restarted}` | `config` **必须** `{"config":{...}}`；半成功：「reload 成功但 restart 失败」 |
| `GET /api/v1/logs` | `{log_dir, file, lines[≤200], truncated}` | **dev_mode=false → 403** |
| `GET /api/v1/logs/levels` | `{levels:["trace","debug","info","warn","error"]}` | 同上 403 |
| 错误体（统一） | `{"error":{"code","message","details"}}` | `code` 是稳定契约字符串 |

**WS `/ws/state` 帧**（服务端包装为 `{type, seq, ts, data}`，**单向事件流**，客户端不发帧）：

| type | data |
|---|---|
| `subscribe_ack` | `{topics:[...]}` |
| `heartbeat` | `{}`（10 s） |
| `turn_state` | `{epoch, status:"completed"\|"failed"}` |
| `runtime_status` | `{event:"voice_started"\|"voice_ended"\|"new_epoch"\|"shutdown_ready", epoch?}` |
| `text_delta` | `{epoch, text}` 或 `{epoch, completed}` |
| `action_state` | `{epoch, kind:"perform", action:{action,strength,source}}` 或 `{epoch, kind:"cease"}` |
| `audio` | `{epoch, audio:<base64 s16le>, sample_rate, slice_ms:20, volume, start, end, muted}`（`muted=true` 时 PCM 全零但 `volume` 仍为真实 RMS） |
| `error` | `{code, message}` |

**渲染面 `/render` 协议 v1**（Flutter → iframe，`postMessage(jsonString, location.origin)`）：

| type | payload | Flutter 桥现状 |
|---|---|---|
| `sync` | `model? scale? dark? tier? lipSync? idleEnabled? mouthSensitivity? clickEnabled?` | ✅ 已实现（`clickEnabled` 待加） |
| `mouth` | `level` 0..1（≤30 Hz） | ✅ 已实现；wasm **优先读 `level`**（§0.4） |
| `stage` | `dark? scale? tier?` | ✅ 已实现 |
| `destroy` | `{}` | ✅ 已实现 |
| `action-state` | 根级 `action` + `payload:{state:"start"\|"end", strength}` | ❌ **待补**（§8.6） |
| `stage-zoom` | `{dir:"in"\|"out"\|"reset"}` | ❌ **待补** |
| `stage-bg` | `{dataUrl}`（空 = 清除） | ❌ 待补（dev） |

上行（iframe → Flutter）：`ready` / `loaded` / `progress` / `error` / `fps` / **`stage-ack`**
（`{applied, msg_type, scale, offset_x, offset_y}`，**无 `version`**，未收 ACK 前不得提示"已生效"；当前被 Flutter 忽略）。
