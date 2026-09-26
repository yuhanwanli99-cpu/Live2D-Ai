import 'dart:async';
import 'dart:convert';
import 'dart:io';
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
/// 2026-09-23（W4）曾在这里测「导演状态面 `latest.preset_id`」的归零判据；
/// **2026-09-24（阶段3 D10–D13）该通道整体退役**——撤销语义并入 `action_cue`
/// 的 `preset_id == 'none'`，gate 相关的 4 条用例随类一并删除，改由下面
/// 「唯一驱动通道」组用**生产类 + 真实协议出口**钉住。
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

  // ── 阶段3（2026-09-24，D10–D13）：唯一驱动通道 = action_cue ─────────────
  //
  // 通道 B（正文上屏时拉只读状态面 latest.preset_id → applyPreset）已整体
  // 退役；撤销语义并入 cue 层：preset_id == 'none' 在该句音频开始时下发一次，
  // 空串仍是「本轮不动」。判据在 `DirectorCuePlan`（live2d_stage.dart，纯逻辑、
  // VM 可测；main.dart 经 package:web 在 VM 里加载不了，与 W4 抽出时同因）。
  group('阶段3 D10/D13：action_cue 承载撤销（唯一驱动通道）', () {
    /// 一条真实 `action_cue` 帧 → `ActionCueEvent`（不手搓 ActionCue）。
    ActionCueEvent cueFrame(int seq, Object? presetId) {
      final String raw = jsonEncode(<String, Object?>{
        'type': 'action_cue',
        'seq': 1,
        'ts': '2026-09-24T00:00:00Z',
        'data': <String, Object?>{
          'epoch': 3,
          'covers_upto_seq': seq,
          'cues': <Object?>[
            <String, Object?>{
              'sentence_seq': seq,
              'preset_id': presetId,
              'intensity': 1,
              'ttl_ms': 2600,
              'priority': 0,
            },
          ],
        },
      });
      return parseWsFrame(raw)! as ActionCueEvent;
    }

    test('preset_id:"none" → 该句音频开始时恰好一次下发撤销', () async {
      final ActionCueEvent ev = cueFrame(5, 'none');

      // 真实协议出口：断言最终真的发了一帧 `{type:preset,id:none}`。
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      final DirectorCuePlan plan = DirectorCuePlan();
      plan.replace(ev.cues); // = main.dart 收到 ActionCueEvent 做的事
      int calls = 0;
      Future<void> apply(ActionCue cue) async {
        calls += 1;
        await bridge.sendPreset(
          cue.presetId,
          source: 'director',
          intensity: cue.intensity.toDouble(),
          ttlMs: cue.ttlMs.toDouble(),
        );
      }

      // 该句音频开始播放（= main.dart 的 _applyDirectorCueForSeq）。
      await plan.applyForSeq(5, apply);
      expect(calls, 1, reason: 'preset_id=none 在该句音频开始时恰好一次');
      expect(transport.sent, hasLength(1), reason: '恰好一帧 preset 下发');
      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      expect(frame['type'], 'preset');
      final Map<String, Object?> payload =
          frame['payload'] as Map<String, Object?>;
      expect(payload['id'], 'none', reason: '撤销哨兵原样透传（渲染面翻成两槽 Revoke）');
      expect(payload['source'], 'director');

      // 同一句不会开始两次；即便被重复调用，也不得再应用（取用即移除）。
      await plan.applyForSeq(5, apply);
      expect(calls, 1, reason: '取用即移除 → 同一句绝不重复下发');
      expect(transport.sent, hasLength(1));

      await bridge.destroy();
    });

    test('同 seq 重复帧只应用一次（整表替换 + 取用即移除，阶段3 §6.2）', () async {
      const ActionCue none = ActionCue(
        sentenceSeq: 7,
        presetId: 'none',
        intensity: 1,
        ttlMs: 2600,
        priority: 0,
      );
      final DirectorCuePlan plan = DirectorCuePlan();
      plan.replace(<ActionCue>[none]); // 第一帧
      plan.replace(<ActionCue>[none]); // 同 seq 重复帧
      plan.replace(<ActionCue>[none, none]); // 一份 plan 里同 seq 多条
      expect(plan.length, 1, reason: '同 seq 的重复帧被 map 语义吃掉，不会累积');

      final List<String> applied = <String>[];
      Future<void> apply(ActionCue cue) async {
        applied.add(cue.presetId);
      }

      await plan.applyForSeq(7, apply);
      expect(applied, <String>['none'], reason: '该句音频开始时只应用一次');
      await plan.applyForSeq(7, apply);
      expect(applied, <String>['none'], reason: '重复调用不得再应用');
      expect(plan.length, 0, reason: '取用即移除');
    });

    test('preset_id 空串 = 本轮不动；cues:[] = 清空也不动；普通预设照常', () async {
      final DirectorCuePlan plan = DirectorCuePlan();
      int calls = 0;
      Future<void> apply(ActionCue cue) async {
        calls += 1;
      }

      plan.replace(cueFrame(2, '').cues);
      await plan.applyForSeq(2, apply);
      expect(calls, 0, reason: '空串仍是「本轮不动」——既不撤销也不下发');

      plan.replace(<ActionCue>[]);
      expect(plan.length, 0, reason: '空 cues = 清空计划 = 本轮不动');
      await plan.applyForSeq(2, apply);
      expect(calls, 0);

      plan.replace(cueFrame(3, 'nod').cues);
      await plan.applyForSeq(3, apply);
      expect(calls, 1, reason: '普通预设照常下发');
      await plan.applyForSeq(null, apply);
      expect(calls, 1, reason: 'seq 为 null 一律安静丢弃');
    });

    test('源码级断言：lib/ 不存在「读 director 状态面 + applyPreset」通道 B', () {
      final List<File> libs = Directory('lib')
          .listSync(recursive: true)
          .whereType<File>()
          .where((File f) => f.path.endsWith('.dart'))
          .toList();
      expect(libs.length, greaterThan(50), reason: '扫描根路径不对，扫不到 lib/**');

      final List<String> stateReaders = <String>[];
      final List<String> combined = <String>[];
      for (final File f in libs) {
        final String src = f.readAsStringSync();
        final bool readsDirectorState = src.contains("state('director')") ||
            src.contains('state("director")') ||
            src.contains('mods/director/state');
        if (!readsDirectorState) continue;
        stateReaders.add(f.path);
        if (src.contains('applyPreset')) combined.add(f.path);
      }
      expect(
        combined,
        isEmpty,
        reason: '同一文件里「读 director 状态面」+「applyPreset」= 通道 B 复活；'
            '唯一驱动者必须是 action_cue',
      );
      expect(
        stateReaders,
        isEmpty,
        reason: '通道 B 已退役：lib/ 里不应再有任何拉 director 状态面的读点',
      );

      // 通道 A 的接线在 main.dart（VM 加载不了 → 源码扫描，先例见
      // test/action_scales_wiring_test.dart）。
      final String main = File('lib/main.dart').readAsStringSync();
      expect(main.contains('ActionCueEvent'), isTrue);
      expect(
        main.contains('_directorCues.replace(event.cues)'),
        isTrue,
        reason: 'action_cue 到达必须整表替换当前计划',
      );
      expect(main.contains('sentenceStarts'), isTrue,
          reason: '锚点必须是「该句音频开始播放」（与声音同拍）');
      expect(main.contains('_applyDirectorCueForSeq'), isTrue);
      for (final String gone in <String>[
        '_applyDirectorPreset',
        'DirectorPresetGate',
        '_directorTurnHandled',
        '_directorFetching',
        '_directorPresetGate',
      ]) {
        expect(main.contains(gone), isFalse, reason: '$gone 属于已退役的通道 B');
      }
      expect(
        File('lib/live2d/live2d_stage.dart')
            .readAsStringSync()
            .contains('DirectorPresetGate'),
        isFalse,
        reason: 'W4 的 gate 三件套已从 live2d_stage.dart 删除',
      );
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
