# DECISION — E2：`display_prefs.dart`（1169 行 / 单类 939 行）怎么拆

> 状态：**方案纸，未实施**。写面 `docs/plans/**`；本文件不改任何源码。
> 上游：`HANDOFF-2026-10-06-e1-e5-debt-round.md` §6 第 1 条（E2 **待维护者裁决**）·
> `NEXT-ROUND-main-2026-10-06.md` 第 3 条（E2 = `display_prefs.dart`）
> 门禁真源：`xtask/src/code_stats/{mod.rs,gates.rs,collect.rs}`
> **本文件所有数字都来自实际命令输出**（命令与原始片段见 §7）；凡未实测的一律写「未实测」。

---

## 0. 结论（先看这一段）

| 问题 | 本次实测答案 |
|---|---|
| E2 现在是**红门禁**吗？ | **不是**。Dart `lib >800` 棘轮 = 2、实测 = **1** ⇒ `code-stats --check` **PASS**（`gates.rs` 的四条门禁里没有产物体积，见 §7.6） |
| 「只把顶层函数/类搬出去」能修好 E2 吗？ | **不能**。同库可搬的**顶层**部分只有 ~**228** 行；搬完 1169 → ~941，**仍 >800**，门禁数字一个都不动 |
| 有没有「零行为改动 + 不再改调用点」的拆法？ | **有**：把类内方法做成**同库 `part` 文件里的 extension**（`main.dart` 在 E1-b 就是这么干的，见 §1.7）。实测类体可外搬 ~**464** 行 ⇒ 主文件约 **512** 行，`lib >800` **1 → 0** |
| 类分解（字段对象 + 门面）值不值？ | **现在不值**。它动的是**数据模型**：157 处构造点、447 处 test 提及、24 键落盘契约；收益是「概念内聚」，不是任何门禁数字。建议**延后**，等 B2 落地后再单独立项 |
| **建议** | **先做 B2（main.dart 配方：同库 part + extension 外搬方法），A 暂不做**；两者不冲突，A 可以后加在 B2 之上 |

**给维护者的一句话**：E2 不是「必须拆才绿」，是「留债」。若允许，建议按 B2 做（真降行数、零行为改动、调用点几乎不动，且**已有守卫会强制**补上两处直读点）；若维护者认为「只降行数不降文件数」没价值，也可**明文登记为「不拆」**，但那样必须**把 `RATCHET_DART_800` 从 2 收紧**——否则这 1 个超限位会被下一次改动悄悄用掉（见 §6 R3）。

---

## 1. 实测现状

### 1.1 尺寸

```
$ wc -l shell/flutter/lib/settings/display_prefs.dart
1169 shell/flutter/lib/settings/display_prefs.dart
```

类体边界（读源码钉的，不是估的）：

| 区间 | 内容 | 行数 |
|---|---|---:|
| 1–17 | 文件头注 + `library;` + 2 条 import（`design/background_item.dart`、`design/theme_id.dart`） | 17 |
| 18–92 | 6 个顶层常量（`kBackgroundMaxCount` / `kBackgroundImageMaxBytes` / `kStageImageMaxChars` / `kShellImageMaxChars` / `kStagePlaylistMaxItems` / `kStagePlaylistMaxChars`）+ 2 个顶层函数 `backgroundBytesOf` / `dataUrlBytes` | 75 |
| **95–1033** | **`class DisplayPrefs`（唯一的大类）** | **939** |
| 1035–1052 | 顶层 `clampTier` | 18 |
| 1054–1065 | 顶层 `_sameList` | 12 |
| 1067–1087 | 顶层 `class StagePlaylistAppendResult` | 21 |
| 1089–1134 | 顶层 `appendToStagePlaylist` | 46 |
| 1136–1148 | 顶层 `stagePlaylistIndexOf` | 13 |
| 1150–1159 | 顶层 `removeStagePlaylistAt` | 10 |
| 1161–1169 | 顶层 `moveStagePlaylist` | 9 |

> 文件头注自己已经如实写着「超出豁免带，拆分归 Stage C3」（第 7–12 行）。

### 1.2 类内分块（`class DisplayPrefs` 939 行）

```
96–121   const 构造（24 个命名参数，全 this.x）            26
123–307  24 个 final 字段 + 逐个文档注释                  185
309–476  static const（默认值/区间/档位/fit 码/alignTable）168  ← 其中 fitName 仅 454–459
478–534  copyWith（24 个可选命名参数）                     57
536–645  toJson（24 键）+ fromJson（factory）              110
647–663  _readBackgroundSource（legacy shellImage 兼容）    17
665–742  effective* / hasBackground* / currentItemIsImage   78
744–800  readBackgrounds / backgroundsBytes / canAddBack…   57
801–939  clamp* ×11 + _read* ×5                            139
941–1032 == / hashCode / toString                           92
```

24 个字段（`grep -n '^  final '` 命中 131–307，逐条）：

```
theme stageImage backgrounds backgroundSource backgroundOpacity backgroundBlur
backgroundScrim backgroundEnabled imageFit imageAlign tileSize slideInterval
slideRandom uiTransparency stagePlaylist scale mouthSensitivity lipSync
idleEnabled muted volume allowDragZoom tier edgeStrength
```

### 1.3 落盘契约（这是整个 E2 里最贵的资产）

- **24 个字段 ⇄ 24 个 JSON 键，1:1**（`toJson` 第 538–563 行）；构造参数也是同一组 24 个。
- **整份偏好是 localStorage 的**一条**记录**：`kDisplayPrefsKey = 'live2d-ai.display-prefs'`，读写是
  `jsonEncode(prefs.toJson())` / `DisplayPrefs.fromJson(decoded.cast<String, Object?>())`
  （`app/browser_io.dart:22,36-57`）。**没有版本号字段**，`fromJson` 靠「非法/缺失回落默认」保持兼容
  （`test` 的 `反序列化永不抛异常` 组 57–143 行就是钉这条）。
- **legacy 兼容已经存在**：`_readBackgroundSource`（647–663）与 `readBackgrounds`（744–776）要认旧的
  `shellImage` / `syncShellStageBg`。**任何拆分类的方案都必须让这两条留在唯一的读入口上**，否则老用户开机会静默掉背景。
- `==` 的语义是**行为契约**（`F-0034-01`/`F-0074-01`）：逐图样式（opacity/fit/align）必须参与判等，
  否则 `if (next == widget.prefs) return;` 会**静默吃掉**用户的编辑（`display_prefs_style_change_detection_test.dart` 头注 1–32 行）。
  `hashCode` 与它对称（`Object.hash` 两段 hashAll，backgrounds 展开进第二段）。

### 1.4 使用点清单（`grep -c` 口径）

| 口径 | lib | test | 合计 |
|---|---:|---:|---:|
| 提及 `DisplayPrefs` | **166 处 / 24 文件** | **447 处 / 31 文件** | 613 |
| `DisplayPrefs(...)` **构造点** | 15 | 141（+ `tool/visual_review_test.dart` 1） | **157** |
| `DisplayPrefs.<方法>` **静态调用**（clamp*/effective*/fitName/backgroundsBytes/canAddBackground/read*） | **11 处真代码**（另 5 处在注释里） | 38 | 49–54 |
| `DisplayPrefs.<常量>` 读取（default*/min*/max*/tiers/fitCode） | 47 | 151 | **198** |

lib 侧 11 处真代码静态调用点（拆类时**必须零破坏**的那批）：

```
settings/sections/appearance_background_library.dart:405  DisplayPrefs.backgroundsBytes(items)
settings/sections/appearance_background_style.dart:65     DisplayPrefs.fitName(prefs.imageFit)
settings/sections/appearance_background.dart:649          DisplayPrefs.effectiveImageFit(...)
settings/sections/appearance_background.dart:655          DisplayPrefs.fitName(effectiveFit)
settings/sections/appearance_background.dart:703          DisplayPrefs.effectiveImageAlign(...)
main.dart:514                                             DisplayPrefs.clampVolume(v)
ui/shell_backdrop.dart:190/194/195                        effectiveImageOpacity/Fit/Align
app/shell_prefs.dart:258/468                              DisplayPrefs.canAddBackground(...)
```

lib 侧提及最多的文件：`appearance_background.dart` 29 · `ui/shell_backdrop.dart` 25 · `app/shell_prefs.dart` 18 ·
`settings/display_prefs.dart` 14 · `appearance_section.dart` 14 · `app/browser_io.dart` 12 · `app/shell_app_root.dart` 9 ·
`data/background_hydration.dart` 8 · `design/background_item.dart` 8 · `appearance_background_style.dart` 8。

### 1.5 测试面（拆完之后要**逐字段**证明等价的那一批）

| 文件 | 行数 | 与 DisplayPrefs 的关系 |
|---|---:|---|
| `test/display_prefs_test.dart` | 890 | **主契约面**：11 个 group —— 默认值 / 反序列化永不抛异常 / 往返与 copyWith / 区间常量自洽 / allowDragZoom+tier / 主题 / 舞台背景图 / 滑杆区间共用常量 / 材质旋钮 / 背景库 / 背景来源 |
| `test/display_prefs_background_fit_test.dart` | 645 | 铺法/位置/贴片/遮罩 + 逐图覆盖 + `toJson().keys` 断言（353/357 行） |
| `test/display_prefs_style_change_detection_test.dart` | 303 | `==` 的**行为**回归（真泵 AppearanceSection + 同形闸门） |
| `test/display_prefs_slide_index_test.dart` | 82 | 轮播下标 |
| `test/appearance_background_style_test.dart` | 487 | 逐图样式编辑 |
| `test/appearance_background_runtime_test.dart` | 478 | 真泵设置面 |
| `test/setting_wiring_test.dart` | 420 | 接线守卫（含源码扫描） |
| `test/shell_backdrop_test.dart` | 189 | 壳背景渲染参数 |

**缺口（对两个方案都成立）**：全仓**没有任何一条测试钉住 `DisplayPrefs` 的 24 键全集**——
`display_prefs_background_fit_test.dart:353/357` 钉的是 `BackgroundImage.toJson()`（逐图），不是偏好本体。
所以「键名写错/少写一个」今天**不会有测试变红**，只会在用户刷新后表现为某个设置复原。见 §4 第 1 条。

### 1.6 门禁关系（当前**绿**）

```
# xtask/src/code_stats/mod.rs —— **2026-10-06 夜复核后的当前值**
155: const PLAN_DART_800: u64 = 2;          // PLAN §5 目标（未变）
144: const RATCHET_DART_800: u64 = 1;       // CI 默认棘轮（本文初次落盘时是 2；Lead 已批准收紧 → 见 §9 Addendum）
# 同文件 131–138 行现在的注释原文：
#   「**再收紧到 1**（2026-10-06 夜：E1-b 拆分 + T1 收尾补做）… **2 → 1**，与实测 1 相符。
#    剩下的 1 = display_prefs.dart 1169（E2 待裁决的类分解），**当前无下调空间** ⇒
#    本常量已是只降不升的下限，只有 E2 做完才可能再降到 0。」
# （本文初次落盘时引用的是收紧**前**的诚实栏「常量仍是 2 …留债给下一批」，那段已被上面这段取代。）
# gates.rs 只有四条门禁：src-rs-500 / dart-800 / src-rs-1000 / deps-desktop（127–172 行）
```

⇒ **棘轮 = 实测 = 1 ⇒ 余量为 0**（2026-10-06 夜收紧后；收紧前的写法见 §9 Addendum）。这不是「E2 可以不做」的宽松状态，
而是「**唯一那一个超限位已经被 `display_prefs.dart` 占死**」：任何**新**的 `lib` 文件越过 800 行 ⇒ 计数 2 > 1 ⇒
`code-stats --check --only lines` **当场红**；而**只有把 `display_prefs.dart` 压到 ≤800，棘轮才可能再降到 0**
（`mod.rs:135-136` 原文：「当前无下调空间 ⇒ 本常量已是只降不升的下限，只有 E2 做完才可能再降到 0」）。
**注意「1 ≤ 1 = PASS」很容易被读成「还有一个空位」——它不是。** 这一条直接影响 §5 的推荐，已在 §9 重算。

### 1.7 仓库里已经有的先例（`main.dart` 配方，E1-b 刚做过）

```
$ grep -n '^part ' shell/flutter/lib/main.dart     → 9 条 part（107–114 行）
$ grep -rn '^extension ' shell/flutter/lib/app/*.dart
app/shell_prefs.dart:17                      extension _ShellPrefsWiring on _ShellRootState
app/shell_admin.dart:12                      extension _ShellAdminWiring on _ShellRootState
app/shell_chat.dart:11                       extension _ShellChatWiring on _ShellRootState
app/shell_settings.dart:11                   extension _ShellSettingsWiring on _ShellRootState
app/shell_cue_voice_wiring.dart:3            extension _ShellCueVoiceWiring on _ShellRootState
app/shell_background_scale_wiring.dart:3     extension _ShellBackgroundScaleWiring on _ShellRootState
app/shell_section_wiring.dart:3              extension _ShellSectionWiring on _ShellRootState
app/shell_lifecycle_wiring.dart:3            extension _ShellLifecycleWiring on _ShellRootState
```

**这就是「类体不能跨 part」的正解**：类本体留在原文件（含字段与 `==`），**方法**搬到同库 `part` 里做成
`extension ... on 类`。E1-b 用它把 `main.dart` 1417 → 657 行，**逐字搬迁、未重写一行业务代码**，
Dart `lib >800` 2 → 1。E2 是同一个形状的第二个实例。

**测试侧的地基也已经铺好**：`test/support/dart_library.dart` 的 `readLibrarySource()` 会自动递归读 `part`；
`test/dart_library_guard_test.dart` 三条判据（part 库不得被字面量直读 / 不得有「按路径读源码」的辅助函数 /
不得「变量路径 + part 库字面量」）会**主动在加 part 的那一刻变红**，把漏扫点逼出来。

---

## 2. 方案 A：类分解（字段对象 + 门面）

### A.1 形状

把 24 个字段按语义收成 4 个子对象，`DisplayPrefs` 退化成**门面**：

| 子对象 | 字段（实测分组） | 字段数 |
|---|---|---:|
| `BackgroundStyle` | backgrounds backgroundSource backgroundOpacity backgroundBlur backgroundScrim backgroundEnabled imageFit imageAlign tileSize slideInterval slideRandom | 11 |
| `StageView` | stageImage stagePlaylist scale tier | 4 |
| `AudioPrefs` | muted volume lipSync mouthSensitivity | 4 |
| `UiPrefs` | theme uiTransparency edgeStrength allowDragZoom idleEnabled | 5 |

门面保留 24 个**转发 getter**（`double get backgroundOpacity => background.opacity;`）+ `copyWith` 24 参 +
`toJson`/`fromJson` 编排 + `==`/`hashCode`/`toString`。

### A.2 影响面（实测）

| 面 | 数量 | 是否必须改 |
|---|---:|---|
| 构造点 `DisplayPrefs(...)` | 157（lib 15 / test 141 / tool 1） | **不必**（门面保留同名同参构造） |
| `prefs.<字段>` 读 | 613 处提及里的多数 | **不必**（转发 getter 保住字段名） |
| `DisplayPrefs.<静态方法>` | lib 11 + test 38 | **不必**（静态方法留门面） |
| `DisplayPrefs.<常量>` | 198 | **不必**（`static const` 留门面） |
| 落盘 24 键 | 1 条 localStorage 记录 | **必须保持不变**（子对象只做分组，不嵌套 JSON） |
| `==`/`hashCode` | 92 行手写 | **必须重写**（每层各自 `==`；`Object.hash` 的实际取值会变——只要与 `==` 对称就行） |
| `toString` | 14 行 | 会变（当前格式无字段级测试钉住） |

### A.3 真实收益 / 真实代价

- 收益：**概念内聚**（「背景 11 项」「音频 4 项」单独成对象，规则与区间也跟着走）；`==`/`hashCode` 不再是 24 字段的大平铺（可缓存子对象 hash）。
- 代价：
  1. **落盘契约的回归面最大**——24 键必须逐键不变，而今天**没有一条测试钉键全集**（§1.5 缺口）。要做 A，**先**加那条测试（§4-1），否则等于在没有网的情况下走钢丝。
  2. `hashCode` 取值变化会影响所有以 `DisplayPrefs` 为 Map key 的地方（本树 grep 未见，但**未逐个确认**）。
  3. 门面如果保留 24 个转发 getter + 24 参 `copyWith` + 4 个子对象，**主文件仍然可能 >800**——所以 A 单独做**不保证清掉门禁数字**，还得叠加 B2。
  4. 排期：按本树既往同类改动（E5/F-0013 那种键级回归、E1-b 的拆分）估 **1–2 个工作日 + 一轮全量 flutter test**；**不影响任何门禁数字**。

### A.4 A 的判断

**不建议现在做**：它的净收益是设计质量，而当前**没有一条门禁或发布判据**因它而变红；它的风险集中在**唯一一条**没有测试保护的契约（落盘 24 键）上。若维护者要的是「背景域该有自己的类型」这个设计目标，正确顺序是 **B2（把体积压下来、腾出可读性）→ 补 24 键守卫 → 再 A**。

---

## 3. 方案 B：低风险外搬

### B.1 B1 —— 只搬**已经存在的顶层**（收益上限，实测）

同库内可搬的顶层块（§1.1 那些）：顶层常量 75 + `backgroundBytesOf`/`dataUrlBytes` 18 +
`clampTier` 18 + `_sameList` 12 + `StagePlaylistAppendResult` 21 + `appendToStagePlaylist` 46 +
`stagePlaylistIndexOf` 13 + `removeStagePlaylistAt` 10 + `moveStagePlaylist` 9 = **222 行**，
外加空行约 **228 行**。

```
1169 − 228 ≈ 941 行 ⇒ 仍然 > 800
⇒ Dart lib >800 计数不变（仍 1），code-stats --check 数字一个都不动
```

**B1 的收益上限就是「文件短一点」**，并且会产生一个副作用：`test/display_prefs_test.dart` 里
`File('lib/settings/display_prefs.dart').readAsStringSync()`（431 行）这类守卫的扫描面会变窄。
**结论：B1 单独做** = 只改了形状、没改任何指标，还平白多两次守卫维护——**不建议单独做**。
（若做，也应当作为 B2 的**附带项**一起做，见下。）

### B.2 B2 —— main.dart 配方：`part` + `extension` 外搬**方法**（推荐）

**关键区分**：Dart 规定「**类体**不能跨 `part`」，但**没规定「方法」必须住在类体里**——
把方法写成同库 `part` 文件里的 `extension X on DisplayPrefs`，**调用点一字不改**（实例成员经 extension 解析，
且 `part` 属于同一 library ⇒ **私有成员可见**、`import 'display_prefs.dart'` 也会把 extension 带进作用域）。

#### B.2.1 能搬 / 不能搬（按 Dart 语义逐条判断）

| 块 | 行数 | 能进 extension？ | 依据 |
|---|---:|---|---|
| `fitName`（454–459） | 6 | 可以 | 普通静态方法（需保留 1 行静态包装，见下） |
| `copyWith`（478–534） | 57 | 可以 | 不是 `Object` 成员；extension 可声明 |
| `toJson`（537–569）/`fromJson`（570–645） | 110 | 部分 | `toJson` 可以；`fromJson` 是 `factory` ⇒ 只能做 `static` 扩展成员，调用点 `DisplayPrefs.fromJson(...)` **需包装**（全仓 lib 1 处：`browser_io.dart:42`） |
| `_readBackgroundSource`（647–663） | 17 | 可以 | 私有静态 ⇒ 同库可见 |
| `effective*` / `hasBackground*`（665–742） | 78 | 可以 | 实例 getter / 实例方法（静态的那几个保留包装） |
| `readBackgrounds`/`backgroundsBytes`/`canAddBackground`（744–800） | 57 | 可以 | 静态 ⇒ 保留包装，lib 有 3 处调用 |
| `clamp*` ×11 + `_read*` ×5（801–939） | 139 | 可以 | 静态 ⇒ 保留包装，lib 有 3 处调用 |
| **`==` / `hashCode` / `toString`（941–1032）** | **92** | **不可以** | 它们是 `Object` 的成员；extension 只在「静态类型没有该成员」时生效 ⇒ 声明了也**永远不会被调用**（静默失效，正是本项目最忌讳的形状）。**必须留类里** |
| `static const` 全集（309–476 里除 fitName） | 162 | 不可以 | `DisplayPrefs.defaultVolume` 这类 **198 处**调用点读的就是类上的静态常量；extension 静态成员只能写作 `X.defaultVolume`，会破 198 处 |
| 24 个字段 + 文档（123–307） | 185 | 不可以 | 字段本体（门面） |
| `const` 构造（96–121） | 26 | 不可以 | `const DisplayPrefs()` 被大量使用 |

**静态方法的处理**：lib 侧 11 处真调用 + test 侧 38 处（如 `DisplayPrefs.clampVolume(v)`、
`DisplayPrefs.effectiveImageOpacity(...)`）。**不推荐改这 49 处调用点**；
推荐在类里留**一行转发包装**：

```dart
// 本体在 part 的 extension 里；这一行只为保住既有调用点（49 处）与公共 API 名字。
static double clampVolume(double v) => PrefsClamp.clampVolume(v);
```

按实测需包装的静态成员：`clamp*` ×11 + `effectiveImage*` ×3 + `backgroundsBytes` /
`canAddBackground` / `readStagePlaylist` / `readBackgrounds` / `fitName` /
`fromJson` ⇒ 包装 ≈ **18 行**。

#### B.2.2 行数预算（实测基数）

```
主文件（display_prefs.dart）保留：
  头注+library+imports                17
  class 头 + const 构造               28
  24 字段 + 文档                     185
  static const 全集（除 fitName）      162
  == / hashCode / toString            92
  静态转发包装                        ~18
  空行/注释边界                        ~10
  ----------------------------------------
  合计 ≈ 512 行           ⇒ < 800 ✓（比现在的 1169 少 657 行）

part 文件（每个都远 < 800）：
  display_prefs_copy_with.dart     copyWith + fitName                ~65
  display_prefs_json.dart          toJson + fromJson + _readBackg…  ~130
  display_prefs_effective.dart     effective*/hasBackground*         ~80
  display_prefs_limits.dart        clamp*/read*/backgroundsBytes…   ~200
  display_prefs_playlist.dart      顶层常量+函数、StagePlaylistAppendResult、append/remove/move… ~230
```

⇒ **`lib >800` 1 → 0**。这是 B2 唯一的、可验证的指标收益；**行为应逐字不变**（搬迁 = 剪切/粘贴 + 加 `extension` 头）。

#### B.2.3 **必须同步的两处直读点**（加 `part` 的那一刻，守卫会把它们判红）

```
test/display_prefs_test.dart:431   File('lib/settings/display_prefs.dart').readAsStringSync()
                                   → 断言不含 'radiusScale'
test/no_backdrop_filter_test.dart:543  File(path).readAsStringSync()（path 来自常量表，含同一路径）
                                   → 「纯逻辑文件只 import dart:*」守卫
```

这两处今天能过，是因为 `display_prefs.dart` **还没有 part**。一旦加了 part：

1. `test/dart_library_guard_test.dart` 的判据①（`File('<part 库>').readAsStringSync()`）命中第 431 行；
2. 判据③（「变量路径读取 + 同文件出现 part 库字面量」）命中 `no_backdrop_filter_test.dart`。

⇒ 两处都改成 `readLibrarySource('lib/settings/display_prefs.dart')`，**红→绿本身就是这次拆分的安全性自证**
（顺带把这两条守卫的真实扫描面从 1 个文件扩到**库 + 全部 part**，比拆分前更强）。
**注意**：这条不是「顺手」——它是 E1-a（守卫不得静默漏扫）在本文件的直接续集，漏掉就等于把 2026-10-06 刚修的缺陷原样复制一遍。

#### B.2.4 副作用与风险（逐条 + 判据）

| # | 风险 | 判据 / 缓解 |
|---|---|---|
| B-r1 | extension 解析不到（调用点静态类型不是 `DisplayPrefs`，或 part 没被 import） | `flutter analyze` **0 issue**（未解析的方法会直接报）；`flutter test` 全绿 |
| B-r2 | 私有成员跨文件不可见（若把 part 写成独立库而非 `part of`） | 必须 `part`（同 library）；文件名与 `part` 指令逐一对照 |
| B-r3 | `factory fromJson` 不能进 extension 导致 API 变更 | 类内留一行 `static DisplayPrefs fromJson(...)` 转发；`browser_io.dart:42` 不改 |
| B-r4 | 落盘 24 键被搬丢/搬错 | §4-1 新增的键全集测试（**先加后拆**） |
| B-r5 | `==` 语义被「顺手重构」 | `==`/`hashCode`/`toString` **留在类里、逐字不动**（B.2.1 已判定它们必须留） |
| B-r6 | 拆分导致源码扫描守卫变窄 | B.2.3 两处升级 + `dart_library_guard_test` 三条判据绿 |
| B-r7 | 拆完没人收紧棘轮 ⇒ 计数可以悄悄涨回 2 | `RATCHET_DART_800` **同 commit** 从 2 收紧到实测值（见 §6 R3） |

---

## 4. 逐字段行为等价测试计划（两个方案共用）

**顺序铁律**：**先加测试，再动代码**（否则拆完出事时分不清是拆错还是本来就没测）。

1. **落盘 24 键全集守卫（新增，最高优先）**——今天完全缺失：

   ```dart
   // test/display_prefs_test.dart（或新文件）
   expect(const DisplayPrefs().toJson().keys.toSet(), <String>{
     'theme','stageImage','backgrounds','backgroundSource','backgroundOpacity','backgroundBlur',
     'backgroundScrim','backgroundEnabled','imageFit','imageAlign','tileSize','slideInterval',
     'slideRandom','uiTransparency','stagePlaylist','scale','mouthSensitivity','lipSync',
     'idleEnabled','muted','volume','allowDragZoom','tier','edgeStrength',
   });
   ```

   判据：**少一个键 / 多一个键 / 改名 ⇒ 红**。这条同时是 A 与 B 的**入场券**。
2. **字段级往返矩阵**——现有 `往返与 copyWith` 组（144–176）是抽样式。加一条**驱动 24 个字段**的表驱动用例：
   每个字段给「非默认值」→ `toJson` → `fromJson` → 断言该字段相等**且其余 23 个不变**。
   判据：任何字段在序列化链路上丢/串 ⇒ 红（点名到字段）。
3. **`copyWith` 24 参覆盖**——表驱动断言「传第 i 个参数，只有第 i 个字段变」（`clearStageImage` 单列一条）。
4. **`==` 判别力**——沿用 `display_prefs_style_change_detection_test` 的形状，扩到**每个字段各一条**：
   `a = const DisplayPrefs()`、`b = a.copyWith(<该字段>=其它值)` ⇒ `expect(a == b, isFalse)`；
   再要求 `a == b ⇒ hashCode 相等`（不要求反向）。
   判据：**任一新字段漏进 `==` ⇒ 红**（这正是 F-0034-01 那一类在线缺陷的形状）。
5. **`fromJson` 抗坏输入**——现有 `反序列化永不抛异常` 组（57–143）保持不动，**不许删条**；补 legacy
   `shellImage`/`syncShellStageBg` 两条（若已有则核对，别重复）。
6. **守卫面**——B.2.3 的两处改 `readLibrarySource`，`dart_library_guard_test` 三条绿。
7. **全量门禁**——`flutter analyze` 0 issue + `flutter test` 条数**不减**（基线 1583）+ `code-stats --check` 四条 PASS。
8. **判别力自证**——按本仓纪律（W-D6），抽 1–2 条新断言做「**故意改错 → 必红 → 复原 → 变绿**」，原始输出进交付报告。

---

## 5. 推荐（带证据）

| 排序 | 动作 | 证据/理由 | 指标变化 |
|---|---|---|---|
| **1** | **加 §4-1 的 24 键守卫**（独立小提交） | 今天零覆盖；它是 A/B 的共同前提 | 无（但把最大盲区堵上） |
| **2** | **做 B2（part + extension 外搬方法）** | 可搬 464 行 ⇒ 主文件 ~512 行；E1-b 已验证同配方；49 处静态调用点零改动（留包装）；`==`/`hashCode`/`toString` 明令留类里 | Dart `lib >800` **1 → 0** |
| **3** | **同 commit 收紧 `RATCHET_DART_800`** | `mod.rs` 的棘轮纪律写着「降了不收紧，回头涨回去就拦不住」 | **2 → 1 已于 2026-10-06 夜执行**（§9）；**B2 完成后 → 0** |
| 4 | B1（只搬顶层） | 222 行，清不掉 `>800` | **不建议单独做** |
| 5 | A（类分解） | 收益是设计质量；风险压在没有测试保护的落盘契约上；不清门禁 | 暂不做；等 1+2 之后单独立项 |

**如果维护者只批一件事**：批 §4-1（键全集守卫）。它花 10 分钟，堵的是「用户刷新后设置复原、无任何错误」这一类当前**完全无测试**的缺陷——比拆文件的收益更硬。

---

## 6. 需要维护者裁决什么（裁决项 / 影响 / 判据）

| ID | 裁决项 | 二选一 | 影响 | 判据（怎样算完成） |
|---|---|---|---|---|
| **R1** | E2 拆不拆、按哪个方案 | (a) **做 B2**（推荐） (b) 明文登记「不拆」 | (a)：5 个文件替代 1 个，主文件 ≤800，行为逐字不变；(b)：E2 从 NEXT-ROUND 摘除，改为「登记为接受的债」 | (a)：`lib >800` = 0 + `flutter test` ≥1583 全绿 + analyze 0；(b)：NEXT-ROUND 该行改成明文「不拆 + 理由」，并**收紧棘轮**（见 R3） |
| **R2** | 允许**类分解**（字段对象/门面）吗 | (a) 暂不允许 (b) 允许，作为独立立项 | (b) 会动 157 构造点 / 24 键契约与 `==`/`hashCode` 实现（`Object.hash` 取值会变）；需要 §4 全套先绿 | (b)：§4-1..4 先合入并绿，再动模型；`toJson` 键全集与逐字段往返用例**不得**随拆分改动 |
| **R3** | `RATCHET_DART_800` 紧不紧 | (a) **收紧到实测值** —— **2 → 1 已于 2026-10-06 夜执行**；B2 落地后再 → 0 (b) 停在 1 不再降 | (a) 现状：余量 0，新超限文件立刻红；(b) 若 E2 不做，必须接受「唯一余量被 display_prefs.dart 长期占用」并在注释里写明 | (a)：改常量 + `code-stats --check` 仍 PASS（**已实测 1 ≤ 1 PASS**）；(b)：`mod.rs` 注释必须写明「这 1 个余量已被占用，不是空位」 |
| **R4** | 静态方法保不保留一行转发包装 | (a) 保留（推荐，49 处调用点零改动） (b) 改调用点为 `ExtensionName.method` | (b) 需要同步改 lib 11 处 + test 38 处，评审面变大 | (a)：`grep -c` 调用点数量不变；(b)：调用点清单进 PR 描述 |

---

## 7. 复现命令与原始片段

```bash
cd /home/skystar/Live2D-Ai
wc -l shell/flutter/lib/settings/display_prefs.dart              # → 1169
grep -n '^  final ' shell/flutter/lib/settings/display_prefs.dart # → 24 个字段（131–307）
grep -n "'[a-zA-Z]*':" shell/flutter/lib/settings/display_prefs.dart  # → toJson 24 键（538–563）
grep -rn 'DisplayPrefs' shell/flutter/lib  | wc -l                # → 166（24 文件）
grep -rn 'DisplayPrefs' shell/flutter/test | wc -l                # → 447（31 文件）
grep -rn 'DisplayPrefs(' shell/flutter | wc -l                    # → 157（构造点）
grep -rnE 'DisplayPrefs\.(clamp|backgroundsBytes|canAddBackground|effective|fitName|read)' shell/flutter | wc -l  # → 54
grep -rnE 'DisplayPrefs\.(default|min|max|tiers|fit[A-Z])' shell/flutter | wc -l                                 # → 198
grep -n 'RATCHET_DART_800\|PLAN_DART_800' xtask/src/code_stats/mod.rs   # → 144: =1（收紧后）  155: =2
grep -n 'gate(' xtask/src/code_stats/gates.rs                     # → 四条：src-rs-500/dart-800/src-rs-1000/deps
ls shell/flutter/lib/app/shell_*_wiring.dart                      # → E1-b 的 5 个 part
```

**7.6 关于「产物预算是不是门禁」**：`gates.rs:127-172` 只构造四条 `GateResult`，**没有 artifacts**；
`collect.rs:132-141` 只是把两个目录塞进 `stats.artifacts` 供报告打印（`report.rs:210-244` 输出「在预算内/超预算」）。
即：产物体积预算是**报告口径**，不参与 `--check` 退出码——这一点与 E4 直接相关
（见 `DECISION-artifact-budget-2026-10-06.md`）。

---

## 8. 本文件明确**没有**做的事

- 没有改任何源码（含 `display_prefs.dart`、测试、`xtask`）。`RATCHET_DART_800` 的 2 → 1 是 **Lead/docs-closeout 在本文落盘后**做的（见 §9 Addendum），**不是本文作者改的**，本文作者对 `xtask/**` 全程只读。
- 没有跑 `flutter analyze` / `flutter test`（本任务纯只读分析；行号与计数全部来自 read/grep/wc，**未做编译级验证**）。
- 没有实测 `hashCode` 取值变化对 Map key 的影响面（只做到「本树 grep 未见」）。
- A 的工作量（1–2 人日）是**按本树同类改动规模给的估计**，不是实测值，已在文中标注为「估」。

---

## 9. Addendum（2026-10-06 夜，本文初次落盘后的**事实更新**）

**触发**：Lead 通报——Dart 棘轮已批准并执行收紧，`docs-closeout` 已改 `xtask/src/code_stats/mod.rs`：
`RATCHET_DART_800` **2 → 1**（现 **L144**）；复算 `cargo test -p xtask` **27 passed**、
`cargo run -p xtask -- code-stats --check` **四表 PASS（Dart 行 1 ≤ 1）**、`cargo fmt` clean；
改动**尚未提交**（Lead 统一提交）。本次 Addendum 由本文作者自己改（写面仍为 `docs/plans/**`，未碰源码）。

### 9.1 本文哪些地方已过期（以本节为准）

| 位置 | 过期表述 | 现行事实 |
|---|---|---|
| §1.6 代码块 | `141: const RATCHET_DART_800: u64 = 2;` + 收紧**前**的诚实栏引用 | **L144 = 1**；`mod.rs:131-138` 已是「再收紧到 1（2026-10-06 夜）」那段 |
| §1.6 结段 | 「棘轮 2 而实测 1 ⇒ 下次再写大一个文件也不会红」 | **作废**：棘轮 = 实测 = 1 ⇒ 余量 0 |
| §5 第 3 行 | 「棘轮 2 → 1」 | **已执行**（§9.1）；B2 后 → 0 |
| §6-R3 | 选项 (a) 写成待办 | (a) 的 2 → 1 **已完成**；(b)「停在 1」现在多一条「必须写明这不是空位」的义务 |
| §7 | `grep … # → 141: =2` | `# → 144: =1（收紧后） 155: =2` |
| §8 第 1 条 | 「`RATCHET_DART_800` 保持 2」 | 该常量由 Lead/队友在本文落盘后改动；**本文作者对 `xtask/**` 全程只读** |

### 9.2 语义变化：**余量 0**（这是本次更新最重要的一条）

棘轮从「2 而实测 1」变成「**1 而实测 1**」——`code-stats --check --only lines` 依旧是 PASS，但 PASS 的**含义变了**：

- **收紧前**：还有 1 个空位 ⇒ 新写一个大文件不会红（**宽松**）；
- **收紧后**：**空位为 0** ⇒ 任何新的 `lib` 文件越过 800 行，计数变 2 > 1，**当场红**（**顶格**）。
- 而「撤销这次顶格」的**唯一**办法，就是让 `display_prefs.dart` 掉到 ≤800（计数 0），再把棘轮降到 0 ——
  `mod.rs:135-136` 的原话已经这么写了：「剩下的 1 = `display_prefs.dart` 1169（E2 待裁决的类分解），**当前无下调空间**」。

**读法警告**：CI 日志里那行 `1 ≤ 1 = PASS` **不能**读成「还有一个空位」。这一次收紧把「E2 是纯留债」变成了
「**E2 握着唯一那格余量**」——这也正是棘轮纪律（降了必须同 commit 收紧）想要的效果。

### 9.3 推荐重算（对 §5 的修订，其余结论不变）

| 排序 | 动作 | 更新后的理由 | 指标变化 |
|---|---|---|---|
| **1** | **加 §4-1 的 24 键守卫** | 不变：它今天零覆盖，且是 A/B 的共同前提 | 无 |
| **2** | **做 B2（part + extension 外搬方法）** | **价值上升**：它是**唯一**能把计数压到 0、从而把棘轮降到 0 的动作 ⇒ 之后任何 >800 文件都必须先拿裁决 | Dart `lib >800` **1 → 0**；棘轮 **1 → 0** |
| **3** | **棘轮随 B2 同 commit 降到 0** | 棘轮纪律要求「计数下降的那个 commit 同 commit 收紧到实测值」（本次 2 → 1 就是这么补的） | 0 |
| 4 | B1（只搬顶层 222 行） | **结论不变且更强**：941 行仍 >800 ⇒ 计数仍 1 ⇒ 棘轮仍只能停在 1、余量仍 0 ⇒ **纯粹无效动作** | 无 |
| 5 | A（类分解） | 结论不变：不清门禁数字；风险压在无测试保护的落盘 24 键上 | 暂不做 |

**一条新的义务（若维护者选「不拆」）**：必须把棘轮**停在 1** 并在 `mod.rs` 注释里点明
「这唯一的余量已被 `display_prefs.dart` 占用，**它不是空位**」——否则下一位维护者会把 `1 ≤ 1` 误读成还有余量，
而那正是「棘轮不再拦人」的假绿灯形状（与本文 §6-R3 的 (b) 判据同源）。

### 9.4 §2 A.3 结论的措辞修正

原句「A 单独做不影响任何门禁数字」保留成立（判定不变），但**补一句**：A 单独做也**不会释放**那唯一的余量
（主文件仍可能 >800），所以它既不清数字也不放余量——这进一步支持「先 B2、再谈 A」的顺序。

