import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/audio/epoch_gate.dart';

WsEvent frame(String raw) => parseWsFrame(raw)!;

void main() {
  group('钉子 13：`new_epoch` 必须**立刻**打断音频（不能等下一帧）', () {
    test('new_epoch → 要求立刻打断', () {
      expect(
        mustInterruptAudio(
          frame('{"type":"runtime_status","data":{"event":"new_epoch","epoch":1}}'),
        ),
        isTrue,
      );
    });

    test('其余 runtime_status 事件**不**触发打断', () {
      for (final String event in <String>[
        'voice_started',
        'voice_ended',
        'shutdown_ready',
      ]) {
        expect(
          mustInterruptAudio(
            frame('{"type":"runtime_status","data":{"event":"$event"}}'),
          ),
          isFalse,
          reason: event,
        );
      }
    });

    test('其它帧类型一律不触发（含 audio / turn_state / text_delta / error）', () {
      final List<String> others = <String>[
        '{"type":"audio","data":{"audio":"AAAA","epoch":0}}',
        '{"type":"turn_state","data":{"epoch":0,"status":"completed"}}',
        '{"type":"text_delta","data":{"epoch":1,"text":"hi"}}',
        '{"type":"error","data":{"code":"x"}}',
        '{"type":"heartbeat"}',
        '{"type":"subscribe_ack","data":{"topics":[]}}',
      ];
      for (final String raw in others) {
        expect(mustInterruptAudio(frame(raw)), isFalse, reason: raw);
      }
    });

    test('缺 event 字段的 runtime_status 不触发（宽容，不抛）', () {
      expect(
        mustInterruptAudio(frame('{"type":"runtime_status","data":{}}')),
        isFalse,
      );
    });
  });

  group('跨轮残留的音频片也要打断（代次比较）', () {
    test('还没开始过（current == null）→ 不算切换', () {
      expect(audioEpochChanged(current: null, incoming: 0), isFalse);
      expect(audioEpochChanged(current: null, incoming: 7), isFalse);
    });

    test('同代次 → 不打断（同一轮的连续分片）', () {
      expect(audioEpochChanged(current: 3, incoming: 3), isFalse);
    });

    test('代次变了 → 打断（旧轮的片还在路上）', () {
      expect(audioEpochChanged(current: 3, incoming: 4), isTrue);
      // 也覆盖「往回跳」这种异常情形（服务端重启后 epoch 会归零）。
      expect(audioEpochChanged(current: 5, incoming: 0), isTrue);
    });

    test('decideAudioFrame 的顺序：先判变化，再更新代次', () {
      final AudioGateDecision first = decideAudioFrame(
        currentEpoch: null,
        incomingEpoch: 0,
      );
      expect(first.interrupt, isFalse, reason: '第一片不该打断');
      expect(first.epoch, 0);

      final AudioGateDecision same = decideAudioFrame(
        currentEpoch: first.epoch,
        incomingEpoch: 0,
      );
      expect(same.interrupt, isFalse);

      final AudioGateDecision next = decideAudioFrame(
        currentEpoch: same.epoch,
        incomingEpoch: 1,
      );
      expect(next.interrupt, isTrue);
      expect(next.epoch, 1);

      // 反例钉子：如果实现写成「先更新 epoch 再比较」，这里会得到
      // `interrupt == false`，跨轮残留的片就漏掉了。
      expect(
        decideAudioFrame(currentEpoch: 1, incomingEpoch: 1).interrupt,
        isFalse,
        reason: '更新之后同代次当然不打断——但那是**下一片**的事',
      );
    });
  });
}
