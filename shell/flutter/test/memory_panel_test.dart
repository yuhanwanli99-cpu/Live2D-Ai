/// `memory` 产品面板（2026-10-08 人话版）的 Flutter 回归。
///
/// 覆盖：概览只剩条数、「记忆列表」两条上屏、导入/编辑/删除的 onCommand args、
/// 删除二次确认、`activeSessionId == null` 的降级文案、清空、带码失败文案、
/// 「压缩过的记忆」块（有正文才画）、11 个 hiddenKeys。
///
/// 契约真源：`crates/live2d-ai-mod-memory/src/commands.rs`（命令 args/返回）
/// + `src/lib.rs` 的 `state_json` + `docs/architecture/memory-mod-v0.md`。

library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/memory_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
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
      // 2026-10-08：与 Rust `memory_settings_spec()` 的 label 逐字一致。
      label: '聊天时用上这些记忆',
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

/// 与 Rust `state_json` 同形的运行态快照（含 L1 会话键）。
Map<String, Object?> memoryState() => const <String, Object?>{
  'store_path': '/tmp/memory.jsonl',
  'bucket_path': '/tmp/sessions/session-a.memory.jsonl',
  'session_scoped': true,
  'active_session': 'session-a',
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

/// 一次 `onCommand` 调用的记录（断言 args 用）。
class RecordedCall {
  RecordedCall(this.command, this.args);

  final String command;
  final Map<String, Object?> args;
}

/// 造一个 `list` 成功返回体（两条记录的默认形状）。
ModCommandResult listReply({
  List<Map<String, Object?>> records = const <Map<String, Object?>>[],
  int? total,
  String bucket = '/tmp/sessions/session-a.memory.jsonl',
}) => ModCommandResult(
  ok: true,
  result: <String, Object?>{
    'ok': true,
    'records': records,
    'total': total ?? records.length,
    'bucket': bucket,
  },
);

ModPanelContext panelContext({
  Map<String, Object?>? state,
  bool enabled = true,
  bool stateLoading = false,
  String? stateError,
  CommandHandler? onCommand,
  String? activeSessionId,
  ValueChanged<String>? onModChanged,
  Future<void> Function()? onRefreshState,
}) => ModPanelContext(
  mod: memoryMod(enabled: enabled),
  state: state,
  stateLoading: stateLoading,
  stateError: stateError,
  activeSessionId: activeSessionId,
  onModChanged: onModChanged,
  onRefreshState: onRefreshState ?? () async {},
  onCommand:
      onCommand ??
      (String command, [Map<String, Object?> args = const <String, Object?>{}]) async =>
          listReply(),
);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 直接构造面板（不经过 `ModsSection`）：`onCommand` 可注入 fake，零网络。
Widget panelWidget(ModPanelContext ctx) => Builder(
  builder: (BuildContext context) => MemoryPanel().build(context, ctx)!,
);

/// 一个记录调用并统一回 `list` 的 fake。
CommandHandler recordingHandler(
  List<RecordedCall> calls, {
  List<Map<String, Object?>> records = const <Map<String, Object?>>[],
  ModCommandResult? reply,
}) => (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
  calls.add(RecordedCall(command, args));
  if (command == 'list') return listReply(records: records);
  return reply ?? const ModCommandResult(ok: true, result: <String, Object?>{'records': 0});
};

void main() {
  group('纯函数：计数 / 摘要 / 桶说明 / args / 失败码文案', () {
    test('memoryCountText：数字照抄，缺失/非数字 → —（不假装是 0）', () {
      expect(memoryCountText(12), '12');
      expect(memoryCountText(0), '0');
      expect(memoryCountText(3.0), '3');
      expect(memoryCountText(null), '—');
      expect(memoryCountText('12'), '—');
    });

    test('memorySummaryLine：只说「记住了 N 条」，缺项显示 —（不假装是 0）', () {
      expect(memorySummaryLine(memoryState()), '记住了 12 条');
      expect(memorySummaryLine(null), '记住了 — 条');
      // 2026-10-08：命中 / 注入 / 淘汰 / 上轮命中 不再上屏。
      for (final String banned in <String>['命中', '注入', '淘汰']) {
        expect(memorySummaryLine(memoryState()).contains(banned), isFalse);
      }
    });

    test('memoryBucketNotice：没有对话说清「先放一起」，有对话说「只看这次」；不出现「桶」', () {
      final String fallback = memoryBucketNotice(null);
      expect(
        fallback,
        '还没有对话。记住的内容先放到一起，开始聊天后再按这次对话分开。',
      );
      final String scoped = memoryBucketNotice('  s-1  ');
      expect(scoped, '只看这次对话记住的内容。');
      for (final String banned in <String>['桶', '全局桶']) {
        expect(fallback.contains(banned), isFalse, reason: '不得出现「$banned」');
        expect(scoped.contains(banned), isFalse, reason: '不得出现「$banned」');
      }
    });


    test('memorySummaryState / text：非对象与空正文都按「没有」处理', () {
      expect(memorySummaryState(null), isNull);
      expect(
        memorySummaryState(const <String, Object?>{'summary': 'x'}),
        isNull,
        reason: '不是对象就不是摘要状态，面板整块不渲染',
      );
      final Map<String, Object?>? s = memorySummaryState(
        const <String, Object?>{
          'summary': <String, Object?>{'version': 2, 'text': '  用户喜欢薄荷。  '},
        },
      );
      expect(s, isNotNull);
      expect(memorySummaryText(s!), '用户喜欢薄荷。');
      expect(
        memorySummaryText(const <String, Object?>{'text': '   '}),
        isNull,
        reason: '空摘要按「没有」处理（失败 = 无摘要）',
      );
      // 2026-10-08：版本号 / 状态行 / 口径说明随「回滚 + 版本」那一块一起删除
      //（面板只画正文）。这三条断言随之删除——留着一个没人渲染的格式化函数
      // 只会让人以为那块 UI 还在。
    });
    test('memoryCommandArgs：没有会话就不带 session_id，有就 trim 后带上', () {
      expect(memoryCommandArgs().containsKey('session_id'), isFalse);
      expect(
        memoryCommandArgs(sessionId: '   ').containsKey('session_id'),
        isFalse,
        reason: '空白 id 等于没有会话',
      );
      final Map<String, Object?> scoped =
          memoryCommandArgs(sessionId: ' s-1 ', extra: <String, Object?>{'limit': 50});
      expect(scoped['session_id'], 's-1');
      expect(scoped['limit'], 50);
    });

    test('memoryRecordText：压平空白 + 截断 + 空记录兜底', () {
      expect(memoryRecordText('第一行\n第二行'), '第一行 第二行');
      expect(memoryRecordText('   '), '（空记录）');
      expect(memoryRecordText(null), '（空记录）');
      final String long = List<String>.filled(200, '字').join();
      final String shown = memoryRecordText(long, maxChars: 10);
      expect(shown.length, 11, reason: '10 个字符 + 一个省略号');
      expect(shown.endsWith('…'), isTrue);
      expect(memoryRecordText('短句'), '短句');
    });

    test('memoryParseRecords：未知形状 → 空列表，不崩', () {
      expect(memoryParseRecords(null), isEmpty);
      expect(memoryParseRecords('不是数组'), isEmpty);
      expect(memoryParseRecords(<Object?>[1, 'x', null]), isEmpty);
      final List<Map<String, Object?>> parsed =
          memoryParseRecords(<Object?>[<Object?, Object?>{'id': 'a', 'text': 'x'}]);
      expect(parsed.length, 1);
      expect(parsed.first['id'], 'a');
    });

    test('memoryCommandErrorMessage：每个码都带码且可处置', () {
      for (final String code in <String>[
        'command_unavailable',
        'unsupported_command',
        'command_failed',
        'not_found',
      ]) {
        expect(
          memoryCommandErrorMessage(ApiException(code, '原因', status: 503), '导入'),
          contains(code),
        );
      }
      expect(
        memoryCommandErrorMessage(
          const ApiException('command_unavailable', '忙', status: 503),
          'delete',
        ),
        contains('可稍后重试'),
      );
      expect(
        memoryClearErrorMessage(
          const ApiException('command_failed', '存储不可写', status: 409),
        ),
        contains('command_failed'),
      );
    });
  });

  group('面板元数据', () {
    test('modId 与 stateLabels（含 records 与会话三键）', () {
      const MemoryPanel panel = MemoryPanel();
      expect(panel.modId, 'memory');
      final Map<String, String> labels = panel.stateLabels;
      expect(labels['records'], '当前条数');
      expect(labels['hits'], '命中次数');
      expect(labels['injects'], '注入轮数');
      expect(labels['enabled_injection'], '注入开关');
      expect(labels['active_session'], '当前会话');
      expect(labels['bucket_path'], '生效记忆库');
      expect(labels['session_scoped'], '会话分桶');
    });
  });

  group('运行态渲染：只剩「记住了 N 条」', () {
    testWidgets('条数上屏；命中 / 注入 / 淘汰 / 上轮命中 一个都不在', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      expect(find.text('记忆概览'), findsOneWidget);
      expect(find.textContaining('记住了 12 条'), findsOneWidget);
      for (final String banned in <String>['累计命中', '注入', '已淘汰', '上轮命中', '记忆条数']) {
        expect(
          find.textContaining(banned),
          findsNothing,
          reason: '产品面上不得出现「$banned」',
        );
      }
    });

    testWidgets('读取中 / 读取失败都如实说，失败带错误码', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(stateLoading: true))),
      );
      await tester.pumpAndSettle();
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
      await tester.pumpAndSettle();
      expect(find.textContaining('state_unavailable'), findsOneWidget);
    });
  });

  group('记忆列表：进入面板即 list + 两行渲染 + 无会话降级', () {
    testWidgets('list 返回两条 → 两行都渲染，args 带 session_id 与 limit', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              activeSessionId: 'session-a',
              onCommand: recordingHandler(
                calls,
                records: const <Map<String, Object?>>[
                  <String, Object?>{'id': 'id-1', 'text': '第一条记忆'},
                  <String, Object?>{'id': 'id-2', 'text': '第二条记忆'},
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('记忆列表'), findsOneWidget);
      expect(find.text('第一条记忆'), findsOneWidget);
      expect(find.text('第二条记忆'), findsOneWidget);
      expect(find.textContaining('共 2 条'), findsOneWidget);
      expect(find.textContaining('生效记忆库：'), findsOneWidget);

      expect(calls, isNotEmpty);
      expect(calls.first.command, 'list');
      expect(calls.first.args['limit'], 50);
      expect(calls.first.args['session_id'], 'session-a');
    });

    testWidgets('activeSessionId = null：不传 session_id + 全局桶降级文案', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(activeSessionId: null, onCommand: recordingHandler(calls)),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(calls.first.command, 'list');
      expect(
        calls.first.args.containsKey('session_id'),
        isFalse,
        reason: '没有会话就不该带 session_id（落到全局桶）',
      );
      expect(find.textContaining('还没有对话'), findsOneWidget);
      expect(find.textContaining('先放到一起'), findsOneWidget);
      expect(find.textContaining('按这次对话分开'), findsOneWidget);
      expect(find.textContaining('还没有记住任何内容'), findsOneWidget);
      expect(find.textContaining('桶'), findsNothing, reason: '产品面上不出现「桶」');
    });

    testWidgets('list 失败：显示带码文案，不谎报有记录', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              activeSessionId: 'session-a',
              onCommand:
                  (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
                if (command == 'list') {
                  throw const ApiException(
                    'command_unavailable',
                    '忙',
                    status: 503,
                  );
                }
                return const ModCommandResult(ok: true);
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('command_unavailable'), findsOneWidget);
      expect(find.textContaining('读取记忆列表失败'), findsOneWidget);
    });
  });

  group('导入一条：onCommand(import) + args + 成功提示', () {
    testWidgets('输入文本 → 点导入 → import args 正确 + 通知已变更', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      final List<String> notified = <String>[];
      int refreshes = 0;
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              activeSessionId: 'session-a',
              onModChanged: notified.add,
              onRefreshState: () async {
                refreshes += 1;
              },
              onCommand: recordingHandler(
                calls,
                records: const <Map<String, Object?>>[],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(
        find.byKey(const Key('memory-import-field')),
        ' 我喜欢薄荷 ',
      );
      await tester.tap(find.byKey(const Key('memory-import-button')));
      await tester.pumpAndSettle();

      final RecordedCall importCall =
          calls.firstWhere((RecordedCall c) => c.command == 'import');
      expect(importCall.args['text'], '我喜欢薄荷');
      expect(importCall.args['session_id'], 'session-a');
      expect(find.textContaining('已导入一条记忆'), findsOneWidget);
      expect(notified, isNotEmpty, reason: '成功后必须通知宿主');
      expect(notified.last, contains('已导入一条记忆'));
      expect(refreshes, greaterThanOrEqualTo(1), reason: '成功后要刷运行态');
      // 列表也被刷新了（initState 一次 + 动作后一次）。
      expect(calls.where((RecordedCall c) => c.command == 'list').length, 2);
    });

    testWidgets('空文本不发命令，给可读文案', (WidgetTester tester) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(onCommand: recordingHandler(calls)))),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('memory-import-button')));
      await tester.pumpAndSettle();

      expect(
        calls.where((RecordedCall c) => c.command == 'import'),
        isEmpty,
        reason: '空 text 不该发出去（Rust 也会拒）',
      );
      expect(find.textContaining('请先输入要记住的内容'), findsOneWidget);
    });
  });

  group('编辑 / 删除：按 id 定位 + 二次确认', () {
    testWidgets('编辑：对话框保存 → update args 带 id / text / session_id', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              activeSessionId: 'session-a',
              onCommand: recordingHandler(
                calls,
                records: const <Map<String, Object?>>[
                  <String, Object?>{'id': 'id-1', 'text': '旧正文'},
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('编辑'));
      await tester.pumpAndSettle();
      expect(find.text('编辑记忆'), findsOneWidget);

      await tester.enterText(
        find.byKey(const Key('memory-edit-field')),
        '新正文',
      );
      await tester.tap(find.text('保存'));
      await tester.pumpAndSettle();

      final RecordedCall updateCall =
          calls.firstWhere((RecordedCall c) => c.command == 'update');
      expect(updateCall.args['id'], 'id-1');
      expect(updateCall.args['text'], '新正文');
      expect(updateCall.args['session_id'], 'session-a');
      expect(find.textContaining('已更新这条记忆'), findsOneWidget);
    });

    testWidgets('删除：先确认；取消不发命令，确认才发 delete', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              activeSessionId: 'session-a',
              onCommand: recordingHandler(
                calls,
                records: const <Map<String, Object?>>[
                  <String, Object?>{'id': 'id-9', 'text': '要被删掉的'},
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // 第一次：打开对话框 → 取消 → 不发命令。
      await tester.tap(find.byTooltip('删除'));
      await tester.pumpAndSettle();
      expect(find.text('删除这条记忆？'), findsOneWidget);
      expect(find.textContaining('删除后不可恢复'), findsOneWidget);
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(
        calls.where((RecordedCall c) => c.command == 'delete'),
        isEmpty,
        reason: '取消不得误删',
      );

      // 第二次：确认 → delete args 带 id。
      await tester.tap(find.byTooltip('删除'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('确认删除'));
      await tester.pumpAndSettle();

      final RecordedCall deleteCall =
          calls.firstWhere((RecordedCall c) => c.command == 'delete');
      expect(deleteCall.args['id'], 'id-9');
      expect(deleteCall.args['session_id'], 'session-a');
      expect(find.textContaining('已删除这条记忆'), findsOneWidget);
    });
  });

  group('清空记忆库：触发 onCommand(clear) + 带码文案', () {
    testWidgets('点击清空 → onCommand(clear)，成功文案含清掉/现存条数', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              activeSessionId: 'session-a',
              onCommand:
                  (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
                calls.add(RecordedCall(command, args));
                if (command == 'list') return listReply();
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
      await tester.pumpAndSettle();

      await tester.tap(find.text('清空记忆库'));
      await tester.pumpAndSettle();

      final RecordedCall clearCall =
          calls.firstWhere((RecordedCall c) => c.command == 'clear');
      expect(clearCall.args['session_id'], 'session-a');
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
                  (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
                if (command == 'list') return listReply();
                return const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'records': 0,
                    'cleared': true,
                    'removed': 2,
                    'residue': true,
                  },
                );
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
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
                  (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
                if (command == 'list') return listReply();
                throw const ApiException('command_failed', '存储不可写', status: 409);
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('清空记忆库'));
      await tester.pumpAndSettle();
      expect(find.textContaining('command_failed'), findsOneWidget);
      expect(find.textContaining('已清空记忆库'), findsNothing);
    });

    testWidgets('Mod 未启用：清空按钮禁用 + 说明', (WidgetTester tester) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: memoryState(),
              enabled: false,
              onCommand: recordingHandler(calls),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.widgetWithText(OutlinedButton, '清空记忆库'),
      );
      expect(button.onPressed, isNull, reason: '未启用就不该假装能清空');
      expect(find.textContaining('Mod 未启用'), findsOneWidget);
      // 未启用时列表也没有记录、导入按钮禁用。
      expect(find.textContaining('还没有记住任何内容'), findsOneWidget);
    });
  });

  group('压缩过的记忆：只有正文时才画（P1-5 的瘦身版）', () {
    testWidgets('有正文：标题 + 正文上屏；版本号 / 回滚按钮 / 「注入」都不在', (
      WidgetTester tester,
    ) async {
      final List<RecordedCall> calls = <RecordedCall>[];
      final Map<String, Object?> state = <String, Object?>{
        ...memoryState(),
        'summary': <String, Object?>{
          'enabled': true,
          'client': 'openai',
          'version': 2,
          'covers_upto': 9,
          'bucket_ratio': 0.78,
          'pending': false,
          'last_error': null,
          'kept_recent': 4,
          'text': '用户喜欢薄荷，且自称星梦。',
        },
      };
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: state,
              activeSessionId: 'session-a',
              onCommand: recordingHandler(calls),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('压缩过的记忆'), findsOneWidget);
      expect(find.textContaining('用户喜欢薄荷'), findsWidgets);
      // 版本号 / 覆盖位点 / 桶占比 / 回滚按钮整块不再上屏。
      expect(find.textContaining('v2'), findsNothing);
      expect(find.textContaining('90%'), findsNothing);
      expect(find.byKey(const Key('memory-summary-rollback-button')), findsNothing);
      expect(find.textContaining('回滚'), findsNothing);
      expect(
        calls.where((RecordedCall c) => c.command == 'summary_rollback'),
        isEmpty,
      );
    });

    testWidgets('有摘要对象但没有正文：整块不画（空摘要 = 没有）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: <String, Object?>{
                ...memoryState(),
                'summary': <String, Object?>{
                  'enabled': true,
                  'version': 3,
                  'covers_upto': 12,
                  'text': '   ',
                },
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('压缩过的记忆'), findsNothing);
    });

    testWidgets('state 里没有 summary 键：整块不渲染（旧服务端兼容）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      expect(find.text('压缩过的记忆'), findsNothing);
      expect(find.text('记忆摘要'), findsNothing);
      expect(find.byKey(const Key('memory-summary-rollback-button')), findsNothing);
    });
  });

  group('说明文案（2026-10-08）：不再有协议说明', () {
    testWidgets('面板上没有「注入开关 / Mod 启停是两件事 / 来源槽叠加」这类长文', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      for (final String banned in <String>[
        '注入开关',
        'Mod 启停是两件事',
        '来源槽叠加',
        kMemoryDocPath,
        '只对下一轮生效',
      ]) {
        expect(
          find.textContaining(banned),
          findsNothing,
          reason: '产品面上不得出现「$banned」',
        );
      }
    });

    testWidgets('hiddenKeys 只留 store_path；其余 10 个进 devKeys（都不进「高级」）', (
      WidgetTester tester,
    ) async {
      const MemoryPanel panel = MemoryPanel();
      expect(panel.hiddenKeys, <String>{'store_path'});
      expect(panel.devKeys, <String>{
        'top_k',
        'max_records',
        'summary_enabled',
        'summary_base_url',
        'summary_model',
        'summary_api_key_env',
        'summary_timeout_ms',
        'summary_ratio',
        'summary_cooldown_turns',
        'summary_keep_recent_turns',
      });
      expect(panel.devKeys.length, 10);
      expect(panel.advancedKeys, isEmpty);
      // 2026-10-09：fieldHelp 已退役（三级功能介绍全删）。
      expect(panel.fieldHelp, isEmpty);
    });

    testWidgets('真泵卡片（产品面）：只留「聊天时用上这些记忆」一个键', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[memoryMod()],
                loading: false,
                onLoadState: (String id) async => ModStateResult(
                  id: id,
                  enabled: true,
                  state: const <String, Object?>{},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('本地记忆'));
      await tester.pumpAndSettle();

      expect(find.text('聊天时用上这些记忆'), findsOneWidget);
      expect(
        find.text('关掉后仍会记住，但不会塞进角色的提示词。'),
        findsNothing,
        reason: '旧 fieldHelp 说明不得再上屏（2026-10-09 全删）',
      );
      for (final ModSettingField f in memorySpec().fields) {
        if (f.key == 'enabled_injection') continue;
        expect(find.text(f.label), findsNothing, reason: '${f.key} 不得上屏');
      }
      expect(find.text('高级'), findsNothing);
      // 两个 Switch：卡片标题行的启用开关 + 「聊天时用上这些记忆」。
      expect(find.byType(Switch), findsNWidgets(2));
    });

    testWidgets('开发模式：store_path 仍不画，其余 10 个 devKeys 全部上屏', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[memoryMod()],
                loading: false,
                devMode: true,
                onLoadState: (String id) async => ModStateResult(
                  id: id,
                  enabled: true,
                  state: const <String, Object?>{},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('本地记忆'));
      await tester.pumpAndSettle();

      // spec 里的 top_k / max_records 在开发模式里画出来。
      expect(find.text('每轮注入条数'), findsOneWidget);
      expect(find.text('条数上限'), findsOneWidget);
      // store_path 仍是 hiddenKeys：开发模式也不画。
      expect(find.text('记忆库路径'), findsNothing);
      expect(find.text('高级'), findsNothing);
    });
  });

  group('刷新锁（2026-10-08）：list 失败后「刷新」仍可点', () {
    testWidgets('读取失败：留下错误、刷新按钮可点、写操作仍锁住', (
      WidgetTester tester,
    ) async {
      final List<String> commands = <String>[];
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              activeSessionId: 'session-a',
              onCommand:
                  (String command, [Map<String, Object?> args = const <String, Object?>{}]) async {
                commands.add(command);
                if (command == 'list') {
                  throw const ApiException('command_unavailable', '忙', status: 503);
                }
                return const ModCommandResult(ok: true);
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('读取记忆列表失败'), findsOneWidget);

      final TextButton refresh = tester.widget<TextButton>(
        find.byKey(const Key('memory-refresh-button')),
      );
      expect(
        refresh.onPressed,
        isNotNull,
        reason: '列表读失败后没有出路 —— 刷新必须仍然可点',
      );
      // 写操作仍要求「快照 == 当前会话」：这一次失败没有改快照，所以行不存在，
      // 但导入按钮必须看得到且不被取数窗口额外锁死。
      expect(find.byKey(const Key('memory-import-button')), findsOneWidget);

      final int before = commands.where((String c) => c == 'list').length;
      await tester.tap(find.byKey(const Key('memory-refresh-button')));
      await tester.pumpAndSettle();
      expect(
        commands.where((String c) => c == 'list').length,
        greaterThan(before),
        reason: '点了刷新却没再发 list',
      );
    });
  });
}

