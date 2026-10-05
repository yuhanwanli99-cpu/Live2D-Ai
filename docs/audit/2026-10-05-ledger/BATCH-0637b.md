# BATCH-0637b 落盘（极简）· 「截取一段再断言」的模式：三个 expect 全在抽取之前

## 编号
`BATCH-0637.md` 已存在（内容是「crates 真实清单（14 个 crate）」）⇒ 本批落 `BATCH-0637b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0637.md
grep -n "appearanceSectionSource()" shell/flutter/test/setting_wiring_test.dart
sed -n '52,58p' shell/flutter/test/setting_wiring_test.dart
```

## 三处调用（全部在 `setting_wiring_test.dart`）
```
:37  String appearanceSectionSource() =>            <- 定义（B0636 核过：两份逐字相同）
:53    final String ui = appearanceSectionSource();
:78    final String ui = appearanceSectionSource();
:149   final String ui = appearanceSectionSource();
```

## `:52-58` 逐字
```
:52  String exposedFitOptions() {
:53    final String ui = appearanceSectionSource();
:54    final int start = ui.indexOf("label: '铺法（图）'");
:55    expect(start, greaterThan(-1), reason: 'UI 里找不到「铺法」这个控件'
:56    final int end = ui.indexOf('onChanged:', start);
:57    expect(end, greaterThan(start), reason: '「铺法」控件没有 onChanged');
:58    return ui.substring(start, end);
```

## 四个可核点
1. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ `:54-58` 是「先定位、再截取、最后断言」的完整三步，
   而**两个 `expect` 都在 `substring` 之前** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批最值钱，可复用）：
   **截取型断言必须先验证「截取区间存在」** ——
   ⇒⇒ ⇒⇒ ⇒ `indexOf` 返回 -1 时 `substring(-1, …)` **要么抛异常、要么截到错误的一段**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒ 后果**：
   没有前两条 `expect`，**「控件被改名」的失败信息会变成一个无关的异常或一个误判**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 与 B0583 核的
   「改动会破坏 grep 式门禁」**同一族** ——
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 区别是方向：
   那条是「改代码会坏门禁」，这条是「**门禁自己坏掉时能不能说得清楚**」
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:54` 的锚点是**字面量**（`"label: '铺法（图）'"`），
   不是符号名 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（承 B0631b 核的「别名只有 colors 一种」）：
   **用字面量当锚点 = 把「UI 文本」变成了契约** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 收益**：
   改名/改标签**立刻会让这条变红**，而**不会**让后面 4 条 `DisplayPrefs.fit*` 断言给出假绿
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 代价**：文案微调会红
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（可提炼）：
   **「用字面量当锚点」是把 UI 文本纳入契约的一种选择，
   要明确决定而不是顺手写** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 顺带**：本条 `reason` 写「UI 里找不到…」
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 那正是**契约面**的正确措辞** ——
   ⇒⇒ ⇒⇒ ⇒ 它没说「代码有问题」，它说「**UI 里**没有」
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:56` 的结束锚点 `onChanged:` 是**结构名**（不是字面量）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（可复用）：
   **起点用「唯一字面量」、终点用「结构标记」** ——
   ⇒⇒ ⇒⇒ ⇒ 起点必须唯一才能定位，终点只要能**截断**就行
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 后果**：若终点也用字面量，
   文案一变就截断失败 ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 两种锚点各有分工，
   混用会让「哪一处在漂」变得不清楚**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:52 exposedFitOptions()` 这个 helper 本身没有头注** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（承 B0631b 第 ④ 点）：
   **一个 helper 的锚点选择（字面量 vs 结构名）是最需要注释的地方，
   因为它决定「什么改动会让这条变红」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 建议（P3，不立）：
   给 `exposedFitOptions` 加一句「锚点是 UI 文案，所以改文案会红」**

## 未核
`:78` 与 `:149` 两处截取的锚点各是什么（是否也是字面量）·
`display_prefs_test.dart` 那份 `appearanceSectionSource` 有没有被调用
· `_p4NewFieldsTests()` 是什么
