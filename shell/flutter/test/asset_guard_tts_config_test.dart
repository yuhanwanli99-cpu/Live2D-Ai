/// A1 资产守护网（A5 接触点）：TTS 配置面（settings/sections/tts_section.dart）。
///
/// # 为什么这里是**行为**断言（能行为化就必须行为化）
///
/// TtsSection 是普通 Flutter widget（零 package:web），在 flutter test 里能
/// 直接挂载、点击、输入 ⇒ 全部按行为断言写，不用源码扫描。
///
/// # 本次新增守的缺口（已存在 vs 新增）
///
/// 既有 test/settings_controller_test.dart 与 test/settings_api_test.dart 守的是
/// **草稿 / PATCH 形状**，没有一条测这个 **widget 面本身**；而这两个测试文件
/// **都在 29 个重放重叠清单里**（/tmp/overlap-real.txt）——重放若直接覆盖它们，
/// 连草稿层的守卫一起丢。所以 A5 的配置面需要一条不依赖那两个文件的守卫。
///
/// 本文件钉的既有红线条目：
/// 1. TTS 是**核心链路**、唯一权威配置面（base_url / voice / model / 音频规格）
///    四项都要在（缺一项 = 配置面被旧版覆盖）；
/// 2. **服务端静音是只读观测值，绝不是第二个开关**（ts_section.dart 头注第 1 条）
///    ——断言整段里没有任何 Switch；
/// 3. PCM 规格只在 devMode 出现（渐进披露）；
/// 4. 连通性自检的确是一个按钮、会回调，且文案不宣传成「试听」（不合成）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/tts_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 服务端真相的一份最小快照（只给 tts 段，其余走 fromJson 默认）。
SettingsView _view({
  String mode = kTtsModeLocal,
  String baseUrl = 'http://127.0.0.1:8080/v1',
  String voice = 'skystar',
  String? model,
  int sampleRate = 24000,
  int channels = 1,
}) => SettingsView.fromJson(<String, Object?>{
  'tts': <String, Object?>{
    'mode': mode,
    'base_url': baseUrl,
    'model': model,
    'voice': voice,
    'has_api_key': false,
    'sample_rate': sampleRate,
    'channels': channels,
  },
});

SettingsController _controller() =>
    SettingsController(api: ApiClient(base: 'http://127.0.0.1:18080'));

Future<void> _pump(
  WidgetTester tester, {
  required SettingsController controller,
  required SettingsView view,
  bool devMode = false,
  bool serverMuted = false,
  Future<void> Function()? onTest,
  // 2026-10-01（W1-b / F-0012-1）：`TtsSection.testResult` 收成
  // `FieldTestResult`（服务端 `ok` + 文案），这里只是转发参数，语义不变。
  FieldTestResult? testResult,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          // 2026-10-09：真宿主（SettingsScaffold）会监听 controller 重建；
          // 这里补上同一条线，否则「拨云端/本地」之后界面不会跟着换。
          child: ListenableBuilder(
            listenable: controller,
            builder: (BuildContext context, Widget? child) => TtsSection(
              controller: controller,
              view: view,
              devMode: devMode,
              serverMuted: serverMuted,
              onTest: onTest,
              testResult: testResult,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

/// 某个 FieldRow 里的输入框（按行标签定位）。
Finder _fieldOf(String label) => find.descendant(
  of: find.widgetWithText(TextFieldRow, label),
  matching: find.byType(TextField),
);

void main() {
  testWidgets('A5：TTS 配置面都在（服务地址 / 音色 / 模型名 / 音频规格）；缺省本地', (
    WidgetTester tester,
  ) async {
    await _pump(tester, controller: _controller(), view: _view(model: null));

    // 2026-10-09：**缺省本地** —— 端口固定，普通层不给改 ⇒ 只读展示，
    // 连输入框都不画。
    expect(find.text('语音来源'), findsOneWidget);
    expect(find.widgetWithText(ReadonlyField, '服务地址'), findsOneWidget);
    expect(find.textContaining('127.0.0.1:8091'), findsOneWidget);
    expect(
      find.widgetWithText(TextFieldRow, '服务地址'),
      findsNothing,
      reason: '本地模式的端口固定，普通层不给改',
    );
    expect(
      find.textContaining('base_url'),
      findsNothing,
      reason: '2026-10-08：产品面上不出现 base_url（标签与说明都不带）',
    );
    expect(find.widgetWithText(ReadonlyField, '音色（voice）'), findsOneWidget);
    expect(
      find.widgetWithText(ReadonlyField, '音频规格'),
      findsOneWidget,
      reason: '采样率/声道数是只读展示（改它后果严重，只在 devMode 可改）',
    );
    expect(find.textContaining('44100 Hz'), findsOneWidget);
    // 本地**不带密钥**（只读状态行）。
    expect(find.widgetWithText(ReadonlyField, '密钥绑定'), findsOneWidget);
    expect(find.textContaining('不带密钥'), findsOneWidget);

    // 切到**云端**：三个可填字段与密钥绑定才出现（地址/音色/模型名由用户填）。
    await tester.tap(find.text('云端'));
    await tester.pump();
    expect(find.widgetWithText(TextFieldRow, '服务地址'), findsOneWidget);
    expect(find.widgetWithText(TextFieldRow, '音色（voice）'), findsOneWidget);
    expect(find.widgetWithText(TextFieldRow, '模型名（可空）'), findsOneWidget);
    expect(find.textContaining('base_url'), findsNothing);
  });

  testWidgets('A5：服务端静音只在开发者模式里画（2026-10-08）', (
    WidgetTester tester,
  ) async {
    // 没开开发者模式：整行不画——它的正文要点出 LIVE2D_AI_MUTE_AUDIO。
    await _pump(
      tester,
      controller: _controller(),
      view: _view(),
      devMode: false,
      serverMuted: true,
    );
    expect(find.widgetWithText(ReadonlyField, '服务端静音'), findsNothing);
    expect(find.textContaining('LIVE2D_AI_MUTE_AUDIO'), findsNothing);

    // 打开开发者模式：原样出现（能力没删，只是挪进第二层）。
    await _pump(
      tester,
      controller: _controller(),
      view: _view(),
      devMode: true,
      serverMuted: true,
    );

    expect(find.widgetWithText(ReadonlyField, '服务端静音'), findsOneWidget);
    expect(find.textContaining('静音中'), findsOneWidget);
    expect(
      find.byType(Switch),
      findsNothing,
      reason:
          'ts_section.dart 头注第 1 条：绝不能把「服务端静音」做成第二个开关——'
          '否则「我能听到」与「有没有声音被生产出来」两个轴同名。',
    );
    expect(find.byType(SwitchListTile), findsNothing);
  });

  testWidgets('A5：PCM 规格只在 devMode 出现（渐进披露）', (WidgetTester tester) async {
    await _pump(tester, controller: _controller(), view: _view(), devMode: false);
    expect(find.text('采样率（Hz）'), findsNothing);
    expect(find.text('声道数'), findsNothing);

    await _pump(tester, controller: _controller(), view: _view(), devMode: true);
    expect(find.text('采样率（Hz）'), findsOneWidget);
    expect(find.text('声道数'), findsOneWidget);
  });

  testWidgets('A5：连通性自检是按钮、会回调，文案不宣传成「试听」', (
    WidgetTester tester,
  ) async {
    int calls = 0;
    await _pump(
      tester,
      controller: _controller(),
      view: _view(),
      onTest: () async {
        calls += 1;
      },
    );

    final FieldActionRow row = tester.widget<FieldActionRow>(
      find.byType(FieldActionRow),
    );
    expect(row.actionLabel, '测试连接');
    // 2026-10-09：控件说明全删 ⇒ 这一行不再挂 description。
    // 「自检只探连通、不合成」仍成立——它是**行为**（本用例下面的回调断言 + 服务端
    // 侧的 /models 探针），不再靠界面文案表达。
    expect(row.description, isNull, reason: '三级功能介绍已删（说明不再当契约）');
    expect(find.textContaining('试听'), findsNothing);

    // 配置面比 600px 高的测试视口长，先滚到按钮再点（不是「点不着还硬点」）。
    await tester.ensureVisible(find.text('测试连接'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('测试连接'));
    await tester.pump();
    expect(calls, 1, reason: '点一次必须回调一次（自检入口真的接上了）');
  });

  testWidgets('A5：云端模式下编辑 address / voice / model 只落进 tts 草稿，不碰别的段', (
    WidgetTester tester,
  ) async {
    final SettingsController controller = _controller();
    await _pump(tester, controller: controller, view: _view());

    // 先拨到云端（缺省本地；本地那三个字段是只读的，本来就没输入框）。
    await tester.tap(find.text('云端'));
    await tester.pump();
    expect(
      controller.draft.ttsMode,
      kTtsModeCloud,
      reason: '拨到云端必须落进草稿（保存时才发得出去）',
    );

    await tester.enterText(
      _fieldOf('服务地址'),
      'https://tts.example/v1',
    );
    await tester.enterText(_fieldOf('音色（voice）'), 'nova');
    await tester.enterText(_fieldOf('模型名（可空）'), 'tts-1');
    await tester.pump();

    expect(controller.draft.ttsBaseUrl, 'https://tts.example/v1');
    expect(controller.draft.ttsVoice, 'nova');
    expect(controller.draft.ttsModel, 'tts-1');
    // 不能顺手把别的段写脏（草稿按段提交：没碰的段必须留在 null）。
    expect(controller.draft.llmBaseUrl, isNull);
    expect(controller.draft.llmModel, isNull);
    expect(controller.draft.ttsSampleRate, isNull);
    expect(controller.draft.ttsChannels, isNull);
  });
}
