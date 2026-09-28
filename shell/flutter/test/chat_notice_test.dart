/// 「模型整轮没有返回文字」这条**系统提示**的回归（2026-09-11）。
///
/// 用户报的原文：「第一次对话之后没输出了」。
///
/// 实测复现（探针驱动真实链路）：发「点点头。」→ **零文字、零音频**、
/// `turn_state{status:"completed"}`、后端日志与 CosyVoice 日志里**没有任何
/// 错误**。也就是说那是一个**完全正常、只是没有说话**的回合。
///
/// 而前端当时的处理是「空气泡直接删掉」（2026-09-11 为了不伪造模型台词而
/// 改的）——于是界面上**什么都没有**，与「应用坏了」无法区分。
///
/// 现在的规则：换成一条 `ChatRole.system` 的系统行。既看得见，又一眼看出
/// 不是角色说的话。本文件同时钉住「看得见」和「不像回复」这两件事。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/turn_liveness.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: Center(child: SizedBox(width: 340, child: child)),
  ),
);

void main() {
  group('① 无文字的一轮在界面上**看得见**（这是那个 bug 的钉子）', () {
    testWidgets('系统提示的文案真的画出来了', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.system,
              text: kWordlessTurnNotice,
            ),
          ),
        ),
      );
      await tester.pump();

      expect(
        find.text(kWordlessTurnNotice),
        findsOneWidget,
        reason: '这条提示看不见 = 用户又会以为「应用没输出了」',
      );
    });
  });

  group('② 但**不像角色的回复**（不许重犯「伪造台词」那个错）', () {
    testWidgets('没有复制 / 重试按钮（它不是内容，是状态说明）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.system,
              text: kWordlessTurnNotice,
            ),
          ),
        ),
      );
      await tester.pump();

      expect(find.text('复制'), findsNothing);
      expect(find.text('重试'), findsNothing);
    });

    testWidgets('不画角色标签文字（与用户裁决一致：气泡上没有「助手」）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.system,
              text: kWordlessTurnNotice,
            ),
          ),
        ),
      );
      await tester.pump();

      expect(find.text('系统'), findsNothing);
      expect(find.text('助手'), findsNothing);
    });

    testWidgets('三个通道同时与对话气泡区分：居中 + 弱色 + 小一号字级', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.system,
              text: kWordlessTurnNotice,
            ),
          ),
        ),
      );
      await tester.pump();

      // ① 居中（用户靠右、角色靠左）。
      expect(
        find.ancestor(
          of: find.text(kWordlessTurnNotice),
          matching: find.byType(Center),
        ),
        findsWidgets,
        reason: '不居中就只剩颜色一个通道，色盲用户分不出这是系统行',
      );

      // ②③ 弱色 + 小一号字级（写死颜色/字号会让换主题时漏改）。
      final BuildContext context = tester.element(find.byType(MessageBubble));
      final TextStyle? style = tester
          .widget<Text>(find.text(kWordlessTurnNotice))
          .style;
      expect(style?.color, appColorsOf(context).contentMuted);
      expect(style?.fontSize, Theme.of(context).textTheme.bodySmall?.fontSize);
    });

    testWidgets('读屏听到的是「系统提示：…」而不是「助手说：…」', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          wrap(
            MessageBubble(
              message: ChatMessage(
                role: ChatRole.system,
                text: kWordlessTurnNotice,
              ),
            ),
          ),
        );
        await tester.pump();

        expect(find.bySemanticsLabel(RegExp('系统提示：')), findsOneWidget);
        expect(find.bySemanticsLabel(RegExp('助手说：')), findsNothing);
      } finally {
        handle.dispose();
      }
    });
  });

  group('③ 接线守卫：判据与文案必须真的被控制器用上', () {
    final String controller = File('lib/chat/chat_controller.dart')
        .readAsStringSync();

    test('`_finishTurn` 走 `settleTurn`（而不是自己写 if 分支）', () {
      expect(
        controller.contains('settleTurn('),
        isTrue,
        reason: '判据写在纯模块里才测得到；控制器自己写分支就绕开了回归',
      );
    });

    test('无文字时的文案来自 `kWordlessTurnNotice`（同一份字符串）', () {
      expect(
        controller.contains('kWordlessTurnNotice'),
        isTrue,
        reason: '控制器另写字面量 → 改文案时漏改一处，测试与界面就会说两套话',
      );
    });

    test('断连收口走 `mustReleaseTurnOnWsLoss`', () {
      expect(controller.contains('mustReleaseTurnOnWsLoss('), isTrue);
    });

    testWidgets('气泡层把 `ChatRole.system` 单独分流（**行为**断言，不是扫源码字面量）', (
      WidgetTester tester,
    ) async {
      // 2026-09-28（F-0005-6）：这条从前断言的是
      // `message_bubble.dart` 源码里 `contains('ChatRole.system')`——而该词在
      // 那个文件里出现**两次**：真正分流的
      // `if (message.role == ChatRole.system)` 与语义文案
      // `'${ChatRole.system.label}提示：$text'`。把分流分支删掉，这句断言
      // 照样为真 ⇒ 守门人指错了地方（同组另外三条源码扫描各只有一处命中，
      // 是有效的）。这里改成行为断言：系统消息**不进气泡那条路**。
      Future<void> pumpRole(WidgetTester tester, ChatRole role) =>
          tester.pumpWidget(
            wrap(
              MessageBubble(
                message: ChatMessage(role: role, text: kWordlessTurnNotice),
              ),
            ),
          );

      // 对照组：助手消息**必须**有气泡面——否则下一条的 findsNothing 是恒真的。
      await pumpRole(tester, ChatRole.assistant);
      expect(find.byKey(kMessageBubbleSurfaceKey), findsOneWidget);

      // 系统消息：没有气泡面（没有底色 / 圆角 / 描边那一层），走的是系统行。
      await pumpRole(tester, ChatRole.system);
      expect(
        find.byKey(kMessageBubbleSurfaceKey),
        findsNothing,
        reason: '系统行被画成了气泡面 = 把状态说明伪装成角色台词（伪造台词）',
      );
      expect(find.text(kWordlessTurnNotice), findsOneWidget);
    });
  });
}
