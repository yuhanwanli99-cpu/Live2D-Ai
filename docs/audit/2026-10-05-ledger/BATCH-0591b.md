# BATCH-0591b 落盘（极简）· 「整页 Tab 顺序确定」只被局部钉住

## 编号
`BATCH-0591.md` 已存在 ⇒ 本批落 `BATCH-0591b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0591.md
grep -n "pageSettings" lib/app/app_shell.dart
grep -rln "FocusTraversalGroup|Tab 顺序|traversalOrder" test/
grep -n "FocusTraversalGroup|traversalOrder|Tab" test/semantics_test.dart
```

## 逐字
```
app_shell.dart:688   final bool pageSettings = host == SettingsHost.page;
app_shell.dart:839   body: pageSettings
app_shell.dart:855   // 外面再包一个 `FocusTraversalGroup`：整页的 Tab 顺序是确定的
app_shell.dart:856   // 不会因为某次重构而静默改变。

semantics_test.dart:195 group('规格 §9.2：焦点组与固定 Tab 顺序', () {
semantics_test.dart:196   testWidgets('音频条是 FocusTraversalGroup，且音量排在静音前面'
semantics_test.dart:215     expect(find.byType(FocusTraversalGroup), findsWidgets);
```

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `pageSettings` 的判据是 `host == SettingsHost.page`**（`:688`）
   ⇒⇒ 与 `PageCrossFade`（`:840`）配套：**三种宿主里只有「整页」这一种走 cross-fade**
   ⇒⇒ 与 `app_shell.dart:634-635` 说的「三种宿主（内联 / …）共用同一实例是安全的
   ——同一时刻只有一个在树上」**对得上**。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 「整页的 Tab 顺序是确定的」这句承诺，只被**局部**钉住**：
   `semantics_test.dart:196` 那条测的是「**音频条**是 FocusTraversalGroup，
   且**音量排在静音前面**」⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 这是「一条组的内部顺序」，
   不是「整页的 Tab 顺序」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：注释里的「整页 / 全部 / 统一」这类全称词，
   要在测试里找到同名的那一条**；找不到就是**承诺大于守卫**。
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 严重度候选 P3**（注释比测试说得宽，不立）。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 而 `:215 expect(find.byType(FocusTraversalGroup), findsWidgets)` 是
   「存在性」断言**（`findsWidgets` = 至少一个）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：存在性断言不区分「包在最外层」与「散落各处」**
   ⇒⇒ ⇒⇒ 上面那条 `testWidgets` 才补了顺序 ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 两条断言互补：一条证「有这个类」、一条证「顺序对」；
   **缺的是「它在最外层」**（`:855` 说的「外面再包一个」）。
   ⇒⇒ 与 B0584b 第 1 点「行为 + 实现各一条」**同族**。

## 未核
`SettingsHost` 的三个取值 · `semantics_test.dart:216` 之后的顺序断言全文 ·
`:688` 的 `host` 由谁决定
