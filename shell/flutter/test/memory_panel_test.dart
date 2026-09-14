/// `memory` 产品面板（产品级加强波次）的 Flutter 回归。
///
/// 覆盖：条数 / 命中 / 注入 / 淘汰 / 上轮命中渲染、「清空记忆库」触发
/// `onCommand('clear')`、成功/失败带码文案、注入开关说明（!= Mod 启停）、
/// 与 persona 的 last-writer-wins 说明、验收步骤的文档指向。
///
/// 契约真源：`crates/live2d-ai-mod-memory/src/lib.rs`（`state_json` / `command`）
/// + `docs/architecture/memory-mod-v0.md`。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/memory_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 与 Rust `memory_settings_spec()` 同形的四字段 schema（**没有**第二个 enabled）。
ModSettingsSpec memorySpec() => ModSettingsSpec(
  modId: 'memory',
  title: '本地记忆',
  version: 1,
  fields: <ModSettingField>[
    const ModSettingField(
      kind: ModFieldKind.string,
      key: 'store_path',
      label: '记忆库路径',
    ),
    const ModSettingField(
      kind: ModFieldKind.number,
      key: 'top_k',
      label: '每轮注入条数',
      min: 1,
      max: 10,
    ),
    const ModSettingField(
      kind: ModFieldKind.number,
      key: 'max_records',
      label: '条数上限',
      min: 1,
      max: 10000,
    ),
    const ModSettingField(
      kind: ModFieldKind.bool,
      key: 'enabled_injection',
      label: '把检索结果注入下一轮提示词',
      defaultValue: true,
    ),
  ],
);

ModInfo memoryMod({bool enabled = true}) => ModInfo(
  id: 'memory',
  name: '本地记忆',
  version: '0.1.0',
  apiVersion: 1,
  enabled: enabled,
  status: enabled ? 'running' : 'disabled',
  config: const <String, Object?>{
    'top_k': 3,
    'max_records': 200,
    'enabled_injection': true,
  },
  settingsSpec: memorySpec(),
);

/// 与 Rust `state_json` 同形的运行态快照。
Map<String, Object?> memoryState() => const <String, Object?>{
  'store_path': '/tmp/memory.jsonl',
  'records': 12,
  'top_k': 3,
  'max_records': 200,
  'enabled_injection': true,
  'turns_seen': 20,
  'writes': 20,
  'hits': 17,
  'injects': 4,
  'errors': 0,
  'evicted': 2,
  'last_hits': 2,
};

typedef CommandHandler =
    Future<ModCommandResult> Function(String command, [Map<String, Object?> args]);

ModPanelContext panelContext({
  Map<String, Object?>? state,
  bool enabled = true,
  bool stateLoading = false,
  String? stateError,
  CommandHandler? onCommand,
}) => ModPanelContext(
  mod: memoryMod(enabled: enabled),
  state: state,
  stateLoading: stateLoading,
  stateError: stateError,
  onRefreshState: () async {},
  onCommand:
      onCommand ??
      (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
          const ModCommandResult(ok: true),
);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 直接构造面板（不经过 `ModsSection`）：`onCommand` 可注入 fake，零网络。
Widget panelWidget(ModPanelContext ctx) => Builder(
  builder: (BuildContext context) => MemoryPanel().build(context, ctx)!
);

void main() {
  group('纯函数：计数 / 摘要 / 失败码文案', () {
    test('memoryCountText：数字照抄，缺失/非数字 → —（不假装是 0）', () {
      expect(memoryCountText(12), '12');
      expect(memoryCountText(0), '0');
      expect(memoryCountText(3.0), '3');
      expect(memoryCountText(null), '—');
      expect(memoryCountText('12'), '—');
    });

    test('memorySummaryLine：五项齐全，缺项显示 —', () {
      final String line = memorySummaryLine(memoryState());
      expect(line, contains('记忆条数 12 条'));
      expect(line, contains('累计命中 17 次'));
      expect(line, contains('注入 4 轮'));
      expect(line, contains('已淘汰 2 条'));
      expect(line, contains('上轮命中 2 条'));
      final String empty = memorySummaryLine(null);
      expect(empty, contains('记忆条数 — 条'));
      expect(empty, contains('上轮命中 — 条'));
    });

    test('memoryClearErrorMessage：每个码都带码且可处置', () {
      expect(
        memoryClearErrorMessage(
          const ApiException('command_unavailable', '忙', status: 503),
        ),
        contains('command_unavailable'),
      );
      expect(
        memoryClearErrorMessage(
          const ApiException('unsupported_command', '不认', status: 409),
        ),
        contains('unsupported_command'),
      );
      expect(
        memoryClearErrorMessage(
          const ApiException('command_failed', '存储不可写', status: 409),
        ),
        contains('command_failed'),
      );
      expect(
        memoryClearErrorMessage(
          const ApiException('not_found', '不在册', status: 404),
        ),
        contains('not_found'),
      );
    });
  });

  group('面板元数据', () {
    test('modId 与 stateLabels（新增 records）', () {
      const MemoryPanel panel = MemoryPanel();
      expect(panel.modId, 'memory');
      final Map<String, String> labels = panel.stateLabels;
      expect(labels['records'], '当前条数');
      expect(labels['hits'], '命中次数');
      expect(labels['injects'], '注入轮数');
      expect(labels['evicted'], '已淘汰');
      expect(labels['last_hits'], '上轮命中');
      expect(labels['enabled_injection'], '注入开关');
    });
  });

  group('运行态渲染：条数 / 命中 / 注入 / 淘汰 / 上轮命中', () {
    testWidgets('五项计数都上屏（文字，不靠颜色）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      expect(find.text('记忆概览'), findsOneWidget);
      expect(find.textContaining('记忆条数 12 条'), findsOneWidget);
      expect(find.textContaining('累计命中 17 次'), findsOneWidget);
      expect(find.textContaining('注入 4 轮'), findsOneWidget);
      expect(find.textContaining('已淘汰 2 条'), findsOneWidget);
      expect(find.textContaining('上轮命中 2 条'), findsOneWidget);
    });

    testWidgets('读取中 / 读取失败都如实说，失败带错误码', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(stateLoading: true))),
      );
      expect(find.text('正在读取运行态…'), findsOneWidget);

      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              stateError: '运行态暂时读不到（未启用 / 未实现 state_json / worker 正忙，'
                  '503 state_unavailable）',
            ),
          ),
        ),
      );
      expect(find.textContaining('state_unavailable'), findsOneWidget);
    });
  });

  group('清空记忆库：触发 onCommand(clear) + 带码文案', () {
    testWidgets('点击清空 → onCommand(clear)，成功文案含清掉/现存条数', (
      WidgetTester tester,
    ) async {
      final List<String> commands = <String>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              onCommand:
                  (String command,
                      [Map<String, Object?> args =
                          const <String, Object?>{}]) async {
                commands.add(command);
                return const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'records': 0,
                    'cleared': true,
                    'removed': 3,
                    'residue': false,
                  },
                );
              },
            ),
          ),
        ),
      );

      await tester.tap(find.text('清空记忆库'));
      await tester.pumpAndSettle();

      expect(commands, <String>['clear']);
      expect(find.textContaining('已清空记忆库'), findsOneWidget);
      expect(find.textContaining('清掉 3 条'), findsOneWidget);
      expect(find.textContaining('现存 0 条'), findsOneWidget);
    });

    testWidgets('有注入残留时如实提示（strip_residue 语义）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              onCommand:
                  (String command,
                      [Map<String, Object?> args =
                          const <String, Object?>{}]) async =>
                      const ModCommandResult(
                        ok: true,
                        result: <String, Object?>{
                          'records': 0,
                          'cleared': true,
                          'removed': 2,
                          'residue': true,
                        },
                      ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('清空记忆库'));
      await tester.pumpAndSettle();
      expect(find.textContaining('仍留着上一轮注入的记忆块'), findsOneWidget);
    });

    testWidgets('失败：文案带错误码（不谎报成功）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              onCommand:
                  (String command,
                      [Map<String, Object?> args =
                          const <String, Object?>{}]) async =>
                      throw const ApiException(
                        'command_failed',
                        '存储不可写',
                        status: 409,
                      ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('清空记忆库'));
      await tester.pumpAndSettle();
      expect(find.textContaining('command_failed'), findsOneWidget);
      expect(find.textContaining('已清空记忆库'), findsNothing);
    });

    testWidgets('Mod 未启用：按钮禁用 + 说明', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState(), enabled: false))),
      );
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.widgetWithText(OutlinedButton, '清空记忆库'),
      );
      expect(button.onPressed, isNull, reason: '未启用就不该假装能清空');
      expect(find.textContaining('Mod 未启用'), findsOneWidget);
    });
  });

  group('说明文案：注入开关 / persona 策略 / 验收步骤指向', () {
    testWidgets('注入开关说明它 != Mod 启停', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      expect(find.textContaining('enabled_injection'), findsOneWidget);
      expect(find.textContaining('它不是 Mod 启停'), findsOneWidget);
    });

    testWidgets('persona 策略：后写覆盖、不合并、不仲裁；并给可执行建议', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      expect(find.textContaining('谁后写谁覆盖'), findsOneWidget);
      expect(find.textContaining('不合并、不仲裁'), findsOneWidget);
      expect(find.textContaining('就别同时开 memory 注入'), findsOneWidget);
    });

    testWidgets('固定步骤的短提示 + 指向文档第 12 节', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      expect(find.textContaining('只对下一轮生效'), findsOneWidget);
      expect(find.textContaining(kMemoryDocPath), findsOneWidget);
      expect(find.textContaining('第 12 节'), findsOneWidget);
    });
  });
}
