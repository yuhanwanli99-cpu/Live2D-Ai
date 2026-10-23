/// 0.4.5 rc.2 「这条发给」：路由 / Pi 出站 / 侧列表的**纯 VM** 回归。
///
/// 这里钉住执行单「界面」「出站」两节的硬约束：
/// - 缺省路由 = 皮套；三个选项文字逐字；Claude Code 画出来但**不可选**；
/// - 说明句逐字；
/// - skin 路由**不发**任何 Pi 请求（既有发送路径不被改动）；
/// - pi 路由只发这一条原文 + 当前会话 id；空输入不发；
/// - 五个错误码的中文映射逐字；
/// - 上一条在飞时再发：只提示忙（不打断皮套、不排队、不新增请求）；
/// - Pi 条目只在侧列表，**从不**碰 ChatMessage 列表。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/peer_api.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/peer_route.dart';

ChatMessage message(String text) =>
    ChatMessage(role: ChatRole.user, text: text);

PeerChatController controllerFor(
  http.Client client, {
  String? Function()? sessionIdOf,
  int Function()? chatMessageCountOf,
}) {
  final PeerChatController controller = PeerChatController(
    api: PeerApi(base: 'http://peer.test', client: client),
    sessionIdOf: sessionIdOf,
    chatMessageCountOf: chatMessageCountOf,
  );
  addTearDown(controller.dispose);
  return controller;
}

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
  group('① 路由（执行单「界面」表）', () {
    test('缺省路由 = 皮套', () {
      final PeerChatController c = controllerFor(
        MockClient((_) async {
          fail('缺省路由不该发 Pi 请求');
        }),
      );
      expect(c.route, PeerRoute.skin);
      expect(c.isPi, isFalse);
    });

    test('三个选项的文字锁死', () {
      expect(PeerRoute.values.map((PeerRoute r) => r.label).toList(), <String>[
        '皮套',
        '本机 Pi',
        'Claude Code（这一版不接）',
      ]);
      expect(PeerRoute.values.map((PeerRoute r) => r.value).toList(), <String>[
        'skin',
        'pi',
        'claude',
      ]);
    });

    test('Claude Code 画出来但不可选中（不是「点了没反应」的假禁用）', () {
      final PeerChatController c = controllerFor(
        MockClient((_) async {
          fail('选路由不该发请求');
        }),
      );
      expect(PeerRoute.claude.selectable, isFalse);
      expect(c.selectRoute(PeerRoute.claude), isFalse);
      expect(c.route, PeerRoute.skin, reason: 'claude 不可选：状态必须不变');
      expect(c.selectRoute(PeerRoute.pi), isTrue);
      expect(c.route, PeerRoute.pi);
      expect(c.selectRoute(PeerRoute.claude), isFalse);
      expect(c.route, PeerRoute.pi, reason: '已选 pi 时点 claude 也不能改路由');
    });

    test('说明句逐字（0.4.5 执行单「界面」）', () {
      expect(kPeerRouteTitle, '这条发给');
      expect(
        kPeerRouteNote,
        '皮套仍走现在的对话和出声。本机 Pi 只回文字，不出声，也不带动皮套。'
        '别人来调本服务仍是入站，没挂上时没有入口。',
      );
    });
  });

  group('② skin 路由：既有发送路径原样', () {
    test('submitFromInput 返回 false（落到皮套发送），且零 Pi 请求 / 零条目', () {
      int calls = 0;
      final PeerChatController c = controllerFor(
        MockClient((_) async {
          calls++;
          return okText('不该出现');
        }),
      );
      final TextEditingController input = TextEditingController(text: '皮套的这句话');
      addTearDown(input.dispose);
      expect(c.submitFromInput(input), isFalse);
      expect(calls, 0);
      expect(c.entries, isEmpty);
      expect(input.text, '皮套的这句话', reason: 'skin 路由不该清空输入框');
    });
  });

  group('③ pi 路由：出站契约', () {
    test('POST /api/v1/peers/pi/message，body = 原文 + 当前会话 id', () async {
      final List<http.Request> requests = <http.Request>[];
      final PeerChatController c = controllerFor(
        MockClient((http.Request request) async {
          requests.add(request);
          return okText('Pi 回了一句');
        }),
        sessionIdOf: () => 'sess-42',
        chatMessageCountOf: () => 3,
      );
      c.selectRoute(PeerRoute.pi);
      final TextEditingController input = TextEditingController(
        text: '  发给 pi 的原文  ',
      );
      addTearDown(input.dispose);

      expect(c.submitFromInput(input), isTrue, reason: 'pi 路由必须被面板消化');
      expect(input.text, isEmpty, reason: '已受理 ⇒ 清空输入框（与皮套一致）');
      await c.settled;

      expect(requests, hasLength(1));
      final http.Request request = requests.single;
      expect(request.method, 'POST');
      expect(request.url.path, kPeerPiPath);
      expect(request.url.path, '/api/v1/peers/pi/message');
      expect(request.headers['content-type'], startsWith('application/json'));
      final Map<String, Object?> body =
          jsonDecode(request.body) as Map<String, Object?>;
      expect(body['text'], '发给 pi 的原文');
      expect(body['session_id'], 'sess-42');

      expect(
        c.entries.map((PeerEntry e) => (e.kind, e.text)).toList(),
        <(PeerEntryKind, String)>[
          (PeerEntryKind.user, '发给 pi 的原文'),
          (PeerEntryKind.pi, 'Pi 回了一句'),
        ],
      );
      expect(c.entries.first.anchor, 3);
      expect(c.busy, isFalse);
    });

    test('没有活动会话时 session_id 是空串', () async {
      String? seen;
      final PeerChatController c = controllerFor(
        MockClient((http.Request request) async {
          seen =
              (jsonDecode(request.body) as Map<String, Object?>)['session_id']
                  as String?;
          return okText('好');
        }),
        // 没有注入 sessionIdOf ⇒ 等价于「没有活动会话」。
      );
      c.selectRoute(PeerRoute.pi);
      expect(c.send('一句话'), isTrue);
      await c.settled;
      expect(seen, isEmpty);
    });

    test('空白输入：不发、不产生条目、不清空输入', () {
      int calls = 0;
      final PeerChatController c = controllerFor(
        MockClient((_) async {
          calls++;
          return okText('x');
        }),
      );
      c.selectRoute(PeerRoute.pi);
      final TextEditingController input = TextEditingController(text: '   ');
      addTearDown(input.dispose);
      expect(c.submitFromInput(input), isTrue, reason: 'pi 路由：空输入也算已处理');
      expect(calls, 0);
      expect(c.entries, isEmpty);
      expect(input.text, '   ');
      expect(c.send('   '), isFalse);
    });

    test('上一条在飞时再发：只提示忙，不新增请求、不清输入', () async {
      int calls = 0;
      final Completer<http.Response> gate = Completer<http.Response>();
      final PeerChatController c = controllerFor(
        MockClient((_) {
          calls++;
          return gate.future;
        }),
      );
      c.selectRoute(PeerRoute.pi);
      final TextEditingController input = TextEditingController(text: '第一条');
      addTearDown(input.dispose);

      expect(c.submitFromInput(input), isTrue);
      expect(c.busy, isTrue);
      // 让第一条请求真正进到注入的 client（HTTP 调用是异步起的）。
      await Future<void>.delayed(Duration.zero);
      expect(calls, 1);
      // 第二条：输入框里是新的文字，仍要保留（没发出去）。
      input.text = '第二条';
      expect(c.submitFromInput(input), isTrue, reason: '忙也必须由 pi 路由消化');
      expect(input.text, '第二条', reason: '忙不是「已发送」：不能清空输入');
      expect(c.busy, isTrue);
      expect(calls, 1, reason: '同一时刻只允许一个 Pi 子进程');
      expect(
        c.entries.last.kind,
        PeerEntryKind.error,
        reason: '忙只提示，不伪造 Pi 回复',
      );
      expect(c.entries.last.text, kPeerBusyText);

      gate.complete(okText('第一条的回话'));
      await c.settled;
      expect(c.busy, isFalse);
      expect(c.entries.last.kind, PeerEntryKind.pi);
    });
  });

  group('④ 错误码中文映射（执行单「出站」表）', () {
    const Map<String, (int, String)> cases = <String, (int, String)>{
      'peer_missing': (404, '本机没有 pi'),
      'peer_busy': (409, '上一条还在等 Pi'),
      'peer_timeout': (504, 'Pi 超时'),
      'peer_failed': (502, 'Pi 没有返回一句可读的话'),
      'peer_unavailable': (404, '这一版不接 Claude Code'),
    };
    for (final MapEntry<String, (int, String)> entry in cases.entries) {
      test('${entry.key} → ${entry.value.$2}', () async {
        final PeerChatController c = controllerFor(
          MockClient((_) async => errBody(entry.key, entry.value.$1)),
        );
        c.selectRoute(PeerRoute.pi);
        expect(c.send('问一句'), isTrue);
        await c.settled;
        expect(c.entries, hasLength(2));
        expect(c.entries.first.kind, PeerEntryKind.user);
        expect(c.entries.last.kind, PeerEntryKind.error);
        expect(c.entries.last.text, entry.value.$2);
        expect(c.busy, isFalse);
      });
    }

    test('错误体缺失时按 HTTP 状态兜底（404 → 本机没有 pi）', () async {
      final PeerChatController c = controllerFor(
        MockClient((_) async => http.Response('not json', 404)),
      );
      c.selectRoute(PeerRoute.pi);
      c.send('问一句');
      await c.settled;
      expect(c.entries.last.text, kPeerMissingText);
    });

    test('200 但正文不是约定 JSON：按 peer_failed 展示，不伪造回复', () async {
      final PeerChatController c = controllerFor(
        MockClient((_) async => http.Response('{"oops": true}', 200)),
      );
      c.selectRoute(PeerRoute.pi);
      c.send('问一句');
      await c.settled;
      expect(c.entries.last.kind, PeerEntryKind.error);
      expect(c.entries.last.text, kPeerFailedText);
      expect(
        c.entries.where((PeerEntry e) => e.kind == PeerEntryKind.pi),
        isEmpty,
      );
    });

    test('连接期异常（无响应）也不抛，映射成 peer_failed', () async {
      final PeerChatController c = controllerFor(
        MockClient((_) async => throw http.ClientException('boom')),
      );
      c.selectRoute(PeerRoute.pi);
      c.send('问一句');
      await c.settled;
      expect(c.entries.last.kind, PeerEntryKind.error);
      expect(c.entries.last.text, kPeerFailedText);
    });
  });

  group('⑤ 侧列表时间线（只在视图层）', () {
    test('Pi 条目按创建时的消息条数插进对应位置', () {
      final List<ChatMessage> messages = <ChatMessage>[
        message('一'),
        message('二'),
        message('三'),
      ];
      final List<PeerEntry> peers = <PeerEntry>[
        const PeerEntry(kind: PeerEntryKind.user, text: 'pi-a', anchor: 1),
        const PeerEntry(kind: PeerEntryKind.pi, text: 'pi-b', anchor: 2),
        const PeerEntry(kind: PeerEntryKind.pi, text: 'pi-c', anchor: 3),
      ];
      final List<Object> merged = mergePeerTimeline(
        messages,
        peers,
        limit: 500,
      );
      expect(
        merged.map(
          (Object e) => e is ChatMessage ? e.text : (e as PeerEntry).text,
        ),
        <String>['一', 'pi-a', '二', 'pi-b', '三', 'pi-c'],
      );
    });

    test('超过上限丢最旧的（与聊天面板视图上限同一条纪律）', () {
      final List<ChatMessage> messages = <ChatMessage>[
        message('一'),
        message('二'),
        message('三'),
      ];
      final List<Object> merged = mergePeerTimeline(messages, <PeerEntry>[
        const PeerEntry(kind: PeerEntryKind.pi, text: 'pi', anchor: 3),
      ], limit: 2);
      expect(
        merged.map(
          (Object e) => e is ChatMessage ? e.text : (e as PeerEntry).text,
        ),
        <String>['三', 'pi'],
      );
    });
  });
}
