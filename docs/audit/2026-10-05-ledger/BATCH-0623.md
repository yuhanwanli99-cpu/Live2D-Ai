# BATCH-0623 落盘（极简）· 「4 档全部具名」+ 一张「只服务测试」的 `registry`

## 编号
`BATCH-0623.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0623.md
sed -n '870,900p' lib/design/tokens.dart
grep -rn "AppDurations.reveal" shell/flutter/lib/ --include=*.dart
```

## 逐字（`tokens.dart:872-893`）
```
:872 /// 动效时长：**4 档，全部具名**。
:873 abstract final class AppDurations {
:874   /// hover / pressed / 开关 / 徽标切换。
:875   static const Duration fast = Duration(milliseconds: 120);
:877   /// 内容切换、消息渐入、横幅滑入。
:878   static const Duration base = Duration(milliseconds: 200);
:880   /// 模态 / sheet 入场。
:881   static const Duration slow = Duration(milliseconds: 320);
:883   /// **仅此一处**长动画：启动揭示。
:884   static const Duration reveal = Duration(milliseconds: 600);
:886   /// **登记表（只服务测试）**。
:887   static const Map<String, Duration> registry = <String, Duration>{
:888     'fast': fast, 'base': base, 'slow': slow, 'reveal': reveal,
:891   };
:892 }
:894 /// 动效曲线：**只允许两条**。
:895 abstract final class Motion {
:896   /// 入场 / 出场（快出慢收）。AIRI 与 Nexus 独立收敛到同一条…
:897   static const Cubic enter = Cubic(0.16, 1.0, 0.3, 1.0);
```

## `AppDurations.reveal` 的 4 处引用
```
app/app_shell.dart:858      （注释）
ui/soft_motion.dart:128     （注释：时长取 `AppDurations.reveal`）
ui/soft_motion.dart:155     （真实调用：duration: AppDurations.reveal）
settings/sections/dev_tools_section.dart:1323  （注释）
```

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ `:872`「4 档，**全部具名**」**经核成立**
   —— 恰好 4 个成员，且 `:874/:877/:880/:883` **每一档都带一句「用它做什么」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（正面，可复用）：档位/枚举型常量的注释写
   「**这一档服务哪些动作**」，比写「这一档是多少毫秒」有用** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ 与 B0594b 核的 `SizeClass`
   「区间 + 这一档会怎样」**同族** ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 正面模式 P39（新）：
   **规模小的时候，枚举 + 每项一句用途，胜过一张表。**
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:883` 的「**仅此一处**长动画：启动揭示」**经核成立**
   —— `reveal` 的**真实调用点只有 1 处**（`soft_motion.dart:155`），
   另外 3 处是注释
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 判据（可复用）：数「唯一」时要把**注释引用**和
   **代码调用**分开数** ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这次两者恰好同向
   （1 处调用、3 处注释）⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒ 对照 B0621：`MediaQuery.disableAnimationsOf`
   那个「唯一」是把 2 个**函数**都算成出口 ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 同样是数「唯一」，一次成立一次不成立
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ 判据：「唯一」型声明的核法有两种：
   数**调用点**、数**定义点** —— 两者要分别报。**
3. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ `:886` 的 `registry` 头注写「**登记表（只服务测试）**」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批最值钱，可直接复用）：
   **当一个常量集合需要「全量」测试时（而不是逐个点名测），
   给它一张 `registry`，让测试能遍历它** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒ 收益有三层**：
   ① 遍历 ⇒ 新增一档**自动进测试**（不用记得补一条）
   ② 可断言「集合的大小 / 成员名 / 是否有未具名的项」
   ③ 它自己**就是一份可读的清单**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 与 B0605b 核的 `NavMetrics`「不为负且有序」
   那条不变式测试**是同一思路的两种形态**：
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 那条是「断言关系」、
   这条是「提供遍历入口」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒ ⇒ 判据：
   写不变式测试有两条路 —— ① 断言具体关系 ② 提供让测试能遍历的登记表；
   ②更耐用（新增成员自动覆盖），①更直观。**
   ⇒⇒ ⇒⇒ ⇒ 顺带：`:894` 的「动效曲线：**只允许两条**」+ `:896`
   「AIRI 与 Nexus 独立收敛到同一条」—— 又一个「小集合 + 外部证据」的正面例
   （与 P5 的「OBS 改名」同族，B0613c 第 ③ 点核过）

## 未核
`registry` 到底被哪个测试消费（`grep -rn "AppDurations.registry"`）·
`Motion` 的两条曲线全文与调用点 · `thinkingBreath` / `hintDelay` 属不属于
`AppDurations`（B0622 第 ③ 点的收窄待确认）
