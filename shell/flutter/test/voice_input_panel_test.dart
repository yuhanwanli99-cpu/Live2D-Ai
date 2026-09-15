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

/// 总闸开的 config（总闸 = 唤醒短语非空）。
const Map<String, Object?> _wakeConfig = <String, Object?>{
  'wake_phrase': '小爱',
};

/// 运行态快照（GET /state 的形状，只取面板读的字段）。
const Map<String, Object?> _stateWithGate = <String, Object?>{
  'wake_gate_open': true,
  'wake_phrase_set': true,
  'manual_enabled': true,
  'sidecar_script': '/repo/docs/examples/voice-sidecar/voice_sidecar.py',
  'sidecar_status': <String, Object?>{
    'state': 'idle',
    'exit_code': null,
    'last_audio': null,
    'stderr_tail': '',
  },
};

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
  Map<String, Object?>? state,
  bool enabled = true,
  String? stateError,
  Future<void> Function()? onRefreshState,
}) => ModPanelContext(
  mod: ModInfo(
    id: 'voice-input',
    name: '语音输入',
    version: '0.1.0',
    apiVersion: 1,
    enabled: enabled,
    status: enabled ? 'running' : 'disabled',
    config: config,
  ),
  state: state,
  stateLoading: false,
  stateError: stateError,
  onRefreshState: onRefreshState ?? () async {},
  onCommand: onCommand ?? _okCommand,
);

Widget _panel(ModPanelContext ctx) => _wrap(
  Builder(
    builder: (BuildContext context) => VoiceInputPanel().build(context, ctx)!,
  ),
);

/// 面板很长：按钮可能在滚动视口之外，点之前先滚到可见（否则 tap 会静默落空）。
Future<void> _tapVisible(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

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

  // ------------------------------------------------ L1：能力总闸 / 验证闸门 / sidecar

  testWidgets('总闸未开：醒目文案说清「总闸就是唤醒短语」且一切转写被拒', (
    WidgetTester tester,
  ) async {
    // config 没有 wake_phrase（缺省 = 总闸关），运行态也如实报 false。
    await tester.pumpWidget(
      _panel(
        _ctx(
          state: const <String, Object?>{'wake_gate_open': false, 'manual_enabled': true},
        ),
      ),
    );
    expect(find.textContaining('总闸就是「唤醒短语」'), findsOneWidget);
    expect(
      find.textContaining('总闸未开：所有转写都会被拒绝'),
      findsOneWidget,
      reason: '总闸关时必须有醒目文案',
    );
    expect(
      find.textContaining('403 voice_gate_closed'),
      findsWidgets,
      reason: '必须点名失败的稳定码',
    );
  });

  testWidgets('总闸开（config + state 一致）：不出现「总闸未开」告警', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _panel(_ctx(config: _wakeConfig, state: _stateWithGate)),
    );
    expect(
      find.textContaining('总闸未开：所有转写都会被拒绝'),
      findsNothing,
    );
    expect(find.textContaining('已开（wake_phrase 已设置）'), findsOneWidget);
  });

  testWidgets('手动闸关：醒目文案点名 voice_manual_off', (WidgetTester tester) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          config: const <String, Object?>{'wake_phrase': '小爱', 'manual_enabled': false},
          state: const <String, Object?>{
            'wake_gate_open': true,
            'manual_enabled': false,
          },
        ),
      ),
    );
    expect(find.textContaining('手动闸已关'), findsOneWidget);
    expect(find.textContaining('403 voice_manual_off'), findsWidgets);
  });

  testWidgets('验证闸门：onCommand 收到 inject + text，并把 accepted/code 摊开', (
    WidgetTester tester,
  ) async {
    final List<(String, Map<String, Object?>)> calls =
        <(String, Map<String, Object?>)>[];
    await tester.pumpWidget(
      _panel(
        _ctx(
          config: _wakeConfig,
          state: _stateWithGate,
          onCommand:
              (
                String command, [
                Map<String, Object?> args = const <String, Object?>{},
              ]) async {
                calls.add((command, args));
                return const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'ok': true,
                    'accepted': true,
                    'text': '开灯',
                    'backend': 'mock',
                    'locale': 'zh-CN',
                    'rejected_code': null,
                    'message': '已注入主链',
                  },
                );
              },
        ),
      ),
    );
    await tester.enterText(find.byType(TextField).first, '小爱 开灯');
    await _tapVisible(tester, find.text('验证闸门'));
    expect(calls.length, 1);
    expect(calls.single.$1, 'inject');
    expect(calls.single.$2['text'], '小爱 开灯');
    expect(find.textContaining('已注入主链：开灯'), findsOneWidget);
  });

  testWidgets('验证闸门：总闸关时把 rejected_code 如实显示（壳内拒绝证据）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          onCommand:
              (
                String command, [
                Map<String, Object?> args = const <String, Object?>{},
              ]) async => const ModCommandResult(
                ok: true,
                result: <String, Object?>{
                  'ok': false,
                  'accepted': false,
                  'text': '你好',
                  'rejected_code': 'voice_gate_closed',
                  'message': '能力总闸未开：先在 Mod 配置里填写「唤醒短语」',
                },
              ),
        ),
      ),
    );
    await tester.enterText(find.byType(TextField).first, '你好');
    await _tapVisible(tester, find.text('验证闸门'));
    expect(find.textContaining('被拒绝（voice_gate_closed）'), findsOneWidget);
    expect(find.textContaining('唤醒短语'), findsWidgets);
  });

  testWidgets('未启用：命令按钮禁用并给 503 说明', (WidgetTester tester) async {
    await tester.pumpWidget(_panel(_ctx(enabled: false)));
    // 「验证闸门」「拉起 sidecar」「检查配置」三个按钮都禁用。
    for (final String label in <String>['检查配置', '验证闸门', '拉起 sidecar']) {
      final Finder button = find.widgetWithText(FilledButton, label);
      expect(button, findsOneWidget, reason: '按钮 $label 必须在');
      final FilledButton widget = tester.widget<FilledButton>(button);
      expect(widget.onPressed, isNull, reason: '$label 未启用时必须禁用');
    }
    expect(find.textContaining('503 command_unavailable'), findsWidgets);
  });

  testWidgets('拉起 sidecar：onCommand 收到 run_sidecar + 路径/transcriber/url', (
    WidgetTester tester,
  ) async {
    final List<(String, Map<String, Object?>)> calls =
        <(String, Map<String, Object?>)>[];
    int refreshed = 0;
    await tester.pumpWidget(
      _panel(
        _ctx(
          enabled: true,
          state: _stateWithGate,
          onRefreshState: () async {
            refreshed++;
          },
          onCommand:
              (
                String command, [
                Map<String, Object?> args = const <String, Object?>{},
              ]) async {
                calls.add((command, args));
                return const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'spawned': true,
                    'pid': 4321,
                    'url': 'http://127.0.0.1:18080/api/v1/voice/transcript',
                  },
                );
              },
        ),
      ),
    );
    // TextField 顺序：0 = 验证闸门注入文本，1 = 音频路径，2 = transcriber，3 = url。
    await tester.enterText(
      find.byType(TextField).at(1),
      'docs/examples/voice-sidecar/fixtures/fake_zh.wav',
    );
    await _tapVisible(tester, find.text('拉起 sidecar'));
    expect(calls.single.$1, 'run_sidecar');
    expect(
      calls.single.$2['audio_path'],
      'docs/examples/voice-sidecar/fixtures/fake_zh.wav',
    );
    expect(calls.single.$2['transcriber'], 'fake');
    expect(calls.single.$2['url'], contains('/api/v1/voice/transcript'));
    expect(find.textContaining('已拉起 sidecar（pid 4321）'), findsOneWidget);
    expect(refreshed, 1, reason: '拉起后要刷一次运行态');
  });

  // ---------------------------------------------------- voiceTranscriptUrl 纯函数

  group('voiceTranscriptUrl：浏览器 origin → transcript 端点', () {
    test('http origin 带端口：拼上固定路径', () {
      expect(
        voiceTranscriptUrl(Uri.parse('http://127.0.0.1:18080/app/')),
        'http://127.0.0.1:18080/api/v1/voice/transcript',
      );
    });

    test('https origin（无端口）不加端口', () {
      expect(
        voiceTranscriptUrl(Uri.parse('https://live2d.example/app/')),
        'https://live2d.example/api/v1/voice/transcript',
      );
    });

    test('null / 非 http(s) / 无 host 都回退到 127.0.0.1:18080', () {
      const String fallback = 'http://127.0.0.1:18080/api/v1/voice/transcript';
      expect(voiceTranscriptUrl(null), fallback);
      expect(voiceTranscriptUrl(Uri.parse('file:///tmp/x')), fallback);
      expect(voiceTranscriptUrl(Uri.parse('about:blank')), fallback);
    });
  });
}
