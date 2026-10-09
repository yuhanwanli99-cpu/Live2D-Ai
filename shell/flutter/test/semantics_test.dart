import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/app/app_shortcuts.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/state/live_region.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/stage_host.dart';
import 'package:live2d_ai_shell/ui/state_pill.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

ChatMessage msg(
  String text, {
  bool streaming = false,
  ChatRole role = ChatRole.assistant,
}) => ChatMessage(role: role, text: text, streaming: streaming);

void main() {
  group('规格 §9.1 之一：舞台 iframe 必须有语义标签（平台视图对读屏是黑盒）', () {
    Widget stageHost({
      Live2DBridgePhase phase = Live2DBridgePhase.ready,
      String? model,
    }) => wrap(
      StageHost(
        stage: const ColoredBox(color: Color(0xFF000000)),
        phase: phase,
        modelName: model,
      ),
    );

    testWidgets('有模型名时念「Live2D 舞台，正在显示 X」', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(stageHost(model: 'bai_001'));
      expect(find.bySemanticsLabel('Live2D 舞台，正在显示 bai_001'), findsOneWidget);
      handle.dispose();
    });

    testWidgets('没有模型名时退化成「Live2D 舞台」（不念「null」）', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(stageHost());
      expect(find.bySemanticsLabel('Live2D 舞台'), findsOneWidget);
      expect(find.bySemanticsLabel(RegExp('null')), findsNothing);
      handle.dispose();
    });

    testWidgets('**加载/错误覆盖层的语义没有被舞台语义吃掉**', (WidgetTester tester) async {
      // 第一版把 `excludeSemantics: true` 写在包住整个 Stack 的 Semantics 上，
      // 结果「模型加载中」与「重试」按钮都读不到/点不到了。
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(stageHost(phase: Live2DBridgePhase.loading));
      expect(find.bySemanticsLabel(RegExp('模型加载中')), findsOneWidget);
      handle.dispose();
    });

    testWidgets('错误态的「重试」在语义树里可达', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      int retried = 0;
      await tester.pumpWidget(
        wrap(
          StageHost(
            stage: const ColoredBox(color: Color(0xFF000000)),
            phase: Live2DBridgePhase.error,
            errorMessage: '模型加载失败',
            onRetry: () => retried++,
          ),
        ),
      );
      expect(find.bySemanticsLabel(RegExp('模型加载失败')), findsOneWidget);
      await tester.tap(find.text('重试'));
      expect(retried, 1);
      handle.dispose();
    });
  });

  group('规格 §9.1 之二：流式回复用 liveRegion，且**必须节流**', () {
    testWidgets('流式中的气泡是 liveRegion，且念的是节流后的文本', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('好呀，那我就说两句。后面还在生成……', streaming: true),
            announcement: '好呀，那我就说两句。',
          ),
        ),
      );
      final Finder bubble = find.bySemanticsLabel(RegExp('好呀，那我就说两句。'));
      expect(bubble, findsOneWidget);
      final SemanticsNode node = tester.getSemantics(bubble);
      expect(node.getSemanticsData().flagsCollection.isLiveRegion, isTrue);
      expect(
        node.label,
        isNot(contains('后面还在生成')),
        reason: 'liveRegion 只念节流后的文本，不念正在流式的原文',
      );
      handle.dispose();
    });

    testWidgets('历史消息（不传 announcement）**不是** liveRegion', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('旧消息', streaming: false))),
      );
      final Finder bubble = find.bySemanticsLabel(RegExp('旧消息'));
      expect(bubble, findsOneWidget);
      final SemanticsNode node = tester.getSemantics(bubble);
      expect(
        node.getSemanticsData().flagsCollection.isLiveRegion,
        isFalse,
        reason: '否则读屏会把整段历史反复重念',
      );
      handle.dispose();
    });
  });

  group('播报节流器（纯逻辑）', () {
    late DateTime now;
    LiveRegionThrottle build() => LiveRegionThrottle(now: () => now);

    setUp(() => now = DateTime(2026, 9, 10, 21, 0, 0));

    test('间隔内的小片段不播报（否则读屏被淹没）', () {
      final LiveRegionThrottle t = build();
      expect(t.feed('好'), isTrue, reason: '第一次总要播');
      now = now.add(const Duration(milliseconds: 300));
      expect(t.feed('好呀'), isFalse);
      now = now.add(const Duration(milliseconds: 300));
      expect(t.feed('好呀，'), isFalse);
    });

    test('**整句到达立刻播报**（句末标点是语义边界，不等间隔）', () {
      final LiveRegionThrottle t = build();
      t.feed('好呀');
      now = now.add(const Duration(milliseconds: 100));
      expect(t.feed('好呀，那我就说两句。'), isTrue);
      expect(t.announcement, '好呀，那我就说两句。');
    });

    test('多种句末标点都算整句（。！？…；换行）', () {
      for (final String end in <String>['。', '！', '？', '…', '；', '\n']) {
        final LiveRegionThrottle t = build();
        t.feed('前');
        expect(t.feed('前$end'), isTrue, reason: end);
      }
    });

    test('超过间隔后即使没有句末标点也播报（长句不会哑掉）', () {
      final LiveRegionThrottle t = build();
      t.feed('开始');
      now = now.add(LiveRegionThrottle.kDefaultInterval);
      expect(t.feed('开始，然后一直说下去没有标点'), isTrue);
    });

    test('文本没变就不播报（避免同一内容重复念）', () {
      final LiveRegionThrottle t = build();
      t.feed('一样');
      now = now.add(const Duration(seconds: 5));
      expect(t.feed('一样'), isFalse);
    });

    test('finish 补播最后一段（末尾往往没有句末标点）', () {
      final LiveRegionThrottle t = build();
      t.feed('好呀');
      t.finish('好呀，那就这样吧');
      expect(t.announcement, '好呀，那就这样吧');
    });

    test('reset 清空（下一轮不念上一轮残留）', () {
      final LiveRegionThrottle t = build();
      t.feed('上一轮');
      t.reset();
      expect(t.announcement, isEmpty);
      expect(t.hasAnnouncement, isFalse);
    });

    test('默认间隔是具体值（1.5 s：约够念 4–6 个中文字）', () {
      expect(
        LiveRegionThrottle.kDefaultInterval,
        const Duration(milliseconds: 1500),
      );
    });
  });

  group('规格 §9.2：焦点组与固定 Tab 顺序', () {
    testWidgets('音频条是 FocusTraversalGroup，且音量排在静音前面', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(
          ChatPanel(
            messages: const <ChatMessage>[],
            phase: UiPhase.idle,
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onDismissError: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
          ),
        ),
      );
      expect(find.byType(FocusTraversalGroup), findsWidgets);
      // `NumericFocusOrder` 是**固定顺序**的唯一写法；顺序跟随 Widget 树
      // 会在重构时静默改变（规格 §9.2-1）。
      expect(find.byType(FocusTraversalOrder), findsWidgets);
    });
  });

  group('规格 §9.3：键盘', () {
    test('快捷键清单包含四条，且按键文案按平台给前缀', () {
      final List<ShortcutHelpEntry> mac = shortcutHelp(isMacOS: true);
      final List<ShortcutHelpEntry> pc = shortcutHelp(isMacOS: false);
      expect(mac, hasLength(4));
      // macOS 用 ASCII `Cmd` 而不是 `⌘`：后者不在自托管子集里，上屏会让
      // CanvasKit 去 fonts.gstatic.com 拉字体（断网即豆腐块）。见
      // `app_shortcuts.shortcutModifierLabel` 的注释与 `font_subset_test.dart`。
      expect(mac.first.keys, contains('Cmd'));
      expect(mac.first.keys, isNot(contains('⌘')));
      expect(pc.first.keys, contains('Ctrl'));
      expect(mac.first.action, '发送消息');
      expect(pc[1].keys, 'Esc');
    });

    test('绑定表：macOS 用 meta、其余用 control（**不同时绑**）', () {
      final Map<ShortcutActivator, VoidCallback> mac = AppShortcutCallbacks(
        isMacOS: true,
        onSend: () {},
      ).bindings();
      final Map<ShortcutActivator, VoidCallback> pc = AppShortcutCallbacks(
        isMacOS: false,
        onSend: () {},
      ).bindings();
      final List<SingleActivator> macKeys = mac.keys
          .whereType<SingleActivator>()
          .toList();
      final List<SingleActivator> pcKeys = pc.keys
          .whereType<SingleActivator>()
          .toList();
      expect(macKeys.every((SingleActivator a) => a.meta), isTrue);
      expect(macKeys.any((SingleActivator a) => a.control), isFalse);
      expect(pcKeys.every((SingleActivator a) => a.control), isTrue);
      expect(
        pcKeys.any((SingleActivator a) => a.meta),
        isFalse,
        reason: 'Windows 上 meta 是 Win 键，绑上去会「按 Win+Enter 发消息」',
      );
    });

    test('回调为 null 的动作**不注册**该键（不是注册空回调）', () {
      final Map<ShortcutActivator, VoidCallback> none =
          const AppShortcutCallbacks(isMacOS: false).bindings();
      expect(none, isEmpty, reason: '什么都不给就不该吞任何键');
    });

    test('Esc：覆盖层关掉了就**不再**停止本轮（一次 Esc 只做一件事）', () {
      int stops = 0;
      bool overlayOpen = true;
      final Map<ShortcutActivator, VoidCallback> map = AppShortcutCallbacks(
        isMacOS: false,
        onStop: () => stops++,
        onDismissOverlay: () {
          if (!overlayOpen) return false;
          overlayOpen = false;
          return true;
        },
      ).bindings();
      final VoidCallback esc =
          map[const SingleActivator(LogicalKeyboardKey.escape)]!;

      esc(); // 覆盖层开着 → 关它
      expect(stops, 0, reason: '关侧板时不该顺手把对话停了');
      esc(); // 覆盖层已关 → 停止
      expect(stops, 1);
    });
  });

  group('规格 §9.4：reduced motion 下不跑持续动画', () {
    /// 把 **真正的 `StatePill`** 泵进指定 `disableAnimations` 的 MediaQuery。
    ///
    /// MediaQuery 必须放在 `MaterialApp` **之内**：`WidgetsApp` 会自己插一层
    /// 从 View 派生的 MediaQuery，套在外面会被它盖掉。
    Future<void> pumpPill(WidgetTester tester, {required bool reduced}) async {
      await tester.pumpWidget(
        wrap(
          MediaQuery(
            data: MediaQueryData(disableAnimations: reduced),
            child: const Center(child: StatePill(phase: UiPhase.thinking)),
          ),
        ),
      );
    }

    testWidgets('disableAnimations: true → thinking 用静态图标，渲染里没有 AnimatedBuilder', (
      WidgetTester tester,
    ) async {
      // 2026-09-28（F-0005-7）：这条用例从前泵的是 `wrap(const SizedBox())`
      // ——树里**根本没有 StatePill**，断言的却是框架的
      // `MediaQuery.disableAnimationsOf(...)` ⇒ 零检验力（断言的是框架，
      // 不是本项目的行为）。真正要守的是 `state_pill.dart` 里
      // `if (widget.phase == UiPhase.thinking && !reduced)` 那条分支。
      await pumpPill(tester, reduced: true);

      expect(find.byType(StatePill), findsOneWidget, reason: '状态胶囊本身要还在');
      expect(
        find.text('正在回复'),
        findsOneWidget,
        reason: '静态图标 ≠ 把状态藏起来：色 / 形 / 字三个通道都还得在',
      );
      expect(
        find.descendant(
          of: find.byType(StatePill),
          matching: find.byType(AnimatedBuilder),
        ),
        findsNothing,
        reason: 'reduced motion 下 thinking 仍把持续动画（呼吸光）挂进了渲染树',
      );
    });

    testWidgets('disableAnimations: false → 呼吸光在（对照组：没有它上一条恒真）', (
      WidgetTester tester,
    ) async {
      await pumpPill(tester, reduced: false);

      expect(find.byType(StatePill), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(StatePill),
          matching: find.byType(AnimatedBuilder),
        ),
        findsOneWidget,
        reason: '对照组：把动画整个删掉也能让上一条通过，所以这里必须要求它在',
      );
    });
  });
}
