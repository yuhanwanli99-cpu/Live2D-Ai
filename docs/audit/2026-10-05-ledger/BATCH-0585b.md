# BATCH-0585b 落盘（极简）· stage_keepalive 只测保活，没有 a11y 侧

## 编号
`BATCH-0585.md` 已存在（内容是 `settings_panel_keepalive_test.dart`）⇒ 本批落 `BATCH-0585b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0585.md
grep -n "testWidgets|canRequestFocus|bySemanticsLabel|initState" \
  shell/flutter/test/stage_keepalive_test.dart
```

## 结果
```
:17  /// 口型时间轴清零、当前动作中断。用 `initState` 计数就是把…
:30  void initState() { super.initState(); }
:111 testWidgets('打开/关闭设置侧板 → 仍是同一个舞台实例', (…
:124 testWidgets('**切换 8 个分区** → 仍是同一个舞台实例', …
:140 testWidgets('**跨断点改宽度**（expanded → medium → compact → …
```
⇒ **`canRequestFocus` 与 `bySemanticsLabel` 在这份文件里 0 命中**
⇒ `settings_panel_keepalive_test.dart` 有（`:222` / `:228`）⇒ **两份不同形**。

## 三个可核点
1. **⇒⇒⇒⇒ 两份都用 `initState` 计数** ⇒⇒ 保活这同一件事，**两处测法相同**
   ⇒⇒ 与 B0584b 第 3 点「失败模式相同就该共用测法」**一致**。
2. **⇒⇒⇒⇒ 而 `stage_keepalive` 多测了两个「失效条件」，`settings_panel` 多测了 a11y**
   - `stage_keepalive`：① 开/关侧板 ② **切换 8 个分区** ③ **跨断点改宽度**
     ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 这正是 `app_shell.dart:634` 写的三条失效条件里的两条**
     （宿主代际 / 设置数据代际 / 换分区）⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 头注的「失效清单」在测试里被逐条兑现**。
   - `settings_panel`：多出 **Tab 序** 与 **读屏** ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 「换一个对象重做一条纪律」时，测试可以顺手多测几面**。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒ 一个待核的缺口**：舞台侧是**贴在 iframe 上的
   `StagePointerInterceptor`**（AGENTS 写「键盘/读屏是否也被挡住」是另一条纪律）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 若 `stage_keepalive` 只保住了实例、没守住「折起时舞台不进 Tab 序」，
   那 a11y 侧只有设置面板有守卫** ⇒⇒ **本批不下结论**（需读 `stage_keepalive` 全文 +
   `StagePointerInterceptor` 头注）**⇒ 下一批核**。

## 未核
`stage_keepalive_test.dart` 全文（是否真的没有 a11y 断言）·
`StagePointerInterceptor` 头注关于 Tab/读屏的说法 · `:140` 之后的断点测试细节
