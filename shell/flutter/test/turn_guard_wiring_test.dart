/// 轮次守卫的**接线守卫**（2026-11-18 审计 #16 / #17 / #18 / #19）。
///
/// chat_controller.dart 经 ws_client.dart 依赖 package:web，VM 测试里加载不了
/// （判据本身在 turn_guard.dart 里测——见 turn_guard_test.dart）。这里守的是
/// **生产接线**：判据真的被用上、顺序正确、没有哪条路径还在静默丢帧。
///
/// 扫描面用 readLibrarySource：本库已拆 part（turn_lifecycle.dart），只读库文件
/// 会漏掉一半（dart_library_guard_test.dart 明确禁止直读 part 库）。
library;

import 'package:flutter_test/flutter_test.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

void main() {
  late String src;

  setUpAll(() {
    src = stripCommentsAndStrings(
      readLibrarySource('lib/chat/chat_controller.dart'),
    );
  });

  /// 取某个方法 / 构造的 {…} 本体。
  ///
  /// 不能取签名后的第一个 `{`：命名参数 `{String? sessionId}` 会先出现。找
  /// 「右括号 + 可选的 async + 左花括号」才是函数体起点。
  String bodyOf(String signature) {
    final int at = src.indexOf(signature);
    expect(
      at,
      greaterThanOrEqualTo(0),
      reason: '找不到 `$signature` —— 结构守卫的锚点没了（先确认是不是改名了）',
    );
    final Match? open = RegExp(r'\)\s*(?:async\s*)?\{')
        .firstMatch(src.substring(at));
    expect(open, isNotNull, reason: '`$signature` 后面找不到函数体');
    final int brace = at + open!.end - 1;
    final int end = matchBracket(src, brace, '{', '}');
    return src.substring(brace, end + 1);
  }

  group('#16：本地切走 = 作废 + 真的叫停服务端 + 迟到帧由守卫丢', () {
    test('send() 用轮次守卫登记身份、受理响应补 epoch', () {
      final String send = bodyOf('Future<void> send(String raw');
      expect(
        send.contains('_guard.start('),
        isTrue,
        reason: '新一轮必须登记 token/会话归属',
      );
      expect(
        send.contains('_guard.noteEpoch('),
        isTrue,
        reason: '受理响应里的 epoch 要记进守卫',
      );
      expect(
        send.contains('_guard.dropAbandonedTurn()'),
        isTrue,
        reason: '本地就失败的那一轮不许把「待停」闸门永久挂住',
      );
    });

    test('_abandonTurn：先记身份；只有「用户离开这个会话」才叫停服务端', () {
      final String abandon = bodyOf('void _abandonTurn({bool stopServer');
      expect(abandon.contains('_guard.abandon()'), isTrue);
      expect(abandon.contains('_stopAbandonedTurnOnServer()'), isTrue);
      expect(
        abandon.contains('stopServer'),
        isTrue,
        reason: '外部注入让位旧轮不能发 stop（会打错目标）',
      );
    });

    test('叫停是 fire-and-forget：stopChat 回执到了才解除待停', () {
      final String stop = bodyOf('void _stopAbandonedTurnOnServer()');
      expect(
        stop.contains('api.stopChat()'),
        isTrue,
        reason: '不发 POST /chat/stop 服务端会继续生成',
      );
      expect(stop.contains('onStopAcknowledged'), isTrue);
      expect(
        stop.contains('await '),
        isFalse,
        reason: '本地收口是确定的，不能让 UI 等 HTTP',
      );
    });

    test('会话操作都带 stopServer: true（#16 / #18）', () {
      for (final String signature in <String>[
        'void newSession()',
        'void selectSession(String id)',
        'void clearActiveSession()',
      ]) {
        expect(
          bodyOf(signature).contains('_abandonTurn(stopServer: true)'),
          isTrue,
          reason: '`$signature` 切走时必须在本地收口之后真的叫停服务端',
        );
      }
      final String del = bodyOf('void deleteSession(String id)');
      expect(del.contains('_abandonTurn(stopServer: true)'), isTrue);
      expect(
        del.contains('_guard.inFlight && _guard.activeSession == id'),
        isTrue,
        reason: '删**别的**会话不许打断在飞那一轮（#18 的 if and only if）',
      );
    });

    test('四类流式载荷都先过守卫；收口帧单独判 identity', () {
      expect(
        RegExp(r'_guard\.verdictFor\(').allMatches(src).length,
        greaterThanOrEqualTo(4),
        reason: 'TextDelta / ReasoningDelta / TextFallback / user_line 都必须先问守卫',
      );
      expect(
        RegExp(r'_guard\.completesCurrentTurn\(').allMatches(src).length,
        greaterThanOrEqualTo(2),
        reason: 'text_delta{completed} 与 turn_state 都要判轮次身份',
      );
      expect(
        src.contains('if (!_guard.verdictFor(epoch).accepted)'),
        isTrue,
        reason: 'user_line 的迟到帧必须静默丢弃（不许写进任何会话）',
      );
    });

    test('new_epoch：先退休旧代次再收口，迟到兜底正文也丢', () {
      final String retire = '_guard.retireActiveEpoch()';
      expect(src.contains(retire), isTrue);
      final int retireAt = src.indexOf(retire);
      final int releaseAt = src.indexOf('_releaseTurn(', retireAt);
      final int nextEpochAt = src.indexOf('_guard.onNewEpoch()', retireAt);
      expect(releaseAt, greaterThan(retireAt), reason: '先记住旧代次（收口会把代次丢掉）');
      expect(nextEpochAt, greaterThan(releaseAt), reason: '收口之后再清作废态');
    });

    test('实时通道断了要给「待停」闸门出口', () {
      expect(src.contains('if (status.isProblem) _guard.reset()'), isTrue);
    });
  });

  group('#17：有轮在跑时不许静默吞掉发送', () {
    test('send()：任何「有轮在飞」都不静默 return（跨会话 / 待停 / 只有思考）', () {
      final String send = bodyOf('Future<void> send(String raw');
      expect(
        send.contains('if (_turnInFlight)'),
        isTrue,
        reason: '闸门按「有没有轮在飞」判，而不是只看 _streaming',
      );
      expect(
        send.contains('_guard.activeSession != sessions.activeId'),
        isTrue,
        reason: '气泡属于别的会话也算「还在跑」',
      );
      expect(
        send.contains('!_streaming ||'),
        isTrue,
        reason: '只有思考、正文还没开始的轮次同样占着服务端生成槽',
      );
      expect(
        send.split('_refuseSendWhileTurnElsewhere()').length - 1,
        2,
        reason: '「有轮在飞」「待停」两条路径都必须回一行原因',
      );
      expect(
        send.contains('if (_streaming) return;'),
        isFalse,
        reason: '不许再有裸 return：同会话当前轮走「不开新轮但也不回原因」这一支',
      );
      expect(send.contains('_guard.pendingStop'), isTrue);
    });

    test('拒绝时写一行原因 + 机器码，并让组合根回落相位', () {
      final String refuse = bodyOf('void _refuseSendWhileTurnElsewhere()');
      expect(refuse.contains('kTurnElsewhereMessage'), isTrue);
      expect(refuse.contains('kTurnElsewhereCode'), isTrue);
      expect(
        refuse.contains('_sendFailedLocally = true'),
        isTrue,
        reason: 'shell 已 markTurnAccepted，不回落相位就会停在转圈',
      );
    });
  });

  group('#19：user_line 写回它自己的会话', () {
    test('判据用目标会话的消息，append 按帧上的 session_id 分流', () {
      final String onLine = bodyOf('void _onUserLine(');
      expect(onLine.contains('_sessionById(frameSessionId)'), isTrue);
      expect(onLine.contains('lastUserText(targetMessages)'), isTrue);
      expect(
        onLine.contains('acceptInjectedUserTurn(body, sessionId: target?.id)'),
        isTrue,
        reason: '注入那一轮也必须落在它发起的会话上（否则助手气泡又串回去）',
      );
      expect(onLine.contains('_appendToSession(target, line)'), isTrue);
    });

    test('acceptInjectedUserTurn 接受可选 sessionId 并记进守卫', () {
      final String injected = bodyOf('bool acceptInjectedUserTurn(');
      expect(
        src.contains(
          'bool acceptInjectedUserTurn(String text, {String? sessionId})',
        ),
        isTrue,
        reason: '注入入口必须接受帧上带来的会话 id（#19）',
      );
      expect(injected.contains('_guard.start(sessionId:'), isTrue);
      expect(injected.contains('_appendToSession(target, line)'), isTrue);
      // 既有约束（turn_liveness_test）：重复判据必须排在 _abandonTurn 之前。
      expect(
        injected.indexOf('isDuplicateInjectedUserLine(') <
            injected.indexOf('_abandonTurn()'),
        isTrue,
      );
    });

    test('append 到非活动会话时补齐 updatedAt + 裁剪（与 store.append 同副作用）', () {
      final String append = bodyOf('void _appendToSession(');
      expect(append.contains('session.messages.add(message)'), isTrue);
      expect(append.contains('session.updatedAt = DateTime.now()'), isTrue);
      expect(append.contains('kMaxMessagesPerSession'), isTrue);
    });
  });
}
