# BATCH-0583b 落盘（极简）· 三条测试，第三条带一句「先证明前置条件」

## 编号
`BATCH-0583.md` 已存在（内容是 `REGISTER-director-v0.md`）⇒ 本批落 `BATCH-0583b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0583.md
sed -n '152,190p' shell/flutter/test/settings_panel_keepalive_test.dart
```

## 三条测试（逐字要点）
```
:152 testWidgets('开 → 关 → 再开，面板实例始终是同一个', (…
:155   await t.tap(find.text('设置'));
:156   await t.pumpAndSettle();
:157   expect(dockWidth(t), greaterThanOrEqualTo(NavMetrics.paneWidth));
:159   await t.tap(find.byIcon(Icons.close));
:160   await t.pumpAndSettle();
:161   expect(dockWidth(t), lessThan(1), reason: '关 = 折到 0 宽');
:163   await t.tap(find.text('设置'));
:164   await t.pumpAndSettle();
:165   expect(dockWidth(t), greaterThanOrEqualTo(NavMetrics.paneWidth));
:168   expect(inits, 1, reason: '面板被重建了 —— 用户的滚动位置与字段…
:171 testWidgets('**滚动位置**真的留住了（这才是保活的用户可见收益…
:176   // 拖**可见的滚动视口**，不是拖内容：内容的中心点在视…
:177   // 从那里起手等于在面板外面滑，什么也不会发生。
:179   await t.drag(
:180     find.descendant(of: dockPanel(), matching: find.byType(Scrollable)),
:181     const Offset(0, -400),
:183   expect(panelScroll(t), greaterThan(100),
:185     reason: '**先得真的滚下去，否则这条断言什么也没证明**',
```

## 五个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 第一条测「实例同一个」，用 `inits == 1`；中间还测了 `dockWidth` 三态**
   （开 ⇒ ≥paneWidth / 关 ⇒ **< 1** / 再开 ⇒ ≥paneWidth）
   ⇒⇒ **⇒⇒⇒⇒ ⇒ 判据：断言要落在**可观察的量**上** ——
   `dockWidth` 是用户能看见的，`inits` 是内部计数器 ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 两条都写，才能既证「行为对」又证「实现没偷懒」。**
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 第三条测的正是 `collapsible_panel.dart:9-10` 列的第一项用户可见收益**
   （滚动位置回到顶部）⇒⇒ **⇒⇒⇒⇒ ⇒ 判据：头注列的失效面要能在测试里找到对应的一条。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:185` 那句 reason 是本仓测试里少见的写法**：
   「**先得真的滚下去，否则这条断言什么也没证明**」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据（可提炼）：断言前置条件要先被断言。**
   `expect(panelScroll(t) > 100)` 是**前置**，后面才是被测的「关掉再开位置还在」
   ⇒⇒ 与 B0400 系的「模型判据 / 门禁判据」**同族**。
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:176-177` 是一个「我踩过才知道」的注释**：
   「拖**可见的滚动视口**，不是拖内容：内容的中心点在视…从那里起手等于**在面板外面滑**」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：测试里写「怎么测才对」比写「测什么」更省后来的时间。**
5. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 但 `:179-181` 的 `find.descendant(of: dockPanel(), matching: find.byType(Scrollable))` 是 `pumpAndSettle` 之前**，
   断言前后顺序 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 未核**：`:183` 之后到下一条测试之间还有多少断言、
   是否验证了「关掉再开后 offset 不变」这一步。
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这是本批唯一没读到的段**，下一批补。

## 未核
`:186` 之后的断言（是否验证「关→开后 offset 不变」）·
`runScenario` / `panelScroll` / `dockPanel` / `dockWidth` 这四个 helper 的实现 ·
`stage_keepalive_test.dart` 是否同形
