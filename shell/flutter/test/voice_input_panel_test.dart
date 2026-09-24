/// voice-input 产品面板的组件契约（2026-09-15 瘦身版）。
///
/// 断言：
/// 1. **主区只有四件事**（一行总闸状态 / 一句链路白话 / 检查配置 / 关闭提示）——
///    旧的 backend/locale 长篇解释、失败码长文、退出码表都不在主区；
/// 2. 「高级」折叠里有验证闸门（inject）与 sidecar 拉起，且退出码长文在其中；
/// 3. 失败码与错误码如实呈现，「检查配置」走 onCommand；
/// 4. 未启用时命令按钮禁用并给 503 说明。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/voice_input_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 自定义唤醒词的 config。
const Map<String, Object?> _wakeConfig = <String, Object?>{'wake_phrase': '小爱'};

/// 运行态快照（GET /state 的形状，只取面板读的字段）。
const Map<String, Object?> _stateWithGate = <String, Object?>{
  'wake_gate_open': true,
  'wake_phrase_set': true,
  'manual_enabled': true,
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

/// 展开「高级」折叠（验证闸门 / sidecar / 退出码都在里面）。
Future<void> _openAdvanced(WidgetTester tester) =>
    _tapVisible(tester, find.text('高级'));

void main() {
  testWidgets('主区只有一行总闸状态 + 一句链路白话（缺省唤醒词 小可爱）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_panel(_ctx()));
    // 键缺失 → 缺省词。
    expect(find.textContaining('唤醒词「小可爱」'), findsOneWidget);
    expect(find.textContaining('缺省'), findsWidgets);
    expect(
      find.textContaining('后面那句会进聊天'),
      findsOneWidget,
      reason: '必须一句话说清「说唤醒词 → 进聊天 → LLM → TTS」',
    );
    // 旧的逐条解释不在主区。
    expect(find.textContaining('从不开 socket'), findsNothing);
    expect(find.textContaining('识别语言开关'), findsNothing);
    expect(find.textContaining('backend：'), findsNothing);
  });

  testWidgets('自定义唤醒词按 config 显示', (WidgetTester tester) async {
    await tester.pumpWidget(_panel(_ctx(config: _wakeConfig, state: _stateWithGate)));
    expect(find.textContaining('唤醒词「小爱」'), findsOneWidget);
    expect(find.textContaining('自定义'), findsOneWidget);
  });

  testWidgets('总闸显式关：醒目文案点名 403 voice_gate_closed', (WidgetTester tester) async {
    await tester.pumpWidget(
      _panel(
        _ctx(
          state: const <String, Object?>{
            'wake_gate_open': false,
            'manual_enabled': true,
          },
        ),
      ),
    );
    expect(find.textContaining('总闸'), findsWidgets);
    expect(find.textContaining('403 voice_gate_closed'), findsOneWidget);
  });

  testWidgets('手动闸关：点名 voice_manual_off', (WidgetTester tester) async {
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
    expect(find.textContaining('403 voice_manual_off'), findsOneWidget);
  });

  testWidgets('高级折叠：验证闸门（inject）与 sidecar 都在里面，不在主区', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_panel(_ctx(config: _wakeConfig, state: _stateWithGate)));
    // 收起时主区看不到它们。
    expect(find.text('验证闸门'), findsNothing);
    expect(find.text('拉起 sidecar'), findsNothing);

    await _openAdvanced(tester);
    expect(find.text('验证闸门'), findsOneWidget);
    expect(find.text('拉起 sidecar'), findsOneWidget);
    // sidecar 退出码长文也在高级里。
    for (final String code in <String>['0', '2', '3', '4', '5']) {
      expect(find.textContaining('退出码 $code：'), findsOneWidget);
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
    expect(find.textContaining('Rust 开网络=否'), findsOneWidget);
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

  testWidgets('验证闸门：onCommand 收到 inject + text，并把 accepted 摊开', (
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
    await _openAdvanced(tester);
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
                  'message': '语音总闸被显式关闭',
                },
              ),
        ),
      ),
    );
    await _openAdvanced(tester);
    await tester.enterText(find.byType(TextField).first, '你好');
    await _tapVisible(tester, find.text('验证闸门'));
    expect(find.textContaining('被拒绝（voice_gate_closed）'), findsOneWidget);
  });

  testWidgets('未启用：命令按钮禁用并给 503 说明', (WidgetTester tester) async {
    await tester.pumpWidget(_panel(_ctx(enabled: false)));
    final FilledButton check = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '检查配置'),
    );
    expect(check.onPressed, isNull);
    expect(find.textContaining('503 command_unavailable'), findsWidgets);

    await _openAdvanced(tester);
    for (final String label in <String>['验证闸门', '拉起 sidecar']) {
      final Finder button = find.widgetWithText(FilledButton, label);
      expect(button, findsOneWidget, reason: '按钮 $label 必须在');
      expect(
        tester.widget<FilledButton>(button).onPressed,
        isNull,
        reason: '$label 未启用时必须禁用',
      );
    }
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
    await _openAdvanced(tester);
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
