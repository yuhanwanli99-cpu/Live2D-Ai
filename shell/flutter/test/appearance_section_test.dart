/// W5e（阶段5 D40）：外观与互动 →「动作幅度」的**本模型覆盖** UI 回归。
///
/// 覆盖四条最容易悄悄坏掉的接线：
/// 1. `active_model_id` 为空时**如实说「未识别当前模型」并禁用开关**；
/// 2. 覆盖**关闭**时三条滑条仍是全局值、仍走草稿回调（旧语义不动）；
/// 3. 覆盖**开启**时三条滑条是逐键 `override[key] ?? global[key]` 的有效值，
///    且只触发本模型回调（不会误写全局草稿）；
/// 4. 「恢复跟随全局」只在覆盖开启时可点，点了触发删除回调。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart' show SliderField, ToggleField;
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 三条滑条共用一个标签集（覆盖开关只切换它们的**语义**，不新增滑条）。
double _sliderValue(WidgetTester tester, String label) =>
    tester.widget<SliderField>(find.widgetWithText(SliderField, label)).value;

/// 直接调 Slider 的 onChanged（避开拖拽几何，测的是路由而不是像素）。
void _drive(WidgetTester tester, String label, double value) {
  final Slider slider = tester.widget<Slider>(
    find.descendant(
      of: find.widgetWithText(SliderField, label),
      matching: find.byType(Slider),
    ),
  );
  slider.onChanged!(value);
}

Widget _section({
  required ActionSettingsView action,
  bool modelOverrideEnabled = false,
  ValueChanged<bool>? onEnabled,
  ValueChanged<double>? onGlobalHead,
  ValueChanged<double>? onGlobalBody,
  ValueChanged<double>? onGlobalExpression,
  ValueChanged<double>? onModelHead,
  ValueChanged<double>? onModelBody,
  ValueChanged<double>? onModelExpression,
  VoidCallback? onReset,
}) => AppearanceSection(
  prefs: const DisplayPrefs(),
  onPrefsChanged: (DisplayPrefs _) {},
  action: action,
  onHeadScaleChanged: onGlobalHead,
  onBodyScaleChanged: onGlobalBody,
  onExpressionScaleChanged: onGlobalExpression,
  modelOverrideEnabled: modelOverrideEnabled,
  onModelOverrideEnabledChanged: onEnabled,
  onModelHeadScaleChanged: onModelHead,
  onModelBodyScaleChanged: onModelBody,
  onModelExpressionScaleChanged: onModelExpression,
  onResetModelOverride: onReset,
);

const ActionSettingsView _globalBai = ActionSettingsView(
  headScale: 0.75,
  bodyScale: 0.80,
  expressionScale: 1.0,
  activeModelId: 'bai',
);

/// 本模型只覆盖 head（body / expression 必须逐键回落全局）。
const ActionSettingsView _baiHeadOnly = ActionSettingsView(
  headScale: 0.75,
  bodyScale: 0.80,
  expressionScale: 1.0,
  activeModelId: 'bai',
  models: <String, ActionModelOverrideView>{
    'bai': ActionModelOverrideView(
      headScale: 1.2,
      bodyScale: null,
      expressionScale: null,
    ),
  },
);

void main() {
  testWidgets('未识别当前模型：如实写 + 开关禁用（不假装能开）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(
        _section(
          action: const ActionSettingsView(
            headScale: 0.75,
            bodyScale: 0.80,
            expressionScale: 1.0,
          ),
          onEnabled: (bool _) {},
        ),
      ),
    );
    expect(find.text('未识别当前模型'), findsOneWidget);
    expect(
      tester
          .widget<ToggleField>(find.widgetWithText(ToggleField, '本模型覆盖'))
          .enabled,
      isFalse,
      reason: '模型 id 未知时必须禁用开关——开了也会被服务端按非法 id 拒',
    );
  });

  testWidgets('覆盖关闭：滑条是全局值，改的是草稿回调（旧语义不动）', (WidgetTester tester) async {
    final List<double> globalHeads = <double>[];
    final List<double> modelHeads = <double>[];
    await tester.pumpWidget(
      _wrap(
        _section(
          action: _globalBai,
          onGlobalHead: globalHeads.add,
          onModelHead: modelHeads.add,
        ),
      ),
    );
    expect(_sliderValue(tester, '头部摆幅'), 0.75);
    expect(_sliderValue(tester, '身体摆幅'), 0.80);
    _drive(tester, '头部摆幅', 1.5);
    expect(globalHeads, <double>[1.5], reason: '关闭覆盖 = 全局草稿路');
    expect(modelHeads, isEmpty, reason: '关闭覆盖时不得触发本模型 PATCH');
  });

  testWidgets('覆盖开启：逐键有效值（head 覆盖、body/expr 回落全局）', (WidgetTester tester) async {
    final List<double> globalHeads = <double>[];
    final List<double> modelHeads = <double>[];
    await tester.pumpWidget(
      _wrap(
        _section(
          action: _baiHeadOnly,
          modelOverrideEnabled: true,
          onGlobalHead: globalHeads.add,
          onModelHead: modelHeads.add,
        ),
      ),
    );
    expect(_sliderValue(tester, '头部摆幅'), 1.2, reason: 'head 用覆盖值');
    expect(_sliderValue(tester, '身体摆幅'), 0.80, reason: 'body 未覆盖 → 回落全局');
    expect(_sliderValue(tester, '表情幅度'), 1.0, reason: 'expression 未覆盖 → 回落全局');
    _drive(tester, '头部摆幅', 1.4);
    expect(modelHeads, <double>[1.4], reason: '开启覆盖 = 直接 PATCH 路');
    expect(globalHeads, isEmpty, reason: '开启覆盖时不得再写全局草稿');
  });

  testWidgets('「恢复跟随全局」：覆盖开启时可点并触发回调；关闭时禁用', (WidgetTester tester) async {
    int resets = 0;
    await tester.pumpWidget(
      _wrap(
        _section(
          action: _baiHeadOnly,
          modelOverrideEnabled: true,
          onReset: () => resets++,
        ),
      ),
    );
    final Finder button = find.widgetWithText(TextButton, '恢复跟随全局');
    expect(tester.widget<TextButton>(button).onPressed, isNotNull);
    // 面板在滚动画布里，按钮在首屏之外——先滚到可见再点（同 W7b 的先例）。
    await tester.ensureVisible(button);
    await tester.pumpAndSettle();
    await tester.tap(button);
    await tester.pump();
    expect(resets, 1, reason: '按钮必须触发宿主的删除覆盖回调');

    // 没有覆盖（关闭态）：按钮在，但禁用——不是点了没反应。
    await tester.pumpWidget(
      _wrap(_section(action: _globalBai, onReset: null)),
    );
    expect(
      tester
          .widget<TextButton>(find.widgetWithText(TextButton, '恢复跟随全局'))
          .onPressed,
      isNull,
    );
  });

  testWidgets('本模型覆盖开关：值来自宿主，关掉触发一次回调', (WidgetTester tester) async {
    final List<bool> toggles = <bool>[];
    await tester.pumpWidget(
      _wrap(
        _section(
          action: _baiHeadOnly,
          modelOverrideEnabled: true,
          onEnabled: toggles.add,
        ),
      ),
    );
    final Finder toggle = find.widgetWithText(ToggleField, '本模型覆盖');
    expect(tester.widget<ToggleField>(toggle).value, isTrue);
    final Switch sw = tester.widget<Switch>(
      find.descendant(of: toggle, matching: find.byType(Switch)),
    );
    sw.onChanged!(false);
    expect(toggles, <bool>[false]);
  });
}
