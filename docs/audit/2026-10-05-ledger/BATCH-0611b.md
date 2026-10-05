# BATCH-0611b 落盘（极简）· BackgroundRuntimeScope：3 个字段 + 一条头注点名的回归

## 编号说明
`BATCH-0611.md` **已存在**（内容是 `--check` 三件事）⇒ 本批改落 `BATCH-0611b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0611.md
sed -n '60,90p'  appearance_background.dart
sed -n '90,110p' 同上
ls shell/flutter/test/appearance_background_runtime_test.dart
grep -n "testWidgets|expect(" 该测试
```

## 类体逐字（:60-95）
```
:60 class BackgroundRuntimeScope extends InheritedWidget {
:61   const BackgroundRuntimeScope({
:62     required this.index,
:63     required this.current,
:64     required this.hydrating,
:65     required super.child,
:66     super.key,
:67   });
:69   /// 轮播到了背景库第几项（来源不是背景库时没有意义）。
:70   final int index;
:72   /// 外壳此刻**真的会画**的那一项（null = 没东西画）。
:73   final BackgroundItem? current;
:75   /// 背景库字节是否还在水合（IndexedDB 读回中）。
:76   final bool hydrating;
:78   /// 取当前上下文；没有外壳注入时返回 null。
:79   static BackgroundRuntimeScope? maybeOf(BuildContext context) =>
:80       context.dependOnInheritedWidgetOfExactType<BackgroundRuntimeScope>();
:82   /// 「当前项」：外壳注入的优先。
:84   /// **没有注入**时回落 prefs.effectiveBackground（= 第 0 项）——只有
:85   /// AppearanceSection 的 widget 测试会走到这一支；生产路径由
:86   /// AppShell._settingsSurface 注入，回归见
:87   /// test/appearance_background_runtime_test.dart（那条钉的就是「一定注入…
:88   static BackgroundItem? currentOf(BuildContext context, DisplayPrefs prefs) =>
:89       maybeOf(context)?.current ?? prefs.effectiveBackground;
:91   @override
:92   bool updateShouldNotify(BackgroundRuntimeScope oldWidget) =>
:93       index != oldWidget.index ||
:94       current != oldWidget.current ||
:95       hydrating != oldWidget.hydrating;
:96 }
```

## 回归测试（**存在，已核**）
`shell/flutter/test/appearance_background_runtime_test.dart` —— 存在
```
:86  testWidgets('轮播走到第 2 张：设置页「当前」标记与铺法 / …
:103   expect(find.textContaining('图片 2 · '), findsOneWidget);
:104-… expect( ×4
```

## 五个可核点
   `test/appearance_background_runtime_test.dart` 在树上，且 `:86` 有一条
   `testWidgets('轮播走到第 2 张：…')`、`:103` 起 5 个 `expect`
   **头注点名了哪条测试，就要 `ls` 那条测试** —— 这次点名的**在**，
   而 B0591 核 `REGISTER-director-v0.md` 时点名的 `:44` 也**对得上**
   —— 但**这是抽样，不是全量**，故写成「倾向」而非「一律」。
   「**只有 `AppearanceSection` 的 widget 测试会走到这一支；生产路径由
   `AppShell._settingsSurface` 注入**」⇒⇒
   要写「谁会走它」** —— 写「没有注入时回落」只说了条件，
   写「只有 X 的测试会走」才说了**频率**。
   `current != oldWidget.current` 走的是 `BackgroundItem` 的 `operator ==`
   没重写 `==`，这里会**每次都是新对象 ⇒ 每次都 notify ⇒ rebuild 每次都发生**
   `InheritedWidget` 的通知成本全部押在 `==` 写没写对。
   ```
   :97  /// 库项的**身份**（F-0003-3：选中 / 展开都存身份，**不存下标**）
   :99  /// 下标在重排之后会指向别的项——那正是「勾选第 0 项、拖动后…
   :101 String backgroundItemIdentity(BackgroundItem item) => switch (item) {
   :102   BackgroundImage(:final String id) => "image:$id",
   :103   BackgroundPattern(:final int id) => "pattern:$id",
   ```
   它**带前缀**（`image:` / `pattern:`）⇒⇒
   判据：身份串要**带类型前缀** —— 否则 `id=1` 的 image 与 `id=1` 的 pattern
   这是与 B0565 核的 `settings.rs` 「`Select.default` 存在是因为
   『第一个选项不等于服务端缺省』」**同族**：值域里有没有哪个值本身带语义。**
   「为什么不直接用 `BackgroundImage.copyWith`：它的 `clearStyle` 是**三项一起**…
   而逐图编辑器要能只清一项；`copyWith(opacity: null)` 又表达不…」
   判据：**「为什么不直接用现成 API」是一个值得写进头注的问题** ——
   下一个改这里的人会想到 `copyWith`，而答案是「它表达不了」。

## 未核
`BackgroundItem` 是否重写 `==`（决定第 3 点的严重度）·
`AppShell._settingsSurface` 的注入点 · `main.dart`（1344）头注 ·
`_ModConfigTileState` 那 440 行
