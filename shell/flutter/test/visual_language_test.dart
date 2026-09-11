import 'dart:io';

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
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/section_header.dart';
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
            sections: visibleSections(),
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

    testWidgets('分区导航是**文字 chip**，没有图标 avatar（8 个中文标签自带辨识度）', (
      WidgetTester tester,
    ) async {
      // 2026-09-11：左侧 rail 已删（用户裁决「把左边的这些设置一级选项去掉
      // 留给舞台」），分区导航只剩面板里这一行 chip —— 约定照旧适用于它。
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      int picked = 0;
      await tester.pumpWidget(
        wrap(
          SettingsScaffold(
            sections: visibleSections(),
            selected: SettingsSection.appearance,
            onSelect: (_) => picked++,
            child: const Text('PANE'),
          ),
        ),
      );
      expect(find.byType(ChoiceChip), findsNWidgets(8));
      for (final SettingsSection s in visibleSections()) {
        final ChoiceChip chip = tester.widget<ChoiceChip>(
          find.widgetWithText(ChoiceChip, s.label),
        );
        expect(chip.avatar, isNull, reason: '用户裁决「尽量少用图片用文字做按钮」');
      }
      // 8 个分区都能点。
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
            sections: visibleSections(),
            selected: SettingsSection.appearance,
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

  // ─────────────────────────────────────────────────────────────
  // 文案里的 Markdown 标记必须被**渲染**，不能露给用户看
  // ─────────────────────────────────────────────────────────────
  //
  // 2026-09-11 在真浏览器里看到：设置面板上到处是字面的 `**`
  //（`**只影响本机显示**`）——文案按 Markdown 写，而 `Text` 不认它。
  group('说明文案不露 Markdown 星号', () {
    testWidgets('SectionHeader 的 description 渲染成粗体而不是两颗星', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(
          const SectionHeader(
            title: '外观',
            description: '四套配色任选。**只影响本机显示**，选完立刻生效。',
          ),
        ),
      );
      final Iterable<Text> texts = tester.widgetList<Text>(find.byType(Text));
      for (final Text t in texts) {
        final InlineSpan? span = t.textSpan;
        final String plain = span == null
            ? (t.data ?? '')
            : span.toPlainText();
        expect(
          plain.contains('**'),
          isFalse,
          reason: '说明文案里漏出了 Markdown 星号：$plain',
        );
      }
      expect(find.textContaining('只影响本机显示'), findsOneWidget);
    });

    testWidgets('字段说明（FieldRow）同样不露星号', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          ToggleField(
            label: '口型同步',
            icon: Icons.graphic_eq,
            value: true,
            onChanged: (_) {},
            description: '关掉后声音照放，但**嘴不动**',
          ),
        ),
      );
      final Iterable<Text> texts = tester.widgetList<Text>(find.byType(Text));
      for (final Text t in texts) {
        final String plain = t.textSpan?.toPlainText() ?? (t.data ?? '');
        expect(plain.contains('**'), isFalse, reason: '漏出星号：$plain');
      }
    });
  });

  // ─────────────────────────────────────────────────────────────
  // 源码级卫生：`**强调**` 只能交给 EmphasizedText 画
  // ─────────────────────────────────────────────────────────────
  //
  // 上一组测的是「EmphasizedText 画得对」；这一组防的是**有人又用裸 `Text`
  // 写了一段带 `**` 的文案**——那正是 2026-09-11 在浏览器里看到的现象
  //（`_StageImageView` 的静态说明就是漏网的那一处）。
  //
  // 扫描是**窗口式**的：`Text(` 的参数经常跨好几行，只比同一行会漏。
  // 判据本身由下面的合成用例自证（不然「扫描器什么都查不出来」也会显示绿）。
  group('源码卫生：不带 EmphasizedText 的 Text 不许出现 `**`', () {
    /// 找出「裸 `Text(...)` 的**实参**里出现 `**`」的位置。
    ///
    /// 按**括号配对**取这次调用的完整实参，不是按行窗口——`Text(` 的参数经常
    /// 跨行，而按行取窗口会把隔壁 `EmphasizedText(...)` 的文案误算进来
    /// （第一版就是这么误报的）。
    /// 跳过字符串字面量里的括号，否则 `Text('a)b')` 会破坏配对。
    List<String> findBareBold(String source, {String label = ''}) {
      final List<String> hits = <String>[];
      // `(?<![A-Za-z_])` 很关键：`EmphasizedText(` 的结尾也是 `Text(`。
      final RegExp re = RegExp(r'(?<![A-Za-z_])Text\(');
      for (final RegExpMatch m in re.allMatches(source)) {
        int depth = 0;
        int i = m.end - 1;
        String? quote;
        for (; i < source.length; i++) {
          final String ch = source[i];
          if (quote != null) {
            if (ch == r'\') {
              i++;
            } else if (ch == quote) {
              quote = null;
            }
            continue;
          }
          if (ch == "'" || ch == '"') {
            quote = ch;
          } else if (ch == '(') {
            depth++;
          } else if (ch == ')') {
            depth--;
            if (depth == 0) {
              i++;
              break;
            }
          }
        }
        final String call = source.substring(m.start, i);
        // 注释里的 `**` 不算（说明「语法长这样」是正常的）。
        final String code = call
            .split('\n')
            .where((String l) => !l.trimLeft().startsWith('//'))
            .join('\n');
        if (RegExp(r'\*\*').hasMatch(code)) {
          final int line = source.substring(0, m.start).split('\n').length;
          hits.add('$label$line  ${call.split('\n').first.trim()}');
        }
      }
      return hits;
    }

    test('扫描器能抓到违规（合成样例自证）', () {
      const String bad = """
      children: <Widget>[
        Text(
          '背景图**盖在纯色底上**：有图时看得到图。',
          style: null,
        ),
      ],""";
      expect(findBareBold(bad), isNotEmpty, reason: '扫描器连合成违规都抓不到');
      const String good = """
      children: <Widget>[
        EmphasizedText(
          '背景图**盖在纯色底上**：有图时看得到图。',
          style: null,
        ),
      ],""";
      expect(findBareBold(good), isEmpty, reason: 'EmphasizedText 是合法用法');
      // 注释里的 `**` 不算。
      const String commented = """
      // 这里说 **强调** 的语法
      Text('普通文案'),
      """;
      expect(findBareBold(commented), isEmpty);
    });

    test('lib/ 里没有违规', () {
      final List<String> hits = <String>[];
      for (final FileSystemEntity e
          in Directory('lib').listSync(recursive: true)) {
        if (e is! File || !e.path.endsWith('.dart')) continue;
        // 渲染器自己当然有 `**`。
        if (e.path.endsWith('ui/emphasized_text.dart')) continue;
        hits.addAll(
          findBareBold(e.readAsStringSync(), label: '${e.path}:'),
        );
      }
      expect(
        hits,
        isEmpty,
        reason: '这些地方会用裸 Text 把 `**` 画给用户看——'
            '改用 EmphasizedText：\n${hits.join('\n')}',
      );
    });
  });
}
