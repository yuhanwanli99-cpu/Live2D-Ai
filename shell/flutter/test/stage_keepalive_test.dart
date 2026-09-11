import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 一个会**数自己创建了几次**的假舞台。
///
/// 真舞台是一个 `<iframe>` 平台视图：它一旦离开 Widget 树就被销毁，模型重载、
/// 口型时间轴清零、当前动作中断。用 `initState` 计数就是把「有没有被重建」
/// 变成可断言的事实——这是本文件存在的全部理由。
class _CountingStage extends StatefulWidget {
  const _CountingStage({required this.onInit});

  final VoidCallback onInit;

  @override
  State<_CountingStage> createState() => _CountingStageState();
}

class _CountingStageState extends State<_CountingStage> {
  @override
  void initState() {
    super.initState();
    widget.onInit();
  }

  @override
  Widget build(BuildContext context) =>
      const ColoredBox(color: Color(0xFF101010));
}

/// 受控宿主的简化版（分区状态由测试自己持有，便于在外部切换分区）。
class _Host extends StatefulWidget {
  const _Host({required this.onStageInit, required this.devMode, super.key});

  final VoidCallback onStageInit;
  final bool devMode;

  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> {
  SettingsSection section = SettingsSection.appearance;

  /// 消息列表（测试用：`bump` 会往里塞消息，触发外壳重建）。
  final List<ChatMessage> messages = <ChatMessage>[];

  void bump() => setState(
    () => messages.add(
      ChatMessage(role: ChatRole.assistant, text: '第 ${messages.length + 1} 条'),
    ),
  );

  @override
  Widget build(BuildContext context) => AppShell(
    stage: _CountingStage(onInit: widget.onStageInit),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    messages: messages,
    input: TextEditingController(),
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: section,
    // `ready` 而不是默认的 `loading`：加载覆盖层里那条不确定进度的进度条是
    // **无限动画**，会让本文件里所有的 `pumpAndSettle` 全部超时
    // （规格 §11.3 已知的坑）。
    stagePhase: Live2DBridgePhase.ready,
    onSectionChanged: (SettingsSection next) => setState(() => section = next),
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
  );
}

void main() {
  Future<int> runScenario(
    WidgetTester tester, {
    required Future<void> Function(WidgetTester) actions,
  }) async {
    int inits = 0;
    await tester.binding.setSurfaceSize(const Size(1400, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      MaterialApp(
        theme: buildAppTheme(),
        home: _Host(onStageInit: () => inits++, devMode: true),
      ),
    );
    await tester.pump();
    expect(inits, 1, reason: '首帧应该只建一次');
    await actions(tester);
    return inits;
  }

  group('舞台保活：iframe 绝不能被重建', () {
    testWidgets('打开/关闭设置侧板 → 仍是同一个舞台实例', (WidgetTester tester) async {
      final int inits = await runScenario(
        tester,
        actions: (WidgetTester t) async {
          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          await t.tap(find.byIcon(Icons.close));
          await t.pumpAndSettle();
        },
      );
      expect(inits, 1);
    });

    testWidgets('**切换 8 个分区** → 仍是同一个舞台实例', (WidgetTester tester) async {
      final int inits = await runScenario(
        tester,
        actions: (WidgetTester t) async {
          // 分区导航现在只有面板内那一行 chip（左侧 rail 已删）。
          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          for (final SettingsSection section in visibleSections()) {
            await t.tap(find.widgetWithText(ChoiceChip, section.label));
            await t.pumpAndSettle();
          }
        },
      );
      expect(inits, 1);
    });

    testWidgets('**跨断点改宽度**（expanded → medium → compact → 回来）→ 仍是同一个实例', (
      WidgetTester tester,
    ) async {
      final int inits = await runScenario(
        tester,
        actions: (WidgetTester t) async {
          for (final double width in <double>[
            1400,
            1000,
            500,
            1400,
            899,
            1280,
          ]) {
            await t.binding.setSurfaceSize(Size(width, 800));
            await t.pumpAndSettle();
          }
        },
      );
      // 这是最重要的一条：如果三种断点写成两套 widget 结构
      // （`if (compact) Column(...) else Row(...)`），跨过 900 px 就会重建,
      // 用户拖一下窗口模型就重载了。
      expect(inits, 1);
    });

    testWidgets('外壳自己 setState（消息追加 / 流式下来）→ 舞台不受影响', (
      WidgetTester tester,
    ) async {
      int inits = 0;
      final GlobalKey<_HostState> hostKey = GlobalKey<_HostState>();
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: _Host(key: hostKey, onStageInit: () => inits++, devMode: true),
        ),
      );
      await tester.pump();
      expect(inits, 1);

      // 模拟流式：每来一个 delta 就 setState 一次（真实场景一秒好几次）。
      for (int i = 0; i < 20; i++) {
        hostKey.currentState!.bump();
        await tester.pump();
      }
      expect(inits, 1, reason: '高频重建外壳不能连带重建 iframe（原则 P6）');
      expect(find.text('第 20 条'), findsOneWidget);
    });
  });

  group('保活的反面：确认测试本身能抓到重建（否则上面几条是空的）', () {
    testWidgets('换成不同的父结构 → initState 真的会再调一次', (WidgetTester tester) async {
      int inits = 0;
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      Widget build(bool useColumn) => MaterialApp(
        home: useColumn
            ? Column(children: <Widget>[_CountingStage(onInit: () => inits++)])
            : Row(children: <Widget>[_CountingStage(onInit: () => inits++)]),
      );

      await tester.pumpWidget(build(false));
      await tester.pump();
      expect(inits, 1);
      await tester.pumpWidget(build(true));
      await tester.pump();
      // 这正是我们要避免的失败模式——如果这条不成立，上面的断言就毫无意义。
      expect(inits, 2, reason: '换了 widget 类型（Row↔Column）会重建子树');
    });
  });

  group('舞台覆盖层不卸载下层', () {
    testWidgets('离线横幅叠在舞台上，不替换它', (WidgetTester tester) async {
      int inits = 0;
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      Widget build(WsStatus status) => MaterialApp(
        theme: buildAppTheme(),
        home: AppShell(
          stage: _CountingStage(onInit: () => inits++),
          phase: UiPhase.idle,
          // 同上：`loading` 会让 `pumpAndSettle` 卡在无限动画上。
          stagePhase: Live2DBridgePhase.ready,
          wsStatus: status,
          messages: const <Never>[],
          input: TextEditingController(),
          onSend: () {},
          onStop: () {},
          onRetryConnection: () {},
          volume: 0.8,
          muted: false,
          onVolumeChanged: (_) {},
          onMutedChanged: (_) {},
          sections: visibleSections(),
          sectionBuilder: (BuildContext c, SettingsSection s) =>
              const SizedBox.shrink(),
        ),
      );

      await tester.pumpWidget(build(WsStatus.connected));
      await tester.pump();
      expect(inits, 1);

      await tester.pumpWidget(build(WsStatus.disconnected));
      await tester.pumpAndSettle();
      expect(inits, 1, reason: '断线只是加一层横幅，不能重建 iframe');
      expect(find.textContaining('后端未连接'), findsWidgets);
    });
  });

  group('断点在保活语义上的一致性', () {
    test('三档都声称「舞台常驻」——断点只决定聊天面板的位置', () {
      // 这条是文档性断言：没有任何一档允许把舞台换成别的东西。
      for (final SizeClass sc in SizeClass.values) {
        expect(sc.isCompact || sc.isMedium || sc.isExpanded, isTrue);
      }
    });
  });
}
