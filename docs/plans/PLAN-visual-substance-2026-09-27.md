# 前端观感第二轮：补「材质本体」（参考 Morrow，2026-09-27）

> **来源（2026-09-28 入库）**：本文原为旧 worktree `/home/skystar/Live2D-Ai` 的**未跟踪唯一副本**，
> 2026-09-28 判定**并入 0.2.0 线**（`docs/plans/`）。它是 09-27 观感二轮的**依据**，实施已随
> Stage A 重放落地；文中的「853 测试 / analyze 0」是**当时**数字，不是现行门禁值（现行为 flutter 1272）。
> 配套视觉资产在 `docs/design/assets/visual-substance-2026-09-27/`（含 `measure_chroma.py`，接验收为 future work）。

> **状态（2026-09-27）**：R1 / R2 / R3 / R4 / E 五项**全部已实施**，
> `flutter analyze` 0 issue、`flutter test` **853** 通过、`ignite.sh --check` 四项全 ok、
> web 产物已重建。实施后的量化复测见 §6。
> 仍**未**做：发布说明（`docs/releases/`）与 `AGENTS.md` 变更历史条目——
> 等你确认观感之后再决定要不要切版本线。

## 0. 结论先行

上一轮（`PLAN-frontend-strengthening-2026-09-11.md`）已经把 **机制** 搬完了
（动效令牌接线、面板形态、边缘高光、交叉过渡），P0–P4 全部 ✅。
**剩下的差距不在机制，在材质本体**——具体是三件事，都可以用数字钉死：

| # | 差距 | 证据（暗色主题实测） | 代价 |
| --- | --- | --- | --- |
| **A** | **界面几乎没有颜色** | 色度均值 **4.6/255** vs Morrow **13.3/255**（亮色：2.8 vs 8.6） | 改 `tokens.dart` 一个函数，~30 行 |
| **B** | **表面阶梯只有 1.5 级** | 67.5% 的像素挤在同一个 4 级亮度桶里；Morrow 分布在 **5 个连续桶** | 改 4 个 palette 的 3 个字段，~40 行 |
| **C** | **唯一的主题旋钮是「颜色」** | 我们的定制面 = 配色（4 套）；Morrow = 配色 **×** 材质（圆角幅度 / 描边强度 / 表面色温） | 加 2 个偏好字段 + 一处派生，~120 行 |

三件事**都不需要抄它的玻璃**——那部分 2026-09-11 已经判定不可达（舞台是平台视图，
`test/no_backdrop_filter_test.dart` 守着命中数为 0），本轮维持原判。

---

## 1. 取证方法（都是可复现的，不是「看一眼觉得」）

### 1.1 源码

- 参考项目 `/home/skystar/morrow` 是 Windows 预编译包（`morrow_studio.exe` + `data/app.so`，
  `SOURCE.txt` 记 **v0.1.9-test.55 / commit `c04ae14`**），上游源码在
  `github.com/StarrySky7D4/morrow`（**AGPL-3.0-only**），已浅克隆到 `/tmp/morrow-src` 逐文件读
  （克隆到的是 `main` 分支 `d9c0431`，比本机那个包新；下文行号均以该 HEAD 为准）。
- 在线 Web 预览 `https://starrysky7d4.github.io/morrow/` 可直接跑，已用它取到**运行态**的
  外观面板结构（开启 a11y 语义树后读 `flt-semantics`）：

  ```
  空间外观 / 打造你的风格
    界面风格: 扁平 · 默认          ← 7 种风格之一（可展开）
    界面语言  跟随系统
    字体
    玻璃质感  磨砂 / 超透 / 液体玻璃
    磨砂不透明度  [====] 76%    20% · 轻盈 ┊ 100% · 纯粹
    组件与卡片 · 独立设置
    主题色调  白色 / 自定义 / 深色
    主题色彩色罗盘        恢复默认
    圆角幅度  [====] 20 / 32   拖至 0 即为方角
    背景画布  默认 / 纯色 / 纹理 / 透明
    液体玻璃效果 [开]   独立于背景类型，四种画布均可开启
  ```

  **对照我们自己的外观分区**：只有「配色（四选一）+ 舞台背景图 + 壳背景 + 舞台与口型 +
  互动」——**全部是「开关/数值」，没有一项是「外观/材质」的旋钮**。

### 1.2 像素量化（唯一可信的「像不像」判据）

两个应用同尺寸（1440×900，DPR 1.5）跑起来截图，对**同一块 chrome 区域**做统计
（工具与四张原图见 `docs/design/assets/visual-substance-2026-09-27/`，可复现）：

| 指标 | 我们·暗 | Morrow·暗 | 我们·亮 | Morrow·亮 |
| --- | --- | --- | --- | --- |
| 色度 chroma 均值（0–255） | **4.6** | **13.3** | **2.8** | **8.6** |
| 偏离中性灰（0–255） | 1.7 | 5.0 | 1.0 | 3.4 |
| chroma > 6 的像素占比 | **8.5%** | **94.4%** | 4.3% | 43.9% |
| 亮度 p10 / p50 / p90 | .000 / .056 / .846 | .140 / .164 / .217 | .914 / .957 / 1.000 | .899 / .968 / .990 |
| 最大亮度桶占比 | **67.5%**（12–15 一档） | 28.5%（40–43） | 68.1%（244–247） | 31.0%（248–251） |
| 有效亮度档位 | ~3 | **~6** | ~3 | **~6** |

> ⚠️ 口径说明：HSV 饱和度在近黑像素上会爆炸（`#0E0E11` 的 (max−min)/max = 0.176，
> 但绝对色差只有 3/255），所以表里一律用**绝对色度**与**偏离中性灰的绝对量**。
> 第一版用 HSV 饱和度得出的「Morrow 是我们的 4.5 倍」是虚高，作废。

**读出来的三件事：**

1. **我们的界面是「无色的」**。暗色下 91.5% 的像素色度 ≤6，颜色只来自文字和少数强调元件；
   亮色下 95.7% 的像素色度 ≤6。层级只能靠 1px 发丝线（`AppColors.hairline` = 墨色 12%/10%）。
2. **我们的表面阶梯是「一档 + 两档半」**。暗色 `surface #0E0E11`（亮度 0.056）→
   `surfaceAlt #17171B`（0.091），**级差 0.035**，落在同一个 4 级桶里；再加 `stage #000`。
   亮色同理：`#F4F4F6` / `#EAEAEE` / `#FFF`，p10–p90 亮度跨度只有 0.086。
   Morrow 是 `#181720`（背景）/ `#292634`（面）/ 叠加 tint，`#151517`–`#43454A` 连续 5 档。
3. **我们的暗色舞台是一整块 `#000`**：全屏 78% 像素亮度 < 0.08。
   舞台纯黑是用户裁决（`AGENTS.md` v0.5.0），**不动**——但它放大了问题：
   UI 侧如果也只有 0.056 亮度，两者之间就没有「材质差」，整个画面读作一个洞。

### 1.3 它真正起作用的那一行代码

`morrow-src/lib/appearance.dart:346`：

```dart
Color themeTint(Color color, [double amount = .08]) => ...
    : tone(themeColor == null ? color
          : Color.lerp(color, themeColor!.withValues(alpha: 1), amount)!);
```

**每个中性色都被混进 8% 的强调色相。** 这一行是它「像被设计过」与
我们「像默认配色」的分界线——而且它的整套色板（暗 `#181720` / `#292634` / `#F0EDF8` /
`#B4AEC5` / `#C0AFFA`）是**同一个紫色相家族**，不是中性灰。
我们四套 palette 的 `surface/surfaceAlt/ink/accent` 全部落在中性轴上（`#0E0E11`、
`#17171B`、`#F1F1F4`、`#E8E8EC`），所以再怎么调圆角和动效也不会有「材质感」。

---

## 2. 改进项（按「收益 ÷ 代价」排序）

### A · 给整套中性色加一个统一的色相偏移（最高性价比）

**改法**：在 `AppPalette` 上加一个派生函数，所有中性色在**构造期**混入固定比例的本套强调色相：

```dart
// lib/design/tokens.dart —— 新增
/// 本套配色的「材质色相」：把强调色的色相以 [tintAmount] 的比例压进所有中性色。
/// 舞台底**不参与**（它是渲染面 framebuffer 的纯色，插值后会出现彩边）。
Color tinted(Color neutral) =>
    Color.lerp(neutral, accent, dark ? 0.10 : 0.07)!;
```

`ink` / `surface` / `surfaceAlt` / `muted` 全部改走 `tinted(...)`，
`registry` 里的字面量**保持不变**（仍是唯一真源，token lint 不用改）。

**四套的色相锚点**（保持各自性格，只加一点点色相）：

| 配色 | 色相锚点 | 暗色 surface | 亮色 surface |
| --- | --- | --- | --- |
| 黑 | 冷靛（偏 `#8A90FF`） | `#12121A` | `#F2F2F7` |
| 白 | 暖白（偏 `#FFF6E8`） | — | `#F5F3EF` |
| 蓝 | 蓝紫（跟 `#4D8DFF` 一族） | `#0D1626` | `#EFF3FA` |
| 灰 | 中性偏暖（偏 `#C8C0B4`） | `#1E1D1B` | `#F1F0EE` |

**门禁**：
- 现有 `test/theme_palette_test.dart` 的 11 条对比度断言**必须仍然全绿**——
  暗色最紧的一条是 `ink vs surface`（现 17.1:1，加 10% 蓝相后约 16.4:1，余量充足）；
- **新增**一条「相邻表面亮度差 ≥ 0.03」的断言（防止有人把阶梯压平，这是本轮的病根）；
- **新增**一条「`stage` 必须仍是中性纯色（`r≈g≈b`）」的断言——拦住色相漏进舞台。

### B · 把表面阶梯从 1.5 级补到 3 级

**现状**：`stage(0.000) → surface(0.056) → surfaceAlt(0.091)`，级差 0.035。
**改法**：新增一档 `raised`（卡片 / sheet / dialog / snackbar / 输入框），
并把级差拉开到 **≥ 0.04 亮度**：

| 槽位 | 暗色（黑套） | 亮色（白套） | 谁在用 |
| --- | --- | --- | --- |
| `stage` | `#000000`（不动） | `#FFFFFF`（不动） | 渲染面 framebuffer |
| `surface` | `#0E0E11` → `#12121A` | `#F4F4F6` → `#F5F3EF` | 面板底、聊天面板 |
| `surfaceAlt` | `#17171B` → `#1E1E25` | `#EAEAEE` → `#ECEAE6` | 助手气泡、输入框 |
| **`raised`（新）** | **`#26262F`** | **`#E2E0DB`** | 卡片、浮层、snackbar |

三档的 gamma 亮度：暗 0.071 / 0.119 / 0.152（现状 0.056 / 0.091）；
亮 0.945 / 0.925 / 0.880（现状 0.957 / 0.918）。**级差都 ≥ 0.035**。

**代价与风险**：
- `AppPalette` 加字段 → 必须同步 `copyWith` / `lerp` / `toValuesMap` / `colorFieldCount`
  （`theme_palette_test.dart:175` 那条结构枚举会自动抓住漏改，这正是 P4 设计的用意）；
- rc.5 的 `ShellBackdrop`（壳背景 0.15 透明度，`ui/shell_backdrop.dart`）与
  `kShellSurfaceAlpha = 0.86`（`ui/chat_panel.dart:112`）需要重测一版，
  因为面板底抬高后「有背景时留一点透」的可读性会变。

### C · 材质正交轴：**只给 2 个旋钮**，不做 7 种风格

Morrow 的 `VisualStyle` 有 7 种（`flat/neumorphism/paper/clay/fluent/brutalist/industrial`），
**我们不抄**（违反 `§13.2②` 的「刻意不做功能堆砌」）。但它有两个**机制**很便宜：

1. `VisualStyle.radiusScale`（`appearance.dart:26`）——「一个旋钮缩放整套圆角，
   **不覆盖用户显式的方角设置**」；
2. `StyleDepthSlider`（0–200%）——「一个旋钮缩放整套描边/阴影的强度」。

**我们的版本**（加两个本地偏好，落在 `DisplayPrefs`）：

| 字段 | 范围 | 默认 | 作用 |
| --- | --- | --- | --- |
| `radiusScale` | 0.5 – 1.5 | 1.0 | 在 `buildAppTheme` 里乘到 `AppRadius` 全套 |
| `edgeStrength` | 0.0 – 1.5 | 1.0 | 乘到 `AppColors.hairline` 的 alpha |

**为什么值得**（三条，都是实测/源码依据）：
- 它把「整套界面偏方 / 偏圆、偏轻 / 偏实」从**逐控件改**变成**一次调**——
  这正是我们目前缺的：现在用户除了换 4 套颜色之外，对外观**没有任何表达**；
- 键鼠用户 vs 触屏用户对圆角/描边的偏好差别很大，而桌面 Web 端我们只服务前者；
- 实现面很小：`AppRadius` 的 7 个 const 改成在主题构造期乘一次，
  `hairline` 改成 `ink.withValues(alpha: base * edgeStrength)`。

**红线**：
- 圆角缩放**不得**让 `pill` 变成非胶囊、也不得让 `none` 变成圆角（`AppRadius` 的语义边界）；
- `edgeStrength = 0` 时发丝线会**完全消失** ⇒ 焦点环（`focusRing`）与危险描边
  **不参与**这个缩放（它们是 `palette.accent` / `palette.danger` 直出，不走 `hairline`）；
- 门禁：`design_tokens_lint_test.dart` 的「零裸值」扫描要认这两个派生量。

### D · 浮起面给一条真实的「抬起」提示（不是加回 Material 阴影）

`buildAppComponents` 现在把**所有** elevation 归零（`theme.dart` 全部 `0`），
这是对的选择（去 AI 味），但代价是**唯一分隔手段是发丝线**，而发丝线在 78% 近黑的画面里
几乎不可见（p99 色度 42 是文字，线本身 chroma ≈ 2）。

**改法**（只给「浮起」面，不给容器）：
`raised` 那几个面（dialog / bottomSheet / snackbar / 设置侧板）加一条
**顶边 1px 高光 + 下方 8–16px 的极低透明度阴影**，阴影色跟 `ThemeData.shadowColor`
已有的先例走（暗色用纯黑、亮色用淡墨 18%）。

**不做**：容器阴影、内阴影、Neumorphism 双向浮雕（`neumorphic_controls.dart` 1461 行，
且它的双向阴影在纯黑舞台上完全读不出来）。

### E · 设置面板的形态（这里 Morrow 明显比我们强）

**实测差异**（读运行态语义树）：

- 我们的字段行 = `标签 + 一整句说明 + 控件`，
  例：「壳背景 / 聊天与侧栏背后的整壳背景，固定 15% 透明度铺一层，不做透明度滑条；底色仍跟主题走。开启「与舞台同步」时与舞台共用同一张图，关掉才用壳自己的图。」
  ——**两句话、四个分句**，压在 12px 灰字上。
- Morrow 的每一组 = `小标题 + 一句价值描述 + 视觉控件`，
  例：「磨砂不透明度 / [====] 76% / 20% · 轻盈 ┊ 100% · 纯粹」——
  滑杆**两端带语义标签**，一眼知道两头是什么。

**低成本可做三条**：
1. **所有滑杆补两端语义标签**（我们的「模型缩放」「口型灵敏度」「磨砂…」右边只有一个 `100%`）；
2. **分区内容按「组卡片」重排**：组标题 + 一行说明 + 组内字段，减少每字段一句长说明的密度；
3. **枚举值用分段控件**（`渲染档位 4K/8K/16K` 现在是三个并排按钮）。

---

## 3. 明确不抄（维持 2026-09-11 的判定，并补两条新的）

| # | 不抄 | 理由 |
| --- | --- | --- |
| 1 | `BackdropFilter` / `liquid_glass.frag` 玻璃 | 舞台是 `<iframe>` 平台视图，模糊不到它；`test/no_backdrop_filter_test.dart` 守着命中数为 0。**Morrow 自己也知道到不了底**（它那个唯一的 `BackdropFilter` 被 `_filterEnabled` 挡着） |
| 2 | 7 种 `VisualStyle` 全家桶 | 违反「刻意不做功能堆砌」；且 7 套外观 × 4 套配色 = 28 种组合的回归面 |
| 3 | `ComponentMaterial` 组件级材质覆盖 + `followComponent` 跟随链 | 它自己都要防循环跟随（`inheritDepth` / `clearFollow` 一堆标志位）；我们的 P2「一个品牌色」与 P4「静默失效最危险」都反对 |
| 4 | 主题插件导入（`theme_plugins/ui_theme_tokens.dart`） | 我们是 4 套固定配色，导入就变成字符串查表，违反 `§2.1` |
| 5 | **（新）** 液体玻璃 / 流动光晕 shader | 与 1 同源，且它是**装饰性重绘**——我们的舞台是 30Hz 口型驱动的热路径（`AGENTS.md` P6），不能在主面板上叠逐像素 shader |
| 6 | **（新）** 把外观面板做成常驻第三栏 | 违反 P1「舞台是主角，UI 都是客人」；我们的设置入口只有一处是用户裁决 |

---

## 4. 合规修正（顺手，5 分钟，但**必须做**）

`CREDITS.md` §6.5 与 `lib/ui/glass_rim.dart` 头注**互相矛盾，且许可已变**：

| 位置 | 现在写的 | 事实 |
| --- | --- | --- |
| `CREDITS.md` §6.5 | 「License: Apache License 2.0」 | 上游 v0.1.9-test.55 的 `LICENSE` 是 **GNU AGPL-3.0-only** |
| `CREDITS.md` §6.5 | 「**截至 2026-09-11，本仓库未复制其任何源代码**」 | `glass_rim.dart:5-7` 明写「形状与参数来自 Morrow 的 `LiquidRimPainter`（`lib/liquid_glass.dart:248-313`）」，并照抄了外圈 0.8/1.6、中圈 2/3、内圈 5/0.65 三组数值 |
| `CREDITS.md` §6.5 | 取证 commit `a3c1766`（v0.1.9-test.1） | 本机跑的包是 v0.1.9-test.55（`c04ae14`）；上游 `main` 已到 `d9c0431`。**`glass_rim.dart` 头注引的 `liquid_glass.dart:248-313` 是 test.1 当时的行号，当前 HEAD 上 `LiquidRimPainter` 在 `:348`** |

> 顺带：`glass_rim.dart` 头注里那三个描边数值（外圈 0.8/1.6、中圈 2/3、内圈 5/0.65）
> 是**照抄参数**，属于需要按 AGPL 记录的事，不是「灵感参考」。

**处置**（本项目自身也是 AGPL-3.0，兼容；要做的是把记录改准）：
1. 许可改 **AGPL-3.0-only**，取证 commit 更新到 `c04ae14` / v0.1.9-test.55；
2. 「未复制源代码」改成**如实记录**：移植了 `LiquidRimPainter` 的三层描边**参数**
   （0.8/1.6、2/3、5/0.65）与渐变四段配比，未移植代码本体；
3. 按 **AGPL §5** 补记「被修改的内容」：描边改为三段常量、去掉 `BackdropFilter` 与 shader、
   颜色基色由写死白改为 `AppColors.rimHighlight`（四套主题各自可读）。

---

## 5. 建议实施顺序

| 阶段 | 内容 | 触及文件 | 门禁 |
| --- | --- | --- | --- |
| **R1** | 合规修正（§4） | `CREDITS.md` | 人工核对 |
| **R2** | 色相偏移 + 表面阶梯（A + B） | `design/tokens.dart`、`design/theme_id.dart` | `flutter analyze` + `flutter test`（`theme_palette_test` 加 2 条新断言） |
| **R3** | 浮起面高光（D） | `ui/theme.dart`、`ui/glass_rim.dart` | 同上 + `glass_rim_test` |
| **R4** | 材质旋钮（C） | `settings/display_prefs.dart`、`design/tokens.dart`、`ui/theme.dart`、`settings/sections/appearance_section.dart` | 同上 + 偏好往返测试 |
| **R5** | 设置面板形态（E） | `ui/field_row.dart`、`settings/sections/*.dart` | 同上 |

R2 是本轮的核心：只改一个文件、两处构造，但会同时改变四套主题的全部观感，
**必须配一次真机肉眼**（Win 浏览器开 `http://127.0.0.1:18080/app/`，
逐主题切一遍，看「舞台纯黑 ↔ UI 深灰」的材质差是否成立）。

---

## 6. 实施结果与复测（2026-09-27）

### 6.1 落地清单

| 项 | 落点 | 状态 |
| --- | --- | --- |
| **R1 合规** | `CREDITS.md` §6.5（许可改 AGPL-3.0-only、取证 commit 补全、按 §5 如实记录 `LiquidRimPainter` 参数取用与三处修改）、`lib/ui/glass_rim.dart` 头注同步 | ✅ |
| **R2 色相 + 阶梯** | `design/tokens.dart`：`AppPalette` 11 个颜色字段（四套全部换值）、新增 `raised`；`ui/theme.dart` 把 `raised` 接到 `ColorScheme.surfaceContainerHigh` + 卡片 / 弹窗 / 浮层 / snackbar / tooltip | ✅ |
| **R3 浮起** | `design/tokens.dart` 的 `appRaisedShadow(AppPalette)` + `ui/settings_scaffold.dart`（**我们自己构建**的面板才用；Material 那些继续 elevation 0） | ✅ |
| **R4 材质旋钮** | `design/tokens.dart` 的 `AppMaterial`；`settings/display_prefs.dart` 的 `radiusScale` / `edgeStrength`（含 clamp / JSON 往返 / 相等性）；`ui/theme.dart` 用 `r()` 缩放全套圆角、`AppColors.of(edgeStrength:)` 缩放 hairline；`main.dart` 接线；`settings/sections/appearance_section.dart` 两根滑杆 | ✅ |
| **E 面板形态** | `ui/field_row.dart` 的 `SliderField` 新增 `minLabel` / `maxLabel`（`null` 时布局逐像素不变）；四处滑杆补两端语义词：圆角「更方/更圆」、描边「轻盈/清晰」、缩放「更远/更近」、口型「克制/夸张」 | ✅ |

### 6.2 新增门禁（11 条）

- `theme_palette_test.dart`：三级面**感知亮度级差 ≥ 0.03**（4 套 × 1）、
  三级面**同色相家族且色度 ≥ 3/255**（4 套 × 1）、
  舞台底**逐字钉死**（防改中性色时顺手把舞台也推了）；
  `copyWith` 结构枚举补 `raised`（11 字段）。
- `theme_test.dart`：圆角幅度真的缩放整套圆角、`none`/`pill` 不参与缩放、
  描边强度**只乘 hairline**（焦点环 / 遮罩 / 悬停逐项断言不变）。
- `display_prefs_test.dart`：两个旋钮的默认值 / JSON 往返 / 旧存档回落 /
  clamp（**描边不许归零**）/ 相等性 / 滑杆读同一组常量。

### 6.3 复测（同尺寸、同裁剪区域）

| 指标 | 改动前 | **改动后** | Morrow（参照） |
| --- | --- | --- | --- |
| 色度均值（0–255） | 4.6 | **9.9** | 13.3 |
| 偏离中性灰 | 1.7 | **3.5** | 5.0 |
| chroma > 6 的像素占比 | 8.5% | **66.8%** | 94.4% |
| 最大亮度桶 | 67.5%（**一档**） | 32.9% + 21.4%（**两档**） | 28.5% |

**没有追平 Morrow 的色度是刻意的**：它是内容工作台，卡片与底图可以更花；
本项目舞台是主角（P1），UI 必须更安静。分界线取在「层级不靠描边也读得出」
——这一条由测试守着，不由审美守着。

### 6.4 门禁数字

| 检查 | 结果 |
| --- | --- |
| `flutter analyze` | 0 issue |
| `flutter test` | **853** 通过 / 0 失败（基线 842 + 新增 11） |
| `flutter build web --release --base-href /app/ --no-web-resources-cdn` | 成功 |
| `./scripts/ignite.sh --check` | 四项全 ok（302 / 200 / 两处无 gstatic） |

### 6.5 仍需你做的

在 Windows 浏览器打开 `http://127.0.0.1:18080/app/`，**逐一切四套配色**看一遍，
再拖一下「圆角幅度 / 描边强度」——这两件事单元测试证明不了：
测试只能证明「值确实接进了主题」，证明不了「看着对」。
