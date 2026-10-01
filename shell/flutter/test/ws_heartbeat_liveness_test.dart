/// 心跳看门狗**判据**的回归（纯逻辑，VM 直接跑）。
///
/// 缺陷本身（2026-10-01，审计 F-0008-1）：服务端每 10 s 发一条心跳，前端把帧
/// 解析了、下发了，然后**被两个消费者丢弃**——`chat_controller.dart` 的
/// `case HeartbeatEvent(): break` 与 `ui_state_tracker.dart` 的 `return false`。
/// 于是前端**没有任何时间判据**：半开连接（休眠唤醒 / NAT 静默丢包 / 服务端被
/// SIGKILL；此时浏览器 `readyState` **仍是 OPEN**，因为它只在「尝试收发并失败」
/// 时才改状态，而本客户端从不发帧）不可检测，`ensureConnected()` 一直走健康
/// 分支不重连，`_status` 一直是「已连接」，`mustReleaseTurnOnWsLoss` 的前提
/// （`status.isProblem`）永远不成立 ⇒「`POST /api/v1/chat` 成功但收不到回复」
/// 的幽灵态可以永久挂着。
///
/// 本文件只测**判据**（纯函数、零依赖）。消费者侧（看门狗把相位/连接态真的
/// 改掉）在 `ui_state_tracker_half_open_test.dart` 里用真 tracker 断言。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_liveness.dart';

void main() {
  // 基准时刻固定成 UTC：判据只做差，但固定基准让失败信息可复算
  // （不受本地时区 / 夏令时影响）。
  final DateTime last = DateTime.utc(2026, 10, 1, 20, 0, 0);
  HeartbeatLiveness atSilence(Duration silent) =>
      heartbeatLiveness(lastHeartbeatAt: last, now: last.add(silent));

  const Duration oneMs = Duration(milliseconds: 1);

  group('heartbeatLiveness：三态由「静默时长」判定（含边界 ±1ms）', () {
    test('刚收到心跳（静默 0）→ online', () {
      expect(atSilence(Duration.zero), HeartbeatLiveness.online);
    });

    test('可疑阈值 **-1ms** → 仍是 online（边界之外）', () {
      expect(
        atSilence(kWsHeartbeatSuspectAfter - oneMs),
        HeartbeatLiveness.online,
      );
    });

    test('**可疑阈值恰好** → suspect（边界含在嫌疑侧，写进契约）', () {
      expect(atSilence(kWsHeartbeatSuspectAfter), HeartbeatLiveness.suspect);
    });

    test('可疑阈值 **+1ms** → suspect（边界之内）', () {
      expect(
        atSilence(kWsHeartbeatSuspectAfter + oneMs),
        HeartbeatLiveness.suspect,
      );
    });

    test('死链阈值 **-1ms** → 仍是 suspect（还没到死链档）', () {
      expect(
        atSilence(kWsHeartbeatOfflineAfter - oneMs),
        HeartbeatLiveness.suspect,
      );
    });

    test('**死链阈值恰好** → offline（边界含在死链侧）', () {
      expect(atSilence(kWsHeartbeatOfflineAfter), HeartbeatLiveness.offline);
    });

    test('死链阈值 **+1ms** → offline', () {
      expect(
        atSilence(kWsHeartbeatOfflineAfter + oneMs),
        HeartbeatLiveness.offline,
      );
    });

    test('从未收到过任何帧（null）→ online：**「未知」不等于「已死」**', () {
      // 刚建连、`subscribe_ack` 还没到时不能判死——否则每次刷新页面都会先闪
      // 一下「后端未连接」。半开连接的前提是「曾经收到过」。
      expect(
        heartbeatLiveness(lastHeartbeatAt: null, now: last),
        HeartbeatLiveness.online,
      );
    });

    test('时钟回拨（now 早于最后心跳）→ online，不判死', () {
      expect(
        heartbeatLiveness(
          lastHeartbeatAt: last,
          now: last.subtract(const Duration(seconds: 5)),
        ),
        HeartbeatLiveness.online,
      );
    });

    test('阈值可注入：判据不绑死在这两个常量上', () {
      const Duration suspect = Duration(seconds: 1);
      const Duration offline = Duration(seconds: 3);
      HeartbeatLiveness withThresholds(Duration silent) => heartbeatLiveness(
        lastHeartbeatAt: last,
        now: last.add(silent),
        suspectAfter: suspect,
        offlineAfter: offline,
      );
      expect(withThresholds(const Duration(milliseconds: 999)),
          HeartbeatLiveness.online);
      expect(withThresholds(suspect), HeartbeatLiveness.suspect);
      expect(withThresholds(offline - oneMs), HeartbeatLiveness.suspect);
      expect(withThresholds(offline), HeartbeatLiveness.offline);
    });
  });

  group('阈值常量与「服务端心跳间隔」的关系', () {
    test('可疑阈值 = 3 个心跳周期（错过 1 次可能是抖动，连续 3 次不是）', () {
      expect(kWsHeartbeatSuspectAfter, kWsHeartbeatInterval * 3);
    });

    test('死链阈值 = 9 个心跳周期', () {
      expect(kWsHeartbeatOfflineAfter, kWsHeartbeatInterval * 9);
    });

    test('死链阈值严格晚于可疑阈值（否则 suspect 永远显示不出来）', () {
      expect(kWsHeartbeatOfflineAfter > kWsHeartbeatSuspectAfter, isTrue);
    });
  });

  group('isChannelTrustworthy：readyState 之外的第二重判据', () {
    test('OPEN + online → 可信', () {
      expect(
        isChannelTrustworthy(
          readyState: kSocketOpen,
          liveness: HeartbeatLiveness.online,
        ),
        isTrue,
      );
    });

    test('OPEN + suspect → **仍然可信**：重连由看门狗负责，不在这里越权', () {
      expect(
        isChannelTrustworthy(
          readyState: kSocketOpen,
          liveness: HeartbeatLiveness.suspect,
        ),
        isTrue,
      );
    });

    test('OPEN + offline → **不可信**：正是半开连接那种坏法', () {
      expect(
        isChannelTrustworthy(
          readyState: kSocketOpen,
          liveness: HeartbeatLiveness.offline,
        ),
        isFalse,
      );
    });

    test('CLOSING / CLOSED → 无论 liveness 都不可信（两道判据都要成立）', () {
      for (final int state in <int>[kSocketClosing, kSocketClosed]) {
        for (final HeartbeatLiveness liveness in HeartbeatLiveness.values) {
          expect(
            isChannelTrustworthy(readyState: state, liveness: liveness),
            isFalse,
            reason: 'readyState=$state liveness=$liveness',
          );
        }
      }
    });

    test('CONNECTING + online → 可信（正在建连，重开只会多一条废连接）', () {
      expect(
        isChannelTrustworthy(
          readyState: kSocketConnecting,
          liveness: HeartbeatLiveness.online,
        ),
        isTrue,
      );
    });
  });
}
