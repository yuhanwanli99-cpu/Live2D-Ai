/// 一级「导演」卡回归（2026-10-17 rc4；原「扩展 → 导演」Mod 卡片）。
///
/// 覆盖：
/// ① **常驻状态卡**：写清「导演常驻开启（无开关）」+ 一句说明，且**没有启用开关**；
/// ② **动作幅度**复用既有滑条与「本模型覆盖」：值 / 回调仍来自宿主接线，不复制第二套；
/// ③ **导演可观测 / 手动调试只在 devMode 为真时渲染**；
/// ④ 八个隐藏键与 motion_bias 在 lib/ **零出现**——导演不再是 Mod，产品面
///    没有任何导演配置表单（扩展区也没有导演卡）；
/// ⑤ 展示纯函数（情绪 / 意图 / 预设 / 数值文案）随选项卡搬到一级卡文件后仍可用。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panels.dart';
import 'package:live2d_ai_shell/settings/sections/director_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart' show SliderField, ToggleField;
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

const ActionSettingsView _action = ActionSettingsView(
  headScale: 0.75,
  bodyScale: 0.80,
  expressionScale: 1.0,
  activeModelId: 'bai',
);

ActionScalesWiring _scales({
  ValueChanged<double>? onHead,
  ValueChanged<double>? onModelHead,
}) => ActionScalesWiring(
  action: _action,
  onHeadScaleChanged: onHead,
  onModelHeadScaleChanged: onModelHead,
);

double _sliderValue(WidgetTester tester, String label) =>
    tester.widget<SliderField>(find.widgetWithText(SliderField, label)).value;

/// 导演 Mod 曾经的配置键（rc4 起产品界面**一个都不画**）。
const List<String> _retiredDirectorKeys = <String>[
  'preset_happy',
  'preset_sad',
  'preset_greeting',
  'staging_enabled',
  'staging_base_url',
  'staging_model',
  'staging_api_key_env',
  'staging_timeout_ms',
  'motion_bias',
];

void main() {
  group('① 常驻状态卡：没有开关', () {
    testWidgets('状态标题 + 一句说明；全卡没有 Switch（无启用开关）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_wrap(const DirectorSection()));
      await tester.pumpAndSettle();

      expect(find.text(kDirectorAlwaysOnTitle), findsOneWidget);
      expect(find.text('导演常驻开启（无开关）'), findsOneWidget);
      expect(find.text(kDirectorAlwaysOnNotice), findsOneWidget);
      // **没有**导演启用开关：没接线时整张卡一个 Switch 都没有。
      expect(find.byType(Switch), findsNothing);
      // 旧卡上的「打开后……」说法随开关一起下架（常驻就不是「打开后」）。
      expect(find.textContaining('打开后'), findsNothing);
    });

    testWidgets('动作幅度接线缺失时不假装有滑条', (WidgetTester tester) async {
      await tester.pumpWidget(_wrap(const DirectorSection()));
      expect(find.text('动作幅度'), findsNothing);
      expect(find.byType(SliderField), findsNothing);
    });
  });

  group('② 动作幅度：复用既有滑条 / 本模型覆盖', () {
    testWidgets('三条滑条渲染宿主给的值；本模型覆盖是唯一的 Switch', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_wrap(DirectorSection(actionScales: _scales())));
      await tester.pumpAndSettle();

      expect(find.text('动作幅度'), findsOneWidget);
      expect(_sliderValue(tester, '头部摆幅'), 0.75);
      expect(_sliderValue(tester, '身体摆幅'), 0.80);
      expect(_sliderValue(tester, '表情幅度'), 1.0);
      // 「本模型覆盖」是既有控件（阶段5 D40），不是导演启用开关。
      expect(find.widgetWithText(ToggleField, '本模型覆盖'), findsOneWidget);
      final Finder overrideSwitch = find.descendant(
        of: find.widgetWithText(ToggleField, '本模型覆盖'),
        matching: find.byType(Switch),
      );
      expect(overrideSwitch, findsOneWidget);
      expect(find.byType(Switch), findsOneWidget, reason: '导演本身没有启用开关');
    });

    testWidgets('滑条回调仍走宿主（草稿 / 本模型覆盖两条既有路由）', (
      WidgetTester tester,
    ) async {
      final List<double> globalHeads = <double>[];
      final List<double> modelHeads = <double>[];
      await tester.pumpWidget(
        _wrap(
          DirectorSection(
            actionScales: _scales(
              onHead: globalHeads.add,
              onModelHead: modelHeads.add,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final Slider slider = tester.widget<Slider>(
        find.descendant(
          of: find.widgetWithText(SliderField, '头部摆幅'),
          matching: find.byType(Slider),
        ),
      );
      slider.onChanged!(1.5);
      expect(globalHeads, <double>[1.5], reason: '覆盖关闭 = 全局草稿路');
      expect(modelHeads, isEmpty);
    });
  });

  group('③ devMode 才渲染可观测 / 调试', () {
    testWidgets('devMode=false：两块都不在语义树', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          const DirectorSection(
            devMode: false,
            observer: Text('OBSERVER-PROBE'),
            directorDebug: DirectorDebugWiring(),
          ),
        ),
      );
      expect(find.text('OBSERVER-PROBE'), findsNothing);
      expect(find.text('手动调试'), findsNothing);
      expect(find.text('导演可观测'), findsNothing);
    });

    testWidgets('devMode=true：可观测面 + 手动调试都在', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          const DirectorSection(
            devMode: true,
            observer: Text('OBSERVER-PROBE'),
            directorDebug: DirectorDebugWiring(),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('OBSERVER-PROBE'), findsOneWidget);
      expect(find.text('手动调试'), findsOneWidget);
    });
  });

  group('④ 产品面没有任何导演配置表单', () {
    test('扩展区面板注册表里没有导演', () {
      expect(
        kModPanels.map((ModPanel p) => p.modId),
        isNot(contains('director')),
        reason: '导演已升为一级分区：扩展区不再有导演卡',
      );
      expect(modPanelFor('director'), isNull);
    });

    test('lib/ 源码里这些键零出现（没有任何界面能渲染它们）', () {
      final List<String> hits = <String>[];
      for (final File f in Directory('lib')
          .listSync(recursive: true)
          .whereType<File>()) {
        if (!f.path.endsWith('.dart')) continue;
        // 只保留会被引用的字符串，注释里提到旧键名不算。
        final String src = stripCommentsKeepStrings(f.readAsStringSync());
        for (final String key in _retiredDirectorKeys) {
          if (src.contains("'$key'") || src.contains('"$key"')) {
            hits.add('${f.path} -> $key');
          }
        }
      }
      expect(hits, isEmpty, reason: '导演配置键仍有界面声明：$hits');
    });

    testWidgets('一级导演卡上也不渲染这些键的旧标签', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          const DirectorSection(
            devMode: true,
            observer: Text('OBSERVER-PROBE'),
            directorDebug: DirectorDebugWiring(),
          ),
        ),
      );
      await tester.pumpAndSettle();
      for (final String label in <String>[
        '开心 → 动作',
        '难过 → 动作',
        '打招呼 → 动作',
        '表演倾向（保存并重载后生效）',
        '高级',
      ]) {
        expect(find.text(label), findsNothing, reason: '导演卡不得出现「$label」');
      }
    });
  });

  group('⑤ 展示纯函数仍供开发者面使用', () {
    test('预设 / 情绪 / 意图 / 数值文案', () {
      expect(directorPresetText('unhappy'), '表情（unhappy）');
      expect(directorPresetText('tilt_left'), '短动作（tilt_left）');
      expect(directorPresetText('none'), '不投递（本轮无动作）');
      expect(directorPresetText(null), '不投递（本轮无动作）');
      expect(directorEmotionText('happy'), '开心（happy）');
      expect(directorEmotionText('bogus'), 'bogus');
      expect(directorIntentText('greeting'), '打招呼（greeting）');
      expect(directorNumberText(1), '1');
      expect(directorNumberText(1.2), '1.20');
      expect(directorNumberText(null), '—');
    });
  });
}
