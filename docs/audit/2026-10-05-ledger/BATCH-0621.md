# BATCH-0621 落盘（极简）· ⚠「全仓库唯一的 reduced-motion 出口」**不成立**：有两个时长出口

## 编号
`BATCH-0621.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0621.md
grep -rn "disableAnimationsOf|reduceMotion|appMotion" shell/flutter/lib/ --include=*.dart
sed -n '145,156p' lib/ui/glass_rim.dart
sed -n '108,113p' lib/ui/soft_motion.dart
```

## 七个命中点，逐个分类
| 文件:行 | 形态 | 类别 |
|---|---|---|
| `ui/soft_motion.dart:46` | `Duration appMotion(...)` 内部调 | **时长出口 ①** |
| `ui/glass_rim.dart:152` | `Duration appMotionLite(...)` 内部调 | **时长出口 ②** |
| `ui/soft_motion.dart:111` | `if (disableAnimationsOf(context)) return child;` | 内联（跳过动画） |
| `ui/soft_motion.dart:166` | 内联 | 内联（**未核**） |
| `ui/glass_rim.dart:98` | `!MediaQue…` | 读**布尔** |
| `ui/streaming_indicator.dart:67` | `final bool reduced = Medi…` | 读**布尔** |
| `ui/state_pill.dart:165` | `final bool reduced = MediaQ…` | 读**布尔** |

另：`appMotion` 的**调用方**有 4 处（`live2d_stage.dart:278`、`page_cross_fade.dart:95`、
`collapsible_panel.dart:69`、`theme_picker.dart:139`）——**它们都走出口 ①，没有绕过**。

## `appMotionLite` 逐字（`glass_rim.dart:145-154`）
```
:145 /// 光向跟随的时长。
:147 /// 这里是 `AppDurations.fast`（120 ms）——比 Morrow 的 180 ms 快一档，
:148 /// 因为本项目只有 4 档时长，而 120 ms 那一档的语义正是
:149 /// 「指针悬停这类即时反馈」。180 ms 那种「跟着鼠标慢慢追」在…
:150 /// 属于新造一档，不值得。
:151 Duration appMotionLite(BuildContext context, Duration? override) =>
:152     MediaQuery.disableAnimationsOf(context)
:153     ? Duration.zero
:154     : (override ?? AppDurations.fast);
```

## 三个可核点
1. **⚠⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 「全仓库唯一的 reduced-motion 出口」**与代码不一致**：
   存在**两个**时长出口 —— `appMotion`（`soft_motion.dart:45`）与
   `appMotionLite`（`glass_rim.dart:151`）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 严重度候选 P3**（声明大于事实），**本批不立**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 理由**：`appMotionLite` 有**独立的成立理由**，
   写在自己的文件里、还有自己的头注解释为什么快一档 ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：**「唯一出口」型声明有两种破法** ——
   ① 有人**绕过**它（真问题）② 有人**合理地另立**一个（措辞问题）
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 本例是 ② ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 处置：改措辞（把「唯一」限定为
   「统一的时长换算里只用 `appMotion`」），而不是合并两个函数。**
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `appMotionLite` 的头注（`:145-150`）**又是那套完整格式**：
   **在 4 档时长里找到语义最接近的一档（120 ms = 指针悬停类即时反馈），
   并说明「180 ms 那种属于新造一档，不值得」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批正面）：**一个常量该取哪一档，
   最好的写法是「先列现有档、指出最接近的、说明为什么不新造」** ——
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 与 B0600 核的 `kBubbleReadingWidth = 480`
   （`:293`「每行 22 字左右是舒适区」）**是同一手法** ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 正面模式：魔数要配「为什么是这个档」。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而那三处**读布尔**的地方不算违反「时长出口」**
   （`streaming_indicator.dart:67` / `state_pill.dart:165` 要的是
   「要不要脉冲」这个**是/否**，不是「时长是多少」）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（承 B0611c 的三档引用质量）：
   判断「唯一」声明是否被违反，**要先确定那个「一」指的是哪一类能力** ——
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒ 「时长换算」是 1 个出口；**
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 「是否要动」是另一类能力，它本来就不该走时长出口**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 本批因此把
   `soft_motion.dart:1` 那句话**收窄**成可核的版本，而不是直接判它错。**

## 未核
`soft_motion.dart:166` 那处内联 · `AppDurations` 的 4 档具体值
· `state_pill.dart:14` 那句提到 `MediaQuery.disableAnimationsOf` 的注释说了什么
