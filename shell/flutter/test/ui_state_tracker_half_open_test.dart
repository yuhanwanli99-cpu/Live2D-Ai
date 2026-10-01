/// 心跳在**状态层**的消费者回归（真 `UiStateTracker`，VM 直接跑）。
///
/// 缺陷本身（2026-10-01，审计 F-0008-1）：服务端每 10 s 发一条心跳，前端解析了、
/// 下发了，然后被丢掉——`ui_state_tracker.dart` 对 `HeartbeatEvent` 曾直接
/// `return false`。于是**没有任何时间判据**能区分这两种情况：
///
/// - 「通道好、这一轮正在思考」→ 应该显示「思考中」；
/// - 「半开连接：`WsStatus` 仍是 `connected`、实际一帧都收不到」→ 应该退回
///   「后端未连接」，绝不能让界面**永久**停在「思考中」。
///
/// 本文件的断言都必须能在修复前**红**（那时心跳被丢、`channelLiveness` 不存在），
/// 修复后**绿**——见回报里的三段原始输出。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/api/ws_liveness.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/state/ui_state_tracker.dart';

/// 服务端心跳帧的**真形状**（与 `ws_frame_test.dart`、后端
/// `web_api/ws/connection.rs` 的样例一致）——不自己编帧，避免测出一条
/// 现实中不存在的输入。
const String kHeartbeatFrame =
    '{"type":"heartbeat","seq":9,"ts":"2026-09-10T12:15:00.000Z"}';

void main() {
  late DateTime now;
  late UiStateTracker tracker;

  setUp(() {
    // 固定基准时钟：半开判据是**时间**判据，不注入时钟就只能靠真实时间，
    // 那条测试要么睡 90 秒、要么根本没断言力。
    now = DateTime.utc(2026, 10, 1, 20, 0, 0);
    tracker = UiStateTracker(clock: () => now);
  });

  tearDown(() => tracker.dispose());

  group('半开连接：心跳被消费 ⇒ 相位真的退回 offline', () {
    test('**幽灵态端到端**：受理一轮后通道彻底静默 → 不再永久停在「思考中」', () {
      tracker.onWsStatus(WsStatus.connected);
      tracker.markTurnAccepted();
      expect(tracker.phase, UiPhase.thinking, reason: '前提：受理后相位进入思考中');

      // 半开连接：一帧都没有，而 `WsStatus` **仍然是 connected**
      //（浏览器只在「尝试收发并失败」时才改 readyState，本客户端从不发帧）。
      now = now.add(kWsHeartbeatOfflineAfter);

      expect(
        tracker.wsStatus,
        WsStatus.connected,
        reason: '前提：状态机自己以为一切正常——这正是半开连接的坏法',
      );
      expect(tracker.channelLiveness, HeartbeatLiveness.offline);
      expect(tracker.signals.wsConnected, isFalse);
      expect(
        tracker.phase,
        UiPhase.offline,
        reason: '修复前这里恒为 thinking：没有任何判据能看出通道已经死了',
      );
    });

    test('收到心跳 → 三态随时间依次 online / suspect / offline', () {
      tracker.onWsStatus(WsStatus.connected);
      final WsEvent? frame = parseWsFrame(kHeartbeatFrame);
      expect(frame, isA<HeartbeatEvent>(), reason: '前提：这帧确实被解析成心跳');
      tracker.consume(frame!);
      expect(tracker.channelLiveness, HeartbeatLiveness.online);

      now = now.add(kWsHeartbeatSuspectAfter);
      expect(tracker.channelLiveness, HeartbeatLiveness.suspect);

      now = now.add(kWsHeartbeatOfflineAfter - kWsHeartbeatSuspectAfter);
      expect(tracker.channelLiveness, HeartbeatLiveness.offline);
    });

    test('只到**嫌疑档**（≈30s）时相位仍是 idle：不把「可疑」说成「后端未连接」', () {
      tracker.onWsStatus(WsStatus.connected);
      expect(tracker.channelLiveness, HeartbeatLiveness.online);

      now = now.add(kWsHeartbeatSuspectAfter);

      expect(tracker.channelLiveness, HeartbeatLiveness.suspect);
      expect(
        tracker.phase,
        UiPhase.idle,
        reason: '可疑期由 WsClient 主动重连处置；状态层不越权判死（否则'
            '连接徽标还写「已连接」、胶囊已经说「后端未连接」，两个元件自相矛盾）',
      );
    });

    test('**任何一帧**都刷新存活时刻（不只是 heartbeat）', () {
      tracker.onWsStatus(WsStatus.connected);
      now = now.add(kWsHeartbeatSuspectAfter);
      expect(tracker.channelLiveness, HeartbeatLiveness.suspect);

      // 正文帧到达：它同样证明「链路能把字节送到我这里」。
      tracker.consume(const TextDeltaEvent(epoch: 1, text: '好'));

      expect(tracker.channelLiveness, HeartbeatLiveness.online);
    });

    test('心跳帧本身不改相位（consume 返回 false），但确实改了存活判据', () {
      tracker.onWsStatus(WsStatus.connected);
      final WsEvent frame = parseWsFrame(kHeartbeatFrame)!;
      expect(tracker.consume(frame), isFalse);
      expect(tracker.phase, UiPhase.idle);

      now = now.add(kWsHeartbeatOfflineAfter);
      expect(tracker.phase, UiPhase.offline);
    });

    test('半开恢复：看门狗重连成功后相位回到 idle', () {
      tracker.onWsStatus(WsStatus.connected);
      now = now.add(kWsHeartbeatOfflineAfter);
      expect(tracker.phase, UiPhase.offline);

      // 看门狗的真实顺序（`WsClient._recoverFromHalfOpen`）：
      // 先 disconnected（在飞的轮次因此被收口，用户看到提示）→ 再重开。
      tracker.onWsStatus(WsStatus.disconnected);
      expect(tracker.phase, UiPhase.offline);
      tracker.onWsStatus(WsStatus.connecting);
      expect(tracker.phase, UiPhase.offline);
      tracker.onWsStatus(WsStatus.connected);

      expect(tracker.channelLiveness, HeartbeatLiveness.online);
      expect(tracker.phase, UiPhase.idle);
    });
  });
}
