/// 「压在舞台上的控件必须垫指针垫层」的回归钉子（2026-09-11）。
///
/// # 为什么值得单独一个文件
///
/// Flutter Web 的舞台是 `<iframe>` 平台视图，DOM 上排在 Flutter 画布之上，
/// 而画布是 `pointer-events: none`。指针落在舞台区域会进 iframe **自己的文档**，
/// 父页收不到——于是压在舞台上的 Flutter 控件全部「看得见、点不着、也滑不动」。
/// 用户报的就是这个：**设置能唤醒，但点不动、不能上下滑**。
///
/// 修法见 `StagePointerInterceptor` 的头注。失败模式很坏：**不报错、只是点不着**，
/// 所以这里把已知的接线点逐个钉住——漏一处，这里就红。
///
/// 注意：`flutter test` 跑在 VM 上，垫层走 `..._stub.dart`（透明返回 child），
/// 所以这里验的是**接线**，不是 DOM 行为。DOM 那一半只能真机验收
/// （2026-09-11 已在浏览器里验过：`elementsFromPoint` 在设置面板上不再是 IFRAME）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/stage_pointer_interceptor.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 外壳是受控的（`section` 由宿主持有），但本文件只关心「谁垫了垫层」——
/// 所以给一个固定的分区 + 空回调就够了（换分区的行为在
/// `app_shell_layout_test.dart` 里另有钉子）。
Widget _shell({
  double width = 1400,
  WsStatus ws = WsStatus.connected,
  Live2DBridgePhase stagePhase = Live2DBridgePhase.ready,
  VoidCallback? onRetryStage,
  Widget? stageCorner,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: AppShell(
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: UiPhase.idle,
    wsStatus: ws,
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
    section: SettingsSection.appearance,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
    stagePhase: stagePhase,
    onRetryStage: onRetryStage,
    stageCorner: stageCorner,
  ),
);

Future<void> _pump(
  WidgetTester tester, {
  double width = 1400,
  WsStatus ws = WsStatus.connected,
  Live2DBridgePhase stagePhase = Live2DBridgePhase.ready,
  VoidCallback? onRetryStage,
  Widget? stageCorner,
}) async {
  await tester.binding.setSurfaceSize(Size(width, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    _shell(
      width: width,
      ws: ws,
      stagePhase: stagePhase,
      onRetryStage: onRetryStage,
      stageCorner: stageCorner,
    ),
  );
  await tester.pump();
}

/// `inner` 外面**至少**有一层指针垫层。
void expectIntercepted(Finder inner, String what) {
  expect(
    find.ancestor(of: inner, matching: find.byType(StagePointerInterceptor)),
    findsWidgets,
    reason: '$what 压在舞台上却没有垫 $StagePointerInterceptor：点不着',
  );
}

void main() {
  testWidgets('非 web 上垫层完全透明（不改尺寸、不挡内容）', (WidgetTester tester) async {
    const Key inner = Key('inner');
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: StagePointerInterceptor(
            child: SizedBox(key: inner, width: 120, height: 40),
          ),
        ),
      ),
    );
    expect(tester.getSize(find.byKey(inner)), const Size(120, 40));
  });

  group('压在舞台上的常驻控件', () {
    testWidgets('断线横幅的「点此重试」有垫层', (WidgetTester tester) async {
      await _pump(tester, ws: WsStatus.disconnected);
      expectIntercepted(find.textContaining('点此重试'), '断线横幅');
    });

    testWidgets('舞台右下角浮标有垫层', (WidgetTester tester) async {
      await _pump(tester, stageCorner: const Text('缩小 放大 复位'));
      expectIntercepted(find.text('缩小 放大 复位'), '舞台角标');
    });
  });

  group('压在舞台上的覆盖层', () {
    testWidgets('loading 覆盖层有垫层', (WidgetTester tester) async {
      // 不确定进度的进度条是无限动画 ⇒ 只能 `pump`，不能 `pumpAndSettle`。
      await _pump(tester, stagePhase: Live2DBridgePhase.loading);
      expectIntercepted(find.textContaining('模型加载中'), '加载覆盖层');
    });

    testWidgets('error 覆盖层的「重试」有垫层（并且真的点得着）', (WidgetTester tester) async {
      int retried = 0;
      await _pump(
        tester,
        stagePhase: Live2DBridgePhase.error,
        onRetryStage: () => retried++,
      );
      expectIntercepted(find.text('重试'), '错误覆盖层');
      await tester.tap(find.text('重试'));
      expect(retried, 1);
    });
  });

  group('设置面板（三种宿主都在舞台之上）', () {
    testWidgets('expanded 内联侧板有垫层', (WidgetTester tester) async {
      await _pump(tester, width: 1400);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.byType(InlineSettingsDock), findsOneWidget);
      expectIntercepted(find.text('PANE:外观与互动'), '内联侧板内容');
    });

    testWidgets('medium 底部浮层有垫层', (WidgetTester tester) async {
      await _pump(tester, width: 1000);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.text('PANE:外观与互动'), findsOneWidget);
      expectIntercepted(find.text('PANE:外观与互动'), '底部浮层内容');
    });

    testWidgets('compact 整页设置有垫层（2026-09-11 P2-1：不再是抽屉）', (
      WidgetTester tester,
    ) async {
      await _pump(tester, width: 500);
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.byType(ListTile), findsNothing, reason: '抽屉已经不在成品里了');
      expectIntercepted(find.text('PANE:外观与互动'), 'compact 整页设置');
    });
  });
}
