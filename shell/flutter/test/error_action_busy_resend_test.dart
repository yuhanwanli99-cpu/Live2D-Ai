/// F-0007-2（P2，审计 2026-09-28）：错误横幅的「打断并重发」**只 stop 不 send**。
///
/// # 缺陷的两个层次（都要钉）
///
/// 1. **动作接错**：`errorActionsFor(code:'busy')` 的动作是 `onPressed: onStop`
///    ——按钮承诺了做不到的事（`onSend` 参数当时**已经传进来**却没人用）。
/// 2. **就算接上「发」也发不出去**：busy 时 `_send` 早已 `_input.clear()`，
///    而 `_send()` 读输入框、空文本直接 return ⇒ 「重发」必须从**会话记录**
///    取回正文（[lastUserText]）。
///
/// # 为什么分「真泵 + 纯编排 + 结构守卫」三层
///
/// `main.dart` 是 `package:web` + `part` 组合根，VM 里 import 不了，所以生产
/// 接线只能用结构守卫钉；而结构守卫单独存在会退化成「源码里有这行字」
/// （`error_action_opens_settings_test.dart` 头注记过同一条教训）。所以：
///
/// - **行为级**：真 `ErrorBanner` + 真 `errorActionsFor` + 与生产**同形**的宿主
///   （pendingText / rounds / order）——点一下，断言**真的发出了一轮**；
/// - **纯编排**：`interruptAndResend` 的「两步都做 + stop 先完成」用 Completer 钉；
/// - **纯判据**：`lastUserText` 取哪一条（busy 现场最后一条是 assistant 失败气泡）；
/// - **结构守卫**：把上面这些钉回**生产接线**（main.dart / shell_chat.dart /
///   chat_controller.dart）。
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/ui/error_actions.dart';
import 'package:live2d_ai_shell/ui/error_banner.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

/// 与 `main.dart` / `shell_chat.dart` 的 busy 接线**同形**的宿主。
///
/// - `pendingText` = 那条被 429 挡下的用户消息（生产里 = 会话记录里最后一条
///   用户消息，见 [lastUserText]）；
/// - `rounds` = 真正**发出去**的新一轮（生产里 = `api.sendChat`）。
class _BusyHost extends StatefulWidget {
  const _BusyHost({super.key, required this.pendingText});

  final String pendingText;

  @override
  State<_BusyHost> createState() => _BusyHostState();
}

class _BusyHostState extends State<_BusyHost> {
  /// 两件事的**发生顺序**（stop / resend）。
  final List<String> order = <String>[];

  /// 真的发出去的每一轮（旧实现这里是空的 —— 那就是 F-0007-2）。
  final List<String> rounds = <String>[];

  bool _streaming = false;

  Future<void> _stop() async {
    order.add('stop');
    _streaming = false; // 生产：服务端腾出 turn
  }

  Future<void> _resend() async {
    // 与 `shell_chat._resendLastUserMessage` 同形：没得重发就什么都不做。
    if (_streaming) return;
    final String text = widget.pendingText;
    if (text.trim().isEmpty) return;
    order.add('resend');
    rounds.add(text);
    _streaming = true; // 生产：`send()` 置位
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: ErrorBanner(
        message: '发送未被受理（服务端忙碌）',
        actions: errorActionsFor(
          '发送未被受理（服务端忙碌）',
          code: 'busy',
          onGoto: (SettingsSection _) {},
          // 生产接线同形：一份组合回调（见 `main.dart`）。
          onInterruptAndResend: () =>
              unawaited(interruptAndResend(stop: _stop, resend: _resend)),
          onResendLast: () {},
        ),
      ),
    ),
  );
}

void main() {
  group('真泵：点「打断并重发」→ 先打断，再真的发出一轮', () {
    testWidgets('旧实现（只 stop）在这里 rounds 为空', (WidgetTester tester) async {
      final GlobalKey<_BusyHostState> key = GlobalKey<_BusyHostState>();
      await tester.pumpWidget(_BusyHost(key: key, pendingText: '你好呀，帮我看看这个'));

      expect(find.text('打断并重发'), findsOneWidget, reason: 'busy 的出路必须看得见');

      await tester.tap(find.text('打断并重发'));
      await tester.pumpAndSettle();

      expect(key.currentState!.order, <String>[
        'stop',
        'resend',
      ], reason: '顺序是契约：先打断（服务端腾出 turn），再重发（否则撞同一个 busy）');
      expect(key.currentState!.rounds, <String>[
        '你好呀，帮我看看这个',
      ], reason: 'F-0007-2 的现场：旧实现只 `stop`，用户那条消息**并没有被发出去**');
    });

    testWidgets('只发一轮（点一下不该发两遍）', (WidgetTester tester) async {
      final GlobalKey<_BusyHostState> key = GlobalKey<_BusyHostState>();
      await tester.pumpWidget(_BusyHost(key: key, pendingText: '再说一次'));
      await tester.tap(find.text('打断并重发'));
      await tester.pumpAndSettle();
      expect(key.currentState!.rounds, hasLength(1));
    });
  });

  group('动作面：busy 的两条分支都给「打断并重发」，且走组合回调', () {
    test('有码：code == busy', () {
      int hits = 0;
      final List<ErrorAction> actions = errorActionsFor(
        '上游忙',
        code: 'busy',
        onGoto: (SettingsSection _) {},
        onInterruptAndResend: () => hits++,
        onResendLast: () {},
      );
      expect(actions, hasLength(1));
      expect(actions.single.label, '打断并重发');
      actions.single.onPressed();
      expect(hits, 1, reason: '按下必须走**组合**回调（旧实现接的是裸 stop）');
    });

    test('码缺失（旧服务端）：文案里带 busy 也走同一条', () {
      int hits = 0;
      final List<ErrorAction> actions = errorActionsFor(
        '服务端忙碌（busy），请等本轮收口',
        onGoto: (SettingsSection _) {},
        onInterruptAndResend: () => hits++,
        onResendLast: () {},
      );
      expect(actions.single.label, '打断并重发');
      actions.single.onPressed();
      expect(hits, 1);
    });

    test('非 busy 的分支不受影响（去设置 / 重试照旧）', () {
      final List<ErrorAction> llm = errorActionsFor(
        null,
        code: 'llm_upstream_401',
        onGoto: (SettingsSection _) {},
        onInterruptAndResend: () {},
        onResendLast: () {},
      );
      expect(llm.single.label, '去 LLM 设置');
      final List<ErrorAction> retry = errorActionsFor(
        '网络错误',
        onGoto: (SettingsSection _) {},
        onInterruptAndResend: () {},
        onResendLast: () {},
      );
      expect(retry.single.label, '重试');
    });
  });

  group('编排：interruptAndResend 两步都做，且 stop 先完成', () {
    test('resend 绝不能被吞掉（F-0007-2 的现场）', () async {
      final List<String> order = <String>[];
      await interruptAndResend(
        stop: () async => order.add('stop'),
        resend: () async => order.add('resend'),
      );
      expect(order, <String>['stop', 'resend'], reason: '旧实现只有 stop');
    });

    test('stop 还没回来就不许发（否则撞同一个 busy）', () async {
      final List<String> order = <String>[];
      final Completer<void> gate = Completer<void>();
      final Future<void> done = interruptAndResend(
        stop: () async {
          order.add('stop');
          await gate.future;
        },
        resend: () async => order.add('resend'),
      );
      // 让微任务队列跑一轮：stop 已开始、尚未完成。
      await Future<void>.delayed(Duration.zero);
      expect(order, <String>['stop'], reason: '顺序反过来就等于把两个请求的先后交给调度去掷骰子');
      gate.complete();
      await done;
      expect(order, <String>['stop', 'resend']);
    });
  });

  group('重发哪一条：lastUserText', () {
    ChatMessage user(String text) =>
        ChatMessage(role: ChatRole.user, text: text);
    ChatMessage assistant(String text, {bool failed = false}) =>
        ChatMessage(role: ChatRole.assistant, text: text, failed: failed);

    test('busy 现场：最后一条是 assistant 失败气泡 → 取它前面那条用户消息', () {
      final List<ChatMessage> messages = <ChatMessage>[
        user('第一句'),
        assistant('回答'),
        user('第二句'),
        assistant('⚠ 发送未被受理（服务端忙碌）', failed: true),
      ];
      expect(
        lastUserText(messages),
        '第二句',
        reason: '取 messages.last 会把前端的失败说明当成用户的话重发 = 伪造一条用户消息',
      );
    });

    test('系统行不是用户消息', () {
      final List<ChatMessage> messages = <ChatMessage>[
        user('我说的话'),
        ChatMessage(role: ChatRole.system, text: '本轮模型没有返回文字'),
      ];
      expect(lastUserText(messages), '我说的话');
    });

    test('没有用户消息 / 全是空文本 → null（没得重发就什么都不做）', () {
      expect(lastUserText(const <ChatMessage>[]), isNull);
      expect(lastUserText(<ChatMessage>[assistant('只有回复')]), isNull);
      expect(lastUserText(<ChatMessage>[user('   ')]), isNull);
    });

    test('最后一条就是用户消息 → 取它', () {
      expect(lastUserText(<ChatMessage>[assistant('旧回答'), user('新一句')]), '新一句');
    });
  });

  group('结构守卫：把行为级那一半钉回生产接线', () {
    /// 取 [signature] 那个函数的**函数体**（含花括号）。
    String bodyOf(String src, String signature) {
      final int at = src.indexOf(signature);
      expect(
        at,
        greaterThanOrEqualTo(0),
        reason: '找不到 `$signature` —— 结构守卫的锚点没了（先确认它是不是改名了）',
      );
      final int brace = src.indexOf('{', at);
      final int end = matchBracket(src, brace, '{', '}');
      return src.substring(brace, end + 1);
    }

    test('main.dart：busy 的出路接的是组合回调，不是裸 stop', () {
      final String src = stripCommentsAndStrings(
        File('lib/main.dart').readAsStringSync(),
      );
      final String args = balancedFrom(src, 'errorActionsFor(', '(', ')');
      expect(args, contains('onInterruptAndResend:'));
      expect(
        args,
        isNot(contains('onStop:')),
        reason: 'F-0007-2 的旧形状：busy 分支只有一个 stop 入口（按了不重发）',
      );
      final String? body = closureBodyAfter(args, 'onInterruptAndResend:');
      expect(body, isNotNull, reason: 'onInterruptAndResend 必须是回调（能写两句话）');
      expect(body!, contains('_interruptAndResend'));
    });

    test('shell_chat.dart：stop 与 resend 都接上，且 stop 在前', () {
      final String src = stripCommentsAndStrings(
        File('lib/app/shell_chat.dart').readAsStringSync(),
      );
      final int iStop = src.indexOf('stop: _stopWithCancellation');
      final int iResend = src.indexOf('resend: _resendLastUserMessage');
      expect(iStop, greaterThanOrEqualTo(0), reason: '没接 stop');
      expect(
        iResend,
        greaterThanOrEqualTo(0),
        reason: '没接 resend —— 那就是 F-0007-2',
      );
      expect(iStop, lessThan(iResend), reason: '顺序反了会撞同一个 busy');
    });

    test('shell_chat.dart：_resendLastUserMessage 有「没得重发就不伪造一轮」的早退', () {
      final String src = stripCommentsAndStrings(
        File('lib/app/shell_chat.dart').readAsStringSync(),
      );
      final String body = bodyOf(
        src,
        'Future<void> _resendLastUserMessage() async',
      );
      expect(body, contains('_chat.streaming'), reason: '已有一轮在飞时不抢');
      expect(body, contains('lastUserMessageText'));
      expect(body, contains('_chat.resendLastUserMessage()'));
      expect(
        body,
        contains('_ui.markTurnAccepted()'),
        reason: '与 _send 同形：相位在 await 之前置位，否则会退回 F-0007-1 的幽灵态',
      );
    });

    test('chat_controller.dart：重发走 send(..., echoUser: false)，不补第二条用户气泡', () {
      final String src = stripCommentsAndStrings(
        File('lib/chat/chat_controller.dart').readAsStringSync(),
      );
      final String body = bodyOf(
        src,
        'Future<bool> resendLastUserMessage() async',
      );
      expect(body, contains('lastUserText(messages)'));
      expect(
        RegExp(r'send\(\s*\w+,\s*echoUser:\s*false').hasMatch(body),
        isTrue,
        reason: '不用 echoUser: false 就会再上屏一条用户气泡 = 用户以为自己说了两遍',
      );
    });
  });
}
