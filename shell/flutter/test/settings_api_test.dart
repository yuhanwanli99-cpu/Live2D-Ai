import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';

/// **真实抓包**的 `GET /api/v1/settings` 响应（本机 v0.4.5，2026-09-10）。
/// 只把 `system_prompt` 截短，其余逐字来自服务端。
const String kRealSettingsJson = '''
{"llm":{"base_url":"https://api.deepseek.com/v1","model":"deepseek-flash",
"has_api_key":true},
"tts":{"base_url":"http://127.0.0.1:8080/v1","model":null,"voice":"skystar",
"has_api_key":false,"sample_rate":24000,"channels":1},
"persona":{"system_prompt":"你是桌面上的 Live2D 桌宠。","max_history_pairs":0,
"name":"","description":"","personality":"","scenario":"","first":""},
"dev_mode":false}
''';

/// **真实抓包**的 `PATCH /api/v1/settings` 响应（回填后的权威设置）。
const String kRealPatchResponseJson = '''
{"persisted":false,"settings":{"llm":{"base_url":"https://api.deepseek.com/v1",
"model":"deepseek-flash","has_api_key":true},
"tts":{"base_url":"http://127.0.0.1:8080/v1","model":null,"voice":"skystar",
"has_api_key":false,"sample_rate":24000,"channels":1},
"persona":{"system_prompt":"你是桌面上的 Live2D 桌宠。","max_history_pairs":0,
"name":"","description":"","personality":"","scenario":"","first":""},
"dev_mode":false}}
''';

/// 造一个 JSON 响应。
///
/// **必须显式声明 `charset=utf-8`**：`http.Response(String body, int status)`
/// 默认按 **latin1** 编码 body，body 里一有中文就直接抛
/// `Invalid argument (string): Contains invalid characters`。
/// 真实的 `ApiClient` 之所以没问题，是因为服务端确实带了这个头
/// （`web_api/responses.rs`：`application/json; charset=utf-8`）——
/// 所以测试也必须带上，否则测的不是真实链路。
http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

void main() {
  group('三态序列化：这是本层最容易错、后果最重的地方', () {
    test('只给一个字段时，同段其它键必须**不出现**（而不是 null）', () {
      // 若这里退化成把未触碰字段写成 null，用户改一下模型就会把 base_url
      // 与 api_key_env 一起清空——静默丢配置。
      final Map<String, Object?> body = const SettingsPatch(
        llm: LlmSettingsPatch(model: TriSet<String>('deepseek-chat')),
      ).toJson();
      expect(body.keys, <String>['llm']);
      final Map<String, Object?> llm = body['llm']! as Map<String, Object?>;
      expect(llm.keys, <String>['model']);
      expect(llm['model'], 'deepseek-chat');
      expect(llm.containsKey('base_url'), isFalse, reason: '未触碰的键不得出现');
      expect(llm.containsKey('api_key_env'), isFalse);
    });

    test('显式清空 → 键出现且值为 null', () {
      final Map<String, Object?> body = const SettingsPatch(
        tts: TtsSettingsPatch(apiKeyEnv: TriClear<String>()),
      ).toJson();
      final Map<String, Object?> tts = body['tts']! as Map<String, Object?>;
      expect(tts.containsKey('api_key_env'), isTrue);
      expect(tts['api_key_env'], isNull);
    });

    test('TriKeep 与「字段为 null」都是不修改', () {
      final Map<String, Object?> body = const SettingsPatch(
        llm: LlmSettingsPatch(
          model: TriKeep<String>(),
          // baseUrl 省略（null）
        ),
      ).toJson();
      // 整段都没改 ⇒ 连段键都不出现。服务端把 `{"llm":{}}` 当 no-op，
      // 所以「省略」与「发空对象」语义等价，但省略能省掉一次无意义的往返。
      expect(body.containsKey('llm'), isFalse);
      expect(body, isEmpty);
    });

    test('空补丁 → 空对象，且 isEmpty 为真（调用方据此跳过请求）', () {
      expect(const SettingsPatch().toJson(), isEmpty);
      expect(const SettingsPatch().isEmpty, isTrue);
      expect(
        const SettingsPatch(llm: LlmSettingsPatch()).isEmpty,
        isTrue,
        reason: '给了空段也等于没改',
      );
    });

    test('dev_mode 也是三态', () {
      expect(
        const SettingsPatch(devMode: TriSet<bool>(true)).toJson()['dev_mode'],
        isTrue,
      );
      expect(
        const SettingsPatch(devMode: TriClear<bool>()).toJson().containsKey(
          'dev_mode',
        ),
        isTrue,
      );
      expect(const SettingsPatch().toJson().containsKey('dev_mode'), isFalse);
    });

    test('persona 多字段混合三态', () {
      final Map<String, Object?> body = const SettingsPatch(
        persona: PersonaSettingsPatch(
          name: TriSet<String>('小星'),
          first: TriClear<String>(),
        ),
      ).toJson();
      final Map<String, Object?> p = body['persona']! as Map<String, Object?>;
      expect(p['name'], '小星');
      expect(p.containsKey('first'), isTrue);
      expect(p['first'], isNull);
      expect(p.containsKey('description'), isFalse);
      // 整数三态
      final Map<String, Object?> b2 = const SettingsPatch(
        persona: PersonaSettingsPatch(maxHistoryPairs: TriSet<int>(6)),
      ).toJson();
      expect(
        (b2['persona']! as Map<String, Object?>)['max_history_pairs'],
        6,
      );
    });
  });

  group('parity：整数/清空/null 在 JSON 往返后仍然可辨', () {
    test('toJson → jsonEncode → jsonDecode 后，null 与缺键仍可区分', () {
      final String encoded = jsonEncode(
        const SettingsPatch(
          llm: LlmSettingsPatch(
            baseUrl: TriSet<String>('http://x/v1'),
            model: TriClear<String>(),
          ),
        ).toJson(),
      );
      final Map<String, Object?> back =
          jsonDecode(encoded) as Map<String, Object?>;
      final Map<String, Object?> llm = back['llm']! as Map<String, Object?>;
      expect(llm['base_url'], 'http://x/v1');
      expect(llm.containsKey('model'), isTrue, reason: '清空必须真的发出去');
      expect(llm['model'], isNull);
      expect(llm.containsKey('api_key_env'), isFalse);
    });
  });

  group('llm.maxTokens：0（不限制）与 null（回落默认）绝不能混为一谈', () {
    Map<String, Object?> llmJson(LlmSettingsPatch patch) =>
        jsonDecode(jsonEncode(patch.toJson())) as Map<String, Object?>;

    test('TriSet(0) 发出的是 0，不是缺键、也不是 null', () {
      final Map<String, Object?> j = llmJson(
        const LlmSettingsPatch(maxTokens: TriSet<int>(0)),
      );
      expect(j.containsKey('max_tokens'), isTrue, reason: '必须真的发出去');
      expect(j['max_tokens'], 0, reason: '0 = 不限制，是合法显式值');
    });

    test('TriClear() 发出的是 null（清除 → 服务端回落默认 512）', () {
      final Map<String, Object?> j = llmJson(
        const LlmSettingsPatch(maxTokens: TriClear<int>()),
      );
      expect(j.containsKey('max_tokens'), isTrue);
      expect(j['max_tokens'], isNull);
    });

    test('字段留 null = 缺键（保持原值），三者互不塌缩', () {
      final Map<String, Object?> j = llmJson(
        const LlmSettingsPatch(model: TriSet<String>('m')),
      );
      expect(j.containsKey('max_tokens'), isFalse);
      expect(
        jsonEncode(const LlmSettingsPatch(maxTokens: TriSet<int>(0)).toJson()),
        isNot(jsonEncode(const LlmSettingsPatch(maxTokens: TriClear<int>()).toJson())),
        reason: '「设为不限制」与「清除」必须是两个不同的线上形态',
      );
    });

    test('TriKeep 不进 JSON（与字段留 null 同义）', () {
      final Map<String, Object?> j = llmJson(
        const LlmSettingsPatch(maxTokens: TriKeep<int>()),
      );
      expect(j, isEmpty);
    });

    test('响应回的是生效值：0 原样透传，缺字段回落 512', () {
      final SettingsView unlimited = SettingsView.fromJson(<String, Object?>{
        'llm': <String, Object?>{'max_tokens': 0},
      });
      expect(unlimited.llm.maxTokens, 0);
      expect(unlimited.llm.isMaxTokensUnlimited, isTrue);

      // 老服务端（含 v0.4.5 抓包）没有这个字段 → 回落默认，而不是 0。
      final SettingsView legacy = SettingsView.fromJson(<String, Object?>{
        'llm': <String, Object?>{'base_url': 'http://x/v1'},
      });
      expect(legacy.llm.maxTokens, LlmSettingsView.defaultMaxTokens);
      expect(legacy.llm.isMaxTokensUnlimited, isFalse);

      final SettingsView capped = SettingsView.fromJson(<String, Object?>{
        'llm': <String, Object?>{'max_tokens': 2048},
      });
      expect(capped.llm.maxTokens, 2048);
      expect(capped.llm.isMaxTokensUnlimited, isFalse);
    });
  });

  group('真实响应解析', () {
    test('GET 设置（真实抓包）', () {
      final SettingsView v = SettingsView.fromJson(
        jsonDecode(kRealSettingsJson) as Map<String, Object?>,
      );
      expect(v.llm.baseUrl, 'https://api.deepseek.com/v1');
      expect(v.llm.model, 'deepseek-flash');
      expect(v.llm.hasApiKey, isTrue);
      expect(v.tts.baseUrl, 'http://127.0.0.1:8080/v1');
      expect(v.tts.model, isNull, reason: 'TTS model 协议上是可为 null 的');
      expect(v.tts.voice, 'skystar');
      expect(v.tts.hasApiKey, isFalse);
      expect(v.tts.sampleRate, 24000);
      expect(v.tts.channels, 1);
      expect(v.persona.systemPrompt, isNotEmpty);
      expect(v.persona.maxHistoryPairs, 0);
      expect(v.devMode, isFalse);
    });

    test('PATCH 响应（真实抓包）：persisted + 回填后的权威设置', () {
      final SettingsPatchResult r = SettingsPatchResult.fromJson(
        jsonDecode(kRealPatchResponseJson) as Map<String, Object?>,
      );
      expect(r.persisted, isFalse, reason: 'NoChange 时服务端不写盘');
      expect(r.settings.llm.model, 'deepseek-flash');
    });

    test('响应缺字段/类型不符时不抛，回落空值', () {
      final SettingsView v = SettingsView.fromJson(<String, Object?>{
        'llm': 'not-an-object',
        'tts': null,
        'dev_mode': 'yes',
      });
      expect(v.llm.baseUrl, '');
      expect(v.llm.hasApiKey, isFalse);
      expect(v.tts.sampleRate, 0);
      expect(v.tts.model, isNull);
      expect(v.devMode, isFalse, reason: '非 true 一律当 false，不做真值转换');
      expect(SettingsView.fromJson(null).llm.model, '');
    });
  });

  group('HTTP 契约（MockClient，VM 可跑）', () {
    test('fetchSettings 打的是 GET /api/v1/settings', () async {
      late http.Request seen;
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request req) async {
          seen = req;
          return jsonResponse(kRealSettingsJson, 200);
        }),
      );
      final SettingsView v = await api.fetchSettings();
      expect(seen.method, 'GET');
      expect(seen.url.path, '/api/v1/settings');
      expect(v.llm.model, 'deepseek-flash');
    });

    test('patchSettings 打的是 PATCH（不是 PUT）且带 application/json', () async {
      late http.Request seen;
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request req) async {
          seen = req;
          return jsonResponse(kRealPatchResponseJson, 200);
        }),
      );
      await api.patchSettings(
        const SettingsPatch(llm: LlmSettingsPatch(baseUrl: TriSet('http://x/v1'))),
      );
      // 这一条来自实测：PUT→404 / PATCH→200。谁改成 PUT，这里立刻变红。
      expect(seen.method, 'PATCH');
      expect(seen.url.path, '/api/v1/settings');
      expect(seen.headers['Content-Type'], contains('application/json'));
      final Map<String, Object?> body =
          jsonDecode(seen.body) as Map<String, Object?>;
      expect(body.keys, <String>['llm']);
      expect((body['llm']! as Map<String, Object?>)['base_url'], 'http://x/v1');
    });

    test('自检端点：ok/false 都走 200，不抛；解析耗时与回显', () async {
      late String path;
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request req) async {
          path = req.url.path;
          return jsonResponse(
            '{"ok":true,"latency_ms":136,"model_echo":"deepseek-flash"}',
            200,
          );
        }),
      );
      final SettingsTestOutcome r = await api.testLlm();
      expect(path, '/api/v1/settings/test/llm');
      expect(r.ok, isTrue);
      expect(r.latencyMs, 136);
      expect(r.modelEcho, 'deepseek-flash');
    });

    test('自检失败是 200 + ok:false + error —— 不该被当成网络异常抛出', () async {
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient(
          (http.Request req) async => jsonResponse(
            '{"ok":false,"error":{"code":"unreachable","message":"连不上"}}',
            200,
          ),
        ),
      );
      final SettingsTestOutcome r = await api.testTts();
      expect(r.ok, isFalse);
      expect(r.errorCode, 'unreachable');
      expect(r.errorMessage, '连不上');
    });

    test('HTTP 层失败仍抛 ApiException（含服务端错误码）', () async {
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient(
          (http.Request req) async => jsonResponse(
            '{"error":{"code":"method_not_allowed","message":"用 PATCH"}}',
            404,
          ),
        ),
      );
      await expectLater(
        api.patchSettings(const SettingsPatch()),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'method_not_allowed')
              .having((ApiException e) => e.status, 'status', 404),
        ),
      );
    });

    test('网络异常被包成 ApiException(network_error)', () async {
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient(
          (http.Request req) async => throw const SocketExceptionStub(),
        ),
      );
      await expectLater(
        api.fetchSettings(),
        throwsA(
          isA<ApiException>().having(
            (ApiException e) => e.code,
            'code',
            'network_error',
          ),
        ),
      );
    });
  });
}

/// 故意不看 `dart:io`：Web 构建里没有 `SocketException`，用一个等价的桩。
class SocketExceptionStub implements Exception {
  const SocketExceptionStub();
  @override
  String toString() => 'SocketException: 连接被拒绝';
}
