/// 一级「导演」卡回归（2026-10-17 rc4；原「扩展 → 导演」Mod 卡片）。
///
/// 覆盖：
/// ① **常驻状态卡**：写清「导演常驻开启（无开关）」+ 一句说明，且**没有启用开关**；
/// ② **动作幅度**复用既有滑条与「本模型覆盖」：值 / 回调仍来自宿主接线，不复制第二套；
/// ③ **导演可观测 / 手动调试只在 devMode 为真时渲染**；
/// ④ 八个隐藏键（词表映射 + staging_*）在 lib/ **零出现**；两个**用户看得懂**
///    的动作偏好键（motion_bias / motion_rate）只允许出现在导演卡的两条下拉里
///    （0.4.4-rc.2，2026-10-23）——扩展区仍然没有导演卡；
/// ⑤ 展示纯函数（情绪 / 意图 / 预设 / 数值文案）随选项卡搬到一级卡文件后仍可用；
/// ⑥ 表演倾向 / 动作频率两个下拉：有 wiring 才画、各三档、改选只回调宿主（草稿）。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/director_motion_draft.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panel.dart';
import 'package:live2d_ai_shell/settings/mods/mod_panels.dart';
import 'package:live2d_ai_shell/settings/sections/director_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart'
    show DropdownField, FieldOption, SliderField, ToggleField;
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
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

/// 导演 Mod 曾经的**八个隐藏键**（rc4 起产品界面一个都不画；0.4.4-rc.2 也不）。
///
/// 「motion_bias」曾在 0.4.4-rc.2 **放回界面**（与新的「motion_rate」一起成为
/// 导演卡上仅有的两个下拉），所以它**不在**这张退役表里。
const List<String> _retiredDirectorKeys = <String>[
  'preset_happy',
  'preset_sad',
  'preset_greeting',
  'staging_enabled',
  'staging_base_url',
  'staging_model',
  'staging_api_key_env',
  'staging_timeout_ms',
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

  group('④ 八个隐藏键仍然零出现（没有第二套配置表单）', () {
    test('扩展区面板注册表里没有导演', () {
      expect(
        kModPanels.map((ModPanel p) => p.modId),
        isNot(contains('director')),
        reason: '导演已升为一级分区：扩展区不再有导演卡',
      );
      expect(modPanelFor('director'), isNull);
    });

    test('lib/ 源码里八个隐藏键零出现（没有任何界面能渲染它们）', () {
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
      expect(hits, isEmpty, reason: '导演隐藏键仍有界面声明：$hits');
    });

    test('两个动作偏好键只在草稿文件里（允许出现，不误判成回潮）', () {
      final String draft = stripCommentsKeepStrings(
        File('lib/settings/director_motion_draft.dart').readAsStringSync(),
      );
      expect(draft.contains("'motion_bias'"), isTrue);
      expect(draft.contains("'motion_rate'"), isTrue);
      // 八个隐藏键在同一个文件里仍然零出现（同一份词法，不靠人眼）。
      for (final String key in _retiredDirectorKeys) {
        expect(draft.contains("'$key'"), isFalse, reason: '草稿文件不得声明隐藏键「$key」');
      }
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
      // 表演倾向 / 动作频率**不再**在这张禁止表里（0.4.4-rc.2 是产品下拉）；
      // 旧词表映射与「高级」折叠仍然不得回来。
      for (final String label in <String>[
        '开心 → 动作',
        '难过 → 动作',
        '打招呼 → 动作',
        '高级',
      ]) {
        expect(find.text(label), findsNothing, reason: '导演卡不得出现「$label」');
      }
    });
  });

  group('⑥ 表演倾向 / 动作频率两个下拉（0.4.4-rc.2）', () {
    test('两个标签逐字钉死（括号里的「保存并重载后生效」是承诺）', () {
      expect(kDirectorMotionBiasLabel, '表演倾向（保存并重载后生效）');
      expect(kDirectorMotionRateLabel, '动作频率（保存并重载后生效）');
    });

    testWidgets('有 wiring：两个下拉都在、各三档、值来自 wiring', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          DirectorSection(
            motion: DirectorMotionWiring(
              bias: 'eager',
              rate: 'dense',
              onBiasChanged: (String _) {},
              onRateChanged: (String _) {},
            ),
            actionScales: _scales(),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text(kDirectorMotionBiasLabel), findsOneWidget);
      expect(find.text(kDirectorMotionRateLabel), findsOneWidget);

      final DropdownField<String> bias = tester.widget(
        find.widgetWithText(DropdownField<String>, kDirectorMotionBiasLabel),
      );
      expect(bias.value, 'eager');
      expect(
        bias.options.map((FieldOption<String> o) => o.value).toList(),
        <String>['restrained', 'normal', 'eager'],
      );
      expect(
        bias.options.map((FieldOption<String> o) => o.label).toList(),
        <String>['克制', '平常', '主动'],
      );

      final DropdownField<String> rate = tester.widget(
        find.widgetWithText(DropdownField<String>, kDirectorMotionRateLabel),
      );
      expect(rate.value, 'dense');
      expect(
        rate.options.map((FieldOption<String> o) => o.value).toList(),
        <String>['sparse', 'normal', 'dense'],
      );
      expect(
        rate.options.map((FieldOption<String> o) => o.label).toList(),
        <String>['少', '平常', '多'],
      );
    });

    testWidgets('位置：状态句下面、动作幅度上面', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          DirectorSection(
            motion: DirectorMotionWiring(
              bias: 'normal',
              rate: 'normal',
              onBiasChanged: (String _) {},
              onRateChanged: (String _) {},
            ),
            actionScales: _scales(),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final double statusY = tester
          .getTopLeft(find.text(kDirectorAlwaysOnTitle))
          .dy;
      final double biasY = tester
          .getTopLeft(find.text(kDirectorMotionBiasLabel))
          .dy;
      final double rateY = tester
          .getTopLeft(find.text(kDirectorMotionRateLabel))
          .dy;
      final double scalesY = tester.getTopLeft(find.text('动作幅度')).dy;
      expect(statusY, lessThan(biasY));
      expect(biasY, lessThan(rateY));
      expect(rateY, lessThan(scalesY));
    });

    testWidgets('没有 wiring：整块不画（纯 widget 测试不受影响）', (WidgetTester tester) async {
      await tester.pumpWidget(_wrap(const DirectorSection()));
      await tester.pumpAndSettle();
      expect(find.byType(DropdownField<String>), findsNothing);
      expect(find.text(kDirectorMotionBiasLabel), findsNothing);
      expect(find.text(kDirectorMotionRateLabel), findsNothing);
    });

    testWidgets('改选只回调宿主：草稿变脏、payload 一次带两键、值回流', (WidgetTester tester) async {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      await tester.pumpWidget(
        _wrap(
          ListenableBuilder(
            listenable: draft,
            builder: (BuildContext context, Widget? _) => DirectorSection(
              motion: DirectorMotionWiring(
                bias: draft.bias,
                rate: draft.rate,
                onBiasChanged: draft.selectBias,
                onRateChanged: draft.selectRate,
              ),
            ),
          ),
        ),
      );
      expect(draft.dirty, isFalse);

      DropdownButton<String> buttonFor(String label) =>
          tester.widget<DropdownButton<String>>(
            find.descendant(
              of: find.widgetWithText(DropdownField<String>, label),
              matching: find.byType(DropdownButton<String>),
            ),
          );

      // 直接调下拉的 onChanged：测的是路由（菜单的像素几何另有 field_row 测试）。
      buttonFor(kDirectorMotionBiasLabel).onChanged!('eager');
      await tester.pump();
      expect(draft.bias, 'eager');
      expect(draft.dirty, isTrue, reason: '改选必须只进草稿（不直接写盘）');

      buttonFor(kDirectorMotionRateLabel).onChanged!('dense');
      await tester.pump();
      expect(draft.payload(), <String, Object?>{
        'motion_bias': 'eager',
        'motion_rate': 'dense',
      });

      // 值与草稿同源：宿主用 draft 回流后，控件显示新档。
      final DropdownField<String> bias = tester.widget(
        find.widgetWithText(DropdownField<String>, kDirectorMotionBiasLabel),
      );
      expect(bias.value, 'eager');
    });

    // 为什么没有「保存失败 SnackBar 真泵」的页面级测试：保存提示函数住在
    // part of main.dart 里，而 main.dart 这个库经 app/browser_io.dart 依赖
    // package:web，在 Dart VM 的 flutter test 里加载不了（见
    // test/settings_discipline_test.dart 的 E 组说明）。所以用源码守卫钉住
    // 三条接线：成功结局也把草稿错误拼进 SnackBar、换 warning 底色、并且
    // **不刷新**（刷新会连错误和内存里的草稿一起抹掉）。
    test('源码守卫：导演保存失败时 SnackBar 可见且不刷新', () {
      final String main = readLibrarySource('lib/main.dart');
      final String? body = closureBodyAfter(main, 'void _showSaveSnack');
      expect(body, isNotNull, reason: '找不到 _showSaveSnack 的闭包体');
      expect(body!.contains('outcome.isSuccess'), isTrue);
      expect(body.contains('_modDraftSaveError'), isTrue);
      expect(body.contains('showDraftError'), isTrue);
      expect(body.contains('_directorMotion.dirty'), isTrue);
      expect(
        body.contains('needsAttention || showDraftError'),
        isTrue,
        reason: '成功结局带草稿错误时必须换 warning 底色',
      );

      final String? reload = closureBodyAfter(
        main,
        'Future<void> _reloadPageAfterSave',
      );
      expect(reload, isNotNull, reason: '找不到 _reloadPageAfterSave 的闭包体');
      expect(
        reload!.contains('_directorMotion.dirty'),
        isTrue,
        reason: '导演草稿没保存成功时不得刷新（否则错误与草稿一起消失）',
      );
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
