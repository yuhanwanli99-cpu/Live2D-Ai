/// voice-input 产品面板的组件契约（产品级加强波次）。
///
/// 断言三件产品级诉求：
/// 1. 人话解释真的在面板上（mock 没有 ASR / sidecar 推模式、Rust 不开 socket /
///    locale 只管归一化，不是识别语言开关）；
/// 2. 当前生效值来自 Mod config（缺省 mock / zh-CN；配错时不假装有效）；
/// 3. 失败码与处置在面板上，「检查配置」走 onCommand 且失败带错误码。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/voice_input_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

Future<ModCommandResult> _okCommand(
  String command, [
  Map<String, Object?> args = const <String, Object?>{},
]) async => const ModCommandResult(ok: true);

ModPanelContext _ctx({
  Map<String, Object?> config = const <String, Object?>{},
  Future<ModCommandResult> Function(String, [Map<String, Object?>])? onCommand,
}) => ModPanelContext(
  mod: ModInfo(
    id: 'voice-input',
    name: '语音输入',
    version: '0.1.0',
    apiVersion: 1,
    enabled: true,
    status: 'running',
    config: config,
  ),
  state: null,
  stateLoading: false,
  stateError: null,
  onRefreshState: () async {},
  onCommand: onCommand ?? _okCommand,
);

Widget _panel(ModPanelContext ctx) => _wrap(
  Builder(
    builder: (BuildContext context) => VoiceInputPanel().build(context, ctx)!,
  ),
);

void main() {
  testWidgets('人话解释：mock / sidecar 推模式 / Rust 不开 socket / locale 只管归一化', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_panel(_ctx()));
    expect(find.textContaining('没有 ASR'), findsWidgets);
    expect(
      find.textContaining('从不开 socket'),
      findsOneWidget,
      reason: '必须说清 Rust 不主动请求 sidecar',
    );
    expect(
      find.textContaining('识别语言开关'),
      findsWidgets,
      reason: 'locale 不是识别语言开关，必须写出来',
    );
    expect(find.textContaining('归一化'), findsWidgets);
  });

  testWidgets('当前生效值：缺省是 mock / zh-CN 且标出「缺省」', (WidgetTester tester) async {
    await tester.pumpWidget(_panel(_ctx()));
    expect(find.textContaining('backend：mock（缺省，未显式设置）'), findsOneWidget);
    expect(find.textContaining('locale：zh-CN（缺省，未显式设置）'), findsOneWidget);
  });

  testWidgets('当前生效值：显式设置按 config 显示', (WidgetTester tester) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          config: const <String, Object?>{
            'backend': 'sidecar',
            'locale': 'en-US',
          },
        ),
      ),
    );
    expect(find.textContaining('backend：sidecar（已设置）'), findsOneWidget);
    expect(find.textContaining('locale：en-US（已设置）'), findsOneWidget);
  });

  testWidgets('配错不假装有效：未知 backend 回落 mock、非法 locale 有告警', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          config: const <String, Object?>{
            'backend': 'whisper',
            'locale': '!!',
          },
        ),
      ),
    );
    expect(
      find.textContaining('backend：mock（配置值 whisper 不认识，已回落 mock）'),
      findsOneWidget,
    );
    expect(find.textContaining('locale：!!（已设置）'), findsOneWidget);
    expect(find.textContaining('不是合法 BCP-47'), findsOneWidget);
  });

  testWidgets('出错怎么办：覆盖 mod_disabled / busy / sidecar 没起 / backend 配错', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_panel(_ctx()));
    expect(find.textContaining('mod_disabled'), findsWidgets);
    expect(find.textContaining('busy'), findsWidgets);
    expect(find.textContaining('连接被拒'), findsWidgets);
    expect(find.textContaining('backend 配错'), findsOneWidget);
    // sidecar 退出码表在面板上（码 -> 人话）。
    for (final String code in <String>['0', '2', '3', '4', '5']) {
      expect(
        find.textContaining('退出码 $code：'),
        findsOneWidget,
        reason: '退出码 $code 的一行必须在面板上',
      );
    }
  });

  testWidgets('检查配置：调 onCommand(selftest) 并把脱敏结论摊开', (WidgetTester tester) async {
    final List<String> calls = <String>[];
    await tester.pumpWidget(
      _panel(
        _ctx(
          onCommand:
              (
                String command, [
                Map<String, Object?> args = const <String, Object?>{},
              ]) async {
                calls.add(command);
                return const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'ok': true,
                    'backend': 'sidecar',
                    'locale': 'zh-CN',
                    'locale_profile': 'cjk',
                    'token_set': true,
                    'route': 'accept_push',
                    'opens_network': false,
                    'problems': <Object?>[],
                    'notes': <Object?>[],
                  },
                );
              },
        ),
      ),
    );
    await tester.tap(find.text('检查配置'));
    await tester.pumpAndSettle();
    expect(calls, <String>['selftest']);
    expect(find.textContaining('配置自洽'), findsOneWidget);
    expect(find.textContaining('token=已设置'), findsOneWidget);
    expect(
      find.textContaining('Rust 开网络=否'),
      findsOneWidget,
      reason: '自检结论要能证明 Rust 不开 socket',
    );
  });

  testWidgets('检查配置失败：错误带码呈现（command_unavailable）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          onCommand:
              (
                String command, [
                Map<String, Object?> args = const <String, Object?>{},
              ]) async {
                throw const ApiException('command_unavailable', 'Mod 未启用或正忙，可重试');
              },
        ),
      ),
    );
    await tester.tap(find.text('检查配置'));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('command_unavailable'),
      findsOneWidget,
      reason: '错误必须带码（项目错误契约）',
    );
  });
}
