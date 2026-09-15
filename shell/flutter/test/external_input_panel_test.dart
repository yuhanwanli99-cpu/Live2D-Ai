/// external-input 产品面板回归（产品级加强波次 + L1 产品级波次）。
///
/// 覆盖四条产品级诉求的**前端可读面**：
/// 1. 计数摘要文案 + 「重置计数」按钮走 onCommand('reset_counters')（含二次确认）；
/// 2. token_set 两态文案（已设置 / 未设：仅本机可用）；
/// 3. 模板 / 前缀说明与**本地**渲染示例（不发请求）；
/// 4. L1 的「测试注入」命令通道 + 停用 403 可读（Mod id 仍是 external-input）。
///
/// 面板自己不做网络：所有动作都经 ModPanelContext.onCommand 注入的 fake，
/// 测试因此零网络、零真实后端。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/external_input_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
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
  ValueChanged<String>? onModChanged,
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
    onModChanged: onModChanged,
  );
}

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

    testWidgets('Mod 未启用：重置按钮禁用且文案说明', (WidgetTester tester) async {
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

  group('④ L1：测试注入 + 停用 403 可读', () {
    test('失败码 command_unavailable → 点明 503 / 未启用 / 开关在哪', () {
      final String msg = externalInputInjectErrorMessage(
        const ApiException('command_unavailable', 'Mod external-input 未启用或正忙，可重试',
            status: 503),
      );
      expect(msg, contains('command_unavailable'));
      expect(msg, contains('未启用或正忙（503）'));
      expect(msg, contains('先打开卡片标题行的开关'));
    });

    test('失败码 command_failed → 点明 2000 字符上限', () {
      final String msg = externalInputInjectErrorMessage(
        const ApiException('command_failed', 'Mod external-input 命令失败：注入文本过长',
            status: 409),
      );
      expect(msg, contains('command_failed'));
      expect(msg, contains('2000'));
    });

    test('injectReceiptText：成功说「已注入」，忙说「被丢弃」', () {
      expect(
        injectReceiptText(
          injectedText: '[弹幕] 主播好',
          accepted: true,
          state: _state(accepts: 7, busy: 1),
        ),
        '已注入：[弹幕] 主播好（已接受 7 / 忙碌丢弃 1）',
      );
      expect(
        injectReceiptText(
          injectedText: 'hi',
          accepted: false,
          state: _state(accepts: 7, busy: 2),
        ),
        contains('被丢弃'),
      );
    });

    testWidgets('面板主标题仍是外部事件接入 + 中性 Mod id 行（无品牌化改名）', (WidgetTester tester) async {
      await tester.pumpWidget(_panel(_ctx(state: _state())));
      expect(find.text('外部事件接入'), findsWidgets);
      expect(
        tester
            .widget<Text>(find.byKey(const Key('external-input-mod-id')))
            .data,
        'Mod id：external-input（端点契约不变）',
      );
    });

    testWidgets('测试注入 → onCommand(test_inject) 收到渲染后的文本 + 刷新计数（不发重启提示）', (
      WidgetTester tester,
    ) async {
      final List<String> commands = <String>[];
      final List<Map<String, Object?>> args = <Map<String, Object?>>[];
      final List<String> changes = <String>[];
      int refreshes = 0;
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(accepts: 7, busy: 1),
            onRefreshState: () async => refreshes++,
            onModChanged: (String what) => changes.add(what),
            onCommand: (String command,
                [Map<String, Object?> a = const <String, Object?>{}]) async {
              commands.add(command);
              args.add(a);
              return const ModCommandResult(
                ok: true,
                result: <String, Object?>{
                  'injected_text': '[弹幕] 主播好',
                  'accepted': true,
                },
              );
            },
          ),
        ),
      );
      // 初值 = 按当前 prefix/模板渲染出的示例。
      expect(
        tester
            .widget<TextField>(find.byKey(const Key('external-input-inject-text')))
            .controller
            ?.text,
        '[弹幕] 主播好',
      );

      await tester.tap(find.byKey(const Key('external-input-inject')));
      await tester.pumpAndSettle();

      expect(commands, <String>['test_inject']);
      expect(args.single['text'], '[弹幕] 主播好');
      expect(args.single['prefix'], false, reason: '文本框里已是最终文本，避免双重前缀');
      expect(find.textContaining('已注入：[弹幕] 主播好'), findsOneWidget);
      expect(find.textContaining('已接受 7 / 忙碌丢弃 1'), findsOneWidget);
      expect(refreshes, 1, reason: '注入后必须刷新计数');
      expect(
        changes,
        isEmpty,
        reason: '注入是瞬时行为：不改配置，不得发「需重新点火」提示',
      );
    });

    testWidgets('命令不可用 → 上屏 503 / 未启用 / 开关文案', (WidgetTester tester) async {
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(),
            onCommand: (String command,
                [Map<String, Object?> a = const <String, Object?>{}]) async {
              throw const ApiException('command_unavailable', 'Mod external-input 未启用或正忙，可重试',
                  status: 503);
            },
          ),
        ),
      );
      await tester.tap(find.byKey(const Key('external-input-inject')));
      await tester.pumpAndSettle();
      expect(find.textContaining('command_unavailable'), findsOneWidget);
      expect(find.textContaining('未启用或正忙（503）'), findsOneWidget);
      expect(find.textContaining('先打开卡片标题行的开关'), findsOneWidget);
    });

    testWidgets('停用时静态显示 403 mod_disabled / 503 说明，且注入按钮仍可点', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_panel(_ctx(enabled: false, state: _state())));
      expect(
        find.byKey(const Key('external-input-disabled-notice')),
        findsOneWidget,
      );
      expect(find.textContaining('403 mod_disabled'), findsOneWidget);
      expect(find.textContaining('503 command_unavailable'), findsOneWidget);
      // 停用后仍可点：点下去正是「读得到的失败」（见上一条）。
      final FilledButton button = tester.widget<FilledButton>(
        find.byKey(const Key('external-input-inject')),
      );
      expect(button.onPressed, isNotNull);
    });

    testWidgets('空文本 → 本地拦下，不发命令', (WidgetTester tester) async {
      final List<String> commands = <String>[];
      await tester.pumpWidget(
        _panel(
          _ctx(
            state: _state(),
            config: const <String, Object?>{},
            onCommand: (String command,
                [Map<String, Object?> a = const <String, Object?>{}]) async {
              commands.add(command);
              return const ModCommandResult(ok: true);
            },
          ),
        ),
      );
      await tester.enterText(
        find.byKey(const Key('external-input-inject-text')),
        '   ',
      );
      await tester.tap(find.byKey(const Key('external-input-inject')));
      await tester.pumpAndSettle();
      expect(commands, isEmpty);
      expect(find.textContaining('不能为空'), findsOneWidget);
    });
  });
}
