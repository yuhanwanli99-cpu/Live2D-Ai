/// 0.4.5 rc.2 界面：输入框上方的「这条发给」路由 + Pi 侧列表气泡（widget）。
///
/// 不启动真实服务：HTTP 用 `MockClient` 注入。这里钉的是**用户看得见**的契约：
/// - 三个选项文字逐字、缺省皮套、Claude Code 禁用；
/// - 说明句逐字；
/// - skin 路由下圆键仍走宿主既有发送（onSend），零 Pi 请求；
/// - pi 路由下圆键发 Pi，回复以「Pi」标签出现在侧列表，ChatMessage 历史零改动；
/// - 失败显示错误条目而不是伪造的 Pi 气泡；忙时只提示忙、不打断皮套。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/peer_api.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/peer_route.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

PeerChatController controllerFor(
  http.Client client, {
  String? Function()? sessionIdOf,
}) {
  final PeerChatController controller = PeerChatController(
    api: PeerApi(base: 'http://peer.test', client: client),
    sessionIdOf: sessionIdOf,
  );
  addTearDown(controller.dispose);
  return controller;
}

ChatPanel panelFor({
  required TextEditingController input,
  required PeerChatController peer,
  required VoidCallback onSend,
  List<ChatMessage> messages = const <ChatMessage>[],
}) => ChatPanel(
  messages: messages,
  phase: UiPhase.idle,
  input: input,
  onSend: onSend,
  onDismissError: () {},
  volume: 1,
  muted: false,
  onVolumeChanged: (_) {},
  onMutedChanged: (_) {},
  peer: peer,
);

http.Response okText(String text) => http.Response(
  jsonEncode(<String, Object?>{'text': text}),
  200,
  headers: const <String, String>{'content-type': 'application/json'},
);

http.Response errBody(String code, int status) => http.Response(
  jsonEncode(<String, Object?>{
    'error': <String, Object?>{'code': code, 'message': 'x'},
  }),
  status,
  headers: const <String, String>{'content-type': 'application/json'},
);

void main() {
  testWidgets('路由控件：标题 / 三个选项 / 说明句逐字，缺省选中皮套', (WidgetTester tester) async {
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    final PeerChatController peer = controllerFor(
      MockClient((_) async {
        fail('只是渲染控件，不该发 Pi 请求');
      }),
    );
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () {})),
    );

    expect(find.text('这条发给'), findsOneWidget);
    expect(find.text('皮套'), findsOneWidget);
    expect(find.text('本机 Pi'), findsOneWidget);
    expect(find.text('Claude Code（这一版不接）'), findsOneWidget);
    expect(
      find.text(
        '皮套仍走现在的对话和出声。本机 Pi 只回文字，不出声，也不带动皮套。'
        '别人来调本服务仍是入站，没挂上时没有入口。',
      ),
      findsOneWidget,
    );

    final ChoiceChip skinChip = tester.widget<ChoiceChip>(
      find.widgetWithText(ChoiceChip, '皮套'),
    );
    expect(skinChip.selected, isTrue, reason: '缺省路由必须是皮套');
    expect(peer.route, PeerRoute.skin);
  });

  testWidgets('Claude Code 选项禁用且点不动（状态不变）', (WidgetTester tester) async {
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    final PeerChatController peer = controllerFor(
      MockClient((_) async => okText('x')),
    );
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () {})),
    );

    final ChoiceChip claudeChip = tester.widget<ChoiceChip>(
      find.widgetWithText(ChoiceChip, 'Claude Code（这一版不接）'),
    );
    expect(
      claudeChip.onSelected,
      isNull,
      reason: '禁用态必须由 Flutter 的 onSelected == null 表达，而不是自绘假禁用',
    );

    await tester.tap(
      find.widgetWithText(ChoiceChip, 'Claude Code（这一版不接）'),
      warnIfMissed: false,
    );
    await tester.pump();
    expect(peer.route, PeerRoute.skin);
  });

  testWidgets('skin 路由：圆键仍走既有发送路径，零 Pi 请求', (WidgetTester tester) async {
    int sends = 0;
    int calls = 0;
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    final PeerChatController peer = controllerFor(
      MockClient((_) async {
        calls++;
        return okText('不该出现');
      }),
    );
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () => sends++)),
    );

    await tester.enterText(find.byType(TextField), '皮套的这句话');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pump();

    expect(sends, 1, reason: 'skin 路由必须落到宿主的既有发送路径');
    expect(calls, 0, reason: 'skin 路由不得发 Pi 请求');
    expect(peer.entries, isEmpty);
    expect(peer.route, PeerRoute.skin);
  });

  testWidgets('pi 路由：发原文 + 会话 id，回复带「Pi」标签，ChatMessage 零改动', (
    WidgetTester tester,
  ) async {
    final List<http.Request> requests = <http.Request>[];
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    final PeerControllerHarness harness = PeerControllerHarness(
      client: MockClient((http.Request request) async {
        requests.add(request);
        return okText('Pi 的回复');
      }),
      sessionIdOf: () => 'sess-42',
    );
    addTearDown(harness.controller.dispose);

    final List<ChatMessage> history = <ChatMessage>[
      ChatMessage(role: ChatRole.user, text: '皮套历史'),
    ];
    await tester.pumpWidget(
      wrap(
        panelFor(
          input: input,
          peer: harness.controller,
          onSend: () => fail('pi 路由不得落到皮套发送路径'),
          messages: history,
        ),
      ),
    );

    await tester.tap(find.widgetWithText(ChoiceChip, '本机 Pi'));
    await tester.pump();
    expect(harness.controller.route, PeerRoute.pi);

    await tester.enterText(find.byType(TextField), '发给 pi 的原文');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pumpAndSettle();

    expect(requests, hasLength(1));
    final Map<String, Object?> body =
        jsonDecode(requests.single.body) as Map<String, Object?>;
    expect(requests.single.url.path, '/api/v1/peers/pi/message');
    expect(body['text'], '发给 pi 的原文');
    expect(body['session_id'], 'sess-42');

    // 回复出现在聊天列表区，带「Pi」标签。
    expect(find.text('Pi'), findsOneWidget);
    expect(find.text('Pi 的回复'), findsOneWidget);
    // 用户原文在同一条记录里可见（但不是 ChatMessage）。
    expect(find.text('发给 pi 的原文'), findsOneWidget);

    // 皮套模型会看到的历史一行没变。
    expect(history, hasLength(1));
    expect(history.single.text, '皮套历史');
    expect(harness.controller.entries.last.kind, PeerEntryKind.pi);
    expect(input.text, isEmpty, reason: '已发送 ⇒ 输入框清空（与皮套一致）');
  });

  testWidgets('pi 失败：侧列表显示错误条目，不伪造 Pi 回复', (WidgetTester tester) async {
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    final PeerChatController peer = controllerFor(
      MockClient((_) async => errBody('peer_missing', 404)),
    );
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () {})),
    );

    await tester.tap(find.widgetWithText(ChoiceChip, '本机 Pi'));
    await tester.pump();
    await tester.enterText(find.byType(TextField), '问一句');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pumpAndSettle();

    expect(find.text('本机没有 pi'), findsOneWidget);
    expect(find.text('Pi'), findsNothing, reason: '失败不是 Pi 的回复：不能给它挂「Pi」标签');
    expect(find.byIcon(Icons.error_outline), findsOneWidget);
    expect(peer.entries.last.kind, PeerEntryKind.error);
  });

  testWidgets('pi 忙：再发只提示忙，不打断皮套、不新增请求', (WidgetTester tester) async {
    int calls = 0;
    int skinSends = 0;
    final Completer<http.Response> gate = Completer<http.Response>();
    final PeerChatController peer = controllerFor(
      MockClient((_) {
        calls++;
        return gate.future;
      }),
    );
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () => skinSends++)),
    );

    await tester.tap(find.widgetWithText(ChoiceChip, '本机 Pi'));
    await tester.pump();

    await tester.enterText(find.byType(TextField), '第一条');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pump();
    expect(peer.busy, isTrue);

    await tester.enterText(find.byType(TextField), '第二条');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pump();

    expect(find.text('上一条还在等 Pi'), findsOneWidget);
    expect(calls, 1, reason: '同一时刻只允许一个 Pi 请求');
    expect(skinSends, 0, reason: 'pi 忙不得打断 / 落到皮套发送路径');
    expect(input.text, '第二条', reason: '没发出去的文字要留在输入框');

    gate.complete(okText('第一条的回复'));
    await tester.pumpAndSettle();
    expect(peer.busy, isFalse);
    expect(find.text('第一条的回复'), findsOneWidget);
  });

  testWidgets('pi 路由：空输入不发', (WidgetTester tester) async {
    int calls = 0;
    final PeerChatController peer = controllerFor(
      MockClient((_) async {
        calls++;
        return okText('x');
      }),
    );
    final TextEditingController input = TextEditingController();
    addTearDown(input.dispose);
    await tester.pumpWidget(
      wrap(panelFor(input: input, peer: peer, onSend: () {})),
    );
    await tester.tap(find.widgetWithText(ChoiceChip, '本机 Pi'));
    await tester.pump();
    await tester.enterText(find.byType(TextField), '   ');
    await tester.tap(
      find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
    );
    await tester.pumpAndSettle();
    expect(calls, 0);
    expect(peer.entries, isEmpty);
  });
}

/// 小夹具：让 widget 测试能读 controller，又保持构造处一处。
class PeerControllerHarness {
  PeerControllerHarness({
    required http.Client client,
    String? Function()? sessionIdOf,
  }) : controller = PeerChatController(
         api: PeerApi(base: 'http://peer.test', client: client),
         sessionIdOf: sessionIdOf,
       );

  final PeerChatController controller;
}
