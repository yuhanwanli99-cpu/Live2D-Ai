/// 记忆面板的**会话切换**回归（2026-11-18：会话分桶已停用）。
///
/// # 历史（2026-10-01，F-0004-1）
///
/// 旧契约是「切会话桶 → 列表必须跟着换」，防止行内操作拿 A 的 id 去 B 里删。
///
/// # 现在（本文件守的契约）
///
/// 服务端说话路径不再读 memory 的会话注入槽（`session_scope.rs::speak_prompt_for`
/// 剔除 persona / memory），2026-11-16 起模型这一轮的记忆来自主链热槽；树内
/// memory Mod 也默认不注册。面板因此只剩**全局**一条路径：
/// - `list` / `import` / `update` / `delete` / `clear` 都不带 `session_id`；
/// - 宿主把 `activeSessionId` 从 A 换成 B 时**不重新取数**、列表不变、行仍可点
///   （不存在跨桶写，因为根本没有桶）；
/// - 面板上不再出现「只看这次对话」这类按会话承诺（见 kMemoryScopeNotice）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/mods/memory_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

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
  config: const <String, Object?>{},
  settingsSpec: _memorySpec(),
);

class _RecordedCall {
  _RecordedCall(this.command, this.args);

  final String command;
  final Map<String, Object?> args;
}

/// 记录调用；`list` 固定回两条记录（与「当前会话」无关）。
class _MemoryFake {
  final List<_RecordedCall> calls = <_RecordedCall>[];

  Future<ModCommandResult> call(
    String command, [
    Map<String, Object?> args = const <String, Object?>{},
  ]) async {
    calls.add(_RecordedCall(command, args));
    if (command != 'list') {
      return const ModCommandResult(ok: true, result: <String, Object?>{});
    }
    return const ModCommandResult(
      ok: true,
      result: <String, Object?>{
        'ok': true,
        'records': <Object?>[
          <String, Object?>{'id': 'id-1', 'text': '第一条记忆'},
          <String, Object?>{'id': 'id-2', 'text': '第二条记忆'},
        ],
        'total': 2,
        'bucket': 'memory.jsonl',
      },
    );
  }

  List<_RecordedCall> get listCalls =>
      calls.where((_RecordedCall c) => c.command == 'list').toList();
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

/// 同一棵 `MaterialApp` / 同一位置换 `ctx`——与宿主（每次 build 新建 ctx）同形。
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

void main() {
  testWidgets('切会话：不重新取数、列表不变、写操作不带 session_id', (WidgetTester tester) async {
    final _MemoryFake fake = _MemoryFake();
    final ValueNotifier<ModPanelContext> notifier =
        ValueNotifier<ModPanelContext>(_ctx(fake, 'session-a'));

    await tester.pumpWidget(_host(notifier));
    await tester.pumpAndSettle();
    expect(find.text('第一条记忆'), findsOneWidget);
    expect(find.text('第二条记忆'), findsOneWidget);
    expect(find.text(kMemoryScopeNotice), findsOneWidget);
    expect(fake.listCalls.length, 1, reason: '进入面板取一次数');
    expect(
      fake.listCalls.single.args.containsKey('session_id'),
      isFalse,
      reason: '列表是全局的（会话绑定已停用）',
    );

    // 宿主切会话：`activeSessionId` 变了，但面板不该跟着换列表 / 重新取数。
    notifier.value = _ctx(fake, 'session-b');
    await tester.pumpAndSettle();
    expect(fake.listCalls.length, 1, reason: '切会话不再重新取数——记忆面板没有「会话桶」可切');
    expect(find.text('第一条记忆'), findsOneWidget);
    expect(find.text('第二条记忆'), findsOneWidget);
    expect(find.text(kMemoryScopeNotice), findsOneWidget);

    // 行内删除：id 来自列表，但 args 不得再看「当前会话」（没有桶可写错）。
    await tester.tap(find.byKey(const Key('memory-delete-id-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('确认删除'));
    await tester.pumpAndSettle();
    final _RecordedCall delete = fake.calls.firstWhere(
      (_RecordedCall c) => c.command == 'delete',
    );
    expect(delete.args['id'], 'id-1');
    expect(delete.args.containsKey('session_id'), isFalse);
  });
}
