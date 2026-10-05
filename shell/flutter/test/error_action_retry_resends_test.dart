/// 「重试」= **重发上一条用户消息**（2026-10-06 裁决，R4-T2/T2-B1）。
///
/// # 修掉的是什么
///
/// 无 `code` 的兜底「重试」过去接组合根的 `_send()`——那条路读**输入框**。
/// 一次失败之后输入框早已被 `send()` 清空（发送是「先上屏、再 POST」），
/// 于是按钮**看得见、按下去毫无反应**：静默 no-op。用户会以为链路又坏了，
/// 而界面连一句解释都没有——比没有按钮更坏。
///
/// 裁决：重试读**会话记录**里最后一条用户消息，与 busy 的「打断并重发」
/// 第二步共用同一条真源（`ChatController.resendLastUserMessage`）。
/// 没有上一条可重发时**按钮不出现**（不摆假出路）。
///
/// # 为什么分「真泵 + 结构守卫」两层
///
/// `main.dart` 是 `package:web` + `part` 组合根，VM 里 import 不了，
/// 所以生产接线只能用源码结构守卫钉；而结构守卫单独存在会退化成
/// 「源码里有这行字」（见 `error_action_busy_resend_test.dart` 头注）。
/// 所以行为级那一层用**与生产同形**的宿主：两条路各自记一笔，
/// 断言被调到的是**读会话记录**那条。
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/ui/error_actions.dart';
import 'package:live2d_ai_shell/ui/error_banner.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

/// 与生产接线**同形**的宿主：`input` = 输入框当前内容（失败后为空串），
/// `lastUser` = 会话记录里最后一条用户消息。
///
/// 两条路分别记下自己发出去了什么——旧接线（读输入框）在这里会记下**空**，
/// 而正确接线记下的是 `last:` 打头的那条。
class _RetryHost extends StatefulWidget {
  const _RetryHost({required this.input, required this.lastUser, super.key});

  final String input;
  final String lastUser;

  @override
  State<_RetryHost> createState() => _RetryHostState();
}

class _RetryHostState extends State<_RetryHost> {
  /// 真正发出去的内容（两条路共用的记账表）。
  final List<String> sent = <String>[];

  /// 生产 `_resendLastUserMessage()`：读**会话记录**最后一条用户消息。
  ///
  /// 对照：旧接线读的是**输入框**（`widget.input`）——在本宿主里它只作为
  /// 「现场条件」存在，读输入框那条路在 `_LegacyRetryHost` 里演示（见下）。
  Future<void> _resendLast() async {
    final String text = widget.lastUser.trim();
    if (text.isEmpty) return;
    sent.add('last:$text');
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: ErrorBanner(
        // 无码、文案里也没有 busy / 未就绪 ⇒ 走兜底那条分支。
        message: '网络错误',
        actions: errorActionsFor(
          '网络错误',
          onGoto: (SettingsSection _) {},
          onInterruptAndResend: () {},
          onResendLast: widget.lastUser.trim().isEmpty
              ? null
              : () => unawaited(_resendLast()),
        ),
      ),
    ),
  );
}

/// 旧形状的对照宿主：兜底「重试」接**读输入框**那条（就是被裁决换掉的接线）。
///
/// 它只用来证明「判别力」：同一个现场（输入框空、有上一条）下它发不出去，
/// 所以上面那条断言不是空转。
class _LegacyRetryHost extends StatefulWidget {
  const _LegacyRetryHost({required this.input, required this.lastUser, super.key});

  final String input;
  final String lastUser;

  @override
  State<_LegacyRetryHost> createState() => _LegacyRetryHostState();
}

class _LegacyRetryHostState extends State<_LegacyRetryHost> {
  final List<String> sent = <String>[];

  Future<void> _sendFromInput() async {
    final String text = widget.input.trim();
    if (text.isEmpty) return;
    sent.add('input:$text');
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: ErrorBanner(
        message: '网络错误',
        actions: <ErrorAction>[
          ErrorAction(label: '重试', onPressed: () => unawaited(_sendFromInput())),
        ],
      ),
    ),
  );
}

void main() {
  group('真泵：无 code 的「重试」重发上一条用户消息', () {
    testWidgets('输入框已空 + 有上一条 → 真的重发那一条', (WidgetTester tester) async {
      final GlobalKey<_RetryHostState> key = GlobalKey<_RetryHostState>();
      await tester.pumpWidget(
        _RetryHost(key: key, input: '', lastUser: '你好呀，帮我看看这个'),
      );

      expect(find.text('重试'), findsOneWidget, reason: '失败必须有出路');
      await tester.tap(find.text('重试'));
      await tester.pumpAndSettle();

      expect(
        key.currentState!.sent,
        <String>['last:你好呀，帮我看看这个'],
        reason: '裁决：重试 = 重发上一条用户消息（不读输入框）',
      );
    });

    testWidgets('旧形状对照：读输入框那条在这个现场发不出任何东西（断言有判别力）', (
      WidgetTester tester,
    ) async {
      final GlobalKey<_LegacyRetryHostState> key =
          GlobalKey<_LegacyRetryHostState>();
      await tester.pumpWidget(
        _LegacyRetryHost(key: key, input: '', lastUser: '你好呀，帮我看看这个'),
      );
      await tester.tap(find.text('重试'));
      await tester.pumpAndSettle();
      expect(
        key.currentState!.sent,
        isEmpty,
        reason: '这就是被换掉的接线：输入框空 ⇒ 静默 no-op',
      );
    });

    testWidgets('没有上一条可重发 → 按钮不出现（不摆假出路）', (WidgetTester tester) async {
      await tester.pumpWidget(const _RetryHost(input: '', lastUser: ''));
      expect(
        find.text('重试'),
        findsNothing,
        reason: '没有可重发的对象时，按钮要么不出现、要么给明确提示；这里选不出现',
      );
    });

    testWidgets('输入框里有字也只是显示，不影响「重发上一条」的判据', (WidgetTester tester) async {
      final GlobalKey<_RetryHostState> key = GlobalKey<_RetryHostState>();
      await tester.pumpWidget(
        _RetryHost(key: key, input: '用户刚打的草稿', lastUser: '上一句'),
      );
      await tester.tap(find.text('重试'));
      await tester.pumpAndSettle();
      expect(
        key.currentState!.sent,
        <String>['last:上一句'],
        reason: '草稿归草稿：重试的语义是「把上一条再发一次」',
      );
    });
  });

  group('结构守卫：把行为级那一半钉回生产接线（main.dart）', () {
    String mainArgs() {
      final String src = stripCommentsAndStrings(
        File('lib/main.dart').readAsStringSync(),
      );
      return balancedFrom(src, 'errorActionsFor(', '(', ')');
    }

    test('main.dart：兜底重试接的是 resend，不是读输入框那条', () {
      final String args = mainArgs();
      expect(
        args,
        contains('onResendLast:'),
        reason: '签名换成了 onResendLast（旧名字 onSend 直接编译不过）',
      );
      expect(
        args,
        contains('_resendLastUserMessage'),
        reason: '必须接 ChatController.resendLastUserMessage 那条真源',
      );
      expect(
        args,
        isNot(contains('_sendWithCancellation')),
        reason: '旧形状：兜底重试接 _sendWithCancellation（读输入框，空即 no-op）',
      );
    });

    test('main.dart：没有上一条可重发时传 null（按钮不出现）', () {
      final String args = mainArgs();
      expect(
        args,
        contains('lastUserMessageText'),
        reason: '用会话记录判「有没有上一条」，而不是等按下去才知道',
      );
    });
  });
}
