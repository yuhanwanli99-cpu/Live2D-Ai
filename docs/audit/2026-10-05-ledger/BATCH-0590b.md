# BATCH-0590b 落盘（极简）· enabled: settingsOpen = 折叠态，因果链闭合

## 编号
`BATCH-0590.md` 已存在（内容是「W9 正文 = 明确写了…」）⇒ 本批落 `BATCH-0590b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0590.md
sed -n '838,860p' shell/flutter/lib/app/app_shell.dart
```

## 逐字（:840-849 与 :854-857）
```
:840 body: pageSettings
:841   ? PageCrossFade(
:842       showSecond: settingsOpen,
:843       first: body,
:843       second: StagePointerInterceptor(
:844         // 收起时把舞台那一块还给 iframe（否则 compact 下
:845         // **永远拖不动模型**）。与 `CollapsiblePanel` 那处
:846         // 同一类问题、同一个开关。
:847         enabled: settingsOpen,
:848         child: _compactSettingsPage(),
:849       )

:854 // 快捷键挂在**根**（`CallbackShortcuts` 最省样板，且不吞未匹…
:855 // 外面再包一个 `FocusTraversalGroup`：整页的 Tab 顺序是确定的
:856 // 不会因为某次重构而静默改变。
```

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 上一批的因果链闭合了**：`:847 enabled: settingsOpen`
   ⇒⇒ 折叠收起 ⇒ `settingsOpen == false` ⇒ 垫层关 ⇒ 舞台那一块还给 iframe
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 与 `:846` 的「与 `CollapsiblePanel` 那处**同一类问题、同一个开关**」逐字对上**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：注释里说「同一个开关」时，
   **那个「同」必须能被两处代码对上** —— 本批对上了（折叠 ✓ / 断线 ✗）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ 修正上一批的说法**：
   `:722` 的 `enabled: wsStatus != connected` **不是**「同一个开关」，
   它是**另一个理由**（断线时不需要收指针）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 结论：两个 `enabled` 各有各的理由，都与垫层存在理由同向**
   ⇒⇒ 与上一批第 2 点「`enabled` 条件要与垫层存在理由同向」**一致**。
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:841-842` 用 `PageCrossFade(showSecond: settingsOpen, first: body, …)`**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：折叠的宿主也是 `settingsOpen`，所以两处条件天然同源**
   ⇒⇒ ⇒⇒ `first: body` / `second: _compactSettingsPage()` ⇒⇒ 两侧与 `settingsOpen` 同步
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 加上 `:847`，这一小段里 `settingsOpen` 出现三次且含义一致**。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:854-856` 是一句「防止重构静默破坏」**：
   「外面再包一个 `FocusTraversalGroup`：整页的 Tab 顺序是确定的，
   **不会因为某次重构而静默改变**」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：写「我加了个 X」时要写**「不加会静默坏在哪」**
   ⇒⇒ ⇒⇒ 与 B0591「一条禁令要带后果」**同族**，
   ⇒⇒ ⇒⇒ 也是 B0584b 第 3 点「否定式断言更稳」的**注释版**。

## 未核
`pageSettings` 这个条件（什么屏宽下走 compact 页）·
`FocusTraversalGroup` 那处是否也有测试 · `AppDurations.reveal` 那个启动淡入
