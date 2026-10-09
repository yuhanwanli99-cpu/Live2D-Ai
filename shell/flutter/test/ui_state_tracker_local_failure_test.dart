/// 本地发送失败的**相位回落**回归（真 `UiStateTracker` + 真泵状态胶囊/聊天面板）。
///
/// 缺陷本身（2026-10-01，审计 F-0007-1）：`POST /api/v1/chat` **本地失败**
/// （网络异常 / 非 2xx（`no_supervisor`=503）/ 200 但未受理（`busy`=429））
/// 时，`ChatController._failLocalTurn` 只改自己的 `_error/_streaming`，
/// 而 `shell_chat._send` 在此之前已经调过 `_ui.markTurnAccepted()`——
/// 清相位的四个入口（`onWsStatus` 断开 / `consume(TextDelta)` /
/// `consume(TurnState)` / `markStopped`）**一个都不会被调用**，因为这三条本地
/// 失败路径后端**不广播任何 WS 帧**（已实读 `chat_routes.rs`）。结果是：
/// - 状态胶囊**永久**停在「思考中」；
/// - 发送键**永久**变「停止本轮」。
///
/// 断言刻意落在**真对象**上（真 tracker 派生相位 + 真泵 `StatePill` 与真
/// `ChatPanel`），并且断言**具体状态**（「思考中」不再出现、「空闲」出现、
/// 按钮 tooltip 从「停止本轮」变回「发送（Enter）」）——不做源码字符串扫描。
///
/// 与「真 `ChatController`」的关系（诚实说明）：`ChatController` 经
/// `audio_player.dart` 传递依赖 `dart:js_interop`/`package:web`，**VM 的
/// `flutter test` 加载不了它**（原始报错见回报），所以这里走的是它失败时
/// **必然调用**的那一个接缝（`markTurnFailed`）。生产接线只有一行，在
/// `lib/app/shell_chat.dart`。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/state/ui_state_tracker.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/state_pill.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) =>
    MaterialApp(theme: buildAppTheme(), home: Scaffold(body: child));

/// 真泵：状态胶囊（读 `tracker.phase`）+ 聊天面板（发送键按相位换语义）。
Future<void> _pumpShell(
  WidgetTester tester,
  UiStateTracker tracker,
  TextEditingController input,
) async {
  await tester.pumpWidget(
    _wrap(
      ListenableBuilder(
        listenable: tracker,
        builder: (BuildContext context, Widget? _) => Column(
          children: <Widget>[
            StatePill(phase: tracker.phase),
            Expanded(
              child: ChatPanel(
                messages: const <ChatMessage>[],
                phase: tracker.phase,
                input: input,
                onSend: () {},
                onStop: () {},
                onDismissError: () {},
                volume: 1,
                muted: false,
                onVolumeChanged: (_) {},
                onMutedChanged: (_) {},
              ),
            ),
          ],
        ),
      ),
    ),
  );
}

void main() {
  group('本地发送失败 ⇒ 相位回落（真 tracker + 真泵）', () {
    testWidgets('状态胶囊离开「思考中」、发送键离开「停止本轮」', (WidgetTester tester) async {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      final TextEditingController input = TextEditingController();
      addTearDown(input.dispose);
      // 相位「思考中」的前提是通道可用（否则 `deriveUiPhase` 先返回 offline）。
      tracker.onWsStatus(WsStatus.connected);
      await _pumpShell(tester, tracker, input);

      // ① 受理（`shell_chat._send` 的第一句）：不该停在 idle。
      tracker.markTurnAccepted();
      await tester.pump();
      expect(tracker.phase, UiPhase.thinking);
      // 2026-10-08：相位标签从「思考中」改成「正在回复」（它不是思考开关）。
      expect(find.text('正在回复'), findsOneWidget);
      expect(find.byTooltip('停止本轮'), findsOneWidget);
      expect(find.byTooltip('发送（Enter）'), findsNothing);

      // ② 本地失败（`shell_chat._send` 在 await 之后按 `sendFailedLocally` 调）。
      tracker.markTurnFailed();
      await tester.pump();

      expect(
        tracker.phase,
        UiPhase.idle,
        reason: '修复前恒为 thinking：没有任何帧会到达，相位就永久卡在「思考中」',
      );
      expect(find.text('正在回复'), findsNothing);
      expect(find.text('空闲'), findsOneWidget);
      expect(
        find.byTooltip('停止本轮'),
        findsNothing,
        reason: '修复前发送键永久变「停止本轮」，用户按了也只能打断一个没开始的轮次',
      );
      expect(find.byTooltip('发送（Enter）'), findsOneWidget);
    });

    testWidgets('**不**伪造成「已打断」：那是 markStopped 的语义', (WidgetTester tester) async {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      final TextEditingController input = TextEditingController();
      addTearDown(input.dispose);
      await _pumpShell(tester, tracker, input);

      tracker.markTurnAccepted();
      await tester.pump();
      tracker.markTurnFailed();
      await tester.pump();

      expect(tracker.interrupted, isFalse);
      expect(find.text('已打断'), findsNothing);
      expect(tracker.errorActive, isFalse);
      expect(find.text('出错'), findsNothing);
    });

    test('说话中（`voice_started`）的本地失败同样回落：不留「说话中」', () {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      tracker.onWsStatus(WsStatus.connected);

      tracker.consume(const RuntimeStatusEvent(event: 'voice_started'));
      expect(tracker.phase, UiPhase.speaking);

      tracker.markTurnFailed();
      expect(tracker.phase, UiPhase.idle);
      expect(tracker.voiceActive, isFalse);
      expect(tracker.turnActive, isFalse);
    });

    test('空闲时调用是无操作（不炸、不发多余的相位变化）', () {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      expect(tracker.phase, UiPhase.offline, reason: '前提：还没连上后端');

      int notifications = 0;
      tracker.addListener(() => notifications++);
      tracker.markTurnFailed();

      expect(tracker.phase, UiPhase.offline);
      expect(notifications, 0);
    });

    test('回落之后重发仍能进入「思考中」（不是把标记一次性用坏）', () {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      tracker.onWsStatus(WsStatus.connected);

      tracker.markTurnAccepted();
      tracker.markTurnFailed();
      expect(tracker.phase, UiPhase.idle);

      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);
    });
  });
}
