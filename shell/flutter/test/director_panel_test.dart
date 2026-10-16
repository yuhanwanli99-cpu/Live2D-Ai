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

/// 与 Rust `director_settings_spec()` **逐键逐 label 对应**的 9 个字段。
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
      key: 'motion_bias',
      label: '表演倾向（保存并重载后生效）',
      defaultValue: 'normal',
      options: <ModSelectOption>[
        ModSelectOption(value: 'restrained', label: '克制'),
        ModSelectOption(value: 'normal', label: '平常'),
        ModSelectOption(value: 'eager', label: '主动'),
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
      // 表演倾向（2026-10-13）：唯一在产品面渲染的配置键，由通用表单画。
      expect(find.text('表演倾向（保存并重载后生效）'), findsOneWidget);
      // 其余 8 个 label 一个都不在（含「高级」折叠——它整块不该出现）。
      for (final ModSettingField f in directorSpec().fields) {
        if (f.key == 'motion_bias') continue;
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

    testWidgets('真泵 ModsSection：表演倾向下拉可选（缺省平常）', (WidgetTester tester) async {
      final ModInfo mod = ModInfo(
        id: 'director',
        name: '导演',
        version: '0.1.0',
        apiVersion: 1,
        enabled: true,
        status: 'running',
        config: const <String, Object?>{},
        settingsSpec: directorSpec(),
      );
      Map<String, Object?>? saved;
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
                onSaveConfig: (String id, Map<String, Object?> config) async {
                  saved = config;
                  return const ModConfigResult(ok: true);
                },
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('导演'));
      await tester.pumpAndSettle();

      // 缺省值 = 平常（spec default normal；config 里没写这个键）。
      expect(find.text('表演倾向（保存并重载后生效）'), findsOneWidget);
      expect(find.text('平常'), findsOneWidget);
      // 三档选项都在（点开下拉可见）。
      await tester.tap(find.text('平常'));
      await tester.pumpAndSettle();
      expect(find.text('克制'), findsOneWidget);
      expect(find.text('主动'), findsOneWidget);
      // 选中主动并保存：值进保存载荷。
      await tester.tap(find.text('主动'));
      await tester.pumpAndSettle();
      // 保存按钮在展开表单的最下面，800x600 的视口里通常在折叠线以下。
      final Finder save = find.text('保存并应用');
      await tester.ensureVisible(save);
      await tester.pumpAndSettle();
      await tester.tap(save);
      await tester.pumpAndSettle();
      expect(saved, isNotNull);
      expect(saved!['motion_bias'], 'eager');
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

  group('⑤ T2（2026-10-13）：底部「保存并重载」能一起提交未提交的表单配置', () {
    testWidgets('改了表演倾向但没按卡片保存 → submitPending 写进 mods.json', (
      WidgetTester tester,
    ) async {
      final ModInfo mod = ModInfo(
        id: 'director',
        name: '导演',
        version: '0.1.0',
        apiVersion: 1,
        enabled: true,
        status: 'running',
        config: const <String, Object?>{},
        settingsSpec: directorSpec(),
      );
      Map<String, Object?>? saved;
      final ModConfigDrafts drafts = ModConfigDrafts();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[mod],
                loading: false,
                drafts: drafts,
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
      await tester.tap(find.text('导演'));
      await tester.pumpAndSettle();

      // 还没改：没有未提交草稿（底部「保存并重载」不因它出现）。
      expect(drafts.hasPending, isFalse);

      // 改档位，但**不按**卡片上的「保存并应用」——模拟用户直接去按底部那颗。
      await tester.tap(find.text('平常'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('主动'));
      await tester.pumpAndSettle();
      expect(drafts.hasPending, isTrue, reason: '改过档位 = 有未提交的 Mod 草稿');
      expect(saved, isNull, reason: '还没提交，磁盘上不该有改动');

      // 底部「保存并重载」的第 0 步（见 shell_settings._saveAll）。
      final List<String> failed = await drafts.submitPending();
      await tester.pumpAndSettle();

      expect(failed, isEmpty, reason: '提交必须成功');
      expect(saved, isNotNull, reason: '必须真的走了一次配置保存');
      expect(
        saved!['motion_bias'],
        'eager',
        reason: '导演表单里未提交的 motion_bias 必须一起写进 mods.json',
      );
      expect(drafts.hasPending, isFalse, reason: '提交成功后草稿清空');
    });
  });
}
