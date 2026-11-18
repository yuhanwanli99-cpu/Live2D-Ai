/// 轮次守卫生的单测（2026-11-18 审计 #16 / #17 / #18）。
///
/// 守四件事：
/// 1. **轮次身份**：帧只有代次对得上（或任一侧未知）才收；正常收口**不**退休
///    代次（同 epoch 的下一轮还要能收自己的帧）；
/// 2. **作废吞帧**：本地切走之后，旧轮的迟到帧（含正文兜底）一律静默丢弃，
///    绝不能因为 `_assistant == null` 就在新会话里新建气泡；
/// 3. **待停闸门**：stop 回执与代次都到了才解除（#17 的「上一段会话还在回答」）；
/// 4. **新代次优先**：停完之后服务端推进到新一代 -> 旧代次退休、新代次照收。
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/chat/turn_guard.dart';

void main() {
  group('轮次身份：token + epoch', () {
    test('start 开一轮：inFlight、会话、epoch 都记下', () {
      final TurnGuard g = TurnGuard();
      expect(g.inFlight, isFalse);
      expect(g.activeSession, isNull);

      final int token = g.start(sessionId: 'a', epoch: 3);
      expect(token, greaterThan(0));
      expect(g.inFlight, isTrue);
      expect(g.activeSession, 'a');
      expect(g.activeEpoch, 3);
    });

    test('当前轮的帧：epoch 相同 / 任一侧未知 都收；不同则丢', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 7);
      expect(g.verdictFor(7), FrameVerdict.accept);
      expect(g.verdictFor(null), FrameVerdict.accept, reason: '旧服务端帧没有 epoch');
      expect(g.verdictFor(8), FrameVerdict.dropStale);
    });

    test('当前轮还不知道代次时，第一帧带上来的 epoch 补记（旧服务端兼容）', () {
      final TurnGuard g = TurnGuard();
      g.start();
      expect(g.verdictFor(5), FrameVerdict.accept);
      expect(g.activeEpoch, 5);
      expect(g.verdictFor(6), FrameVerdict.dropStale);
    });

    test('noteEpoch 只认自己的 token：旧 token 的响应不许改新一轮', () {
      final TurnGuard g = TurnGuard();
      final int first = g.start();
      g.settle();
      final int second = g.start();
      g.noteEpoch(first, 9);
      expect(g.activeEpoch, isNull, reason: '旧 token 的受理响应属于上一轮');
      g.noteEpoch(second, 10);
      expect(g.activeEpoch, 10);
    });

    test('正常收口不退休代次：同一 epoch 的下一轮照收', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 4);
      g.settle();
      expect(g.verdictFor(4), FrameVerdict.accept, reason: '正常完成不推进 epoch');
      expect(g.retiredEpochs, isEmpty);
    });
  });

  group('本地作废（#16）：旧轮迟到帧一律静默丢弃', () {
    test('abandon：从进行中移到待停；属于旧轮的帧被丢', () {
      final TurnGuard g = TurnGuard();
      g.start(sessionId: 'a', epoch: 2);

      expect(g.abandon(), isTrue);
      expect(g.inFlight, isFalse);
      expect(g.pendingStop, isTrue);
      expect(g.abandonedSession, 'a');
      expect(
        g.verdictFor(2),
        FrameVerdict.dropAbandoned,
        reason: '这就是「切走之后 delta 落进新会话」那条缺陷的拦截点',
      );
      expect(g.verdictFor(null), FrameVerdict.dropAbandoned);
    });

    test('空闲时 abandon 是 no-op（切会话不该凭空作废一轮）', () {
      final TurnGuard g = TurnGuard();
      expect(g.abandon(), isFalse);
      expect(g.pendingStop, isFalse);
    });

    test('回执 + 代次都到了才解除待停；旧代次之后仍被丢', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 2);
      g.abandon();

      g.onStopAcknowledged();
      expect(g.pendingStop, isFalse);
      expect(g.retiredEpochs, contains(2));
      // 空闲态：旧代次的迟到帧照样丢（stop 推进 epoch，旧代次永不复用）。
      expect(g.verdictFor(2), FrameVerdict.dropStale);
      // 新代次 / 外部注入照收（服务端主动开口要能开新一轮）。
      expect(g.verdictFor(3), FrameVerdict.accept);
    });

    test('代次还不知道时先等：stop 回执先到也不解除，受理响应补 epoch 后才解除', () {
      final TurnGuard g = TurnGuard();
      final int token = g.start();
      g.abandon();
      g.onStopAcknowledged();
      expect(g.pendingStop, isTrue, reason: '没有 epoch 就分不清迟到帧，不许放行输入');

      g.noteEpoch(token, 6);
      expect(g.pendingStop, isFalse);
      expect(g.retiredEpochs, contains(6));
      expect(g.verdictFor(6), FrameVerdict.dropStale);
    });

    test('受理响应先到、stop 回执后到：同样解除', () {
      final TurnGuard g = TurnGuard();
      final int token = g.start();
      g.abandon();
      g.noteEpoch(token, 6);
      expect(g.pendingStop, isTrue);
      g.onStopAcknowledged();
      expect(g.pendingStop, isFalse);
      expect(g.retiredEpochs, contains(6));
    });

    test('作废期间出现**新代次**的帧 = 服务端已推进：收下并解除作废态', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 2);
      g.abandon();

      expect(g.verdictFor(3), FrameVerdict.accept);
      expect(g.pendingStop, isFalse, reason: '新一代开始 = 旧轮不可能还活着');
      expect(g.retiredEpochs, contains(2));
    });

    test('作废期间出现**更旧代次**的帧：同样丢（epoch 只增不减）', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 5);
      g.abandon();
      expect(g.verdictFor(4), FrameVerdict.dropAbandoned);
      expect(g.pendingStop, isTrue, reason: '更旧的帧不能当成新一代来解除闸门');
    });

    test('本地就失败的轮：dropAbandonedTurn 立刻解除，不许一直挡输入', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 2);
      g.abandon();
      // 模拟「用户已经切走、HTTP 才回来说没受理」：服务端根本没开始，
      // 也不会有 stop 回执——作废态必须能自己解除。
      g.dropAbandonedTurn();
      expect(g.pendingStop, isFalse);
      expect(g.verdictFor(2), FrameVerdict.accept, reason: '没有作废过的东西不设防');
    });

    test('新的本地轮 start 会让作废态让位', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 2);
      g.abandon();
      g.start(sessionId: 'b', epoch: 9);
      expect(g.pendingStop, isFalse);
      expect(g.verdictFor(9), FrameVerdict.accept);
    });
  });

  group('收口帧（turn_state / completed / 致命 error 的身份判据）', () {
    test('当前轮匹配才收口；代次不符不许收掉新一轮', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 3);
      expect(g.completesCurrentTurn(3), isTrue);
      expect(g.completesCurrentTurn(4), isFalse);
    });

    test('空闲 / 作废态一律 false：收口帧不许在别的会话里补提示', () {
      final TurnGuard g = TurnGuard();
      expect(g.completesCurrentTurn(1), isFalse);
      g.start(epoch: 2);
      g.abandon();
      expect(g.completesCurrentTurn(2), isFalse);
      // 作废轮自己的收口帧：只退休那一代，让之后迟到的兜底正文继续被丢。
      expect(g.retiredEpochs, contains(2));
      expect(g.verdictFor(2), FrameVerdict.dropStale);
    });

    test('belongsToCurrentTurn：空闲 false；未知 epoch 按「属于」处理', () {
      final TurnGuard g = TurnGuard();
      expect(g.belongsToCurrentTurn(1), isFalse);
      g.start();
      expect(g.belongsToCurrentTurn(null), isTrue);
      expect(g.belongsToCurrentTurn(1), isTrue);
      expect(g.activeEpoch, 1, reason: '顺手补记代次');
    });
  });

  group('new_epoch / reset', () {
    test('onNewEpoch：当前代退休并收口，之后它的迟到帧全丢', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 5);
      g.onNewEpoch();
      expect(g.inFlight, isFalse);
      expect(g.retiredEpochs, contains(5));
      expect(g.verdictFor(5), FrameVerdict.dropStale);
    });

    test('retireActiveEpoch 必须在收口之前调（收口会丢掉代次）', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 5);
      g.retireActiveEpoch();
      g.settle();
      expect(g.retiredEpochs, contains(5));
      expect(g.verdictFor(5), FrameVerdict.dropStale);
    });

    test('reset：通道断了，所有身份失效（含待停闸门，必须有出口）', () {
      final TurnGuard g = TurnGuard();
      g.start(epoch: 5);
      g.abandon();
      g.reset();
      expect(g.inFlight, isFalse);
      expect(g.pendingStop, isFalse);
      expect(g.retiredEpochs, isEmpty);
    });
  });

  group('文案与码（#17）', () {
    test('一行原因说清「不能发」与「下一步」', () {
      expect(kTurnElsewhereMessage, '上一段会话还在回答，先停止它');
      expect(kTurnElsewhereCode, isNotEmpty);
      expect(kTurnElsewhereCode, isNot(equals(kTurnElsewhereMessage)));
    });
  });

  group('助手气泡落点（#21：先有 user_line、气泡后建）', () {
    test('守卫已记过这一轮：气泡写回它记的会话，**不再** start', () {
      final ({String? sessionId, bool startTurn}) target = assistantBubbleTarget(
        turnInFlight: true,
        turnSessionId: 'A',
        activeSessionId: 'B',
      );
      expect(target.sessionId, 'A', reason: '帧属于 A、人看着 B 时回复必须进 A');
      expect(
        target.startTurn,
        isFalse,
        reason: '再 start 一次会把属主覆盖成当前活动会话 B（这正是串台根因）',
      );
    });

    test('守卫在飞但没记会话（旧服务端不带 id）：回落活动会话，仍不重开一轮', () {
      final ({String? sessionId, bool startTurn}) target = assistantBubbleTarget(
        turnInFlight: true,
        turnSessionId: null,
        activeSessionId: 'B',
      );
      expect(target.sessionId, 'B');
      expect(target.startTurn, isFalse);
    });

    test('空闲：气泡落在活动会话并开一轮', () {
      final ({String? sessionId, bool startTurn}) target = assistantBubbleTarget(
        turnInFlight: false,
        turnSessionId: null,
        activeSessionId: 'B',
      );
      expect(target.sessionId, 'B');
      expect(target.startTurn, isTrue);
    });

    test('还没有会话（首条注入）：sessionId 为 null，交给 store ensureActive', () {
      final ({String? sessionId, bool startTurn}) target = assistantBubbleTarget(
        turnInFlight: false,
        turnSessionId: null,
        activeSessionId: null,
      );
      expect(target.sessionId, isNull);
      expect(target.startTurn, isTrue);
    });
  });
}
