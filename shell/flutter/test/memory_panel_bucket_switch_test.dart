/// 记忆面板**跟着「会话桶」切换**（2026-10-01，F-0004-1）。
///
/// # 现场
///
/// 面板的桶说明行是 build 里**现读** `ctx.activeSessionId` 的（一帧就改口），
/// 而列表是 `initState` 里拉一次的异步结果——`_MemoryPanelBodyState` 从前
/// 没有 `didUpdateWidget`。切会话后于是：**文案说桶 B、列表还是桶 A**，
/// 而行内「删除/编辑」拿的是列表里那条记录的 `id`、`session_id` 却现读
/// `ctx`（= B）⇒ 一次**跨桶写**：用户在 B 的界面上删掉了 A 的记忆。
///
/// # 现在的契约（本文件守的就是它）
///
/// 1. 桶换了 → 立刻为新桶取一次数（`list` 的 `session_id` 必须是新桶）；
///    取数窗口里**旧桶的行先消失**，按钮先失效（旧 id 点不到）。
/// 2. 行为的 `session_id` 一律取「列表所属桶」的快照，不是 `ctx`。
/// 3. 迟到的旧桶响应**丢掉**（否则刚切过去的列表会被旧桶内容重新填满）。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/memory_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

const String kBucketA = 'session-a';
const String kBucketB = 'session-b';

ModSettingsSpec _memorySpec() => ModSettingsSpec(
  modId: 'memory',
  title: '本地记忆',
  version: 1,
  fields: <ModSettingField>[
    const ModSettingField(
      kind: ModFieldKind.string,
      key: 'store_path',
      label: '记忆库路径',
    ),
  ],
);

ModInfo _memoryMod() => ModInfo(
  id: 'memory',
  name: '本地记忆',
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config: const <String, Object?>{'store_path': '/tmp/memory.jsonl'},
  settingsSpec: _memorySpec(),
);

Map<String, Object?> _record(String id, String text) =>
    <String, Object?>{'id': id, 'text': text};

ModCommandResult _listReply({
  required String session,
  required List<Map<String, Object?>> records,
}) => ModCommandResult(
  ok: true,
  result: <String, Object?>{
    'ok': true,
    'records': records,
    'total': records.length,
    'bucket': 'sessions/$session.memory.jsonl',
  },
);

class RecordedCall {
  RecordedCall(this.command, this.args);

  final String command;
  final Map<String, Object?> args;
}

/// 记录调用 + 可手动放行的 `list`（用来制造「取数窗口」与「迟到响应」）。
class _MemoryFake {
  final List<RecordedCall> calls = <RecordedCall>[];
  final Map<String, List<Map<String, Object?>>> records =
      <String, List<Map<String, Object?>>>{};

  /// `manualLists = true` 时，`list` 的 Future 由测试手动 complete。
  bool manualLists = false;
  final List<Completer<ModCommandResult>> pending = <Completer<ModCommandResult>>[];
  final List<String?> pendingSessions = <String?>[];

  Future<ModCommandResult> call(
    String command, [
    Map<String, Object?> args = const <String, Object?>{},
  ]) {
    calls.add(RecordedCall(command, args));
    if (command != 'list') {
      return Future<ModCommandResult>.value(
        const ModCommandResult(ok: true, result: <String, Object?>{'records': 0}),
      );
    }
    final String? session = args['session_id'] as String?;
    if (manualLists) {
      final Completer<ModCommandResult> completer =
          Completer<ModCommandResult>();
      pending.add(completer);
      pendingSessions.add(session);
      return completer.future;
    }
    return Future<ModCommandResult>.value(
      _listReply(session: session ?? 'global', records: records[session] ?? const <Map<String, Object?>>[]),
    );
  }

  void releaseList(int index) {
    final String? session = pendingSessions[index];
    pending[index].complete(
      _listReply(
        session: session ?? 'global',
        records: records[session] ?? const <Map<String, Object?>>[],
      ),
    );
  }

  List<String?> get listSessions => calls
      .where((RecordedCall c) => c.command == 'list')
      .map((RecordedCall c) => c.args['session_id'] as String?)
      .toList();

  List<RecordedCall> get writes =>
      calls.where((RecordedCall c) => c.command != 'list').toList();
}

ModPanelContext _ctx(_MemoryFake fake, String? session) => ModPanelContext(
  mod: _memoryMod(),
  state: const <String, Object?>{},
  stateLoading: false,
  stateError: null,
  activeSessionId: session,
  onModChanged: (_) {},
  onRefreshState: () async {},
  onCommand: fake.call,
);

/// 同一棵 `MaterialApp`/同一位置换 `ctx` —— 与宿主（`main.dart` 现读
/// `_chat.sessions.activeId` 每次 build 新建 ctx）的行为同形。
Widget _host(ValueNotifier<ModPanelContext> notifier) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: ValueListenableBuilder<ModPanelContext>(
        valueListenable: notifier,
        builder: (BuildContext context, ModPanelContext ctx, Widget? _) =>
            MemoryPanel().build(context, ctx)!,
      ),
    ),
  ),
);

Future<void> _switchBucket(
  WidgetTester tester,
  ValueNotifier<ModPanelContext> notifier,
  _MemoryFake fake,
  String session,
) async {
  notifier.value = _ctx(fake, session);
  await tester.pumpAndSettle();
}

void main() {
  group('F-0004-1：切会话桶 → 列表跟着换（不是只有文案改口）', () {
    testWidgets('旧桶条目从列表消失、新桶条目上屏，且 list 的 session_id 是新桶', (
      WidgetTester tester,
    ) async {
      final _MemoryFake fake = _MemoryFake();
      fake.records[kBucketA] = <Map<String, Object?>>[
        _record('a-1', 'A 的记忆一'),
        _record('a-2', 'A 的记忆二'),
      ];
      fake.records[kBucketB] = <Map<String, Object?>>[_record('b-1', 'B 的记忆一')];
      final ValueNotifier<ModPanelContext> notifier =
          ValueNotifier<ModPanelContext>(_ctx(fake, kBucketA));

      await tester.pumpWidget(_host(notifier));
      await tester.pumpAndSettle();
      expect(find.text('A 的记忆一'), findsOneWidget);
      // 2026-10-08：说明改成人话（不再出现「桶」/ 会话 id）。
      expect(find.text('只看这次对话记住的内容。'), findsOneWidget);

      await _switchBucket(tester, notifier, fake, kBucketB);

      expect(
        find.text('B 的记忆一'),
        findsOneWidget,
        reason: '切桶后列表没有跟着换 —— 面板停在桶 A 的内容上',
      );
      expect(
        find.text('A 的记忆一'),
        findsNothing,
        reason: '桶 A 的条目还留在桶 B 的界面上（行内操作的 id 就是它）',
      );
      expect(fake.listSessions, <String?>[kBucketA, kBucketB]);
    });

    testWidgets('切桶取数窗口：旧桶的行点不到、写操作按钮全部失效', (WidgetTester tester) async {
      final _MemoryFake fake = _MemoryFake()..manualLists = true;
      fake.records[kBucketA] = <Map<String, Object?>>[_record('a-1', 'A 的记忆一')];
      fake.records[kBucketB] = <Map<String, Object?>>[_record('b-1', 'B 的记忆一')];
      final ValueNotifier<ModPanelContext> notifier =
          ValueNotifier<ModPanelContext>(_ctx(fake, kBucketA));

      await tester.pumpWidget(_host(notifier));
      await tester.pump();
      fake.releaseList(0); // 桶 A 的列表到位
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('memory-delete-a-1')), findsOneWidget);

      // 切到 B：B 的 list 还没回来。
      notifier.value = _ctx(fake, kBucketB);
      await tester.pump();
      expect(
        find.byKey(const Key('memory-delete-a-1')),
        findsNothing,
        reason: '切桶后旧桶的行还挂在树上 —— 点下去就是拿 A 的 id 去 B 里删',
      );
      expect(find.byTooltip('删除'), findsNothing);
      expect(
        tester
            .widget<TextButton>(find.byKey(const Key('memory-import-button')))
            .onPressed,
        isNull,
        reason: '取数窗口里「导入」还可用 —— 它会把内容写进哪个桶说不清',
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.ancestor(
                of: find.text('清空记忆库'),
                matching: find.byType(OutlinedButton),
              ),
            )
            .onPressed,
        isNull,
        reason: '取数窗口里「清空」还可用 —— 可能清错桶',
      );

      // 新桶的列表到位 → 行回来，且写操作打的是新桶。
      fake.releaseList(1);
      await tester.pumpAndSettle();
      expect(find.text('B 的记忆一'), findsOneWidget);
      await tester.tap(find.byKey(const Key('memory-delete-b-1')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('确认删除'));
      await tester.pumpAndSettle();

      final RecordedCall delete = fake.writes.single;
      expect(delete.command, 'delete');
      expect(delete.args['id'], 'b-1');
      expect(
        delete.args['session_id'],
        kBucketB,
        reason: '删除打到了别的桶 —— 记录的家（快照）与 ctx 分叉了',
      );
    });

    testWidgets('迟到的旧桶响应被丢掉（不把新桶的列表重新填回旧桶）', (WidgetTester tester) async {
      final _MemoryFake fake = _MemoryFake()..manualLists = true;
      fake.records[kBucketA] = <Map<String, Object?>>[_record('a-1', 'A 的记忆一')];
      fake.records[kBucketB] = <Map<String, Object?>>[_record('b-1', 'B 的记忆一')];
      final ValueNotifier<ModPanelContext> notifier =
          ValueNotifier<ModPanelContext>(_ctx(fake, kBucketA));

      await tester.pumpWidget(_host(notifier));
      await tester.pump();
      // 桶 A 的响应还没回来就切到 B。
      notifier.value = _ctx(fake, kBucketB);
      await tester.pump();
      fake.releaseList(1); // B 先到
      await tester.pumpAndSettle();
      expect(find.text('B 的记忆一'), findsOneWidget);

      fake.releaseList(0); // A 迟到
      await tester.pumpAndSettle();
      expect(
        find.text('A 的记忆一'),
        findsNothing,
        reason: '迟到的旧桶响应把面板填回了旧桶（代际没有对齐）',
      );
      expect(find.text('B 的记忆一'), findsOneWidget);
    });
  });
}
