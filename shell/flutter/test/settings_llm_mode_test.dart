/// 0.3.7 T1 / 0.3.9 rc.2：LLM 云端 / 本地的**草稿与交互**回归（纯逻辑 + 真泵）。
///
/// 守四件事：
/// 1. 旧服务端缺 `mode` → 按云端理解（不能把界面拨到本地那套配置）；
/// 2. 草稿切模式只发 `mode`（服务端普通可选值），不预填/不清空地址 / 模型；
/// 3. 0.3.9 rc.2 起没有厂商下拉 / 快照脏标记：PATCH body 里**不再出现**
///    `vendor` / `vendors` / `vendor_modified` 键，二级字段（地址 / 模型名 /
///    token / 思考 / 音色 / 采样率 / 声道 / 密钥变量名）随模式一起正确发出；
/// 4. 本地模式给 Ollama 示例文案（示例，不预填）、自检说明仍是「只打 /models」。
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/llm_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 真实抓包形状的 `GET /api/v1/settings`（0.3.6 形态：**没有** `llm.mode`）。
///
/// 0.3.9 rc.2 的后端不再返回厂商键，这里也**不再造** vendor / vendors。
const String _getJson = '''
{"llm":{"base_url":"https://api.deepseek.com/v1","model":"deepseek-flash",
"has_api_key":true,"max_tokens":4096,"show_reasoning":false},
"tts":{"mode":"local","base_url":"http://127.0.0.1:8091/v1","model":null,
"voice":"ZH","has_api_key":false,"sample_rate":44100,"channels":1},
"persona":{"system_prompt":"","max_history_pairs":0},
"action":{"head_scale":0.75,"body_scale":0.8,"expression_scale":1.0,"models":{}},
"dev_mode":false,"active_model_id":""}
''';

http.Response _jsonResponse(String body) => http.Response.bytes(
  utf8.encode(body),
  200,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

SettingsController _controller() {
  final MockClient client = MockClient(
    (http.Request request) async => _jsonResponse(_getJson),
  );
  return SettingsController(
    api: ApiClient(base: 'http://127.0.0.1:18080', client: client),
  );
}

Future<void> _pump(
  WidgetTester tester, {
  required SettingsController controller,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          // 真宿主（SettingsScaffold）会监听 controller 重建；这里补上同一条线。
          child: ListenableBuilder(
            listenable: controller,
            builder: (BuildContext context, Widget? child) => LlmSection(
              controller: controller,
              view: controller.remote!,
              devMode: false,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

/// 递归断言：补丁 JSON **任何一层**都不出现厂商键（0.3.9 rc.2 契约）。
void _expectNoVendorKeys(Object? node, {String path = r'$'}) {
  if (node is Map) {
    for (final MapEntry<Object?, Object?> e in node.entries) {
      final String key = e.key.toString();
      expect(
        <String>['vendor', 'vendors', 'vendor_modified'].contains(key),
        isFalse,
        reason: '补丁里不得再出现厂商键（$path.$key）',
      );
      _expectNoVendorKeys(e.value, path: '$path.$key');
    }
  } else if (node is List) {
    for (int i = 0; i < node.length; i += 1) {
      _expectNoVendorKeys(node[i], path: '$path[$i]');
    }
  }
}

void main() {
  group('视图解析（旧服务端兼容）', () {
    test('缺 mode → cloud；local 才 isLocal', () {
      final LlmSettingsView old = LlmSettingsView.fromJson(
        <String, Object?>{'base_url': 'https://api.deepseek.com/v1'},
      );
      expect(old.mode, kLlmModeCloud, reason: '缺字段不能把界面拨到本地');
      expect(old.isLocal, isFalse);

      final LlmSettingsView local = LlmSettingsView.fromJson(
        <String, Object?>{'mode': kLlmModeLocal, 'base_url': 'x'},
      );
      expect(local.mode, kLlmModeLocal);
      expect(local.isLocal, isTrue);
    });
  });

  group('草稿 / PATCH 形状', () {
    test('切模式只发 mode（不预填、不清空地址 / 模型）', () {
      final SettingsDraft d = SettingsDraft();
      d.llmMode = kLlmModeLocal;
      expect(d.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'mode': 'local'},
      });
      expect(d.llmBaseUrl, isNull);
      expect(d.llmModel, isNull);
      expect(d.llmMaxTokens, isNull);
      expect(d.llmShowReasoning, isNull);
    });

    test('PATCH body 不再出现 vendor 键；cloud / local 二级字段正确', () {
      // 一帧补丁里把两档的二级字段都填上：LLM 本地 + TTS 云端。
      final SettingsDraft localCloud = SettingsDraft()
        ..llmMode = kLlmModeLocal
        ..llmBaseUrl = 'http://127.0.0.1:11434/v1'
        ..llmModel = 'qwen2.5:7b'
        ..llmMaxTokens = Tri.set<int>(2048)
        ..llmShowReasoning = Tri.set<bool>(true)
        ..ttsMode = kTtsModeCloud
        ..ttsBaseUrl = 'https://tts.example/v1'
        ..ttsVoice = 'nova'
        ..ttsModel = 'tts-1'
        ..ttsApiKeyEnv = Tri.set<String>(kTtsCloudKeyEnv)
        ..ttsSampleRate = 24000
        ..ttsChannels = 2;
      final Map<String, Object?> body = localCloud.toPatch().toJson();
      expect(body, <String, Object?>{
        'llm': <String, Object?>{
          'mode': kLlmModeLocal,
          'base_url': 'http://127.0.0.1:11434/v1',
          'model': 'qwen2.5:7b',
          'max_tokens': 2048,
          'show_reasoning': true,
        },
        'tts': <String, Object?>{
          'mode': kTtsModeCloud,
          'base_url': 'https://tts.example/v1',
          'api_key_env': kTtsCloudKeyEnv,
          'model': 'tts-1',
          'voice': 'nova',
          'sample_rate': 24000,
          'channels': 2,
        },
      });
      _expectNoVendorKeys(body);
      expect(
        jsonEncode(body).contains('vendor'),
        isFalse,
        reason: '序列化后的请求体里不得出现任何厂商键',
      );

      // 反方向：LLM 云端 + TTS 本地 —— 切模式只改 mode 这一项。
      final SettingsDraft cloudLocal = SettingsDraft()
        ..llmMode = kLlmModeCloud
        ..ttsMode = kTtsModeLocal;
      final Map<String, Object?> simple = cloudLocal.toPatch().toJson();
      expect(simple, <String, Object?>{
        'llm': <String, Object?>{'mode': kLlmModeCloud},
        'tts': <String, Object?>{'mode': kTtsModeLocal},
      });
      _expectNoVendorKeys(simple);
    });

    test('切回服务端本就使用的模式 → 草稿撤回（不白写盘）', () async {
      final SettingsController c = _controller();
      await c.load();
      expect(c.remote!.llm.mode, kLlmModeCloud, reason: '0.3.6 配置 = 云端');

      c.edit((SettingsDraft d) {
        d.llmMode = kLlmModeLocal;
      });
      expect(c.dirty, isTrue, reason: '切到本地是真的改动');

      c.edit((SettingsDraft d) {
        d.llmMode = kLlmModeCloud;
      });
      expect(c.draft.llmMode, isNull, reason: '改回原值 = 没改');
      expect(c.dirty, isFalse);
    });
  });

  group('真泵 LlmSection', () {
    testWidgets('拨到本地：草稿 + Ollama 示例文案；没有厂商下拉', (
      WidgetTester tester,
    ) async {
      final SettingsController c = _controller();
      await c.load();
      await _pump(tester, controller: c);

      // 缺省显示云端（服务端旧配置没有 mode 字段）。
      final SegmentedField<String> seg = tester
          .widget<SegmentedField<String>>(find.byType(SegmentedField<String>));
      expect(seg.value, kLlmModeCloud);
      expect(find.text('模型来源'), findsOneWidget);
      expect(find.text(kLlmCloudNotice), findsOneWidget);
      // 0.3.9 rc.2：厂商下拉已删除——模式二选一是唯一切换手段。
      expect(find.text('当前厂商'), findsNothing);

      await tester.tap(find.text('本地'));
      await tester.pump();

      expect(c.draft.llmMode, kLlmModeLocal, reason: '只进草稿，保存才生效');
      expect(c.draft.llmBaseUrl, isNull, reason: '不预填、不清空地址');
      expect(c.draft.llmModel, isNull);
      expect(c.draft.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'mode': 'local'},
      });

      // 本地模式给 Ollama 示例文案（只是文案，不预填）。
      expect(find.text(kLlmLocalNotice), findsOneWidget);
      expect(find.textContaining(kLlmLocalExampleBaseUrl), findsWidgets);
    });

    testWidgets('本地模式自检入口仍在（只探连通，文案不写试听）', (
      WidgetTester tester,
    ) async {
      int calls = 0;
      final SettingsController c = _controller();
      await c.load();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ListenableBuilder(
                listenable: c,
                builder: (BuildContext context, Widget? child) => LlmSection(
                  controller: c,
                  view: c.remote!,
                  devMode: false,
                  onTest: () async {
                    calls += 1;
                  },
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      // 拨到本地后自检仍在、且说明仍是「只打 /models」。
      await tester.tap(find.text('本地'));
      await tester.pump();
      expect(find.textContaining('只打 /models'), findsWidgets);
      expect(find.textContaining('试听'), findsNothing);

      await tester.ensureVisible(find.text('测试连接'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('测试连接'));
      await tester.pump();
      expect(calls, 1);
    });
  });
}
