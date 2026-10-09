/// director 产品面板回归（2026-10-08 再瘦身版）。
///
/// 覆盖：
/// ① 卡片上**只剩一句话**（[kDirectorTakeoverNotice]），旧文案与决策词一个都不在；
/// ② 8 个配置键经 [ModPanel.hiddenKeys] **整体不渲染**（也不收进「高级」）；
/// ③ 启用开关仍在（它是唯一的用户开关）；
/// ④ 纯函数（情绪 / 意图 / 预设 / 数值文案）仍供开发者面使用。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/director_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 与 Rust `director_settings_spec()` **逐键逐 label 对应**的 8 个字段。
ModSettingsSpec directorSpec() => const ModSettingsSpec(
  modId: 'director',
  title: '导演',
  version: 2,
  fields: <ModSettingField>[
    ModSettingField(
      kind: ModFieldKind.select,
      key: 'preset_happy',
      label: '开心 → 动作',
      defaultValue: 'smile',
      options: <ModSelectOption>[
        ModSelectOption(value: 'none', label: '不投递'),
        ModSelectOption(value: 'smile', label: '微笑'),
      ],
    ),
    ModSettingField(
      kind: ModFieldKind.select,
      key: 'preset_sad',
      label: '难过 → 动作',
      defaultValue: 'unhappy',
      options: <ModSelectOption>[
        ModSelectOption(value: 'none', label: '不投递'),
      ],
    ),
    ModSettingField(
      kind: ModFieldKind.select,
      key: 'preset_greeting',
      label: '打招呼 → 动作',
      defaultValue: 'nod',
      options: <ModSelectOption>[
        ModSelectOption(value: 'none', label: '不投递'),
      ],
    ),
    ModSettingField(
      kind: ModFieldKind.bool,
      key: 'staging_enabled',
      label: '二路 LLM 异步 cue（不是接管开关；接管后本项不再发请求）',
      defaultValue: false,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'staging_base_url',
      label: '二路端点 base_url（留空则用对话模型；如 http://127.0.0.1:11434/v1）',
      secret: false,
      defaultValue: null,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'staging_model',
      label: '二路模型名（留空则用对话模型）',
      secret: false,
      defaultValue: null,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'staging_api_key_env',
      label: '二路密钥变量名（.env 里的名字；留空则用对话模型的密钥）',
      secret: false,
      defaultValue: null,
    ),
    ModSettingField(
      kind: ModFieldKind.number,
      key: 'staging_timeout_ms',
      label: '二路超时（毫秒，100~5000）',
      min: 100,
      max: 5000,
    ),
  ],
);

ModPanelContext ctxFor({bool enabled = true}) => ModPanelContext(
  mod: ModInfo(
    id: 'director',
    name: '导演',
    version: '0.1.0',
    apiVersion: 1,
    enabled: enabled,
    status: enabled ? 'running' : 'disabled',
  ),
  state: null,
  stateLoading: false,
  stateError: null,
  onRefreshState: () async {},
  onCommand:
      (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
          const ModCommandResult(ok: true, result: <String, Object?>{}),
);

Widget wrapPanel(ModPanelContext ctx) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: Builder(
        builder: (BuildContext context) =>
            const DirectorPanel().build(context, ctx)!,
      ),
    ),
  ),
);

void main() {
  group('① 只剩一句话', () {
    testWidgets('面板渲染那句话；决策词与旧文案一个都不在', (WidgetTester tester) async {
      await tester.pumpWidget(wrapPanel(ctxFor()));
      await tester.pumpAndSettle();

      expect(find.text(kDirectorTakeoverNotice), findsOneWidget);
      expect(
        find.text('打开后，说话时会带上表情和轻微的头、颈动作。'),
        findsOneWidget,
      );

      for (final String banned in <String>[
        '接管状态',
        '规则预设',
        '实际下发',
        '本轮预设',
        '依据',
        '清空决策账本',
        '先启用并聊一轮',
        '运行态暂时读不到',
        'command_unavailable',
      ]) {
        expect(find.textContaining(banned), findsNothing, reason: '不得出现「$banned」');
      }
    });

    testWidgets('未启用时同样只有那一句话（不再有清空按钮与其失败码）', (WidgetTester tester) async {
      await tester.pumpWidget(wrapPanel(ctxFor(enabled: false)));
      await tester.pumpAndSettle();

      expect(find.text(kDirectorTakeoverNotice), findsOneWidget);
      expect(find.byType(OutlinedButton), findsNothing);
      expect(find.textContaining('503'), findsNothing);
    });
  });

  group('② 8 个配置键产品面不渲染（也不进「高级」）', () {
    test('面板声明的隐藏键恰好是那 8 个', () {
      const DirectorPanel panel = DirectorPanel();
      expect(panel.modId, 'director');
      expect(panel.hiddenKeys, <String>{
        'preset_happy',
        'preset_sad',
        'preset_greeting',
        'staging_enabled',
        'staging_base_url',
        'staging_model',
        'staging_api_key_env',
        'staging_timeout_ms',
      });
      expect(panel.hiddenKeys.length, 8);
    });

    testWidgets('真泵 ModsSection：8 个 label 一个都不上屏，「高级」也不出现', (
      WidgetTester tester,
    ) async {
      final ModInfo mod = ModInfo(
        id: 'director',
        name: '导演',
        version: '0.1.0',
        apiVersion: 1,
        enabled: true,
        status: 'running',
        config: const <String, Object?>{'preset_happy': 'smile'},
        settingsSpec: directorSpec(),
      );
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[mod],
                loading: false,
                onLoadState: (String id) async =>
                    ModStateResult(id: id, enabled: true, state: const <String, Object?>{}),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('导演'));
      await tester.pumpAndSettle();

      // 一句话在。
      expect(find.text(kDirectorTakeoverNotice), findsOneWidget);
      // 8 个 label 一个都不在（含「高级」折叠——它整块不该出现）。
      for (final ModSettingField f in directorSpec().fields) {
        expect(find.text(f.label), findsNothing, reason: '${f.key} 不得上屏');
      }
      expect(find.text('高级'), findsNothing);
      expect(find.textContaining('二路'), findsNothing);
      expect(find.textContaining('→ 动作'), findsNothing);
      // ③ 启用开关仍在（唯一的用户开关）。
      expect(find.byType(Switch), findsOneWidget);
      // 通用运行态兜底块不渲染（有专用面板）。
      expect(find.text('运行态（只读）'), findsNothing);
    });
  });

  group('④ 纯函数仍供开发者面使用', () {
    test('预设 / 情绪 / 意图 / 数值文案', () {
      expect(directorPresetText('unhappy'), '表情（unhappy）');
      expect(directorPresetText('tilt_left'), '短动作（tilt_left）');
      expect(directorPresetText('none'), '不投递（本轮无动作）');
      expect(directorPresetText(null), '不投递（本轮无动作）');
      expect(directorEmotionText('happy'), '开心（happy）');
      expect(directorEmotionText('bogus'), 'bogus');
      expect(directorIntentText('greeting'), '打招呼（greeting）');
      expect(directorNumberText(1), '1');
      expect(directorNumberText(1.2), '1.20');
      expect(directorNumberText(null), '—');
    });
  });
}
