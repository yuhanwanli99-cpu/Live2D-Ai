/// voice-input 产品面板回归（2026-10-08 二次口径）。
///
/// # 这一版守什么
///
/// ① 卡片上**只剩一句话**：点「听」→ 说完再点一次 → 字进输入框；
/// ② 9 个配置键全部进 [ModPanel.devKeys]：`devMode == false` 时一个都不画，
///    `devMode == true` 时按普通表单全部画出来（**不塞进「高级」**）；
/// ③ 两个键的说明来自 [ModPanel.fieldHelp]（不是协议字段），且不再写「按住听」；
/// ④ 旧文案与错误码在**产品卡片上一个都找不到**（本文件逐条钉住）。
///
/// 面板自己不做网络：所有动作都经 ModPanelContext.onCommand 注入的 fake。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/voice_input_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 与 Rust `voice_input_settings_spec()` **逐键逐 label 对应**的 9 个字段。
ModSettingsSpec voiceSpec() => const ModSettingsSpec(
  modId: 'voice-input',
  title: '语音输入',
  version: 3,
  fields: <ModSettingField>[
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'wake_phrase',
      label: '唤醒词',
      defaultValue: '小可爱',
    ),
    ModSettingField(
      kind: ModFieldKind.bool,
      key: 'manual_enabled',
      label: '按住说话',
      defaultValue: true,
    ),
    ModSettingField(
      kind: ModFieldKind.select,
      key: 'backend',
      label: '识别后端',
      options: <ModSelectOption>[
        ModSelectOption(value: 'mock', label: 'mock'),
      ],
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'locale',
      label: '文本归一化语言（BCP-47）',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'token',
      label: '访问令牌（空 = 不鉴权）',
      secret: true,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'sidecar_script',
      label: 'sidecar 脚本路径（空 = 仓库默认）',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'sidecar_url',
      label: 'sidecar POST 的 URL（空 = 命令参数给）',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'sidecar_transcriber',
      label: 'ASR 命令（fake = 读同名 .txt）',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'sidecar_python',
      label: 'Python 解释器（缺省 python3）',
    ),
  ],
);

ModInfo voiceMod() => ModInfo(
  id: 'voice-input',
  name: '语音输入',
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config: const <String, Object?>{'wake_phrase': '小可爱'},
  settingsSpec: voiceSpec(),
);

ModPanelContext ctxFor({bool enabled = true, bool devMode = false}) =>
    ModPanelContext(
      mod: ModInfo(
        id: 'voice-input',
        name: '语音输入',
        version: '0.1.0',
        apiVersion: 1,
        enabled: enabled,
        status: enabled ? 'running' : 'disabled',
      ),
      state: null,
      stateLoading: false,
      stateError: null,
      devMode: devMode,
      onRefreshState: () async {},
      onCommand:
          (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
              const ModCommandResult(ok: true),
    );

Widget wrapPanel(ModPanelContext ctx) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: Builder(
        builder: (BuildContext context) =>
            const VoiceInputPanel().build(context, ctx)!,
      ),
    ),
  ),
);

/// 真泵整张 Mod 卡片（通用表单 + 产品面板）——本文件的主断言面。
Widget wrapCard({bool devMode = false}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: ModsSection(
        mods: <ModInfo>[voiceMod()],
        loading: false,
        devMode: devMode,
        onLoadState: (String id) async =>
            ModStateResult(id: id, enabled: true, state: const <String, Object?>{}),
      ),
    ),
  ),
);

/// 产品卡片/面板上**一个都不许出现**的词（计划书 §提示词 + 各节）。
const List<String> kBannedOnVoiceCard = <String>[
  '总闸',
  '手动闸',
  '唤醒词（听到它才开始听',
  '403',
  '503',
  'command_unavailable',
  'voice_gate_closed',
  'voice_manual_off',
  'sidecar',
  'ASR',
  'BCP-47',
  'backend',
  'locale',
  'token',
  '高级',
  '验证闸门',
  '退出码',
  '检查配置',
  '自检',
  '按住听',
];

void main() {
  group('① 卡片只剩一句话', () {
    testWidgets('面板渲染那句话；旧的闸门 / 协议 / 错误码一个都不在', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrapPanel(ctxFor()));
      await tester.pumpAndSettle();

      expect(find.textContaining('点聊天栏的「听」'), findsOneWidget);
      expect(
        find.textContaining('说完再点一次，字会出现在输入框里'),
        findsOneWidget,
      );
      for (final String banned in kBannedOnVoiceCard) {
        expect(
          find.textContaining(banned),
          findsNothing,
          reason: '产品卡片上不得出现「$banned」',
        );
      }
      // 面板本体不再有按钮（启用开关在卡片标题行，由 ModsSection 画）。
      expect(find.byType(FilledButton), findsNothing);
      expect(find.byType(OutlinedButton), findsNothing);
      expect(find.byType(TextField), findsNothing);
    });

    testWidgets('未启用时同样只有那一句话（不再有「命令会回 503」这类闲置说明）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrapPanel(ctxFor(enabled: false)));
      await tester.pumpAndSettle();
      expect(find.textContaining('点聊天栏的「听」'), findsOneWidget);
      expect(find.textContaining('503'), findsNothing);
      expect(find.textContaining('command_unavailable'), findsNothing);
    });
  });

  group('② 9 个键进 devKeys（产品面不画，开发模式照画，都不进「高级」）', () {
    test('devKeys 恰好是与 Rust spec 对应的那 9 个；hiddenKeys 空', () {
      const VoiceInputPanel panel = VoiceInputPanel();
      expect(panel.modId, 'voice-input');
      expect(panel.devKeys, <String>{
        'wake_phrase',
        'manual_enabled',
        'backend',
        'locale',
        'token',
        'sidecar_script',
        'sidecar_url',
        'sidecar_transcriber',
        'sidecar_python',
      });
      expect(panel.devKeys.length, 9);
      expect(panel.hiddenKeys, isEmpty);
      expect(panel.advancedKeys, isEmpty);
    });

    test('fieldHelp 已退役（2026-10-09：三级功能介绍全删）', () {
      const VoiceInputPanel panel = VoiceInputPanel();
      expect(panel.fieldHelp, isEmpty);
      // 两个键的常量仍在（它们也是别处的引用面），只是不再配说明。
      expect(kVoiceWakePhraseKey, isNotEmpty);
      expect(kVoiceManualKey, isNotEmpty);
    });

    test('stateLabels 里不再有「总闸 / 手动闸」这类内部名', () {
      const VoiceInputPanel panel = VoiceInputPanel();
      for (final String label in panel.stateLabels.values) {
        for (final String banned in <String>['总闸', '手动闸', 'sidecar', 'ASR']) {
          expect(label.contains(banned), isFalse, reason: '「$label」含「$banned」');
        }
      }
    });

    testWidgets('产品面保存不覆盖 devKeys：既有值原样带走（不是 spec 默认值）', (
      WidgetTester tester,
    ) async {
      Map<String, Object?>? saved;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[
                  ModInfo(
                    id: 'voice-input',
                    name: '语音输入',
                    version: '0.1.0',
                    apiVersion: 1,
                    enabled: true,
                    status: 'running',
                    // 用户手写进 mods.json 的部署细节：保存一次不许被默认值冲掉。
                    config: const <String, Object?>{
                      'wake_phrase': '小可爱',
                      'backend': 'sidecar',
                      'locale': 'zh-CN',
                      'token': 'sekret',
                      'sidecar_script': '/opt/vs.py',
                      'sidecar_url': 'http://127.0.0.1:18080/api/v1/voice/transcript',
                      'sidecar_transcriber': 'cmd:whisper',
                      'sidecar_python': 'python3.12',
                    },
                    settingsSpec: voiceSpec(),
                  ),
                ],
                loading: false,
                onLoadState: (String id) async => ModStateResult(
                  id: id,
                  enabled: true,
                  state: const <String, Object?>{},
                ),
                onSaveConfig: (String id, Map<String, Object?> config) async {
                  saved = config;
                  return const ModConfigResult(ok: true);
                },
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('语音输入'));
      await tester.pumpAndSettle();
      final Finder save = find.text('保存并应用');
      await tester.ensureVisible(save);
      await tester.pumpAndSettle();
      await tester.tap(save);
      await tester.pumpAndSettle();

      expect(saved, isNotNull);
      for (final MapEntry<String, Object?> e in const <String, Object?>{
        'backend': 'sidecar',
        'locale': 'zh-CN',
        'token': 'sekret',
        'sidecar_script': '/opt/vs.py',
        'sidecar_transcriber': 'cmd:whisper',
        'sidecar_python': 'python3.12',
      }.entries) {
        expect(saved![e.key], e.value, reason: 'devKeys 里的 ${e.key} 被保存改写了');
      }
    });

    testWidgets('产品面（devMode=false）：9 个键一个都不画', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrapCard());
      await tester.tap(find.text('语音输入'));
      await tester.pumpAndSettle();
      expect(find.text('唤醒词'), findsNothing);
      expect(find.widgetWithText(TextFieldRow, '唤醒词'), findsNothing);
      for (final ModSettingField f in voiceSpec().fields) {
        expect(find.text(f.label), findsNothing, reason: '${f.key} 产品面不得上屏');
      }
      // 只有卡片标题行的启用开关。
      expect(find.byType(Switch), findsOneWidget);
      expect(find.text('高级'), findsNothing);
    });

    testWidgets('开发模式（devMode=true）：9 个键全画，仍不进「高级」', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrapCard(devMode: true));
      await tester.tap(find.text('语音输入'));
      await tester.pumpAndSettle();
      expect(find.widgetWithText(TextFieldRow, '唤醒词'), findsOneWidget);
      expect(find.widgetWithText(ToggleField, '按住说话'), findsOneWidget);
      // 9 个键全在普通表单里（没有「高级」折叠）。
      for (final ModSettingField f in voiceSpec().fields) {
        expect(find.text(f.label), findsOneWidget, reason: '${f.key} 应上屏');
      }
      expect(find.text('高级'), findsNothing);
      // 两个 Switch：卡片标题行的启用开关 + 「按住说话」。
      expect(find.byType(Switch), findsNWidgets(2));
    });
  });
}
