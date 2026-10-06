/// 阶段4f / D28：**会话 baseline 应用接线**回归（协议 V10 §9.3 / O8 / D31）。
///
/// host 两路交付 baseline——① chat / stop **响应体的 `baseline` 字段**；
/// ② 既有 `action_cue` 帧上的 `{baseline:true, reason}`（D31）。任一路到达
/// 即**立即应用**到舞台；空 baseline ⇒ 恰好一次 `none` 撤销（回待机），
/// **不得**退化成「什么都不做」。两路都发生在本地取消事务的四步**之后**。
///
/// 这里测的是**纯逻辑**（`live2d/session_baseline.dart` + `api/ws_frame.dart`
/// + `api/api_client.dart`）与 `main.dart` 的**接线**（源码扫描，先例见
/// `stage_cancel_test.dart` / `action_scales_wiring_test.dart`）——
/// `main.dart` 经 `package:web` 依赖，在 VM 里加载不了。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/session_baseline.dart';
import 'package:live2d_ai_shell/live2d/stage_cancel.dart';
import 'support/dart_library.dart';

/// host 的 D31 取消信号原文（真实形状，见 `chat_routes.rs` 的
/// `return_to_session_baseline`：既有 action_cue 帧 + `baseline`/`reason`）。
String baselineFrame({required String reason, required String cues}) =>
    '{"type":"action_cue","data":{"epoch":7,"covers_upto_seq":0,'
    '"cues":$cues,"baseline":true,"reason":"$reason"}}';

void main() {
  test('baseline_applied_on_new_message', () {
    // ② WS 路：`action_cue{baseline:true}`（D31）。帧必须被识别成 baseline 信号，
    // 而不是按句计划。
    final WsEvent? parsed = parseWsFrame(
      baselineFrame(
        reason: 'new-message',
        cues: '[{"field":"head","y":-0.4,"intensity":2,"at":"now","hold":true}]',
      ),
    );
    expect(parsed, isA<ActionCueEvent>());
    final ActionCueEvent event = parsed! as ActionCueEvent;
    expect(event.baseline, isTrue, reason: 'D31 标记必须被解析出来');
    expect(event.reason, 'new-message');
    expect(event.cues.length, 1);

    // ① HTTP 路：响应体的 `baseline` 原文（同一个 cue 形状）。
    final List<BaselinePresetCall> fromHttp = baselineApplication(
      <Object?>[
        <String, Object?>{
          'field': 'head',
          'y': -0.4,
          'intensity': 2,
          'at': 'now',
          'hold': true,
        },
      ],
    );
    final List<BaselinePresetCall> fromWs =
        baselineApplicationFromCues(event.cues);

    // 两路交付必须是**同一份下发**（同一个解码器、同一个解释）。
    expect(fromWs.length, 1);
    expect(fromHttp.length, fromWs.length);
    final BaselinePresetCall call = fromWs.single;
    expect(call.id, 'head', reason: 'v1 body/head 的语义 id = 字段名');
    expect(call.field, 'head');
    expect(call.y, -0.4);
    expect(call.intensity, 2);
    expect(call.at, 'now');
    expect(call.hold, isTrue);
    expect(call.ttlMs, isNull, reason: '帧里没写 ttl ⇒ 交给舞台缺省，不是 0');
    expect(fromHttp.single.toString(), call.toString());

    // expression 的 id 也从 baseline 里带出来（D28：立即应用，锚在 now）。
    final List<BaselinePresetCall> face = baselineApplication(<Object?>[
      <String, Object?>{
        'field': 'expression',
        'id': 'smile',
        'intensity': 3,
        'at': 'now',
        'hold': true,
      },
    ]);
    expect(face.single.id, 'smile');
    expect(face.single.field, 'expression');
    expect(face.single.intensity, 3);
  });

  test('baseline_cleared_to_idle_when_absent', () {
    // 所有「空 baseline」形态都必须翻成**恰好一次** `none` 撤销（回待机）。
    final List<Object?> shapes = <Object?>[
      null,
      <Object?>[],
      'garbage',
      42,
      <Object?>[1, 'x', true], // 全是非对象元素 ⇒ 实质为空
    ];
    for (final Object? raw in shapes) {
      final List<BaselinePresetCall> calls = baselineApplication(raw);
      expect(
        calls.length,
        1,
        reason: '空 baseline 必须给一次下发，不得什么都不做：raw=$raw',
      );
      expect(calls.single.id, 'none', reason: '等价 applyPreset(\'none\')');
      expect(calls.single.field, isNull, reason: '撤销走 legacy 哨兵路径');
    }
    // WS 路没有 cue 时同口径。
    final List<BaselinePresetCall> empty =
        baselineApplicationFromCues(const <ActionCue>[]);
    expect(empty.length, 1);
    expect(empty.single.id, 'none');
  });

  test('baseline_application_follows_the_cancellation_transaction_order', () async {
    // main.dart 的四个出口实体（顺序由 StageCancellation 纯逻辑钉住）。
    final DirectorCuePlan plan = DirectorCuePlan()
      ..replace(<ActionCue>[
        const ActionCue(
          sentenceSeq: 2,
          presetId: 'nod',
          intensity: 2,
          ttlMs: 900,
          priority: 40,
        ),
      ]);
    final List<String> log = <String>[];
    final StageCancellation cancellation = StageCancellation(
      clearCuePlan: (String reason) {
        plan.replace(const <ActionCue>[]);
        log.add('clear-cue-plan');
      },
      revokeStage: (String reason) => log.add('revoke-stage'),
      dropPendingAudio: (String reason) => log.add('drop-pending-audio'),
      returnToBaseline: (String reason) {
        log.add('return-to-baseline');
        // 第 ④ 步的本地缺省臂：此刻还没有交付内容 ⇒ 缺省「空 baseline」= none。
        for (final BaselinePresetCall call
            in baselineApplicationFromCues(const <ActionCue>[])) {
          log.add('apply:${call.id}');
        }
      },
    );

    cancellation.cancel('new-message');
    expect(
      cancellation.lastSteps,
      <String>[
        'clear-cue-plan',
        'revoke-stage',
        'drop-pending-audio',
        'return-to-baseline',
      ],
      reason: '四出口顺序不变（阶段4d 钉住）',
    );
    expect(plan.length, 0, reason: '清动作 cue 计划');
    expect(
      log,
      <String>[
        'clear-cue-plan',
        'revoke-stage',
        'drop-pending-audio',
        'return-to-baseline',
        'apply:none',
      ],
    );

    // host 的交付**只能**发生在取消事务之后（本地取消在 POST 之前，
    // 而两路交付都要等 host 收到 POST）——到达即应用，且不补帧。
    for (final BaselinePresetCall call in baselineApplication(<Object?>[
      <String, Object?>{
        'field': 'expression',
        'id': 'smile',
        'intensity': 1,
        'hold': true,
      },
    ])) {
      log.add('apply:${call.id}');
    }
    expect(log.last, 'apply:smile', reason: '交付在四步之后立即应用');
    expect(
      log.indexOf('apply:smile'),
      greaterThan(log.indexOf('return-to-baseline')),
      reason: 'baseline 应用不得插到清 cue / 撤销 / 丢音频之前',
    );

    // **不补帧**：取消后旧 cue 再「开始播放」也不得 apply（阶段4d 契约不变）。
    int applied = 0;
    await plan.applyForSeq(2, (ActionCue cue) async {
      applied += 1;
    });
    expect(applied, 0);
  });

  test('baseline_cues_ignore_unknown_fields_and_bad_shapes', () {
    // 未知键忽略（V11），已知键照常；单个对象与数组等价。
    final List<BaselinePresetCall> single = baselineApplication(
      <String, Object?>{
        'preset_id': 'nod',
        'intensity': 2,
        'ttl_ms': 1500,
        'sentence_seq': 1,
        'unknown_key': <String, Object?>{'deep': true},
        'another': 'ignored',
      },
    );
    expect(single.single.id, 'nod');
    expect(single.single.field, isNull, reason: 'legacy 路径没有 field');
    expect(single.single.intensity, 2);
    expect(single.single.ttlMs, 1500);
    expect(single.single.sentenceSeq, 1);

    // 数组里的非对象元素丢弃，对象元素照收（host baseline_cues 同口径）。
    final List<BaselinePresetCall> mixed = baselineApplication(<Object?>[
      'nope',
      7,
      <String, Object?>{'field': 'body', 'x': 0.25, 'intensity': 1},
    ]);
    expect(mixed.length, 1);
    expect(mixed.single.id, 'body');
    expect(mixed.single.field, 'body');
    expect(mixed.single.x, 0.25);

    // 两路都缺 id / preset_id 的 expression cue：id 落到空串（不猜成别的预设）。
    final List<BaselinePresetCall> bare = baselineApplication(
      <String, Object?>{'field': 'expression'},
    );
    expect(bare.single.id, '');
    expect(bare.single.field, 'expression');
  });

  test('api_client_forwards_the_baseline_from_chat_and_stop_responses', () async {
    final List<String> seen = <String>[];
    final ApiClient api = ApiClient(
      base: 'http://127.0.0.1:18080',
      client: MockClient((http.Request request) async {
        if (request.url.path == '/api/v1/chat/stop') {
          return http.Response(
            '{"accepted":true,"session_id":"A","baseline":null}',
            200,
            headers: const <String, String>{'content-type': 'application/json'},
          );
        }
        return http.Response(
          '{"accepted":true,"epoch":3,"pending_cleared":false,'
          '"session_id":"A","baseline":{"field":"expression","id":"smile",'
          '"intensity":1,"at":"now","hold":true}}',
          200,
          headers: const <String, String>{'content-type': 'application/json'},
        );
      }),
      onSessionBaseline: (Object? baseline, String reason) {
        seen.add('$reason:${jsonEncode(baseline)}');
      },
    );

    final ChatAccepted accepted = await api.sendChat('hi', sessionId: 'A');
    expect(accepted.accepted, isTrue);
    expect(seen.length, 1, reason: 'chat 响应体的 baseline 必须转发');
    expect(seen.single.startsWith('new-message:'), isTrue);
    expect(seen.single.contains('"id":"smile"'), isTrue);

    await api.stopChat();
    expect(seen.length, 2, reason: 'stop 响应体的 baseline 必须转发');
    expect(seen.last, 'stop:null', reason: '显式 null = 待机，也要交付（撤销）');

    // 键缺席 = host 没交付（例如 busy 路径）→ **不回调**，不动舞台。
    final List<String> untouched = <String>[];
    final ApiClient bare = ApiClient(
      base: 'http://127.0.0.1:18080',
      client: MockClient(
        (http.Request request) async => http.Response(
          '{"accepted":true,"epoch":1,"pending_cleared":false}',
          200,
          headers: const <String, String>{'content-type': 'application/json'},
        ),
      ),
      onSessionBaseline: (Object? baseline, String reason) => untouched.add(reason),
    );
    await bare.sendChat('hi');
    expect(untouched, isEmpty, reason: 'baseline 键缺席不得动舞台');
  });

  test('wiring（源码扫描）：两路交付都接进 baseline 应用', () {
    final String main = readLibrarySource('lib/main.dart');
    expect(
      main.contains('onSessionBaseline: _onSessionBaselineDelivered'),
      isTrue,
      reason: '第 ① 路（HTTP 响应体）必须接线',
    );
    expect(
      main.contains('if (event.baseline) {'),
      isTrue,
      reason: '第 ② 路（action_cue{baseline:true}，D31）必须识别',
    );
    expect(
      main.contains('_applyBaselineCues(event.cues, event.reason'),
      isTrue,
      reason: 'baseline 帧必须**立即应用**，而不是装进按句计划',
    );
    expect(
      main.contains('_applyBaselineCues(const <ActionCue>[], reason)'),
      isTrue,
      reason: '取消事务第 ④ 步的缺省臂 = 空 baseline（回待机），不是什么都不做',
    );
    expect(
      main.contains('returnToBaseline: _requestSessionBaseline,'),
      isTrue,
      reason: '第 ④ 步仍挂在取消事务上（与清 cue / 撤销 / 丢音频同序）',
    );
    expect(
      main.contains('parseBaselineCues(baseline)'),
      isTrue,
      reason: '两路交付共用同一个解码入口',
    );
    // **不建前端镜像**：main.dart 里不得出现缓存 baseline 的字段。
    expect(
      RegExp(r'_sessionBaseline\b').hasMatch(main),
      isFalse,
      reason: '不得有 baseline 镜像字段（V8）',
    );
    expect(
      RegExp(r'List<ActionCue>\s+_baseline').hasMatch(main),
      isFalse,
      reason: '不得把 baseline 存成前端状态',
    );
  });
}
