# 前端加强计划：参考 Morrow 前端（2026-09-11）

> 一句话口径：**搬 Morrow 已经被验证的「观感机制」（材质插值 / 面板形态 / 动效接线 / 视觉审查），
> 来填补本项目「令牌齐全但接线为零」的空白；不搬它的设计纪律（断点与时长散值），
> 更不搬它的 `BackdropFilter` 玻璃。**
>
> 参考对象：<https://github.com/StarrySky7D4/morrow>（`morrow_studio`，Apache-2.0）
> 取证 commit：`a3c1766 Release v0.1.9-test.1 with animated compact settings navigation`
> **状态：用户已同意（2026-09-11），全部完成。** P0–P4 逐项落地，其中 P3-3
> 经实地核查后**判定不做**（理由见该节），P4-2 的口径在实施时收窄（见「实施期
> 发现」第 5 条）。

## 实施进度（每期收尾时更新）

| 期 | 状态 | 门禁数字 | 备注 |
| --- | --- | --- | --- |
| P0 纠错与清理 | ✅ 完成 | 634 | 删死代码 285 行、合并舞台双覆盖层、数值真源合一 |
| **P4-1 修令牌门禁假阴性** | ✅ **提前完成** | 634 | 先修尺子：修好当场报出 7 条过期台账 |
| P1 动效接线 | ✅ 完成 | **657** | soft_motion / CollapsiblePanel 保活 / 底色插值 / 焦点环 / 统一提示 / 折叠横幅 |
| P2 面板形态 | ✅ 完成 | **703** | compact 整页过渡（层数 2→1）、内联结果切分区即清 + 代际对账、气泡轻量 Markdown + 复制 |
| P3 材质 | ✅ 完成 | **721** | `GlassRim` 上三个面（只描边不模糊）、`glassBarrier` 接线（台账清空）；**P3-3 核查后判定不做**（见该节的三条理由） |
| P4 工程质量 | ✅ 完成 | **738**（+12 张出图） | P4-1 修门禁假阴性（提前）、P4-2 裸间距扫描（**口径收窄**，见下）、P4-3 textScaler 16 条、P4-4 视觉审查工具、P4-5 文档除锈 4 处、P4-6 断言模板（P2-1 已落地） |

**门禁口径**：起点 `flutter test` **627 通过**；每期结束时不得低于起点，
且新增守卫必须全绿。当前 **738**（另有视觉审查工具出 12 张 PNG）。

### 实施期发现的三件计划外的事（都记在 CHANGELOG 里）

1. **P4-1 必须提前**——不修尺子的话，P1 接线之后台账依然是假的（见 §5.2）。
   修好之后立刻报出 7 条「其实早就接线了」的过期条目。
2. **新门禁当场抓到一个真漏**：`theme_picker` 的选中态动画写的是
   `duration: AppDurations.fast`——令牌对、**闸门漏**，开了「减少动画」的用户
   那里它照动不误且不报错。这正是 `motion_wiring_test.dart` 要防的东西。
3. **「折叠而不是卸载」会带来一个平台视图副作用**（计划里没预料到）：
   侧板常驻之后，那层 `StagePointerInterceptor`（透明 `<div>`）也一直留着，
   收起时它**仍在吞舞台的指针**——模型会拖不动。修法是给垫层加 `enabled`，
   把 `pointer-events` 设回 `none`（**改样式而不是不挂载**：不挂载会换掉
   widget 类型，下面的子树重建，保活就白做了）。
   **P2-1 之后这个问题又出现了一次**（compact 的整页设置同样常驻），
   同一个开关又用了一次——`enabled` 这个设计是对的。
4. **「两页都常驻」是对 Morrow 的一处刻意偏离**（P2-1）：它的
   `SettingsPageTransition` 是「过半才构建第二页」，那会让 compact 关掉设置
   再打开时丢掉滚动位置（P1-1 刚为侧板立过这条纪律），而且切换那一帧要把
   整棵设置页建出来、正好卡在动画中段。我们两页都留在树里、用 `Offstage` 隐藏。
5. **P4-2 的口径在实施时收窄了**（计划写的是「扩到间距/宽高」）：真去量了一遍，
   18 个文件 44 处，其中绝大多数 `width:`/`height:` 是**元素自身尺寸**
   （图标 18、发送键 40×40、数值列 44/52/56/110/150、1 px 描边、3–4 px 圆点），
   不是「元素之间的间距」。把它们也拦下来会逼着实现把 1 px 描边写成
   `Space.s1`（=4）——**那是改设计，不是归位**。所以收窄成只扫 `EdgeInsets`
   （没有歧义），并把 1–2 px 的基线微调命名为 `OpticalNudge`（它不在 4 px
   网格上，也不该在）。改完 10 处全部归位。
6. **P4-3 实测四档全过**：原以为会抓到一批溢出，实际没有——那些固定盒子当时
   取值都够用。所以这一期的产出是「把这条以前没人看过的轴钉住」，而不是
   「修了几个溢出」。证据是它确实会红（写的时候气泡那条当场报溢出）。
7. **减法也是收获**：P2-1 之后 `showSectionDrawer`、
   `NavMetrics.sheetMaxWidthCompact/sheetHeightFactorCompact`、
   `sheetConstraintsOf` 的 compact 分支全部变成死代码，一并删掉——
   与 P0「先删后改」同一条纪律。

---

## 0. 为什么是这份参考（取证）

### 0.1 两个项目是同一类东西，可比性成立

| 维度 | 本项目（Live2D-Ai 前端） | Morrow |
| --- | --- | --- |
| 定位 | 通用人形皮套 AI 接入平台，**本地优先的桌宠** | 灵感工作台 / 随身听，**本地优先的个人工具** |
| 技术栈 | Flutter Web（由 Rust 在 `/app/` 同源托管） | Flutter **Windows + Web 双端** |
| 规模 | `lib/` 68 文件 / **14107** 行；`test/` 37 文件 / 9329 行 | `lib/` 59 文件 / **9489** 行；`test/` 16 文件 / 3167 行 |
| 结构 | 分层清晰（design / ui / app / state / api …，单文件 ≤500 行门禁） | **单文件 3619 行的 `main.dart`** + 若干模块 |
| 门禁 | `flutter analyze` + **627 条测试** + 令牌扫描 + 视觉语言扫描 | `flutter analyze` + `flutter test`，lint 仅 `flutter_lints` 默认集 |
| 运行时依赖 | **仅 `http` + `web`**（硬约束：新增依赖 = 0） | 20+ 包（media_kit / super_clipboard / flutter_acrylic / window_manager …） |

**结论**：论代码质量与纪律，本项目更强；论**观感完成度**，Morrow 明显更强。
加强的方向因此是「把它的观感机制搬进来」，而不是「学它的工程做法」。

### 0.2 实机取证（不是只看源码）

Morrow 的 Windows 版已在本机跑起来并截图（1280×720，深色 + 液体玻璃 + 光晕背景）：

| 图 | 内容 | 路径 |
| --- | --- | --- |
| 01 | 主界面全貌（三栏：侧边栏 233 px + 内容 + 右侧外观栏） | `docs/design/assets/morrow-reference-2026-09-11/01-overview-dark-liquid.png` |
| 02 | 侧边栏 3× 放大（导航层级、选中态、字号阶梯） | `…/02-sidebar-3x.png` |
| 03 | 自绘标题栏 2× 放大（左侧应用名，右侧窗口按钮） | `…/03-titlebar-2x.png` |
| 04 | 右侧外观栏 3× 放大（材质/主题/背景控件堆叠） | `…/04-right-rail-3x.png` |
| 05 | 主内容区 2× 放大（首屏层叠与文字层级） | `…/05-hero-2x.png` |

从截图量出来的客观事实（供对照，不是主观评价）：

- 深色底**不是单一平面**：主内容区是蓝灰渐变（`#2b313f → #344355`），
  右侧栏更亮一档（`#313946`），靠**面差**而不是描边分层级；
- 强调色是**青色**（`#60d6fa` 系，实测高饱和像素占比最高的一族），
  也就是 `shared_preferences.json` 里落盘的 `themeColor: 4284536570 = 0xFF60D6FA`；
- 窗口是 **20 px 圆角**（`windowRadius: 20.0`）+ 自绘标题栏，**没有系统标题栏**。

### 0.3 Morrow 有六个坑，一个都不要抄（反向取证）

参考一个项目，**必须先看清它没解决什么**，否则会把它的债一起搬进来：

| # | Morrow 的坑 | 取证 | 我们的状态 |
| --- | --- | --- | --- |
| 坑 1 | **Flutter Web 中文缺字**：`main.dart:264-265` 写死 `'Segoe UI'` + `['Microsoft YaHei','Arial']`，`pubspec.yaml` 的 `flutter:` 段**没有 `fonts:` 声明**，仓库里**没有任何字体文件**，而界面全中文 | 源码实测 | **我们已经解决**（`NotoSansSC` 自托管子集 + `--no-web-resources-cdn` + `theme_test.dart` 守卫）。**这是本项目唯一领先 Morrow 的硬指标，绝不能因为「参考它」而回退** |
| 坑 2 | **工程纪律弱**：`analysis_options.yaml` 仅 `include: flutter_lints/flutter.yaml`（`rules:` 全被注释）、**无 CI**（仓库无 `.github/`）、无覆盖率门槛、`main.dart` 单文件 **3619 行** | 源码实测 | 我们有 627 条测试 + 令牌扫描 + 视觉语言扫描 + 单文件 ≤500 行门禁。**只借它的「测什么」，不借它的「怎么管」** |
| 坑 3 | **无状态管理 + 巨型单文件**：`lib/main.dart` 里 `_StudioState` 一个类就 2400 行 | 源码实测 | 我们的分层（design/ui/app/state/api）不动 |
| 坑 4 | **存储是单快照 + 字面量 key**：`storage.dart:29-33`，`version != 1` 直接抛 `FormatException` 而非迁移 | 源码实测 | 我们已有自己的偏好层，不动 |
| 坑 5 | **断点判据有两处来源**：布局读 `LayoutBuilder.constraints.maxWidth`（`main.dart:897-898`），而「变宽就关掉窄屏设置页」读 `MediaQuery.sizeOf(context).width - paddingOf().horizontal`（`main.dart:716-718`）。Morrow 里两者恰好数值相等，**是巧合而非设计** | 源码实测 | 我们的 `Breakpoints.sizeClassOf(width)` 是**单一纯函数**，继续只此一处 |
| 坑 6 | **同一个 32 px 写两遍**：标题栏高度在 `desktop_frame.dart:84` 用常量，在 `main.dart:894` 用字面量 `EdgeInsets.only(top: 32)` | 源码实测 | 我们的扫描规则 P4-2 正是防这类事 |

> 一句话：**Morrow 是「观感上值得抄、工程上不值得抄」的参考**。
> 本计划的所有条目都遵守这条分界——凡涉及代码组织、门禁、依赖、存储的，一律不学它。

### 0.4 本仓库的基线（本计划执行前 / 执行后都要跑）

本机已装 Flutter `3.47.3` / Dart `3.13.3`（在 `~/flutter/bin`，**不在 PATH 里**，
`which flutter` 是空的——这一点此前被审计误判为「本机无 Flutter」）：

```bash
cd shell/flutter && export PATH="$HOME/flutter/bin:$PATH" \
  && LIVE2D_AI_MUTE_AUDIO=1 flutter analyze && LIVE2D_AI_MUTE_AUDIO=1 flutter test
```

2026-09-11 实测基线：**`flutter test` → 627 passed**（`All tests passed!`，exit 0）。
任何一期的验收都以「不低于 627 且新增守卫全绿」为准。

---

## 1. 结论先行：三条复用判定

### 判定一：最该搬的是「动效机制 + 面板形态」，因为它正好补上我们最大的空白

Morrow 的全部动效都收在一个**四行的辅助函数**里（`lib/appearance.dart:243-246`）：

```dart
Duration motionDuration(BuildContext context, int milliseconds) =>
    MediaQuery.disableAnimationsOf(context)
    ? Duration.zero
    : Duration(milliseconds: milliseconds);
```

它被 **16 处**调用（`main.dart` 16 次 / `appearance.dart` 4 次 / 折叠面板、桌面框、
色彩罗盘、提示语各 1–2 次），因此 Morrow 的**每一个**过渡都天然尊重
`prefers-reduced-motion`。

对照本项目：我们的令牌**比它更规范**（`AppDurations` 4 档 + `Motion` 2 条曲线，
`lib/design/tokens.dart:596-628`），但**接线数为 0**——`AppDurations.base/slow/reveal`
与 `Motion.enter` 在全仓库零引用，整个应用只有 `theme_picker.dart` 一处
`AnimatedContainer`。结果是：**令牌最全，动效最少**。

> 所以第一期不是「造动效」，而是「把已有的 4 档时长 + 2 条曲线接到该接的地方」。

### 判定二：最不该照抄的是 `BackdropFilter` 玻璃——Morrow 自己也知道它到不了底

Morrow 全仓库 `BackdropFilter` **只出现 1 次**（`lib/liquid_glass.dart:209`），
而且被一层开关挡着（`lib/liquid_glass.dart:106`）：

```dart
bool get _filterEnabled => !widget.canvas || !widget.transparentCanvas;
```

旁边的注释把原因写得很清楚（`lib/liquid_glass.dart:204-206`）：

> `// The transparent canvas cannot sample the OS desktop through a`
> `// Flutter backdrop. Leave its interior unpainted; retain the rim.`

即：**透明画布上模糊不到后面的东西，只能放弃铺底、只保留边缘光**。
本项目的情况是同一类问题的更强版本——舞台是 **`<iframe>` 平台视图**，
压在上面的 `BackdropFilter` **既模糊不到 iframe，也照付模糊代价**，
所以规格 §12-7 已经把 `BackdropFilter` / `ImageFilter.blur` 定为**命中数必须为 0**
（`test/no_backdrop_filter_test.dart` 守着）。

**可以搬的是不含 `BackdropFilter` 的那一半**：`LiquidRimPainter`
（`lib/liquid_glass.dart:248-313`）—— 纯 `CustomPaint`，靠指针位置驱动一个
1.6 px 的渐变描边 + 两道内圈高光，**零模糊、零采样、零依赖**。
这正是本项目「glassScrim + 1 px hairline」目前缺的那一点「贵气」。

### 判定三：抄它的**覆盖面**，不抄它的**词汇表**

Morrow 的断点是 1050 / 760 / 590 / 520 / 460（`main.dart:719, 897-898, 1495, 1681, 3110`），
时长是 150 / 200 / 220 / 240 / 250 / 260 / 280 / 320 / 340 / 360 / 700（11 个取值）。
本项目是 3 档断点（`lib/design/breakpoints.dart`）+ 4 档时长 + 2 条曲线，
并且有 `design_tokens_lint_test.dart` 扫描裸值。

**保持本项目的约束，用它的场景清单**。Morrow 全仓的动效时长共 **15 类场景**
（不含协议/音频时长），下表把它们逐类映射到我们的 4 档令牌 + 2 条曲线上——
**一类都不漏，但一个裸数字都不新增**：

| Morrow 场景 | Morrow 时长 | 本项目映射 | 本项目落点 |
| --- | --- | --- | --- |
| compact 设置页切换 | 320 | `slow` (320) + `state` | P2-1 |
| 页面内容交叉淡入 | 300 | `slow` (320) | P2-1 |
| 侧栏 / 设置栏折叠 | 340 | `slow` (320) | P1-1 |
| 玻璃材质形变 | 360 | `slow` (320) | P1-3 |
| 背景换肤渐变 | 360 | `slow` (320) | P1-3 |
| 媒体画布切换 | 280 | `slow` (320) | 本项目无此场景 |
| 对话框入场（scale .96→1） | 260 | `slow` (320) | 本项目浮层入场 |
| 卡片 / 内联内容切换 | 220/240 | `base` (200) | P2-2 |
| hover / 导航选中 / 标题栏配色 | 220 | `base` (200) | P1-2 |
| 消息渐入 / 横幅滑入 | 200 | `base` (200) | P1-2 |
| 液体玻璃光斑跟随 | 180 | `fast` (120) | P3-1 |
| 开关 / 徽标切换 | 150 | `fast` (120) | P1-4 |
| 启动揭示 | —（Morrow 无） | `reveal` (600) | 本项目独有，可接 |
| 提示语轮换淡入 | 700 | 本项目无此场景 | — |
| Tooltip 延迟 | 400（延迟，非时长） | `AppRhythms.hintDelay` (400) | 已是令牌 |

---

## 2. 可复用资产清单（逐文件判定）

判定分三档：**直接搬**（改令牌引用即可） / **改造搬**（要接本项目令牌与约束） /
**只取思路**（不搬代码，只改做法） / **不搬**（与本项目红线或范围冲突）。

| # | 资产 | Morrow 位置 | 依赖 | 判定 | 本项目落点 |
| --- | --- | --- | --- | --- | --- |
| R1 | **`LiquidRimPainter`** 指针跟随边缘光（三层：1.6 px 四段渐变 / 3 px 柔光 / 0.65 px 内白） | `lib/liquid_glass.dart:248-313` | 无 | **直接搬**（色值改令牌；`intensity` 接四套主题） | 新建 `lib/ui/glass_rim.dart`（§4 P3-1） |
| R2 | **`GlassMaterial` + `GlassMaterialTween`** 可插值材质模型（`blur`/`liquid`/`decoration` 一起 lerp） | `lib/liquid_glass.dart:4-74` | `ui.lerpDouble` | **直接搬** | 新建 `lib/design/glass_material.dart` |
| R3 | **`CollapsiblePanel`** 折叠面板（**反向折叠时子树不重建**） | `lib/collapsible_panel.dart:1-53` | 无 | **改造搬**（接 `AppDurations`） | 新建 `lib/app/collapsible_panel.dart` |
| R4 | **`SettingsPageTransition`** 双页交叉淡化（两页不重叠） | `lib/settings_page_transition.dart:1-105` | 无 | **改造搬** | compact 设置页切换 |
| R5 | **`SoftSwap` / `SoftSize`** 微小动效封装 | `lib/appearance.dart:403-444` | 无 | **直接搬** | 新建 `lib/ui/soft_motion.dart` |
| R6 | `motionDuration(context, ms)` 统一 reduce 入口 | `lib/appearance.dart:243-246` | 无 | 只取思路（我们已有 4 档令牌，缺的是**统一出口**） | 保持令牌，禁止新写裸 `Duration` |
| R7 | **三材质配方数值**（磨砂 22 / 超透 1 / 液体 5 的模糊，20%–100% 不透明度） | `lib/appearance.dart:196-206, 167-246` | `BackdropFilter` | **只取配方**（数值用作渲染面参考），Flutter 侧**不搬** | §4 P3 |
| R8 | 材质形变过渡（同一 `Glass` 上 `TweenAnimationBuilder<GlassMaterial>`，360 ms） | `lib/appearance.dart:220-239` | 无 | **改造搬**（那是本项目主题切换缺的过渡） | §4 P1-3 |
| R9 | **视觉审查工具**：`flutter_test` 里渲染成 PNG 供人眼复核 | `tool/feature_visual_test.dart` | `flutter_test` | **直接搬思路**（补我们「无 golden / 无 integration」的洞） | 新建 `tool/visual_review_test.dart`（§4 P4） |
| R10 | 紧凑设置导航（仓库 HEAD 那个提交） | `lib/main.dart:719-770, 895-910` | 无 | **改造搬** | §4 P2-1 |
| R11 | 光晕背景 `AmbientPainter` / 球体 `OrbPainter` | `lib/main.dart:3467-3630` | 无 | **只取思路，且需用户裁决** | §7 裁决点 ①（与「舞台中央不放贴图」冲突） |
| R12 | 色彩罗盘（HSV 拖拽 + HEX 输入 + 明暗） | `lib/color_compass.dart:1-224` | 无 | 待定 | §7 裁决点 ②（只在做「自定义主题色」时才需要） |
| R13 | 自绘标题栏 + 窗体圆角（Windows 原生） | `lib/main.dart`（`desktopCaption`）、`lib/desktop_frame.dart`、`lib/window_effects.dart` | `flutter_acrylic`/`window_manager` | **不搬** | 与 F3「桌面壳」一起单独立项，且**会引入运行时依赖**（违反依赖=0） |
| R14 | 附件 / 剪贴板 / 媒体播放 / 歌词联网 | `lib/attachments/`、`lib/music/`、`lib/media/` | `super_clipboard`/`media_kit`/`http` | **不搬** | 与本项目「不做功能堆砌」的用户裁决冲突 |
| R15 | 存储层（`shared_preferences` + IndexedDB 双端） | `lib/storage*.dart`、`lib/media/texture_storage_*.dart` | 平台插件 | 只取「**两平台分别落盘**」的思路 | 本项目已有 localStorage 偏好，够用 |
| **R16** | **`compact_settings_test.dart` 的断言模板**（比实现更值钱） | `test/compact_settings_test.dart:149-199`（全 240 行） | `flutter_test` | **直接搬思路** | §4 P4-6：把它的五组断言搬成我们的回归 |

### 2.1 三个最值钱的资产，逐段拆解

#### R1 `LiquidRimPainter`：一个「不模糊的玻璃」长什么样

它的全部做法是：**在圆角矩形边框上画三层描边，方向和强度跟着指针位置走**
（`lib/liquid_glass.dart:260-304`）：

```dart
final rect = (Offset.zero & size).deflate(.8);
final rrect = RRect.fromRectAndRadius(rect, Radius.circular(radius));
final gradient = LinearGradient(
  begin: Alignment(light.dx, light.dy),      // ← 指针位置决定方向
  end: Alignment(-light.dx, -light.dy),
  colors: [
    Colors.white.withValues(alpha: (dark ? .70 : .94) * intensity),
    Colors.white.withValues(alpha: .08 * intensity),
    const Color(0xFF77799F).withValues(alpha: .12 * intensity),
    Colors.white.withValues(alpha: .55 * intensity),
  ],
  stops: const [0, .36, .66, 1],
);
canvas.drawRRect(rrect, Paint()
  ..style = PaintingStyle.stroke
  ..strokeWidth = 1.6
  ..shader = gradient.createShader(rect));
```

外圈 1.6 px（白 → 8% → 灰紫 → 55% 的四段渐变）、中圈 3 px 内缩 2 px 的柔光、
内圈 0.65 px 10% 白。指针位置用 `MouseRegion` 归一化到 `[-1, 1]`，再经
`TweenAnimationBuilder<Offset>` 180 ms `easeOutCubic` 追上（`lib/liquid_glass.dart:180-194, 220-239`），
`onExit` 回落到固定方向 `(-.65, -.8)`。

**为什么适合我们**：`CustomPaint` + `Paint..shader`，不采样任何背景，
所以压在 iframe 上也没有平台视图问题；`intensity` 参数天然对接我们的主题
（四套主题各给一个 `liquid` 强度即可）。它替换的正是我们现在那句
「1 px `hairline` 描边 + 面差」在**设置侧板 / 浮层 / 舞台角标**上的观感。

#### R3 `CollapsiblePanel`：折叠动画里最容易被写错的那件事

Morrow 的写法（`lib/collapsible_panel.dart:24-52`）把「折叠」拆成两件事：
**尺寸因子**交给 `Align(widthFactor/heightFactor)`，**语义与焦点**交给
`IgnorePointer / ExcludeFocus / ExcludeSemantics / TickerMode`：

```dart
return ClipRect(
  child: Align(
    alignment: Alignment.topLeft,
    widthFactor: axis == Axis.horizontal ? value : null,   // ← 尺寸
    heightFactor: axis == Axis.vertical ? value : null,
    child: panel,                                            // ← 同一个 child 实例
  ),
);
```

关键在 `child: child` **被传进 `TweenAnimationBuilder` 的 `child` 参数**——
它是同一个 Element，**折叠再展开时子树不重建**：滚动位置、输入框焦点、
音频播放状态全都活着。头注写的正是这句：

> `/// Keeps one live child through reversals: scroll, focus and player state survive.`

**为什么适合我们**：本项目对「舞台保活」已经有硬约束与守卫
（`test/stage_keepalive_test.dart`），但**设置面板自己**没有——现在
`app_shell.dart` 里 `if (settingsOpen && inlineSettings) Positioned.fill(...)`
是**条件插入**，关掉再打开 = 面板重建 = 滚动位置回到顶部、分区草稿状态重置。
换成 `CollapsiblePanel` 后：设置面板**滑出而不是消失**，且**回来时还在原位**。

#### R4 `SettingsPageTransition`：两页**不重叠**的交叉淡化

`lib/settings_page_transition.dart:56-104` 的要点有两个：

```dart
final settingsVisible = motion.value >= .5;            // ← 过半才换树
final contentOpacity = 1 - Curves.easeInCubic.transform((motion.value * 2).clamp(0.0, 1.0));
final settingsOpacity = Curves.easeOutCubic.transform((motion.value * 2 - 1).clamp(0.0, 1.0));
```

1. **前一半只淡出旧页、后一半才淡入新页**（两条曲线各占 `t∈[0,.5]` / `[.5,1]`），
   所以两页**永远不同时可见**，不会出现「两个页面叠在一起」的花屏；
2. 位移只有 ±12 px，配合 `IgnorePointer(ignoring: motion.isAnimating)` 防止
   动画中点穿。

搬这条的收益：本项目 compact 断点的设置是
「抽屉（第 1 层）→ 底部浮层（第 2 层）」两步跳（`lib/app/nav_host.dart:114-151`），
**两步之间没有任何过渡**，观感上像「弹了两次」。Morrow 的 compact 是
**页内过渡**（`main.dart:899-905`，`showSettings: !desktop && compactSettingsOpen`），
一次过渡到位。

---

## 3. 现状差距映射（审计编号 → 本期处置）

下表左列是本仓库 2026-09-11 前端审计的编号（A–M），右列是本计划的落点：

| 审计项 | 内容 | Morrow 对应资产 | 本计划处置 |
| --- | --- | --- | --- |
| **A** | `ui/display_panel.dart`(203) + `ui/glass_panel.dart`(82) 死代码 | — | **P0-1 删除**（顺带消掉仅存的裸内边距） |
| **B** | 舞台**双错误覆盖层 / 双加载指示器**（`live2d_stage.dart:265-268` vs `stage_host.dart:95-98`） | — | **P0-2 合并**（用户可见，最高优先） |
| **C** | 死参数 / 死令牌；`AppDurations.base/slow/reveal` 与 `Motion.enter` 零接线 | R3/R4/R5/R8 | **P1 接线**（§4） |
| **D** | 令牌门禁**假阴性**（`design_tokens_test.dart:130-155` 按字面量统计，真实写法是 `appColorsOf(context).hairline`） | — | **P4-1 修门禁**（否则 P1 做完也检不出来） |
| **E** | 绕过设计层的写死值（`field_row.dart:294/364` 的 `OutlineInputBorder`、多处 width/height） | — | **P4-2**：扩扫描规则 + 逐个归位到 `Space` |
| **F** | 数值真源重复（`actions_section.dart:95-103` vs `display_prefs.dart:116-124`） | — | **P0-3**：改读 `DisplayPrefs` 常量 |
| **G** | 警示样式两套、重试按钮三档混用 | `SoftSwap` 统一切换动效 | **P1-5** 统一到一个 `InlineNotice` |
| **H** | 聊天气泡不渲染 Markdown，`maxWidth: 460` 大于面板宽（`message_bubble.dart:78`） | `flutter_markdown_plus`（**不引入**） | **P2-3**：轻量渲染（粗体/行内码/列表）+ 复制按钮；**不引依赖** |
| **I** | 无障碍：`focusRing` 未接线、无焦点样式、textScaler 无测试 | R1 的 `highContrastOf` 用法 | **P1-4 + P4-3** |
| **J** | 400 px 内联侧板里塞 8 个 chip 会换 3 行（`settings_scaffold.dart:151-165`）；真实布局从未被测 | R10（compact 设置页） | **P2-1** 按断点换导航形态 + 补真实布局测试 |
| **K** | 内联结果（`_llmTest` 等）切分区后不清理，会显示过期结论 | `SoftSwap` | **P2-2** |
| **L** | 无 golden / 无 integration，12 个浏览器 bug 逃过全绿测试 | **R9 视觉审查工具** | **P4-4**（本期最高性价比的工程投入） |
| **M** | 主题切换：`MaterialApp` lerp 200 ms 而舞台底**立即**跳变 | **R8 材质插值** | **P1-3** |

---

## 4. 分期计划

排序原则：**先修用户看得见的错（P0）→ 再补观感（P1/P2）→ 再动材质（P3）→ 最后补工程**。
每期都能独立提交、独立回滚。

### P0 · 纠错与清理（半天，零观感风险）

| # | 改动 | 文件 | 验收 |
| --- | --- | --- | --- |
| P0-1 | 删除死代码 285 行 | 删 `lib/ui/display_panel.dart`、`lib/ui/glass_panel.dart`；同步 `test/design_tokens_lint_test.dart:172` 的字符串断言 | `flutter analyze` 0 issue；`flutter test` ≥627 |
| P0-2 | **合并舞台双覆盖层**：错误/加载只保留一层，文案统一 | `lib/live2d/live2d_stage.dart:260-274, 357-364` 删本地覆盖层；`lib/ui/stage_host.dart:91-104` 保留唯一实现 | 新增 `test/stage_overlay_single_test.dart`：断言舞台上 `重试` 按钮**恰好 1 个**、`加载中` 文案**恰好 1 处** |
| P0-3 | 数值真源合一 | `lib/settings/sections/actions_section.dart:95-103` 改读 `DisplayPrefs.minScale/maxScale/min|maxMouthSensitivity` | 新增断言：`actions_section` 不再出现字面量 `0.5/2.0/0.2/3.0` |

### P1 · 动效接线（2–3 天，把 4 档令牌真的用起来）

**这一期是本计划的重点**：不改信息架构，只让已有控件「会动」。

| # | 改动 | 借用的 Morrow 资产 | 令牌映射 |
| --- | --- | --- | --- |
| P1-1 | 设置面板**滑出/滑回**（expanded 内联侧板），且**关掉再打开保留滚动位置与分区** | **R3 `CollapsiblePanel`** | `AppDurations.slow`(320) + `Motion.state` |
| P1-2 | 离线横幅**滑入**、消息**渐入**、状态胶囊切换 | R5 `SoftSwap` | `AppDurations.base` + `Motion.enter`（入场） |
| P1-3 | **主题切换的舞台底色不再瞬跳**：把 `stageColor` 下发放进和 UI 同一个插值时间轴 | **R8 材质插值**（`TweenAnimationBuilder<GlassMaterial>` 思路） | `AppDurations.slow`(320) + `Motion.state` |
| P1-4 | 键盘焦点可见：接上 `AppColors.focusRing`（1 px 不透明边框，不用阴影，保证对比度可测） | R1 的 `highContrastOf` 兼容写法 | `AppDurations.fast`(120) |
| P1-5 | 统一「行内提示 + 重试」为一个组件（消掉 G 项的三档混用与 `⚠` 字面文本） | R5 `SoftSwap` | `AppDurations.base` |

**守卫测试**（新增）：
- `test/motion_wiring_test.dart`：扫描 `lib/**`，断言
  `AppDurations.base/slow/reveal` 与 `Motion.enter` **引用数 > 0**（现在为 0），
  且 `AnimatedContainer/AnimatedSwitcher/TweenAnimationBuilder` 处**不得**出现裸
  `Duration(milliseconds:`（改由令牌提供）；
- 断言所有新动效都读 `MediaQuery.disableAnimationsOf`（或经统一 helper），
  对应规格 §9.4 的「免疫漏网」两条；
- `test/settings_panel_keepalive_test.dart`：用 `initState` 计数钉死
  「关掉设置再打开，面板子树**不重建**、滚动位置保留」（照抄
  `stage_keepalive_test.dart` 的手法）。

### P2 · 面板形态与信息架构（3–5 天）

| # | 改动 | 借用的 Morrow 资产 |
| --- | --- | --- |
| P2-1 | **compact 设置改为页内过渡**（取代「抽屉 → 浮层」两次弹出）；导航形态按断点换：expanded=面板内 8 项列表，compact=顶部横向可折叠条 | **R4 `SettingsPageTransition`** + **R10** |
| P2-2 | 内联结果（LLM/TTS 测试、导入结果）**切分区即清**；结果出现/消失用 `SoftSwap` | R5 |
| P2-3 | 消息气泡：轻量 Markdown（`**粗体**`、行内码、`- 列表`）+ 复制按钮 + 修复失效的 `maxWidth: 460` | R5（切换动效） |

> P2-1 是本计划**最需要裁决**的一期（§7 裁决点 ③），因为它会动
> 「设置入口唯一」与「8 个分区」的既有交互裁决。默认建议：**只动 compact**，
> medium/expanded 保持现状。

#### P2-1 的具体配方（照抄 Morrow 已验证的参数，不即兴发挥）

Morrow 的整套「设置页切换」只有 **105 行、零控制器（一个 `AnimationController`）、
零依赖**（`lib/settings_page_transition.dart`），参数如下：

| 要素 | 取值 | 出处 |
| --- | --- | --- |
| 时长 | **320 ms**（本项目映射到 `AppDurations.slow`） | `main.dart:901-903` |
| 时间轴切分 | 前 50% 只淡出 A，后 50% 只淡入 B | `settings_page_transition.dart:58-66` |
| 可见性硬开关 | `settingsVisible = motion.value >= 0.5` | 同上 |
| 曲线 | 出 `easeInCubic` / 入 `easeOutCubic` | 同上 |
| 位移 | 各 **±12 px** 水平，方向相反；**没有 scale** | `:80-82, 95-97` |
| 防误触 | 两侧都套 `IgnorePointer(ignoring: motion.isAnimating)` | `:75-76, 90-91` |
| 保活 | `Offstage`（不 paint 但 element 还在）+ `ExcludeFocus` + `TickerMode` | `:67-79` |
| 首帧 | 控制器初值直接 `showSettings ? 1 : 0` ⇒ **首帧不播动画** | `:26-34` |
| 减少动画 | `Duration.zero` 时 `motion.value = 目标值`（**立即 snap 到终态**，不是不动） | `:37-49` |

最后一行尤其值得抄：它让「动画播到一半时系统打开减少动画」也能**立刻收敛到终态**
（Morrow 在 `compact_settings_test.dart:201-239` 里正是这么测的）。

### P3 · 材质与「像不像真玻璃」（可选，先做原型再决定）

| # | 改动 | 说明 |
| --- | --- | --- |
| P3-1 | **`LiquidRimPainter` 上到三个面**：内联设置侧板、底部浮层、舞台角标 | 纯 `CustomPaint`，**零 `BackdropFilter`**；四套主题各给一个 `intensity` 与 tint |
| P3-2 | 把 `glassScrim / glassBarrier` 两个**已声明但零引用**的令牌接上 | 现在它们是死令牌（审计 C 项） |
| P3-3 | **（探索）** 真·折射放到**渲染面**里做：玻璃的 backbuffer 是我们自己的 canvas，没有平台视图问题 | 渲染器已有离屏 FBO 通路（见 `docs/architecture/mask-premake-algorithm.md`），可在后处理 pass 里复刻 Morrow `shaders/liquid_glass.frag` 的 SDF 圆角 + 法线偏移（`lib/liquid_glass.dart:148-167` 的 `u_size/u_radius/u_depth` 三个 uniform）。**先出可行性原型，不排期** |

**红线**：P3-1 只允许 `CustomPaint`；任何方案若在舞台区域引入
`BackdropFilter`/`ImageFilter.blur`，直接否决（`test/no_backdrop_filter_test.dart` 会红）。

#### P3-1 的落地形状：复用它**已经验证过的**逃生口，而不是自己发明

Morrow 的 `LiquidGlassSurface` 里有一个开关（`lib/liquid_glass.dart:106`）：

```dart
bool get _filterEnabled => !widget.canvas || !widget.transparentCanvas;
```

`_filterEnabled == false` 时，`BackdropFilter` 那一整块 `Positioned.fill`
**根本不进 widget 树**（`lib/liquid_glass.dart:206-218`），只剩 `widget.child`
和 `LiquidRimPainter`——**这正是我们要的分支**。它的原始用途是「透明画布
不能采样桌面」，而我们要的是「舞台区域不能采样平台视图」，**同一类问题、
同一个开关**。

所以 P3-1 的落地方式确定为：

1. 把 `LiquidRimPainter`（`lib/liquid_glass.dart:248-313`）+ `GlassMaterial`
   （`lib/liquid_glass.dart:4-68`）搬成 `lib/ui/glass_rim.dart`，
   **不搬 `LiquidGlassSurface` 里的 `BackdropFilter` 那一支**；
2. 新组件的参数只有一个布尔：`overPlatformView`（默认 `true`，因为在我们的
   外壳里**任何**压在舞台上的面板都满足这个条件）——为 `true` 时永远走
   「纯描边 + 边缘光」；
3. 顺带把口径钉进测试：`test/no_backdrop_filter_test.dart` 增加一条
   「`lib/ui/glass_rim.dart` 内 `BackdropFilter` 命中数 == 0」。

> 为什么值得抄它的**参数表**而不是自己调：Morrow 这套 rim 是三层叠加
> （1.6 px 四段渐变 / 3 px 内缩 2 px 柔光 / 0.65 px 10% 白），
> 并且把「指针位置」用 180 ms `easeOutCubic` 追上（`lib/liquid_glass.dart:220-239`），
> 这些数字是被它的像素级测试（`test/transparent_liquid_test.dart:60-68`
> 断言中心 alpha == 0、上边缘 alpha > 0）钉过的，比我们从零调参便宜。

#### P3-3 的结论：**不做**（2026-09-11 实地核查后判定，附证据）

计划原文给 P3-3 设了先决条件：「先确认，再动手」「这条件不满足就不做 P3-3」。
核查完了，**结论是不做**，理由有三条，都是代码级事实：

1. **渲染面根本不知道「面板」在哪。** 玻璃面板是 Flutter 的 widget，画在
   `<iframe>` **之上**；而 iframe 里的 canvas 只知道自己的模型。渲染面要
   「在面板边缘做折射」，必须先由 Flutter 把面板的矩形与圆角发给它——
   那是协议新增字段 + 一套坐标同步（还要处理窗口缩放、DPR、边框）。
2. **实时路径直出交换链，没有中间靶标。** `ModelRendererCore::
   render_to_view_submit`（`crates/l2d/src/renderer/model_core.rs:144-176`）
   把模型**直接**渲染到 surface view（`LoadOp::Clear(TRANSPARENT)`），
   全仓**没有任何 `.wgsl` 文件**。做折射等于新增：中间纹理 + 全屏 pass +
   新 WGSL + 尺寸变更时的重建 + 一个新的 `ModelRendererCore` 入口，
   再经 wasm-bindgen 把 uniform 透出去。这不是「原型」的量级。
3. **它需要每帧级别的指针位置。** 而本项目有一条硬纪律：
   「高频信号永不进入 Widget 树」（规格 §1 P6）。把鼠标位置每帧发给渲染面
   虽然不进 Widget 树，但要新增一条**高频协议通道**及其节流策略——
   这是协议层的改动，不该塞进一个「前端加强」的探索项里。

**顺带记一条被否掉的替代方案**（免得以后有人重新发明）：理论上可以让
渲染面**半透明**地折射整个舞台边缘（不需要知道面板位置）。但那样得到的是
「舞台四周一圈玻璃」，不是「设置面板是一块玻璃」——与本项目「舞台中央不放
贴图、四套主题纯色底」的既有裁决也不合。**不做。**

**要做的话，正确的前置立项是**：① 在 `sync` 协议里定一个「折射区域」字段
（矩形 + 圆角 + 强度，带版本与缺省值）；② 渲染面加一个可开关的后处理 pass；
③ 指针位置走一条新的高频通道。三条都不属于本轮。

#### P3-3 的先决条件（原始记录，保留以便复核）

Morrow 的着色器在 **Windows / Web 上本来就跑不到**：
`ui.ImageFilter.isShaderFilterSupported` 是 Impeller 专属
（`lib/liquid_glass.dart:112`），Windows/Web 的 Skia/CanvasKit 走的是
矩阵放大回退（`lib/liquid_glass.dart:154-162`）。也就是说：

- **它的「液体折射」观感有一半本来就来自 rim 高光，不是折射** ——
  这也解释了为什么 P3-1（只搬 rim）性价比最高；
- 我们要做 P3-3，等于**在一个它做不到的平台上做它做不到的事**，
  所以它只能当算法参考（SDF 圆角 + 法线偏移，`shaders/liquid_glass.frag:7-23`），
  实现必须落在我们自己的渲染面里，**并且先验证后处理 pass 的代价**。
  这条件不满足就不做 P3-3。

### P4 · 工程质量（2 天，回报最高）

| # | 改动 | 借用的 Morrow 资产 |
| --- | --- | --- |
| P4-1 | **修令牌门禁假阴性**：`countTokenReferences` 要认 `appColorsOf(context).hairline` 这类真实写法（`test/design_tokens_test.dart:63-79`） | — |
| P4-2 | 裸值扫描扩到**间距/宽高**（`Space.*` 之外的 `EdgeInsets`/`SizedBox` 数字），并逐个归位 | — |
| P4-3 | `textScaler` 用例：1.0 / 1.3 / 1.5 / 2.0 下三档断点不裁切 | — |
| P4-4 | **视觉审查工具**：`flutter test` 里把外壳在 3 档断点 × 4 套主题渲染成 PNG，落到 `build/visual-review/`，供人眼复核 | **R9 `tool/feature_visual_test.dart`** |
| P4-5 | 修文档过期处（见 §5.3） | — |
| P4-6 | **把 Morrow 的五组断言模板搬成回归**（见下） | **R16 `test/compact_settings_test.dart:149-199`** |

P4-4 的价值最直接：它把「测试全绿 ≠ 界面是对的」这条已经付过学费的教训
（12 个浏览器 bug 逃过 810+590 条测试）变成一个**每次改前端都能跑的产物**。
Morrow 的 `tool/feature_visual_test.dart` 已经证明这条路可行：
它用 `FontLoader` 加载本机字体、`tester.view.physicalSize` 定尺寸、
`RenderRepaintBoundary.toImage()` 出 PNG，全程不依赖浏览器。

P4-6 抄的是 Morrow 那份 **240 行测试**里的五组断言（这些断言比它的实现更值钱）：

| 组 | Morrow 的断言 | 我们对应要钉的 |
| --- | --- | --- |
| 互斥性 | 设置打开时，内容页 / 搜索框 / footer 三者**同时** `findsNothing` | 设置打开时舞台缩控、聊天输入框不得同时可交互 |
| 跨断点状态保留 | 打开设置→返回后，搜索文本保留、滚动偏差 `±0.1 px` | 面板开合后聊天滚动位置、草稿保留 |
| 双向 + 反向打断 | 打开中途 `pump` 到 80/160 ms 采样，任意帧**两页不同时存在** | 面板滑出中途反向，不得出现两层面板 |
| 中途 pop / Esc | 动画进行中 `handlePopRoute()` 必须回到工作台 | Esc / 关闭按钮在动画中的行为 |
| 减少动画收敛 | 80 ms 时注入 `disableAnimations: true`，断言 opacity **直接 == 1** | 我们的新动效在 reduced-motion 下立即到终态 |


---

## 5. 与既有约束的对齐（红线）

### 5.1 硬红线（违反即否决）

| 红线 | 出处 | 与 Morrow 的冲突点 | 本计划如何遵守 |
| --- | --- | --- | --- |
| **禁用 `BackdropFilter` / `ImageFilter.blur`** | 规格 §12-7；`test/no_backdrop_filter_test.dart` | Morrow 的整套玻璃都建立在它上面（全仓仅 1 个构造点 `lib/liquid_glass.dart:209`，每个 `Glass` 实例一个，宽屏一帧同见 6–10 个） | 只搬 `LiquidRimPainter`，并复用它自己的 `_filterEnabled` 逃生口（`lib/liquid_glass.dart:106`）→ 新组件加 `overPlatformView` 强制走「只画 rim」分支（P3-1）；真折射放渲染面（P3-3） |
| **舞台必须保活** | 规格 §0.1 C5；`test/stage_keepalive_test.dart` | Morrow 的 `SettingsPageTransition` 用 `Offstage` 换树 | 只用于**设置页之间**，舞台永远在 `Stack` 底层不换树 |
| **压在舞台上的控件必须套 `StagePointerInterceptor`** | `AGENTS.md` 前端层 | Morrow 没有平台视图，无此问题 | P1/P2 每个新浮层逐个接线，并在 `stage_pointer_interceptor_test.dart` 登记 |
| **零新增运行时依赖** | 规格 §0.1 C2 | Morrow 有 20+ 包 | **一个都不引入**（Markdown 自己写轻量渲染，不引 `flutter_markdown_plus`） |
| **零裸色值 / 裸字号 / 裸圆角 / 裸断点**；动效只用 4 档 + 2 曲线 | 规格 §2.7/§2.8 | Morrow 有 11 个时长、5 个断点 | §4 的映射表；`motion_wiring_test` 新增裸 `Duration` 扫描 |
| **中文正文 ≤12 px 下限、`contentFaint` 不承载文字** | 规格 §9.5 | Morrow 部分小字更小 | 搬结构不改字号 |
| **离线优先**（无 CDN、字体自托管） | `AGENTS.md` 前端层 | Morrow 用系统字体（`Segoe UI`/`Microsoft YaHei`） | 继续用 `NotoSansSC` 子集，不动字体 |
| **只有 `/app/` 一个界面入口** | `AGENTS.md` | — | 不新增入口 |

### 5.2 需要一起改掉的既有问题（P0 已列，此处汇总理由）

- **双覆盖层**（B）：两个 `_StageErrorOverlay` 会同时出现在舞台上、两个「重试」按钮。
  这不是「风格问题」，是**用户会看到两个按钮**。
- **令牌门禁假阴性**（D）：不修的话，P1 接线之后 `AppColors` 的台账仍然是 stale 的，
  下一次「没接线」还会隐形——**先修尺子，再量东西**。

### 5.3 文档过期处（P4-5 一并修）

1. `docs/design/web-ui-spec-v3.md` §12-6「亮色主题：不做」与 §13.13「四套配色」冲突；
2. 同文件 §13.15.4 仍在描述已删除的 `SectionRail`；
3. `docs/verification/flutter-shell-manual-checklist.md` §17 仍写「rail 上再点同一分区能收起」；
4. `shell/README.md` §8.3 称「界面尚未把 `source` 表达出来」，但代码已接线。

---

## 6. 不做清单（明确排除）

以下引导自 Morrow，但**本计划不做**，理由均与既有用户裁决或项目范围一致：

| 不做 | 理由 |
| --- | --- |
| 桌面壳（自绘标题栏 / 窗体圆角 / 托盘 / 全局快捷键） | 属路线图 R3「Tauri 桌宠壳」，**要引入运行时依赖**；与「只核心链路做丰满」的裁决冲突（`docs/research/neko-ui-alignment-and-gaps.md` §3.1 P0-1） |
| 附件系统 / 剪贴板富文本 / Office 解析 / 随身听 / 歌词联网 | 功能堆砌；用户裁决「特意不做类似功能堆砌」（`spec-v3` §13.2②） |
| 光晕背景 `AmbientPainter` 铺到舞台 | 与用户裁决「舞台背影全黑/全白即可，**中央不要放舞台贴图**」直接冲突（§7 裁决点 ①） |
| 色彩罗盘 / 自定义主题色 | 现为黑/白/蓝/灰四套固定配色（用户裁决）；只有要「自定义」时才需要（§7 裁决点 ②） |
| 玻璃材质三档（磨砂/超透/液体）选择器 | 本项目不是「材质工作台」；材质观感只做**边缘光**这一层（P3） |
| 完整 Markdown 渲染 / 代码高亮 / 逐字打字机 | 与「整句合成完成 → 播放」的时序裁决冲突（`spec-v3` §12-1） |

---

## 7. 需要用户裁决的 5 个点

| # | 问题 | 备选 | 建议 |
| --- | --- | --- | --- |
| ① | 舞台区域是否允许**非纯色**的氛围底（如 Morrow 的径向光晕）？ | A. 保持纯色（现状）／B. 允许极低对比度的氛围光／C. 只允许在**设置面板**与启动首帧用光晕 | **A**（维持既有裁决）；若要 B/C 需明确改判 |
| ② | 是否要「自定义主题色」（色彩罗盘）？ | A. 不加／B. 在四套主题之外加第五套「自定义」 | **A**（保持四套，减少变量） |
| ③ | compact 设置是否改为**页内过渡**（取代「抽屉→浮层」两次弹出）？ | A. 改（推荐，观感一次到位）／B. 保持两步法 | **A**，但仅限 compact |
| ④ | P3-3 渲染面折射原型是否要做？ | A. 先出原型再定／B. 不做 | **A**（成本可控，且是这个产品唯一的「独占观感」） |
| ⑤ | 是否允许把 Morrow 的代码/思路写进 `CREDITS.md` 并在仓库内保留这几张参考截图？ | A. 保留并署名（Apache-2.0 要求保留 NOTICE）／B. 只留文字不留图 | **A**（截图已落在 `docs/design/assets/morrow-reference-2026-09-11/`） |

### 7.1 许可与署名

Morrow 是 **Apache-2.0**（`LICENSE` + `NOTICE`：`Copyright 2026 StarrySky7D4 and contributors`）。
Apache-2.0 允许商用与修改，义务是：**保留版权与许可声明、标注修改**。
本计划若落地 R1/R3/R4/R5 的代码，需要：

1. 在 `CREDITS.md` 增加一节「前端观感参考：Morrow」，
   写明仓库地址、commit、以及**我们改了什么**（令牌化、去 `BackdropFilter`、
   接入本项目 reduced-motion 出口）；
2. 在受影响的源文件头注写明来源（`AGENTS.md` 风格的「为什么」注释本来就是本项目的惯例）；
3. **不复制** Morrow 的第三方依赖代码（`third_party/um_decrypt` 等一律不碰）。

---

## 8. 执行顺序与验收总表

```text
P0（半天）─┬─ 删死代码 ─ 合并双覆盖层 ─ 数值真源合一
           └─ 验收：analyze 0 issue，test ≥ 627，新增 2 条守卫

P1（2–3 天）─┬─ 面板滑出保活 ─ 横幅/消息渐入 ─ 主题切换不瞬跳 ─ 焦点环 ─ 提示统一
             └─ 验收：test ≥ 627 + motion_wiring / panel_keepalive 全绿

P2（3–5 天）─┬─ compact 设置页内过渡 ─ 内联结果清理 ─ 气泡 Markdown 轻量渲染
             └─ 验收：新增真实布局测试（真 8 分区、无桩 pane）

P3（原型）── LiquidRimPainter 上三个面 + 渲染面折射可行性
             └─ 验收：no_backdrop_filter_test 仍全绿；人工看图

P4（2 天）──┬─ 修令牌门禁假阴性 ─ 裸值扫描扩容 ─ textScaler 用例
            ├─ 视觉审查工具出 3×4 张 PNG（三档断点 × 四套主题）
            └─ 五组断言模板落地（互斥/保活/打断/pop/reduced-motion 收敛）
```

**每期结束必须跑**（本机可直接执行）：

```bash
cd shell/flutter && export PATH="$HOME/flutter/bin:$PATH"
LIVE2D_AI_MUTE_AUDIO=1 flutter analyze
LIVE2D_AI_MUTE_AUDIO=1 flutter test
```

改了 `shell/flutter/**` 之后**必须重新构建**才在 `/app/` 生效：

```bash
flutter build web --release --base-href /app/ --no-web-resources-cdn
```

（`--no-web-resources-cdn` 不是可选项：缺省会把 CanvasKit 指向 `gstatic.com`，断网即白屏。）

---

## 9. 变更历史

- **2026-09-11**：首版。依据 = ① Morrow 源码静态分析（commit `a3c1766`，
  三个方向并行深读：外壳/响应式/动效、材质/主题/贴图、设计文档/测试/许可）；
  ② Morrow Windows 版**实机运行截图** + 调色板与几何实测
  （`docs/design/assets/morrow-reference-2026-09-11/`）；
  ③ 本项目前端审计（A–M 项）与在制前端事项调研（U/B/F 编号）；
  ④ 本项目 `flutter test` 基线实测 **627 passed**
  （Flutter 3.47.3 / Dart 3.13.3，本机 `~/flutter/bin`）。
  **本计划尚未执行任何代码改动。**
- **2026-09-11（同日，实施）**：用户答复「同意 开始实施」，按 §7 的推荐口径
  执行（裁决点 ①A / ②A / ③A / ④A / ⑤A）。P0 与 P1 一期完成、P4-1 提前完成；
  门禁 627 → **657 通过**。实施期新发现并记录了三件事（见文首「实施进度」）。

  > 本轮调研推翻了两条常见误判，已写入正文：
  > ① Morrow 的「液体玻璃」在 Windows/Web 上**本来就跑不到逐像素折射**
  > （`ui.ImageFilter.isShaderFilterSupported` 是 Impeller 专属，
  > `lib/liquid_glass.dart:112`），它有一半观感来自 rim 高光 —— 所以 P3-1 才是重点；
  > ② Morrow 的 Flutter Web **中文缺字问题它自己没修**
  > （`main.dart:264-265` + `pubspec.yaml` 无 `fonts:` + 仓库无字体文件），
  > 这恰是本项目已解决的坑，**它是反例不是正例**（§0.3）。
