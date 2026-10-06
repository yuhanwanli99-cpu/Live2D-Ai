/// 阶段4d：stage-clock（音频播放时钟）**30ms 节奏下发**回归。
///
/// 协议 §6.1/§6.5 + O13：前端把「段内播放位置」按 30ms 节拍下发渲染面，渲染面
/// 用它替代 `performance.now()` 当 `now_ms`。停止后**停发**（不补帧，V7 §6.6）。
///
/// 采样生产者是 `AudioPlayer`（30ms ticker + `StageClockCadence`），但那个类
/// import `package:web`、在 VM 上加载不了；这里的测试直接驱动同一个
/// `StageClockCadence` 并把结果送进**真桥**（`Live2DBridge` + 假 transport），
/// 于是「节奏」与「wire 形状」两条都落到可回归的代码上。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/audio/stage_clock.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'support/dart_library.dart';

class _FakeTransport implements Live2DTransport {
  final StreamController<String> _controller =
      StreamController<String>.broadcast();
  final List<String> sent = <String>[];

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async => sent.add(json);

  @override
  Future<void> dispose() async {
    await _controller.close();
  }

  void emit(String frame) => _controller.add(frame);
}

List<Map<String, Object?>> _stageClockFrames(_FakeTransport transport) =>
    transport.sent
        .map((String s) => jsonDecode(s) as Map<String, Object?>)
        .where((Map<String, Object?> f) => f['type'] == 'stage-clock')
        .toList();

void main() {
  test('stage_clock_is_sent_at_thirty_ms_cadence_while_playing', () {
    fakeAsync((FakeAsync async) {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      async.flushMicrotasks();

      final StageClockCadence cadence = StageClockCadence();
      int posMs = 0;
      bool playing = true;
      Timer.periodic(kStageClockInterval, (Timer _) {
        if (playing) posMs += kStageClockInterval.inMilliseconds;
        final StageClockSample? accepted = cadence.accept(
          StageClockSample(seg: 4, posMs: posMs, playing: playing),
          async.elapsed,
        );
        if (accepted != null) {
          unawaited(
            bridge.sendStageClock(
              seg: accepted.seg,
              posMs: accepted.posMs,
              playing: accepted.playing,
            ),
          );
        }
      });

      // 播放 300ms → 恰好 10 次（30ms 一帧）。
      async.elapse(const Duration(milliseconds: 300));
      final List<Map<String, Object?>> playingFrames = _stageClockFrames(
        transport,
      );
      expect(playingFrames, hasLength(10), reason: '300ms / 30ms = 10 次下发');

      // O13 冻结的 wire 形状：version 1 + type stage-clock +
      // payload{seg,pos_ms,playing}。
      final Map<String, Object?> lastPlaying = playingFrames.last;
      expect(lastPlaying['version'], 1);
      final Map<String, Object?> payload =
          lastPlaying['payload']! as Map<String, Object?>;
      expect(payload.keys.toSet(), <String>{'seg', 'pos_ms', 'playing'});
      expect(payload['seg'], 4);
      expect(payload['pos_ms'], 300);
      expect(payload['playing'], true);

      // 停播：**只追加一次** playing:false，此后静默（不补帧）。
      playing = false;
      async.elapse(const Duration(milliseconds: 300));
      final List<Map<String, Object?>> all = _stageClockFrames(transport);
      expect(all, hasLength(11), reason: '停播只发一次，绝不再补帧');
      final Map<String, Object?> stopPayload =
          all.last['payload']! as Map<String, Object?>;
      expect(stopPayload['playing'], false);
      expect(stopPayload['seg'], 4);

      bridge.dispose();
    });
  });

  test('接线（源码扫描）：音频 30ms 节拍 → main.dart → 舞台', () {
    final String audio = File('lib/audio/audio_player.dart').readAsStringSync();
    expect(
      audio.contains('_clockCadence.accept('),
      isTrue,
      reason: 'AudioPlayer 必须经同一个 StageClockCadence 出口（节奏的唯一实现）',
    );
    expect(audio.contains('_stageClock.add(accepted)'), isTrue);
    expect(
      audio.contains('Stream<StageClockSample> get stageClock'),
      isTrue,
      reason: 'AudioPlayer 必须暴露采样流',
    );
    final String main = readLibrarySource('lib/main.dart');
    expect(main.contains('_audio.stageClock.listen('), isTrue);
    expect(main.contains('sendStageClock('), isTrue);
    final String stage = File(
      'lib/live2d/live2d_stage.dart',
    ).readAsStringSync();
    expect(stage.contains('void sendStageClock('), isTrue);
  });
}
