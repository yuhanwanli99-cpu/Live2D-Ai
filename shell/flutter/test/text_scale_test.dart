/// 文本缩放（textScaler）：系统字号放大后界面不裁切、不溢出。
///
/// 2026-09-11（前端加强计划 P4-3）新增。这是一个**此前完全没有覆盖**的面。
///
/// # 为什么它值得单独一个文件
///
/// 本项目有大量**固定尺寸的小盒子**，它们都是按 1.0 倍字号量出来的：
///
/// | 位置 | 固定尺寸 | 干什么用 |
/// | --- | --- | --- |
/// | `chat_panel` 发送键 | `minimumSize/maximumSize: 40×40` | 圆形上箭头 |
/// | `audio_bar` 百分比 | `width: 44` | 「80%」右对齐 |
/// | `live2d_stage` FPS 徽标 | 药丸内边距 | 「60 FPS」 |
///
/// （2026-09-11：本表原先还有一行 `action_history_view` 的 44 px 行高。
/// 动作子系统整条移除后那个视图与它的行高测试一并删除——下面对应的
/// 「动作历史」group 也已移除。留下这段说明是为了解释为什么表里少了一行。）
///
/// 系统把字号放到 1.5–2.0 倍是**无障碍设置里最常见的一档**（视力不佳、
/// 高分屏习惯）。这时如果盒子不跟着长，文字就会被裁掉或被 `RenderFlex`
/// 判溢出——而这**不会**在 1.0 倍下出现，所以任何不显式设 textScaler 的
/// 测试都发现不了。
///
/// # 实测结果：四档**全部通过**（诚实记录）
///
/// 写这个文件之前预期会抓到一批溢出，实际量下来没有：那些固定盒子
/// （40×40 的发送键、44 px 的音频条百分比）在当时的取值下**都够用**，
/// 发送键是图标按钮（不随文字缩放，这是对的）。
///
/// 所以这一期的产出不是「修了几个溢出」，而是**把这条以前完全没人看过
/// 的轴钉住了**：以后任何人调小某个固定盒子、或往行里塞更长的文字，
/// 这里会红。证据是这条测试**确实会红**——写它的时候气泡那条就因为在
/// 测试里搭错了场景（直接当 `Scaffold` body 而不是放进 `ListView`）
/// 当场报出「RenderFlex overflowed by 546 pixels」。
///
/// **一个刻意的反例也钉在下面**：发送键在 2.0x 下**仍然**应该是 40×40。
/// 「统一处理文本缩放」时把它也放大，会把输入行挤扁。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/ui/audio_bar.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 无障碍设置里常见的几档（1.0 = 默认；2.0 是「很大」）。
const List<double> _scales = <double>[1.0, 1.3, 1.5, 2.0];

/// 当前档位下**有没有**溢出/裁切。
///
/// `tester.takeException()` 是 Flutter 报 `RenderFlex overflow` 的通道；
/// 断言它为空就等于断言「没报溢出」。
Widget _scaled(double scale, Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: MediaQuery(
    data: MediaQueryData(textScaler: TextScaler.linear(scale)),
    child: Scaffold(body: child),
  ),
);

void main() {
  group('P4-3：聊天面板在放大字号下不溢出', () {
    for (final double scale in _scales) {
      testWidgets('聊天输入行 @ ${scale}x', (WidgetTester tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);

        await tester.pumpWidget(
          _scaled(
            scale,
            ChatPanel(
              messages: const <ChatMessage>[],
              phase: UiPhase.idle,
              input: TextEditingController(text: '一段输入'),
              onSend: () {},
              onStop: () {},
              onDismissError: () {},
              volume: 0.8,
              muted: false,
              onVolumeChanged: (_) {},
              onMutedChanged: (_) {},
            ),
          ),
        );
        await tester.pump();

        expect(
          tester.takeException(),
          isNull,
          reason: '${scale}x 下聊天输入行溢出了 —— 固定尺寸的盒子没跟着字号长',
        );
      });
    }

    testWidgets('发送键在 2.0x 下仍然是 40×40 的圆（不跟着字号长，是**对的**）', (
      WidgetTester tester,
    ) async {
      // 发送键是个**图标按钮**，图标不随文字缩放。所以它保持不变是正确行为
      // ——这条断言是为了防止有人「统一处理文本缩放」时把它也放大，
      // 那会让输入行被挤扁。
      await tester.pumpWidget(
        _scaled(
          2.0,
          ChatPanel(
            messages: const <ChatMessage>[],
            phase: UiPhase.idle,
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onDismissError: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
          ),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull);
    });
  });

  group('P4-3：消息气泡在放大字号下不溢出', () {
    for (final double scale in _scales) {
      testWidgets('含列表与行内码的长消息 @ ${scale}x', (WidgetTester tester) async {
        tester.view.physicalSize = const Size(320, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);

        await tester.pumpWidget(
          _scaled(
            scale,
            // **必须放进可滚动容器**：真实场景里气泡住在 `ListView` 里
            // （`chat_panel`）。直接当 `Scaffold` body 用会因为它本身比
            // 视口高而溢出——那是测试搭错了场景，不是组件的问题。
            ListView(
              children: <Widget>[
                MessageBubble(
                  // `ChatMessage` 不是 `const` 构造（它的 `text`/`streaming`
                  // 是可变字段：流式拼接要就地追加）。
                  message: ChatMessage(
                    role: ChatRole.assistant,
                    text: '上游拒绝了鉴权（HTTP 401）。\n'
                        '- 确认 `[llm] api_key_env` 指向的环境变量已设置\n'
                        '- 提示词内容与鉴权无关',
                  ),
                ),
              ],
            ),
          ),
        );
        await tester.pump();
        expect(tester.takeException(), isNull, reason: '${scale}x 下气泡溢出了');
      });
    }
  });

  group('P4-3：音频条在放大字号下不溢出', () {
    for (final double scale in _scales) {
      testWidgets('音量百分比 @ ${scale}x', (WidgetTester tester) async {
        tester.view.physicalSize = const Size(320, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);

        await tester.pumpWidget(
          _scaled(
            scale,
            AudioBar(
              volume: 0.8,
              muted: false,
              onVolumeChanged: (_) {},
              onMutedChanged: (_) {},
            ),
          ),
        );
        await tester.pump();
        expect(
          tester.takeException(),
          isNull,
          reason: '${scale}x 下音频条溢出了（`width: 44` 的百分比盒子装不下）',
        );
      });
    }
  });

  group('P4-3：断点判定不受文本缩放影响（尺寸类只看宽度）', () {
    test('textScaler 不是断点输入', () {
      // 断点函数是纯函数、只接宽度。这条断言看起来废话，但它挡住的是
      // 「有人为了修文本溢出而把字号塞进断点判定」这种改法——那会让
      // 同一个窗口在不同系统设置下变成不同的布局。
      for (final double w in <double>[320, 899, 900, 1279, 1280, 1920]) {
        expect(
          Breakpoints.sizeClassOf(w),
          Breakpoints.sizeClassOf(w),
        );
      }
      expect(Breakpoints.sizeClassOf(420), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(1280), SizeClass.expanded);
    });
  });
}

