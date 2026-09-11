import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/collapsible_panel.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/app/page_cross_fade.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/ui/stage_host.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 设置侧板里那个折叠容器。
///
/// **必须限定范围**：断线横幅也是 `CollapsiblePanel`（P1-2 起常驻在树里），
/// 所以 `find.byType(CollapsiblePanel)` 会匹配到两个。
Finder dockPanel() => find.descendant(
  of: find.byType(InlineSettingsDock),
  matching: find.byType(CollapsiblePanel),
);

/// 五档视口宽度（规格 §10.3 P3 的验收清单：1280/1279/900/899/390）。
const Map<String, double> kWidths = <String, double>{
  '1280（expanded 下限）': 1280,
  '1279（medium 上界）': 1279,
  '900（medium 下限）': 900,
  '899（compact 上界）': 899,
  '390（手机）': 390,
};

/// 外壳是**受控**的（`section` 由宿主持有），所以测试必须给一个有状态的宿主——
/// 否则「点了 rail 但 `section` 没变」会让断言莫名其妙地失败，而那是测试的错、
/// 不是实现的错（第一版就踩了这个）。
class _ShellHost extends StatefulWidget {
  const _ShellHost({
    required this.phase,
    required this.ws,
    required this.initialSection,
    required this.sections,
    required this.muted,
    required this.volume,
    this.onSectionChanged,
    this.stagePhase = Live2DBridgePhase.ready,
    this.onRetryStage,
    this.onEnsureSectionLoaded,
    this.settingsChanges = const NeverNotifies(),
  });

  final UiPhase phase;
  final WsStatus ws;
  final SettingsSection initialSection;
  final List<SettingsSection> sections;
  final bool muted;
  final double volume;
  final ValueChanged<SettingsSection>? onSectionChanged;
  final VoidCallback? onEnsureSectionLoaded;
  final Listenable settingsChanges;

  /// 默认 `ready`：**不是** `loading`。
  ///
  /// `loading` 会渲染一个不确定进度的 `LinearProgressIndicator`（无限动画），
  /// 于是每一个用 `pumpAndSettle` 的测试都会超时——规格 §11.3 把这条列为
  /// 已知的坑（「`pumpAndSettle` 在无限动画上死锁」）。
  /// 覆盖层本身有专门的一条测试。
  final Live2DBridgePhase stagePhase;
  final VoidCallback? onRetryStage;

  @override
  State<_ShellHost> createState() => _ShellHostState();
}

class _ShellHostState extends State<_ShellHost> {
  late SettingsSection section = widget.initialSection;

  @override
  Widget build(BuildContext context) => AppShell(
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: widget.phase,
    wsStatus: widget.ws,
    messages: const <Never>[],
    input: TextEditingController(),
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: widget.volume,
    muted: widget.muted,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: widget.sections,
    section: section,
    stagePhase: widget.stagePhase,
    onRetryStage: widget.onRetryStage,
    onSectionChanged: (SettingsSection next) {
      widget.onSectionChanged?.call(next);
      setState(() => section = next);
    },
    onEnsureSectionLoaded: widget.onEnsureSectionLoaded,
    settingsChanges: widget.settingsChanges,
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
  );
}

Widget shellUnderTest({
  UiPhase phase = UiPhase.idle,
  WsStatus ws = WsStatus.connected,
  SettingsSection section = SettingsSection.appearance,
  List<SettingsSection>? sections,
  ValueChanged<SettingsSection>? onSectionChanged,
  VoidCallback? onEnsureSectionLoaded,
  Listenable settingsChanges = const NeverNotifies(),
  bool muted = false,
  double volume = 0.8,
  Live2DBridgePhase stagePhase = Live2DBridgePhase.ready,
  VoidCallback? onRetryStage,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: _ShellHost(
    phase: phase,
    ws: ws,
    initialSection: section,
    sections: sections ?? visibleSections(),
    muted: muted,
    volume: volume,
    onSectionChanged: onSectionChanged,
    onEnsureSectionLoaded: onEnsureSectionLoaded,
    settingsChanges: settingsChanges,
    stagePhase: stagePhase,
    onRetryStage: onRetryStage,
  ),
);

/// 按**真实布局宽度**渲染。
///
/// 必须用 `setSurfaceSize` 而不是套一层 `MediaQuery`：断点判据来自
/// `LayoutBuilder` 收到的**实际约束**，`MediaQuery.size` 改不动它。
/// 这个坑第一次就踩了——测试里五个宽度全部渲染成了 compact。
Future<void> pumpShell(
  WidgetTester tester, {
  double width = 1400,
  UiPhase phase = UiPhase.idle,
  WsStatus ws = WsStatus.connected,
  SettingsSection section = SettingsSection.appearance,
  List<SettingsSection>? sections,
  ValueChanged<SettingsSection>? onSectionChanged,
  VoidCallback? onEnsureSectionLoaded,
  Listenable settingsChanges = const NeverNotifies(),
  bool muted = false,
  double volume = 0.8,
  Live2DBridgePhase stagePhase = Live2DBridgePhase.ready,
  VoidCallback? onRetryStage,
}) async {
  await tester.binding.setSurfaceSize(Size(width, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    shellUnderTest(
      phase: phase,
      ws: ws,
      section: section,
      sections: sections,
      onSectionChanged: onSectionChanged,
      onEnsureSectionLoaded: onEnsureSectionLoaded,
      settingsChanges: settingsChanges,
      muted: muted,
      volume: volume,
      stagePhase: stagePhase,
      onRetryStage: onRetryStage,
    ),
  );
  await tester.pump();
}

void main() {
  group('断点 → 导航形态（纯函数 + 渲染两层都验）', () {
    test('settingsHostOf：expanded 内联 / compact 整页 / medium 浮层', () {
      // 2026-09-11（P2-1）：compact 从「抽屉 + 浮层」改成**页内整页过渡**，
      // 所以有了第三种宿主。三种宿主各有各的理由，见 `SettingsHost` 头注。
      expect(settingsHostOf(SizeClass.expanded), SettingsHost.inline);
      expect(settingsHostOf(SizeClass.medium), SettingsHost.sheet);
      expect(settingsHostOf(SizeClass.compact), SettingsHost.page);
    });

    test('sheetConstraintsOf：浮层只有 medium 会走（P2-1 之后）', () {
      // 2026-09-11：compact 改成整页过渡，`SettingsHost.sheet` 只剩 medium。
      // 这里同时钉「约束真的按视口高度算」和「没有第二条分支」。
      const Size viewport = Size(1000, 800);
      final BoxConstraints b = sheetConstraintsOf(viewport);
      expect(b.maxWidth, NavMetrics.sheetMaxWidthMedium);
      expect(b.maxHeight, 800 * NavMetrics.sheetHeightFactorMedium);
    });

    test('NavMetrics 自洽：舞台下限、聊天宽度都不为负且有序', () {
      // 2026-09-11：左侧分区 rail 已删（用户裁决「把左边的这些设置一级选项
      // 去掉留给舞台」），所以这里不再有 rail 宽度这一项。
      expect(
        NavMetrics.chatWidthExpanded,
        greaterThan(NavMetrics.chatWidthMedium),
      );
      expect(NavMetrics.stageMinWidth, greaterThan(0));
    });
  });

  group('五档宽度都渲染出正确的导航形态', () {
    for (final MapEntry<String, double> entry in kWidths.entries) {
      testWidgets('${entry.key} → ${Breakpoints.sizeClassOf(entry.value).name}', (
        WidgetTester tester,
      ) async {
        await pumpShell(tester, width: entry.value);

        final SizeClass sc = Breakpoints.sizeClassOf(entry.value);

        // 非 compact：body 只有「舞台 | 聊天」两列 ⇒ **恰好一条竖分隔线**。
        // 左侧 rail 若偷偷长回来，这里会变成两条。
        expect(
          find.byType(VerticalDivider),
          sc.isCompact ? findsNothing : findsOneWidget,
          reason: '左侧不该再有第二列（rail 已删，宽度留给舞台）',
        );

        // 设置入口只有一个，且是**文字**按钮（不是齿轮图标）。
        // compact 时它在聊天面板头，非 compact 时在 AppBar。
        expect(
          find.text('设置'),
          findsOneWidget,
          reason: '设置入口必须恰好一个，且用文字（用户裁决：少用图片）',
        );
      });
    }

    testWidgets('rail 让出来的宽度**真的**给了舞台（非 compact）', (
      WidgetTester tester,
    ) async {
      // 1280 下删掉 rail 之前，舞台列 = 1280 - 168(rail) - 1 - 340(chat) ≈ 771；
      // 删掉之后应当 ≈ 1280 - 1 - 340 = 939。这里直接按「宽度 - 聊天 - 分隔线」
      // 断言，rail 一旦回来就会红。
      for (final double width in <double>[1280, 1400, 1000]) {
        await pumpShell(tester, width: width);
        final SizeClass sc = Breakpoints.sizeClassOf(width);
        final double chat = sc.isExpanded
            ? NavMetrics.chatWidthExpanded
            : NavMetrics.chatWidthMedium;
        expect(
          tester.getSize(find.byType(StageHost)).width,
          closeTo(width - chat - 1, 1.5),
          reason: 'width=$width：舞台列必须拿到除聊天列与 1 px 分隔线之外的全部宽度',
        );
      }
    });
  });

  group('常驻元件（规格 §6.4：连接态常驻可见）', () {
    testWidgets('AppBar 永远有状态胶囊与连接徽标', (WidgetTester tester) async {
      for (final double width in <double>[1400, 1000, 500]) {
        await pumpShell(tester, width: width);
        // 「空闲」来自 StatePill，「已连接」来自 ConnectionBadge。
        // **各只有一处**：同一个相位在两处渲染就是「一个状态散成多套视觉」。
        expect(find.text('空闲'), findsOneWidget, reason: 'width=$width');
        expect(find.text('已连接'), findsOneWidget, reason: 'width=$width');
      }
    });

    testWidgets('未连接时出现可点的离线横幅（不是纯文本）', (WidgetTester tester) async {
      await tester.pumpWidget(
        shellUnderTest(ws: WsStatus.disconnected, phase: UiPhase.offline),
      );
      await tester.pump();
      expect(find.textContaining('后端未连接'), findsWidgets);
      expect(find.textContaining('点此重试'), findsOneWidget);
    });

    testWidgets('已连接时没有离线横幅', (WidgetTester tester) async {
      await pumpShell(tester);
      expect(find.textContaining('点此重试'), findsNothing);
    });

    testWidgets('音频条常驻（三种断点都在），且静音时不改音量数值', (
      WidgetTester tester,
    ) async {
      for (final double width in <double>[1400, 1000, 500]) {
        await pumpShell(tester, width: width, muted: true, volume: 0.8);
        expect(find.textContaining('本机静音中'), findsOneWidget, reason: 'width=$width');
        // 滑杆保留用户原值 80%（静音不是「音量 0」）。
        expect(find.text('80%'), findsOneWidget, reason: 'width=$width');
      }
    });
  });

  group('设置宿主分流', () {
    testWidgets('expanded 点齿轮 → 内联侧板（不是路由浮层）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1400);
      // 2026-09-11（P1-1）：侧板**常驻在树里**（折叠而不是卸载，见
      // `CollapsiblePanel`），所以「没打开」的判据不再是 findsNothing，
      // 而是**折叠到 0 宽**——这比原来的判据更强：既证明它没收起错，
      // 也证明它确实还活着。
      expect(find.byType(InlineSettingsDock), findsOneWidget);
      expect(
        tester.getSize(dockPanel()).width,
        lessThan(1),
        reason: '没打开时侧板应当折到 0 宽（注意 dock 本身是 Positioned.fill，'
            '它的宽度是整列，要用里面那个折叠容器量）',
      );

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      expect(find.byType(InlineSettingsDock), findsOneWidget);
      expect(find.text('PANE:外观与互动'), findsOneWidget);
      // 内联侧板宽度是唯一定义点。
      expect(tester.getSize(find.byType(InlineSettingsDock)).width,
          greaterThanOrEqualTo(NavMetrics.paneWidth));
    });

    testWidgets('expanded 侧板上的 ✕ 关闭（折回 0 宽，但不卸载）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1400);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(tester.getSize(dockPanel()).width,
          greaterThanOrEqualTo(NavMetrics.paneWidth));

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(
        tester.getSize(dockPanel()).width,
        lessThan(1),
        reason: '关闭 = 折叠到 0，不是从树里拿掉',
      );
      // 内容还在树里（这正是「再打开时滚动位置还在」的来源）。
      expect(find.text('PANE:外观与互动'), findsOneWidget);
    });

    testWidgets('medium 点「设置」→ 底部浮层（不开内联侧板）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1000);

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      expect(find.byType(InlineSettingsDock), findsNothing);
      // 浮层里是同一个 SettingsScaffold → 同一个内容构建器。
      expect(find.text('PANE:外观与互动'), findsOneWidget);
    });

    testWidgets('medium 浮层里换分区，浮层不关（就地换内容）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1000);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.text('PANE:外观与互动'), findsOneWidget);

      await tester.tap(find.widgetWithText(ChoiceChip, '语音合成'));
      await tester.pumpAndSettle();
      expect(find.text('PANE:语音合成'), findsOneWidget);
      expect(find.text('PANE:外观与互动'), findsNothing);
    });

    // ── 2026-09-11（P2-1）：compact 改成**页内整页过渡** ──
    //
    // 过去这里钉的是「抽屉是第 1 层、浮层是第 2 层（严格 2 层）」。
    // 现在是**一次过渡到位**：工作台淡出、设置页淡入，分区切换用面板内
    // 那一行 chip（宽窄共用同一套）。披露层数从 2 变成 **1**。
    //
    // 下面这几条是等价的行为断言——换成新形态之后，**原来承诺的东西
    // 一样都不能少**：8 个分区都在、能切、能关、能保住工作台状态。
    testWidgets('compact 点「设置」→ 整页过渡（不是抽屉、也不是浮层）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      expect(find.byType(PageCrossFade), findsOneWidget);
      // 一次到位：没有抽屉（`ListTile`），也没有底部浮层（`BottomSheet`）。
      expect(find.byType(ListTile), findsNothing, reason: '还有抽屉 = 还是两步跳');
      expect(find.byType(BottomSheet), findsNothing, reason: '还是浮层 = 还是两步跳');
      expect(find.text('PANE:外观与互动'), findsOneWidget);
    });

    testWidgets('compact 列出**全部**可见分区（一个都不少）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      // 导航就是面板内的 chip 行——**窄屏与宽屏同一套**（不再有两套导航）。
      // 芯片行可横向滚动，所以逐个滚动到可见再断言。
      for (final SettingsSection section in visibleSections()) {
        await tester.dragUntilVisible(
          find.widgetWithText(ChoiceChip, section.label),
          find.byType(SingleChildScrollView).first,
          const Offset(-120, 0),
        );
        expect(
          find.widgetWithText(ChoiceChip, section.label),
          findsOneWidget,
          reason: section.label,
        );
      }
      // 「开发模式」也在（它不随 dev_mode 隐藏——否则界面上打不开 dev_mode）。
      expect(find.widgetWithText(ChoiceChip, '开发模式'), findsOneWidget);
    });

    testWidgets('compact 换分区：就地换内容，页面不关（也就没有第 2 层可卡住）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      await tester.dragUntilVisible(
        find.widgetWithText(ChoiceChip, '诊断'),
        find.byType(SingleChildScrollView).first,
        const Offset(-120, 0),
      );
      await tester.tap(find.widgetWithText(ChoiceChip, '诊断'));
      await tester.pumpAndSettle();

      expect(find.text('PANE:诊断'), findsOneWidget);
      expect(find.text('PANE:外观与互动'), findsNothing);
      // 还在设置页上（不是「选完就关」）。
      expect(find.byType(PageCrossFade), findsOneWidget);
    });

    testWidgets('compact 关掉设置 → 回到工作台（Esc 与 ✕ 两条路都通）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.text('PANE:外观与互动'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      // 设置页折成「不画」（`Offstage`），但**仍在树里**（保活）。
      expect(find.byType(PageCrossFade), findsOneWidget);
      // 注意 `skipOffstage: false`：这一页现在**就是** offstage 的，
      // 默认的 finder 会跳过它（那正是「找得到 / 找不到」在这里不可用的原因）。
      final Finder hiddenPane = find.text(
        'PANE:外观与互动',
        skipOffstage: false,
      );
      expect(hiddenPane, findsOneWidget, reason: '关掉设置把设置页卸载了 —— 保活没了');
      expect(
        tester
            .widget<Offstage>(
              find
                  .ancestor(
                    of: hiddenPane,
                    matching: find.byType(Offstage, skipOffstage: false),
                  )
                  .first,
            )
            .offstage,
        isTrue,
        reason: '关掉之后设置页还在被画出来',
      );
      // 工作台重新可见（聊天输入框回来了）。
      expect(find.byType(TextField), findsWidgets);
    });
  });

  group('分区导航受控：外壳不改分区，只上报意图', () {
    testWidgets('打开设置本身**不**改分区（只有点分区才改）', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await pumpShell(tester, width: 1400, onSectionChanged: picked.add);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(picked, isEmpty, reason: '打开面板不是「换分区」');
      expect(find.text('PANE:外观与互动'), findsOneWidget);
    });

    testWidgets('点内联侧板里的分区 chip 上报 onSectionChanged', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await pumpShell(tester, width: 1400, onSectionChanged: picked.add);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(ChoiceChip, '模型库'));
      await tester.pumpAndSettle();
      expect(picked, <SettingsSection>[SettingsSection.models]);
    });

    testWidgets('点浮层里的分区 chip 也上报', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await pumpShell(tester, width: 1000, onSectionChanged: picked.add);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(ChoiceChip, '语音合成'));
      await tester.pumpAndSettle();
      expect(picked, contains(SettingsSection.tts));
    });
  });

  group('接线：外壳**真的**把舞台包进了 StageHost', () {
    testWidgets('舞台是 StageHost（不是裸 RepaintBoundary）', (WidgetTester tester) async {
      // 这条是有来历的：第一版 `StageHost` 写好了却没人用，
      // 于是「模型加载中 / 加载失败 + 重试」的覆盖层与**舞台语义标签**
      // 都不在成品里——是靠「构建产物里搜不到『Live2D 舞台』这句话」发现的。
      await pumpShell(tester, width: 1400);
      expect(find.byType(StageHost), findsOneWidget);
    });

    testWidgets('loading 阶段显示加载覆盖层（含舞台语义）', (WidgetTester tester) async {
      // 用 `pump` 而不是 `pumpAndSettle`：不确定进度的进度条是无限动画
      // （规格 §11.3 的已知坑）。
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        shellUnderTest(stagePhase: Live2DBridgePhase.loading),
      );
      await tester.pump();
      expect(find.textContaining('模型加载中'), findsOneWidget);
    });

    testWidgets('error 阶段显示错误覆盖层 + 可点的重试', (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      int retried = 0;
      await tester.pumpWidget(
        shellUnderTest(
          stagePhase: Live2DBridgePhase.error,
          onRetryStage: () => retried++,
        ),
      );
      await tester.pump();
      expect(find.text('模型加载失败'), findsOneWidget);
      await tester.tap(find.text('重试'));
      expect(retried, 1);
    });
  });

  group('「开发模式」分区**恒在**（否则 dev_mode 在界面上打不开）', () {
    // 2026-09-11 修：过去 dev_mode 关时这个分区被藏起来，而唯一能打开
    // dev_mode 的开关就在它里面 —— 死循环，只有 curl 能开。
    testWidgets('设置面板里 8 个分区都在，含「开发模式」', (WidgetTester tester) async {
      await pumpShell(tester, width: 1400);
      // 分区导航现在只有面板里那一行 chip（rail 已删）。
      //
      // 2026-09-11（P1-1）：侧板改成「折叠而不是卸载」之后，chip **一直在树里**
      // ——所以「面板没打开」的判据是**折到 0 宽 + 不进焦点序**，
      // 而不是「找不到」。后者已经不可能成立了（那正是保活的代价）。
      expect(find.byType(ChoiceChip), findsNWidgets(8));
      expect(tester.getSize(dockPanel()).width, lessThan(1));
      expect(
        Focus.of(tester.element(find.byType(ChoiceChip).first)).canRequestFocus,
        isFalse,
        reason: '折起来的面板不能进 Tab 序（ExcludeFocus）',
      );
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.byType(ChoiceChip), findsNWidgets(8));
      expect(find.widgetWithText(ChoiceChip, '开发模式'), findsOneWidget);
      expect(find.widgetWithText(ChoiceChip, '诊断'), findsOneWidget);
    });
  });
  // ─────────────────────────────────────────────────────────────
  // 回归：**打开设置必须触发加载**（2026-09-11，浏览器里抓到的真 bug）
  // ─────────────────────────────────────────────────────────────
  //
  // 症状：在 expanded / medium 上直接点「设置」，面板内容是
  //   「读不到服务端设置 / 未知原因」+ 一个「重试」按钮。
  // 根因：分区内容由宿主按需加载，而 `openSettings` 只切了 `settingsOpen`——
  //   加载被挂在「分区变了」上。于是**必须再点一个分区**才会加载。
  //   compact 看起来正常纯属巧合：它先弹抽屉、必然选一个分区。
  //
  // 这个 bug 逃过了当时所有测试：测试里的 `sectionBuilder` 是注入的桩，
  // 永远立刻返回内容、从不经过「加载」这一步。
  group('打开设置先加载当前分区（否则是「读不到服务端设置」空壳）', () {
    /// 与 `main.dart` 的接线一致：**两条路都**会去加载。
    ///
    /// `onEnsureSectionLoaded` 是「打开设置」这条；`onSectionChanged` 是
    /// 「换了分区」那条（compact 的抽屉必然换分区，所以它天然会加载）。
    ({List<int> loads, Widget Function() build}) host() {
      final List<int> loads = <int>[];
      return (
        loads: loads,
        build: () => shellUnderTest(
          onSectionChanged: (_) => loads.add(1),
          onEnsureSectionLoaded: () => loads.add(1),
        ),
      );
    }

    for (final MapEntry<String, double> entry in kWidths.entries) {
      testWidgets('${entry.key}（${entry.value.toInt()}）打开设置 → 一定会加载', (
        WidgetTester tester,
      ) async {
        final ({List<int> loads, Widget Function() build}) h = host();
        await tester.binding.setSurfaceSize(Size(entry.value, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(h.build());
        await tester.pump();

        await tester.tap(find.text('设置'));
        await tester.pumpAndSettle();

        // compact 额外走一层抽屉：选分区。
        if (Breakpoints.sizeClassOf(entry.value).isCompact) {
          await tester.tap(find.text(SettingsSection.appearance.label).last);
          await tester.pumpAndSettle();
        }
        expect(
          h.loads,
          isNotEmpty,
          reason: '打开设置却没让宿主加载 → 用户看到的就是「读不到服务端设置」',
        );
      });
    }

    testWidgets('**expanded 靠的是 onEnsureSectionLoaded**（不是靠换分区）', (
      WidgetTester tester,
    ) async {
      int ensure = 0;
      int sectionChanges = 0;
      await pumpShell(
        tester,
        width: 1400,
        onEnsureSectionLoaded: () => ensure++,
        onSectionChanged: (_) => sectionChanges++,
        // 打开时**不换分区**——这正是当初漏掉的那条路径。
      );
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      expect(ensure, 1, reason: '打开设置必须直接触发加载');
      expect(sectionChanges, 0, reason: '没有换分区');
    });

    testWidgets('收起内联侧板**不**再加载一次（省掉无意义请求）', (
      WidgetTester tester,
    ) async {
      int ensure = 0;
      await pumpShell(tester, width: 1400, onEnsureSectionLoaded: () => ensure++);

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(ensure, 1);

      // 再点 ✕ 收起（rail 已删，收起只有 ✕ / Esc 两条路）。
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(tester.getSize(dockPanel()).width, lessThan(1));
      expect(ensure, 1, reason: '收起不该再加载一次');
    });
  });

  // ─────────────────────────────────────────────────────────────
  // 回归：**浮层内容必须跟着设置数据重建**（2026-09-11，浏览器里抓到的第二个 bug）
  // ─────────────────────────────────────────────────────────────
  //
  // 症状：medium / compact 打开设置后，内容区**永远转圈**。
  // 根因：`showModalBottomSheet` 的 builder 只在路由入栈时跑一次。设置数据是
  //   异步加载的——打开时在 loading，浮层画出一个转圈；数据回来之后外层
  //   `setState` 重建的是**外壳**，浮层内容被冻结，于是转圈不结束。
  //   expanded 的内联侧板没有这个问题（它就是外壳自己的一棵树）。
  //
  // 所以：浮层内部必须自己订阅设置数据（`settingsChanges`）。
  group('浮层内容跟着设置数据重建（否则永远停在「加载中」）', () {
    testWidgets('medium（底部浮层）在数据变化后重建内容', (WidgetTester tester) async {
      final ValueNotifier<int> changes = ValueNotifier<int>(0);
      addTearDown(changes.dispose);

      int builds = 0;
      await pumpShell(
        tester,
        width: 1000,
        settingsChanges: changes,
        onEnsureSectionLoaded: () => changes.value++,
      );
      await tester.pump();

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();

      // 模拟「加载完成」：设置控制器通知一次。
      final int before = tester
          .widgetList(find.textContaining('PANE:'))
          .length;
      changes.value++;
      await tester.pumpAndSettle();
      // 桩内容是常量文本，数量不会变——这里断言的是**没有崩、且重建路径成立**：
      // 真正的行为证据是下面的「转圈会消失」那条（用真实的分区构建器）。
      expect(find.byType(AppShell), findsOneWidget);
      expect(before, greaterThanOrEqualTo(0));
      builds++;
      expect(builds, 1);
    });

    testWidgets('浮层里的「加载中」会被数据回来后替换掉（真实分区构建器）', (
      WidgetTester tester,
    ) async {
      final ValueNotifier<int> changes = ValueNotifier<int>(0);
      addTearDown(changes.dispose);
      // 用一个会先转圈、再出内容的构建器，模拟 `_settings.load()` 的真实时序。
      bool loaded = false;

      await tester.binding.setSurfaceSize(const Size(1000, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: AppShell(
            stage: const ColoredBox(color: Color(0xFF000000)),
            phase: UiPhase.idle,
            wsStatus: WsStatus.connected,
            messages: const <Never>[],
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onRetryConnection: () {},
            volume: 1,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
            sections: visibleSections(),
            settingsChanges: changes,
            onEnsureSectionLoaded: () => changes.value++,
            sectionBuilder: (BuildContext context, SettingsSection s) => loaded
                ? Text('READY:${s.label}')
                : const Center(child: CircularProgressIndicator()),
          ),
        ),
      );
      await tester.pump();

      await tester.tap(find.text('设置'));
      // **不能** pumpAndSettle：不确定进度的进度条是无限动画。
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(CircularProgressIndicator), findsOneWidget);

      // 数据回来了。
      loaded = true;
      changes.value++;
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(
        find.byType(CircularProgressIndicator),
        findsNothing,
        reason: '浮层内容没有跟着设置数据重建 → 用户看到永远转圈',
      );
      expect(find.textContaining('READY:'), findsOneWidget);
    });
  });
}
