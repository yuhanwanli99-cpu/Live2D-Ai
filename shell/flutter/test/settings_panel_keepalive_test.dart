/// 设置侧板的**保活**回归（2026-09-11，前端加强计划 P1-1）。
///
/// # 这条钉子防的是什么
///
/// `app_shell` 过去写的是 `if (settingsOpen && inlineSettings) Positioned.fill(...)`：
/// 关一次设置 = 侧板整棵子树**卸载**，再打开 = **重建**。
/// 用户看到的是「我刚滚到下面把某项改完，关一下再开，滚动条跳回顶部」。
///
/// 现在侧板常驻在树里，靠 `CollapsiblePanel` 把宽度折到 0。
/// 本文件用 `initState` 计数把「有没有被重建」变成可断言的事实——
/// 与 `stage_keepalive_test.dart` 同一手法（那条是给 iframe 用的，
/// 这条是给设置面板用的，两者失败模式一样：**不报错，只是状态没了**）。
///
/// # 为什么不用 `find.text(...)`
///
/// 折叠之后内容**仍在树里**（那正是保活的代价），所以
/// 「找得到 / 找不到」区分不出「折起来了」和「还开着」。
/// 判据必须是**宽度**（`CollapsiblePanel` 的 size）与**焦点可达性**。
library;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/collapsible_panel.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 会数自己创建了几次的假设置面板内容。
///
/// 真面板里装的是 `SettingsScaffold` + 滚动视图 + 一堆字段，
/// 用户看得见的「状态丢失」就是从这些 Element 被重建开始的，
/// 所以这里把它们压成一个可计数的替身。
class _CountingPane extends StatefulWidget {
  const _CountingPane({required this.onInit, required this.section});

  final VoidCallback onInit;
  final SettingsSection section;

  @override
  State<_CountingPane> createState() => _CountingPaneState();
}

class _CountingPaneState extends State<_CountingPane> {
  int builds = 0;

  @override
  void initState() {
    super.initState();
    widget.onInit();
  }

  @override
  Widget build(BuildContext context) {
    builds++;
    // 用 `Column` 而不是 `ListView`：真面板里这个 child 是被
    // `SettingsScaffold` 塞进 `SingleChildScrollView` 的，那边高度无界，
    // 里面再放一个 ListView 会直接抛 `hasSize`。滚动由外层的
    // `SingleChildScrollView` 负责——本测试量的正是**它**的位置。
    return Column(
      children: <Widget>[
        for (int i = 0; i < 60; i++)
          SizedBox(
            height: 40,
            child: Text('${widget.section.label} 第 $i 行'),
          ),
      ],
    );
  }
}

class _Host extends StatefulWidget {
  const _Host({required this.onPaneInit});

  final VoidCallback onPaneInit;

  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> {
  SettingsSection section = SettingsSection.appearance;

  @override
  Widget build(BuildContext context) => AppShell(
    stage: const ColoredBox(color: Color(0xFF101010)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
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
    section: section,
    onSectionChanged: (SettingsSection next) => setState(() => section = next),
    sectionBuilder: (BuildContext context, SettingsSection s) => _CountingPane(
      // **不给 key**：给了会变的东西当 key 就等于主动放弃复用。
      onInit: widget.onPaneInit,
      section: s,
    ),
    stagePhase: Live2DBridgePhase.ready,
  );
}

/// 设置侧板里那个折叠容器（断线横幅也是 `CollapsiblePanel`，必须限定范围）。
Finder dockPanel() => find.descendant(
  of: find.byType(InlineSettingsDock),
  matching: find.byType(CollapsiblePanel),
);

double dockWidth(WidgetTester tester) => tester.getSize(dockPanel()).width;

/// 侧板内那个滚动视图的位置（真实面板里是 `SettingsScaffold` 的
/// `SingleChildScrollView`；这里就是它）。
double panelScroll(WidgetTester tester) => tester
    .state<ScrollableState>(
      find.descendant(of: dockPanel(), matching: find.byType(Scrollable)).first,
    )
    .position
    .pixels;

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
        home: _Host(onPaneInit: () => inits++),
      ),
    );
    await tester.pumpAndSettle();
    expect(inits, 1, reason: '首帧应该只建一次面板');
    await actions(tester);
    return inits;
  }

  group('P1-1 设置侧板保活：关掉再打开不重建', () {
    testWidgets('开 → 关 → 再开，面板实例始终是同一个', (WidgetTester tester) async {
      final int inits = await runScenario(
        tester,
        actions: (WidgetTester t) async {
          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          expect(dockWidth(t), greaterThanOrEqualTo(NavMetrics.paneWidth));

          await t.tap(find.byIcon(Icons.close));
          await t.pumpAndSettle();
          expect(dockWidth(t), lessThan(1), reason: '关 = 折到 0 宽');

          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          expect(dockWidth(t), greaterThanOrEqualTo(NavMetrics.paneWidth));
        },
      );
      expect(
        inits,
        1,
        reason: '面板被重建了 —— 用户的滚动位置与字段状态会一起丢',
      );
    });

    testWidgets('**滚动位置**真的留住了（这才是保活的用户可见收益）', (WidgetTester tester) async {
      double offsetAfterClose = 0;
      await runScenario(
        tester,
        actions: (WidgetTester t) async {
          await t.tap(find.text('设置'));
          await t.pumpAndSettle();

          // 拖**可见的滚动视口**，不是拖内容：内容的中心点在视口外面，
          // 从那里起手等于在面板外面滑，什么也不会发生。
          await t.drag(
            find.descendant(of: dockPanel(), matching: find.byType(Scrollable)),
            const Offset(0, -400),
          );
          await t.pumpAndSettle();
          expect(
            panelScroll(t),
            greaterThan(100),
            reason: '先得真的滚下去，否则这条断言什么也没证明',
          );

          await t.tap(find.byIcon(Icons.close));
          await t.pumpAndSettle();
          offsetAfterClose = panelScroll(t);

          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          expect(
            panelScroll(t),
            closeTo(offsetAfterClose, 0.5),
            reason: '再打开时回到了顶部 —— 说明面板被重建了',
          );
        },
      );
    });

    testWidgets('折起来的面板**不进 Tab 序、不被读屏念到、展开后恢复**', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await runScenario(
        tester,
        actions: (WidgetTester t) async {
          try {
          // 折起来时（初始态）：
          expect(
            find.byType(_CountingPane),
            findsOneWidget,
            reason: '折叠不等于卸载 —— 这正是滚动位置能留住的原因',
          );
          final Finder firstChip = find.byType(ChoiceChip).first;
          expect(
            Focus.of(t.element(firstChip)).canRequestFocus,
            isFalse,
            reason: '收起的面板还在 Tab 序里 —— 键盘用户会掉进看不见的控件',
          );
          // 读屏：`ExcludeSemantics` 会把整棵子树从语义树里摘掉，
          // 所以正确的判据是「**找不到**它的标签」，不是「它被标成 hidden」。
          expect(
            find.bySemanticsLabel(RegExp('外观与互动')),
            findsNothing,
            reason: '收起的面板仍会被读屏念到（ExcludeSemantics 没生效）',
          );

          // 展开后必须全部恢复 —— 别把「关掉」做成了「永久禁用」。
          // 判据用**真的点一下**而不是再读一次 `canRequestFocus`：
          // 那边返回的是「这个 Focus 节点自己能不能要焦点」，
          // 在 `ExcludeFocus` 的层叠下语义不直观，点得着才是用户要的。
          await t.tap(find.text('设置'));
          await t.pumpAndSettle();
          expect(find.bySemanticsLabel(RegExp('外观与互动')), findsWidgets);

          await t.tap(find.widgetWithText(ChoiceChip, '语音合成'));
          await t.pumpAndSettle();
          expect(
            find.text('语音合成 第 0 行'),
            findsOneWidget,
            reason: '展开后分区 chip 点不动 —— 保活做成了永久禁用',
          );
          } finally {
            // `SemanticsHandle` 必须在**测试体结束前**释放
            // （`addTearDown` 太晚：框架的检查跑在 tearDown 之前）。
            handle.dispose();
          }
        },
      );
    });
  });
}
