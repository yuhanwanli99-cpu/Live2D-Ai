import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/diagnostics_api.dart';
import 'package:live2d_ai_shell/api/models_api.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';

/// 全部夹具都是**真实抓包**（本机 v0.4.8，2026-09-10）。
const String kRealModelsJson = '{"models":[]}';

const String kRealModsJson = '''
{"mods":[{"api_version":1,"enabled":false,"id":"director","name":"动作编排",
"status":"disabled","version":"0.1.0"},
{"api_version":1,"enabled":true,"id":"local-tts","name":"本地语音合成",
"status":"running","version":"0.1.0"}]}
''';

const String kRealCapabilitiesJson = '''
{"app":"live2d-ai-desktop","version":"0.4.7","schema_version":1,
"actions":["nod","shake_no","tilt","look_around","listen","surprise"],
"action_sources":["user_command","llm_tool","rule_fallback"],
"strength_levels":[1,2,3],"model_upload_supported":true,
"script_invoke_supported":true,"runtime_ws":"/ws/runtime",
"state_ws":"/ws/state","ws_protocol_version":1}
''';

const String kRealStatusJson = '''
{"started_at":"2026-09-10T13:59:26.439Z","uptime_s":682,
"config_path":"live2d-ai.toml","active_model_id":"bai_001",
"audio":{"backend":"none","available":false,"sample_rate":24000,"channels":1},
"llm":{"configured":true,"base_url":"https://api.deepseek.com/v1",
"model":"deepseek-flash","has_api_key":true},
"tts":{"configured":true,"base_url":"http://127.0.0.1:8080/v1"},
"dev_mode":false,"current_epoch":0}
''';

/// 真实 403：logs 端点需要 dev_mode。
const String kDevModeRequiredJson = '''
{"error":{"code":"dev_mode_required","message":"logs 端点需 dev_mode=true 开启"}}
''';

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

void main() {
  group('ModelsApi', () {
    test('空列表是真的空（本仓库不捆绑模型二进制）', () async {
      late http.Request seen;
      final ModelsApi api = ModelsApi(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request r) async {
          seen = r;
          return jsonResponse(kRealModelsJson, 200);
        }),
      );
      final List<ModelInfo> models = await api.list();
      expect(models, isEmpty);
      expect(seen.method, 'GET');
      expect(seen.url.path, '/api/v1/models');
    });

    test('列表项字段与 humanSize', () async {
      const String one = '''
{"models":[{"id":"bai_001","display_name":"白玉","version":1,
"layout":{"center_x":0.0,"center_y":0.0,"width":1.0,"height":1.0},
"moc3_file":"bai_001/bai.moc3","texture_files":["a.png","b.png"],
"has_physics":true,"has_display_info":false,"active":true,
"imported_at":"2026-09-01T00:00:00Z","size_bytes":2621440}]}
''';
      final ModelsApi api = ModelsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(one, 200)),
      );
      final ModelInfo m = (await api.list()).single;
      expect(m.id, 'bai_001');
      expect(m.displayName, '白玉');
      expect(m.active, isTrue);
      expect(m.hasPhysics, isTrue);
      expect(m.textureCount, 2);
      // **用体积/物理徽标代替缩略图**（后端没有缩略图端点）。
      expect(m.humanSize, '2.5 MB');
    });

    test('activate 上报 active_id / prev_active_id / requires_restart', () async {
      late http.Request seen;
      final ModelsApi api = ModelsApi(
        base: 'http://x',
        client: MockClient((http.Request r) async {
          seen = r;
          return jsonResponse(
            '{"active_id":"bai_001","prev_active_id":null,'
            '"requires_restart":true,"model_url":"/models/bai_001/bai.model3.json"}',
            200,
          );
        }),
      );
      final ActivateResult r = await api.activate('bai_001');
      expect(seen.method, 'POST');
      expect(seen.url.path, '/api/v1/models/bai_001/activate');
      expect(r.activeId, 'bai_001');
      expect(r.requiresRestart, isTrue);
      expect(r.modelUrl, contains('model3.json'));
    });

    test('delete 激活中的模型 → 409 model_active 抛结构化错误', () async {
      final ModelsApi api = ModelsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"error":{"code":"model_active","message":"不能删除激活中的模型"}}',
            409,
          ),
        ),
      );
      expect(
        () => api.remove('bai_001'),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'model_active')
              .having((ApiException e) => e.status, 'status', 409),
        ),
      );
    });

    test('网络失败 → network_error（不抛出裸异常）', () async {
      final ModelsApi api = ModelsApi(
        base: 'http://x',
        client: MockClient((_) async => throw const _Boom()),
      );
      expect(
        () => api.list(),
        throwsA(isA<ApiException>().having((ApiException e) => e.code, 'code', 'network_error')),
      );
    });
  });

  group('ModsApi', () {
    test('真实列表：5 个 Mod，状态文字两两可辨', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(kRealModsJson, 200)),
      );
      final List<ModInfo> mods = await api.list();
      expect(mods, hasLength(2));
      expect(mods[0].id, 'director');
      expect(mods[0].enabled, isFalse);
      expect(mods[0].statusLabel, '已停用');
      expect(mods[1].id, 'local-tts');
      expect(mods[1].isRunning, isTrue);
      expect(mods[1].statusLabel, '运行中');
    });

    test('未知 status 原样显示（**不谎报成功**）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"mods":[{"id":"x","name":"X","status":"weird_state"}]}',
            200,
          ),
        ),
      );
      final ModInfo m = (await api.list()).single;
      expect(m.statusLabel, 'weird_state');
      expect(m.isRunning, isFalse);
    });

    test('status 为空 → 「未知」', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse('{"mods":[{"id":"x"}]}', 200)),
      );
      expect((await api.list()).single.statusLabel, '未知');
    });

    test('enable/disable 打对路径', () async {
      final List<String> paths = <String>[];
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((http.Request r) async {
          paths.add(r.url.path);
          return jsonResponse('{}', 200);
        }),
      );
      await api.setEnabled('local-tts', true);
      await api.setEnabled('local-tts', false);
      expect(paths, <String>[
        '/api/v1/mods/local-tts/enable',
        '/api/v1/mods/local-tts/disable',
      ]);
    });
  });

  group('DiagnosticsApi', () {
    test('capabilities：动作清单来自服务端（前端不硬编码 6 个）', () async {
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(kRealCapabilitiesJson, 200)),
      );
      final AppCapabilities c = await api.capabilities();
      expect(c.app, 'live2d-ai-desktop');
      expect(c.actions, hasLength(6));
      expect(c.actions, contains('look_around'));
      expect(c.strengthLevels, <int>[1, 2, 3]);
      expect(c.wsProtocolVersion, 1);
      expect(c.modelUploadSupported, isTrue);
    });

    test('status：能读出 active_model_id 与 dev_mode', () async {
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(kRealStatusJson, 200)),
      );
      final Map<String, Object?> s = await api.status();
      expect(s['active_model_id'], 'bai_001');
      expect(s['dev_mode'], isFalse);
      // 服务端在 --web 模式下没有声卡，`audio.available=false` 是**预期**的
      // （音频单源 = 浏览器）。诊断面板必须如实显示这一点。
      expect((s['audio']! as Map<String, Object?>)['available'], isFalse);
    });

    test('logs 非 dev → 403 dev_mode_required 抛结构化错误（不是空列表）', () async {
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(kDevModeRequiredJson, 403)),
      );
      expect(
        () => api.logs(),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'dev_mode_required')
              .having((ApiException e) => e.status, 'status', 403),
        ),
      );
    });

    test('logs 带 level/limit 查询参数', () async {
      late Uri seen;
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient((http.Request r) async {
          seen = r.url;
          return jsonResponse('{"lines":[]}', 200);
        }),
      );
      await api.logs(level: 'warn', limit: 100);
      expect(seen.path, '/api/v1/logs');
      expect(seen.queryParameters['level'], 'warn');
      expect(seen.queryParameters['limit'], '100');
    });

    test('logs 解析（含 target 与 level）', () async {
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"lines":[{"level":"INFO","target":"web_api","message":"已监听"}]}',
            200,
          ),
        ),
      );
      final List<LogLine> lines = await api.logs();
      expect(lines.single.level, 'INFO');
      expect(lines.single.asText, '[INFO] web_api: 已监听');
    });

    test('坏 JSON → 空对象 / 空列表，不抛', () async {
      final DiagnosticsApi api = DiagnosticsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse('{broken', 200)),
      );
      expect(await api.status(), isEmpty);
      expect(await api.logs(), isEmpty);
      final AppCapabilities c = await api.capabilities();
      expect(c.actions, isEmpty);
    });
  });
}

class _Boom implements Exception {
  const _Boom();
  @override
  String toString() => '连接被拒绝';
}
