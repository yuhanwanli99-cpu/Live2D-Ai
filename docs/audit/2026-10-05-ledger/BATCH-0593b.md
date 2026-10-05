# BATCH-0593b 落盘（极简）· SettingsHost 三个取值 + 一个纯映射函数

## 编号
`BATCH-0593.md` 已存在（内容是「缺省启用 vs 缺省停用」）⇒ 本批落 `BATCH-0593b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0593.md
grep -rn "SettingsHost" lib/ | grep -v "^\S*:[0-9]*:\s*///"
sed -n '76,100p' lib/app/nav_host.dart
```

## 逐字（:82-99）
```
:82 enum SettingsHost {
:84   /// 内联侧板（浮在舞台列之上，不占列宽）。
:85   inline,
:87   /// 页内整页切换（compact）。
:88   page,
:90   /// 底部浮层（medium）。
:91   sheet,
:92 }
:94 /// 断点 → 宿主。
:95 SettingsHost settingsHostOf(SizeClass sizeClass) {
:96   if (sizeClass.hasInlineSettings) return SettingsHost.inline;
:97   if (sizeClass.isCompact) return SettingsHost.page;
:98   return SettingsHost.sheet;
:99 }
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 三个取值，每个的 `///` 都写了**断点名**：
   `inline`（内联侧板 / 浮在舞台列之上，不占列宽）· `page`（**compact**）· `sheet`（**medium**）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：枚举项的文档写「它对应哪一档」，
   比写「它是什么」有用**（读的人要判断的是「我这一屏会是哪个」）。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `settingsHostOf` 是一个**纯映射**（`:95-99`）**
   ⇒⇒ 三行 if、无副作用、无状态 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 可单测** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒ 判据：把「断点 → 宿主」抽成一个纯函数后，
   `app_shell.dart:686-688` 那三个 bool（`host` / `inlineSettings` / `pageSettings`）
   就可以各自被单独钉住** ⇒⇒ 与 B0591b 第 1 点「只有 page 走 cross-fade」**对得上**。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 但 `:97` 用 `isCompact`、`:98` 用**兜底** `return sheet`**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：兜底的那一档必须是**安全的一档**。
   这里 `sheet`（底部浮层）是 medium 档 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 若 `hasInlineSettings`
   与 `isCompact` 同时为真，先命中 inline** ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 顺序即优先级，
   这条没写在注释里** ⇒ **P3 候选，不立**（一行三段 if 自明）。
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:78-82` 解释了 compact 为何改成整页换**：
   「观感上像『弹了两次』…顺带把『**最多 2 层披露**』变成 **1 层**——这比原来更符合渐进…」
   「舞台仍然保活：`PageCrossFade` 用 `Offstage`（**不 paint，但 Element 还在**）」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：同一段注释里给了**设计理由**（2 层→1 层）
   与**实现约束**（舞台保活）⇒⇒ ⇒⇒ 这正是 B0581b「症状 → 根因」写法的短版
   ⇒⇒ ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ 与 B0584b 第 4 点「保活让内容留在树上」互为印证**。

## 未核
`SizeClass.hasInlineSettings` / `isCompact` 的阈值 ·
`settingsHostOf` 有没有单测 · `:76-82` 那段注释的完整上文
