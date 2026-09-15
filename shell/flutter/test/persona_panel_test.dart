
/// persona 面板（产品级加强波次）：粘贴/导入路径、状态提示、与 memory 的共存文案。
///
/// 契约来自 `crates/live2d-ai-mod-persona/src/command.rs`（两条命令 + `state_json`）
/// 与 `lib/settings/mods/persona_panel.dart`。
///
/// 这里**不打网络**：`ModPanelContext.onCommand` 是注入的，测试给 fake 即可覆盖
/// 「点导入 → 送了什么 → 显示了什么」。文件选择同理（`pickCardFile` 注入）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panels.dart';
import 'package:live2d_ai_shell/settings/mods/persona_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

ModInfo _personaMod({bool enabled = true, String status = 'running'}) => ModInfo(
  id: 'persona',
  name: '角色卡',
  version: '0.2.0',
  apiVersion: 1,
  enabled: enabled,
  status: status,
  config: const <String, Object?>{},
);

Future<ModCommandResult> _okCommand(
  String command, [
  Map<String, Object?> args = const <String, Object?>{},
]) async => const ModCommandResult(ok: true);

ModPanelContext _ctx({
  ModInfo? mod,
  Map<String, Object?>? state,
  bool stateLoading = false,
  String? stateError,
  Future<ModCommandResult> Function(String, [Map<String, Object?>])? onCommand,
}) => ModPanelContext(
  mod: mod ?? _personaMod(),
  state: state,
  stateLoading: stateLoading,
  stateError: stateError,
  onRefreshState: () async {},
  onCommand: onCommand ?? _okCommand,
);

Widget _host(PersonaPanel panel, ModPanelContext ctx) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: Builder(builder: (BuildContext c) => panel.build(c, ctx)!),
    ),
  ),
);

/// 一个永远「用户选好了某张 PNG」的注入读取器。
PersonaCardFilePicker _picker([String url = 'data:image/png;base64,AAAA']) =>
    () async => (dataUrl: url, error: null);

Future<void> _tap(WidgetTester tester, String label) async {
  final Finder target = find.text(label);
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

void main() {
  group('面板注册与标签', () {
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

  group('纯函数：稳定取值 → 中文', () {
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

    test('personaStatusIsFailed 同时认 failed 与 error（否则失败提示永不出现）', () {
      expect(personaStatusIsFailed('failed'), isTrue);
      expect(personaStatusIsFailed('error'), isTrue);
      expect(personaStatusIsFailed('running'), isFalse);
      expect(personaStatusIsFailed('disabled'), isFalse);
    });
  });

  group('渲染：导入区、基线说明、与 memory 的共存提示', () {
    testWidgets('粘贴框 / 三个动作 / 基线说明 / 共存提示都在', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(PersonaPanel(pickCardFile: _picker()), _ctx()),
      );

      expect(find.text('粘贴角色卡 JSON（主路径）'), findsOneWidget);
      expect(find.text('导入并生效'), findsOneWidget);
      expect(find.text('选择 PNG 角色卡文件'), findsOneWidget);
      expect(find.text('清除导入卡'), findsOneWidget);
      // 基线语义必须说清（「打开开关即生效 / 关闭就还原」）。
      expect(find.textContaining('关闭开关会还原成主链基线'), findsOneWidget);
      // 与 memory 的共存策略（last-writer-wins）必须上屏。
      expect(find.textContaining('persona.system_prompt'), findsOneWidget);
      expect(find.textContaining('后写覆盖、不做仲裁'), findsOneWidget);
      expect(find.textContaining('别同时开'), findsOneWidget);
    });

    testWidgets('停用：导入按钮禁用 + 说清为什么要先开开关', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          PersonaPanel(pickCardFile: _picker()),
          _ctx(mod: _personaMod(enabled: false)),
        ),
      );

      final FilledButton import = tester.widget<FilledButton>(
        find.ancestor(
          of: find.text('导入并生效'),
          matching: find.byType(FilledButton),
        ),
      );
      expect(import.onPressed, isNull, reason: '停用时不该假装能导入');
      final OutlinedButton pick = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('选择 PNG 角色卡文件'),
          matching: find.byType(OutlinedButton),
        ),
      );
      expect(pick.onPressed, isNull);
      expect(find.textContaining('先打开上面的开关再导入'), findsOneWidget);
      expect(find.textContaining('command_unavailable'), findsOneWidget);
    });

    testWidgets('没注入文件读取器时：不摆按不动的按钮，只说明粘贴是主路径', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_host(const PersonaPanel(), _ctx()));
      expect(find.text('选择 PNG 角色卡文件'), findsNothing);
      expect(find.textContaining('没有接文件选择入口'), findsOneWidget);
      expect(find.textContaining('粘贴是主路径'), findsOneWidget);
      // 粘贴这条路必须照常可用。
      expect(find.text('导入并生效'), findsOneWidget);
    });

    testWidgets('status=failed / error：可处置提示 + 日志指引；running 时没有', (
      WidgetTester tester,
    ) async {
      for (final String status in <String>['failed', 'error']) {
        await tester.pumpWidget(
          _host(const PersonaPanel(), _ctx(mod: _personaMod(status: status))),
        );
        expect(
          find.textContaining('角色卡没被接受，修好再打开开关'),
          findsOneWidget,
          reason: 'status=$status 必须给出可处置提示',
        );
        expect(find.textContaining('mod: persona'), findsOneWidget);
      }

      await tester.pumpWidget(
        _host(const PersonaPanel(), _ctx(mod: _personaMod())),
      );
      expect(find.textContaining('角色卡没被接受'), findsNothing);
    });

    testWidgets('运行态：有卡时报来源与名称，没卡时说明保持基线', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            state: const <String, Object?>{
              'ready': true,
              'card_name': 'NEKO',
              'card_format': 'v2',
              'card_source': 'imported',
            },
          ),
        ),
      );
      expect(find.textContaining('当前角色卡：NEKO'), findsOneWidget);
      expect(find.textContaining('V2 卡'), findsOneWidget);
      expect(find.textContaining('来源：界面导入'), findsOneWidget);

      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            state: const <String, Object?>{
              'ready': false,
              'card_source': 'none',
              'card_name': '',
            },
          ),
        ),
      );
      expect(find.textContaining('当前没有生效的角色卡'), findsOneWidget);
    });

    testWidgets('运行态读不到时如实说（不冒充「没有卡」）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(stateError: '运行态暂时读不到（未启用 / worker 正忙，503 state_unavailable）'),
        ),
      );
      expect(find.textContaining('state_unavailable'), findsOneWidget);
    });
  });

  group('导入路径', () {
    testWidgets('粘贴 JSON → onCommand(import_card, card_json) 并显示卡名', (
      WidgetTester tester,
    ) async {
      String? seenCommand;
      Map<String, Object?>? seenArgs;
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              seenCommand = command;
              seenArgs = args;
              return const ModCommandResult(
                ok: true,
                result: <String, Object?>{'card_name': 'NEKO', 'card_format': 'v2'},
              );
            },
          ),
        ),
      );

      await tester.enterText(find.byType(TextField), '{"name":"NEKO"}');
      await _tap(tester, '导入并生效');

      expect(seenCommand, 'import_card');
      expect(seenArgs!['card_json'], '{"name":"NEKO"}');
      expect(find.textContaining('已导入「NEKO」'), findsOneWidget);
      expect(find.textContaining('V2 卡'), findsWidgets);
    });

    testWidgets('空粘贴不发命令，就地提示', (WidgetTester tester) async {
      bool called = false;
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              called = true;
              return const ModCommandResult(ok: true);
            },
          ),
        ),
      );
      await _tap(tester, '导入并生效');
      expect(called, isFalse, reason: '空输入不该发命令');
      expect(find.textContaining('先粘贴角色卡 JSON'), findsOneWidget);
    });

    testWidgets('导入失败：原因 + 错误码一起上屏', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              throw const ApiException(
                'command_failed',
                'card_json 不是可识别的角色卡 JSON',
                status: 409,
              );
            },
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), '{ not json');
      await _tap(tester, '导入并生效');

      expect(find.textContaining('不是可识别的角色卡'), findsOneWidget);
      expect(find.textContaining('command_failed'), findsOneWidget);
      expect(find.textContaining('已导入'), findsNothing, reason: '失败不得显示成功文案');
    });

    testWidgets('503：告诉用户先开开关，并带上码', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              throw const ApiException('command_unavailable', '未启用或正忙', status: 503);
            },
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), '{"name":"X"}');
      await _tap(tester, '导入并生效');

      expect(find.textContaining('先打开上面的开关再试'), findsOneWidget);
      expect(find.textContaining('command_unavailable'), findsOneWidget);
    });

    testWidgets('清除导入卡 → clear_import，并如实报结果', (WidgetTester tester) async {
      String? seenCommand;
      await tester.pumpWidget(
        _host(
          const PersonaPanel(),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              seenCommand = command;
              return const ModCommandResult(
                ok: true,
                result: <String, Object?>{'cleared': true, 'card_source': 'none'},
              );
            },
          ),
        ),
      );
      await _tap(tester, '清除导入卡');
      expect(seenCommand, 'clear_import');
      expect(find.textContaining('已清除导入卡'), findsOneWidget);
    });
  });

  group('文件选择（注入 fake，零 DOM）', () {
    testWidgets('选到 PNG → data_base64 收到完整 dataURL（前缀由 Mod 剥）', (
      WidgetTester tester,
    ) async {
      Map<String, Object?>? seenArgs;
      await tester.pumpWidget(
        _host(
          PersonaPanel(pickCardFile: _picker()),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              seenArgs = args;
              return const ModCommandResult(ok: true, result: <String, Object?>{});
            },
          ),
        ),
      );
      await _tap(tester, '选择 PNG 角色卡文件');
      expect(seenArgs!['data_base64'], 'data:image/png;base64,AAAA');
      expect(find.textContaining('已导入这张卡'), findsOneWidget);
    });

    testWidgets('读文件失败：如实说，不发命令', (WidgetTester tester) async {
      bool called = false;
      await tester.pumpWidget(
        _host(
          PersonaPanel(
            pickCardFile: () async => (dataUrl: null, error: '读取图片失败（文件可能已被移动）'),
          ),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              called = true;
              return const ModCommandResult(ok: true);
            },
          ),
        ),
      );
      await _tap(tester, '选择 PNG 角色卡文件');
      expect(called, isFalse);
      expect(find.textContaining('读取图片失败'), findsOneWidget);
    });

    testWidgets('取消选择：不弹任何东西', (WidgetTester tester) async {
      bool called = false;
      await tester.pumpWidget(
        _host(
          PersonaPanel(pickCardFile: () async => (dataUrl: null, error: null)),
          _ctx(
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              called = true;
              return const ModCommandResult(ok: true);
            },
          ),
        ),
      );
      await _tap(tester, '选择 PNG 角色卡文件');
      expect(called, isFalse);
      expect(find.textContaining('失败'), findsNothing, reason: '取消不是失败');
    });
  });
}
