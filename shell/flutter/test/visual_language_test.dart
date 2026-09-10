import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/design/typography.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/audio_bar.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/settings_scaffold.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// **视觉语言契约**（2026-09-11 用户裁决）。
///
/// 用户原话：「整个前端 ui 很 **ai 化同质**，看起来不舒服」、
/// 「尤其是发送，**用上键加圆圈**而不是发送」、「**尽量少用图片用文字做按钮**」。
///
/// 前两条是审美判断，没法自动断言；但它们落在代码上之后会变成几条**具体的、
/// 可回归的**形状约定。这个文件守的就是那几条——不然下一次「顺手改回去」
/// （比如有人觉得齿轮更省地方）不会有任何东西拦住。
void main() {
  Widget wrap(Widget child) =>
      MaterialApp(theme: buildAppTheme(), home: Scaffold(body: child));

  group('组件外观：elevation 一律 0（层级靠描边与面差，不靠投影）', () {
    for (final AppThemeId id in AppThemeId.values) {
      test('$id 下没有一处控件带高度阴影', () {
        final ThemeData t = buildAppTheme(id);
        expect(t.appBarTheme.elevation, 0);
        expect(t.appBarTheme.scrolledUnderElevation, 0);
        expect(t.cardTheme.elevation, 0);
        expect(t.dialogTheme.elevation, 0);
        expect(t.bottomSheetTheme.elevation, 0);
        expect(t.snackBarTheme.elevation, 0);
        // Material 的 surface tint 会在滚动时给顶栏刷一层渐变，也要关掉。
        expect(t.appBarTheme.surfaceTintColor, Colors.transparent);
        expect(t.cardTheme.surfaceTintColor, Colors.transparent);
      });
    }

    test('按钮共用同一个圆角档位（不是每处 styleFrom 各写一个）', () {
      final ThemeData t = buildAppTheme();
      final RoundedRectangleBorder want = RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
      );
      for (final ButtonStyle? style in <ButtonStyle?>[
        t.filledButtonTheme.style,
        t.textButtonTheme.style,
        t.outlinedButtonTheme.style,
        t.elevatedButtonTheme.style,
      ]) {
        expect(
          style?.shape?.resolve(<WidgetState>{}),
          want,
          reason: '有按钮的圆角与其余不一致——那正是「东一块西一块」的来源',
        );
      }
    });

    test('按钮文字用 labelLarge（与被点区域同一字级，不与正文糊在一起）', () {
      final ThemeData t = buildAppTheme();
      final TextStyle? style = t.filledButtonTheme.style?.textStyle?.resolve(
        <WidgetState>{},
      );
      expect(style?.fontSize, AppFontSizes.registry['labelLarge']!.size);
    });
  });

  group('发送键：圆形 + 上箭头（**不是**写着「发送」的按钮）', () {
    Widget panel({required UiPhase phase, VoidCallback? onSend, VoidCallback? onStop}) =>
        ChatPanel(
          messages: const <ChatMessage>[],
          phase: phase,
          input: TextEditingController(),
          onSend: onSend ?? () {},
          onStop: onStop ?? () {},
          onDismissError: () {},
          volume: 1,
          muted: false,
          onVolumeChanged: (_) {},
          onMutedChanged: (_) {},
        );

    testWidgets('空闲态：一个圆形按钮 + 上箭头，且**没有**「发送」二字', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrap(panel(phase: UiPhase.idle)));
      expect(find.byIcon(Icons.arrow_upward_rounded), findsOneWidget);
      expect(
        find.text('发送'),
        findsNothing,
        reason: '用户明确要求「用上键加圆圈而不是发送」',
      );
      // 形状必须是圆：方角/胶囊都不算。
      final IconButton button = tester.widget<IconButton>(
        find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded),
      );
      expect(button.style?.shape?.resolve(<WidgetState>{}), const CircleBorder());
    });

    testWidgets('思考中：同一个位置变成圆形停止键（不跳位、不换形状）', (
      WidgetTester tester,
    ) async {
      int stops = 0;
      await tester.pumpWidget(
        wrap(panel(phase: UiPhase.thinking, onStop: () => stops++)),
      );
      expect(find.byIcon(Icons.stop_rounded), findsOneWidget);
      expect(find.byIcon(Icons.arrow_upward_rounded), findsNothing);
      final IconButton button = tester.widget<IconButton>(
        find.widgetWithIcon(IconButton, Icons.stop_rounded),
      );
      expect(button.style?.shape?.resolve(<WidgetState>{}), const CircleBorder());
      await tester.tap(find.byIcon(Icons.stop_rounded));
      expect(stops, 1);
    });
  });

  group('文字优先：能写字的按钮不画图标', () {
    testWidgets('设置入口是「设置」两个字，不是齿轮', (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        wrap(
          AppShell(
            stage: const SizedBox.shrink(),
            phase: UiPhase.idle,
            wsStatus: WsStatus.connected,
            messages: const <ChatMessage>[],
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onRetryConnection: () {},
            volume: 1,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
            sections: visibleSections(devMode: false),
            sectionBuilder: (BuildContext c, SettingsSection s) =>
                const SizedBox.shrink(),
          ),
        ),
      );
      expect(find.text('设置'), findsOneWidget);
      expect(
        find.byIcon(Icons.settings_outlined),
        findsNothing,
        reason: '齿轮要认图标，「设置」两个字不用',
      );
    });

    testWidgets('左侧导航里**一个图标都没有**（8 个中文标签自己就能分辨）', (
      WidgetTester tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      int picked = 0;
      await tester.pumpWidget(
        wrap(
          SectionRail(
            sections: visibleSections(devMode: true),
            selected: SettingsSection.actions,
            onSelect: (_) => picked++,
          ),
        ),
      );
      expect(find.byType(SectionRail), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(SectionRail),
          matching: find.byType(Icon),
        ),
        findsNothing,
        reason: '用户裁决「尽量少用图片用文字做按钮」',
      );
      // 8 个分区都能点。
      for (final SettingsSection s in visibleSections(devMode: true)) {
        expect(find.text(s.label), findsOneWidget);
      }
      await tester.tap(find.text('诊断'));
      expect(picked, 1);
    });

    testWidgets('本机静音开关是文字（静音 / 已静音）', (WidgetTester tester) async {
      Widget bar(bool muted) => AudioBar(
        volume: 1,
        muted: muted,
        onVolumeChanged: (_) {},
        onMutedChanged: (_) {},
      );
      await tester.pumpWidget(wrap(bar(false)));
      expect(find.widgetWithText(TextButton, '静音'), findsOneWidget);
      await tester.pumpWidget(wrap(bar(true)));
      expect(find.widgetWithText(TextButton, '已静音'), findsOneWidget);
    });

    testWidgets('设置里的分区导航 chip 不带 avatar 图标', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          SettingsScaffold(
            sections: visibleSections(devMode: false),
            selected: SettingsSection.actions,
            onSelect: (_) {},
            child: const SizedBox.shrink(),
          ),
        ),
      );
      final Iterable<ChoiceChip> chips = tester.widgetList<ChoiceChip>(
        find.byType(ChoiceChip),
      );
      expect(chips, isNotEmpty);
      for (final ChoiceChip chip in chips) {
        expect(chip.avatar, isNull, reason: 'chip 前面的小图标是纯噪声');
      }
    });
  });
}
