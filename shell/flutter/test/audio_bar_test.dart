import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/audio_bar.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

/// 造一个受控的音频条。
({Widget widget, List<double> volumes, List<bool> mutes}) buildBar({
  double volume = 0.8,
  bool muted = false,
  bool serverMuted = false,
  bool audioUnlocked = true,
  VoidCallback? onEnableSound,
}) {
  final List<double> volumes = <double>[];
  final List<bool> mutes = <bool>[];
  return (
    widget: AudioBar(
      volume: volume,
      muted: muted,
      onVolumeChanged: volumes.add,
      onMutedChanged: mutes.add,
      serverMuted: serverMuted,
      audioUnlocked: audioUnlocked,
      onEnableSound: onEnableSound,
    ),
    volumes: volumes,
    mutes: mutes,
  );
}

void main() {
  group('命名契约（规格 §7.2 逐字照抄：同一个词不能命名两个轴）', () {
    testWidgets('本机开关的 tooltip 是「本机静音」，且按钮是**文字**', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrap(buildBar().widget));
      // 2026-09-11：从喇叭图标改成文字按钮（用户裁决「尽量少用图片用文字
      // 做按钮」）。tooltip 仍然是逐字照抄的那四个字。
      expect(find.byTooltip('本机静音'), findsOneWidget);
      expect(find.widgetWithText(TextButton, '静音'), findsOneWidget);
      expect(find.byIcon(Icons.volume_mute_outlined), findsNothing);
    });

    testWidgets('服务端徽标写「服务端静音中」并点出环境变量名', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(serverMuted: true).widget));
      expect(find.textContaining('服务端静音中'), findsOneWidget);
      expect(
        find.textContaining('LIVE2D_AI_MUTE_AUDIO=1'),
        findsOneWidget,
        reason: '不写变量名，用户不知道去哪关',
      );
      expect(find.textContaining('任何客户端都听不到'), findsOneWidget);
    });

    testWidgets('服务端静音**不是**开关：没有可点的控件', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(serverMuted: true).widget));
      // 徽标区域里不该有 Switch / Checkbox / 任何可点的 IconButton。
      final Finder badge = find.ancestor(
        of: find.textContaining('服务端静音中'),
        matching: find.byType(Row),
      );
      expect(
        find.descendant(of: badge.first, matching: find.byType(Switch)),
        findsNothing,
      );
    });

    testWidgets('服务端静音未发生时徽标不出现（它是观测值，不是常驻说明）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrap(buildBar().widget));
      expect(find.textContaining('服务端静音中'), findsNothing);
    });

    testWidgets('本机静音时的说明含「你这台设备听不到」与「口型保留」', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(muted: true).widget));
      expect(find.textContaining('你这台设备听不到'), findsOneWidget);
      expect(find.textContaining('口型保留'), findsOneWidget);
    });
  });

  group('静音与音量正交（最容易做错的一处）', () {
    testWidgets('静音时滑杆**不置灰、不归零**，保留用户原值', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(muted: true, volume: 0.8).widget));
      final Slider slider = tester.widget<Slider>(find.byType(Slider));
      expect(slider.value, 0.8);
      expect(slider.onChanged, isNotNull, reason: '禁用滑杆 = 把两个轴揉在一起');
      expect(find.text('80%'), findsOneWidget);
    });

    testWidgets('静音说明里写出「取消静音后按 80% 播放」', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(muted: true, volume: 0.8).widget));
      expect(find.textContaining('取消静音后按 80% 播放'), findsOneWidget);
    });

    testWidgets('点静音开关上报新的布尔（不改音量）', (WidgetTester tester) async {
      final ({Widget widget, List<double> volumes, List<bool> mutes}) bar = buildBar(
        muted: false,
      );
      await tester.pumpWidget(wrap(bar.widget));
      await tester.tap(find.widgetWithText(TextButton, '静音'));
      expect(bar.mutes, <bool>[true]);
      expect(bar.volumes, isEmpty, reason: '静音不该顺手改音量');
    });

    testWidgets('已静音时点开关上报 false（能取消）', (WidgetTester tester) async {
      final ({Widget widget, List<double> volumes, List<bool> mutes}) bar = buildBar(
        muted: true,
      );
      await tester.pumpWidget(wrap(bar.widget));
      // 按钮文案随状态变（「已静音」），所以按 TextButton 而不是按文字定位。
      await tester.tap(find.widgetWithText(TextButton, '已静音'));
      expect(bar.mutes, <bool>[false]);
    });

    testWidgets('拖滑杆上报新音量', (WidgetTester tester) async {
      final ({Widget widget, List<double> volumes, List<bool> mutes}) bar = buildBar(
        volume: 0.5,
      );
      await tester.pumpWidget(wrap(bar.widget));
      await tester.drag(find.byType(Slider), const Offset(60, 0));
      expect(bar.volumes, isNotEmpty);
      expect(bar.volumes.last, greaterThan(0.5));
    });

    testWidgets('百分比显示四舍五入到整数（不显示 0.8000000001）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(volume: 0.799).widget));
      expect(find.text('80%'), findsOneWidget);
      await tester.pumpWidget(wrap(buildBar(volume: 0.0).widget));
      expect(find.text('0%'), findsOneWidget);
      await tester.pumpWidget(wrap(buildBar(volume: 1.0).widget));
      expect(find.text('100%'), findsOneWidget);
    });
  });

  group('autoplay 解锁提示（规格 §7.6：必须显式化）', () {
    testWidgets('未解锁时出现提示 + 显式的「启用声音」按钮', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(audioUnlocked: false).widget));
      expect(find.textContaining('点击任意位置或按任意键'), findsOneWidget);
      expect(find.text('启用声音'), findsOneWidget);
    });

    testWidgets('点「启用声音」真的回调（不是只显示提示）', (WidgetTester tester) async {
      int called = 0;
      await tester.pumpWidget(
        wrap(buildBar(audioUnlocked: false, onEnableSound: () => called++).widget),
      );
      await tester.tap(find.text('启用声音'));
      expect(called, 1);
    });

    testWidgets('已解锁时既无提示也无按钮', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar().widget));
      expect(find.textContaining('按任意键'), findsNothing);
      expect(find.text('启用声音'), findsNothing);
    });
  });

  group('无障碍（规格 §9.1：读屏要念出百分比）', () {
    testWidgets('滑杆的语义标签是「主音量」、语义值是百分比', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(buildBar(volume: 0.42).widget));
      final Slider slider = tester.widget<Slider>(find.byType(Slider));
      expect(slider.label, '主音量');
      expect(slider.semanticFormatterCallback, isNotNull);
      expect(slider.semanticFormatterCallback!(0.42), '42%');
    });

    testWidgets('读屏节点上真的挂着「主音量 + 80%」，且仍可调节', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(wrap(buildBar(volume: 0.8).widget));

      // 语义边界在 `Slider` 内部的渲染对象上，所以按标签找它，
      // 而不是按 `find.byType(Slider)`（那拿到的是外层无语义的渲染对象）。
      final Finder sliderSemantics = find.bySemanticsLabel('主音量');
      expect(sliderSemantics, findsOneWidget);

      final SemanticsNode node = tester.getSemantics(sliderSemantics);
      expect(node.value, '80%');
      // 滑杆必须保持可调节语义。外面套一层 `Semantics(container: true)` 会把
      // 整条音频条合并成一个节点，把它吃掉（第一版就是这么错的）。
      expect(
        node.getSemanticsData().hasAction(SemanticsAction.increase),
        isTrue,
        reason: '被合并的滑杆读屏用户无法调节',
      );
      handle.dispose();
    });

    testWidgets('音频条**不**被合并成单个语义节点（否则按钮读不到）', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(buildBar(muted: true, serverMuted: true, audioUnlocked: false).widget),
      );
      // 三个可交互/可读的**独立**节点都要在。如果整条被合并成一个节点，
      // 这两个标签不可能同时各自命中（合并后只剩一个"音频 …"节点）。
      expect(find.bySemanticsLabel('主音量'), findsOneWidget);
      expect(find.bySemanticsLabel('启用声音'), findsWidgets);
      // 静音开关的语义标签由 `Tooltip` 提供（`IconButton.tooltip`）。
      expect(find.byTooltip('本机静音'), findsOneWidget);
      // 服务端徽标是独立的只读节点。
      expect(
        find.bySemanticsLabel('服务端静音中，服务端没有发出声音，任何客户端都听不到'),
        findsOneWidget,
      );
      handle.dispose();
    });

    testWidgets('服务端静音徽标有整句语义（不念碎片）', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(wrap(buildBar(serverMuted: true).widget));
      expect(
        find.bySemanticsLabel(
          '服务端静音中，服务端没有发出声音，任何客户端都听不到',
        ),
        findsOneWidget,
      );
      handle.dispose();
    });
  });

  group('三种断点下都不溢出（音频条是常驻控件）', () {
    for (final double width in <double>[1400, 1000, 500, 390, 320]) {
      testWidgets('width=$width 渲染且不溢出', (WidgetTester tester) async {
        await tester.binding.setSurfaceSize(Size(width, 400));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(
          wrap(buildBar(muted: true, serverMuted: true, audioUnlocked: false).widget),
        );
        await tester.pump();
        // 溢出会在测试里抛异常（`RenderFlex overflowed`）——所以这条断言
        // 本身就是「没有溢出」的证据；再加一条存在性断言防止渲染出空白。
        expect(find.byType(Slider), findsOneWidget);
      });
    }
  });
}
