/// 「重试」的语义收尾（2026-10-06 裁决，R4-T5）。
///
/// # 病是什么
///
/// 组合根里 `onRetryLast` 曾经接 `_sendWithCancellation()`——**读输入框**的那条路。
/// 一次失败之后输入框早已被 `send()` 清空（发送是「先上屏、再 POST」），于是两条
/// 消费路径都是**静默 no-op**：
/// - 顶栏状态胶囊（`StatePill.onTap`）：出错时可点，点下去什么都没发生；
/// - 失败气泡的「重试」按钮：看得见，点了没反应。
///
/// 本文件把新契约钉成**行为**（不是源码字面量）：
/// 1. 没有上一条可重发 ⇒ 胶囊**不可点**、失败气泡**不出按钮**；
/// 2. 有上一条 ⇒ 点下去**真的调到**那条重发回调（组合根接的是
///    `ChatController.resendLastUserMessage` 那条真源）；
/// 3. 仓里**不许**再有第三条「读输入框的重试」（`_sendWithCancellation` 只许接在
///    `onSend:` 上）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/error_banner.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/state_pill.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// 只装顶栏那颗胶囊的最小 AppShell 宿主。
///
/// `phase: error` + `error != null` 就是「出错」现场；`onRetryLast` 由测试给，
/// 生产里它是**可空**的（没有上一条可重发时 = null）。
class _PillHost extends StatelessWidget {
  const _PillHost({required this.error, this.onRetryLast});

  final String? error;
  final VoidCallback? onRetryLast;

  @override
  Widget build(BuildContext context) => AppShell(
    prefs: const DisplayPrefs(),
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: UiPhase.error,
    wsStatus: WsStatus.connected,
    // **必须显式 `ready`**：`loading` 会渲染不确定进度的进度条（无限动画）⇒
    // 任何 `pumpAndSettle` 都会超时（`app_shell_layout_test.dart` 记过这个坑）。
    stagePhase: Live2DBridgePhase.ready,
    messages: const <Never>[],
    input: TextEditingController(),
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: SettingsSection.theme,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        const SizedBox.shrink(),
    error: error,
    errorActions: const <ErrorAction>[],
    onRetryLast: onRetryLast,
  );
}

Future<void> _pumpPill(
  WidgetTester tester, {
  String? error,
  VoidCallback? onRetryLast,
}) async {
  await tester.binding.setSurfaceSize(const Size(1000, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      builder: (BuildContext context, Widget? child) => MediaQuery(
        // `setSurfaceSize` 只改根约束：不对齐 `MediaQuery.size`，断点会算出
        // 两个不同的值（`error_action_opens_settings_test.dart` 头注记过）。
        data: MediaQuery.of(context).copyWith(size: const Size(1000, 800)),
        child: child!,
      ),
      home: _PillHost(error: error, onRetryLast: onRetryLast),
    ),
  );
  await tester.pump();
}

ChatMessage _failedBubble() =>
    ChatMessage(role: ChatRole.assistant, text: '（生成失败）', failed: true);

Future<void> _pumpBubble(WidgetTester tester, {VoidCallback? onRetry}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: MessageBubble(message: _failedBubble(), onRetry: onRetry),
      ),
    ),
  );
  await tester.pump();
}

/// `anchor` 起到这条实参结束（配平后的逗号）为止。
///
/// 判「这条接线接的是哪条路」，不比对整段字面量。`dart format` 会把
/// 三元表达式折行，所以不能只看锚点那一行。
String _argLine(String src, String anchor) {
  final int at = src.indexOf(anchor);
  expect(at, greaterThanOrEqualTo(0), reason: '找不到 `$anchor` 接线');
  var depth = 0;
  for (var i = at; i < src.length; i++) {
    final String c = src[i];
    if (c == '(' || c == '[' || c == '{') {
      depth++;
    } else if (c == ')' || c == ']' || c == '}') {
      if (depth == 0) break;
      depth--;
    } else if (c == ',' && depth == 0) {
      return src.substring(at, i);
    }
  }
  final int end = src.indexOf('\n', at);
  return src.substring(at, end < 0 ? src.length : end);
}

void main() {
  group('顶栏状态胶囊：不可重发 ⇒ 不可点（不是「点了没反应」）', () {
    testWidgets('有 error 但没有上一条可重发 → 胶囊不可点', (WidgetTester tester) async {
      await _pumpPill(tester, error: '网络错误', onRetryLast: null);
      final StatePill pill = tester.widget<StatePill>(find.byType(StatePill));
      expect(pill.onTap, isNull, reason: '组合根没有可重发对象时给的就是 null ⇒ 胶囊这一层必须不可点');
      expect(
        find.descendant(
          of: find.byType(StatePill),
          matching: find.byType(InkWell),
        ),
        findsNothing,
        reason: 'onTap 为 null 时 StatePill 不套 InkWell —— 这就是「不可点」的实现面',
      );
    });

    testWidgets('有 error 且有上一条 → 点胶囊真的调到那条重发回调', (WidgetTester tester) async {
      int taps = 0;
      await _pumpPill(tester, error: '网络错误', onRetryLast: () => taps++);
      final StatePill pill = tester.widget<StatePill>(find.byType(StatePill));
      expect(pill.onTap, isNotNull);
      await tester.tap(find.byType(StatePill));
      await tester.pump();
      expect(taps, 1, reason: '点了必须有一件真事（旧形态是静默 no-op）');
    });

    testWidgets('没有 error → 胶囊不可点（哪怕有可重发的上一条）', (WidgetTester tester) async {
      await _pumpPill(tester, error: null, onRetryLast: () {});
      final StatePill pill = tester.widget<StatePill>(find.byType(StatePill));
      expect(pill.onTap, isNull, reason: '没出错时点状态胶囊不属于任何出路');
    });
  });

  group('失败气泡：「重试」只在真的有事可做时才出现', () {
    testWidgets('onRetry == null → 按钮不出现', (WidgetTester tester) async {
      await _pumpBubble(tester, onRetry: null);
      expect(find.text('重试'), findsNothing, reason: '没有上一条可重发 ⇒ 不摆一颗按下去没反应的按钮');
    });

    testWidgets('onRetry 非空 → 按钮出现且点了真的调到', (WidgetTester tester) async {
      int taps = 0;
      await _pumpBubble(tester, onRetry: () => taps++);
      expect(find.text('重试'), findsOneWidget);
      await tester.tap(find.text('重试'));
      await tester.pump();
      expect(taps, 1);
    });
  });

  group('接线（语义级，不钉字面量形态）', () {
    test('组合根：onRetryLast 接「重发上一条」，判据是 lastUserMessageText，不可重发传 null', () {
      final String code = stripCommentsAndStrings(
        readLibrarySource('lib/main.dart'),
      );
      final String line = _argLine(code, 'onRetryLast:');
      expect(line, contains('lastUserMessageText'), reason: '判据 = 有没有上一条');
      expect(
        line,
        contains('_resendLastUserMessage'),
        reason: '重试 = 重发上一条用户消息',
      );
      expect(line, contains('? null'), reason: '不可重发 ⇒ 传 null（按钮/点按都不出现）');
      expect(
        line,
        isNot(contains('_sendWithCancellation')),
        reason: '旧形态读输入框 ⇒ 失败后静默 no-op',
      );
    });

    test('全仓：读输入框那条路只许接在「发送」上（不许第三条「重试」）', () {
      final String code = stripCommentsAndStrings(
        readLibrarySource('lib/main.dart'),
      );
      final List<String> offenders = <String>[
        for (final String line in code.split('\n'))
          if (line.contains('_sendWithCancellation') &&
              !line.contains('onSend:') &&
              !line.contains('Future<void> _sendWithCancellation'))
            line.trim(),
      ];
      expect(
        offenders,
        isEmpty,
        reason:
            '`_sendWithCancellation` = 读输入框的「新消息」路，只属于发送键；'
            '任何「重试」接它都会在失败后变成静默 no-op。命中：$offenders',
      );
    });

    test('chat_panel：只有失败占位气泡拿到 onRetry，且回调可空', () {
      final String panel = stripCommentsAndStrings(
        readLibrarySource('lib/ui/chat_panel.dart'),
      );
      final String line = _argLine(panel, 'onRetry:');
      expect(line, contains('isPlaceholder'), reason: '只有失败占位气泡有「重试」');
      expect(line, contains('onRetryLast'), reason: '接的是组合根那条重发回调');
      expect(line, contains('null'), reason: '不可重发时传 null ⇒ 按钮不出现');
    });
  });
}
