/// 聊天气泡的两条用户反馈（2026-09-11）。
///
/// 1. **「把助手和用户这两个不显示在对话或者删除」** —— 气泡上方那行角色
///    文字标签去掉；但**读屏语义保留**（读屏用户没有「左右对齐」这个通道）。
/// 2. **「在无系统提示词情况下为何会有空提示词」** —— 用户看到的那句
///    「（本轮没有文字输出——通常是模型只调用了动作工具）」**不是模型的输出**，
///    是前端在 `_finishTurn` 里凭上下文猜出来的一句台词，长得却和真回复一样。
///    现在：没有文字就**不留气泡**（「没有回复」本身就是准确信息）。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_markdown.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: ListView(children: <Widget>[child])),
);

void main() {
  group('① 角色标签不再显示（用户裁决）', () {
    testWidgets('气泡里没有「你」/「助手」这行文字', (WidgetTester tester) async {
      for (final ChatRole role in ChatRole.values) {
        await tester.pumpWidget(
          wrap(
            MessageBubble(
              message: ChatMessage(role: role, text: '一条消息'),
            ),
          ),
        );
        await tester.pump();

        expect(
          find.text(role.label),
          findsNothing,
          reason: '${role.name} 气泡上还画着「${role.label}」标签',
        );
        // 正文还在（别把整条气泡删了）。
        expect(find.text('一条消息'), findsOneWidget);
      }
    });

    testWidgets('**读屏语义保留**：标签去掉不等于身份信息丢掉', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          wrap(
            MessageBubble(
              message: ChatMessage(role: ChatRole.assistant, text: '你好呀'),
            ),
          ),
        );
        await tester.pump();

        // 可见文字没有，但语义树里仍然报「助手说：你好呀」+ 单独的「助手」节点。
        expect(
          find.bySemanticsLabel(RegExp('助手说：你好呀')),
          findsOneWidget,
          reason: '读屏用户没有「左右对齐」这个通道 —— 去掉语义标签会真的分不清谁在说话',
        );
        expect(find.bySemanticsLabel('助手'), findsOneWidget);
      } finally {
        // **必须在这里释放**：框架的「有没有漏 dispose」检查跑在
        // tearDown 之前，用 `addTearDown` 会晚一步。
        handle.dispose();
      }
    });
  });

  group('② 空气泡不再被伪造成一句话', () {
    test('占位文案不再是「本轮没有文字输出…」那句猜测', () {
      // 这一条钉的是**源码里不再出现那句伪台词**——它太像真回复了。
      // （行为断言在 `chat_controller` 里，那个文件依赖 package:web，
      //  VM 测试加载不了；用源码扫描补上。）
      //
      // **先去注释再扫**（项目惯例，真正的实现只有一份：
      // `support/source_scan.dart`）。字符串**保留**：下面两处 `contains`
      // 比的正是字面量原文。
      final String src = stripCommentsKeepStrings(
        File('lib/chat/chat_controller.dart').readAsStringSync(),
      );
      expect(
        src.contains('本轮没有文字输出'),
        isFalse,
        reason: '前端又在替模型编台词了 —— 用户会以为这是模型说的',
      );
    });

    test('失败仍然给一句话（「什么都没发生」才是最糟的反馈）', () {
      final String src = stripCommentsKeepStrings(
        File('lib/chat/chat_controller.dart').readAsStringSync(),
      );
      expect(src.contains('（生成失败）'), isTrue);
    });

    testWidgets('内容为空且非流式的气泡**整个不画**（不是「高度大于 0 就算过」）', (
      WidgetTester tester,
    ) async {
      // 2026-09-28（F-0005-3）：这条从前断言的是
      // `tester.getSize(find.byType(Container).first).height > 0`——而气泡面
      // 自身的上下内边距是 2 × `Space.s2` = 16 px，**恒 > 0** ⇒ 结构上恒真；
      // 注释说「只断言没有文字」，代码里却没有任何一处真的查过文字。
      // 它声称防止的现象是真的：`text == ''` 时实现仍然构造 `_MessageBody`，
      // 界面上出现一个带底色 / 圆角 / 描边的 16 px 空泡。
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(role: ChatRole.assistant, text: ''),
          ),
        ),
      );
      await tester.pump();

      // ① 没有气泡面（底色 / 圆角 / 描边那一层 Container）。
      expect(
        find.byKey(kMessageBubbleSurfaceKey),
        findsNothing,
        reason: '空消息仍然画出了一个气泡面 —— 看起来就是「模型回了一句空白」',
      );
      // ② 没有正文节点（空字符串也不该造一个 SelectableText 出来）。
      expect(
        find.byType(SelectableText),
        findsNothing,
        reason: '空文本仍然构造了正文区',
      );
      // ③ 量化：整条气泡高度为 0（旧的 `> 0` 正是被那 16 px 内边距骗过的）。
      //
      // `skipOffstage: false`：高度为 0 的列表项在 sliver 眼里是「不可见」，
      // 默认 finder 会把它跳过 —— 那正好是「已经修好」的样子，不是找不到。
      expect(
        tester
            .getSize(find.byType(MessageBubble, skipOffstage: false))
            .height,
        0,
        reason: '空消息仍占着高度（16 px 空白气泡面）',
      );
    });

    testWidgets('边界①：**失败轮**的空文本照旧有气泡面（要能重试）', (WidgetTester tester) async {
      // 「空泡不画」这条不许把失败轮一起删掉：失败轮有真实 UI 语义
      // （危险色 + 「重试」）。
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.assistant,
              text: '',
              failed: true,
            ),
            onRetry: () {},
          ),
        ),
      );
      await tester.pump();

      expect(
        find.byKey(kMessageBubbleSurfaceKey),
        findsOneWidget,
        reason: '失败轮被当成「空消息」删掉了 —— 用户失去唯一的恢复入口',
      );
      expect(find.text('重试'), findsOneWidget);
    });

    testWidgets('边界②：**流式中**的空文本照旧有气泡面（要能看到在等）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.assistant,
              text: '',
              streaming: true,
            ),
          ),
        ),
      );
      await tester.pump();

      expect(
        find.byKey(kMessageBubbleSurfaceKey),
        findsOneWidget,
        reason: '流式空文本要有「…」占位，删掉它就变成「界面什么都没有」',
      );
      expect(find.text('…'), findsOneWidget);
    });

    testWidgets('边界③：只有思考、没有正文**不是空消息**（思考就是本轮内容）', (
      WidgetTester tester,
    ) async {
      // 这条钉住「空泡不画」的**例外**：正文空但有思考时，思考区就是本轮
      // 唯一的内容，必须照旧显示（否则又回到用户报过的「无模型返回」）。
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: ChatMessage(
              role: ChatRole.assistant,
              text: '',
              reasoning: '我先算一下：7²+11²+13²=339。',
            ),
          ),
        ),
      );
      await tester.pump();

      expect(find.byKey(kMessageBubbleSurfaceKey), findsOneWidget);
      expect(find.textContaining('我先算一下'), findsOneWidget);
    });
  });

  group('② 附带：Markdown 渲染仍然正常（别在改标签时弄丢）', () {
    test('粗体仍然被解释', () {
      expect(hasChatMarkdown('这是**重点**'), isTrue);
    });
  });
}
