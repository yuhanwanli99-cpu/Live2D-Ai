import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/inline_notice.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

void main() {
  group('滑杆字段', () {
    testWidgets('显示值、可拖动、上报新值', (WidgetTester tester) async {
      final List<double> seen = <double>[];
      await tester.pumpWidget(
        wrap(
          SliderField(
            label: '模型缩放',
            icon: Icons.zoom_out_map,
            value: 1.0,
            min: 0.5,
            max: 2.0,
            onChanged: seen.add,
          ),
        ),
      );
      expect(find.text('100%'), findsOneWidget);
      await tester.drag(find.byType(Slider), const Offset(60, 0));
      expect(seen, isNotEmpty);
      expect(seen.last, greaterThan(1.0));
    });

    testWidgets('语义值念「标签 + 百分比」（不是裸数字）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          SliderField(
            label: '模型缩放',
            icon: Icons.zoom_out_map,
            value: 1.2,
            min: 0.5,
            max: 2.0,
            onChanged: (_) {},
          ),
        ),
      );
      final Slider slider = tester.widget<Slider>(find.byType(Slider));
      expect(slider.label, '模型缩放');
      expect(slider.semanticFormatterCallback!(1.2), '模型缩放 120%');
    });

    testWidgets('非百分比模式用原始值与后缀', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          SliderField(
            label: '旋转',
            icon: Icons.rotate_right,
            value: 12.5,
            min: 0,
            max: 30,
            percentage: false,
            suffix: '°',
            onChanged: (_) {},
          ),
        ),
      );
      expect(find.text('12.5°'), findsOneWidget);
      final Slider slider = tester.widget<Slider>(find.byType(Slider));
      expect(slider.semanticFormatterCallback!(12.5), '旋转 12.5°');
    });

    testWidgets('enabled=false 时滑杆禁用（但仍显示值）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          SliderField(
            label: '缩放',
            icon: Icons.zoom_out_map,
            value: 1.0,
            min: 0.5,
            max: 2.0,
            onChanged: (_) {},
            enabled: false,
          ),
        ),
      );
      final Slider slider = tester.widget<Slider>(find.byType(Slider));
      expect(slider.onChanged, isNull);
      expect(find.text('100%'), findsOneWidget);
    });
  });

  group('开关字段', () {
    testWidgets('显示标签、可切换、上报布尔', (WidgetTester tester) async {
      final List<bool> seen = <bool>[];
      await tester.pumpWidget(
        wrap(
          ToggleField(
            label: '口型同步',
            icon: Icons.record_voice_over,
            value: true,
            onChanged: seen.add,
          ),
        ),
      );
      expect(find.text('口型同步'), findsOneWidget);
      await tester.tap(find.byType(Switch));
      expect(seen, <bool>[false]);
    });

    testWidgets('语义标签带上字段名（否则读屏只会念「开/关」）', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          ToggleField(
            label: '待机小动作',
            icon: Icons.self_improvement,
            value: false,
            onChanged: (_) {},
          ),
        ),
      );
      expect(find.bySemanticsLabel('待机小动作'), findsWidgets);
      handle.dispose();
    });
  });

  group('文本字段', () {
    testWidgets('输入上报新值', (WidgetTester tester) async {
      final List<String> seen = <String>[];
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'base_url',
            icon: Icons.link,
            value: 'http://a/v1',
            onChanged: seen.add,
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), 'http://b/v1');
      expect(seen, <String>['http://b/v1']);
    });

    testWidgets('外部值变化时同步进输入框（保存后回填 / 放弃改动）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'voice',
            icon: Icons.record_voice_over,
            value: 'alice',
            onChanged: (_) {},
          ),
        ),
      );
      expect(find.text('alice'), findsOneWidget);

      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'voice',
            icon: Icons.record_voice_over,
            value: 'nova',
            onChanged: (_) {},
          ),
        ),
      );
      expect(find.text('nova'), findsOneWidget);
    });

    testWidgets('用户正在编辑时，外部回填**不打断**光标', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'voice',
            icon: Icons.record_voice_over,
            value: 'alice',
            onChanged: (_) {},
          ),
        ),
      );
      // 聚焦并打字（此时 _hasFocus = true）。
      await tester.tap(find.byType(TextField));
      await tester.pump();
      await tester.enterText(find.byType(TextField), 'ali');

      // 父级带着一个「旧」值重建（例如别处的 setState）。
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'voice',
            icon: Icons.record_voice_over,
            value: 'alice',
            onChanged: (_) {},
          ),
        ),
      );
      // 光标还在用户输入的内容上——不能被推回 'alice'。
      final TextField field = tester.widget<TextField>(find.byType(TextField));
      expect(field.controller!.text, 'ali');
    });
  });

  group('数字字段', () {
    testWidgets('合法输入上报', (WidgetTester tester) async {
      final List<int> seen = <int>[];
      await tester.pumpWidget(
        wrap(
          NumberField(
            label: '历史轮数',
            icon: Icons.history,
            value: 3,
            min: 0,
            max: 20,
            onChanged: seen.add,
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), '7');
      expect(seen, <int>[7]);
    });

    testWidgets('越界/非数字**不回调**（防手滑把配置写坏）', (WidgetTester tester) async {
      final List<int> seen = <int>[];
      await tester.pumpWidget(
        wrap(
          NumberField(
            label: '历史轮数',
            icon: Icons.history,
            value: 3,
            min: 0,
            max: 20,
            onChanged: seen.add,
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), '999');
      await tester.enterText(find.byType(TextField), 'abc');
      await tester.enterText(find.byType(TextField), '-5');
      expect(seen, isEmpty, reason: '越界与非法输入不该产生任何改动');
    });

    testWidgets('范围提示写出来（用户不用猜边界）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          NumberField(
            label: '历史轮数',
            icon: Icons.history,
            value: 3,
            min: 0,
            max: 20,
            onChanged: (_) {},
          ),
        ),
      );
      expect(find.text('0 – 20'), findsOneWidget);
    });
  });

  group('分段 / 下拉字段', () {
    testWidgets('分段按钮选中当前值并上报', (WidgetTester tester) async {
      final List<int> seen = <int>[];
      await tester.pumpWidget(
        wrap(
          SegmentedField<int>(
            label: '渲染档位',
            icon: Icons.speed,
            value: 8192,
            options: const <FieldOption<int>>[
              FieldOption<int>(value: 4096, label: '4K'),
              FieldOption<int>(value: 8192, label: '8K'),
              FieldOption<int>(value: 16384, label: '16K'),
            ],
            onChanged: seen.add,
          ),
        ),
      );
      await tester.tap(find.text('16K'));
      expect(seen, <int>[16384]);
    });

    test('分段按钮超过 5 项会被断言拦下（挤到换行就失去意义）', () {
      expect(
        () => SegmentedField<int>(
          label: 'x',
          icon: Icons.speed,
          value: 1,
          options: const <FieldOption<int>>[
            FieldOption<int>(value: 1, label: 'a'),
            FieldOption<int>(value: 2, label: 'b'),
            FieldOption<int>(value: 3, label: 'c'),
            FieldOption<int>(value: 4, label: 'd'),
            FieldOption<int>(value: 5, label: 'e'),
            FieldOption<int>(value: 6, label: 'f'),
          ],
          onChanged: (_) {},
        ),
        throwsA(isA<AssertionError>()),
      );
    });

    testWidgets('下拉字段（项多时）', (WidgetTester tester) async {
      final List<String> seen = <String>[];
      await tester.pumpWidget(
        wrap(
          DropdownField<String>(
            label: '声音',
            icon: Icons.graphic_eq,
            value: 'a',
            options: const <FieldOption<String>>[
              FieldOption<String>(value: 'a', label: '甲'),
              FieldOption<String>(value: 'b', label: '乙'),
            ],
            onChanged: seen.add,
          ),
        ),
      );
      await tester.tap(find.byType(DropdownButton<String>));
      await tester.pumpAndSettle();
      await tester.tap(find.text('乙').last);
      await tester.pumpAndSettle();
      expect(seen, <String>['b']);
    });
  });

  group('只读字段：是文本，不是禁用输入框', () {
    testWidgets('渲染文本且没有输入控件', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          const ReadonlyField(
            label: '采样率',
            icon: Icons.settings_voice,
            text: '24000 Hz · 单声道',
          ),
        ),
      );
      expect(find.text('24000 Hz · 单声道'), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      expect(find.byType(Switch), findsNothing);
    });
  });

  group('错误槽：内联在字段下方，不用 toast', () {
    testWidgets('有错误时显示**图标 + 文案**（不是 `⚠` 字符）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'base_url',
            icon: Icons.link,
            value: '不是URL',
            onChanged: (_) {},
            error: 'base_url 非法',
          ),
        ),
      );
      // 2026-09-11（P1-5）：错误槽改成统一的 `InlineNotice`——
      // 图标是 Material 图标，不再是 `⚠` 这个字符（字形/基线跨平台不一致，
      // 而且它躲过了「图标要能对齐」这件事）。所以判据是「文案 + 图标都在」。
      expect(find.textContaining('base_url 非法'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(InlineNotice),
          matching: find.byIcon(Icons.error_outline),
        ),
        findsOneWidget,
      );
      expect(find.textContaining('⚠'), findsNothing);
    });

    testWidgets('空错误串不显示（避免一个空的提示框）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          TextFieldRow(
            label: 'base_url',
            icon: Icons.link,
            value: 'http://a',
            onChanged: (_) {},
            error: '',
          ),
        ),
      );
      expect(find.textContaining('⚠'), findsNothing);
    });
  });

  group('动作行', () {
    testWidgets('busy 时禁用并改文案', (WidgetTester tester) async {
      int calls = 0;
      await tester.pumpWidget(
        wrap(
          FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试',
            onPressed: () => calls++,
          ),
        ),
      );
      await tester.tap(find.text('测试'));
      expect(calls, 1);

      await tester.pumpWidget(
        wrap(
          FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试',
            onPressed: () => calls++,
            busy: true,
          ),
        ),
      );
      expect(find.text('测试中…'), findsOneWidget);
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.byType(OutlinedButton),
      );
      expect(button.onPressed, isNull, reason: '测试中不能再点（防重复请求）');
    });

    testWidgets('错误结果走错误槽（内联，不是 toast）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          const FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试',
            onPressed: null,
            result: '连不上 http://x',
            resultIsError: true,
          ),
        ),
      );
      expect(find.textContaining('连不上 http://x'), findsOneWidget);
    });
  });
}
