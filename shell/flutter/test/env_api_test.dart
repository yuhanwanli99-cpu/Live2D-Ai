import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/env_api.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';

http.Response _json(String body, int status) => http.Response(
  body,
  status,
  headers: <String, String>{'content-type': 'application/json; charset=utf-8'},
);

void main() {
  group('EnvApi.list（GET /api/v1/env）', () {
    test('解析键名与「是否已设置」；**响应里没有值这个概念**', () async {
      final EnvApi api = EnvApi(
        base: 'http://x',
        client: MockClient(
          (_) async => _json(
            '{"keys":[{"section":"llm","key":"DEEPSEEK_API_KEY","set":true},'
            '{"section":"tts","key":"ZHIPU_API_KEY","set":false}],'
            '"env_file":"/repo/.env"}',
            200,
          ),
        ),
      );
      final EnvStatus s = await api.list();
      expect(s.envFile, '/repo/.env');
      expect(s.keys, hasLength(2));
      expect(s.forSection('llm')?.key, 'DEEPSEEK_API_KEY');
      expect(s.forSection('llm')?.set, isTrue);
      expect(s.forSection('tts')?.key, 'ZHIPU_API_KEY');
      expect(s.forSection('tts')?.set, isFalse);
      expect(s.forSection('nope'), isNull);
    });

    test('缺字段/坏 JSON 不抛，按空处理', () async {
      final EnvApi api = EnvApi(
        base: 'http://x',
        client: MockClient((_) async => _json('not json', 200)),
      );
      final EnvStatus s = await api.list();
      expect(s.keys, isEmpty);
      expect(s.envFile, isEmpty);
    });
  });

  group('EnvApi.write（PUT /api/v1/env）', () {
    test('发 PUT + application/json，body 只含 key/value', () async {
      late http.Request seen;
      final EnvApi api = EnvApi(
        base: 'http://x/',
        client: MockClient((http.Request req) async {
          seen = req;
          return _json('{"key":"DEEPSEEK_API_KEY","set":true}', 200);
        }),
      );
      final EnvWriteResult result = await api.write('DEEPSEEK_API_KEY', 'sk-abc');
      expect(result.set, isTrue);
      // 旧服务端没有 apply_status 字段 → unknown：**不静默当成已生效**。
      expect(result.applyStatus, ApplyStatus.unknown);
      expect(seen.method, 'PUT');
      expect(seen.url.path, '/api/v1/env');
      expect(seen.headers['Content-Type'], contains('application/json'));
      expect(jsonDecode(seen.body), <String, Object?>{
        'key': 'DEEPSEEK_API_KEY',
        'value': 'sk-abc',
      });
    });

    test('空值写入 = 清除（返回 set=false）', () async {
      final EnvApi api = EnvApi(
        base: 'http://x',
        client: MockClient((_) async => _json('{"key":"K","set":false}', 200)),
      );
      expect((await api.write('K', '')).set, isFalse);
    });

    /// S-002（2026-10-16）：**写盘成功 ≠ 引擎装上**——状态原样带回界面。
    test('apply_status 原样带回来（写盘成功不等于已生效）', () async {
      final EnvApi api = EnvApi(
        base: 'http://x',
        client: MockClient(
          (_) async => _json(
            '{"key":"K","set":true,"apply_status":"restart_required"}',
            200,
          ),
        ),
      );
      final EnvWriteResult r = await api.write('K', 'sk');
      expect(r.set, isTrue);
      expect(r.applyStatus, ApplyStatus.restartRequired);
      expect(r.applyStatus.isLive, isFalse, reason: '只有 applied 才算已生效');
    });

    test('后端拒绝时抛 ApiException（带服务端 code）', () async {
      final EnvApi api = EnvApi(
        base: 'http://x',
        client: MockClient(
          (_) async => _json(
            '{"error":{"code":"unknown_key","message":"配置里没有声明这个键名"}}',
            400,
          ),
        ),
      );
      await expectLater(
        api.write('NOPE', 'x'),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'unknown_key')
              .having((ApiException e) => e.status, 'status', 400),
        ),
      );
    });
  });
}
