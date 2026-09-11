import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_liveness.dart';

/// `ensureConnected()` 存活判据的回归（纯逻辑，VM 直接跑）。
///
/// 缺陷本身（2026-09-11）：判据曾是 `_socket != null`——把「有没有 socket
/// 对象」当成「连接还活着」。`close()` 之后 `onclose` 是**异步**回调，
/// 这中间 `_socket` 仍非 null 而连接已经废了；而 `shutdown_ready` 走的正是
/// 「先 close 再退避重连」。于是那份专为「POST 成功但收不到回复」而写的
/// 保险，恰好在最需要它的那一刻变成空操作。
///
/// `ws_client.dart` 依赖 `package:web`，VM 测试里加载不了，所以判据抽在这里。
void main() {
  group('readyState → 还值不值得留着这个 socket', () {
    test('CONNECTING(0) 留着：正在建连，重开只会多一条废连接', () {
      expect(isLiveSocket(kSocketConnecting), isTrue);
      expect(isZombieSocket(kSocketConnecting), isFalse);
    });

    test('OPEN(1) 留着：可用', () {
      expect(isLiveSocket(kSocketOpen), isTrue);
      expect(isZombieSocket(kSocketOpen), isFalse);
    });

    test('**CLOSING(2) 是僵尸**：连接已废，必须摘掉重开', () {
      expect(isLiveSocket(kSocketClosing), isFalse);
      expect(isZombieSocket(kSocketClosing), isTrue);
    });

    test('**CLOSED(3) 是僵尸**', () {
      expect(isLiveSocket(kSocketClosed), isFalse);
      expect(isZombieSocket(kSocketClosed), isTrue);
    });

    test('两个判据互补且覆盖全部 4 个取值', () {
      for (int state = 0; state <= 3; state++) {
        expect(
          isLiveSocket(state) != isZombieSocket(state),
          isTrue,
          reason: 'readyState=$state 必须恰好落在「留着」或「僵尸」之一',
        );
      }
    });

    test('取值与 W3C 常量一致（写错数字会静默失效）', () {
      expect(<int>[
        kSocketConnecting,
        kSocketOpen,
        kSocketClosing,
        kSocketClosed,
      ], <int>[0, 1, 2, 3]);
    });
  });
}
