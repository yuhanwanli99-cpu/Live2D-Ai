# Flutter Web 应用架构与设计系统方案（2026-09）

> 状态：**设计结论**，不含实现代码。面向 `shell/flutter/` 的从零重写。
> 本文档只回答「怎么组织」，不回答「怎么敲」。所有结论都标注【已验证】/【推断】。
>
> 调研环境（实测，非假设）：
> - Flutter **3.47.3** stable（revision `e8113bf456`，2026-09-04）· Dart **3.13.3** · DevTools 2.60.0
>   ——`flutter --version` 实测；SDK 位于 `~/flutter`（默认不在 PATH）。
> - `shell/flutter/pubspec.yaml:7` 声明 `sdk: ^3.13.3`，与实测一致。
> - 运行时依赖仅 `http ^1.6.0` + `web ^1.1.1`（`pubspec.yaml:10-14`）。
> - 基线门禁：`flutter test` **38/38 通过**（本机实测，见 §4.1）。
>
> **注意（调研期间发生的变化）**：本报告写作过程中，`AGENTS.md` 与 `shell/flutter/lib/` 被并发修改
> （语音约定从「默认静音」改为「**默认出声**」，新增主音量与 `audio/gain.dart`）。
> §0/§4.1 的行号与计数已按**修改后**的代码重新核对（2026-09-10 19:20）。

---

## 零、结论速览

| # | 问题 | 结论（一句话） |
|---|---|---|
| 1 | 设计 token | `ColorScheme` 定语义色 + `ThemeExtension` 承载 Material 没有的（间距/圆角/动效/玻璃）+ **文本与尺寸常量收在唯一一个 `tokens.dart`**；用**扫描式单测**（零新依赖）守住「零裸色值/零裸字号」 |
| 2 | 信息架构 | **导航承载「7 分区设置」，聊天不是导航目的地而是常驻面板**；宽屏 `NavigationRail(extended)`、中屏 icon-only rail、窄屏 `NavigationBar` + 底部弹层；**舞台用 IndexedStack 常驻不销毁** |
| 3 | 状态管理 | 四类状态**四层归位**；`ChangeNotifier` + `InheritedNotifier` **够用**，不引入 provider/riverpod；给定 5 条明确触发条件 |
| 4 | 可测试性 | 照抄 `display_prefs.dart` / **`gain.dart`** 的「纯逻辑无 web 依赖 + 注入接口」范式；补上**断点/布局决策纯函数化**这一关键缺口；三层分工写清「测什么/不测什么」；基线 38 测试 |
| 5 | Web 性能 | **最致命的两个坑是当前构建真实存在的**：CanvasKit 从 gstatic CDN 加载、CJK 字体运行时从 gstatic 下载。离线/无外网场景会白屏 + 方块字 |
| 6 | 无障碍 | 内建组件已免费提供大部分；自建组件显式 `Semantics`（含滑杆的 `semanticFormatterCallback`）；**`prefers-reduced-motion` 在 Web 上引擎已自动接入**，无需手写 JS；另发现「默认出声 vs 键盘解锁」缺口（§9.1） |
| 7 | 组件清单 | Material 3 内建覆盖约 70%；自建集中在 token / 断点 / 舞台覆盖层 / 字段行 4 类 |

**本仓库当前状态（用于对照）**：`lib/` 共 14 个 Dart 文件、2525 行；`main.dart` 481 行塞了 AppShell + 聊天面板 + 4 个私有组件；无 `Semantics`、无 `FocusTraversalGroup`、无 `Shortcuts`、无 `prefers-*` 处理（全仓库 grep 零命中，见 §6）；依赖里无任何状态管理库（`pubspec.lock` 实测）。

---

## 一、设计 token 如何在 Flutter 里落地

### 1.1 三者的取舍：`ColorScheme` / `TextTheme` / `ThemeExtension`

| 载体 | 适合放什么 | 不适合放什么 | 本方案用法 |
|---|---|---|---|
| `ColorScheme` | **有语义的 Material 槽位色**（`primary` / `surfaceContainerHighest` / `error`…） | 玻璃叠色、舞台专用的半透明黑、任何非 Material 语义的色 | 用 `ColorScheme.fromSeed` 生成后**逐槽位覆写** |
| `TextTheme` | 文本**层级**（`titleMedium` / `bodySmall`…） | 字号本身作为 token 被引用；非文本尺寸 | 一次性配好 fontFamily/fallback，业务只读 `theme.textTheme.*` |
| `ThemeExtension<T>` | Material 没有的维度：**间距 / 圆角 / 阴影 / 动效时长与曲线 / 断点 / 玻璃参数 / 舞台叠色** | 任何能用 `colorScheme` 表达的东西（会双份真相） | **自建 token 扩展的唯一落点** |

依据：
- `ThemeData.extensions` 是 `Map<Object, ThemeExtension<dynamic>>`，提供 `theme.extension<T>()` 类型化读取（SDK 实测：`packages/flutter/lib/src/material/theme_data.dart:986` 字段、`:993` 方法、`:275` 构造参数）。
- `ThemeExtension` 是抽象基类，强制实现 `copyWith` 与 `lerp`（`theme_data.dart:131`、`:140`、`:145`）——**`lerp` 是它相对「随便塞个常量类」的唯一实质优势**：主题切换/主题动画时数值可插值。

> **反面意见（必须记录）**：间距和圆角本质上**不需要插值**，用 `ThemeExtension` 承载它们属于「为了统一而统一」。若要更省事，可把间距/圆角/时长定为 `tokens.dart` 里的 `const double`（编译期常量、可 `const` 构造、零运行时开销），只把**需要随主题动画/随明暗切换**的项（玻璃、舞台叠色、阴影）放进 `ThemeExtension`。
>
> **我的建议：折中。** 尺寸与时长放 `abstract final class Space/Radius/Motion/Duration` 常量；颜色族与玻璃放 `ThemeExtension`。理由：Dark-only 桌面应用（`main.dart:53-67` 只有 `brightness: Brightness.dark`，无亮色主题）**没有主题动画需求**，`lerp` 的价值近乎为零；而 `const` 常量能进 `const` 构造函数、消除一整个类的间接层。

**Dark-only 判断的依据**：`ColorScheme.fromSeed(seedColor: Color(0xFF7C5CFF), brightness: Brightness.dark)`（`main.dart:53-56`）+ 硬编码 `scaffoldBackgroundColor: Color(0xFF0B0D12)`（`main.dart:64`）。桌宠形态下亮色主题不是需求。→ 若将来要加亮色，才把尺寸也搬进 `ThemeExtension`。

### 1.2 「零裸色值、零裸字号」如何被静态守住

**不支持**自定义 lint 插件直达 `flutter analyze`：本仓库 `analysis_options.yaml:10` 只 `include: package:flutter_lints/flutter.yaml`，而 `custom_lint` / `analyzer_plugin` 在 `pubspec.lock` 中**零命中**（实测），引入它们等于新增依赖。`flutter build web` 也没有 `--web-renderer` 之外的 lint 钩子。

**结论：门禁是 `flutter analyze` + `flutter test`，所以把规则做成 `test/` 里的测试。** 这是唯一零新依赖、且自动进入既有门禁的方案。

**方案：`test/design_tokens_lint_test.dart`（源码扫描式）**

```
// 伪代码：遍历 lib/**.dart，先剥离注释与字符串，再匹配禁用模式
final offenders = <String>[];
for (final file in Directory('lib').listSync(recursive: true)) {
  if (!file.path.endsWith('.dart')) continue;
  if (file.path.endsWith('design/tokens.dart')) continue;   // 唯一豁免点
  final src = stripCommentsAndStrings(file.readAsStringSync());
  for (final rule in kForbiddenPatterns) {
    if (rule.pattern.hasMatch(src)) offenders.add('${file.path}: ${rule.name}');
  }
}
expect(offenders, isEmpty, reason: '裸值必须进 tokens.dart');
```

`kForbiddenPatterns` 草案：

| 规则名 | 模式 | 覆盖的现存违规（实测行号） |
|---|---|---|
| 裸色字面量 | `Color(0x` / `Colors.<name>` | `main.dart:54,64,258-262,302,401,420,425,460-461`；`live2d_stage.dart:158,205,245,247` |
| 裸字号 | `fontSize:` | `main.dart:269,312,428,470`；`display_panel.dart:83,104`；`live2d_stage.dart:221,260` |
| 裸圆角 | `BorderRadius.circular(` | `main.dart:421,462`；`live2d_stage.dart:206,246` |
| 裸间距 | `EdgeInsets.*(`、`SizedBox(` 的数值参 | `main.dart`(多处)、`display_panel.dart:31,43,...` |
| 裸时长 | `Duration(` | 仅允许 `design/tokens.dart` 与 `api/`、`audio/`（协议层） |
| 裸断点 | 数字与 `maxWidth` 比较 | `main.dart:208` 的 `>= 900` |

**已知边界（必须诚实记录）**：
- 文本扫描**不理解 AST**。剥注释/字符串后仍可能被「字符串拼接出的 `Color(`」等极端写法绕过，也会对 `style: someVar` 这类间接引用无能为力（那正是我们想要的间接引用，不误报即可）。
- 对 `EdgeInsets`/`SizedBox` 的规则**误报率高**（`EdgeInsets.zero` 合法、`SizedBox.shrink()` 合法），建议 v1 **只上「裸色值 + 裸字号 + 裸圆角」三条**，间距/时长靠 review + 后续收紧。
- **`Color(0x` 的豁免只有 `tokens.dart` 一处**——这是让规则可执行的关键：豁免面越小，规则越难被侵蚀。

> **替代方案（未采用）**：引入 `custom_lint` + 自写插件能拿到真实 AST、误报极低，但 (a) 新增 dev 依赖需过许可审查，(b) 必须挂在 `flutter analyze` 之外的命令上，要么改门禁、要么没人跑。**若将来门禁允许扩展命令，这是升级路径。**

### 1.3 Token 清单草案

**颜色**

保留现有种子色 `#7C5CFF` 与舞台底色 `#0B0D12`（`main.dart:54,64`）——它们是既定的产品视觉基因，不应在重写中丢失。

```
// 种子与关键覆写（写进 tokens.dart，业务代码永不直接引用）
seed                 #7C5CFF
stageBackdrop        #0B0D12   // scaffold 底 + 舞台底（合并现有两处硬编码：main.dart:64, live2d_stage.dart:158）
chatPanelSurface     #10131A   // main.dart:302
bubbleAssistant      #1B2030   // main.dart:461
dangerSurface        #3A1D1D   // main.dart:420,460；live2d_stage.dart:245
dangerBorder         #8A3B3B   // live2d_stage.dart:247
onStageMuted         white54   // main.dart:401 空态文案
wsIdle/Conn/Ok/Warn/Closed   // main.dart:258-262 的状态色族（现在直接用了 Colors.*）
```

原则：**M3 语义槽位交给 `ColorScheme.fromSeed`**（`primary/onSurface/surfaceContainerHighest/error/outline…`），只覆写上表这几个 Material 无法表达的。注意 `surfaceVariant` 已废弃，改用 `surfaceContainerHighest`（SDK 实测：`color_scheme.dart:198` 存在该参数、`:219` 标注 `'Use surfaceContainerHighest instead.'`）。

**玻璃/叠色**（`ThemeExtension`，因为要随明暗/主题插值）

| token | 值 | 用途 |
|---|---|---|
| `glassScrim` | `#000000` @ 0.60 | 舞台徽标底（现 `live2d_stage.dart:205`） |
| `glassBarrier` | `#000000` @ 0.32 | 弹层遮罩 |
| `hairline` | `onSurface` @ 0.12 | 1px 分隔线（替代阴影表达层级） |
| `hoverWash` | `onSurface` @ 0.06 | 悬停 |
| `focusRing` | `colorScheme.primary` @ 1.0 | 键盘焦点环（**必须不透明**，见 §6） |

**字号阶梯**（`TextTheme` 配好后，业务只读槽位；下表是「槽位 → 实测现值」的映射，用于验证不丢层级）

| 槽位 | size / weight | 现网对应 |
|---|---|---|
| `labelSmall` | 11 / w500 | 气泡角色名 `main.dart:470` |
| `bodySmall` | 12 / w400 | 徽标 `main.dart:269`、错误条 `main.dart:428` |
| `labelLarge` | 13 / w500 | 舞台错误文案 `live2d_stage.dart:260` |
| `bodyMedium` | 14 / w400 | 正文 |
| `titleMedium` | 16 / w600 | 面板标题 `display_panel.dart:41` |
| `titleLarge` | 16 / w700 | 「对话」`main.dart:312` |
| `headlineSmall` | 20 / w600 | 分区标题 |

> 现状真实问题：`main.dart:312` 用 16/w700，`display_panel.dart:41` 用 `titleMedium`(16/w600)——**同为 16 两个不同字重，这就是「没有阶梯」的症状**。重写时必须二选一。
>
> 另有两处 11px 的内联覆盖（`display_panel.dart:83,104`）直接在 `bodySmall` 上 `copyWith(fontSize: 11)`——**这是「阶梯不够用就就地加字号」的典型滑坡**，说明 11 这一级应当成为正式槽位（现映射到 `labelSmall`）。

**间距**（4px 基准网格）

| token | px | token | px |
|---|---|---|---|
| `space0` | 0 | `space6` | 24 |
| `space1` | 4 | `space7` | 32 |
| `space2` | 8 | `space8` | 40 |
| `space3` | 12 | `space9` | 48 |
| `space4` | 16 | `space10` | 64 |
| `space5` | 20 | | |

**圆角**

`radiusNone 0` · `radiusXs 4` · `radiusSm 6` · `radiusMd 8`（现 `main.dart:421`）· `radiusLg 10`（现 `main.dart:462`）· `radiusXl 12`（现 `live2d_stage.dart:246`）· `radiusPill 999`（现 `live2d_stage.dart:206`）。

**阴影**（dark-only 下刻意极少）

| token | 值 | 说明 |
|---|---|---|
| `shadowNone` | — | **默认**：dark 下用 1px `hairline` 边框表达层级 |
| `shadowCard` | `#000000` @ 0.45, blur 16, offset(0,4) | 仅浮层 |
| `shadowFocus` | — | **不用阴影**：焦点环用 `border` + `primary`，保证对比度可测 |

**动效时长与曲线**

| token | 值 | 用途 |
|---|---|---|
| `durState` | 120ms | hover/pressed |
| `durEnter` | 200ms | 内容切换（现 `web-ui-spec-v2.md` 用 200ms 的 rail 切换） |
| `durLg` | 320ms | 模态入场 |
| `durReveal` | 600ms | 启动揭示，**仅此一处长动画** |
| `easeOut` | `Curves.easeOutCubic` | 入场 |
| `easeInOut` | `Curves.easeInOutCubic` | 状态切换 |
| `easeEmphasized` | `Curves.easeOutCubic`（Dark-only 简化） | — |

---

## 二、信息架构与导航

### 2.1 先厘清一件事：聊天不是「导航目的地」

把「舞台 + 聊天 + 7 个设置分区」平铺成 9 个导航项是本设计最容易被做错的地方。

**产品判断（推断，但依据充分）**：形态是**桌面 AI 伴侣 / 桌宠**（`AGENTS.md` §项目背景），核心循环是「说话 ↔ 看它反应」（`AGENTS.md` §语音输出约定）。**聊天与舞台是同时被消费的**——用户一边看口型一边读文字。所以：

- **宽屏/中屏：聊天是常驻面板，不占导航项。** 现有代码已经是这个判断（`main.dart:226-231` 宽屏 `Row`：`Expanded(flex: 3)` 舞台 + 380px 聊天栏）。
- **窄屏：聊天才降级为可切换的目的地。**
- **舞台：任何尺寸、任何状态下都不销毁**（见 §2.5）。

于是导航真正要承载的是 **7 个设置分区**：

| # | 分区 | 后端接口 | 归属状态 |
|---|---|---|---|
| 1 | 角色与人设 | `persona` 段（`GET`+`PATCH /api/v1/settings`） | 远端 |
| 2 | LLM | `llm` 段 + `POST /api/v1/settings/test/llm` | 远端 |
| 3 | TTS / 语音 | `tts` 段 + `POST /api/v1/settings/test/tts` | 远端 |
| 4 | 模型库 | `/api/v1/models`（list / import / get / delete / activate / display） | 远端 |
| 5 | 外观与语音 | **无后端**，纯本地 `DisplayPrefs`（含静音/主音量） | 本地 |
| 6 | Mod | `/api/v1/mods`（list / enable / config） | 远端 |
| 7 | 诊断与日志 | `/api/v1/logs`、`/api/v1/logs/levels`、`/api/v1/commands` | 远端只读 |

> **【已验证】接口契约修正**：任务书说 `GET/PUT /api/v1/settings`，**实际是 `GET` + `PATCH`**。证据：`crates/live2d-ai-desktop/src/web_api/mod.rs:192-202` 的 `match_route` 对 `/api/v1/settings` 只分派 `SettingsGet`(GET) / `SettingsPatch`(PATCH)，其余 `NotFound`；settings 模块只导出 `handle_get`(`settings_routes/mod.rs:38`) 与 `handle_patch`(`:347`)，全目录 `Method::Put` **零命中**。前端必须用 `PATCH`。
>
> **另一个契约修正**：`app/status` 与 `app/capabilities` 不在这 7 个分区里，它们是**全局状态**（顶栏），不是设置项。

### 2.2 三种候选导航

**候选 A：左侧 `NavigationRail`（设置）+ 右侧常驻聊天**

```
┌──────┬──────────────────────────┬─────────┐
│ Rail │        Stage             │  Chat   │
│ 人设 │      (iframe /render)    │  常驻   │
│ LLM  │                          │  380px  │
│ TTS  │                          │         │
│ 模型 │                          │         │
│ 外观 │                          │         │
│ Mod  │                          │         │
│ 诊断 │                          │         │
└──────┴──────────────────────────┴─────────┘
```

**候选 B：顶栏 + 设置弹层（modal sheet / fullscreen dialog）**

```
┌────────────────────────────────────────────┐
│ TopBar: 标题   [状态]  [设置] [模型]        │
├──────────────────────────┬─────────────────┤
│        Stage             │      Chat       │
└──────────────────────────┴─────────────────┘
        ↑ 设置以弹层覆盖，不占常驻空间
```

**候选 C：全屏设置页 + 返回（设置成为独立 route）**

```
route '/'      → 舞台 + 聊天
route '/settings' → 全屏设置（舞台不可见）
```

### 2.3 三档断点下的对比

断点由**纯函数**决定，不写在 Widget 里（现状 `main.dart:208` 的 `constraints.maxWidth >= 900` 是反例）：

```
enum SizeClass { compact, medium, expanded }   // <900 / 900–1280 / ≥1280
SizeClass sizeClassOf(double width);           // 可单测
```

| 维度 | A 侧栏 Rail | B 顶栏 + 弹层 | C 独立设置页 |
|---|---|---|---|
| **≥1280（expanded）** | ✅ **最佳**：`extended: true` 带文字标签，分区平铺可达；舞台 + 聊天 + 设置三方同屏 | ⚠️ 每次切分区要开弹层，多一次点击 | ❌ 直接违约（舞台不可见） |
| **900–1280（medium）** | ✅ 好：icon-only rail（约 72–80px），hover tooltip 补标签；舞台仍宽 | 🔶 可用：左列表 + 右详情的两栏 sheet | ❌ 违约 |
| **<900（compact）** | ❌ 差：rail 挤占本就紧张的宽度；80px 在 390px 视口占 20% | ✅ **最佳**：`NavigationBar`（底部，最多 5 项）+ 底部弹层；舞台缩到上半屏仍可见 | ❌ 违约 |
| **舞台是否始终可见** | ✅ 始终可见（rail 不覆盖） | ✅ 始终可见（弹层可做非全屏 + 保留舞台顶部） | ❌ 违约 |
| **实现复杂度** | 中（rail 状态 + 选中分区 + 内容区路由） | 中高（弹层 + `StatefulBuilder` 双源刷新陷阱） | 低（但违约） |
| **`DisplayPrefs` 实时下发手感** | ✅ 最好：滑杆常驻可见，拖动能立刻看到舞台变化 | 🔶 弹层遮住部分舞台 | ❌ 最差 |

**现有实现的教训（【已验证】）**：`main.dart:150-169` 的宽屏路径**只实现了弹层**，且为了「滑杆拖动时舞台同步变化」被迫用 `showModalBottomSheet` + `StatefulBuilder`，并在注释里承认「弹层不随宿主 setState 重建」——**这是导航与状态耦合的味道**。A 方案用常驻 rail 天然消掉这个问题。

### 2.4 结论：主选 A，带 B 的降级

**推荐组合（不是三选一，而是按断点降级）：**

| 断点 | 设置导航 | 聊天 | 外观&口型 |
|---|---|---|---|
| **≥1280** | `NavigationRail(extended: true)` 常驻左栏，分区内容**内联**在 rail 右侧（第三栏） | 右侧常驻 380px | 内联分区（滑杆实时联动舞台） |
| **900–1280** | `NavigationRail(extended: false)` icon-only；选中分区以 `showModalBottomSheet(constraints: maxWidth ≈ 560)` 覆盖，保留舞台可见 | 右侧常驻 320–380px | 弹层 |
| **<900** | `NavigationBar`（底部 4–5 项）+ `showModalBottomSheet(isScrollControlled: true, constraints: maxWidth 520)` | 全屏切换（舞台在其后保留） | 弹层 |

**为什么 ≥1280 要「内联」而不是弹层**：`DisplayPrefs` 的价值在于**实时看舞台反应**（`display_panel.dart:11-12` 注释明确「滑杆实时下发，换来所见即所得」）。内联面板让滑杆与舞台同屏，是这个产品唯一必须保住的手感。

**为什么 <900 用底部 `NavigationBar` 而不是 `NavigationRail`**：Material 3 的规范取向——`NavigationBar` 用于 compact、`NavigationRail` 用于 medium/expanded。SDK 内两者都在（实测 `material/navigation_bar.dart` 与 `material/navigation_rail.dart` 均存在；`NavigationRail` 提供 `extended` / `labelType` / `minWidth` 参数，`:94,:102,:107`）。

**窄屏「舞台必须一直可见」的诚实回答（推断）**：在 390px 视口 + 底部弹层打开时，舞台**必然被大部分遮住**。可行的最强保底是：
- 弹层用 `isScrollControlled: true` + `constraints: maxWidth 520` + **限制高度**（如 `maxHeight = 0.72 × 视口`），让舞台顶部露出一条；
- 或弹层半透明（`glassScrim`），舞台仍隐约可见。

**但严格意义上「完全不被遮挡」在 compact 下做不到。** 这需要产品裁决（见 §9 决策点 D2）。

### 2.5 关键实现约束：舞台必须常驻不重建

`Live2DStage` 内部持有 `Live2DBridge`（`live2d_stage.dart:44`），iframe 由 `ValueKey<int>(_generation)` 标识（`:160`），**Key 一变或 Widget 被卸载 → iframe 重建 → 模型重新加载**（`retry()` 的语义就是重建，`:140-149`）。

因此：
- 分区切换、聊天/设置切换**绝不能让 `Live2DStage` 离开 Widget 树**；
- 用 `IndexedStack`（或 `Offstage`）承载「舞台 / 聊天」两态，而不是条件表达式；
- 弹层优先 `showModalBottomSheet` / `showDialog`（overlay 不卸载下层），而非替换 body。

**代价**：`IndexedStack` 中非选中子项仍会被布局与**可能的**绘制（`IndexedStack` 只绘制选中项，但保留状态）。iframe 平台视图在 overlay 覆盖下将持续运行——对 Live2D 而言这是**期望行为**（口型动画不该停）。**副作用**：弹层打开时舞台逻辑仍在跑，需注意 CPU（见 §5 毛玻璃条目）。

---

## 三、状态管理

### 3.1 四类状态的归位

| # | 状态 | 真相来源 | 生命周期 | 归属层 | 现状对照 |
|---|---|---|---|---|---|
| 1 | **服务端设置** | `GET /api/v1/settings` | 跨会话（落盘 `live2d-ai.toml`） | `SettingsController extends ChangeNotifier` | **现状：完全缺失**（前端只有 `fetchStatus`，`api_client.dart:90`） |
| 2 | **本地偏好** | `localStorage` | 跨会话（浏览器本地） | `DisplayPrefs`（**纯值**，已有）+ 一个 `LocalPrefsController` | ✅ `settings/display_prefs.dart` 已是正确范式 || 3 | **WS 实时事件** | `/ws/state` 单向流 | 连接期 | `WsClient`（**已经是** `Stream` 出口） | ✅ `api/ws_client.dart:168-169` |
| 4 | **聊天流** | 事件 1–3 的**投影** | 会话内（内存） | `ChatController extends ChangeNotifier` | ✅ `chat/chat_controller.dart:31` |

**核心判断：这四类不该共用一个 store。** 它们的失效节奏完全不同——设置改动是分钟级、聊天 delta 是几十毫秒级。混在一个 `ChangeNotifier` 里会让每来一个 `text_delta` 都重建设置面板。

**现状已有的好设计（应保留）**：
- `WsClient` 暴露 `Stream<WsEvent>` 与 `Stream<WsStatus>`，**自己是命令式对象、不继承 ChangeNotifier**（`ws_client.dart:153-170`）；
- `ChatController` 把 JSON 帧翻译成领域事件（sealed class `WsEvent`，`ws_client.dart:12`），`UnknownWsEvent` 保证前向兼容（`:127`）；
- **`DisplayPrefs` 把「用户意图」与「设备实际输出」正交拆开**：`muted`（意图）与 `volume`（主音量）互不影响，二者又都与口型正交（`display_prefs.dart:42-47`）——这个「正交量各自独立、**不互相重置**」的性质是正确设计的范例，重写时应推广到其他设置（例如「口型灵敏度」与「口型开关」已是同样关系，`display_prefs.dart:29-30`）。

### 3.2 `ChangeNotifier` / `ValueNotifier` + `InheritedNotifier` 够不够？

**结论：够。当前不需要 provider/riverpod。** 但有三个必须遵守的约束，否则会在 6 个月后失控。

**为什么够：**
1. `ChangeNotifier` 是 `flutter/foundation` 的一部分（`chat_controller.dart:3` 已 import），零依赖；
2. `InheritedNotifier` 同样是纯 framework——它监听一个 `Listenable`，在通知时重建依赖它的子树。四类状态对应 4 个子树即可；
3. **可测试性反而更好**：`ChangeNotifier` 可以在 `flutter test` 里直接 `new` + `addListener` 断言，不需要 provider 的 `ProviderContainer` 样板。现有 `chat_controller.dart` 与 `live2d_bridge.dart` 都已是这个形态，测试已在跑。

**三个必守约束：**
1. **`InheritedNotifier` 的作用域要窄。** 一个覆盖整个 `MaterialApp` 的 `InheritedNotifier<SettingsController>` 会让每次设置变更重建全树。做法：每个控制器一个 `InheritedNotifier`，且**挂在真正消费它的子树根部**。
2. **高频状态不进 `InheritedNotifier`。** 口型电平（30Hz）**绝不能**走 rebuild 路径。现有实现已经对了：`main.dart:104-106` 用 `GlobalKey<Live2DStageState>`（`:80` 声明）直接调 `setMouth(level)`——**绕开 Widget 树**。这个模式必须保留并写进规范。
3. **派生状态显式缓存，不要每次 build 重算。** 例如「未保存改动」标记 = `remoteSettings != draftSettings`；在 `build` 里算没问题（廉价），但不要在里面做 JSON 深比较 + 网络请求。

**何时值得引入 provider / riverpod（触发条件，非默认）**：

| # | 触发条件 | 为什么 `ChangeNotifier` 会撑不住 |
|---|---|---|
| T1 | 需要**细粒度重建**：设置面板有 30+ 字段，改一个开关不想重建整页 | `ChangeNotifier` 只有「全量通知」一个粒度；必须靠 `Selector`/`ValueListenableBuilder` 手工拆，代码噪音大 |
| T2 | **跨控制器派生**：`ChatController` 需要读 `SettingsController.persona.name` 并自动失效 | 手工在构造里 `addListener` 会形成网格，容易出现循环通知 |
| T3 | 出现**异步资源**语义（loading / error / retry / 取消）超过 3 处 | 每个都要手写 `_loading/_error/_load()` 三件套。Riverpod 的 `AsyncValue` 直接消除 |
| T4 | 需要**测试替换整棵依赖树**（如 repo 层全 mock） | `InheritedNotifier` 需要手写 `updateShouldNotify` + 测试包装 Widget，样板随时间增长 |
| T5 | 控制器数量 **> 8** 且共享生命周期开始互相引用 | 手工 dispose 顺序变成 bug 源 |

**若触发，选型建议（推断）**：`provider`（包 `provider`）比 `riverpod` 更贴合现状——它是 `InheritedWidget` 的官方推荐演进，迁移是渐进的（先包 `ChangeNotifierProvider` 不动逻辑）。`riverpod` 胜在编译期安全与 `AsyncValue`，但引入新的心智模型（`ref`、`Notifier` 基类）与代码生成选项，对一个「只做展示与调度」的前端偏重。两者都是 MIT，与 AGPL-3.0-only 兼容（【推断】，基于常见 MIT 许可文本；**正式引入前须走许可审查**，参照 `docs/research/license-report.md` 的既有流程）。

> **治理提醒**：`AGENTS.md` 规定前端只做展示与调度，**不得复制核心逻辑**（状态机/动作仲裁/LLM/TTS 协议）。`ChatController` 现在承担的「epoch 变化 → 打断音频」（`chat_controller.dart:139-142`）**不是**核心逻辑复制，而是**展示层的调度决策**（什么时候停声音），边界是清楚的。重写时不要把它搬进新控制器之外的地方，也不要让它去判断「该不该出新 epoch」——那是 Rust 的事。

### 3.3 设置的双源问题（最容易出错的地方）

`GET /api/v1/settings` 返回的是**服务端真相**，但用户在面板里正在编辑的是**草稿**。这两个必须分开：

```
class SettingsController extends ChangeNotifier {
  Settings? _remote;      // 上次 GET/PATCH 成功后的服务端真相
  SettingsDraft? _draft;  // 用户正在编辑的副本
  bool get dirty => _draft != null && _draft != SettingsDraft.from(_remote);
  // patch 成功后：_remote = 响应体; _draft = null（或重置为响应体）
}
```

**「未保存改动」必须有可见提示**——现有 `docs/design/web-ui-spec-v2.md:156` 已经把「未保存检测」列为任务，说明这是已知需求。**离开分区 / 关闭弹层 / 刷新页面（`beforeunload`）三个时机都要拦。**

**密钥安全（`AGENTS.md` §密钥安全 硬约束）**：设置视图只给 `has_api_key: bool`，**永远不返回变量名**（`crates/live2d-ai-runtime/src/settings/view.rs:15-19` 的文档注释明确）。前端因此**只能**：
- 显示「已配置 / 未配置」布尔状态；
- 提供「设置新 key」的**写入路径**（PATCH 一个值）与「清除」（PATCH null）。
- **绝不**回显、缓存到 localStorage、写进日志、或放进 `debugPrint`。

---

## 四、可测试性

### 4.1 现有范式与门禁基线

`display_prefs.dart` 是**可单测的范例**（任务书语），其价值在三条明确的纪律：

1. **文件级 `library;` + 头注写明「纯逻辑、无 web 依赖，可在 VM 上单测」**（`display_prefs.dart:1-6`）；
2. **持久化被推到边界外**：文件本身不 import `package:web`，`localStorage` 读写留在 `main.dart`（`main.dart:19-41`），因为「那里才允许 `package:web`」（`display_prefs.dart:5`）；
3. **反序列化永不抛异常**，且有测试逐条钉死（`display_prefs_test.dart`：null / 类型不符 / 非有限数 / 越界 / 整数形式）。

**同一范式已被成功复制到 `audio/gain.dart`**（调研期间新增，36 行）：把「滑杆位置 → WebAudio 线性增益」这条算术从 `audio_player.dart` 里抽出来，头注理由与 `display_prefs.dart` 完全相同——「这是一段会被测试反复钉住的算术，不该埋在 `package:web` 的导入后面（那样只能在浏览器里跑）」（`gain.dart:3-4`）。**这就是本报告 §4.2 要求的 L1 层的活样本**：凡是算术、凡是映射、凡是判定，都该长成这个样子。重写时应把「断点判定」「脏值比较」「patch 构造」照此办理。

**实测基线：38 个测试全绿。** 内部覆盖：
- `audio_schedule_test.dart` —— `scheduleSlice` 时间轴不回退、`LevelTimeline` 的 push/drain/clear（`schedule.dart:80-99`）；
- `audio_gain_test.dart`（新增）—— `gainForVolume` 的边界与非有限值兜底（`gain.dart:29-36`）；
- `live2d_bridge_test.dart` —— `Live2DBridge` 的 ready 前入队/ready 后 flush（`live2d_bridge.dart:169-201`）、容忍无 `version` 的 `stage-ack`（`:249-251`）、error 帧进 error 态（`:254-261`）；
- `display_prefs_test.dart` —— 见上。其中一条**特别值得作为重写的模板**：`'旧版本存档（没有 volume/muted 字段）按新默认读'`（`display_prefs_test.dart:119-128`）钉死了「新增字段的缺省必须回落到**新默认**，而不是继承旧语义」——因为 `muted` 的默认刚在 2026-09-10 从 `true` 翻成 `false`（`display_prefs.dart:15,37-39`），老存档里的 `muted=true` 是**旧默认**，不该被继承。**每一次「默认值语义变更」都必须有这种回归，否则老用户升级后会静默变成哑巴。**

### 4.2 分层建议

| 层 | 位置 | 依赖 | 测什么 | **不**测什么 |
|---|---|---|---|---|
| **L1 纯逻辑** | `lib/settings/`、`lib/design/`、`lib/audio/schedule.dart`、`lib/audio/gain.dart` | **零**（禁 `package:web`、禁 `package:flutter/material.dart`） | 解析/序列化、clamp、**断点判定**、布局模式决策、patch 构造、脏值比较、增益/时间线算术 | 不测 UI、不测网络、不测 Material 组件 |
| **L2 控制器** | `lib/settings/*_controller.dart`、`lib/chat/chat_controller.dart` | `flutter/foundation` + **注入的接口** | 事件→状态投影、错误路径、dispose 无泄漏、并发/乱序事件、**每类 WS 帧各一条** | 不测真实 HTTP/WS、不测像素 |
| **L3 Widget** | `lib/ui/**` | `flutter_test` | 用户可见契约：文案、可达性、断点切换后**出现/消失**、空态/错误态、**Semantics 标签**、滑杆的 `semanticFormatterCallback` | 不测像素偏移、不测 Material 内部实现 |

> **L1 的判定标准（从现有两个范例反推）**：一个文件属于 L1，当且仅当它**不 import `package:web`、不 import `package:flutter/material.dart`**，且内部只有纯函数/不可变值。`display_prefs.dart`、`schedule.dart`、`gain.dart` 都满足（三者的头注都显式声明了「纯逻辑、无 web 依赖，可在 VM 上单测」）。**这个判定标准可以直接写成一条测试**：扫描这些目录，断言不含被禁 import。

**L2 的关键手法：把网络抽象成接口，而不是 mock `http.Client`。**

```
abstract interface class SettingsApi {
  Future<Settings> fetch();
  Future<Settings> patch(SettingsPatch patch);
}
abstract interface class LocalStore {
  String? read(String key);
  void write(String key, String value);
}
```

Fake 实现（在 `test/` 里）比 `MockClient` 更能表达「失败模式」：超时、409、`error.code` 未知、返回体缺字段。`ApiClient`（`api_client.dart:37`）目前是**具体类、构造里 `new http.Client()`**（`:42`）——重写时应让它**实现 `SettingsApi` 接口**，或至少让 `_client` 可注入。**这是当前对可测试性最大的单点阻碍。**

**L1 缺口（必须补）**：断点逻辑现在藏在 Widget 的 `LayoutBuilder` 里（`main.dart:206-208`），**完全不可单测**。抽成 `SizeClass sizeClassOf(double width)` 后，可以得到「1280/1279/900/899/390 各返回什么」的表驱动测试——这是本次重构在可测试性上最大的净收益。

### 4.3 Widget 测试的具体做法与边界

**断点测试不需要真窗口**（`flutter_test` 标准做法）：

```
tester.view.physicalSize = const Size(double.infinity, ...);   // 或具体值
tester.view.devicePixelRatio = 1.0;
addTearDown(tester.view.reset);
await tester.pumpWidget(const Live2DShellApp());
expect(find.byType(NavigationRail), findsOneWidget);           // ≥1280
expect(find.byType(NavigationBar), findsNothing);
```

**三个必须提前知道的坑：**

| 坑 | 原因 | 应对 |
|---|---|---|
| **`Live2DStage` 在测试里跑不起来** | `live2d_host_stub.dart` 是非 web 平台占位（`live2d_host_stub.dart` 全文 36 行）；`HtmlElementView` 在 VM 测试里无实现 | 在测试中把 stage 替换为**可注入的占位**（构造参数或环境覆盖），或只测不含 stage 的子树。**不要**为了测试给 stage 加分支判断 |
| **`Image.network` / iframe 不发真实请求** | `flutter_test` 的 `HttpClient` 被替换为永远返回 400 的桩 | 资源类组件用 `mockNetworkImagesFor` 式包装，或让路径可注入 |
| **`pumpAndSettle` 在无限动画上死锁** | `CircularProgressIndicator`（`main.dart:319`、`live2d_stage.dart:217`）永不 settle | 用 `pump(Duration(...))` 推进固定时长，而非 `pumpAndSettle` |

**L3 不该测的**：Material 组件的内部行为（那是框架的测试）、精确像素与颜色值（改 token 就会碎一地——**这正说明它不该被测试**，token 的守卫交给 §1.2 的扫描测试）。

---

## 五、Flutter Web 的性能与坑

> 本节前两条是**当前构建真实存在、会直接导致白屏/方块字的问题**，已在本机实测确认，优先级高于其他所有性能话题。

### 5.1 【已验证·最高优先级】CanvasKit 从外部 CDN 加载

**实测证据**（本机 `shell/flutter/build/web/` 产物）：

```
flutter_bootstrap.js:  canvasKitBaseUrl?n.canvasKitBaseUrl:e.engineRevision&&!e.useLocalCanvasKit
                       ?W("https://www.gstatic.com/flutter-canvaskit
main.dart.js:          https://www.gstatic.com/flutter-canvaskit/06a2e2a110089dff50fe635cffd2a61e1b24fbcd/
```

**成因**（SDK 源码实测）：`packages/flutter_tools/lib/src/build_system/targets/web.dart:129-139` —— 当 `kUseLocalCanvasKitFlag != 'true'` 且未显式给 `FLUTTER_WEB_CANVASKIT_URL` 时，构建**硬编码** CDN URL：

```dart
'FLUTTER_WEB_CANVASKIT_URL=https://www.gstatic.com/flutter-canvaskit/${globals.flutterVersion.engineRevision}/'
```

**后果**：本项目是 **loopback-only 桌面应用**（`AGENTS.md`、`shell/README.md` 均强调 loopback）。在**无外网**环境下：CanvasKit 的 `canvaskit.wasm` 拉不下来 → **整个 Flutter 应用白屏**（不是降级，是不渲染）。而 `build/web/canvaskit/` 里那份 37MB 的本地副本（含 `canvaskit.wasm` 7.0MB）**根本没被引用**。

**解法（两选一）：**
1. `flutter build web --no-web-resources-cdn`；或
2. `--dart-define=FLUTTER_WEB_CANVASKIT_URL=/app/canvaskit/`（`canvasKitUrlAlreadySet` 分支会跳过 CDN 注入，见 `web.dart:132-135`）。

**建议 (2)**：显式写 `/app/canvaskit/` 与现有 `--base-href /app/` 一致，意图更清楚、更抗工具链默认值漂移。**并在 `shell/README.md` 的构建命令里固定下来**，否则下次有人照文档敲一遍又回到 CDN。

### 5.2 【已验证·最高优先级】中文字体运行时从外部下载

**实测证据**：
- `build/web/assets/fonts/` 里**只有** `MaterialIcons-Regular.otf`，**没有任何正文/中文字体**；
- 引擎源码 `packages/flutter/lib/.../engine/configuration.dart:365-368`：
  ```dart
  /// Returns the base URL to load fallback fonts from. Fallback fonts are
  /// downloaded automatically when there is no font bundled with the app that
  /// can show a glyph that is being rendered.
  /// Defaults to 'https://fonts.gstatic.com/s/'.
  ```
- `main.dart.js` 里确实烘焙了 `https://fonts.gstatic.com/s/`；
- 回退表里含 `Noto Sans HK/JP/SC/TC` 分片（`engine/.../font_fallback_data.dart:119+` 起）。

**后果**：界面是**纯中文**，而 **CanvasKit 不读系统字体**——每个中文码点都要向 `fonts.gstatic.com` 请求 Noto 分片。无外网时：
- 首帧后文字**不显示**（豆腐块/空白）；
- 更糟：`font_fallback_service.dart:56-58` 有 `_maxRetries = 3`；`main.dart.js`（:`28195`）内有阈值逻辑——**累计 10 次全局失败且零成功就「Disable service for this session」**。即一次会话内字体永久不可用，刷新才重置。

**解法**：
- 在 `pubspec.yaml` 的 `flutter: fonts:` 里**打包一份中文字体**。建议 **Noto Sans SC**（OFL-1.1，与 AGPL-3.0-only 兼容）——但**必须做子集化**：全量 CJK 字体 8–16MB，会显著撑大首屏。
- 用 `fontFamilyFallback: ['Noto Sans SC', ...]` 配置到 `ThemeData.textTheme`，保证中文优先命中本地字体，**不再触发回退服务**。
- 子集化工具与产物体积需实测（见 §9 决策点 D3）。

### 5.3 首屏体积

| 文件 | 实测（未压缩） | 说明 |
|---|---|---|
| `main.dart.js` | **2.5 MB** | Dart→JS 产物（release） |
| `canvaskit.wasm` | **7.0 MB** | 引擎，单独协商压缩后显著减小 |
| `canvaskit.js` | 88 KB | |
| `assets/` | 1.4 MB | 主要是 `MaterialIcons-Regular.otf` |
| **`build/web` 总计** | **41 MB** | 含 `skwasm*` / `chromium` / `wimp` / `*.symbols` 等**当前构建用不到的副本** |

**结论**：真正的首屏关键路径是 `main.dart.js` + `canvaskit.wasm`（约 9.5MB 未压缩）。这对**同源 loopback** 是可接受的（本地磁盘/回环，无网络延迟），对远程部署则不可接受。

**优化项**：
- release 下 `--no-source-maps`（默认已关，确认即可）；
- 构建后用 `xtask` 风格的清理剔除 `canvaskit/*.symbols`、`chromium/`、`skwasm_heavy.*` 等未引用副本——**但先确认引擎确实不请求它们**，不要盲目删；
- **不要**为减体积引入 tree-shaking 之外的方案。
- `--tree-shake-icons` 默认已开（`flutter build web --help` 实测 `defaults to on`），保留。

### 5.4 Renderer 取舍

**实测（本机 Flutter 3.47.3）：`flutter build web` 已没有 `--web-renderer` 这个 flag**（完整 help 输出中不存在）。SDK 里 `WebRendererMode` 枚举**只剩两个值**：

```
packages/flutter_tools/lib/src/web/compile.dart:186-191
enum WebRendererMode { canvaskit,  // Always uses canvaskit.
                       skwasm; }   // Always use skwasm.
```

**含义**：
- **HTML renderer 已不存在**（`--web-renderer` flag 一并移除）。所以「CanvasKit vs HTML renderer 取舍」这个问题在本版本上**已经不存在**——凡问「HTML renderer 是不是更省」的讨论都已过期。
- 剩下的是 **JS+CanvasKit（默认）vs `--wasm`+skwasm**。
- `defaultForJs = canvaskit`、`defaultForWasm = skwasm`（`compile.dart:208-209`）。

**结论：本阶段用默认的 JS + CanvasKit。** 理由：
1. skwasm 依赖 WASM GC，浏览器支持面窄于 JS 路径；桌宠场景不保证浏览器版本；
2. **关键功能不受影响**：Live2D 渲染在 iframe 里由 Rust/wasm 负责，**不占 Flutter 渲染管线**，所以 CanvasKit 的 2D 性能差异对本应用影响远小于典型 Flutter Web 应用；
3. 产物里同时含 skwasm 副本（实测 `canvaskit/skwasm.wasm` 3.5MB）说明工具链在预置，将来切换成本低。

**保持可切换**：构建命令写在一处，`--wasm` 随时可试，不需要改代码。

### 5.5 字体加载（除 5.2 外）

- `useMaterial3: true` + 默认字体策略下，**图标字体**已在产物里（`MaterialIcons-Regular.otf`），`--tree-shake-icons` 已裁到实际用到的字形；
- **不要在运行时 `loadFontFromList`** 加载大字体——会与回退服务竞争，且难清理。

### 5.6 `Image` 解码

- `Image.network` 支持 `cacheWidth` / `cacheHeight` 指定解码尺寸（SDK 实测 `widgets/image.dart:468,472`，文档注释 `:314,321,402`）。**但 `:408` 明确**：**Web 平台上 `cacheWidth`/`cacheHeight` 的处理与原生不同**（走无 CORS 的 `<img>` 路径），不能假定它一定生效。
- **用户导入的模型贴图是本应用最可能的大图来源**（`/api/v1/models/import`）。推论：**在 Rust 侧生成缩略图/多级尺寸**，前端只取合适尺寸——这既符合「前端只做展示」的治理约束，也绕开了 Web 端解码尺寸不可控的问题。
- 用 `precacheImage`（`widgets/image.dart:121`）在展示前预解码，避免滚动中掉帧。
- `PaintingBinding.instance.imageCache.maximumSizeBytes` 可调（`painting/image_cache.dart:138,147`），默认约 100MB 量级——对只有少量贴图的应用无需调整，**不要盲目调大**（Web 内存比原生紧张）。

### 5.7 长列表

- 聊天列表**必须**用 `ListView.builder`（现状已正确：`main.dart:332`），带 `itemExtent`/`prototypeItem` 更佳；
- **禁止 `shrinkWrap: true` + `NeverScrollableScrollPhysics`** 的组合（实测常见误用，会一次性布局全部子项）；
- 消息在内存里要**设上限**（如 500 条滚动丢弃）。现状 `ChatController.messages` 是无界 `List`（`chat_controller.dart:48`）且**不持久化**——重写时应明确「聊天历史是否落 localStorage」的策略（见 §9 决策点 D5）。长会话下无界列表 + 每帧全量 rebuild 是真实风险。

### 5.8 `BackdropFilter` 毛玻璃的真实成本

**结论：这是本项目最容易「为了好看而毁掉性能」的地方。**

- `BackdropFilter` 是对**背后已绘制的像素**做高斯模糊（SDK `widgets/basic.dart:636`）。在 CanvasKit 下，每个实例通常需要 `saveLayer` 级别的离屏渲染；多个叠加或大面积使用会显著抬高每帧 GPU 成本。
- **与 iframe 平台视图的交互是额外风险**：舞台是 `<iframe>` 平台视图。毛玻璃需要读回其下的合成结果——Web 上这种跨合成层/跨平台视图的模糊尤其容易出问题（撕裂、模糊内容错位、强制同步）。
- **建议**：
  - 每屏**最多一个** `BackdropFilter`；
  - 用 `RepaintBoundary` 把它与滚动/动画内容隔离，避免每帧重复模糊；
  - **禁止**在 `ListView` 的 item 内使用（每个 item 一个 blur = 灾难）；
  - 提供**「降低特效」开关**（复用本地偏好机制），默认对低端设备关闭。

> 现有设计稿 `docs/design/web-ui-spec-v2.md:78` 明确要求 `--glass` + `backdrop-blur(8px)`。**这条直接搬到 Flutter Web 是有风险的**，需要产品确认（§9 决策点 D4）。

### 5.9 动画 jank

- **架构上的好消息**：口型（最高频信号，30Hz）**不经过 Flutter 的重建/绘制**。它走 `audio.levels` → `GlobalKey` → `Live2DBridge.sendMouth` → postMessage → iframe（`main.dart:104-106`、`live2d_bridge.dart:123-141`，且桥内已节流 ≤30Hz，`:40`）。**这是本设计最值得保护的性质**：Live2D 动画完全不占用 Flutter 帧预算。
  - 该性质在「静音」路径上也有额外价值：静音走 `GainNode` 增益置 0，而**口型仍由服务端 `volume` 驱动**（`gain.dart:9-11` 说明「音频图必须保持活着，否则 `LevelTimeline` 的到期释放会漂移」）。**不要**为了省事在静音时断开音频图或停掉口型——那会同时破坏同步与手感。
- **因此第一条纪律**：高频信号**永不**进入 Widget 树。不要为了「统一状态管理」把口型电平塞进 `ChangeNotifier` 再 rebuild。
- `RepaintBoundary` 包住**舞台**与**聊天列表**，避免局部重绘波及全屏（`live2d_stage.dart:158` 的 `ColoredBox` 底色在有 iframe 平台视图时尤其值得隔离）。
- `AnimatedBuilder` / `ValueListenableBuilder` 的 scope 要尽可能小。
- 用 `--profile` 构建 + 性能 overlay 实测帧时间；`flutter run --profile` 在 Web 上同样可用。
- 列表入场动画**避免逐项 `AnimatedOpacity`**（见 §5.11）。

### 5.10 Service Worker 与旧产物

**实测结论：Flutter 3.47 生成的 `flutter_service_worker.js` 是「自注销」的**，且会强制刷新所有客户端：

```js
// shell/flutter/build/web/flutter_service_worker.js（全文）
self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    await self.registration.unregister();          // 注销自己
    const clients = await self.clients.matchAll({ type: 'window' });
    clients.forEach((client) => {
      if (client.url && 'navigate' in client) client.navigate(client.url);  // 强制刷新
    });
  })());
});
```

**含义**：
- **「Service Worker 缓存导致旧产物」这个 Flutter Web 经典坑，在当前版本已被框架自行处理**——SW 不会长期驻留，页面会自动刷新到新产物；
- **但有两个前提必须保住**：
  1. `build/web/version.json` 能取到（它存在，实测内容为 `{"app_name":"live2d_ai_shell","version":"1.0.0","build_number":"1",...}`）；
  2. **服务端不能把 `flutter_service_worker.js` / `version.json` 长缓存**。好消息：Rust 侧已对所有 `/app/*` 响应加 `Cache-Control: no-cache`（`shell/README.md` §3 明确）；
- **风险点（推断）**：既然 SW 会在 activate 时强制 `client.navigate()`，**如果用户正处在一轮语音对话中，前端重建会中断播放**。这在开发期很烦，生产期影响小（只在产物更新时触发一次）。

### 5.11 应避免的「漂亮但昂贵」效果

| 效果 | 为什么在 Flutter Web 上代价高 | 替代 |
|---|---|---|
| 全屏/大面积毛玻璃 | §5.8：`saveLayer` + 平台视图合成 | 半透明纯色 + 1px `hairline` |
| 大面积模糊阴影卡片 | dark 下每张卡片一次 blur；Web GPU 路径昂贵 | `hairline` 边框表达层级（`shadowNone` 为默认） |
| 列表逐项入场动画（stagger） | 需为每项维护独立 `AnimationController` 或一个驱动的 `Interval`；N 项 = N 次重绘 | 只做容器级淡入 |
| 自定义 fragment shader / 粒子 | `.frag` 在 Web 上要编译+传输；与 iframe 平台视图争 GPU | 不做；视觉表现交给 Live2D 本身 |
| 长时间运行的 shimmer 骨架屏 | 持续重绘（且**无视 `disableAnimations`**，见 §6） | 静态占位 + 单个 `CircularProgressIndicator` |
| 逐字符「打字机」拆分 `TextSpan` | 每个字符一个 span 会显著增加文本布局成本，且破坏无障碍（读屏逐字念） | 流式 `Text` 直接追加；`Semantics(liveRegion: true)` 节流播报 |
| 滚动区域内放 iframe/WebGL + Flutter 内容混排 | 平台视图与 Flutter 合成层在 Web 上存在问题（见 `docs/research/flutter-live2d-implementations-2026-09.md` 引用的 #101580 / #145360 / #175119） | 舞台固定不滚动（本设计已是如此） |

**额外提醒**：`docs/design/web-ui-spec-v2.md:132-133` 里「设置模态开 scale .96→1 + fade 320ms」和「rail 切换 fade+translateX 200ms」**可以做**——短时长、小面积、一次性。真正要砍的是**持续**和**大面积**的。

---

## 六、无障碍与键盘

### 6.1 现状：零覆盖

**实测：全仓库 `grep -rn "Semantics\|FocusTraversal\|Shortcuts\|Actions(" shell/flutter/lib/` 零命中；`grep -rn "prefers-reduced-motion\|matchMedia"` 零命中。** 无障碍与键盘支持目前**完全不存在**。

### 6.2 `Semantics`

**大部分是免费的**——Material 组件自带语义：`IconButton` 有 `tooltip` 会变成语义标签（`main.dart:183-185` 设置按钮、`:430` 关闭按钮已用），`SwitchListTile` / `ListTile` / `TextField` 都自带。

**自建的 `Slider` 行需要额外注意**：`display_panel.dart` 有三处 `_SliderRow`（模型缩放 `:53`、口型灵敏度 `:64`、**主音量 `:87`**），滑杆**没有可读的当前值语义**——视觉上靠右侧一个 `Text` 显示数值（`:93` 显示百分比），但读屏用户拖滑杆时听不到数值变化。必须给 `Slider` 补 [`semanticFormatterCallback`](https://api.flutter.dev/flutter/material/Slider/semanticFormatterCallback.html)（SDK 实测：`material/slider.dart:187,240,548`），或包一层带 `value` 的 `Semantics`。**这是自建「字段行」组件时最容易漏的一处。**

**必须手写的四处：**

| 位置 | 做法 | 理由 |
|---|---|---|
| **舞台 iframe** | `Semantics(label: 'Live2D 舞台，正在显示 <模型名>', container: true)` 包一层；内部的 iframe 内容对 Flutter 不可见 | 平台视图对读屏是黑盒；现在用户完全不知道舞台存在 |
| **流式回复** | `Semantics(liveRegion: true)` 包住流式气泡，但**必须节流**播报 | 逐 delta 播报会淹没读屏（一个 `text_delta` 可能几毫秒一个） |
| **状态徽标** | `main.dart:250-273` 的 `_WsBadge` 现在是「图标 + 文字」，颜色编码（grey/amber/green/orange/red）**只靠颜色传达状态** | 色盲用户不可辨；必须补语义标签，且文字本身要表意 |
| **装饰性元素** | `ExcludeSemantics` 包住纯装饰（如 `main.dart:267` 的小圆点） | 避免「一坨无意义节点」 |

`Semantics` / `MergeSemantics` / `ExcludeSemantics` / `BlockSemantics` 均在 SDK（实测 `widgets/basic.dart:7847, 8026, 8084, 8046`）。

### 6.3 焦点管理

三个组件都在 SDK（实测）：
- `FocusTraversalGroup`（`widgets/focus_traversal.dart:2039`）—— 定义一组内的 Tab 顺序；
- `Shortcuts`（`widgets/shortcuts.dart:1004`）+ `CallbackShortcuts`（`:1182`）；
- `Actions`（`widgets/actions.dart:729`）。

**做法：**
1. **每个面板/弹层包一个 `FocusTraversalGroup`**，用 `OrderedTraversalPolicy` 固定顺序（否则顺序跟随 Widget 树，重构就会静默改变 Tab 顺序）；
2. **焦点陷阱**：`showModalBottomSheet` / `showDialog` 自动处理焦点陷阱，**这是优先用它们而不是自绘 overlay 的又一个理由**；
3. 焦点可见性：**不要依赖 `shadowFocus`**——用 `border` + 不透明 `primary` 画焦点环，保证可测对比度。Flutter 已内建「键盘导航时显示焦点高亮、鼠标点击时不显示」的行为（`FocusManager.highlightMode`），不要覆盖它。

### 6.4 Esc 关闭

**免费的部分**：Material 的 `Dialog` / `showModalBottomSheet` 已绑定 `DismissIntent` 与 Esc；`Tooltip` 也用 Esc 关。

**需要自己加的部分**：
- 在应用根挂 `Shortcuts`，把 `DismissIntent` 绑到「关闭当前非模态覆盖层」（如内联设置分区返回舞台）；
- **注意**：不要重复处理导致模态自己被关两次，也不要让内联面板的 Esc 抢掉模态的 Esc（顺序：模态优先）。

### 6.5 快捷键

用 `CallbackShortcuts`（最省样板）挂在根：

| 键 | 动作 | 备注 |
|---|---|---|
| `Ctrl/Cmd + Enter` | 发送消息 | `TextField` 已有 `textInputAction: TextInputAction.send`（`main.dart:353`），但**在 `maxLines > 1` 时 Enter 是换行**，所以必须补显式快捷键 |
| `Esc` | 关闭覆盖层 / 取消流式 | 见 6.4 |
| `Ctrl/Cmd + ,` | 打开设置 | 桌面惯例 |
| `Ctrl/Cmd + /` | 快捷键帮助 | 可后置 |

**焦点管理纪律**：发送后焦点**留在输入框**（现在隐式如此，但重写时容易丢）；弹层打开时焦点进弹层，关闭后**焦点回到触发控件**（Material 的 `Navigator` 会处理，但要 `pumpAndSettle` 验证）。

### 6.6 `prefers-reduced-motion` —— **引擎已自动接入，无需手写 JS**

这是本节最重要的结论。

**链路（SDK + 引擎源码实测）：**

1. 引擎监听媒体查询：`engine/.../platform_dispatcher/media_query_manager.dart:26`
   ```dart
   static const REDUCED_MOTION = '(prefers-reduced-motion: reduce)';
   ```
2. 命中后写 `AccessibilityFeatures`：`engine/.../platform_dispatcher.dart:1287-1302`
   ```dart
   void _updateReducedMotion(bool reduced) {
     ...
     // There's no distinction on the web between "reduceMotion" and
     // "disableAnimations", so we set both at the same time.
     reduceMotion: reduced,
     disableAnimations: reduced,
   ```
3. 框架暴露读取 API：`MediaQuery.disableAnimationsOf(context)`（`widgets/media_query.dart:2015`）、`MediaQuery.accessibleNavigationOf(context)`（`:1928`）；
4. **`AnimationController` 默认就尊重它**：`animation/animation_controller.dart:44-76`
   ```dart
   /// When [AccessibilityFeatures.disableAnimations] is true, the device is asking
   /// Flutter to reduce or disable animations as much as possible. ...
   enum AnimationBehavior {
     normal,   // 默认：disableAnimations 为 true 时缩短时长
     preserve; // 重复动画的默认：刻意免疫，防止高频闪烁
   }
   bool get _enableAnimations => switch (this) {
     normal => !SemanticsBinding.instance.disableAnimations,
     preserve => true,
   };
   ```

**所以：**

- ✅ **不需要任何手写 JS `matchMedia` 监听。** 直接用 `MediaQuery.disableAnimationsOf(context)` 做条件渲染；
- ✅ **`AnimationController` 自动缩短时长**，包括 Material 的 `AnimatedContainer` 等；
- ✅ 多帧图片（GIF）也会被暂停——`widgets/image.dart:287-291` 明确 `MediaQueryData.disableAnimations` 会暂停它们。

**但有两个「免疫」的漏网之鱼，必须手动处理：**

| 漏网 | 原因 | 应对 |
|---|---|---|
| **`AnimationController.repeat()` 类动画** | `AnimationBehavior.preserve` 是重复动画的默认，**刻意免疫**（防止高频闪烁） | 自建的持续动画（呼吸光、脉冲、shimmer）必须显式 `AnimationBehavior.normal`，或在 `disableAnimationsOf` 为 true 时**不启动** |
| **`CircularProgressIndicator`** | 靠 `Ticker` 驱动而非 `AnimationController` 的时长缩短 | 不要用它做「装饰性」加载；它现在在 `main.dart:319`（流式）与 `live2d_stage.dart:217`（加载中）**用于表意**，可接受。装饰性场景改用静态 |

**顺带（同一机制，顺手可做）**：引擎同样处理 `prefers-color-scheme`（`platform_dispatcher.dart:22` 注释列出两者）。当前是 dark-only（`main.dart:53-67`），无需响应；但 `MediaQuery.platformBrightnessOf` 是可用的，将来加亮色成本低。

---

## 七、组件清单

### 7.1 内建即可（不要自建）

| 需求 | 用 Material 3 内建 | 实测存在 |
|---|---|---|
| 应用骨架 | `Scaffold` / `AppBar` | ✅ |
| 宽屏导航 | `NavigationRail`（`extended` / `labelType` / `minWidth`） | ✅ `navigation_rail.dart:94,102,107` |
| 窄屏导航 | `NavigationBar` | ✅ |
| 抽屉式设置 | `NavigationDrawer` | ✅ |
| 开关行 | `SwitchListTile` | ✅（`display_panel.dart:108,117,126` 已用） |
| 滑杆行 | `Slider` / `SliderTheme` | ✅ |
| 文本输入 | `TextField` + `InputDecoration` | ✅（`main.dart:348-359` 已用） |
| 按钮 | `FilledButton` / `OutlinedButton` / `TextButton` / `*.tonalIcon` / `IconButton` | ✅（均已用） |
| 分段选择 | `SegmentedButton` | ✅ 用于 DEV/PROD 之类二选一 |
| 下拉选择 | `DropdownMenu` | ✅ 用于模型 / 音色选择 |
| 搜索 | `SearchAnchor` | ✅ 用于模型库搜索 |
| 分组列表 | `ListTile` / `ExpansionTile` | ✅ 用于设置分区 |
| 分隔与容器 | `Divider` / `VerticalDivider` / `Card` | ✅（`main.dart:229,237,343` 已用） |
| 工具提示 | `Tooltip` | ✅ |
| 进度 | `CircularProgressIndicator` / `LinearProgressIndicator` | ✅ |
| 对话框 | `AlertDialog` / `showModalBottomSheet`（自带焦点陷阱 + Esc） | ✅（`main.dart:151` 已用弹层） |
| 主题 | `ThemeData` / `ColorScheme.fromSeed` / `ThemeExtension` | ✅ |

### 7.2 必须自建

| 组件 | 一句话职责 | 层 | 为什么内建不行 |
|---|---|---|---|
| `lib/design/tokens.dart` | **全应用唯一**的颜色/字号/间距/圆角/动效常量与 `ThemeExtension` 定义 | L1 | Material 没有间距/动效 token 维度；且必须单点豁免扫描测试 |
| `lib/design/breakpoints.dart` | `SizeClass sizeClassOf(double width)` 纯函数 | L1 | `LayoutBuilder` 里的魔法数字不可测（现 `main.dart:208` 的反例） |
| `AppShell` | 按 `SizeClass` 选择导航形态 + `IndexedStack` 保活舞台 | L3 | 布局决策是产品专属 |
| `StageHost` | 包装 `Live2DStage`：保活、`RepaintBoundary`、错误/加载覆盖层、语义标签 | L3 | 现有 `Live2DStage` 混了「桥管理」与「覆盖层 UI」两件事（`live2d_stage.dart:152-177`），应拆开 |
| `ChatPanel` | 消息列表 + 输入区 + 停止 | L3 | 产品专属（现状在 `main.dart:275-388`，需拆出） |
| `MessageBubble` | 单条消息：角色标签、流式光标、失败态 | L3 | 产品专属（`main.dart:441-481`） |
| `StreamingIndicator` | 流式中的节流提示（**不用 `CircularProgressIndicator` 做装饰**） | L3 | 见 §6.6 |
| `ErrorBanner` | 可关闭的行内错误提示 | L3 | 产品专属（`main.dart:408-439`） |
| `SettingsScaffold` | 设置的分区导航 + 草稿生命周期 + 未保存提示 + 保存栏 | L3 | 产品专属；**这是设置区最有价值的一个抽象** |
| `FieldRow` | 统一的「图标 + 标签 + 控件 + 帮助文本 + **语义值**」行 | L3 | 现 `display_panel.dart:154-204` 的 `_SliderRow` 是雏形，但只支持滑杆、且缺 `semanticFormatterCallback`；需泛化为滑杆/开关/文本/下拉 |
| `SectionHeader` | 设置分区标题 + 分区说明 | L3 | 避免每处手写字号字重（正是 §1.3 发现的 16/w600 vs 16/w700 问题源） |
| `ModelPicker` | 模型列表/导入/激活/显示配置 | L3 | 产品专属，接 `/api/v1/models` 六个子路由 |
| `ModList` | Mod 开关 + 配置表单 | L3 | 产品专属，接 `/api/v1/mods` |
| `LogViewer` | 日志列表 + 级别过滤 | L3 | 产品专属，接 `/api/v1/logs`；**必须虚拟化长列表** |
| `ConnectionBadge` | WS 连接状态（文字 + 语义，不靠颜色） | L3 | 现状 `main.dart:250-273` 只靠颜色 |
| `TestConnectionButton` | 触发 `settings/test/llm|tts` 并展示延迟/错误 | L3 | 产品专属 |
| `CommandPalette`（可选） | 列出并触发 `/api/v1/commands` | L3 | 可后置 |

**内建 + 自建的边界原则**：**凡是 Material 已表达「行为」的（按钮、输入、开关、导航），一律内建**；**凡是表达「本产品语义」的（舞台、分区、字段行、状态徽章），一律自建**。自建组件**内部**尽量由内建组件拼装，避免重造 `InkWell` / 焦点 / 语义。

**看似需自建但实际内建已够（避免重复造轮子）**：卡片、遮罩弹窗、抽屉、Tab、折叠面板、Tooltip、Toast（用 `ScaffoldMessenger`+`SnackBar`）、日期/数字输入。**现仓库没有自建这些，重写时也不要**。

---

## 八、落地顺序建议

| 阶段 | 内容 | 验收 |
|---|---|---|
| P0 | `tokens.dart` + `breakpoints.dart` + 扫描测试 + `--no-web-resources-cdn` 构建命令固定 | `flutter analyze && flutter test` 全绿；无外网下应用能起来且中文正常 |
| P1 | `AppShell` + `IndexedStack` 舞台保活 + `SizeClass` 三档导航骨架（设置分区先放空壳） | 三档断点各有 Widget 测试；舞台切换不重载模型 |
| P2 | `ChatPanel` 拆分 + `ChatController` 保持现有语义 + L2 fake 测试（每类 WS 帧一条） | 聊天行为与现状等价 |
| P3 | 7 个设置分区：先「外观与语音」（纯本地，验证字段行与实时联动），再 LLM/TTS（含 test 按钮），再模型/Mod/诊断 | `SettingsController` 的草稿/dirty/patch 测试 |
| P4 | 无障碍与键盘：`Semantics`（4 处 + 滑杆 formatter）、`FocusTraversalGroup`、Esc、快捷键、`disableAnimationsOf` 分支、**音频解锁覆盖键盘**（§9.1） | 键盘可完成主流程且**有声音**；`disableAnimations` 下无持续动画 |
| P5 | 性能收口：毛玻璃降级开关、`RepaintBoundary`、列表上限、产物清理 | profile 构建实测帧时间 |

---

## 九、不确定 / 需人工决策的点

| # | 决策点 | 我的倾向 | 需要谁定 |
|---|---|---|---|
| **D1** | **任务书写 `GET/PUT /api/v1/settings`，实际是 `GET/PATCH`**（§2.1 已验证）。是否确认以 `PATCH` 为准？ | 以代码为准用 `PATCH`；若确实需要 PUT，应改 Rust 而不是让前端猜 | Rust 侧 / 架构 |
| **D2** | **compact（<900）下舞台无法完全不被遮挡**（§2.4）。接受「弹层遮住大部、露出顶部」，还是坚持「设置全屏、舞台不可见」？ | 接受遮挡 + 限制弹层高度至 ~72% | **产品** |
| **D3** | **是否打包中文字体？**（§5.2）不打包 = 无外网时中文不显示（实测机制已确认）。打包 = 产物增大，需子集化 | 必须打包 + 子集化；具体字体与体积需实测后定 | 产品（体积预算）+ 许可审查 |
| **D4** | **设计稿要求的 `backdrop-blur` 毛玻璃是否保留？**（§5.8、`web-ui-spec-v2.md:78`） | 降级为半透明纯色 + hairline；若保留则限 1 处 + 可关闭 | 产品 |
| **D5** | **聊天历史是否落 localStorage？**现状是纯内存、无上限（`chat_controller.dart:48`） | 落盘但**只存文本 + 上限 200 条**，绝不存音频/key | 产品 |
| **D6** | **是否引入 provider/riverpod？**§3.2 给了 5 条触发条件 | **暂不引入**；触发 T1–T5 任一再评估（届时需过许可审查） | 架构（可在 P3 复盘） |
| **D7** | **亮色主题是否要支持？**当前 dark-only（`main.dart:53-67`） | 不做；但这决定了 token 是否全量进 `ThemeExtension`（§1.1） | 产品 |
| **D8** | **`ThemeExtension` vs `const` 常量**的分界（§1.1）——我建议尺寸用 `const`、颜色族用 extension | 同左 | 架构（低风险，可 P0 定） |
| **D9** | **7 个设置分区的命名与分组顺序**（§2.1 表格是我按后端契约归纳的，不是既有共识）。注意分区 5 现已含**主音量**（`display_panel.dart:87`），建议改名「外观与语音」 | 同表；「外观与语音」必须独立成区（纯本地语义，与其他区不同） | 产品 |
| **D10** | **`IndexedStack` 保活舞台的 CPU 代价**（§2.5）：弹层打开时 iframe 仍在渲染。是否需要「弹层打开时暂停 idle 动作」？ | 保留渲染（口型不该停），仅暂停 idle | 产品 + 实测 |
| **D11** | **「默认出声」与「首次访问的自动播放限制」的冲突**（新增，见下） | 需要一次显式指针交互解锁 | 产品 + 实测 |

**另外两处需要人工确认的推断**（非决策，是事实核验）：
- `provider` / `riverpod` 的许可与 AGPL-3.0-only 兼容性，我按 MIT（常见文本）推断，**未逐行核验其 LICENSE 文件**；正式引入前须走 `docs/research/license-report.md` 的流程。
- 现有依赖许可（`http`、`web`、`flutter_lints`、`collection`、`material_color_utilities`）为 BSD-3-Clause 系，与 AGPL-3.0-only 兼容（【推断】，基于 Dart/Flutter 生态惯例 + `pubspec.lock` 包身份，未逐一打开 LICENSE）。
- `build/web` 总 41MB 中含 `skwasm*` / `chromium/` / `wimp*` / `*.symbols` 等，我**推断**当前 JS+CanvasKit 路径不请求它们，但**未实测网络请求验证**；P5 清理前须实测确认。

### 9.1 D11 详情：「默认出声」带来一个键盘可达性缺口（【已验证】）

语音约定刚改为**默认出声**（`display_prefs.dart:15,37-39`），但 Web 的 autoplay 限制要求**先有一次用户手势**才能 `resume()` `AudioContext`。现有实现的解锁入口是：

```dart
// main.dart:175-178
return Listener(
  // 首次指针交互解锁 WebAudio（浏览器 autoplay 限制）。
  behavior: HitTestBehavior.translucent,
  onPointerDown: (_) => _audio.unlock(),
```

`AudioPlayer.unlock()`（`audio_player.dart:213`；类注释 `:134` 说明 autoplay 限制）**只挂在 `onPointerDown` 上**。

**后果**：纯键盘用户（Tab + Enter 发送消息、回车触发）**永远不会解锁音频**——消息发得出去、模型嘴在动，但**没有声音**，且界面不会给出任何解释。这与「默认出声，否则用户会以为 TTS 坏了」的裁决意图**直接冲突**。

**建议**：解锁入口至少要覆盖
1. `onPointerDown`（保留）；
2. **任何键盘事件**（`Focus`/`KeyboardListener` 的任意键，或首次 `Shortcuts` 命中）；
3. 理想情况下在 `AudioContext.state == 'suspended'` 时于 UI 上显示一条可见提示（「点击任意位置以启用声音」），把失败**显式化**。

**验收**：`flutter test` 无法覆盖（需要真实浏览器），应进 `docs/verification/` 的人工自测清单：**仅用键盘发一条消息，确认有声音。**

---

## 十、参考链接

**Flutter 官方文档**
- [Flutter architectural overview](https://docs.flutter.dev/resources/architectural-overview)
- [Web renderers](https://docs.flutter.dev/platform-integration/web/renderers)
- [Web FAQ（含 CanvasKit / 字体 / CDN 说明）](https://docs.flutter.dev/platform-integration/web/faq)
- [Building a web application with Flutter](https://docs.flutter.dev/platform-integration/web/building)
- [Flutter 中的无障碍](https://docs.flutter.dev/ui/accessibility-and-internationalization/accessibility)
- [Web 上的无障碍](https://docs.flutter.dev/ui/accessibility-and-internationalization/web-accessibility)
- [State management 官方推荐](https://docs.flutter.dev/data-and-backend/state-mgmt/intro)
- [Simple app state management（ChangeNotifier + InheritedNotifier）](https://docs.flutter.dev/data-and-backend/state-mgmt/simple)
- [Lists & grids（性能）](https://docs.flutter.dev/ui/layout/lists)
- [Performance best practices](https://docs.flutter.dev/perf/best-practices)
- [Implicit animations](https://docs.flutter.dev/ui/animations/implicit)
- [Material 3 设计令牌](https://m3.material.io/foundations/design-tokens/overview)
- [Material 3 断点](https://m3.material.io/foundations/layout/understanding-layout/parts-of-layout)

**Flutter API（本机 SDK 已逐条核验）**
- [`ThemeExtension`](https://api.flutter.dev/flutter/material/ThemeExtension-class.html) · [`ThemeData.extensions`](https://api.flutter.dev/flutter/material/ThemeData/extensions.html) · [`ThemeData.extension<T>()`](https://api.flutter.dev/flutter/material/ThemeData/extension.html)
- [`ColorScheme.fromSeed`](https://api.flutter.dev/flutter/material/ColorScheme/ColorScheme.fromSeed.html) · [`ColorScheme.surfaceContainerHighest`](https://api.flutter.dev/flutter/material/ColorScheme/surfaceContainerHighest.html)
- [`MediaQuery.disableAnimationsOf`](https://api.flutter.dev/flutter/widgets/MediaQuery/disableAnimationsOf.html) · [`MediaQuery.accessibleNavigationOf`](https://api.flutter.dev/flutter/widgets/MediaQuery/accessibleNavigationOf.html)
- [`AnimationBehavior`](https://api.flutter.dev/flutter/animation/AnimationBehavior.html) · [`AccessibilityFeatures.disableAnimations`](https://api.flutter.dev/flutter/dart-ui/AccessibilityFeatures/disableAnimations.html)
- [`FocusTraversalGroup`](https://api.flutter.dev/flutter/widgets/FocusTraversalGroup-class.html) · [`Shortcuts`](https://api.flutter.dev/flutter/widgets/Shortcuts-class.html) · [`CallbackShortcuts`](https://api.flutter.dev/flutter/widgets/CallbackShortcuts-class.html) · [`Actions`](https://api.flutter.dev/flutter/widgets/Actions-class.html) · [`DismissIntent`](https://api.flutter.dev/flutter/widgets/DismissIntent-class.html)
- [`Semantics`](https://api.flutter.dev/flutter/widgets/Semantics-class.html) · [`MergeSemantics`](https://api.flutter.dev/flutter/widgets/MergeSemantics-class.html) · [`ExcludeSemantics`](https://api.flutter.dev/flutter/widgets/ExcludeSemantics-class.html) · [`SemanticsProperties.liveRegion`](https://api.flutter.dev/flutter/semantics/SemanticsProperties/liveRegion.html)
- [`BackdropFilter`](https://api.flutter.dev/flutter/widgets/BackdropFilter-class.html) · [`Image`（cacheWidth / precacheImage）](https://api.flutter.dev/flutter/widgets/Image-class.html) · [`ImageCache`](https://api.flutter.dev/flutter/painting/ImageCache-class.html)
- [`NavigationRail`](https://api.flutter.dev/flutter/material/NavigationRail-class.html) · [`NavigationBar`](https://api.flutter.dev/flutter/material/NavigationBar-class.html) · [`NavigationDrawer`](https://api.flutter.dev/flutter/material/NavigationDrawer-class.html) · [`SegmentedButton`](https://api.flutter.dev/flutter/material/SegmentedButton-class.html) · [`DropdownMenu`](https://api.flutter.dev/flutter/material/DropdownMenu-class.html) · [`SearchAnchor`](https://api.flutter.dev/flutter/material/SearchAnchor-class.html)

**本仓库**
- `AGENTS.md`（双主导分层、前端豁免 rust-ratio、语音「一句一单元」约定、密钥红线）
- `shell/README.md`（构建命令、`--base-href /app/` 必要性、同源约束）
- `docs/research/flutter-live2d-implementations-2026-09.md`（渲染面方案选型，本文档的上游）
- `docs/design/web-ui-spec-v2.md`（既有视觉规格：三层信息层级、毛玻璃、动效时长）
- `docs/design/web-ui-settings-wiring-v1.md`（设置接线与 persona 字段扩展）
- `docs/research/license-report.md`（依赖许可审查流程）
