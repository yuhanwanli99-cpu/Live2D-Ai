/// **verifier 对抗性探针**（不属于提交 93416f0c；只在 /tmp 的副本里跑）。
/// 目的：尝试推翻 W1-a 的两条 P1 修复，或指出其未覆盖的边界。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/api/ws_liveness.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/chat/turn_liveness.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/state/ui_state_tracker.dart';

void main() {
  group('探针 A · isChannelTrustworthy 的空白格：CONNECTING + offline', () {
    test('CONNECTING + offline → 判为「不可信」（=> ensureConnected 会摘掉还在建连的 socket）',
        () {
      // 现有测试矩阵覆盖 CONNECTING+online，**没有** CONNECTING+offline。
      expect(
        isChannelTrustworthy(
          readyState: kSocketConnecting,
          liveness: HeartbeatLiveness.offline,
        ),
        isFalse,
      );
    });

    test('对照：OPEN + suspect 仍判可信（重连交给看门狗）', () {
      expect(
        isChannelTrustworthy(
          readyState: kSocketOpen,
          liveness: HeartbeatLiveness.suspect,
        ),
        isTrue,
      );
    });
  });

  group('探针 B · 半开时「显示层退回」不等于「在飞轮次被释放」', () {
    test('liveness=offline 只改 wsConnected；turnActive 仍为真，且收口判据读的是 WsStatus',
        () {
      final DateTime base = DateTime.utc(2026, 10, 1, 20);
      DateTime now = base;
      final UiStateTracker tracker = UiStateTracker(clock: () => now);
      addTearDown(tracker.dispose);

      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      now = now.add(kWsHeartbeatOfflineAfter);

      // 显示层：相位确实退回 offline（这就是 F-0008-1 想要的观感）
      expect(tracker.phase, UiPhase.offline);
      expect(tracker.signals.wsConnected, isFalse);
      // 但轮次标志**没有**被 liveness 清掉：
      expect(tracker.signals.turnActive, isTrue, reason: 'liveness 不改轮次标志');
      // 且 ChatController 的收口判据只看 WsStatus：
      expect(
        mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.connected),
        isFalse,
        reason: '半开而 WsStatus 仍是 connected ⇒ 收口判据不成立',
      );
      // ⇒ 结论：真正解开「在飞轮次 / _streaming」的是 **WsClient 看门狗把状态置 disconnected**，
      //    liveness→wsConnected 只是显示层兜底。
      expect(
        mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.disconnected),
        isTrue,
      );
    });
  });

  group('探针 C · markTurnFailed 会一并清掉外部（语音/弹幕）轮次的标志', () {
    test('turnActive + voiceActive 同时被清（实施者自报残余竞态，实测成立）', () {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);

      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(const RuntimeStatusEvent(event: 'voice_started'));
      tracker.markTurnAccepted();
      expect(tracker.signals.voiceActive, isTrue);
      expect(tracker.signals.turnActive, isTrue);

      tracker.markTurnFailed(); // 一次「本地发送失败」

      expect(tracker.signals.turnActive, isFalse);
      expect(
        tracker.signals.voiceActive,
        isFalse,
        reason: '外部轮次（语音）的 voice 标志被本地失败的收口顺带清掉',
      );
      expect(tracker.phase, UiPhase.idle);
    });
  });

  group('探针 D · markTurnAccepted 之后立刻 markTurnFailed 的净效果', () {
    test('相位回到 idle（用户消息已清空输入框，但 send() 早退时什么都没发）', () {
      final UiStateTracker tracker = UiStateTracker();
      addTearDown(tracker.dispose);
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);
      tracker.markTurnFailed();
      expect(tracker.phase, UiPhase.idle);
    });
  });
}
