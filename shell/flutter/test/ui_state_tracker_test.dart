import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/state/ui_state_tracker.dart';

/// 可控定时器：记录「打断保持期」的到期回调，由测试手动触发。
class _ManualTimer {
  final List<({Duration delay, void Function() fire})> pending =
      <({Duration delay, void Function() fire})>[];
  int cancelled = 0;

  Timer call(Duration delay, void Function() callback) {
    pending.add((delay: delay, fire: callback));
    return _FakeTimer(() => cancelled++);
  }

  void fireAll() {
    for (final ({Duration delay, void Function() fire}) t in pending) {
      t.fire();
    }
  }
}

class _FakeTimer implements Timer {
  _FakeTimer(this._cancel);
  final void Function() _cancel;

  @override
  bool get isActive => true;
  @override
  int get tick => 0;
  @override
  void cancel() => _cancel();
}

WsEvent frame(String raw) => parseWsFrame(raw)!;

void main() {
  group('连接态 → offline（唯一的 hard gate）', () {
    test('初始未连接 → offline', () {
      final UiStateTracker tracker = UiStateTracker();
      expect(tracker.phase, UiPhase.offline);
    });

    test('connecting **不算**可用（不能显示成已连接）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connecting);
      expect(tracker.phase, UiPhase.offline);
      expect(tracker.signals.wsConnected, isFalse);
    });

    test('connected → idle', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      expect(tracker.phase, UiPhase.idle);
    });

    test('断开时清掉进行中的信号（否则重连后停在「说话中」）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_started"}}'));
      expect(tracker.phase, UiPhase.speaking);

      tracker.onWsStatus(WsStatus.disconnected);
      expect(tracker.voiceActive, isFalse);
      expect(tracker.turnActive, isFalse);
      expect(tracker.phase, UiPhase.offline);
    });

    test('状态没变时不通知（避免无意义重建）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      int notified = 0;
      tracker.addListener(() => notified++);
      tracker.onWsStatus(WsStatus.connected);
      expect(notified, 0);
    });
  });

  group('渲染面失败**不再**冒充「后端未连接」（2026-09-11 修）', () {
    // 过去这里有一条 `bridgeError → offline`，两个毛病：
    //   ① 渲染面（iframe）出错时后端明明连着，胶囊却写「后端未连接」，
    //      而旁边的连接徽标写「已连接」—— 同一个界面两个相反的结论；
    //   ② 那个信号只置位、永不清除（`onBridgePhase(false)` 全仓库无人调用），
    //      于是**任何一次瞬时**的渲染面错误都会把胶囊**永久**钉住。
    // 现在渲染面的失败只出现在**舞台自己的**「加载失败 + 重试」覆盖层上。
    test('WS 连着时，相位**只**由轮次信号决定（与渲染面无关）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      expect(tracker.phase, UiPhase.idle);
      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);
    });

    test('`offline` 的唯一判据是「WS 不可用」', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      expect(tracker.phase, isNot(UiPhase.offline));
      tracker.onWsStatus(WsStatus.disconnected);
      expect(tracker.phase, UiPhase.offline);
      tracker.onWsStatus(WsStatus.connected);
      expect(
        tracker.phase,
        isNot(UiPhase.offline),
        reason: '重连之后必须能回到正常态（旧实现把这一条钉死了）',
      );
    });
  });

  group('一轮的生命周期：thinking → speaking → 收口', () {
    test('受理 → thinking', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);
    });

    test('voice_started → speaking（即使 turnActive 也在）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_started"}}'));
      expect(tracker.phase, UiPhase.speaking);
    });

    test('voice_ended → 不会退回 thinking（本轮还没收口）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_started"}}'));
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_ended"}}'));
      expect(tracker.voiceActive, isFalse);
      expect(tracker.phase, UiPhase.thinking);
    });

    test('turn_state completed → 回落 idle', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"turn_state","data":{"epoch":0,"status":"completed"}}'));
      expect(tracker.phase, UiPhase.idle);
      expect(tracker.errorActive, isFalse);
    });

    test('turn_state failed → error', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"turn_state","data":{"epoch":0,"status":"failed"}}'));
      expect(tracker.phase, UiPhase.error);
      expect(tracker.errorMessage, isNotNull);
    });

    test('text_delta 的 completed 锚点也收口', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"text_delta","data":{"epoch":0,"completed":true}}'));
      expect(tracker.phase, UiPhase.idle);
    });

    test('text_delta 只有正文（没有 completed）→ 不动状态', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      final bool handled = tracker.consume(
        frame('{"type":"text_delta","data":{"epoch":0,"text":"你好","ts_ms":1}}'),
      );
      expect(handled, isFalse);
      expect(tracker.phase, UiPhase.thinking);
    });

    test('新一轮受理会把上一轮的错误收掉', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"turn_state","data":{"epoch":0,"status":"failed"}}'));
      expect(tracker.phase, UiPhase.error);
      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);
      expect(tracker.errorMessage, isNull);
    });
  });

  group('打断（new_epoch）：有保持期的瞬时状态', () {
    test('new_epoch → interrupted，即使 voiceActive 还残留', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_started"}}'));

      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch","epoch":1}}'));
      expect(tracker.phase, UiPhase.interrupted);
      expect(tracker.voiceActive, isFalse);
      expect(timer.pending, hasLength(1));
      expect(timer.pending.single.delay, kInterruptedHold);
    });

    test('保持期到期 → 回落 idle', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch"}}'));
      expect(tracker.phase, UiPhase.interrupted);

      timer.fireAll();
      expect(tracker.phase, UiPhase.idle);
      expect(tracker.interrupted, isFalse);
    });

    test('连续 new_epoch 会取消旧定时器（不会提前回落）', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch"}}'));
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch"}}'));
      expect(timer.cancelled, 1, reason: '旧定时器必须被取消');
      expect(timer.pending, hasLength(2));
    });

    test('断开时取消打断定时器并清掉打断态', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch"}}'));
      tracker.onWsStatus(WsStatus.disconnected);
      expect(tracker.interrupted, isFalse);
      expect(timer.cancelled, greaterThanOrEqualTo(1));
    });

    test('dispose 后到期回调不再通知（不留悬空定时器）', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"new_epoch"}}'));
      tracker.dispose();
      // 手动触发已经排队的到期回调：不该抛。
      // `Timer.cancel()` 挡不住**已排队**的回调，而在已 dispose 的
      // `ChangeNotifier` 上 notify 会断言失败——所以回调里有 `_disposed` 守卫。
      expect(timer.fireAll, returnsNormally);
    });
  });

  group('error 帧', () {
    test('error 帧 → error 相位 + 文案', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(
        frame('{"type":"error","data":{"code":"llm_upstream","message":"上游 401"}}'),
      );
      expect(tracker.phase, UiPhase.error);
      // 2026-09-11：文案必须带码（`code：message`），因为**这条**路径才是
      // `main.dart` 里上屏的那一条（`_ui.errorMessage ?? _chat.error`）。
      expect(tracker.errorMessage, 'llm_upstream：上游 401');
      expect(tracker.errorCode, 'llm_upstream');
    });

    test('error 帧缺 message → 只剩 code（不留空冒号）', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"error","data":{"code":"llm_upstream"}}'));
      expect(tracker.errorMessage, 'llm_upstream');
      expect(tracker.errorCode, 'llm_upstream');
    });

    // 回归：曾出现「后端把码发来了，界面上还是只有散文」——因为本类把 hint
    // 和 code 都丢了，而它优先级更高。
    test('error 帧的 hint 与 stage 不丢', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(
        frame(
          '{"type":"error","data":{"code":"llm_upstream_401","stage":"llm",'
          '"message":"上游非成功状态 401","hint":"确认 api_key_env 已设置"}}',
        ),
      );
      expect(tracker.errorMessage, 'llm_upstream_401：上游非成功状态 401（确认 api_key_env 已设置）');
      expect(tracker.errorCode, 'llm_upstream_401');
    });

    test('turn_state failed 且无 error 帧 → 明说没有详情并给出去处', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"turn_state","data":{"status":"failed"}}'));
      expect(tracker.errorMessage, contains('未给出错误详情'));
      expect(tracker.errorMessage, contains('诊断日志'));
      expect(tracker.errorCode, 'turn_failed_no_detail');
    });

    test('用户关掉错误 → 回落', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.consume(frame('{"type":"turn_state","data":{"status":"failed"}}'));
      tracker.clearError();
      expect(tracker.phase, UiPhase.idle);
      expect(tracker.errorMessage, isNull);
      expect(tracker.errorCode, isNull);
    });

    test('没有错误时 clearError 不通知', () {
      final UiStateTracker tracker = UiStateTracker();
      int notified = 0;
      tracker.addListener(() => notified++);
      tracker.clearError();
      expect(notified, 0);
    });
  });

  group('不相关帧一律不消费、不通知', () {
    test('audio / heartbeat / subscribe_ack / 已移除的 action_state / 未知帧', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      int notified = 0;
      tracker.addListener(() => notified++);

      final List<String> irrelevant = <String>[
        '{"type":"heartbeat","seq":9}',
        '{"type":"subscribe_ack","data":{"topics":[]}}',
        '{"type":"audio","data":{"audio":"AAAA","sample_rate":24000}}',
        // 2026-09-11：动作子系统移除后 `action_state` 不再有事件类，
        // 走「未知帧」分支。旧服务端 / 迟到帧同样必须被安全忽略。
        '{"type":"action_state","data":{"kind":"perform",'
            '"action":{"action":"nod","source":"llm_tool","strength":2}}}',
        '{"type":"brand_new","data":{}}',
      ];
      for (final String raw in irrelevant) {
        expect(tracker.consume(frame(raw)), isFalse, reason: raw);
      }
      expect(notified, 0);
      expect(tracker.phase, UiPhase.idle);
    });

    test('runtime_status 的未知 event 不消费', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      expect(
        tracker.consume(
          frame('{"type":"runtime_status","data":{"event":"shutdown_ready"}}'),
        ),
        isFalse,
      );
    });
  });

  group('真实抓包帧走一遍（与 ws_frame_test 同源）', () {
    test('一次真实成功轮：thinking → speaking → completed → idle', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);

      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking);

      // 实抓：{"data":{"epoch":0,"text":"好呀，那我就说两句话啦。\n\n","ts_ms":8590},…}
      tracker.consume(
        frame(
          '{"data":{"epoch":0,"text":"好呀","ts_ms":8590},"seq":364,'
          '"type":"text_delta"}',
        ),
      );
      expect(tracker.phase, UiPhase.thinking, reason: '只有正文时仍是思考中');

      tracker.consume(
        frame('{"data":{"epoch":0,"status":"completed"},"seq":564,'
            '"ts":"2026-09-10T12:14:53.166Z","type":"turn_state"}'),
      );
      expect(tracker.phase, UiPhase.idle);
    });

    test('一次真实失败轮（上游 401）→ error', () {
      final UiStateTracker tracker = UiStateTracker();
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(
        frame('{"data":{"epoch":0,"status":"failed"},"seq":2,'
            '"ts":"2026-09-10T12:13:43.310Z","type":"turn_state"}'),
      );
      expect(tracker.phase, UiPhase.error);
    });
  });

  _markStoppedTests();
}

void _markStoppedTests() {
  group('markStopped：本地立刻收口（不等服务端）', () {
    test('点停止 → interrupted，且清掉进行中的信号', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.consume(frame('{"type":"runtime_status","data":{"event":"voice_started"}}'));
      expect(tracker.phase, UiPhase.speaking);

      tracker.markStopped();
      expect(tracker.phase, UiPhase.interrupted);
      expect(tracker.turnActive, isFalse);
      expect(tracker.voiceActive, isFalse);
    });

    test('停止后到期 → 回落 idle', () {
      final _ManualTimer timer = _ManualTimer();
      final UiStateTracker tracker = UiStateTracker(timerFactory: timer.call);
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      tracker.markStopped();
      timer.fireAll();
      expect(tracker.phase, UiPhase.idle);
    });
  });
}
