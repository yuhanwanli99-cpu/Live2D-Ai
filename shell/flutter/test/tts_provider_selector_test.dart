/// 「模型服务 / 语音合成」的**出声分档**选择器（2026-11-16）。
///
/// # 这一版守什么
///
/// ① 缺省仍是 `openai`（本地 / 云端二选一照旧在），**旧服务端不返回
///    `provider` 时不能把界面拨到 DeepSeek 上**；
/// ② 选 DeepSeek：`base_url` 的输入框消失（端点固定，画了也不能改），
///    音色变成**两项下拉**（stella / tide），规格显示 24 kHz / 单声道；
/// ③ 选 DeepSeek 会把音色草稿摆到白名单里（服务端也会拒，这里只是让用户
///    点保存前就看到合法值）；
/// ④ 切回本地：地址/音色/规格回到原来那一套（不因为选过 DeepSeek 就丢值）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/tts_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

SettingsView _view({
  String provider = kTtsProviderOpenAi,
  String mode = kTtsModeLocal,
  String voice = 'ZH',
}) => SettingsView.fromJson(<String, Object?>{
  'tts': <String, Object?>{
    'mode': mode,
    'provider': provider,
    'base_url': 'http://127.0.0.1:8091/v1',
    'model': null,
    'voice': voice,
    'has_api_key': false,
    'sample_rate': 44100,
    'channels': 1,
  },
});

SettingsController _controller() =>
    SettingsController(api: ApiClient(base: 'http://127.0.0.1:18080'));

Future<void> _pump(
  WidgetTester tester, {
  required SettingsController controller,
  required SettingsView view,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          child: ListenableBuilder(
            listenable: controller,
            builder: (BuildContext context, Widget? child) => TtsSection(
              controller: controller,
              view: view,
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
  testWidgets('缺省 provider=openai：本地/云端二选一照旧，不画 DeepSeek 下拉', (
    WidgetTester tester,
  ) async {
    final SettingsController controller = _controller();
    await _pump(tester, controller: controller, view: _view());

    // 分档选择器在（一段 SegmentedButton，两项）。
    expect(find.text('语音合成来源'), findsOneWidget);
    expect(find.text('本地（OpenAI 兼容）'), findsOneWidget);
    expect(find.text('DeepSeek'), findsOneWidget);

    // 缺省那一段仍是「语音来源」本地 / 云端。
    expect(find.text('语音来源'), findsOneWidget);
    expect(find.text('云端'), findsOneWidget);

    // 音色下拉**只在 DeepSeek 档出现**。
    expect(find.widgetWithText(DropdownField<String>, '音色（voice_id）'), findsNothing);
    expect(controller.draft.ttsProvider, isNull, reason: '默认档不该产生草稿');
  });

  testWidgets('选 DeepSeek：地址输入框消失、音色两项下拉、规格 24000/单声道', (
    WidgetTester tester,
  ) async {
    final SettingsController controller = _controller();
    await _pump(tester, controller: controller, view: _view());

    await tester.tap(find.text('DeepSeek'));
    await tester.pump();

    // 草稿记下了新分档（保存时才会下发）。
    expect(controller.draft.ttsProvider, kTtsProviderDeepSeek);

    // 端点固定：只读展示，且**没有**任何地址输入框。
    expect(find.widgetWithText(ReadonlyField, '服务地址'), findsOneWidget);
    expect(find.textContaining('api.deepseek.com/v1/tools/tts'), findsOneWidget);
    expect(
      find.widgetWithText(TextFieldRow, '服务地址'),
      findsNothing,
      reason: 'DeepSeek 档不读 base_url，画输入框只会误导',
    );

    // 音色是两项下拉（不是自由文本）。
    expect(
      find.widgetWithText(DropdownField<String>, '音色（voice_id）'),
      findsOneWidget,
    );
    expect(find.widgetWithText(TextFieldRow, '音色（voice）'), findsNothing);
    final DropdownButton<String> dropdown = tester.widget<DropdownButton<String>>(
      find.byType(DropdownButton<String>),
    );
    expect(
      dropdown.items!.map((DropdownMenuItem<String> i) => i.value).toList(),
      <String>['stella', 'tide'],
    );
    // 草稿音色被摆到白名单里（原来是 ZH，不是合法值）。
    expect(controller.draft.ttsVoice, kDeepSeekDefaultVoice);

    // 规格只读串：24 kHz / 单声道。
    expect(find.textContaining('24000 Hz'), findsOneWidget);
    expect(find.textContaining('44100 Hz'), findsNothing);

    // 「语音来源」本地 / 云端那一段在 DeepSeek 档不出现（端点固定，没有本地/云端之分）。
    expect(find.text('语音来源'), findsNothing);
  });

  testWidgets('切回本地：原来那一套值还在（选过 DeepSeek 不丢用户配置）', (
    WidgetTester tester,
  ) async {
    final SettingsController controller = _controller();
    await _pump(tester, controller: controller, view: _view());

    await tester.tap(find.text('DeepSeek'));
    await tester.pump();
    await tester.tap(find.text('本地（OpenAI 兼容）'));
    await tester.pump();

    expect(controller.draft.ttsProvider, kTtsProviderOpenAi);
    // 地址与规格回到 OpenAI 兼容档的那一套（草稿里没被清掉）。
    expect(find.text('语音来源'), findsOneWidget);
    expect(find.textContaining('127.0.0.1:8091'), findsOneWidget);
    expect(find.widgetWithText(ReadonlyField, '音色（voice）'), findsOneWidget);
  });

  testWidgets('服务端本来就报 deepseek：界面直接是那一档（草稿为空）', (
    WidgetTester tester,
  ) async {
    final SettingsController controller = _controller();
    await _pump(
      tester,
      controller: controller,
      view: _view(provider: kTtsProviderDeepSeek, voice: 'tide'),
    );

    expect(
      find.widgetWithText(DropdownField<String>, '音色（voice_id）'),
      findsOneWidget,
    );
    final DropdownButton<String> dropdown = tester.widget<DropdownButton<String>>(
      find.byType(DropdownButton<String>),
    );
    expect(dropdown.value, 'tide', reason: '要显示服务端真正的音色，不能回落');
    expect(controller.draft.ttsProvider, isNull);
  });
}
