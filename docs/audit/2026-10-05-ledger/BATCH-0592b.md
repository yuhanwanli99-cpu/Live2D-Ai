# BATCH-0592b 落盘（极简）· 固定顺序用 NumericFocusOrder（有守卫）+ 快捷键四条

## 编号
`BATCH-0592.md` 已存在 ⇒ 本批落 `BATCH-0592b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0592.md
grep -n "enum SettingsHost" -A4 lib/app/app_shell.dart      -> 零命中
sed -n '216,232p' test/semantics_test.dart
```

## 逐字（:216-232）
```
:216 // `NumericFocusOrder` 是**固定顺序**的唯一写法；顺序跟随 Widge…
:217 // 会在重构时静默改变（规格 §9.2-1）。
:218 expect(find.byType(FocusTraversalOrder), findsWidgets);
:219 }); });                     <- 结束 §9.2 的 group

:222 group('规格 §9.3：键盘', () {
:223   test('快捷键清单包含四条，且按键文案按平台给前缀', () {
:224     final List<ShortcutHelpEntry> mac = shortcutHelp(isMacOS: true);
:225     final List<ShortcutHelpEntry> pc  = shortcutHelp(isMacOS: false);
:226     expect(mac, hasLength(4));
:227     // macOS 用 ASCII `Cmd` 而不是 `⌘`：后者不在自托管子集里，…
:228     // CanvasKit 去 fonts.gstatic.com 拉字体（断网即豆腐块）。见
:229     // `app_shortcuts.shortcutModifierLabel` 的注释与 `font_subset_test.dart`…
:230     expect(mac.first.keys, contains('Cmd'));
:231     expect(mac.first.keys, isNot(contains('⌘')));
:232     expect(pc.first.keys, contains('Ctrl'));
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:216-218` 把「整页 Tab 顺序确定」这句承诺**真正落地了**
   ⇒⇒ 手段是 `NumericFocusOrder`（注释明说「**是固定顺序的唯一写法**」）
   ＋ `expect(find.byType(FocusTraversalOrder), findsWidgets)`
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 上一批那条 P3 候选（承诺大于守卫）**部分撤回**：
   **「顺序会随 Widget 树静默改变」这个具体风险有守卫**（用了 `NumericFocusOrder`
   ⇒ 顺序不依赖 Widget 树）⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 真正没被钉的只剩
   「这个组**在最外层**」**（那是上一批第 ③ 点，不是「顺序」）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：`FocusTraversalGroup` 决定**谁参与**，
   `NumericFocusOrder` 决定**什么次序**——两件事、两处 API、两种守卫，
   **本仓两样都用了** ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 与 B0561 核的 `sameAs` / `identity` 分工**同族**。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:217` 又一次是「防止重构静默改变」**（第三次出现这个句式）
   ⇒⇒ 第一次 B0590b 的 `FocusTraversalGroup`、第二次这里、第三次待查
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：同一句式在一份文件里出现多次，
   说明它是这个仓的**写作习惯** ⇒⇒ 值得单独记进正面模式。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:226 expect(mac, hasLength(4))` 把「四条」钉住了**，
   且 `:230-231` **同向 + 反向各一条**（`contains('Cmd')` 与 `isNot(contains('⌘'))`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：断言「应该是 A」时补一条「不应该是非 A」，
   才挡得住「A 恰好等于别的东西」**。
   ⇒⇒ 而 `:227-229` 给出了**为什么不能用 `⌘`**：不在自托管子集里 ⇒ 断网豆腐块
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：字体子集这类约束要在**断言里**钉住，
   而不只是写在 AGENTS 的前端约定里**（AGENTS 那条本审计 B0007 核过有 `font_subset_test.dart` 守卫，
   **这里也有第二处 `isNot(contains('⌘'))`** ⇒⇒ 两处守卫，**同一约束**）。
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `enum SettingsHost` 在 `app_shell.dart` 里零命中** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 上一批问的「三个取值」得换个地方找** ⇒ **本批不猜**，列入未核。

## 未核
`SettingsHost` 的定义位置与取值 · `semantics_test.dart:233` 之后还有什么 ·
`NumericFocusOrder` 的具体数值（有没有在测试里钉住数值本身）· §9.2 的 group 是否还有别的组
