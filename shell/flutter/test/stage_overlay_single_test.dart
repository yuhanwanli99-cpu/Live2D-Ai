import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/ui/stage_host.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

// ─────────────────────────────────────────────────────────────────────────────
// 舞台**只有一个**加载/错误覆盖层（2026-09-11 前端加强计划 P0-2）
//
// 修之前：`StageHost` 画一套（「模型加载中 N%」幕布 + 「模型加载失败 + 重试」），
// `Live2DStage` 自己又画一套（「渲染面加载中 N%」徽标 + 「渲染面错误 + 重试」）。
// 两套**叠在舞台同一块区域**上，于是：
//
//   - 错误时用户看到**两个「重试」按钮**（点了哪一个都能重建，但看起来像坏了）；
//   - 加载时看到两条**措辞不同**的进度（「渲染面加载中」与「模型加载中」）。
//
// 为什么当时全绿的 627 条测试抓不到：所有布局测试都往 `StageHost` 里注入
// **桩 stage**（`app_shell_layout_test.dart` 的 `PANE:…` 手法），而
// `semantics_test.dart` 也是单独组合 `StageHost` —— **没有任何一条测试
// 把真的 `Live2DStage` 放进 `StageHost`**。本文件补的就是这个组合。
//
// 断言方式刻意选「数出现在屏幕上的东西」而不是「数源码里的类名」：
// 前者与用户看到的一致，后者会在重命名后静默失效。
// ─────────────────────────────────────────────────────────────────────────────

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

/// 把**真的** `Live2DStage` 放进 `StageHost`（这正是 `main.dart` 的接法）。
Widget realStageHost({
  required Live2DBridgePhase phase,
  String? errorMessage,
  VoidCallback? onRetry,
}) => wrap(
  StageHost(
    stage: const Live2DStage(key: ValueKey<String>('real-stage')),
    phase: phase,
    errorMessage: errorMessage,
    onRetry: onRetry,
  ),
);

void main() {
  group('P0-2：舞台加载/错误覆盖层全局唯一', () {
    testWidgets('error 态下舞台上**只有一个**「重试」按钮', (WidgetTester tester) async {
      int retried = 0;
      await tester.pumpWidget(
        realStageHost(
          phase: Live2DBridgePhase.error,
          errorMessage: '渲染面初始化失败',
          onRetry: () => retried++,
        ),
      );
      await tester.pump();

      expect(
        find.text('重试'),
        findsOneWidget,
        reason: '两套覆盖层会给出两个重试按钮——用户会以为界面坏了',
      );
      // 那个唯一的按钮必须真的能用。
      await tester.tap(find.text('重试'));
      expect(retried, 1);
    });

    testWidgets('error 态下只有一句失败标题（不出现「渲染面错误」这套措辞）', (WidgetTester tester) async {
      await tester.pumpWidget(
        realStageHost(
          phase: Live2DBridgePhase.error,
          errorMessage: '渲染面初始化失败',
        ),
      );
      await tester.pump();

      expect(find.text('模型加载失败'), findsOneWidget);
      expect(
        find.text('渲染面错误'),
        findsNothing,
        reason: 'Live2DStage 不该再自带一份错误条',
      );
    });

    testWidgets('loading 态下只有一条进度文案（不出现「渲染面加载中」这套措辞）', (
      WidgetTester tester,
    ) async {
      // 用 `pump` 而不是 `pumpAndSettle`：不确定进度的进度条是无限动画。
      await tester.pumpWidget(realStageHost(phase: Live2DBridgePhase.loading));
      await tester.pump();

      expect(find.textContaining('模型加载中'), findsOneWidget);
      expect(
        find.textContaining('渲染面加载中'),
        findsNothing,
        reason: '同一个加载态两条不同措辞的进度，用户不知道信哪个',
      );
    });

    testWidgets('ready 态下两种覆盖层都不在（覆盖层是叠上去的，不是替换舞台）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(realStageHost(phase: Live2DBridgePhase.ready));
      await tester.pump();

      expect(find.textContaining('加载中'), findsNothing);
      expect(find.text('模型加载失败'), findsNothing);
      // 舞台本体必须还在（保活约束：覆盖层从不卸载下层）。
      expect(find.byType(Live2DStage), findsOneWidget);
    });
  });

  group('P0-2：装饰徽标不带自己的定位', () {
    test('FPS 徽标不再自带 `Align`（否则 Positioned 的 right/top 是假的）', () {
      // 这条防的是「写完 `Positioned(right: 12, top: 12)` 却渲染在左上角」：
      // 内层 `Align` 会撑满 Positioned 给的松弛约束，于是调用方的定位失效。
      // 徽标现在只画药丸本身，定位交给调用方。
      final String src = File(
        'lib/live2d/live2d_stage.dart',
      ).readAsStringSync();
      final int badge = src.indexOf('class _StageBadge');
      expect(badge, greaterThan(0), reason: '找不到 _StageBadge，测试需要跟着改');
      final String body = src.substring(badge);
      expect(
        body.contains('Align('),
        isFalse,
        reason:
            '_StageBadge 内部又出现了 Align —— '
            '调用方的 Positioned(right/top) 会被它悄悄吃掉',
      );
    });
  });
}
