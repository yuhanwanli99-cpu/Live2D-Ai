# BATCH-0584b 落盘（极简）· 四条测试：保活 / 滚动 / Tab 序 / 读屏

## 编号
`BATCH-0584.md` 已存在（内容是「第 6 处反证写入 FINDINGS」）⇒ 本批落 `BATCH-0584b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0584.md
sed -n '190,232p' shell/flutter/test/settings_panel_keepalive_test.dart
```

## 第三条测试的后半（:193-203）
```
:193  await t.tap(find.byIcon(Icons.close));
:194  await t.pumpAndSettle();
:195  offsetAfterClose = panelScroll(t);
:197  await t.tap(find.text('设置'));
:198  await t.pumpAndSettle();
:199  expect(
:200    panelScroll(t),
:201    closeTo(offsetAfterClose, 0.5),
:202    reason: '再打开时回到了顶部 —— 说明面板被重建了',
```

## 第四条测试（:207-231，摘）
```
:207 testWidgets('折起来的面板**不进 Tab 序、不被读屏念到、展开后恢…
:208   final SemanticsHandle handle = tester.ensureSemantics();
:214     // 折起来时（初始态）：
:215-218 expect(find.byType(_CountingPane), findsOneWidget,
          reason: '折叠不等于卸载 —— 这正是滚动位置能留住的原…');
:219     final Finder firstChip = find.byType(ChoiceChip).first;
:220-223 expect(Focus.of(t.element(firstChip)).canRequestFocus, isFalse,
          reason: '收起的面板还在 Tab 序里 —— 键盘用户会掉进看…');
:224-225 // 读屏：`ExcludeSemantics` 会把整棵子树从语义树里摘掉，
          // 所以正确的判据是「**找不到**它的标签」，不是「它被…
:226-229 expect(find.bySemanticsLabel(RegExp('外观与互动')), findsNothing,
          reason: '收起的面板仍会被读屏念到（ExcludeSemantics 没生效…
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 第三条的后半把 `offsetAfterClose` 抓在关掉之后、再开时与它比**
   （`closeTo(…, 0.5)`，容差 0.5px）⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 判据：保活要证的是「**值相等**」，
   不是「值不为 0」** ⇒⇒ 容差写成 `0.5` 而不是 `0`，也说明作者知道它会抖。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 第四条的 `canRequestFocus` 是**结构性能红**的判据**：
   把 `ExcludeSemantics` 去掉 ⇒ `isFalse` 立刻不成立 ⇒ 测试红
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 与 B0500「可证伪的测试优先于覆盖率」同族**。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:224-225` 是一句「判据怎么选才对」的注释**：
   「`ExcludeSemantics` 会把整棵子树从语义树里摘掉，
   所以正确的判据是「**找不到**它的标签」，不是「它被…」」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据（可提炼）：**否定式断言比肯定式更难写对**——
   「它被隐藏了」需要定义「隐藏」，而「找不到它」直接用 `findsNothing`。
   ⇒⇒ 与 B0604 核的 `NoSessionPrompts`（`enabled()->false`）**同族**：
   **把「不可用」表达成一个可查询的事实。**
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 四条测试合起来覆盖了保活的**四个代价面**：
   ① 实例被重建 ② 滚动位置丢 ③ 键盘用户掉进看不见的面板 ④ 读屏念到看不见的内容
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 而 `collapsible_panel.dart:8-10` 的头注只列了前两项**
   （滚动/焦点/草稿/内联结果）⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：
   **测试可以比头注更广** —— 后两项是实现时顺带发现并补上的，
   **头注没跟上**（轻微文档债，不立发现）。

## 未核
`runScenario` / `panelScroll` / `dockPanel` / `dockWidth` 四个 helper ·
`stage_keepalive_test.dart` 是否同形（只测保活还是也有 a11y 侧）·
`:232` 之后「展开后恢复」那一半
