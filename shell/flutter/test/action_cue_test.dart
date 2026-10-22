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
import 'support/dart_library.dart';
import 'support/source_scan.dart';

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
      plan.upsert(ev.cues); // = main.dart 收到 ActionCueEvent 做的事
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

    test('同 seq 重复帧只应用一次（按句覆盖 + 取用即移除，阶段3 §6.2）', () async {
      const ActionCue none = ActionCue(
        sentenceSeq: 7,
        presetId: 'none',
        intensity: 1,
        ttlMs: 2600,
        priority: 0,
      );
      final DirectorCuePlan plan = DirectorCuePlan();
      plan.upsert(<ActionCue>[none]); // 第一帧
      plan.upsert(<ActionCue>[none]); // 同 seq 重复帧
      plan.upsert(<ActionCue>[none, none]); // 一帧里同 seq 多条
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

    test('preset_id 空串 = 本轮不动；普通预设照常', () async {
      final DirectorCuePlan plan = DirectorCuePlan();
      int calls = 0;
      Future<void> apply(ActionCue cue) async {
        calls += 1;
      }

      plan.upsert(cueFrame(2, '').cues);
      await plan.applyForSeq(2, apply);
      expect(calls, 0, reason: '空串仍是「本轮不动」——既不撤销也不下发');

      plan.upsert(cueFrame(3, 'nod').cues);
      await plan.applyForSeq(3, apply);
      expect(calls, 1, reason: '普通预设照常下发');
      await plan.applyForSeq(null, apply);
      expect(calls, 1, reason: 'seq 为 null 一律安静丢弃');
    });

    // 0.3.14（执行单 §测试）：逐句覆盖取代整表替换——后一帧不得丢掉还没开播的句。
    test('seq 2 还在时 upsert seq 3：seq 2 仍可取用', () async {
      final DirectorCuePlan plan = DirectorCuePlan();
      plan.upsert(cueFrame(2, 'nod').cues);
      plan.upsert(cueFrame(3, 'smile').cues);
      expect(plan.length, 2, reason: '不同 seq 各留一条（整表替换时代只剩 3）');

      final List<String> applied = <String>[];
      Future<void> apply(ActionCue cue) async => applied.add(cue.presetId);
      await plan.applyForSeq(2, apply);
      expect(applied, <String>['nod'], reason: 'seq 2 还在：必须仍能取到并下发');
      expect(plan.length, 1, reason: '只取走 seq 2，seq 3 留着');
      await plan.applyForSeq(3, apply);
      expect(applied, <String>['nod', 'smile']);
      expect(plan.length, 0);
    });

    test('同 seq 后一条覆盖前一条（留最后一条）', () async {
      final DirectorCuePlan plan = DirectorCuePlan();
      plan.upsert(cueFrame(4, 'nod').cues);
      plan.upsert(cueFrame(4, 'none').cues);
      expect(plan.length, 1);
      final List<String> applied = <String>[];
      await plan.applyForSeq(4, (ActionCue cue) async => applied.add(cue.presetId));
      expect(applied, <String>['none'], reason: '同 seq 留最后一条');
    });

    test('空列表 upsert = no-op（不清空）；clear 之后长度为 0', () async {
      final DirectorCuePlan plan = DirectorCuePlan();
      plan.upsert(cueFrame(2, 'nod').cues);
      plan.upsert(const <ActionCue>[]);
      expect(plan.length, 1, reason: '空 cues 不得清掉别的 seq（no-op，不是清空）');

      plan.clear();
      expect(plan.length, 0, reason: 'clear = 整份作废');

      int calls = 0;
      await plan.applyForSeq(2, (ActionCue cue) async => calls += 1);
      expect(calls, 0, reason: '清掉之后再开始播放也不得应用');
    });

    test('源码级断言：lib/ 不存在「读 director 状态面 + applyPreset」通道 B', () {
      final List<File> libs = Directory('lib')
          .listSync(recursive: true)
          .whereType<File>()
          .where((File f) => f.path.endsWith('.dart'))
          .toList();
      expect(libs.length, greaterThan(50), reason: '扫描根路径不对，扫不到 lib/**');

      // 阶段5 维护者补丁（D44）：本判据改为**语义断言**。「读 director 状态面」
      // 不再只认字面量——观测面经常量 `kDirectorObserverModId` 读同一个只读端点，
      // 常量化不得成为规避本回归的手段。旧写法还断言 stateReaders 为空，那在
      // 「只读观测」出现后已不成立（退役的是**驱动**通道，不是读取本身）。
      final List<String> combined = <String>[];
      for (final File f in libs) {
        final String src = f.readAsStringSync();
        final bool readsDirectorState = src.contains("state('director')") ||
            src.contains('state("director")') ||
            src.contains('mods/director/state') ||
            src.contains('kDirectorObserverModId');
        if (!readsDirectorState) continue;
        if (src.contains('applyPreset')) combined.add(f.path);
      }
      expect(
        combined,
        isEmpty,
        reason: '同一文件里「读 director 状态面」+「applyPreset」= 通道 B 复活；'
            '唯一驱动者必须是 action_cue',
      );
      // 只读面自己再钉一条：观测面不得驱动舞台。
      final File observer =
          File('lib/settings/sections/director_observer_section.dart');
      if (observer.existsSync()) {
        expect(
          observer.readAsStringSync().contains('applyPreset'),
          isFalse,
          reason: '导演可观测是只读展示面：不得出现 applyPreset',
        );
      }

      // 通道 A 的接线在 main.dart（VM 加载不了 → 源码扫描，先例见
      // test/action_scales_wiring_test.dart）。
      final String main = readLibrarySource('lib/main.dart');
      expect(main.contains('ActionCueEvent'), isTrue);
      expect(
        main.contains('_directorCues.upsert(event.cues)'),
        isTrue,
        reason: 'action_cue 到达必须按 sentence_seq 覆盖（0.3.14；整表替换已删）',
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

    // 0.3.14（执行单 §行为 / §测试）：更新只有一条路（按句 upsert），清空只在
    // 六个触发点。源码扫描面走 readLibrarySource（main.dart 的 parts 一并扫到）。
    test('源码级断言（0.3.14）：只按句 upsert；clear 只在六个触发点', () {
      final String lib = stripCommentsAndStrings(
        readLibrarySource('lib/main.dart'),
      );

      // ① 整表替换彻底退役：计划类里没有 replace，接线也不许再调它。
      expect(
        readLibrarySource('lib/live2d/director_cue_plan.dart')
            .contains('void replace('),
        isFalse,
        reason: '更新只有 upsert 一条路（整表替换会把还没开播的句子丢掉）',
      );
      expect(
        lib.contains('_directorCues.replace'),
        isFalse,
        reason: '_directorCues 不得再用整表替换',
      );

      // ② 非 baseline 的 action_cue → upsert；baseline 立即应用 + 清计划。
      final String cueBranch = balancedFrom(
        lib,
        'if (event is ActionCueEvent)',
        '{',
        '}',
      );
      expect(cueBranch.contains('_directorCues.upsert(event.cues)'), isTrue);
      final String baseline = balancedFrom(
        cueBranch,
        'if (event.baseline)',
        '{',
        '}',
      );
      expect(
        baseline.contains('_directorCues.clear()'),
        isTrue,
        reason: 'baseline 是取消信号：还没开播的按句计划一并作废',
      );
      expect(
        baseline.contains('_applyBaselineCues('),
        isTrue,
        reason: 'baseline 仍立即应用到舞台（不进按句 map）',
      );
      expect(
        baseline.contains('upsert'),
        isFalse,
        reason: 'baseline 不写入按句计划',
      );

      // ③ 停止（StageCancellation 的 clearCuePlan 实体）。
      final String cancel = balancedFrom(lib, 'StageCancellation(', '(', ')');
      expect(
        cancel.contains('_directorCues.clear()'),
        isTrue,
        reason: '停止 / 新消息必须清计划',
      );

      // ④ new_epoch（判据复用既有纯函数 mustInterruptAudio）。
      expect(
        lib.contains('if (mustInterruptAudio(event)) _directorCues.clear();'),
        isTrue,
        reason: 'new_epoch = 上一轮作废 ⇒ 清计划',
      );

      // ⑤ 换模成功。
      final String swap = balancedFrom(
        lib,
        'void _onStageModelSwapped(',
        '{',
        '}',
      );
      expect(
        swap.contains('_directorCues.clear()'),
        isTrue,
        reason: '旧 cue 不得落到新模型',
      );

      // ⑥ 被接受的新 user_line：三种「接受」都清，重复帧 / 空文本不清。
      final String userLine = balancedFrom(
        lib,
        'void _onUserLineCueBoundary()',
        '{',
        '}',
      );
      for (final String accepted in <String>[
        'case UserLineAction.acceptInjected:',
        'case UserLineAction.appendOnly:',
        'case UserLineAction.appendSystem:',
      ]) {
        expect(userLine.contains(accepted), isTrue, reason: '$accepted 必须清计划');
      }
      final int clear = userLine.indexOf('_directorCues.clear()');
      expect(
        clear,
        greaterThan(userLine.indexOf('case UserLineAction.acceptInjected:')),
        reason: '三种「被接受」共用同一个清点',
      );
      expect(
        clear,
        lessThan(userLine.indexOf('case UserLineAction.skipDuplicate:')),
        reason: '重复帧不得清计划（clear 不得落到重复 / 空文本分支）',
      );
      expect(
        lib.contains('_chat.lastUserLineAction'),
        isTrue,
        reason: '判据只在控制器的决策点算一次：壳晚于它，重算会把「已接受」算成重复',
      );

      // ⑦ 播放从忙变闲、且聊天不在 streaming。
      final String idle = balancedFrom(
        lib,
        'void _onPlaybackPendingChanged()',
        '{',
        '}',
      );
      expect(
        idle.contains('_playbackReporter.report(busy)'),
        isTrue,
        reason: 'hasPending 翻转必须静默上报 /api/v1/playback',
      );
      final int guard = idle.indexOf('!_chat.streaming');
      expect(
        guard,
        greaterThanOrEqualTo(0),
        reason: '还在 streaming 时不许清（后面还有句子要来）',
      );
      expect(
        idle.indexOf('_directorCues.clear()'),
        greaterThan(guard),
        reason: '播完清计划必须在 streaming 判据之后',
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

  // ── 阶段4d：v1 三族字段 → 既有 preset 消息（只增不改）──────────────
  //
  // 编排者冻结的集成细节（O13 未覆盖的 Flutter→渲染面字段 cue 消息）：消息类型
  // 仍是既有 `preset`，旧键 {id,intensity,ttl_ms,source} 一个不动，**只增**
  // field/x/y/z/hold/at/seq/epoch/sentence_seq；缺 field 时语义与今天逐字相同。
  group('阶段4d：v1 cue 字段经既有 preset 消息下发', () {
    test('body cue 的 field/x/y/hold/at 原样进 preset payload', () async {
      final String raw = jsonEncode(<String, Object?>{
        'type': 'action_cue',
        'data': <String, Object?>{
          'epoch': 7,
          'cues': <Object?>[
            <String, Object?>{
              'sentence_seq': 2,
              'preset_id': '',
              'field': 'body',
              'x': 0.0,
              'y': 0.3,
              'intensity': 1,
              'ttl_ms': 900,
              'hold': true,
              'at': 'seg:2',
              'seq': 1,
            },
          ],
        },
      });
      final ActionCueEvent ev = parseWsFrame(raw)! as ActionCueEvent;
      final ActionCue cue = ev.cues.single;
      expect(cue.field, 'body');
      expect(cue.y, closeTo(0.3, 1e-9));
      expect(cue.hold, isTrue);
      expect(cue.at, 'seg:2');
      expect(cue.seq, 1);

      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      // main.dart 的 _applyDirectorCueForSeq 原样透传这些键。
      await bridge.sendPreset(
        cue.presetId,
        source: 'director',
        intensity: cue.intensity.toDouble(),
        ttlMs: cue.ttlMs.toDouble(),
        field: cue.field,
        x: cue.x,
        y: cue.y,
        z: cue.z,
        hold: cue.hold,
        at: cue.at,
        seq: cue.seq,
        epoch: ev.epoch,
        sentenceSeq: cue.sentenceSeq,
      );

      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      expect(frame['type'], 'preset', reason: '仍是既有 preset 消息类型');
      final Map<String, Object?> payload =
          frame['payload']! as Map<String, Object?>;
      expect(payload['id'], '');
      expect(payload['field'], 'body');
      expect(payload['x'], 0.0);
      expect(payload['y'], 0.3);
      expect(payload['intensity'], 1.0);
      expect(payload['ttl_ms'], 900.0);
      expect(payload['hold'], isTrue);
      expect(payload['at'], 'seg:2');
      expect(payload['seq'], 1);
      expect(payload['epoch'], 7);
      expect(payload['sentence_seq'], 2);
      await bridge.destroy();
    });

    test('expression cue 用 id 装面板 id；缺 field 时旧语义逐字不变', () {
      final ActionCue old = ActionCue.fromJson(<String, Object?>{
        'sentence_seq': 1,
        'preset_id': 'nod',
        'intensity': 2,
        'ttl_ms': 1800,
        'priority': 40,
      });
      expect(old.field, isNull);
      expect(old.x, isNull);
      expect(old.hold, isNull);
      expect(old.at, isNull);
      expect(old.id, isNull);
      expect(old.seq, isNull);
      expect(old.presetId, 'nod');
      expect(old.ttlMs, 1800);

      final ActionCue expr = ActionCue.fromJson(<String, Object?>{
        'sentence_seq': 1,
        'preset_id': '',
        'field': 'expression',
        'id': 'smile',
        'intensity': 1,
        'ttl_ms': 2600,
        'hold': false,
        'at': 'now',
      });
      expect(expr.field, 'expression');
      expect(expr.id, 'smile');
      expect(expr.hold, isFalse);
    });
  });
}
