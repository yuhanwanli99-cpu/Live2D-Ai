/// 外壳里的**左侧会话栏 + 聊天列头标题**（2026-11-18）。
///
/// 审计现场：宽屏（≥900）只有顶栏一个「会话」按钮，聊天列头写成
/// `onOpenSessions: compact ? … : null` 所以非 compact 连头都不渲染；设置侧板一
/// 收起，聊天列里既没有标题也没有条数——看不出在哪一段对话里。
///
/// 这个文件钉三件事：
/// 1. expanded 展开 248 px 列表 / medium 只留 52 px 窄轨，**两档都留当前会话名**；
/// 2. 设置侧板展开时标题仍然可见（侧板只盖舞台右缘）；
/// 3. 顶栏不再有「会话」入口（少一个按钮，900–1000 档不再挤出黄黑条）。
///
/// 从 `app_shell_layout_test.dart` 拆出来：那个文件已贴近 800 行的测试棘轮。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/session_rail.dart';
import 'package:live2d_ai_shell/ui/stage_host.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

ChatSession demoSession(String id, String title, {int minute = 0}) => ChatSession(
  id: id,
  title: title,
  createdAt: DateTime(2026, 11, 18, 10, minute),
  updatedAt: DateTime(2026, 11, 18, 10, minute),
);

Widget shellAt({
  required List<ChatSession> sessions,
  String? activeId,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: AppShell(
    prefs: const DisplayPrefs(),
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    messages: const <Never>[],
    input: TextEditingController(),
    onSend: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(devMode: true),
    section: SettingsSection.theme,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
    stagePhase: Live2DBridgePhase.ready,
    sessions: sessions,
    activeSessionId: activeId,
    onNewSession: () {},
    onSelectSession: (_) {},
    onRenameSession: (_, _) {},
    onDeleteSession: (_) {},
  ),
);

Future<void> pumpShellAt(
  WidgetTester tester,
  double width, {
  List<ChatSession>? sessions,
  String? activeId,
}) async {
  await tester.binding.setSurfaceSize(Size(width, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    shellAt(
      sessions: sessions ??
          <ChatSession>[
            demoSession('a', '甲会话', minute: 0),
            demoSession('b', '乙会话', minute: 1),
          ],
      activeId: activeId ?? 'b',
    ),
  );
  await tester.pump();
}

void main() {
  group('会话栏常驻：收起设置后仍看得出在哪段对话', () {
    testWidgets('expanded：会话栏展开 + 聊天头标题 + 顶栏不再有「会话」', (
      WidgetTester tester,
    ) async {
      await pumpShellAt(tester, 1400);

      expect(find.byType(SessionRail), findsOneWidget);
      expect(
        tester.getSize(find.byType(SessionRail)).width,
        kSessionRailExpandedWidth,
      );
      expect(
        find.text('乙会话'),
        findsNWidgets(2),
        reason: '会话栏一行 + 聊天列头标题',
      );
      expect(
        find.descendant(of: find.byType(AppBar), matching: find.text('会话')),
        findsNothing,
        reason: '顶栏那一排会把「会话」挤出黄黑条，所以入口搬到会话栏 / 列头',
      );
      expect(find.text('会话'), findsOneWidget, reason: '聊天列头的入口按钮');
    });

    testWidgets('medium：只有 52 px 窄轨，但当前会话名字还在', (
      WidgetTester tester,
    ) async {
      await pumpShellAt(tester, 1000);

      expect(find.byType(SessionRail), findsOneWidget);
      expect(
        tester.getSize(find.byType(SessionRail)).width,
        kSessionRailCollapsedWidth,
      );
      expect(
        find.text('乙会话'),
        findsNWidgets(2),
        reason: '窄轨竖排标题 + 聊天列头标题',
      );
      expect(
        find.descendant(of: find.byType(AppBar), matching: find.text('会话')),
        findsNothing,
      );
    });

    testWidgets('展开设置侧板后，当前会话标题仍然可见（审计 #1）', (
      WidgetTester tester,
    ) async {
      await pumpShellAt(tester, 1400);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(
        find.text('乙会话'),
        findsNWidgets(2),
        reason: '设置侧板只盖舞台右缘；它收起 / 展开都不该带走「我在哪段对话」',
      );
    });

    testWidgets('「收起」→ 窄轨仍留当前名字；舞台拿回宽度', (
      WidgetTester tester,
    ) async {
      await pumpShellAt(tester, 1400);
      final double before = tester.getSize(find.byType(StageHost)).width;
      await tester.tap(find.text('收起'));
      await tester.pumpAndSettle();
      expect(
        tester.getSize(find.byType(SessionRail)).width,
        kSessionRailCollapsedWidth,
      );
      expect(find.text('乙会话'), findsNWidgets(2));
      expect(tester.getSize(find.byType(StageHost)).width, greaterThan(before));
    });


    testWidgets('舞台列宽度 = 总宽 - 会话栏 - 聊天列 - 两条分隔线', (
      WidgetTester tester,
    ) async {
      // 2026-11-18：除聊天列之外还有一列会话栏（展开 248 / 窄轨 52）。
      // 宽度只从 sessionRailWidthFor 这个纯函数读，避免两处各写一份。
      for (final double width in <double>[1280, 1400, 1000]) {
        await pumpShellAt(tester, width);
        final SizeClass sc = Breakpoints.sizeClassOf(width);
        final double chat = sc.isExpanded
            ? NavMetrics.chatWidthExpanded
            : NavMetrics.chatWidthMedium;
        final double rail = sessionRailWidthFor(sc, expanded: sc.isExpanded);
        expect(
          tester.getSize(find.byType(StageHost)).width,
          closeTo(width - rail - chat - 2, 1.5),
          reason: 'width=\$width：舞台列 = 总宽 - 会话栏 - 聊天列 - 两条 1 px 分隔线',
        );
      }
    });
    testWidgets('各宽度都不溢出（顶栏少一个按钮之后也不该有黄黑条）', (
      WidgetTester tester,
    ) async {
      for (final double width in <double>[390, 899, 900, 1000, 1279, 1280, 1400]) {
        await pumpShellAt(tester, width);
        expect(tester.takeException(), isNull, reason: 'width=$width');
      }
    });
  });
}
