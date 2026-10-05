# BATCH-0582b 落盘（极简）· 保活测试用 initState 计数，结构上能红

## 编号
`BATCH-0582.md` 已存在（内容是 `parallel-mods/ 17 份`）⇒ 本批落 `BATCH-0582b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0582.md
grep -n "testWidgets|expect(|initState|createState" \
  shell/flutter/test/settings_panel_keepalive_test.dart
sed -n '10,16p' 同上
```

## 测试逐字（要点）
```
:10 /// 本文件用 `initState` 计数把「有没有被重建」变成可断言的事实…
:11 /// 与 `stage_keepalive_test.dart` 同一手法（那条是给 iframe 用的，
:12 /// 这条是给设置面板用的，两者失败模式一样：**不报错，只是状…**
:14 /// # 为什么不用 `find.text(...)`
:16 /// 折叠之后内容**仍在树里**（那正是保活的代价），所以…

:47  State<_CountingPane> createState() => _CountingPaneState();
:54  void initState() {
:55    super.initState();
:81  State<_Host> createState() => _HostState();
:146   expect(inits, 1, reason: '首帧应该只建一次面板');
:152 testWidgets('开 → 关 → 再开，面板实例始终是同一个', …
:158   expect(dockWidth(t), greaterThanOrEqualTo(NavMetrics.paneWidth));
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ 这条断言结构上能红**：
   `initState` 计数 + `expect(inits, 1, reason: '首帧应该只建一次面板')`
   ⇒⇒ 若把 `:724` 换回 `if (settingsOpen) …`，计数会变 2/3 ⇒ **测试会红**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这是 B0500 判据的正面实例：测试名 + reason 说清了「要拦住什么」**。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ 而 `:14-16` 解释了为什么不用更常见的 `find.text`**：
   「折叠之后内容**仍在树里**（那正是保活的代价）」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据（重要）**：
   **保活会让「内容还在树上」成为常态，所以 `find.text` 在这里天然测不出折叠/展开** ——
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这就是「同一件事要用不同手段测」的典型**：
   **测「子树在不在」与测「内容在不在」是两个断言，别混用。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ `:11-12` 指出两个测试「失败模式一样：不报错，只是状…」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：两个测试若失败模式相同，就该共用同一套测法**
   ⇒⇒ ⇒⇒ 与 B0561 核的 `sameAs`/`identity` 分工**同族**（那里是「同一件事的两层」，
   这里是「两个对象的同一件事」）。
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ 与 B0580 第 2 点的判据对上**：
   `collapsible_panel.dart:13-16` 写「舞台那条纪律**已有测试**（`stage_keepalive_test.dart`），
   但设置面板自己**没有**」⇒⇒ 而现在 `settings_panel_keepalive_test.dart` **已存在**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 「第一处有测试 → 推广到第二处 → 第二处也有测试」这条链是完整的**。

## 未核
`:152` 那条 widget 测试的完整步骤（开→关→开怎么点）· `:158` 之后还有什么断言 ·
`InlineSettingsDock` 与 `CollapsiblePanel` 的分工 · CollapsiblePanel 的「三个实现要点」
