import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_dispatch.dart';
import 'package:live2d_ai_shell/actions/action_event.dart';
import 'package:live2d_ai_shell/api/action_source.dart';
import 'package:live2d_ai_shell/api/commands_api.dart';
import 'package:live2d_ai_shell/ui/action_grid.dart';
import 'package:live2d_ai_shell/ui/action_toolbar.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 真实抓包形状的动作目录（`GET /api/v1/commands`，6 条）。
List<CommandInfo> realCommands() => <CommandInfo>[
  _cmd('live2d_perform_action_nod', '点头', 'nod'),
  _cmd('live2d_perform_action_shake_no', '摇头', 'shake_no'),
  _cmd('live2d_perform_action_tilt', '歪头', 'tilt'),
  _cmd('live2d_perform_action_look_around', '环视', 'look_around'),
  _cmd('live2d_perform_action_listen', '倾听', 'listen'),
  _cmd('live2d_perform_action_surprise', '惊讶', 'surprise'),
];

CommandInfo _cmd(String id, String name, String action) => CommandInfo(
  id: id,
  name: name,
  description: '表演$name动作',
  targetAction: action,
  kind: 'single',
  allowedSources: const <String>['user_command'],
  strengthDefault: 2,
  strengthMin: 1,
  strengthMax: 3,
  stepCount: 1,
);

void main() {
  group('ActionGrid：动作按钮由 capabilities 驱动，不硬编码', () {
    testWidgets('渲染 6 个动作卡片（名字来自服务端目录）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: realCommands(),
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      for (final String name in <String>['点头', '摇头', '歪头', '环视', '倾听', '惊讶']) {
        expect(find.text(name), findsOneWidget, reason: name);
      }
    });

    testWidgets('空目录 → 如实说明（不是空白面板）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: const <CommandInfo>[],
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      expect(find.text('当前没有可手动触发的动作'), findsOneWidget);
      expect(find.textContaining('capabilities'), findsOneWidget);
    });

    testWidgets('强度用**分段按钮**（三个离散档），不是滑杆', (WidgetTester tester) async {
      final List<int> picked = <int>[];
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: realCommands(),
            strength: 2,
            onStrengthChanged: picked.add,
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      expect(find.byType(Slider), findsNothing, reason: '1/2/3 是离散档位，滑杆会暗示 2.5 有意义');
      await tester.tap(find.text('强'));
      expect(picked, <int>[3]);
    });

    testWidgets('点卡片触发对应指令', (WidgetTester tester) async {
      final List<String> invoked = <String>[];
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: realCommands(),
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (CommandInfo c) => invoked.add(c.id),
            dispatch: ActionDispatch(),
          ),
        ),
      );
      await tester.tap(find.text('歪头'));
      expect(invoked, <String>['live2d_perform_action_tilt']);
    });

    testWidgets('正在演的动作卡片：显示「正在演」+ 进度环', (WidgetTester tester) async {
      // `useLocalExpiry: false`：这条测的是**渲染**，不是兜底过期。
      // 开着的话 widget 测试会报「widget 树都销毁了还有 pending timer」
      // （本地兜底定时器本来就还没到期——这正是它的设计）。
      final ActionDispatch dispatch = ActionDispatch(useLocalExpiry: false);
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: realCommands(),
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (_) {},
            dispatch: dispatch,
          ),
        ),
      );
      expect(find.text('正在演'), findsNothing);

      dispatch.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 0,
          action: 'nod',
          strength: 2,
          source: ActionSource.userCommand,
        ),
      );
      await tester.pump();
      expect(find.text('正在演'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('读屏念「正在演」而不只是按钮名', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      final ActionDispatch dispatch = ActionDispatch(useLocalExpiry: false);
      dispatch.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 0,
          action: 'tilt',
          source: ActionSource.userCommand,
        ),
      );
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: realCommands(),
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (_) {},
            dispatch: dispatch,
          ),
        ),
      );
      expect(find.bySemanticsLabel('歪头，正在演'), findsOneWidget);
      handle.dispose();
    });

    testWidgets('超过 8 槽的动作**不静默丢弃**：如实说明有几个没显示', (
      WidgetTester tester,
    ) async {
      final List<CommandInfo> many = <CommandInfo>[
        for (int i = 0; i < 10; i++) _cmd('id$i', '动作$i', 'a$i'),
      ];
      await tester.pumpWidget(
        wrap(
          ActionGrid(
            commands: many,
            strength: 2,
            onStrengthChanged: (_) {},
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      expect(find.textContaining('另有 2 个动作未显示'), findsOneWidget);
      expect(find.textContaining('槽位上限 8'), findsOneWidget);
    });

    test('屏幕槽位上限是 8（VRChat 与 VTS 双源收敛的值）', () {
      expect(kActionSlotLimit, 8);
    });

    test('高频动作是三个社交动作（应答 + 情绪）', () {
      expect(kHighFrequencyActions, <String>['nod', 'shake_no', 'surprise']);
    });
  });

  group('ActionToolbar：不打开设置就能点', () {
    testWidgets('只放 maxSlots 个，且与网格共享忙碌态', (WidgetTester tester) async {
      final ActionDispatch dispatch = ActionDispatch(useLocalExpiry: false);
      await tester.pumpWidget(
        wrap(
          ActionToolbar(
            commands: realCommands(),
            onInvoke: (_) {},
            dispatch: dispatch,
            maxSlots: 3,
          ),
        ),
      );
      // 组件**不排序**（排序是调用方的业务决定）；这里按传入顺序取前 3 个。
      expect(find.text('点头'), findsOneWidget);
      expect(find.text('摇头'), findsOneWidget);
      expect(find.text('歪头'), findsOneWidget);
      expect(find.text('环视'), findsNothing, reason: 'maxSlots=3');

      dispatch.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 0,
          action: 'nod',
          source: ActionSource.userCommand,
        ),
      );
      await tester.pump();
      // 工具条里那一个也进忙碌态（同一份 dispatch，两处不会不一致）。
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('空目录 → 整个工具条不占位', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionToolbar(
            commands: const <CommandInfo>[],
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      expect(find.byType(TextButton), findsNothing);
    });

    testWidgets('tooltip 用服务端描述（不重新编文案）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionToolbar(
            commands: realCommands().take(1).toList(),
            onInvoke: (_) {},
            dispatch: ActionDispatch(),
          ),
        ),
      );
      expect(find.byTooltip('表演点头动作'), findsOneWidget);
    });
  });

  group('舞台角标三键：只发方向，真值以 ack 为准', () {
    testWidgets('三个按钮各自上报 in/out/reset', (WidgetTester tester) async {
      final List<String> dirs = <String>[];
      await tester.pumpWidget(
        wrap(StageCornerControls(onZoom: dirs.add, scaleFromAck: 1.0)),
      );
      await tester.tap(find.byIcon(Icons.add));
      await tester.tap(find.byIcon(Icons.remove));
      await tester.tap(find.byIcon(Icons.center_focus_strong_outlined));
      expect(dirs, <String>['in', 'out', 'reset']);
    });

    testWidgets('**没收到 ack 时不显示百分比**（不猜数值）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(StageCornerControls(onZoom: (_) {}, scaleFromAck: null)),
      );
      expect(find.text('—'), findsOneWidget);
      expect(find.textContaining('%'), findsNothing);
    });

    testWidgets('收到 ack 后显示渲染面回报的实际缩放', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(StageCornerControls(onZoom: (_) {}, scaleFromAck: 1.21)),
      );
      expect(find.text('121%'), findsOneWidget);
    });

    testWidgets('复位按钮的提示说明它同时归零位置', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(StageCornerControls(onZoom: (_) {}, scaleFromAck: 1.0)),
      );
      expect(find.byTooltip('复位缩放与位置'), findsOneWidget);
    });
  });
}
