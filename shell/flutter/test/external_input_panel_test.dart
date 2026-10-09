/// external-input 产品面板回归（2026-10-08 二次口径）。
///
/// # 这一版守什么
///
/// ① 卡片常驻那句话：打开后直播间的弹幕和礼物会说给角色听；
/// ② **四个配置键回到产品表单**（端口提示 / 令牌 / 弹幕怎么说给角色 /
///    每条前面加的字），标签逐字取自 Rust spec，说明来自 [ModPanel.fieldHelp]；
/// ③ `devMode == false` 时**没有**自检（发送这条测试 / 计数 / 计数清零）；
///    `devMode == true` 时才画，且命令名与入参正确；
/// ④ 旧产品面里那些排障词（curl / 403 / 503 / command_unavailable / sidecar /
///    「高级」）一个都不在；
/// ⑤ 保留下来的**纯函数**仍与 Rust 同语义（模板渲染 / 计数口径 / 令牌两态）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/external_input_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 一份运行态快照（键与 Rust state_json 一致）。
Map<String, Object?> _state({
  int accepts = 0,
  int rejects = 0,
  int busy = 0,
  int v2 = 0,
  bool tokenSet = false,
}) => <String, Object?>{
  'accepts': accepts,
  'rejects': rejects,
  'busy': busy,
  'v2_ignored': v2,
  'ready': true,
  'token_set': tokenSet,
};

/// 与 Rust `external_input_settings_spec()` **逐键逐 label**对应的 4 个字段。
ModSettingsSpec externalSpec() => const ModSettingsSpec(
  modId: 'external-input',
  title: '外部事件接入',
  version: 2,
  fields: <ModSettingField>[
    ModSettingField(
      kind: ModFieldKind.number,
      key: 'listen_port',
      label: '端口（只作提示）',
      min: 1024,
      max: 65535,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'token',
      label: '令牌',
      secret: true,
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'text_template',
      label: '弹幕怎么说给角色',
    ),
    ModSettingField(
      kind: ModFieldKind.string,
      key: 'prefix',
      label: '每条前面加的字',
    ),
  ],
);

ModPanelContext _ctx({
  Map<String, Object?>? state,
  Map<String, Object?> config = const <String, Object?>{
    'prefix': '[弹幕] ',
    'text_template': '{text}',
  },
  bool enabled = true,
  bool devMode = false,
  Future<ModCommandResult> Function(String command, Map<String, Object?> args)?
  onCommand,
}) => ModPanelContext(
  mod: ModInfo(
    id: 'external-input',
    name: '外部事件接入',
    version: '0.2.0',
    apiVersion: 1,
    enabled: enabled,
    status: enabled ? 'running' : 'disabled',
    config: config,
  ),
  state: state,
  stateLoading: false,
  stateError: null,
  onRefreshState: () async {},
  devMode: devMode,
  onCommand:
      (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
          onCommand == null
          ? const ModCommandResult(ok: true)
          : onCommand(command, args),
);

/// 把面板挂进最小 widget 树（build 需要 BuildContext）。
Widget _panel(ModPanelContext ctx) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: Builder(
        builder: (BuildContext context) =>
            const ExternalInputPanel().build(context, ctx) ??
            const SizedBox.shrink(),
      ),
    ),
  ),
);

/// 真泵整张 Mod 卡片（通用表单 + 产品面板）。
Widget _card({
  bool devMode = false,
  ModCommandSender? onCommand,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: ModsSection(
        mods: <ModInfo>[
          ModInfo(
            id: 'external-input',
            name: '外部事件接入',
            version: '0.2.0',
            apiVersion: 1,
            enabled: true,
            status: 'running',
            config: const <String, Object?>{'listen_port': 18080},
            settingsSpec: externalSpec(),
          ),
        ],
        loading: false,
        devMode: devMode,
        onCommand: onCommand,
        onLoadState: (String id) async =>
            ModStateResult(id: id, enabled: true, state: _state(accepts: 5)),
      ),
    ),
  ),
);

/// 滚进视口再点（自检块在 800x600 的测试面之外，直接 tap 会点到视口外）。
Future<void> _tapVisible(WidgetTester tester, Key key) async {
  final Finder target = find.byKey(key);
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

/// 产品卡片上**一个都不许出现**的排障词（2026-10-08 二次口径）。
const List<String> kBannedOnExternalCard = <String>[
  'curl',
  '403',
  '503',
  'mod_disabled',
  'command_unavailable',
  'sidecar',
  '高级',
];

void main() {
  const ExternalInputPanel panel = ExternalInputPanel();

  group('① 纯函数：标签 / 摘要 / 令牌 / 模板渲染（与 Rust 同语义）', () {
    test('stateLabels 给四项计数 + ready/token_set 中文标签', () {
      expect(
        kExternalInputCounterKeys,
        <String>['accepts', 'rejects', 'busy', 'v2_ignored'],
      );
      expect(panel.stateLabels['accepts'], '已接受');
      expect(panel.stateLabels['rejects'], '已拒绝');
      expect(panel.stateLabels['busy'], '忙碌拒绝');
      expect(panel.stateLabels['v2_ignored'], '礼物 v2 兜底');
      expect(panel.stateLabels['ready'], '已就绪');
      expect(panel.stateLabels['token_set'], '令牌已设置');
    });

    test('counterSummary 是一行人话；缺字段按 0', () {
      expect(
        counterSummary(_state(accepts: 12, rejects: 1, busy: 3, v2: 0)),
        '已接受 12 条弹幕/礼物 · 拒绝 1 · 忙碌丢弃 3 · 礼物 v2 兜底 0',
      );
      expect(
        counterSummary(const <String, Object?>{}),
        '已接受 0 条弹幕/礼物 · 拒绝 0 · 忙碌丢弃 0 · 礼物 v2 兜底 0',
      );
    });

    test('tokenStatusText 两态 + 未知态（用文字说清，不从颜色猜）', () {
      expect(tokenStatusText(_state(tokenSet: true)), contains('已设置令牌'));
      expect(tokenStatusText(_state(tokenSet: false)), contains('未设令牌'));
      expect(tokenStatusText(_state(tokenSet: false)), contains('仅本机可用'));
      expect(tokenStatusText(null), contains('运行态还没读到'));
    });

    test('renderInjectedText 与 Rust render_injected_text 同语义', () {
      expect(
        renderInjectedText(prefix: '[弹幕] ', template: '{text}', text: '主播好'),
        '[弹幕] 主播好',
      );
      expect(
        renderInjectedText(prefix: '', template: '观众说：{text}（请回应）', text: '666'),
        '观众说：666（请回应）',
      );
      expect(
        renderInjectedText(prefix: '', template: '【外部消息】', text: 'hi'),
        '【外部消息】hi',
      );
      expect(
        renderInjectedText(prefix: '', template: '{text}/{text}', text: 'x'),
        'x/x',
      );
      expect(renderInjectedText(prefix: '', template: '', text: 'hi'), 'hi');
    });

    test('templateExample：都空 → null；否则给本地示例', () {
      expect(templateExample(const <String, Object?>{}), isNull);
      expect(
        templateExample(const <String, Object?>{
          'prefix': '[弹幕] ',
          'text_template': '{text}',
        }),
        '[弹幕] 主播好',
      );
      expect(
        templateExample(const <String, Object?>{'prefix': '[礼物] '}, sample: '弹药'),
        '[礼物] 弹药',
      );
    });
  });

  group('② 四个键回到产品表单（说明跟着键走）', () {
    test('hiddenKeys / devKeys / advancedKeys 都空', () {
      expect(panel.modId, 'external-input');
      expect(panel.hiddenKeys, isEmpty);
      expect(panel.devKeys, isEmpty);
      expect(panel.advancedKeys, isEmpty);
    });

    test('fieldHelp 已退役（2026-10-09：三级功能介绍全删）', () {
      expect(panel.fieldHelp, isEmpty);
    });

    testWidgets('真泵卡片：4 个 label 上屏、旧说明一句都不上屏，启用开关仍在', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_card());
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();

      for (final ModSettingField f in externalSpec().fields) {
        expect(find.text(f.label), findsOneWidget, reason: '${f.key} 应上屏');
      }
      for (final String gone in <String>[
        '服务实际听哪个端口不由这项决定',
        '{text} 会换成弹幕或礼物原文',
        '加在每条前面',
      ]) {
        expect(find.text(gone), findsNothing, reason: '旧 fieldHelp 说明不得再上屏：\$gone');
      }
      // secret 那一行有它自己固定的「留空 = 不修改」说明。
      expect(find.text('留空表示不修改（服务端不回传密钥）'), findsOneWidget);
      expect(find.text('高级'), findsNothing);
      expect(find.byType(Switch), findsOneWidget);
      expect(find.textContaining('打开后，直播间的弹幕和礼物会说给角色听'), findsOneWidget);
    });

    testWidgets('产品面：找得到「令牌」，找不到「发送这条测试」', (WidgetTester tester) async {
      await tester.pumpWidget(_card());
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      expect(find.text('令牌'), findsOneWidget);
      expect(find.text('发送这条测试'), findsNothing);
      expect(find.textContaining('计数清零'), findsNothing);
    });
  });

  group('③ 开发模式才画自检（发送 / 计数 / 清零）', () {
    testWidgets('devMode=true：自检块上屏，四项计数读出', (WidgetTester tester) async {
      await tester.pumpWidget(_card(devMode: true, onCommand: (_, _, _) async {
        return const ModCommandResult(ok: true, result: <String, Object?>{'accepted': true});
      }));
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      expect(find.text('发送这条测试'), findsOneWidget);
      expect(find.textContaining('已接受 5 条弹幕/礼物'), findsOneWidget);
      expect(find.text('计数清零'), findsOneWidget);
    });

    testWidgets('发送：命令 test_inject + args.text；成功句写清主链是否接住', (
      WidgetTester tester,
    ) async {
      final List<(String, Map<String, Object?>)> sent =
          <(String, Map<String, Object?>)>[];
      await tester.pumpWidget(
        _card(
          devMode: true,
          onCommand: (String id, String command, Map<String, Object?> args) async {
            sent.add((command, args));
            return const ModCommandResult(
              ok: true,
              result: <String, Object?>{
                'accepted': true,
                'injected_text': '[弹幕] 主播好',
              },
            );
          },
        ),
      );
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('external-input-test-field')),
        ' 主播好 ',
      );
      await _tapVisible(tester, const Key('external-input-test-send'));
      expect(sent.single.$1, 'test_inject');
      expect(sent.single.$2['text'], '主播好');
      expect(find.textContaining('主链接住了这条'), findsOneWidget);
    });

    testWidgets('主链忙：如实说没接住（不谎报成功）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _card(
          devMode: true,
          onCommand: (String id, String command, Map<String, Object?> args) async =>
              const ModCommandResult(
                ok: true,
                result: <String, Object?>{'accepted': false},
              ),
        ),
      );
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('external-input-test-field')),
        'hi',
      );
      await _tapVisible(tester, const Key('external-input-test-send'));
      expect(find.textContaining('主链正忙'), findsOneWidget);
      expect(find.textContaining('主链接住了这条'), findsNothing);
    });

    testWidgets('失败句句尾带错误码（用户拿码去日志里搜）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _card(
          devMode: true,
          onCommand: (String id, String command, Map<String, Object?> args) async {
            throw const ApiException('command_unavailable', '未启用或正忙', status: 503);
          },
        ),
      );
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('external-input-test-field')),
        'hi',
      );
      await _tapVisible(tester, const Key('external-input-test-send'));
      expect(find.textContaining('（command_unavailable）'), findsOneWidget);
    });

    testWidgets('计数清零：命令 reset_counters + 空 args', (WidgetTester tester) async {
      final List<(String, Map<String, Object?>)> sent =
          <(String, Map<String, Object?>)>[];
      await tester.pumpWidget(
        _card(
          devMode: true,
          onCommand: (String id, String command, Map<String, Object?> args) async {
            sent.add((command, args));
            return const ModCommandResult(ok: true);
          },
        ),
      );
      await tester.tap(find.text('外部事件接入'));
      await tester.pumpAndSettle();
      await _tapVisible(tester, const Key('external-input-reset-counters'));
      expect(sent.single.$1, 'reset_counters');
      expect(sent.single.$2, isEmpty);
      expect(find.text('计数已清零'), findsOneWidget);
    });
  });

  group('④ 排障词仍然不在产品面', () {
    testWidgets('旧词一个都不在（含未启用态与开发模式）', (WidgetTester tester) async {
      for (final bool devMode in <bool>[false, true]) {
        await tester.pumpWidget(
          _panel(_ctx(state: _state(accepts: 5), devMode: devMode)),
        );
        await tester.pumpAndSettle();
        for (final String banned in kBannedOnExternalCard) {
          expect(
            find.textContaining(banned),
            findsNothing,
            reason: 'devMode=$devMode 时面板上不得出现「$banned」',
          );
        }
      }
    });
  });
}
