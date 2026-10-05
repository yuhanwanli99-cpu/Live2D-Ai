# BATCH-0581b 落盘（极简）· 已换成 CollapsiblePanel，且有专门的回归测试

## 编号
`BATCH-0581.md` 已存在（内容是「README 独有 22 份」）⇒ 本批落 `BATCH-0581b.md`。

## 命令（只读）
```
grep -n "CollapsiblePanel|settingsOpen && inlineSettings" lib/app/app_shell.dart
sed -n '630,636p' lib/app/app_shell.dart
```

## 结果
```
:633 /// 面板常驻树里（折叠**不卸载**，见 `CollapsiblePanel` /
:634 /// `settings_panel_keepalive_test.dart`）。失效条件只有三条，都真的与…
:724 child: CollapsiblePanel(
:736 // 见 `InlineSettingsDock` 与 `CollapsiblePanel` 的头注
:845 // **永远拖不动模型**）。与 `CollapsiblePanel` …
```
⇒ **原来的 `if (settingsOpen && inlineSettings) Positioned.fill(` 已不在 `app_shell.dart` 里**
⇒ ⇒ `:724` 已经是 `child: CollapsiblePanel(`。

## app_shell.dart:630-635 逐字
```
:630 /// 当前分区的**内容**（[AppShell.sectionBuilder] 的产物），按代际缓…
:632 /// 为什么不每帧重算（F-0005-2）：外壳会被聊天增量（毫秒级）…
:633 /// 面板常驻树里（折叠**不卸载**，见 `CollapsiblePanel` /
:634 /// `settings_panel_keepalive_test.dart`）。失效条件只有三条，都真的与…
:635 /// 有关：宿主代际、设置数据代际、换分区。三种宿主（内联 / …
```

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 上一批那个「反向漂移」不存在**：
   `app_shell.dart:11` 的「设置是**叠加物或浮层**」是**泛指**，
   而代码用 `Positioned.fill` + 折叠 ⇒ **两者不矛盾**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 撤回我上一批第 2 点里的 P3 候选**（B0271）。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ 而这次是「文档说了、代码也做了」**：
   `collapsible_panel.dart:10` 记的是**缺陷**（条件插入 ⇒ 子树重建），
   `:633-634` 记的是**现状与守卫**（常驻不卸载 + `settings_panel_keepalive_test.dart`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：一个组件的头注同时记「为什么不能那样写」和
   「现在这样写、且有测试」，才算把一次重构闭环；只记其一就是半份。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ `:632` 挂着 F-0005-2（聊天增量毫秒级 ⇒ 不能每帧重算）**
   而 `:634` 挂着「宿主代际 / 设置数据代际 / 换分区」三条失效条件
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：缓存要写「什么时候必须作废」**，
   而本仓**给了三条具体触发**而不是「适时刷新」
   ⇒⇒ 与 B0591 核的「一条禁令要带后果」**同族**：一条缓存规则带了它的**失效清单**。

## 未核
`settings_panel_keepalive_test.dart` 的断言（用不用 `initState` 计数）·
`CollapsiblePanel` 的「三个实现要点」· `InlineSettingsDock` 与 `CollapsiblePanel` 的分工
