import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';

/// rc.6（2026-10-17）：多厂商选择的纯逻辑回归。
///
/// 服务端契约：GET 的 vendors 只给 `has_api_key`；PATCH 的 vendor 是三态
/// （不写 = 不动 / 值 = 快照→当前 / null = 只清标签）。
const String kGetJson = '''
{"llm":{"base_url":"https://api.example.com/v1","model":"foo",
"has_api_key":true,"max_tokens":2048,"show_reasoning":true,
"vendor":"foo","vendor_modified":true,
"vendors":{"foo":{"base_url":"https://api.example.com/v1","model":"foo",
"has_api_key":true,"max_tokens":2048,"show_reasoning":true}}},
"tts":{"mode":"cloud","base_url":"https://tts.example.com/v1","model":null,
"voice":"alloy","has_api_key":false,"sample_rate":24000,"channels":2,
"vendor":"bar","vendor_modified":false,
"vendors":{"bar":{"base_url":"https://tts.example.com/v1","model":null,
"voice":"alloy","has_api_key":false,"sample_rate":24000,"channels":2}}},
"persona":{"system_prompt":"","max_history_pairs":0},
"action":{"head_scale":0.75,"body_scale":0.8,"expression_scale":1.0,
"models":{}},
"performance":{"enabled":false,"wired":false,"base_url":"","model":"",
"has_api_key":false,"timeout_ms":4000,"structured":"auto"},
"dev_mode":false,"active_model_id":""}
''';

void main() {
  group('GET 视图（rc.6）', () {
    test('解析 vendor / vendor_modified / vendors，且只有 hasApiKey', () {
      final SettingsView parsed = SettingsView.fromJson(
        <String, Object?>{
          'llm': <String, Object?>{
            'base_url': 'https://api.example.com/v1',
            'model': 'foo',
            'vendor': 'foo',
            'vendor_modified': true,
            'vendors': <String, Object?>{
              'foo': <String, Object?>{
                'base_url': 'https://api.example.com/v1',
                'model': 'foo',
                'has_api_key': true,
                'max_tokens': 2048,
                'show_reasoning': true,
              },
            },
          },
          'tts': <String, Object?>{
            'vendor': 'bar',
            'vendors': <String, Object?>{
              'bar': <String, Object?>{
                'base_url': 'https://tts.example.com/v1',
                'voice': 'alloy',
                'has_api_key': false,
                'sample_rate': 24000,
                'channels': 2,
              },
              'melo': <String, Object?>{
                'base_url': 'http://127.0.0.1:8091/v1',
                'voice': 'ZH',
                'has_api_key': false,
                'sample_rate': 44100,
                'channels': 1,
              },
            },
          },
        },
      );
      expect(parsed.llm.vendor, 'foo');
      expect(parsed.llm.vendorModified, isTrue);
      expect(parsed.llm.vendors['foo']!.hasApiKey, isTrue);
      expect(parsed.llm.vendors['foo']!.maxTokens, 2048);
      expect(parsed.tts.vendor, 'bar');
      expect(parsed.tts.vendorModified, isFalse);
      expect(parsed.tts.vendors['bar']!.voice, 'alloy');
      expect(parsed.tts.vendors['melo']!.baseUrl, 'http://127.0.0.1:8091/v1');
      expect(parsed.tts.vendors['melo']!.sampleRate, 44100);
    });

    test('旧服务端缺 vendor 字段 → null / false / 空表（不 brick）', () {
      final SettingsView v = SettingsView.fromJson(
        <String, Object?>{
          'llm': <String, Object?>{'base_url': 'x'},
          'tts': <String, Object?>{'base_url': 'y'},
        },
      );
      expect(v.llm.vendor, isNull);
      expect(v.llm.vendorModified, isFalse);
      expect(v.llm.vendors, isEmpty);
      expect(v.tts.vendor, isNull);
      expect(v.tts.vendors, isEmpty);
    });
  });

  group('PATCH 三态（rc.6）', () {
    test('vendor 值 / null / 缺省分别序列化', () {
      final SettingsDraft d = SettingsDraft();
      expect(d.toPatch().isEmpty, isTrue);

      d.llmVendor = Tri.set('foo');
      expect(d.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'vendor': 'foo'},
      });

      d.llmVendor = Tri.clear<String>();
      expect(d.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'vendor': null},
      });

      d.llmVendor = null;
      expect(d.toPatch().isEmpty, isTrue);
    });

    test('TTS vendor 与其它字段同发（melo / 自定义都不带 mode）', () {
      final SettingsDraft d = SettingsDraft();
      d.ttsVendor = Tri.set('melo');
      expect(d.toPatch().toJson(), <String, Object?>{
        'tts': <String, Object?>{'vendor': 'melo'},
      });

      d.ttsVendor = Tri.set('bar');
      d.ttsBaseUrl = 'https://tts.example.com/v1';
      expect(d.toPatch().toJson(), <String, Object?>{
        'tts': <String, Object?>{
          'vendor': 'bar',
          'base_url': 'https://tts.example.com/v1',
        },
      });
    });
  });

  group('草稿与远端对齐', () {
    test('同值撤回、不同值保留', () async {
      final MockClient client = MockClient((http.Request request) async {
        return http.Response.bytes(
          utf8.encode(kGetJson),
          200,
          headers: const <String, String>{
            'content-type': 'application/json; charset=utf-8',
          },
        );
      });
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://127.0.0.1:18080', client: client),
      );
      await c.load();
      c.edit((SettingsDraft d) {
        d.llmVendor = Tri.set('foo');
        d.ttsVendor = Tri.set('other');
      });
      expect(c.draft.llmVendor, isNull, reason: '与远端同值应撤回');
      expect(c.draft.ttsVendor, isA<TriSet<String>>());

      c.discard();
      c.edit((SettingsDraft d) {
        d.ttsVendor = Tri.clear<String>();
      });
      expect(c.draft.ttsVendor, isA<TriClear<String>>(), reason: '远端有标签时 clear 保留');
    });
  });
}
