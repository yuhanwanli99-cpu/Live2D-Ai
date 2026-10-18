/// 0.3.7 T1：LLM 云端 / 本地的**草稿与交互**回归（纯逻辑 + 真泵）。
///
/// 守四件事：
/// 1. 旧服务端缺 `mode` → 按云端理解（不能把界面拨到本地那套配置）；
/// 2. 草稿切模式只发 `mode`（服务端普通可选值），不预填/不清空地址 / 模型；
/// 3. 与 TTS 的 `_switchMode` 逐条对齐：切模式 = 清厂商**标签**（`Tri.clear`），
///    服务端只清标签、不动快照——界面里快照仍在、仍可选；
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
const String _getJson = '''
{"llm":{"base_url":"https://api.deepseek.com/v1","model":"deepseek-flash",
"has_api_key":true,"max_tokens":4096,"show_reasoning":false,
"vendor":"foo","vendor_modified":false,
"vendors":{"foo":{"base_url":"https://api.example.com/v1","model":"foo-large",
"has_api_key":true,"max_tokens":4096,"show_reasoning":false}}},
"tts":{"mode":"local","base_url":"http://127.0.0.1:8091/v1","model":null,
"voice":"ZH","has_api_key":false,"sample_rate":44100,"channels":1},
"persona":{"system_prompt":"","max_history_pairs":0},
"action":{"head_scale":0.75,"body_scale":0.8,"expression_scale":1.0,"models":{}},
"dev_mode":false,"active_model_id":""}
''';

/// 服务端真相（可去掉 / 换掉厂商标签）。
String _getBody({String? vendor = 'foo'}) {
  final Map<String, Object?> j =
      (jsonDecode(_getJson) as Map<String, Object?>);
  final Map<String, Object?> llm =
      (j['llm']! as Map<Object?, Object?>).cast<String, Object?>();
  if (vendor == null) {
    llm.remove('vendor');
  } else {
    llm['vendor'] = vendor;
  }
  return jsonEncode(j);
}

http.Response _jsonResponse(String body) => http.Response.bytes(
  utf8.encode(body),
  200,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

SettingsController _controller({String? vendor = 'foo'}) {
  final String body = _getBody(vendor: vendor);
  final MockClient client = MockClient(
    (http.Request request) async => _jsonResponse(body),
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

    test('视图里的厂商快照不含 mode（快照语义不变）', () {
      final SettingsView v = SettingsView.fromJson(
        <String, Object?>{
          'llm': <String, Object?>{
            'mode': kLlmModeLocal,
            'base_url': 'http://127.0.0.1:11434/v1',
            'model': 'qwen2.5:7b',
            'vendors': <String, Object?>{
              'foo': <String, Object?>{'base_url': 'https://api.example.com/v1'},
            },
          },
        },
      );
      expect(v.llm.mode, kLlmModeLocal);
      expect(v.llm.vendors.containsKey('foo'), isTrue, reason: '切模式不动快照');
    });
  });

  group('草稿 / PATCH 形状', () {
    test('切模式只发 mode；与清标签同发（界面实际形状）', () {
      final SettingsDraft d = SettingsDraft();
      d.llmMode = kLlmModeLocal;
      expect(d.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'mode': 'local'},
      });
      d.llmVendor = Tri.clear<String>();
      expect(d.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'mode': 'local', 'vendor': null},
      });
      // 不预填、不清空地址 / 模型（LLM 的本地形态没有冻结地址）。
      expect(d.llmBaseUrl, isNull);
      expect(d.llmModel, isNull);
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
    testWidgets('拨到本地：草稿 + Ollama 示例文案；厂商快照仍在、仍可选', (
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

      await tester.tap(find.text('本地'));
      await tester.pump();

      expect(c.draft.llmMode, kLlmModeLocal, reason: '只进草稿，保存才生效');
      expect(
        c.draft.llmVendor,
        isA<TriClear<String>>(),
        reason: '与 TTS 同款：切模式只清厂商标签（不删快照）',
      );
      expect(c.draft.llmBaseUrl, isNull, reason: '不预填、不清空地址');
      expect(c.draft.llmModel, isNull);
      expect(c.draft.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'mode': 'local', 'vendor': null},
      });

      // 本地模式给 Ollama 示例文案（只是文案，不预填）。
      expect(find.text(kLlmLocalNotice), findsOneWidget);
      expect(find.textContaining(kLlmLocalExampleBaseUrl), findsWidgets);

      // 厂商快照**没被删**：视图里仍有 foo，下拉的可选项里仍能选到它。
      expect(c.remote!.llm.vendors.containsKey('foo'), isTrue);
      final DropdownField<String> vendorField = tester
          .widget<DropdownField<String>>(find.byType(DropdownField<String>));
      expect(
        vendorField.options.any((FieldOption<String> o) => o.value == 'foo'),
        isTrue,
        reason: '切模式只清标签，不删快照——快照仍要可选',
      );
    });

    testWidgets('切厂商仍是「快照→当前」：只记 vendor 草稿，模式草稿让位', (
      WidgetTester tester,
    ) async {
      // 服务端没有标签（手动填写）；快照 foo 在。
      final SettingsController c = _controller(vendor: null);
      await c.load();
      await _pump(tester, controller: c);

      expect(llmVendorOf(c.draft, c.remote!.llm), '');

      await tester.tap(find.byType(DropdownButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('foo').last);
      await tester.pumpAndSettle();

      expect(c.draft.llmVendor, isA<TriSet<String>>());
      expect(
        (c.draft.llmVendor! as TriSet<String>).value,
        'foo',
        reason: '选厂商 = 服务端一次快照→当前复制（既有 rc.6 语义）',
      );
      expect(c.draft.llmMode, isNull, reason: '切厂商不顺手改模式草稿');
      expect(c.draft.toPatch().toJson(), <String, Object?>{
        'llm': <String, Object?>{'vendor': 'foo'},
      });
    });

    testWidgets('本地模式自检入口仍在（只探连通，文案不写试听）', (
      WidgetTester tester,
    ) async {
      int calls = 0;
      final SettingsController c = _controller(vendor: null);
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
