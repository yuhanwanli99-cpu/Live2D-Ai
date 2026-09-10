import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_dispatch.dart';
import 'package:live2d_ai_shell/actions/action_event.dart';
import 'package:live2d_ai_shell/api/action_source.dart';

/// 可控时钟（幂等窗口是时间相关的逻辑，必须能确定性地推进）。
class _Clock {
  DateTime now = DateTime(2026, 9, 10, 21, 0, 0);
  void advance(Duration d) => now = now.add(d);
  DateTime call() => now;
}

ActionEvent perform({
  required String action,
  int? strength,
  ActionSource source = ActionSource.userCommand,
}) => ActionEvent(
  kind: ActionEventKind.perform,
  epoch: 0,
  action: action,
  strength: strength,
  source: source,
);

ActionEvent cease() =>
    const ActionEvent(kind: ActionEventKind.cease, epoch: 0);

void main() {
  group('重复点击：只能本地判（服务端响应看不出 AlreadyActive）', () {
    test('首次点击不算重复', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      expect(d.recordInvoke('nod', 2), isFalse);
      expect(d.lastInvoke!.action, 'nod');
      expect(d.lastInvoke!.strength, 2);
    });

    test('窗口内「同动作 + 同强度」算重复', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      d.recordInvoke('nod', 2);
      clock.advance(const Duration(milliseconds: 500));
      expect(d.recordInvoke('nod', 2), isTrue);
    });

    test('窗口外的同动作不算重复（动作多半已经演完）', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      d.recordInvoke('nod', 2);
      clock.advance(ActionDispatch.kDuplicateWindow + const Duration(seconds: 1));
      expect(d.recordInvoke('nod', 2), isFalse);
    });

    test('**强度不同不算重复**（core 的幂等键含强度）', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      d.recordInvoke('nod', 2);
      expect(d.recordInvoke('nod', 3), isFalse);
      expect(d.recordInvoke('nod', null), isFalse);
    });

    test('动作不同不算重复', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      d.recordInvoke('nod', 2);
      expect(d.recordInvoke('shake_no', 2), isFalse);
    });

    test('窗口边界：恰好等于窗口时不算重复（严格小于）', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      d.recordInvoke('nod', 2);
      clock.advance(ActionDispatch.kDuplicateWindow);
      expect(d.recordInvoke('nod', 2), isFalse);
    });

    test('窗口是个具体值（改动要显式改，不能飘）', () {
      expect(ActionDispatch.kDuplicateWindow, const Duration(seconds: 2));
    });

    test('重复提示要解释原因（否则就是「点了没反应」）', () {
      final ActionDispatch d = ActionDispatch();
      expect(d.duplicateHint, contains('正在播放'));
      expect(d.duplicateHint, contains('不会重播'));
    });
  });

  group('「正在演」的权威来源是 WS，不是 HTTP 响应', () {
    test('perform → 忙碌；cease → 复位', () {
      final ActionDispatch d = ActionDispatch();
      expect(d.performing, isNull);

      expect(d.consume(perform(action: 'nod', strength: 2)), isTrue);
      expect(d.performing, 'nod');
      expect(d.performingStrength, 2);
      expect(d.isBusy('nod'), isTrue);
      expect(d.isBusy('shake_no'), isFalse);

      expect(d.consume(cease()), isTrue);
      expect(d.performing, isNull);
      expect(d.isBusy('nod'), isFalse);
    });

    test('区分「用户点的」与「AI 触发的」（决定按钮是否进忙碌态）', () {
      final ActionDispatch d = ActionDispatch();
      d.consume(perform(action: 'nod', source: ActionSource.userCommand));
      expect(d.performingIsUser, isTrue);

      d.consume(perform(action: 'tilt', source: ActionSource.llmTool));
      expect(
        d.performingIsUser,
        isFalse,
        reason: 'AI 触发的动作不该让用户点的按钮显示「正在演」',
      );
    });

    test('**抢占**：新 perform 直接顶掉旧的（core 不会先发 cease）', () {
      final ActionDispatch d = ActionDispatch();
      d.consume(perform(action: 'nod'));
      d.consume(perform(action: 'shake_no'));
      expect(d.performing, 'shake_no');
      expect(d.isBusy('nod'), isFalse);
    });

    test('没有在演时的 cease 是 no-op（不通知）', () {
      final ActionDispatch d = ActionDispatch();
      int notified = 0;
      d.addListener(() => notified++);
      expect(d.consume(cease()), isFalse);
      expect(notified, 0);
    });

    test('unknown kind / 无动作名的 perform 不消费', () {
      final ActionDispatch d = ActionDispatch();
      expect(
        d.consume(
          const ActionEvent(kind: ActionEventKind.unknown, epoch: 0),
        ),
        isFalse,
      );
      expect(
        d.consume(
          ActionEvent(
            kind: ActionEventKind.perform,
            epoch: 0,
            source: ActionSource.llmTool,
          ),
        ),
        isFalse,
      );
      expect(d.performing, isNull);
    });
  });

  group('错误提示', () {
    test('recordError 写入并在下次 invoke 时清掉', () {
      final ActionDispatch d = ActionDispatch();
      d.recordError('invalid_params：strength 必须在 1..=3');
      expect(d.lastError, contains('invalid_params'));

      d.recordInvoke('nod', 2);
      expect(d.lastError, isNull, reason: '新的一次尝试该把旧错误收掉');
    });

    test('clearError 幂等（没错误时不通知）', () {
      final ActionDispatch d = ActionDispatch();
      int notified = 0;
      d.addListener(() => notified++);
      d.clearError();
      expect(notified, 0);

      d.recordError('x');
      d.clearError();
      expect(d.lastError, isNull);
      expect(notified, 2);
    });
  });

  group('通知语义（原则 P6：高频信号不该拖垮界面）', () {
    test('recordInvoke 通知一次；重复判定不改状态也通知（要弹提示）', () {
      final _Clock clock = _Clock();
      final ActionDispatch d = ActionDispatch(now: clock.call);
      int notified = 0;
      d.addListener(() => notified++);
      d.recordInvoke('nod', 2);
      expect(notified, 1);
      d.recordInvoke('nod', 2);
      expect(notified, 2);
    });

    test('consume 的每个有效事件通知一次', () {
      final ActionDispatch d = ActionDispatch();
      int notified = 0;
      d.addListener(() => notified++);
      d.consume(perform(action: 'nod'));
      expect(notified, 1);
      d.consume(cease());
      expect(notified, 2);
    });
  });

  _localExpiryTests();
}

/// 可控定时器

/// 可控定时器（本地兜底过期是时间相关的，必须能确定性地触发）。
class _Timers {
  final List<({Duration delay, void Function() fire})> pending =
      <({Duration delay, void Function() fire})>[];
  int cancelled = 0;

  Timer call(Duration delay, void Function() cb) {
    pending.add((delay: delay, fire: cb));
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

void _localExpiryTests() {
  group('本地兜底过期：**服务端不发 cease**，只能前端兜住', () {
    test('时长表与渲染面的 choreography_total_ms 对齐（含出处）', () {
      // 这些数字来自 l2d-wasm-demo/src/web/surface.rs:1590-1597 的断言。
      expect(kActionDurationMs['nod'], 1320);
      expect(kActionDurationMs['shake_no'], 1000);
      expect(kActionDurationMs['tilt'], 1500);
      expect(kActionDurationMs['look_around'], 1920);
      expect(kActionDurationMs['listen'], 2180);
      expect(kActionDurationMs['surprise'], 1300);
      expect(kUnknownActionDurationMs, 1500);
    });

    test('兜底时长 = 动作时长 + 小余量（余量不能太大）', () {
      expect(
        actionDurationFor('nod'),
        const Duration(milliseconds: 1320) + kActionExpirySlack,
      );
      expect(actionDurationFor('没见过的动作').inMilliseconds, 1750);
      expect(
        kActionExpirySlack.inMilliseconds,
        lessThanOrEqualTo(300),
        reason: '余量太大 = 动作演完了按钮还写着「正在演」',
      );
    });

    test('perform 时装上到期；到期后自动复位并标记为「本地结束」', () {
      final _Timers timers = _Timers();
      final ActionDispatch d = ActionDispatch(timerFactory: timers.call);
      d.consume(perform(action: 'nod'));
      expect(d.isBusy('nod'), isTrue);
      expect(timers.pending, hasLength(1));
      expect(timers.pending.single.delay, actionDurationFor('nod'));

      timers.fireAll();
      expect(d.isBusy('nod'), isFalse);
      expect(d.expiredLocally, isTrue, reason: '这次结束是本地兜底的，不是服务端收口');
    });

    test('真正的 cease 立刻生效，并撤销本地到期（**cease 是权威**）', () {
      final _Timers timers = _Timers();
      final ActionDispatch d = ActionDispatch(timerFactory: timers.call);
      d.consume(perform(action: 'nod'));
      d.consume(cease());
      expect(d.isBusy('nod'), isFalse);
      expect(d.expiredLocally, isFalse, reason: '服务端收口的');
      expect(timers.cancelled, greaterThanOrEqualTo(1));
    });

    test('抢占时旧到期被撤销，只留新的一个', () {
      final _Timers timers = _Timers();
      final ActionDispatch d = ActionDispatch(timerFactory: timers.call);
      d.consume(perform(action: 'nod'));
      d.consume(perform(action: 'shake_no'));
      expect(timers.pending, hasLength(2));
      expect(timers.cancelled, greaterThanOrEqualTo(1));
      expect(d.isBusy('shake_no'), isTrue);
      // 旧的那个到期回调即使被触发也不该清掉新的（`_performing` 已经不是它）。
      timers.pending.first.fire();
      expect(d.isBusy('shake_no'), isTrue);
    });

    test('关掉 useLocalExpiry → 纯 WS 驱动（不装定时器）', () {
      final _Timers timers = _Timers();
      final ActionDispatch d = ActionDispatch(
        timerFactory: timers.call,
        useLocalExpiry: false,
      );
      d.consume(perform(action: 'nod'));
      expect(timers.pending, isEmpty);
      expect(d.isBusy('nod'), isTrue);
    });

    test('dispose 撤销到期（不留悬空定时器）', () {
      final _Timers timers = _Timers();
      final ActionDispatch d = ActionDispatch(timerFactory: timers.call);
      d.consume(perform(action: 'nod'));
      d.dispose();
      expect(timers.cancelled, greaterThanOrEqualTo(1));
    });
  });
}
