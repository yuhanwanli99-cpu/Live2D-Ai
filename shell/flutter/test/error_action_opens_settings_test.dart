/// F-0001-1（P1）：错误横幅的「去 LLM / 语音合成设置」在**设置面板关闭态**
///（默认态）点下去**没反应**。
///
/// # 根因
///
/// 错误动作的 `onGoto` 接的是 `_gotoSection`，而它只负责「**换分区**」——
/// 默认态下面板是**关着**的。于是点「去 LLM 设置」时分区确实换了，但用户
/// 面前的设置面板仍然关着，读到的就是「按钮失灵」（TRIAGE §1 第 23 行）。
/// 修法（HANDOFF §4 Step 0）：换完分区之后**真的把面板打开**。
///
/// 两个容易写错的点，各有一条测试钉住：
///
/// 1. `_gotoSection` 开头有早退（`main.dart:1129`：`if (next == _section) return;`）
///    ——用户**已经停在**目标分区时它什么都不做，所以「打开」那一步必须写在
///    `_gotoSection` **之外**；
/// 2. 「换分区 + 开浮层」**不能在同一帧里做**（medium/sheet 宿主实测会撞框架
///    断言 `setState() or markNeedsBuild() called during build`：`AppShell
///    .didUpdateWidget` 同步 `sectionNotifier` 时，浮层里那个
///    `ValueListenableBuilder` 已经挂上，而它是 Overlay 下的**兄弟**、不是宿主的
///    后代）⇒ 打开那一步延到本帧之后，见 `main.dart` 里的注释。
///
/// # 为什么是「同形宿主 + 结构守卫」两条
///
/// `main.dart` 是 `package:web` + `part` 组合根，**VM 里 import 不了** ⇒
/// 那几行本身无法在 `flutter test` 里直接跑。所以：
///
/// 1. **行为级**（前三条）：用**与 `main.dart` 的错误动作接线同形**的宿主
///    （真 `AppShell` + 真 `errorActionsFor`）证明「补了这一步，关闭态点下去
///    面板**真的开**」——断言落在 `find` 到的面板内容与侧板宽度上；
/// 2. **结构守卫**（最后一条）：扫 `main.dart` 的 `errorActionsFor(...)` 实参，
///    断言 `onGoto` 的回调体里**确实**调了 `openSettings()`——否则第 1 条只
///    证明了宿主，证明不了生产接线。
///
/// 两条合起来才等于「生产路径上点了会开面板」；**只有**第 2 条会退化成源码
/// 文本扫描，所以它不被允许单独存在。
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/collapsible_panel.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/error_actions.dart';
import 'package:live2d_ai_shell/ui/error_banner.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

/// 侧板里那个折叠容器。
///
/// `InlineSettingsDock` 本身是 `Positioned.fill`，量宽度要用里面这个折叠容器
/// （与 `app_shell_layout_test.dart` 同一判据）；侧板「关着」= 折到 0 宽，
/// 不是从树里卸载。
Finder dockPanel() => find.descendant(
  of: find.byType(InlineSettingsDock),
  matching: find.byType(CollapsiblePanel),
);

/// 面板里那个分区内容的文本。
///
/// **从枚举取值、不写字面量**：`SettingsSection.llm.label` 是 `'LLM'`，而它对应
/// 分区在界面里的标题是「对话模型」——写死标题会得到一条「面板其实开了、但断言
/// 找不到」的假红。
String paneTextOf(SettingsSection section) => 'PANE:${section.label}';

/// 与 `main.dart` 的错误动作接线**同形**的宿主（受控分区 + 真 `AppShell`）。
class _ErrorHost extends StatefulWidget {
  const _ErrorHost({required this.shellKey, required this.initialSection});

  final GlobalKey<AppShellState> shellKey;
  final SettingsSection initialSection;

  @override
  State<_ErrorHost> createState() => _ErrorHostState();
}

class _ErrorHostState extends State<_ErrorHost> {
  late SettingsSection _section = widget.initialSection;

  /// `main.dart:1128-1138` 的同形实现（含**那条早退**）。
  void _gotoSection(SettingsSection next) {
    if (next == _section) return;
    setState(() => _section = next);
  }

  /// `main.dart` 错误动作接线的同形实现（`errorActionsFor(... onGoto: ...)`）。
  List<ErrorAction> get _errorActions => errorActionsFor(
    '上游返回 401（错误详情见诊断日志）',
    code: 'llm_upstream_401',
    onGoto: (SettingsSection next) {
      _gotoSection(next);
      // ← F-0001-1 的那一步。**延到本帧之后**（原因见文件头注第 2 条：
      //   同一帧里「换分区 + 开浮层」会撞 `markNeedsBuild during build`）。
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        unawaited(
          widget.shellKey.currentState?.openSettings() ?? Future<void>.value(),
        );
      });
    },
    // busy 的出路（这里用不到：本文件测的是 `onGoto` 那条），但签名的
    // 组合回调必须给——见 `test/error_action_busy_resend_test.dart`。
    onInterruptAndResend: () {},
    onResendLast: () {},
  );

  @override
  Widget build(BuildContext context) => AppShell(
    key: widget.shellKey,
    prefs: const DisplayPrefs(),
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    // **必须显式 `ready`**：`loading` 会渲染一个不确定进度的
    // `LinearProgressIndicator`（无限动画）⇒ 任何 `pumpAndSettle` 都会超时
    // （`app_shell_layout_test.dart` 头注记过同一个坑）。
    stagePhase: Live2DBridgePhase.ready,
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
    section: _section,
    onSectionChanged: (SettingsSection next) => setState(() => _section = next),
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text(paneTextOf(s)),
    error: '上游返回 401（错误详情见诊断日志）',
    errorActions: _errorActions,
  );
}

/// 按**真实布局宽度**渲染，并让 `MediaQuery.size` 与布局约束一致。
///
/// `setSurfaceSize` 只改根约束，`MediaQuery.sizeOf` 仍是默认的 800×600
/// （实测：`setSurfaceSize(1000×800)` 之后 `MediaQuery.sizeOf` 打印 `800×600`）。
/// 生产环境里两者本来就相等（都是窗口尺寸）；不手动对齐的话，`openSettings()`
///（读 `MediaQuery`）与外壳布局（读 `LayoutBuilder` 约束）会算出**两个不同的
/// 断点**，`settingsOpen = true` 就落进一个不渲染它的布局里——那条红与产品无关。
Future<void> _pumpHost(
  WidgetTester tester, {
  required GlobalKey<AppShellState> shellKey,
  required double width,
  SettingsSection initialSection = SettingsSection.appearance,
}) async {
  await tester.binding.setSurfaceSize(Size(width, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      builder: (BuildContext context, Widget? child) => MediaQuery(
        data: MediaQuery.of(context).copyWith(size: Size(width, 800)),
        child: child!,
      ),
      home: _ErrorHost(shellKey: shellKey, initialSection: initialSection),
    ),
  );
  await tester.pump();
}

/// 点错误横幅的「去 LLM 设置」，并让「换分区」与「打开面板」两帧都跑完。
Future<void> _tapErrorAction(WidgetTester tester) async {
  await tester.tap(find.text('去 LLM 设置'));
  await tester.pumpAndSettle();
}

void main() {
  // ─────────────────────────────────────────────────────────────────────
  // 行为级（真 AppShell + 真 errorActionsFor）
  // ─────────────────────────────────────────────────────────────────────

  testWidgets('expanded：关闭态点「去 LLM 设置」→ 内联侧板真的展开并停在 LLM', (
    WidgetTester tester,
  ) async {
    final GlobalKey<AppShellState> shellKey = GlobalKey<AppShellState>();
    await _pumpHost(tester, shellKey: shellKey, width: 1400);

    // 前提：面板关着（expanded 的侧板常驻在树里，所以判据是**宽度**）。
    expect(
      tester.getSize(dockPanel()).width,
      lessThan(1),
      reason: '前提：侧板折到 0 宽',
    );

    await _tapErrorAction(tester);

    expect(
      tester.getSize(dockPanel()).width,
      greaterThanOrEqualTo(NavMetrics.paneWidth),
      reason: '修复前：点「去设置」面板完全不展开（默认态就是关着的）',
    );
    expect(find.text(paneTextOf(SettingsSection.llm)), findsOneWidget);
  });

  testWidgets('medium：关闭态点「去 LLM 设置」→ 浮层真的打开并停在 LLM', (
    WidgetTester tester,
  ) async {
    final GlobalKey<AppShellState> shellKey = GlobalKey<AppShellState>();
    await _pumpHost(tester, shellKey: shellKey, width: 1000);

    // 前提：面板关着 → 面板内容不在树上（medium 是浮层宿主）。
    expect(
      find.text(paneTextOf(SettingsSection.llm)),
      findsNothing,
      reason: '前提：设置面板是关着的',
    );
    expect(find.text('去 LLM 设置'), findsOneWidget, reason: '前提：错误横幅给出了那条出路');

    await _tapErrorAction(tester);

    // F-0001-1：**面板被打开**（find 到面板内容），而不是「函数被调用」。
    expect(
      find.text(paneTextOf(SettingsSection.llm)),
      findsOneWidget,
      reason: '修复前：分区换了但面板没开 ⇒ 用户看到「点了没反应」',
    );
  });

  testWidgets('已在目标分区时也要打开：`_gotoSection` 的早退不许吞掉这一步', (
    WidgetTester tester,
  ) async {
    final GlobalKey<AppShellState> shellKey = GlobalKey<AppShellState>();
    // 用户**已经停在** LLM 分区 → `_gotoSection` 早退（什么都不做）
    // ⇒ 若「打开」那一步被塞进 `_gotoSection` 内部，这条路径永远打不开面板。
    await _pumpHost(
      tester,
      shellKey: shellKey,
      width: 1000,
      initialSection: SettingsSection.llm,
    );
    expect(
      find.text(paneTextOf(SettingsSection.llm)),
      findsNothing,
      reason: '前提：面板仍关着',
    );

    await _tapErrorAction(tester);

    expect(
      find.text(paneTextOf(SettingsSection.llm)),
      findsOneWidget,
      reason: '早退路径同样必须把面板打开——所以那一步必须写在 `_gotoSection` **之外**',
    );
  });

  // ─────────────────────────────────────────────────────────────────────
  // 结构守卫：把上面的「同形宿主」钉回**生产接线**
  // ─────────────────────────────────────────────────────────────────────

  test('结构：main.dart 的错误动作 onGoto 回调体内调了 openSettings()', () {
    final String src = stripCommentsAndStrings(
      File('lib/main.dart').readAsStringSync(),
    );

    final String args = balancedFrom(src, 'errorActionsFor(', '(', ')');
    expect(
      args,
      contains('onGoto'),
      reason: '错误动作必须仍有「去设置」那条出路（规格 §6.6：失败必须有出路）',
    );

    final String? onGotoBody = closureBodyAfter(args, 'onGoto:');
    expect(
      onGotoBody,
      isNotNull,
      reason: 'onGoto 必须是**回调**（能写两句话）；裸 tear-off `_gotoSection` '
          '就没地方补「打开面板」这一步了（F-0001-1 修复前正是这个形状）',
    );
    expect(
      RegExp(r'openSettings\s*\(').hasMatch(onGotoBody!),
      isTrue,
      reason: 'F-0001-1：换分区之后必须真的把设置面板打开，'
          '否则面板关闭态（默认态）下点「去 LLM 设置」= 点了没反应。'
          '类型 = 结构（main.dart 在 VM 下 import 不了），'
          '行为级那一半见本文件前三条。',
    );
  });
}
