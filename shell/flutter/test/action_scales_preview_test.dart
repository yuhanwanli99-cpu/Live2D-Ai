/// 动作幅度**即时预览**（2026-09-16）的回归。
///
/// 覆盖三条最容易悄悄坏掉的行为：
/// 1. `effectiveActionScales`（草稿优先 / 远端兜底 / 都没加载 -> null）——纯函数；
/// 2. `ActionScalesSyncer`（拖一次只发一帧 / 值没变不重发 / 重建后强发）——假时钟；
/// 3. **保存后与磁盘对齐**：草稿清空、远端回填、有效值就是服务端刚接受的那份
///    （对应「保存后 GET settings 为新值」）。
library;

import 'dart:convert';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/live2d/action_scales_sync.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';

/// 带 `action` 段的 GET 响应（v0.4.8 形状 + 2026-09-16 的 action 段）。
String getJson({double head = 0.75, double body = 0.80, double expr = 1.0}) =>
    jsonEncode(<String, Object?>{
      'llm': <String, Object?>{
        'base_url': 'https://api.deepseek.com/v1',
        'model': 'deepseek-flash',
        'has_api_key': true,
        'max_tokens': 512,
      },
      'tts': <String, Object?>{
        'base_url': 'http://127.0.0.1:8080/v1',
        'model': null,
        'voice': 'skystar',
        'has_api_key': false,
        'sample_rate': 24000,
        'channels': 1,
      },
      'persona': <String, Object?>{
        'system_prompt': '你是桌宠',
        'max_history_pairs': 0,
      },
      'action': <String, Object?>{
        'head_scale': head,
        'body_scale': body,
        'expression_scale': expr,
      },
      'dev_mode': false,
    });

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

/// 造控制器：GET 回 `initial`；PATCH 记下 body 并回**接受后**的 settings。
({SettingsController controller, List<Map<String, Object?>> patched}) build({
  String? initial,
}) {
  final String getBody = initial ?? getJson();
  final List<Map<String, Object?>> patched = <Map<String, Object?>>[];
  final MockClient client = MockClient((http.Request request) async {
    if (request.method == 'GET') return jsonResponse(getBody, 200);
    final Map<String, Object?> body =
        jsonDecode(request.body) as Map<String, Object?>;
    patched.add(body);
    // 服务端「回填」的 view：合并 action 段里的显式值。
    double head = 0.75;
    double bodyScale = 0.80;
    double expr = 1.0;
    final Object? actionPatch = body['action'];
    if (actionPatch is Map) {
      final Object? h = actionPatch['head_scale'];
      if (h is num) head = h.toDouble();
      final Object? b = actionPatch['body_scale'];
      if (b is num) bodyScale = b.toDouble();
      final Object? e = actionPatch['expression_scale'];
      if (e is num) expr = e.toDouble();
    }
    return jsonResponse(
      jsonEncode(<String, Object?>{
        'persisted': true,
        'apply_status': 'applied',
        'settings': jsonDecode(
          getJson(head: head, body: bodyScale, expr: expr),
        ),
      }),
      200,
    );
  });
  return (
    controller: SettingsController(api: ApiClient(client: client)),
    patched: patched,
  );
}

void main() {
  group('effectiveActionScales：草稿优先 / 远端兜底 / 未加载不下发', () {
    const ActionSettingsView remote = ActionSettingsView(
      headScale: 0.75,
      bodyScale: 0.80,
      expressionScale: 1.0,
    );

    test('远端与草稿都没有 -> null（渲染面用自己的出厂默认，不谎报）', () {
      expect(effectiveActionScales(remote: null), isNull);
    });

    test('只有草稿（设置还没加载）也能算出来，用于即时预览', () {
      final ActionSettingsView? v = effectiveActionScales(
        remote: null,
        draftHead: 2.0,
      );
      expect(v, isNotNull);
      expect(v!.headScale, 2.0);
      expect(v.bodyScale, ActionSettingsView.defaultBodyScale);
      expect(v.expressionScale, ActionSettingsView.defaultExpressionScale);
    });

    test('草稿字段逐项覆盖远端，没动的字段保持远端值', () {
      final ActionSettingsView? v = effectiveActionScales(
        remote: remote,
        draftBody: 0.4,
      );
      expect(v!.headScale, 0.75, reason: '没动 head 就用磁盘值');
      expect(v.bodyScale, 0.4, reason: '正在拖 body -> 即时预览');
      expect(v.expressionScale, 1.0);
    });

    test('滑条显示：全局草稿压过磁盘，没动的键保持磁盘值', () {
      final ActionSettingsView shown = displayActionScales(
        remote: remote,
        draftHead: 1.4,
      );
      expect(shown.headScale, 1.4, reason: '正在拖的头必须停在滑条上');
      expect(shown.bodyScale, 0.80);
      expect(shown.expressionScale, 1.0);
    });

    test('滑条显示：本模型还没回读的键压过磁盘覆盖，未动的键仍回落', () {
      const ActionSettingsView withOverride = ActionSettingsView(
        headScale: 0.75,
        bodyScale: 0.80,
        expressionScale: 1.0,
        activeModelId: 'bai',
        models: <String, ActionModelOverrideView>{
          'bai': ActionModelOverrideView(
            headScale: 1.2,
            bodyScale: null,
            expressionScale: null,
          ),
        },
      );
      final ActionSettingsView shown = displayActionScales(
        remote: withOverride,
        pendingModelId: 'bai',
        pendingKeys: const <String, double>{'head_scale': 1.6},
      );
      expect(shown.effectiveForActiveModel.headScale, 1.6);
      expect(
        shown.effectiveForActiveModel.bodyScale,
        0.80,
        reason: '没在拖的身体仍回落全局，不能被临时值钉死',
      );
    });

    test('载荷三键齐全且都是有限数（渲染面契约）', () {
      final Map<String, double> payload = toActionScalesPayload(
        effectiveActionScales(remote: remote, draftHead: 1.1)!,
      );
      expect(payload.keys.toSet(), <String>{'head', 'body', 'expression'});
      expect(payload.values.every((double v) => v.isFinite), isTrue);
      expect(payload['head'], 1.1);
    });
  });

  group('ActionScalesSyncer：拖动只发一帧 / 去重 / 重建后强发', () {
    test('防抖窗口内连拖多次：只发最后一帧', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
        for (int i = 1; i <= 20; i++) {
          syncer.schedule(<String, double>{
            'head': i / 10,
            'body': 1.4,
            'expression': 1.0,
          });
          async.elapse(const Duration(milliseconds: 5));
        }
        expect(sent, isEmpty, reason: '窗口没到点不该发（避免刷爆渲染面）');
        async.elapse(kActionScalesDebounce);
        expect(sent, hasLength(1), reason: '一次手势只下发一帧');
        expect(sent.single['head'], 2.0, reason: '发的是最后一帧');
        syncer.dispose();
      });
    });

    test('值没变不重发；变了才发（与舞台无关的编辑不打扰渲染面）', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
        const Map<String, double> a = <String, double>{
          'head': 0.75,
          'body': 1.4,
          'expression': 1.0,
        };
        syncer.syncNow(a);
        expect(sent, hasLength(1));
        syncer.syncNow(a);
        expect(sent, hasLength(1), reason: '同一份值不该重复下发');
        syncer.schedule(a);
        async.elapse(kActionScalesDebounce);
        expect(sent, hasLength(1));
        syncer.schedule(<String, double>{...a, 'head': 1.2});
        async.elapse(kActionScalesDebounce);
        expect(sent, hasLength(2));
        expect(sent.last['head'], 1.2);
        syncer.dispose();
      });
    });

    test('force 绕过去重：iframe 重建后必须重发', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
        const Map<String, double> a = <String, double>{
          'head': 0.75,
          'body': 1.4,
          'expression': 1.0,
        };
        syncer.syncNow(a);
        syncer.syncNow(a);
        expect(sent, hasLength(1));
        syncer.syncNow(a, force: true);
        expect(sent, hasLength(2), reason: '新桥必须收到当前值');
        syncer.dispose();
      });
    });

    test('null（未加载）不发帧，但会清掉去重标记', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
        syncer.syncNow(null);
        expect(sent, isEmpty);
        const Map<String, double> a = <String, double>{
          'head': 0.75,
          'body': 1.4,
          'expression': 1.0,
        };
        syncer.syncNow(a);
        expect(sent, hasLength(1));
        syncer.syncNow(null);
        syncer.syncNow(a);
        expect(sent, hasLength(2), reason: 'null 之后同一份值仍要发得出');
        syncer.dispose();
      });
    });

    test('dispose 丢掉窗口内的待发帧', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
        syncer.schedule(<String, double>{
          'head': 1.0,
          'body': 1.0,
          'expression': 1.0,
        });
        syncer.dispose();
        async.elapse(kActionScalesDebounce * 3);
        expect(sent, isEmpty);
      });
    });
  });

  group('保存后才落盘：拖滑条改草稿，保存回填新值', () {
    test('拖动 -> 有效值立刻变（即时预览），磁盘值不动', () async {
      final built = build();
      final SettingsController controller = built.controller;
      await controller.load();
      expect(controller.remote!.action.headScale, 0.75);
      expect(
        effectiveActionScales(
          remote: controller.remote!.action,
          draftHead: controller.draft.actionHeadScale,
        )!.headScale,
        0.75,
      );

      // 拖动滑条：只改草稿（与 `AppearanceSection.onHeadScaleChanged` 同一条路）。
      controller.edit((SettingsDraft d) => d.actionHeadScale = 2.0);
      expect(controller.dirty, isTrue, reason: '拖动 = 有未保存改动');
      expect(
        effectiveActionScales(
          remote: controller.remote!.action,
          draftHead: controller.draft.actionHeadScale,
        )!.headScale,
        2.0,
        reason: '即时预览取草稿',
      );
      expect(
        controller.remote!.action.headScale,
        0.75,
        reason: '没保存 -> 磁盘（remote）不动',
      );
      expect(built.patched, isEmpty, reason: '拖动不发 PATCH');

      // 保存：PATCH 带 action.head_scale，远端回填 2.0，草稿清空。
      final SaveOutcome outcome = await controller.save();
      expect(outcome, SaveOutcome.savedApplied);
      expect(built.patched, hasLength(1));
      expect(
        (built.patched.single['action']! as Map<String, Object?>)['head_scale'],
        2.0,
      );
      expect(controller.remote!.action.headScale, 2.0, reason: 'GET 回填新值');
      expect(controller.draft.actionHeadScale, isNull, reason: '保存后草稿清空');
      expect(controller.dirty, isFalse);
      expect(
        effectiveActionScales(
          remote: controller.remote!.action,
          draftHead: controller.draft.actionHeadScale,
        )!.headScale,
        2.0,
        reason: '保存后有效值 = 服务端刚接受的那份（即时预览与磁盘对齐）',
      );
    });

    test('放弃草稿（discard）后有效值立刻回到磁盘值', () async {
      final built = build();
      final SettingsController controller = built.controller;
      await controller.load();
      controller.edit((SettingsDraft d) => d.actionBodyScale = 0.3);
      expect(controller.dirty, isTrue);
      controller.discard();
      expect(controller.dirty, isFalse);
      expect(controller.draft.actionBodyScale, isNull);
      expect(
        effectiveActionScales(
          remote: controller.remote!.action,
          draftBody: controller.draft.actionBodyScale,
        )!.bodyScale,
        0.80,
        reason: '放弃 = 与磁盘对齐（草稿回 null，舞台应回到 0.80）',
      );
    });

    test('改回原值不算改动（prune）——不会留下假的未保存提示', () async {
      final built = build();
      final SettingsController controller = built.controller;
      await controller.load();
      controller.edit((SettingsDraft d) => d.actionExpressionScale = 1.5);
      expect(controller.dirty, isTrue);
      controller.edit((SettingsDraft d) => d.actionExpressionScale = 1.0);
      expect(controller.dirty, isFalse);
      expect(
        effectiveActionScales(
          remote: controller.remote!.action,
          draftExpression: controller.draft.actionExpressionScale,
        )!.expressionScale,
        1.0,
      );
    });
  });
}
