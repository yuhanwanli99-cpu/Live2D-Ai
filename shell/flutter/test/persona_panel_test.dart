/// persona 回归（2026-10-08 二次口径：导入回到扩展卡片）。
///
/// # 这一版守什么
///
/// ① 扩展区的 persona 卡片**重新承担角色卡导入**：粘贴 JSON、PNG 选择、
///    导入并生效 / 导入为全局人设 / 清除当前会话的卡 / 清除全局导入卡；
/// ② 命令与入参逐字符合契约表（`import_card` / `clear_import` + session_id）；
/// ③ 8 个配置键经 [ModPanel.hiddenKeys] 整体不渲染，**也不收进「高级」**；
/// ④ 「人设」页只剩系统提示词 + 记住几轮——导入入口**只有一处**；
/// ⑤ 纯函数（来源 / 格式 / 作用域 / 失败态）仍供双方使用。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panels.dart';
import 'package:live2d_ai_shell/settings/mods/persona_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/settings/sections/persona_section.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';

/// 与 Rust `persona_settings_spec()` **逐键对应**的 8 个字段。
ModSettingsSpec personaSpec() => const ModSettingsSpec(
  modId: 'persona',
  title: '角色卡',
  version: 1,
  fields: <ModSettingField>[
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'card_path',
      label: '角色卡文件路径（.json / 内嵌 chara 的 .png）',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'card_json',
      label: '角色卡 JSON 文本（与路径二选一，优先）',
    ),
    ModSettingField(
      kind: ModFieldKind.bool,
      key: 'include_discipline',
      label: '附加对话纪律模板',
      defaultValue: true,
    ),
    ModSettingField(
      kind: ModFieldKind.bool,
      key: 'say_first_mes',
      label: '启用时朗读开场白',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'name',
      label: '覆盖：名称',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'description',
      label: '覆盖：描述',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'personality',
      label: '覆盖：性格',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'scenario',
      label: '覆盖：场景',
    ),
  ],
);

ModInfo _personaMod({bool enabled = true, String status = 'running'}) => ModInfo(
  id: 'persona',
  name: '角色卡',
  version: '0.2.0',
  apiVersion: 1,
  enabled: enabled,
  status: status,
  config: const <String, Object?>{},
  settingsSpec: personaSpec(),
);

ModPanelContext _ctx({
  bool enabled = true,
  String status = 'running',
  String? activeSessionId,
  Map<String, Object?>? state,
  PersonaCardFilePicker? pickCardFile,
  Future<ModCommandResult> Function(String command, Map<String, Object?> args)?
  onCommand,
}) => ModPanelContext(
  mod: _personaMod(enabled: enabled, status: status),
  state: state,
  stateLoading: false,
  stateError: null,
  activeSessionId: activeSessionId,
  pickCardFile: pickCardFile,
  onRefreshState: () async {},
  onCommand:
      (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
          onCommand == null
          ? const ModCommandResult(ok: true)
          : onCommand(command, args),
);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 真泵**扩展卡片**（通用表单 + 产品面板）——导入回归的主断言面。
Widget _panelCard({
  String? activeSessionId = 's-1',
  Map<String, Object?>? state,
  PersonaCardFilePicker? pickCardFile,
  Future<ModCommandResult> Function(String id, String command, Map<String, Object?> args)?
  onCommand,
}) => _wrap(
  ModsSection(
    mods: <ModInfo>[_personaMod()],
    loading: false,
    activeSessionId: activeSessionId,
    pickCardFile: pickCardFile,
    onCommand: onCommand,
    onLoadState: (String id) async => ModStateResult(
      id: id,
      enabled: true,
      state: state ?? const <String, Object?>{},
    ),
  ),
);

/// 扩展卡片里**一个都不许出现**的旧文案（上一版把这张卡片掏空时的遗留）。
const List<String> kBannedOnPersonaCard = <String>[
  'persona.system_prompt',
  '后写覆盖、不做仲裁',
  'command_unavailable',
  '503',
  '高级',
];

void main() {
  group('① 注册表与标签', () {
    test('注册表里的 persona 面板就是 PersonaPanel，运行态字段都有中文标签', () {
      expect(modPanelFor('persona'), isA<PersonaPanel>());
      final Map<String, String> labels = const PersonaPanel().stateLabels;
      for (final String key in <String>[
        'ready',
        'card_source',
        'card_name',
        'card_format',
        'applied_chars',
        'has_base_snapshot',
        'include_discipline',
        'say_first_mes',
        'config_has_card',
        'session_bound',
        'sessions',
        'active_session',
        'scope',
      ]) {
        expect(
          labels[key],
          isNotNull,
          reason: '运行态字段 $key 必须有中文标签（不靠原始 key 表意）',
        );
      }
      expect(labels['card_source'], '卡来源');
    });
  });

  group('② 纯函数：稳定取值 → 中文', () {
    test('personaCardSourceLabel 覆盖全部取值，未知值不崩', () {
      expect(personaCardSourceLabel('imported'), '界面导入');
      expect(personaCardSourceLabel('config_json'), '配置 card_json');
      expect(personaCardSourceLabel('config_path'), '配置 card_path');
      expect(personaCardSourceLabel('overrides'), '仅手工覆盖项');
      expect(personaCardSourceLabel('none'), '未配置');
      expect(personaCardSourceLabel('未来的值'), '未配置');
    });

    test('personaCardFormatLabel', () {
      expect(personaCardFormatLabel('v1'), 'V1 卡');
      expect(personaCardFormatLabel('v2'), 'V2 卡');
      expect(personaCardFormatLabel('manual'), '手工覆盖');
      expect(personaCardFormatLabel(''), '未知');
    });

    test('personaScopeLabel', () {
      expect(personaScopeLabel('session'), contains('只对当前会话生效'));
      expect(personaScopeLabel('global'), contains('所有会话'));
      expect(personaScopeLabel('?'), '未知');
    });

    test('personaStatusIsFailed 同时认 failed 与 error（否则失败提示永不出现）', () {
      expect(personaStatusIsFailed('failed'), isTrue);
      expect(personaStatusIsFailed('error'), isTrue);
      expect(personaStatusIsFailed('running'), isFalse);
      expect(personaStatusIsFailed('disabled'), isFalse);
    });
  });

  group('③ 卡片：一句话 + 导入区（旧排障词一个都不在）', () {
    testWidgets('那句话与四个入口上屏；未启用 / 失败态同样只有这一套', (
      WidgetTester tester,
    ) async {
      for (final ({bool enabled, String status}) c in <({bool enabled, String status})>[
        (enabled: true, status: 'running'),
        (enabled: false, status: 'disabled'),
        (enabled: true, status: 'failed'),
      ]) {
        await tester.pumpWidget(
          _wrap(
            Builder(
              builder: (BuildContext context) =>
                  const PersonaPanel().build(
                    context,
                    _ctx(
                      enabled: c.enabled,
                      status: c.status,
                      activeSessionId: 's-1',
                    ),
                  )!,
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(
          find.text(kPersonaTakeoverNotice),
          findsOneWidget,
          reason: 'enabled=${c.enabled} status=${c.status}',
        );
        expect(find.text('导入并生效'), findsOneWidget);
        expect(find.text('导入为全局人设'), findsOneWidget);
        expect(find.text('清除当前会话的卡'), findsOneWidget);
        expect(find.text('清除全局导入卡'), findsOneWidget);
        for (final String banned in kBannedOnPersonaCard) {
          expect(
            find.textContaining(banned),
            findsNothing,
            reason: '卡片上不得出现「$banned」',
          );
        }
      }
    });

    testWidgets('运行态有卡名 → 一句话写「当前卡」与作用域；没有就不画', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          Builder(
            builder: (BuildContext context) => const PersonaPanel().build(
              context,
              _ctx(
                activeSessionId: 's-1',
                state: const <String, Object?>{
                  'card_name': 'NEKO',
                  'scope': 'session',
                },
              ),
            )!,
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find.textContaining('当前卡：「NEKO」'),
        findsOneWidget,
      );
      expect(find.textContaining('只对当前会话生效'), findsOneWidget);

      await tester.pumpWidget(
        _wrap(
          Builder(
            builder: (BuildContext context) => const PersonaPanel().build(
              context,
              _ctx(
                activeSessionId: 's-1',
                state: const <String, Object?>{'ready': true},
              ),
            )!,
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('当前卡'), findsNothing);
    });
  });

  group('④ 8 个配置键不渲染，也不进「高级」', () {
    test('hiddenKeys 恰好是那 8 个', () {
      const PersonaPanel panel = PersonaPanel();
      expect(panel.modId, 'persona');
      expect(panel.hiddenKeys, <String>{
        'card_path',
        'card_json',
        'include_discipline',
        'say_first_mes',
        'name',
        'description',
        'personality',
        'scenario',
      });
      expect(panel.hiddenKeys.length, 8);
      expect(panel.advancedKeys, isEmpty);
      expect(panel.devKeys, isEmpty);
    });

    testWidgets('真泵卡片：8 个 label 与「高级」都不上屏，启用开关仍在', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_panelCard());
      await tester.tap(find.text('角色卡'));
      await tester.pumpAndSettle();

      for (final ModSettingField f in personaSpec().fields) {
        expect(find.text(f.label), findsNothing, reason: '${f.key} 不得上屏');
      }
      expect(find.text('高级'), findsNothing);
      expect(find.byType(Switch), findsOneWidget);
      expect(find.text(kPersonaTakeoverNotice), findsOneWidget);
    });
  });

  group('⑤ 扩展卡片是唯一入口（命令与入参逐字符合契约）', () {
    late List<(String, Map<String, Object?>)> sent;

    setUp(() => sent = <(String, Map<String, Object?>)>[]);

    Future<ModCommandResult> Function(String, String, Map<String, Object?>)
    recorder([ModCommandResult? result]) =>
        (String id, String command, Map<String, Object?> args) async {
          sent.add((command, args));
          return result ?? const ModCommandResult(ok: true);
        };

    Future<void> expand(WidgetTester tester, Widget card) async {
      await tester.pumpWidget(card);
      await tester.tap(find.text('角色卡'));
      await tester.pumpAndSettle();
    }

    Future<void> tapButton(WidgetTester tester, String label) async {
      final Finder target = find.text(label);
      await tester.ensureVisible(target);
      await tester.pumpAndSettle();
      await tester.tap(target);
      await tester.pumpAndSettle();
    }

    testWidgets('四个入口都在，没注入读取器时没有 PNG 按钮', (WidgetTester tester) async {
      await expand(tester, _panelCard());
      expect(find.widgetWithText(FilledButton, '导入并生效'), findsOneWidget);
      expect(find.widgetWithText(OutlinedButton, '导入为全局人设'), findsOneWidget);
      expect(find.widgetWithText(TextButton, '清除当前会话的卡'), findsOneWidget);
      expect(find.widgetWithText(TextButton, '清除全局导入卡'), findsOneWidget);
      expect(find.text('选择 PNG 角色卡文件'), findsNothing);
      expect(find.byKey(const Key('persona-card-json')), findsOneWidget);
    });

    testWidgets('注入 PNG 读取器后按钮出现', (WidgetTester tester) async {
      await expand(
        tester,
        _panelCard(
          pickCardFile: () async =>
              (dataUrl: 'data:image/png;base64,AAAA', error: null),
        ),
      );
      expect(find.text('选择 PNG 角色卡文件'), findsOneWidget);
    });

    testWidgets('粘贴 JSON + 「导入并生效」→ import_card 带 session_id', (
      WidgetTester tester,
    ) async {
      await expand(tester, _panelCard(onCommand: recorder()));
      await tester.enterText(
        find.byKey(const Key('persona-card-json')),
        '{"name":"NEKO"}',
      );
      await tapButton(tester, '导入并生效');
      expect(sent.single.$1, 'import_card');
      expect(sent.single.$2['card_json'], '{"name":"NEKO"}');
      expect(sent.single.$2['session_id'], 's-1');
    });

    testWidgets('「清除当前会话的卡」→ clear_import 带 session_id', (
      WidgetTester tester,
    ) async {
      await expand(
        tester,
        _panelCard(
          onCommand: recorder(
            const ModCommandResult(
              ok: true,
              result: <String, Object?>{
                'scope': 'session',
                'session_id': 's-1',
                'cleared': true,
              },
            ),
          ),
        ),
      );
      await tapButton(tester, '清除当前会话的卡');
      expect(sent.single.$1, 'clear_import');
      expect(sent.single.$2['session_id'], 's-1');
      expect(find.textContaining('已清除会话 s-1 的角色卡'), findsOneWidget);
    });

    testWidgets('「清除全局导入卡」→ clear_import **不带** session_id', (
      WidgetTester tester,
    ) async {
      await expand(
        tester,
        _panelCard(
          onCommand: recorder(
            const ModCommandResult(
              ok: true,
              result: <String, Object?>{
                'scope': 'global',
                'cleared': true,
                'card_source': 'none',
              },
            ),
          ),
        ),
      );
      await tapButton(tester, '清除全局导入卡');
      expect(sent.single.$1, 'clear_import');
      expect(sent.single.$2.containsKey('session_id'), isFalse);
      expect(find.textContaining('已清除全局导入卡'), findsOneWidget);
    });

    testWidgets('没有活动会话：会话相关的两个按钮禁用，全局清除仍可点', (
      WidgetTester tester,
    ) async {
      await expand(tester, _panelCard(activeSessionId: null));
      expect(
        tester
            .widget<TextButton>(
              find.widgetWithText(TextButton, '清除当前会话的卡'),
            )
            .onPressed,
        isNull,
      );
      expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, '清除全局导入卡'))
            .onPressed,
        isNotNull,
      );
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, '导入并生效'))
            .onPressed,
        isNull,
        reason: '没有会话时不许偷偷走全局：按钮必须禁用',
      );
    });

    testWidgets('清除失败：错误句带错误码（用户拿码去日志里搜）', (WidgetTester tester) async {
      await expand(
        tester,
        _panelCard(
          onCommand: (String id, String command, Map<String, Object?> args) async {
            throw const ApiException(
              'command_unavailable',
              '未启用或正忙',
              status: 503,
            );
          },
        ),
      );
      await tapButton(tester, '清除全局导入卡');
      expect(find.textContaining('清除角色卡失败：persona 没在运行'), findsOneWidget);
      expect(find.textContaining('先在卡片标题行打开它'), findsOneWidget);
      expect(find.textContaining('（503 command_unavailable）'), findsOneWidget);
    });

    testWidgets('注入 PNG 读取器后按钮出现，走同一条 import_card 通道', (
      WidgetTester tester,
    ) async {
      await expand(
        tester,
        _panelCard(
          onCommand: recorder(),
          pickCardFile: () async =>
              (dataUrl: 'data:image/png;base64,AAAA', error: null),
        ),
      );
      await tapButton(tester, '选择 PNG 角色卡文件');
      expect(sent.single.$1, 'import_card');
      expect(sent.single.$2['data_base64'], 'data:image/png;base64,AAAA');
      expect(sent.single.$2['session_id'], 's-1');
    });
  });

  group('⑥ 人设页只剩主链两项（导入入口唯一）', () {
    late SettingsController controller;

    setUp(() {
      controller = SettingsController(
        api: ApiClient(base: 'http://127.0.0.1:18080'),
      );
    });

    testWidgets('找不到「导入并生效」「选择 PNG 角色卡文件」「粘贴角色卡 JSON」', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          PersonaSection(
            controller: controller,
            view: SettingsView.fromJson(const <String, Object?>{}),
            devMode: false,
          ),
        ),
      );
      await tester.pumpAndSettle();
      for (final String banned in <String>[
        '导入并生效',
        '导入为全局人设',
        '选择 PNG 角色卡文件',
        '粘贴角色卡 JSON',
        '清除当前会话的卡',
        '清除全局导入卡',
        '角色卡导入',
      ]) {
        expect(
          find.textContaining(banned),
          findsNothing,
          reason: '人设页不得再有「$banned」（导入属扩展）',
        );
      }
      expect(find.text('系统提示词'), findsOneWidget);
      expect(find.text('记住几轮对话'), findsOneWidget);
    });

    test('人设页源码里不再引 package:web / 卡命令通道', () {
      final String src = readLibrarySource(
        'lib/settings/sections/persona_section.dart',
      );
      for (final String banned in <String>[
        'browser_io.dart',
        'PersonaImportCommand',
        'import_card',
        'clear_import',
        'pickCardFile',
      ]) {
        expect(src.contains(banned), isFalse, reason: '人设页源码不得含「$banned」');
      }
    });
  });
}
