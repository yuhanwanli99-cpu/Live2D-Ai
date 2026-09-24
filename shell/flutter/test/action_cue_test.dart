import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/audio/sentence_assembler.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';

/// 只记录发出去的帧（与 `live2d_bridge_test.dart` 同形；协议级断言用）。
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

/// P1-3 / P1-4（2026-09-16）：导演按句 cue 的协议 + 句号对齐。
///
/// 2026-09-23（W4）追加：导演状态面「本轮没有预设」的**归零语义**——
/// `DirectorPresetGate` 的 seq 去重（同 seq 重复帧只归零一次）+ 撤销帧协议。
void main() {
  test('action_cue 帧解析成 ActionCueEvent（缺省忽略 = 兼容）', () {
    final String raw = jsonEncode(<String, Object?>{
      'type': 'action_cue',
      'seq': 1,
      'ts': '2026-09-16T00:00:00Z',
      'data': <String, Object?>{
        'epoch': 7,
        'covers_upto_seq': 3,
        'cues': <Object?>[
          <String, Object?>{
            'sentence_seq': 1,
            'preset_id': 'nod',
            'intensity': 2,
            'ttl_ms': 1800,
            'priority': 40,
          },
          <String, Object?>{
            'sentence_seq': 3,
            'preset_id': 'smile',
            'intensity': 1,
            'ttl_ms': 2600,
            'priority': 10,
          },
        ],
      },
    });
    final WsEvent? e = parseWsFrame(raw);
    expect(e, isA<ActionCueEvent>());
    final ActionCueEvent ev = e! as ActionCueEvent;
    expect(ev.epoch, 7);
    expect(ev.coversUptoSeq, 3);
    expect(ev.cues.length, 2);
    expect(ev.cues.first.sentenceSeq, 1);
    expect(ev.cues.first.presetId, 'nod');
    expect(ev.cues.first.intensity, 2);
    expect(ev.cues.first.ttlMs, 1800);
    expect(ev.cues.first.priority, 40);
  });

  test('action_cue 坏 cues 不抛：非对象项被忽略', () {
    final String raw = jsonEncode(<String, Object?>{
      'type': 'action_cue',
      'data': <String, Object?>{
        'epoch': 1,
        'cues': <Object?>[
          'x',
          42,
          <String, Object?>{'sentence_seq': 2, 'preset_id': 'shake'},
        ],
      },
    });
    final WsEvent? e = parseWsFrame(raw);
    expect(e, isA<ActionCueEvent>());
    final ActionCueEvent ev = e! as ActionCueEvent;
    expect(ev.cues.length, 1);
    expect(ev.cues.single.sentenceSeq, 2);
    expect(ev.cues.single.presetId, 'shake');
  });

  test('AssembledSentence 携带 sentence_seq（导演按句对齐的锚点）', () {
    final SentenceAssembler asm = SentenceAssembler();
    final AssembledSentence? s = asm.push(
      PcmFrame(
        pcm: Uint8List.fromList(<int>[0, 0, 1, 0, 2, 0]),
        sampleRate: 24000,
        start: true,
        end: true,
        sentenceSeq: 4,
      ),
    );
    expect(s, isNotNull);
    expect(s!.sentenceSeq, 4);
  });

  test('未知 type 仍进 UnknownWsEvent（不因新帧破坏兼容）', () {
    final WsEvent? e = parseWsFrame('{"type":"future_frame","data":{}}');
    expect(e, isA<UnknownWsEvent>());
  });

  // ── W4（2026-09-23）：撤销语义 ──────────────────────────────────────────
  //
  // 判据在 `DirectorPresetGate`（live2d_stage.dart，纯逻辑、VM 可测）：
  // **先** seq 去重，**再** `preset_id` 为 null / 空 / 'none' → revoke。
  // 这条顺序写反 = 一轮里每个 text_delta 都重复下发 none，舞台会被抖散。
  group('W4：seq 去重与「本轮没有预设」的归零判据', () {
    test('同一条 seq 的重复帧只有第一次是新决策（只会归零一次）', () {
      final DirectorPresetGate gate = DirectorPresetGate();
      // 第一次：新 seq、无 preset → 归零。
      expect(gate.decide(7, null).action, DirectorPresetAction.revoke);
      // 随后同一 seq 的每一帧（text_delta 很多条）都必须被丢掉。
      for (int i = 0; i < 5; i++) {
        expect(
          gate.decide(7, null).action,
          DirectorPresetAction.ignore,
          reason: '同 seq 重复帧必须 ignore —— 否则每一帧都会重复归零',
        );
      }
      expect(gate.lastSeq, 7);
    });

    test('新 seq 且无 preset（null / 空串 / none）一定归零两槽', () {
      final DirectorPresetGate gate = DirectorPresetGate();
      int seq = 0;
      for (final Object? none in <Object?>[null, '', 'none']) {
        seq += 1;
        final DirectorPresetDecision d = gate.decide(seq, none);
        expect(
          d.action,
          DirectorPresetAction.revoke,
          reason: 'preset_id=$none 表示「本轮没有预设」，必须显式归零',
        );
        expect(d.presetId, 'none', reason: '归零下发的就是撤销哨兵 none');
      }
    });

    test('新 seq 且有 preset → apply（不改 id）', () {
      final DirectorPresetGate gate = DirectorPresetGate();
      final DirectorPresetDecision d = gate.decide(3, 'smile');
      expect(d.action, DirectorPresetAction.apply);
      expect(d.presetId, 'smile');
    });

    test('seq 回退 = 账本重启：先复位再接受，不把新决策当旧的丢', () {
      final DirectorPresetGate gate = DirectorPresetGate();
      expect(gate.decide(9, 'nod').action, DirectorPresetAction.apply);
      // Mod 停用再启用 / 保存配置重启后计数从 1 重新开始。
      expect(gate.decide(1, 'smile').action, DirectorPresetAction.apply);
      expect(gate.lastSeq, 1);
    });
  });

  group('W4：撤销哨兵的协议帧（Dart 侧 = 渲染面 Revoke 的入口）', () {
    test('sendPreset(none, source=director) 发的是 {type:preset, id:none}', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendPreset('none', source: 'director');

      expect(transport.sent, hasLength(1));
      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      expect(frame['type'], 'preset');
      final Map<String, Object?> payload =
          frame['payload'] as Map<String, Object?>;
      expect(payload['id'], 'none');
      expect(payload['source'], 'director');
      // 渲染面侧证据（两槽同清）在 Rust 回归
      // `none_revokes_immediately_and_switching_clears_stale_params`
      // （crates/l2d-wasm-demo/src/preset/tests/packs.rs）：handle(Revoke) 后
      // `sink.overrides.is_empty()`。
      await bridge.destroy();
    });
  });
}
