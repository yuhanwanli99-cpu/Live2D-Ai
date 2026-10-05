# BATCH-0609b 落盘（极简）· 宿主 709 行的类清单：四个类，其中一个 391 行

## 编号说明
`BATCH-0609.md` **已存在**（内容是 `ignite.sh` 42 行）⇒ 本批改落 `BATCH-0609b.md`。
**⇒⇒⇒⇒⇒ 遵守 B0608 定的新纪律：`write` 前先 `ls` 确认**（这次没再覆盖）。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0609.md                      -> 存在
grep -n "^class |^enum " appearance_section.dart
```

## 类清单（4 个）
```
AppearanceSection        51    ← 唯一对外的 widget
_ModelOverrideHeader    442
_StageImageView         522
_StagePlaylistEditor    648
（文件 709 行 ⇒ 最后一个类 648→708 = 60 行）
```

## 五个可核点
1. **⇒⇒⇒⇒⇒ 四个类，三个是私有（下划线）** ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：
   `part` 文件里的 widget 名与分区共用一个库（`appearance_background.dart:7` 自己写的），
   所以它们**必须**私有 —— **「必须私有」是拆成 part 的一个正当理由**
   ⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ 这修正了我在 B0607b 的疑问**：
   我当时说「拆成 part 会不会让私有名泄漏」—— 头注已答：**不泄漏，因为共用一个库**。
2. **⇒⇒⇒⇒⇒ `AppearanceSection` 51 → 708，本体 657 行**
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：一个类的「跨度」要单独看**；
   1874 / 1551 / 1344 这三个数**掩盖了内部结构**，
   而 `AppearanceSection` 51→708 说明**宿主里最大的一块是它自己的 build**。
3. **⇒⇒⇒⇒⇒ `_ModelOverrideHeader` 只有 80 行**（442→521）
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：类名带 `Header` 的往往很小**，
   `grep -n "^class "` 给的行号相减就是它的行数 —— **不用通读**。
4. **⇒⇒⇒⇒⇒ `_StagePlaylistEditor` 60 行 / `_StageImageView` 126 行**
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据：`Editor` 与 `View` 的行数比（60 vs 126）
   说明「编辑」比「展示」轻** —— 而 B0607b 核的 part 里 `_ItemStyleEditor` 1366、
   `_LibraryManager` 799(`State` 起) ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 同一「背景域」在宿主与 part
   两侧都有 Editor/View/Manager，一侧重一侧轻 ⇒ 拆分的重量分布是不均匀的**，
   **这才是「值不值得再拆」的真问题**（不是 709 这个数字）。
5. **⇒⇒⇒⇒⇒ 三组域的落点**：`AppearanceSection` 一个类里装三组（`:3-4` 头注自述）
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：头注说「三组」而类清单只有一个 `AppearanceSection`
   ⇒ 三组是 build 里的三段，不是三个类** ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 这是 Dart 常见的
   「一个大 StatelessWidget + 若干私有辅助类」形态**，与 Rust 的多文件拆分风格不同。

## 未核
`AppearanceSection.build` 里三组的实际行数分布（要读 51-441）·
`_StageImageView` 126 行是否含播放逻辑 · `main.dart`（1344）头注 ·
`_ModConfigTileState` 那 440 行

## 补核（同批当场做完）：`AppearanceSection` 的构造器与拆分口径
### 构造器规模
```
:52-…  const AppearanceSection({ required prefs, required onPrefsChanged, devMode=false,
        onPickStageImage, onClearStageImage, stageImageMessage, stageImageFailed=false,
        onPickShellImage, onClearShellImage, shellImageMessage, shellImageFailed=false,
        onAddToPlaylist, onClearPlaylist, action,
        onHeadScaleChanged, onBodyScaleChanged, onExpressionScaleChanged,
        modelOverrideEnabled=false, onModelOverrideEnabledChanged, onModelOverrideHeadScaleChanged,
        onModelOverrideBodyScaleChanged, onModelOverrideExpressionScaleChanged,
        onResetModelOverride, modelOverrideMessage, modelOverrideFailed=false,
        onRemoveBackground, onRemoveBackgrounds, onReorderBackground, … })
窗口 52-100 内 `this.*` 参数数 = 30
```
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒ 30 个构造器参数**（其中必填 2 个）⇒⇒
**⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据：参数个数是这个类的真实「扇入」** ——
它同时接了「外观 / 舞台轮播 / 模型幅度 / 背景库增删排序」四条线的回调
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 这也解释了为什么它必须留在宿主**：拆走 background 域之后，
**它仍要接 background 域的 4 个回调**（`onRemoveBackground` /
`onRemoveBackgrounds` / `onReorderBackground` / …）⇒⇒
**⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据（补 B0608 的第 2 点）**：
**「一个类的行数」与「一个类的扇入」是两件事**；
判断该不该拆，看**扇入能不能随域一起搬走** —— 这里的 4 个背景回调**搬不走**
（因为 part 与宿主共用一个库，参数还是要从外面接进来）。

### 拆分口径：头注 `:22-25` 明写「不做」
```
:22 /// 剩下的是这四块的字段编排（外观 / 舞台单图轮播 / 舞台与口型 / 互…
:23 /// 外加服务端的动作幅度），而抽取的任务口径是「**只搬不改**」——任…
:24 /// 「**不必强行压到 ≤500，那是 Stage C3 的事**」。C3 继续按「舞台与口型…
:25 /// 与「互动」两域拆即可，字段清单与顺序在那之前不动。
```
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 这是本仓最好的一处「明示不做」**：
① 明写本轮口径是「**只搬不改**」 ② 明写「**不必强行压到 ≤500，那是 Stage C3 的事**」
③ 明写下一步只拆「舞台与口型」「互动」两域，且**「字段清单与顺序在那之前不动**」
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据（三条一起才够）**：
**说清楚「这轮不做什么」需要三个要素：口径（只搬不改）、
不做的理由（那是下一件事）、下一件事的范围（哪两域 + 什么不动）。**
⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 修正 B0608 第 2 点**：
我数出「三组」而 `:49` 只挂 1 个 part，**但 `:22` 写的是「四块」**
⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 第四块是「**外加服务端的动作幅度**」**
（`_ModelOverrideHeader` 442 行 + `onHeadScaleChanged` / `onBodyScaleChanged` /
`onExpressionScaleChanged` + `onModelOverride*` 六个回调）
⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据：数「域」时以**头注自己的分组**为准，
不要按类名猜** —— 我按「外观 / 舞台与口型 / 互动」三组数，**漏了动作幅度那一块**
⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 这就是 B0440 第 5 形态
「有写无读」的一个变体**：**头注把「四块」列出来了，我没数第四块**。
