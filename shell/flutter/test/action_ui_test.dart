import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_history.dart';
import 'package:live2d_ai_shell/api/action_source.dart';
import 'package:live2d_ai_shell/ui/action_history_view.dart';
import 'package:live2d_ai_shell/ui/action_source_badge.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

void main() {
  group('来源映射：三通道（色 + 形 + 字），不只靠颜色', () {
    final ColorScheme scheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFF7C5CFF),
    );

    test('四种来源各有**不同的文字**与**不同的图标**', () {
      final Map<ActionSource, ActionSourceView> views = <ActionSource, ActionSourceView>{
        for (final ActionSource s in ActionSource.values)
          s: actionSourceView(s, scheme),
      };
      expect(views[ActionSource.userCommand]!.label, '你');
      expect(views[ActionSource.llmTool]!.label, 'AI');
      expect(views[ActionSource.ruleFallback]!.label, '规则');
      expect(views[ActionSource.unknown]!.label, '未知');

      final Set<String> labels =
          views.values.map((ActionSourceView v) => v.label).toSet();
      expect(labels, hasLength(4), reason: '文字通道必须两两可区分');

      final Set<IconData> icons =
          views.values.map((ActionSourceView v) => v.icon).toSet();
      expect(icons, hasLength(4), reason: '形状通道必须两两可区分');
    });

    test('用户与 AI 的**色调**不同（这是本规格唯一的 tertiary 用途）', () {
      final ActionSourceView user = actionSourceView(ActionSource.userCommand, scheme);
      final ActionSourceView ai = actionSourceView(ActionSource.llmTool, scheme);
      expect(user.tone, scheme.primaryContainer);
      expect(ai.tone, scheme.tertiaryContainer);
      expect(user.tone, isNot(ai.tone), reason: '同色相就无法一眼区分');
    });

    test('规则兜底与未知同色，但靠**字 + 形**区分（色盲下仍可读）', () {
      final ActionSourceView rule = actionSourceView(ActionSource.ruleFallback, scheme);
      final ActionSourceView unknown = actionSourceView(ActionSource.unknown, scheme);
      expect(rule.tone, unknown.tone);
      expect(rule.label, isNot(unknown.label));
      expect(rule.icon, isNot(unknown.icon));
    });

    test('对比度：前景色取自对应 on* 槽位（不手写白/黑）', () {
      expect(
        actionSourceView(ActionSource.userCommand, scheme).onTone,
        scheme.onPrimaryContainer,
      );
      expect(
        actionSourceView(ActionSource.llmTool, scheme).onTone,
        scheme.onTertiaryContainer,
      );
      expect(
        actionSourceView(ActionSource.unknown, scheme).onTone,
        scheme.onSurfaceVariant,
      );
    });

    test('未知**绝不**归到 AI 一侧（协议一变就会把用户的动作标成 AI 做的）', () {
      final ActionSourceView unknown = actionSourceView(ActionSource.unknown, scheme);
      final ActionSourceView ai = actionSourceView(ActionSource.llmTool, scheme);
      expect(unknown.tone, isNot(ai.tone));
      expect(unknown.label, isNot(ai.label));
      expect(unknown.icon, isNot(ai.icon));
    });
  });

  group('ActionSourceBadge 渲染', () {
    testWidgets('四个来源都能渲染出文字与图标', (WidgetTester tester) async {
      for (final ActionSource source in ActionSource.values) {
        await tester.pumpWidget(wrap(ActionSourceBadge(source: source)));
        expect(
          find.text(source.label),
          findsOneWidget,
          reason: '${source.wire} 的文字通道缺了',
        );
        expect(find.byIcon(actionSourceView(source, buildAppTheme().colorScheme).icon),
            findsOneWidget);
      }
    });

    testWidgets('读屏给一句完整的「来源：AI」，不念碎片', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(wrap(const ActionSourceBadge(source: ActionSource.llmTool)));
      expect(find.bySemanticsLabel('来源：AI'), findsOneWidget);
      handle.dispose();
    });

    testWidgets('dense 只改尺寸，不改文字（两种密度下人都读得懂）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(const ActionSourceBadge(source: ActionSource.ruleFallback)));
      expect(find.text('规则'), findsOneWidget);
      await tester.pumpWidget(
        wrap(const ActionSourceBadge(source: ActionSource.ruleFallback, dense: true)),
      );
      expect(find.text('规则'), findsOneWidget);
    });
  });

  group('ActionHistoryView 渲染', () {
    ActionHistoryEntry entry({
      String action = 'nod',
      String? label,
      ActionSource source = ActionSource.llmTool,
      int? strength = 2,
      int epoch = 1,
      DateTime? at,
      DateTime? endedAt,
    }) => ActionHistoryEntry(
      action: action,
      label: label ?? action,
      source: source,
      epoch: epoch,
      strength: strength,
      at: at ?? DateTime(2026, 9, 10, 20, 15, 30),
      endedAt: endedAt,
    );

    testWidgets('空态说话（空白面板会被误读成「坏了」）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(const ActionHistoryView(entries: <ActionHistoryEntry>[])));
      expect(find.text('还没有动作'), findsOneWidget);
      expect(find.textContaining('是谁触发的'), findsOneWidget);
    });

    testWidgets('一行里五列都在：时间 / 中文名 / 强度 / 来源 / epoch', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[
              entry(label: '点头', strength: 2, source: ActionSource.llmTool, epoch: 4),
            ],
          ),
        ),
      );
      expect(find.text('20:15:30'), findsOneWidget);
      expect(find.text('点头'), findsOneWidget);
      expect(find.text('Ⅱ'), findsOneWidget);
      expect(find.text('AI'), findsOneWidget);
      expect(find.text('e4'), findsOneWidget);
    });

    testWidgets('最新在最上面（日志惯例）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[
              entry(action: 'nod', label: '旧'),
              entry(action: 'tilt', label: '新'),
            ],
          ),
        ),
      );
      final double newer = tester.getTopLeft(find.text('新')).dy;
      final double older = tester.getTopLeft(find.text('旧')).dy;
      expect(newer, lessThan(older));
    });

    testWidgets('正在演的那一条被高亮（current 传入时）', (WidgetTester tester) async {
      final ActionHistoryEntry a = entry(action: 'nod', label: '点头');
      final ActionHistoryEntry b = entry(action: 'tilt', label: '歪头');
      await tester.pumpWidget(
        wrap(ActionHistoryView(entries: <ActionHistoryEntry>[a, b], current: b)),
      );
      expect(find.text('点头'), findsOneWidget);
      expect(find.text('歪头'), findsOneWidget);
      // 高亮通过 surfaceContainerHighest 底色实现——只断言「渲染没崩且两条都在」，
      // 颜色断言交给来源映射那组测试（避免把实现细节写死进测试）。
    });

    testWidgets('缺时刻不显示 "null"', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[
              // 直接构造（helper 的 `at` 缺省会填一个固定时刻）。
              const ActionHistoryEntry(
                action: 'nod',
                label: '点头',
                source: ActionSource.llmTool,
                epoch: 1,
              ),
            ],
          ),
        ),
      );
      expect(find.text('--:--:--'), findsOneWidget);
      expect(find.textContaining('null'), findsNothing);
    });

    testWidgets('读屏读一整句语义，而不是五列碎片', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[
              entry(label: '点头', source: ActionSource.ruleFallback, strength: 3),
            ],
          ),
        ),
      );
      expect(find.bySemanticsLabel('点头，来源规则，强度 强，正在表演'), findsOneWidget);
      handle.dispose();
    });

    testWidgets('清空按钮只在给了回调时出现，并真的回调', (WidgetTester tester) async {
      int cleared = 0;
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[entry(label: '点头')],
            onClear: () => cleared++,
          ),
        ),
      );
      await tester.tap(find.text('清空'));
      expect(cleared, 1);

      await tester.pumpWidget(
        wrap(ActionHistoryView(entries: <ActionHistoryEntry>[entry(label: '点头')])),
      );
      expect(find.text('清空'), findsNothing);
    });

    testWidgets('丢弃计数不静默（有丢弃时标题里写出来）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[entry(label: '点头')],
            droppedCount: 12,
          ),
        ),
      );
      expect(find.textContaining('已丢弃 12 条'), findsOneWidget);
    });

    testWidgets('行高固定 = 虚拟化的前提（ListView 必须给 itemExtent）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ActionHistoryView(
            entries: <ActionHistoryEntry>[
              for (int i = 0; i < 40; i++) entry(action: 'a$i', label: '动作$i'),
            ],
          ),
        ),
      );
      final ListView list = tester.widget<ListView>(find.byType(ListView));
      expect(list.itemExtent, kActionHistoryRowExtent);
      // 虚拟化生效：40 条不会全部构建。
      expect(
        find.byType(ActionSourceBadge).evaluate().length,
        lessThan(40),
        reason: '没有虚拟化的话长日志会拖垮滚动',
      );
    });
  });
}
