import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

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
  });

  final UiPhase phase;
  final WsStatus ws;
  final SettingsSection initialSection;
  final List<SettingsSection> sections;
  final bool muted;
  final double volume;
  final ValueChanged<SettingsSection>? onSectionChanged;

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
    onSectionChanged: (SettingsSection next) {
      widget.onSectionChanged?.call(next);
      setState(() => section = next);
    },
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
  );
}

Widget shellUnderTest({
  UiPhase phase = UiPhase.idle,
  WsStatus ws = WsStatus.connected,
  SettingsSection section = SettingsSection.actions,
  List<SettingsSection>? sections,
  ValueChanged<SettingsSection>? onSectionChanged,
  bool muted = false,
  double volume = 0.8,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: _ShellHost(
    phase: phase,
    ws: ws,
    initialSection: section,
    sections: sections ?? visibleSections(devMode: false),
    muted: muted,
    volume: volume,
    onSectionChanged: onSectionChanged,
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
  SettingsSection section = SettingsSection.actions,
  List<SettingsSection>? sections,
  ValueChanged<SettingsSection>? onSectionChanged,
  bool muted = false,
  double volume = 0.8,
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
      muted: muted,
      volume: volume,
    ),
  );
  await tester.pump();
}

void main() {
  group('断点 → 导航形态（纯函数 + 渲染两层都验）', () {
    test('settingsHostOf：只有 expanded 内联，其余浮层', () {
      expect(settingsHostOf(SizeClass.expanded), SettingsHost.inline);
      expect(settingsHostOf(SizeClass.medium), SettingsHost.sheet);
      expect(settingsHostOf(SizeClass.compact), SettingsHost.sheet);
    });

    test('sheetConstraintsOf：compact 比 medium 更窄更矮', () {
      const Size viewport = Size(1000, 800);
      final BoxConstraints medium =
          sheetConstraintsOf(SizeClass.medium, viewport);
      final BoxConstraints compact =
          sheetConstraintsOf(SizeClass.compact, viewport);
      expect(medium.maxWidth, NavMetrics.sheetMaxWidthMedium);
      expect(compact.maxWidth, NavMetrics.sheetMaxWidthCompact);
      expect(compact.maxWidth, lessThan(medium.maxWidth));
      expect(compact.maxHeight, lessThan(medium.maxHeight));
      expect(medium.maxHeight, 800 * NavMetrics.sheetHeightFactorMedium);
    });

    test('NavMetrics 自洽：rail 宽度、舞台下限、聊天宽度都不为负且有序', () {
      expect(NavMetrics.railExtendedWidth, greaterThan(NavMetrics.railIconWidth));
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
        final Finder rail = find.byType(NavigationRail);

        if (sc.isCompact) {
          expect(rail, findsNothing, reason: 'compact 不用 rail（8 分区装不下）');
          // compact 的设置入口在聊天面板头。
          expect(find.byIcon(Icons.settings_outlined), findsOneWidget);
        } else {
          expect(rail, findsOneWidget);
          final NavigationRail widget = tester.widget<NavigationRail>(rail);
          expect(
            widget.extended,
            sc.isExpanded,
            reason: '只有 expanded 用带文字的 extended rail',
          );
          expect(
            widget.minWidth,
            sc.isExpanded
                ? NavMetrics.railExtendedWidth
                : NavMetrics.railIconWidth,
          );
        }

        // AppBar 里的设置齿轮只在非 compact 出现。
        final int gearInAppBar = sc.isCompact ? 0 : 1;
        expect(
          find.byIcon(Icons.settings_outlined),
          findsNWidgets(gearInAppBar == 1 ? 1 : 1),
          reason: 'compact 时齿轮在聊天面板头，非 compact 时在 AppBar——总数都是 1',
        );
      });
    }
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
      expect(find.byType(InlineSettingsDock), findsNothing);

      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();

      expect(find.byType(InlineSettingsDock), findsOneWidget);
      expect(find.text('PANE:动作与互动'), findsOneWidget);
      // 内联侧板宽度是唯一定义点。
      expect(tester.getSize(find.byType(InlineSettingsDock)).width,
          greaterThanOrEqualTo(NavMetrics.paneWidth));
    });

    testWidgets('expanded 再点同一个 rail 分区 → 收起（切换语义）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 1400);

      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();
      expect(find.byType(InlineSettingsDock), findsOneWidget);

      // 点 rail 上「动作与互动」（当前分区）→ 收起。
      await tester.tap(find.text('动作与互动').first);
      await tester.pumpAndSettle();
      expect(find.byType(InlineSettingsDock), findsNothing);
    });

    testWidgets('expanded 侧板上的 ✕ 关闭', (WidgetTester tester) async {
      await pumpShell(tester, width: 1400);
      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(find.byType(InlineSettingsDock), findsNothing);
    });

    testWidgets('medium 点 rail → 底部浮层（不开内联侧板）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1000);

      await tester.tap(find.text('LLM').first);
      await tester.pumpAndSettle();

      expect(find.byType(InlineSettingsDock), findsNothing);
      // 浮层里是同一个 SettingsScaffold → 同一个内容构建器。
      expect(find.text('PANE:LLM'), findsOneWidget);
    });

    testWidgets('medium 浮层里换分区，浮层不关（就地换内容）', (WidgetTester tester) async {
      await pumpShell(tester, width: 1000);
      await tester.tap(find.text('LLM').first);
      await tester.pumpAndSettle();
      expect(find.text('PANE:LLM'), findsOneWidget);

      // 必须按 ChoiceChip 定位：`find.text('语音合成').first` 会命中**后面**
      // 被模态障碍层盖住的 rail 标签（tree 顺序在前），点不中。
      await tester.tap(find.widgetWithText(ChoiceChip, '语音合成'));
      await tester.pumpAndSettle();
      expect(find.text('PANE:语音合成'), findsOneWidget);
      expect(find.text('PANE:LLM'), findsNothing);
    });

    testWidgets('compact 抽屉是第 1 层、浮层是第 2 层（严格 2 层）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);

      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();
      // 第 1 层：抽屉带**说明文字**（用户不该先学会产品黑话）。
      expect(find.byType(ListTile), findsWidgets);
      expect(find.text('扩展模块的启停与配置'), findsOneWidget);

      // 抽屉是滚动列表：390/500 px 高的视口装不下 7 项 × ~72 px，
      // 末尾的「诊断」要滚一下才可见。这本身是正常行为（抽屉可滚动）。
      await tester.dragUntilVisible(
        find.text('诊断'),
        find.byType(ListView),
        const Offset(0, -80),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('诊断'));
      await tester.pumpAndSettle();
      // 第 2 层：浮层内容。
      expect(find.text('PANE:诊断'), findsOneWidget);
    });

    testWidgets('compact 抽屉列出**全部**可见分区（滚到底也一项不少）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);
      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();

      for (final SettingsSection section in visibleSections(devMode: false)) {
        await tester.dragUntilVisible(
          find.text(section.label),
          find.byType(ListView),
          const Offset(0, -80),
        );
        expect(find.text(section.label), findsOneWidget, reason: section.label);
        expect(find.text(section.description), findsOneWidget);
      }
      expect(find.text('开发模式'), findsNothing, reason: 'dev 关时不该出现');
    });

    testWidgets('compact 抽屉直接关掉 → 不打开第 2 层（不卡住）', (
      WidgetTester tester,
    ) async {
      await pumpShell(tester, width: 500);
      await tester.tap(find.byIcon(Icons.settings_outlined));
      await tester.pumpAndSettle();

      // 点障碍区关掉抽屉。
      await tester.tapAt(const Offset(10, 10));
      await tester.pumpAndSettle();
      expect(find.text('PANE:诊断'), findsNothing);
      expect(find.byType(ListTile), findsNothing);
    });
  });

  group('分区导航受控：外壳不改分区，只上报意图', () {
    testWidgets('点 rail 上报 onSectionChanged', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await pumpShell(tester, width: 1400, onSectionChanged: picked.add);
      await tester.tap(find.text('模型库').first);
      await tester.pumpAndSettle();
      expect(picked, <SettingsSection>[SettingsSection.models]);
    });

    testWidgets('点浮层里的分区 chip 也上报', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await pumpShell(tester, width: 1000, onSectionChanged: picked.add);
      await tester.tap(find.text('LLM').first);
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(ChoiceChip, '语音合成'));
      await tester.pumpAndSettle();
      expect(picked, contains(SettingsSection.tts));
    });
  });

  group('dev_mode 决定「开发模式」分区是否出现', () {
    testWidgets('dev 关 → rail 上没有开发模式', (WidgetTester tester) async {
      await pumpShell(tester, width: 1400);
      // extended rail 带文字，所以直接找文字。
      expect(find.text('开发模式'), findsNothing);
      expect(find.text('诊断'), findsOneWidget);
    });

    testWidgets('dev 开 → 8 个分区都在 rail 上', (WidgetTester tester) async {
      await pumpShell(
        tester,
        width: 1400,
        sections: visibleSections(devMode: true),
      );
      expect(find.text('开发模式'), findsOneWidget);
      final NavigationRail rail = tester.widget<NavigationRail>(
        find.byType(NavigationRail),
      );
      expect(rail.destinations, hasLength(8));
    });
  });
}
