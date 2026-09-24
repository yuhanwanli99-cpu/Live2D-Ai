/// `memory` 产品面板（产品级加强波次 + L1 主动管理面）的 Flutter 回归。
///
/// 覆盖：概览渲染、「记忆列表」两条上屏、导入/编辑/删除的 onCommand args、
/// 删除二次确认、`activeSessionId == null` 的全局桶降级文案、清空、
/// 带码失败文案、会话说明与「同轮生效」提示。
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
      label: '把检索结果注入本轮提示词',
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

    test('memoryBucketNotice：null 说清全局桶降级；有会话说清只影响该会话', () {
      final String fallback = memoryBucketNotice(null);
      expect(fallback, contains('还没有会话'));
      expect(fallback, contains('全局桶'));
      expect(fallback, contains('与所有会话共享'));
      expect(fallback, contains('发一条消息'));
      final String scoped = memoryBucketNotice('  s-1  ');
      expect(scoped, contains('当前会话桶：s-1'));
      expect(scoped, contains('只影响这个会话'));
      expect(fallback, isNot(contains('当前会话桶')));
    });


    test('memorySummaryState / version / text：非对象与空正文都按「没有」处理', () {
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
      expect(memorySummaryVersion(s!), 2);
      expect(memorySummaryText(s), '用户喜欢薄荷。');
      expect(
        memorySummaryText(const <String, Object?>{'text': '   '}),
        isNull,
        reason: '空摘要按「没有」处理（失败 = 无摘要）',
      );
      expect(memorySummaryVersion(const <String, Object?>{}), 0, reason: '缺键 = 0 版');
    });

    test('memorySummaryStatusLine：未启用 / 无摘要 / 有摘要 / 失败原因四种口径', () {
      final String off = memorySummaryStatusLine(
        const <String, Object?>{'enabled': false},
      );
      expect(off, contains('未启用'));
      expect(off, contains('summary_base_url'));
      final String none = memorySummaryStatusLine(
        const <String, Object?>{
          'enabled': true,
          'version': 0,
          'bucket_ratio': 0.82,
          'pending': true,
        },
      );
      expect(none, contains('还没有摘要'));
      expect(none, contains('82%'));
      expect(none, contains('正在后台生成'));
      final String some = memorySummaryStatusLine(
        const <String, Object?>{
          'enabled': true,
          'version': 3,
          'covers_upto': 12,
          'bucket_ratio': 0.9,
          'last_error': '摘要请求失败（client=openai）',
        },
      );
      expect(some, contains('v3'));
      expect(some, contains('12'));
      expect(some, contains('90%'));
      expect(some, contains('上次失败'), reason: '失败必须可见，不能静默');
    });

    test('memorySummaryHint：说清保留最近几轮 + 回滚不删原文', () {
      final String hint = memorySummaryHint(
        const <String, Object?>{'kept_recent': 4, 'note': '未配 key'},
      );
      expect(hint, contains('最近 4 轮'));
      expect(hint, contains('回滚'));
      expect(hint, contains('原文一条没删'));
      expect(hint, contains('未配 key'));
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

  group('运行态渲染：条数 / 命中 / 注入 / 淘汰 / 上轮命中', () {
    testWidgets('五项计数都上屏（文字，不靠颜色）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
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
      expect(find.textContaining('还没有会话'), findsOneWidget);
      expect(find.textContaining('全局桶'), findsOneWidget);
      expect(find.textContaining('与所有会话共享'), findsOneWidget);
      expect(find.textContaining('这个桶里还没有记忆'), findsOneWidget);
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
      expect(find.textContaining('这个桶里还没有记忆'), findsOneWidget);
    });
  });


  group('真摘要：状态 + 回滚按钮（P1-5）', () {
    testWidgets('有摘要时显示版本/正文，回滚按钮可点并走 summary_rollback', (
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
              onCommand: recordingHandler(
                calls,
                reply: const ModCommandResult(
                  ok: true,
                  result: <String, Object?>{
                    'version': 1,
                    'rolled_back': <String, Object?>{
                      'version': 2,
                      'covers_upto': 9,
                      'text': '用户喜欢薄荷，且自称星梦。',
                    },
                  },
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('v2'), findsWidgets);
      expect(find.textContaining('用户喜欢薄荷'), findsWidgets);
      final TextButton rollback = tester.widget<TextButton>(
        find.byKey(const Key('memory-summary-rollback-button')),
      );
      expect(rollback.onPressed, isNotNull, reason: '有版本才能回滚');
      await tester.tap(find.byKey(const Key('memory-summary-rollback-button')));
      await tester.pumpAndSettle();
      expect(
        calls.where((RecordedCall c) => c.command == 'summary_rollback').length,
        1,
      );
      expect(calls[1].args['session_id'], 'session-a');
      expect(find.textContaining('已回滚'), findsWidgets);
    });

    testWidgets('没有摘要时回滚按钮禁用；未启用整块仍可见（如实说）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          panelWidget(
            panelContext(
              state: <String, Object?>{
                ...memoryState(),
                'summary': <String, Object?>{
                  'enabled': false,
                  'version': 0,
                  'pending': false,
                },
              },
              onCommand: recordingHandler(<RecordedCall>[]),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('未启用'), findsWidgets);
      final TextButton rollback = tester.widget<TextButton>(
        find.byKey(const Key('memory-summary-rollback-button')),
      );
      expect(rollback.onPressed, isNull, reason: '没有版本就不该假装能回滚');
    });

    testWidgets('state 里没有 summary 键：整块不渲染（旧服务端兼容）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      expect(find.text('记忆摘要'), findsNothing);
      expect(find.byKey(const Key('memory-summary-rollback-button')), findsNothing);
    });
  });
  group('说明文案（瘦身版）：一句话说清边界，不塞契约全文', () {
    testWidgets('注入开关说明它与 Mod 启停是两件事', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('注入开关'), findsOneWidget);
      expect(find.textContaining('Mod 启停是两件事'), findsOneWidget);
    });

    testWidgets('会话下按来源槽叠加，且不再复述文档 / 长契约', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(panelWidget(panelContext(state: memoryState()))),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('来源槽叠加'), findsOneWidget);
      // 长文已移出面板：文档路径与逐条验收步骤不再上屏。
      expect(find.textContaining(kMemoryDocPath), findsNothing);
      expect(find.textContaining('第 12 / 13 节'), findsNothing);
      expect(find.textContaining('只对下一轮生效'), findsNothing);
    });
  });
}
