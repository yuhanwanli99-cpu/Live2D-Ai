/// director 一等决策面板回归（产品级加强波次）。
///
/// 覆盖四条诉求：① `latest` 拆成带标签的行 + 最近 N 条列表；
/// ② **零投递说明**必须上屏（delivered=false / channel=none / 仅建议）；
/// ③ 空态可读（未启用 / 还没聊过 → 「先启用并聊一轮」）；
/// ④ `clear` 按钮（调用 ctx.onCommand，显示带错误码的结果）。
///
/// 面板**自己不做网络**：测试注入 fake `ModPanelContext` 即可零网络覆盖全部交互。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/director_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 一份与 Rust `state_json` 同形的运行态快照。
Map<String, Object?> directorState({int decisions = 2}) {
  const Map<String, Object?> latest = <String, Object?>{
    'seq': 2,
    'turn': '2',
    'emotion': 'happy',
    'intent': 'greeting',
    'suggested_tts': <String, Object?>{'speed': 1.08, 'pitch': 1.2},
    'closed': true,
    'delivered': false,
  };
  return <String, Object?>{
    'delivered': false,
    'channel': 'none',
    'turns_seen': 2,
    'turns_ended': 2,
    'decisions': decisions,
    'silent': 0,
    'errors': 0,
    'log_capacity': 20,
    'emotion_lexicon': 'builtin',
    'latest': decisions == 0 ? null : latest,
    'recent_decisions': decisions == 0
        ? const <Object?>[]
        : <Object?>[
            <String, Object?>{
              'seq': 1,
              'turn': '1',
              'emotion': 'sad',
              'intent': 'complaint',
              'suggested_tts': <String, Object?>{'speed': 0.9, 'pitch': 0.92},
              'closed': true,
              'delivered': false,
            },
            latest,
          ],
  };
}

ModPanelContext ctxFor({
  Map<String, Object?>? state,
  bool enabled = true,
  bool stateLoading = false,
  String? stateError,
  Future<void> Function()? onRefreshState,
  Future<ModCommandResult> Function(String, [Map<String, Object?>])? onCommand,
}) {
  return ModPanelContext(
    mod: ModInfo(
      id: 'director',
      name: '导演',
      version: '0.1.0',
      apiVersion: 1,
      enabled: enabled,
      status: enabled ? 'running' : 'disabled',
    ),
    state: state,
    stateLoading: stateLoading,
    stateError: stateError,
    onRefreshState: onRefreshState ?? () async {},
    onCommand:
        onCommand ??
        (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
            const ModCommandResult(
              ok: true,
              result: <String, Object?>{
                'cleared': 2,
                'counts': <String, Object?>{'decisions': 2},
              },
            ),
  );
}

Widget wrap(ModPanelContext ctx) => MaterialApp(
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

ModSettingsSpec directorSpec() => const ModSettingsSpec(
  modId: 'director',
  title: '导演',
  version: 1,
  fields: <ModSettingField>[
    ModSettingField(
      kind: ModFieldKind.number,
      key: 'log_capacity',
      label: '决策日志容量',
      defaultValue: 20,
      min: 1,
      max: 200,
    ),
  ],
);

void main() {
  group('一等面板：latest 拆成带标签的行 + 最近 N 条列表', () {
    testWidgets('latest 的每个字段都上屏（含中文标签与稳定码）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(ctxFor(state: directorState())));
      await tester.pumpAndSettle();

      // 顶部零投递说明（文字表意，不靠颜色）。
      expect(find.text(kDirectorZeroDeliveryNotice), findsOneWidget);
      expect(find.textContaining('不驱动动作'), findsOneWidget);
      expect(find.textContaining('不影响 TTS 输出'), findsOneWidget);
      expect(find.textContaining('delivered=false'), findsOneWidget);
      expect(find.textContaining('channel=none'), findsOneWidget);

      // latest 的五行。
      expect(find.text('本轮决策'), findsOneWidget);
      expect(find.textContaining('本轮情绪'), findsOneWidget);
      expect(find.textContaining('开心'), findsWidgets);
      expect(find.textContaining('happy'), findsWidgets);
      expect(find.textContaining('意图'), findsWidgets);
      expect(find.textContaining('打招呼'), findsWidgets);
      expect(find.textContaining('建议语速'), findsOneWidget);
      // 1.08 / 1.20 各出现两次（latest 行 + 最近列表里 #2 那一行）。
      expect(find.textContaining('1.08'), findsNWidgets(2));
      expect(find.textContaining('建议音高'), findsOneWidget);
      expect(find.textContaining('1.20'), findsNWidgets(2));
      expect(find.textContaining('是否已结项'), findsOneWidget);
      expect(find.textContaining('已结项'), findsWidgets);
      expect(find.textContaining('轮次'), findsOneWidget);

      // 最近 N 条（旧到新，带 turn id）。
      expect(find.text('最近决策（旧到新）'), findsOneWidget);
      expect(find.textContaining('#1'), findsOneWidget);
      expect(find.textContaining('#2'), findsOneWidget);
      expect(find.textContaining('难过'), findsWidgets);
      expect(find.textContaining('0.92'), findsOneWidget);

      // clear 按钮文案。
      expect(find.text('清空决策账本'), findsOneWidget);
    });

    testWidgets('数值格式：整数不拖小数点、非整数两位', (WidgetTester tester) async {
      expect(directorNumberText(1), '1');
      expect(directorNumberText(1.2), '1.20');
      expect(directorNumberText(0.92), '0.92');
      expect(directorNumberText(null), '—');
      // 未知稳定码回落原始码，不隐藏。
      expect(directorEmotionText('bogus'), 'bogus');
      expect(directorEmotionText('happy'), '开心（happy）');
      expect(directorIntentText('greeting'), '打招呼（greeting）');
    });
  });

  group('空态可读（不是空白）', () {
    testWidgets('未启用 → 「先启用并聊一轮」指引 + clear 按钮禁用', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(ctxFor(enabled: false, state: directorState(decisions: 0))),
      );
      await tester.pumpAndSettle();

      expect(find.textContaining('先启用并聊一轮'), findsOneWidget);
      expect(find.textContaining('未启用'), findsWidgets);
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('清空决策账本'),
          matching: find.byType(OutlinedButton),
        ),
      );
      expect(button.onPressed, isNull, reason: '未启用时命令必回 503，按钮不得假装能点');
    });

    testWidgets('已启用但还没聊过（decisions==0）→ 同一句指引', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(ctxFor(state: directorState(decisions: 0))),
      );
      await tester.pumpAndSettle();

      expect(find.textContaining('先启用并聊一轮'), findsOneWidget);
      expect(find.textContaining('还没有决策'), findsOneWidget);
      expect(find.text('最近决策（旧到新）'), findsNothing);
    });

    testWidgets('state 未取到 → 可处置文案，不空白', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(ctxFor(state: null)));
      await tester.pumpAndSettle();
      expect(find.textContaining('运行态暂时读不到'), findsOneWidget);
    });

    testWidgets('stateError 原样上屏（带错误码）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ctxFor(
            state: null,
            stateError: '运行态暂时读不到（未启用 / 未实现 state_json / worker 正忙，'
                '503 state_unavailable）',
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('state_unavailable'), findsOneWidget);
    });
  });

  group('clear 命令按钮', () {
    testWidgets('点按钮 → onCommand("clear") → 显示清空前 counts + 重取运行态', (
      WidgetTester tester,
    ) async {
      String? called;
      bool refreshed = false;
      await tester.pumpWidget(
        wrap(
          ctxFor(
            state: directorState(),
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              called = command;
              return const ModCommandResult(
                ok: true,
                result: <String, Object?>{
                  'cleared': 2,
                  'counts': <String, Object?>{'decisions': 2},
                },
              );
            },
            onRefreshState: () async {
              refreshed = true;
            },
          ),
        ),
      );
      await tester.pumpAndSettle();

      final Finder button = find.text('清空决策账本');
      await tester.ensureVisible(button);
      await tester.pumpAndSettle();
      await tester.tap(button);
      await tester.pumpAndSettle();

      expect(called, 'clear');
      expect(refreshed, isTrue, reason: '清空后必须重取运行态');
      expect(find.textContaining('已清空 2 条决策'), findsOneWidget);
      expect(find.textContaining('decisions=2'), findsOneWidget);
    });

    testWidgets('命令失败 → 带错误码的文案（不谎报成功）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ctxFor(
            state: directorState(),
            onCommand: (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
              throw const ApiException('command_unavailable', 'Mod worker 正忙');
            },
          ),
        ),
      );
      await tester.pumpAndSettle();

      final Finder button = find.text('清空决策账本');
      await tester.ensureVisible(button);
      await tester.pumpAndSettle();
      await tester.tap(button);
      await tester.pumpAndSettle();

      expect(find.textContaining('command_unavailable'), findsOneWidget);
      expect(find.textContaining('已清空'), findsNothing);
    });
  });

  group('注册面接线：面板出现在 director 的 Mod 卡片里', () {
    testWidgets('ModsSection 展开导演卡片 → 一等面板渲染', (WidgetTester tester) async {
      final ModInfo mod = ModInfo(
        id: 'director',
        name: '导演',
        version: '0.1.0',
        apiVersion: 1,
        enabled: true,
        status: 'running',
        config: const <String, Object?>{'log_capacity': 20},
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
                onLoadState: (String id) async => ModStateResult(
                  id: id,
                  enabled: true,
                  state: directorState(),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('导演'));
      await tester.pumpAndSettle();

      expect(find.text('本轮决策'), findsOneWidget);
      expect(find.text(kDirectorZeroDeliveryNotice), findsOneWidget);
      expect(find.text('最近决策（旧到新）'), findsOneWidget);
    });
  });
}
