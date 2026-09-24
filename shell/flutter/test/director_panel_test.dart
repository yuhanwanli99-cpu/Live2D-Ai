/// director 面板回归（2026-09-15 瘦身版）。
///
/// 覆盖：① 顶部一句话说明（启用后按情绪触发表情 / 短动作）；
/// ② 主区只显示**本轮预设**（latest.preset_id）+ 情绪/意图一行注脚；
/// ③ 空态可读（未启用 / 还没聊过 → 「先启用并聊一轮」）；
/// ④ clear 按钮（调用 ctx.onCommand，显示带错误码的结果）。
///
/// 面板**自己不做网络**：测试注入 fake ModPanelContext 即可零网络覆盖全部交互。
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
Map<String, Object?> directorState({int decisions = 2, String preset = 'smile'}) {
  final Map<String, Object?> latest = <String, Object?>{
    'seq': 2,
    'turn': '2',
    'emotion': 'happy',
    'intent': 'greeting',
    'suggested_tts': <String, Object?>{'speed': 1.08, 'pitch': 1.2},
    'preset_id': preset,
    'closed': true,
    'delivered': preset != 'none',
  };
  return <String, Object?>{
    'delivered': preset != 'none',
    'channel': 'preset',
    'turns_seen': 2,
    'turns_ended': 2,
    'decisions': decisions,
    'silent': 0,
    'errors': 0,
    'log_capacity': 20,
    'emotion_lexicon': 'builtin',
    'latest': decisions == 0 ? null : latest,
    'recent_decisions': decisions == 0 ? const <Object?>[] : <Object?>[latest],
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

/// 与 Rust 侧同形的 3 个预设 Select（缺省 = 内置映射，不是第一个选项 none）。
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
  ],
);

void main() {
  group('瘦身主区：本轮预设 + 一行说明', () {
    testWidgets('预设结论上屏；旧的「最近决策 / 建议语速」不再出现', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(ctxFor(state: directorState())));
      await tester.pumpAndSettle();

      // 顶部一句话说明。
      expect(find.text(kDirectorPresetNotice), findsOneWidget);
      expect(find.textContaining('按情绪触发'), findsOneWidget);
      expect(find.textContaining('不改 TTS 输出'), findsOneWidget);

      // 2026-09-22：紧跟一句「与表演层谁主谁退」——防止用户以为两套大脑抢 cue。
      expect(find.text(kDirectorLegacyNotice), findsOneWidget);
      expect(find.textContaining('此处 staging_* 仅兼容旧配置'), findsOneWidget);
      expect(find.textContaining('设置 → LLM → 表演层'), findsOneWidget);
      expect(find.textContaining('收不到 SentenceReady'), findsOneWidget);

      // 本轮预设（latest.preset_id → 中文 + 稳定码）。
      expect(find.text('本轮预设'), findsOneWidget);
      expect(find.text('表情（smile）'), findsOneWidget);

      // 一行注脚（解释「为什么选这条」），不做成表格。
      expect(find.textContaining('依据：开心（happy）'), findsOneWidget);
      expect(find.textContaining('打招呼（greeting）'), findsOneWidget);

      // 删掉的东西不得回来。
      expect(find.text('最近决策（旧到新）'), findsNothing);
      expect(find.textContaining('建议语速'), findsNothing);
      expect(find.textContaining('建议音高'), findsNothing);
      expect(find.textContaining('本轮不做 L1'), findsNothing);
      expect(find.textContaining('未接任何下行通道'), findsNothing);

      expect(find.text('清空决策账本'), findsOneWidget);
    });

    testWidgets('preset=none → 「不投递（本轮无动作）」', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(ctxFor(state: directorState(preset: 'none'))),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('不投递'), findsOneWidget);
    });

    testWidgets('纯函数：预设 / 情绪 / 意图 / 数值文案', (WidgetTester tester) async {
      // 新 id：表情 / 短动作按包通道分类。
      expect(directorPresetText('unhappy'), '表情（unhappy）');
      expect(directorPresetText('tilt_left'), '短动作（tilt_left）');
      // 另一个表情包（新 id）。
      expect(directorPresetText('surprised'), '表情（surprised）');
      expect(directorPresetText('nod'), '短动作（nod）');
      expect(directorPresetText('none'), '不投递（本轮无动作）');
      expect(directorPresetText(null), '不投递（本轮无动作）');
      expect(directorPresetText('bogus'), '短动作（bogus）');
      expect(directorEmotionText('happy'), '开心（happy）');
      expect(directorEmotionText('bogus'), 'bogus');
      expect(directorIntentText('greeting'), '打招呼（greeting）');
      expect(directorNumberText(1), '1');
      expect(directorNumberText(1.2), '1.20');
      expect(directorNumberText(null), '—');
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
      await tester.pumpWidget(wrap(ctxFor(state: directorState(decisions: 0))));
      await tester.pumpAndSettle();

      expect(find.textContaining('先启用并聊一轮'), findsOneWidget);
      expect(find.textContaining('还没有决策'), findsOneWidget);
      expect(find.text('本轮预设'), findsNothing);
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
    testWidgets('ModsSection 展开导演卡片 → 瘦身面板渲染；通用运行态块不渲染', (
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

      expect(find.text('本轮预设'), findsOneWidget);
      expect(find.text(kDirectorPresetNotice), findsOneWidget);
      // 通用运行态块整块不渲染（showRuntimeState=false）。
      expect(find.text('运行态（只读）'), findsNothing);
      expect(find.text('刷新运行态'), findsNothing);
    });
  });
}
