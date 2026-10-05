# BATCH-0594b 落盘（极简）· SizeClass 有「唯一真源」，且 getter 互不重叠

## 编号
`BATCH-0594.md` 已存在（内容是「不被版本控制」）⇒ 本批落 `BATCH-0594b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0594.md
grep -rn "bool get hasInlineSettings|bool get isCompact|class SizeClass|enum SizeClass" lib/
sed -n '12,24p;45,62p' lib/design/breakpoints.dart
grep -rln "settingsHostOf" test/
```

## 逐字
```
breakpoints.dart:12 enum SizeClass {
:14   /// < 900：窄屏（手机 / 窄窗口）。导航走抽屉 + 底部弹层。
:15   compact,
:17   /// 900–1279：中屏。设置走底部浮层，舞台与聊天并排。
:18   medium,
:20   /// ≥ 1280：宽屏。设置侧板浮在舞台列之上（导航只有 AppBar 的「…
:21   /// 分区切换在面板内的 chip 行——左侧 rail 已于 2026-09-11 删除、…
:22   expanded,
:23 }
:…    /// 各档下限（含）。**唯一真源**——`sizeClassOf` 与测试都读这里。
:45 extension SizeClassX on SizeClass {
:49   bool get isCompact   => this == SizeClass.compact;
:50   bool get isMedium    => this == SizeClass.medium;
:51   bool get isExpanded  => this == SizeClass.expanded;
:53   /// 是否宽到可以常驻侧栏（聊天面板并排而不是上下叠）。
:54   bool get hasSideChat => this != SizeClass.compact;
:56   /// 是否宽到可以把设置做成「浮在舞台列右缘的侧板」而不是全…
:57   bool get hasInlineSettings => this == SizeClass.expanded;
:58 }
```
⇒ 阈值在 `:…`（那句「各档下限（含）」下的常量）——**本批未读那一段**。
⇒ `settingsHostOf` 在 `test/app_shell_layout_test.dart` 里**有引用**。

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 「**唯一真源**——`sizeClassOf` 与测试都读这里」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：出现「唯一真源」四个字时，
   **去数一数到底有几处读它**（本批：生产 1 处 `sizeClassOf` + 测试 1 处）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒ 承诺被兑现了** ⇒⇒ 与 B0591 核的 `check_public_secrets.py:5`
   （自称有 Gitleaks、实际 0 命中）**正好是一对** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：「唯一真源」这个说法本身是可核的，核它就能验仓库。**
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 三个 `isXxx` 互不重叠（各等于自己），
   而 `hasSideChat` 用 `!=`、`hasInlineSettings` 用 `==`**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：判据写法要选**能自证互斥**的那一种** ——
   `hasInlineSettings == expanded` 与 `isCompact == compact` **不可能同时真**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 上一批第 3 点那个「两个条件同时为真」的担心**因此消解**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒ 修正上一批 P3 候选**（B0593b 第 3 点）。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 三个枚举项的 `///` 都带了**数字区间**（< 900 / 900–1279 / ≥ 1280）
   ＋一句行为后果**（导航走抽屉 / 设置走底部浮层 / 设置侧板浮在舞台列之上）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：阈值枚举的文档写「区间 + 这一档会怎样」，
   读者不必去翻代码就知道自己落在哪**。
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:21` 那句把一次历史决策钉在枚举项上**：
   「分区切换在面板内的 chip 行——**左侧 rail 已于 2026-09-11 删除**」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：删掉一个 UI 元素后，在**它原来的位置**留一句「已删 + 日期」**，
   下一个人就不会把它当成漏做** ⇒⇒ 与 B0598 核的「已删除要在限定语里区分」**互补**。

## 未核
`sizeClassOf` 的实现与「各档下限」常量（`:…`）· `app_shell_layout_test.dart` 里
`settingsHostOf` 的断言 · `breakpoints.dart` 头注 · `hasSideChat` 的调用点
