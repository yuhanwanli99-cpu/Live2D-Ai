import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/ui/settings_scaffold.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(SettingsScaffold scaffold) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: scaffold),
);

SettingsScaffold build({
  bool dirty = false,
  bool saving = false,
  String? status,
  bool statusIsError = false,
  VoidCallback? onSave,
  VoidCallback? onDiscard,
  ValueChanged<SettingsSection>? onSelect,
  VoidCallback? onClose,
}) => SettingsScaffold(
  // 本文件测的是脚手架本身（导航 / 底部操作条），用全量分区（含诊断）。
  sections: visibleSections(devMode: true),
  selected: SettingsSection.service,
  onSelect: onSelect ?? (_) {},
  onClose: onClose,
  dirty: dirty,
  saving: saving,
  onSave: onSave,
  onDiscard: onDiscard,
  statusMessage: status,
  statusIsError: statusIsError,
  child: const Text('PANE'),
);

void main() {
  group('未保存提示（规格 §4.3 的硬要求：改动必须有可见提示）', () {
    testWidgets('不 dirty 时既没有「未保存」也没有操作条', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(build()));
      expect(find.text('未保存'), findsNothing);
      expect(find.text('保存'), findsNothing);
      expect(find.text('放弃'), findsNothing);
      expect(find.text('PANE'), findsOneWidget);
    });

    testWidgets('dirty 时出现「未保存」Pill 与保存/放弃按钮', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(build(dirty: true, onSave: () {}, onDiscard: () {})),
      );
      expect(find.text('未保存'), findsOneWidget);
      expect(find.text('保存'), findsOneWidget);
      expect(find.text('放弃'), findsOneWidget);
    });

    testWidgets('点保存/放弃真的回调', (WidgetTester tester) async {
      int saved = 0;
      int discarded = 0;
      await tester.pumpWidget(
        wrap(
          build(
            dirty: true,
            onSave: () => saved++,
            onDiscard: () => discarded++,
          ),
        ),
      );
      await tester.tap(find.text('保存'));
      await tester.tap(find.text('放弃'));
      expect(saved, 1);
      expect(discarded, 1);
    });

    testWidgets('saving 时两个按钮都禁用且文案变「保存中…」（防重复提交）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(build(dirty: true, saving: true, onSave: () {}, onDiscard: () {})),
      );
      expect(find.text('保存中…'), findsOneWidget);
      final FilledButton save = tester.widget<FilledButton>(
        find.byType(FilledButton),
      );
      expect(save.onPressed, isNull);
      final TextButton discard = tester.widget<TextButton>(
        find.widgetWithText(TextButton, '放弃'),
      );
      expect(discard.onPressed, isNull);
    });
  });

  group('保存结果行：**按 apply_status 分流**的文案直接显示在这里', () {
    testWidgets('成功文案显示在操作条里', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(build(status: '已保存并生效')));
      expect(find.text('已保存并生效'), findsOneWidget);
      // 没有改动时不显示保存/放弃按钮，但结果要留着让用户看到。
      expect(find.text('保存'), findsNothing);
    });

    testWidgets('需重启的文案也走这里（用户必须知道没生效）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(build(status: '已保存，需重启生效')));
      expect(find.text('已保存，需重启生效'), findsOneWidget);
    });

    testWidgets('错误文案用 danger 色（statusIsError）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(build(status: '保存失败：url_invalid', statusIsError: true)),
      );
      final Text text = tester.widget<Text>(find.text('保存失败：url_invalid'));
      expect(text.style?.color, isNotNull);
    });
  });

  group('分区导航', () {
    testWidgets('每个分区一个 chip（7 个，含「开发模式」——它不再被藏起来）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(wrap(build()));
      expect(find.byType(ChoiceChip), findsNWidgets(7));
      expect(find.text('开发模式'), findsOneWidget);
    });

    testWidgets('点 chip 上报分区', (WidgetTester tester) async {
      final List<SettingsSection> picked = <SettingsSection>[];
      await tester.pumpWidget(wrap(build(onSelect: picked.add)));
      await tester.tap(find.widgetWithText(ChoiceChip, '模型服务'));
      expect(picked, <SettingsSection>[SettingsSection.service]);
    });

    testWidgets('有 onClose 时显示关闭按钮并回调', (WidgetTester tester) async {
      int closed = 0;
      await tester.pumpWidget(wrap(build(onClose: () => closed++)));
      await tester.tap(find.byIcon(Icons.close));
      expect(closed, 1);
    });

    testWidgets('分区标题与说明来自枚举（单点真相）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(build()));
      expect(find.text('模型服务'), findsWidgets);
      // 2026-10-09：分区说明改成一句短的中文（不再是「服务地址、模型名和密钥」）。
      expect(find.text('语言模型与语音合成'), findsOneWidget);
      // 产品面上不出现「LLM」「TTS」这些内部名。
      expect(find.text('LLM'), findsNothing);
      expect(find.text('TTS'), findsNothing);
    });

    testWidgets('没开开发者模式：导航里没有「诊断」，但「开发模式」还在', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SettingsScaffold(
              sections: visibleSections(),
              selected: SettingsSection.persona,
              onSelect: (_) {},
              child: const Text('PANE'),
            ),
          ),
        ),
      );
      expect(find.byType(ChoiceChip), findsNWidgets(7));
      expect(find.widgetWithText(ChoiceChip, '诊断'), findsNothing);
      expect(find.widgetWithText(ChoiceChip, '开发模式'), findsOneWidget);
      expect(find.widgetWithText(ChoiceChip, '模型服务'), findsOneWidget);
      expect(find.widgetWithText(ChoiceChip, '扩展'), findsOneWidget);
    });
  });
}
