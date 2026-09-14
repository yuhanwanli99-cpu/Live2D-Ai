/// `external-input` 产品面板回归（产品级加强波次）。
///
/// 覆盖三条产品级诉求的**前端可读面**：
/// 1. 计数摘要文案 + 「重置计数」按钮走 `onCommand('reset_counters')`（含二次确认）；
/// 2. `token_set` 两态文案（已设置 / 未设：仅本机可用）；
/// 3. 模板 / 前缀说明与**本地**渲染示例（不发请求）。
///
/// 面板自己不做网络：所有动作都经 `ModPanelContext.onCommand` 注入的 fake，
/// 测试因此零网络、零真实后端。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/external_input_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 一份运行态快照（键与 Rust `state_json` 一致）。
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

/// 构造面板上下文；默认 config 带一段 prefix + 模板，便于断言示例。
ModPanelContext _ctx({
  Map<String, Object?>? state,
  Map<String, Object?> config = const <String, Object?>{
    'prefix': '[弹幕] ',
    'text_template': '{text}',
  },
  bool enabled = true,
  String? stateError,
  Future<ModCommandResult> Function(String, [Map<String, Object?>])? onCommand,
  Future<void> Function()? onRefreshState,
}) {
  return ModPanelContext(
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
    stateError: stateError,
    onRefreshState: onRefreshState ?? () async {},
    onCommand: onCommand ??
        (String command,
            [Map<String, Object?> args = const <String, Object?>{}]) async =>
            const ModCommandResult(ok: true),
  );
}

/// 把面板挂进最小 widget 树（`build` 需要 BuildContext）。
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

void main() {
  const ExternalInputPanel panel = ExternalInputPanel();

  group('① 纯函数：标签 / 摘要 / 令牌 / 模板渲染', () {
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
      // 只有 prefix（模板空）→ prefix + 原样文本。
      expect(
        templateExample(const <String, Object?>{'prefix': '[礼物] '}, sample: '弹药'),
        '[礼物] 弹药',
      );
    });
  });

  group('② 面板渲染：摘要 / 令牌 / 模板示例', () {
    testWidgets('计数摘要 + 已设置令牌 + 渲染示例同时上屏', (WidgetTester tester) async {
      await tester.pumpWidget(
        _panel(_ctx(state: _state(accepts: 12, rejects: 1, busy: 3, v2: 4, tokenSet: true))),
      );
      expect(
        find.text('已接受 12 条弹幕/礼物 · 拒绝 1 · 忙碌丢弃 3 · 礼物 v2 兜底 4'),
        findsOneWidget,
      );
      expect(find.textContaining('已设置令牌'), findsOneWidget);
      expect(find.text('当前配置的渲染示例：[弹幕] 主播好'), findsOneWidget);
      expect(find.byKey(const Key('external-input-reset')), findsOneWidget);
    });

    testWidgets('token_set=false → 「未设令牌：仅本机可用」', (WidgetTester tester) async {
      await tester.pumpWidget(_panel(_ctx(state: _state(tokenSet: false))));
      expect(find.textContaining('未设令牌'), findsOneWidget);
      expect(find.textContaining('仅本机可用'), findsOneWidget);
    });

    testWidgets('运行态未读到时不编造计数', (WidgetTester tester) async {
      await tester.pumpWidget(_panel(_ctx(state: null)));
      expect(find.byKey(const Key('external-input-counter-summary')), findsNothing);
      expect(find.textContaining('运行态还没读到'), findsWidgets);
    });

    testWidgets('模板为空时说明「原样送进链路」', (WidgetTester tester) async {
      await tester.pumpWidget(
        _panel(_ctx(state: _state(), config: const <String, Object?>{})),
      );
      expect(
        tester
            .widget<Text>(
              find.byKey(const Key('external-input-template-example')),
            )
            .data,
        '当前没有配置前缀 / 模板：外部文本会原样送进链路（示例：主播好）。',
      );
    });
  });

  group('③ 重置计数：二次确认 + 命令通道', () {
    testWidgets('按钮文案说清会清零；确认后触发 onCommand(reset_counters)', (
      WidgetTester tester,
    ) async {
      final List<String> calls = <String>[];
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(accepts: 5),
            onCommand: (String command,
                [Map<String, Object?> args = const <String, Object?>{}]) async {
              calls.add(command);
              return const ModCommandResult(
                ok: true,
                result: <String, Object?>{
                  'reset': true,
                  'before': <String, Object?>{
                    'accepts': 5,
                    'rejects': 0,
                    'busy': 0,
                    'v2_ignored': 0,
                  },
                },
              );
            },
          ),
        ),
      );
      expect(find.text('重置计数（四项全部清零）'), findsOneWidget);
      expect(find.textContaining('清零后无法恢复'), findsOneWidget);

      await tester.tap(find.byKey(const Key('external-input-reset')));
      await tester.pumpAndSettle();
      expect(find.text('确认清零'), findsOneWidget);
      expect(find.textContaining('清零后无法恢复'), findsWidgets);

      await tester.tap(find.text('确认清零'));
      await tester.pumpAndSettle();
      expect(calls, <String>['reset_counters']);
      expect(find.textContaining('已清零'), findsOneWidget);
      expect(find.textContaining('清零前：已接受 5'), findsOneWidget);
    });

    testWidgets('取消 = 不清零（命令一次都不发）', (WidgetTester tester) async {
      final List<String> calls = <String>[];
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(),
            onCommand: (String command,
                [Map<String, Object?> args = const <String, Object?>{}]) async {
              calls.add(command);
              return const ModCommandResult(ok: true);
            },
          ),
        ),
      );
      await tester.tap(find.byKey(const Key('external-input-reset')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(calls, isEmpty);
    });

    testWidgets('命令失败带错误码上屏', (WidgetTester tester) async {
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(),
            onCommand: (String command,
                [Map<String, Object?> args = const <String, Object?>{}]) async {
              throw const ApiException('command_unavailable', 'Mod 未启用或正忙，可重试',
                  status: 503);
            },
          ),
        ),
      );
      await tester.tap(find.byKey(const Key('external-input-reset')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('确认清零'));
      await tester.pumpAndSettle();
      expect(find.textContaining('command_unavailable'), findsOneWidget);
    });

    testWidgets('Mod 未启用：按钮禁用且文案说明', (WidgetTester tester) async {
      await tester.pumpWidget(
        _panel(_ctx(enabled: false, state: _state())),
      );
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.byKey(const Key('external-input-reset')),
      );
      expect(button.onPressed, isNull);
      expect(find.textContaining('Mod 未启用'), findsOneWidget);
    });
  });
}
