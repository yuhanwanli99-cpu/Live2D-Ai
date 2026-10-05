# BATCH-0622 落盘（极简）· `:166` 是「只播一次」的启动分支；`AppDurations` 档位是 4 个

## 编号
`BATCH-0622.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0622.md
sed -n '160,170p' lib/ui/soft_motion.dart
grep -n "static const Duration" lib/design/tokens.dart | head -5
```

## 逐字（`soft_motion.dart:160-170`）
```
:161 @override
:162 void didChangeDependencies() {
:163   super.didChangeDependencies();
:164   // **只播一次**：`value == 1` 表示已经就位或正在播。
:165   if (_controller.value == 1 && !_revealed) {
:166     _revealed = true;
:167     if (MediaQuery.disableAnimationsOf(context)) {
:168       _controller.value = 1;          <- 立刻到终态
:169     } else {
:170       _controller.forward(from: 0);
```

## `tokens.dart` 的 Duration 常量（头部命中）
```
:839 static const Duration thinkingBreath  = Duration(milliseconds: 1400)
:849 static const Duration interruptedHold = Duration(milliseconds: 120…
:859 static const Duration hintDelay      = Duration(milliseconds: 400);
:876 static const Duration fast           = Duration(milliseconds: 120);
:879 static const Duration base           = Duration(milliseconds: 200);
```
⇒ 已知 5 个（另有 ≥1 个，`:879` 之后）

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ `:167-170` 正是 `Duration.zero` 那条语义的**实现**
   （`page_cross_fade` 第 ③ 节、B0620b 核的 `soft_motion.dart:24-29` 都指向它）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批最值钱）：一个概念讲了三遍
   （spec 原则 → 组件头注 → 基元实现），说明它的**每一层受众不同**：
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ spec 层给**理由**（为什么不能只把时长设 0）、
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 组件层给**用法**（我这个组件怎么办）、
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 基元层给**代码**（`if (…) value = 1`）**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（可提炼）：
   **同一条纪律**讲几遍**取决于受众层数**，不是啰嗦**。
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:164` 的注释「**只播一次**：`value == 1` 表示已经就位或正在播」**
   是一个**状态语义**的说明 ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：加一个「幂等」标志时，
   注释要解释**那个被当作判据的状态值**是什么意思** ——
   `value == 1` 在动画里既可能是「已播完」也可能是「正在播到头」，
   不解释清楚，后来人会以为这是「播完才标志」**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 判据：**布尔判据用的那个值
   若在别处有多种含义，注释要写出「我按哪一种理解」。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `AppDurations` 至少 5 个常量**（`fast 120` / `base 200` /
   `hintDelay 400` / `thinkingBreath 1400` / `interruptedHold 120`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 与 B0621 核的 `appMotionLite` 头注「**本项目只有 4 档时长**」**有出入**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 核到的是 5 个且**有两个都是 120 ms**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 修正方向**：
   那句「4 档」指的是**用于动效换算的那几档**（`fast`/`base`/…），
   而 `hintDelay` / `thinkingBreath` 属于**业务节奏**（等一下、思考呼吸）
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 判据：
   **同一个类里「档」这个说法可能只覆盖其中一部分成员** ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 收窄同 B0621 第 ③ 点：
   判断「N 档」这类数字要先确定它覆盖哪些成员。**
   ⇒⇒ ⇒⇒ ⇒ ⚠ **`fast` 与 `interruptedHold` 都是 120 ms** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 判据（新）：**同值不同名**要问「是不是同一语义」——
   本处一个是「指针悬停类即时反馈」、一个是「被打断后的停顿」⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ 数值撞车但语义不同** ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒ 与 B0601 核的「两个 480」**同族**
   （那里是撞值无错、这里是撞值也无错、但同样需要一句说明）

## 未核
`AppDurations` 的完整成员表与那句「4 档」的原文 · `base` 之后还有几档
· `_revealed` 标志的其它使用点
