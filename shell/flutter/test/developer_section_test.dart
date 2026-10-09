/// 开发模式开关的「不谎报可关」契约（rc.3 N3，2026-09-13）+ 调试面板叠加表情契约（W3，2026-09-21）。
///
/// 第一部分：`--dev-mode` 会让服务端**强制**开启 `dev_mode`。那时界面里的开关必须
/// 显示为「由启动参数强制开启、关不掉」，而不是让用户点一下、看起来关了、其实没关。
///
/// 第二部分（W3）：调试面板的「叠加基础表情」**默认关**、基础表情按渲染面同口径
/// 到点即视为 none（UI 不再谎称在演、点手势不再把老脸续期），并且两块按钮清单
/// **从标签表的 channel 字段派生**，取不到表时如实说明。
///
/// 到点时刻的 ttl **来自舞台回执**（`PresetStatus.ttl`，渲染面 `duration_ms` 的
/// 投影），面板不自己写死；所以这里给一个假 `PresetStatus` 通知器模拟舞台回执，
/// 再用假时钟把到点钉死。
library;

import 'package:flutter/foundation.dart' show ValueListenable;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_stage.dart'
    show kDefaultExpressionIntensity, kDefaultPresetIntensity, PresetStatus;
import 'package:live2d_ai_shell/settings/mods/director_debug_panels.dart';
import 'package:live2d_ai_shell/settings/preset_labels.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart' show ToggleField;
import 'package:live2d_ai_shell/ui/theme.dart';

/// 假 ttl：测试文件不在 design_tokens_lint 的扫描面内，可以写具体值。
const Duration _expressionTtl = Duration(milliseconds: 2600);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 假标签表（真源字段形状与 `assets/actions/preset_labels.json` 一致）。
///
/// 故意把 `none` 也带 `channel: expression`：面板必须**排除**这个撤销哨兵。
const PresetLabelTable _labels = PresetLabelTable(<String, PresetLabel>{
  'none': PresetLabel(zh: '中性', en: 'Neutral', channel: 'expression'),
  'smile': PresetLabel(zh: '微笑', en: 'Smile', channel: 'expression'),
  'unhappy': PresetLabel(zh: '不悦', en: 'Unhappy', channel: 'expression'),
  'surprised': PresetLabel(zh: '惊讶', en: 'Surprised', channel: 'expression'),
  'nod': PresetLabel(zh: '点头', en: 'Nod', channel: 'motion'),
  'shake': PresetLabel(zh: '摇头', en: 'Shake', channel: 'motion'),
});

/// 另一张「故意不对齐」的假表：只有它才能证明按钮组是**派生**出来的。
const PresetLabelTable _channelTable = PresetLabelTable(<String, PresetLabel>{
  'none': PresetLabel(zh: '中性', en: 'Neutral', channel: 'expression'),
  'wink': PresetLabel(zh: '眨眼', en: 'Wink', channel: 'expression'),
  'jump': PresetLabel(zh: '跳', en: 'Jump', channel: 'motion'),
  'mystery': PresetLabel(zh: '谜', en: 'Mystery', channel: 'other'),
});

Finder _presetButton(PresetLabelTable labels, String id) =>
    find.widgetWithText(OutlinedButton, labels.display(id));

Finder _overlayToggle() => find.widgetWithText(ToggleField, '叠加基础表情');

Finder _overlaySwitch() =>
    find.descendant(of: _overlayToggle(), matching: find.byType(Switch));

/// 模拟舞台回执：面板的到点时刻只认这条 `PresetStatus.ttl`。
void _armStatus(
  ValueNotifier<PresetStatus?> status,
  String id,
  DateTime at,
) {
  status.value = PresetStatus(
    id: id,
    source: 'debug',
    startedAt: at,
    ttl: _expressionTtl,
  );
}

/// 2026-10-09：这两块（表情调试 / 动作调试 + 临时幅度）已从核心「开发模式」页
/// 搬到「扩展 → 导演」卡片的下级块。本文件直接挂 [DebugPanels] 测它的行为；
/// 「它只出现在导演卡片、且只在 devMode 里」由下面的结构断言与
/// `action_scales_wiring_test` 的 director 面板用例守着。
Widget _section({
  PresetApply? onApplyPreset,
  PresetLabelTable labels = _labels,
  DebugClock? clock,
  ValueListenable<PresetStatus?>? status,
  bool devMode = true,
}) => DebugPanels(
  labels: labels,
  status: status,
  clock: clock,
  onApplyPreset: onApplyPreset,
);

void main() {
  testWidgets('forcedByLaunchFlag=true：开关是 on 且不可点，并说明原因', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        DeveloperSection(
          devMode: true,
          onDevModeChanged: (_) {},
          forcedByLaunchFlag: true,
        ),
      ),
    );
    // devMode 打开时面板里还有「叠加基础表情」的开关，必须点名第一个
    //（开发者模式开关住在 DebugPanels 之上）。
    final Switch toggle = tester.widget<Switch>(find.byType(Switch).first);
    expect(toggle.value, isTrue, reason: '有效 dev_mode 是 on');
    expect(toggle.onChanged, isNull, reason: '强制开启时不许看起来能关');
    expect(
      find.textContaining('启动参数'),
      findsOneWidget,
      reason: '得说清为什么关不掉，否则就是「名实不符」',
    );
  });

  testWidgets('forcedByLaunchFlag=false：开关可点，且没有「启动参数」文案', (
    WidgetTester tester,
  ) async {
    bool? changed;
    await tester.pumpWidget(
      _wrap(
        DeveloperSection(
          devMode: false,
          onDevModeChanged: (bool v) => changed = v,
          forcedByLaunchFlag: false,
        ),
      ),
    );
    // devMode 关闭时 DebugPanels 不渲染，这里只有开发者模式一个开关。
    final Switch toggle = tester.widget<Switch>(find.byType(Switch).first);
    expect(toggle.onChanged, isNotNull);
    expect(find.textContaining('启动参数'), findsNothing);
    await tester.tap(find.byType(Switch));
    expect(changed, isTrue, reason: '非强制时点一下必须真的改状态');
  });

  testWidgets('2026-10-09：两块调试**不在**核心开发模式页上（只在导演卡片下级）', (
    WidgetTester tester,
  ) async {
    // 能力没删：它们搬到「扩展 → 导演」卡片（settings/mods/director_debug_panels.dart），
    // 仍只在开发者模式里显示。这里钉两件事：
    // ① 核心「开发模式」页（DeveloperSection）两种 devMode 下都不画它们；
    // ② 组件本身还在、两块的标题都在。
    const List<String> titles = <String>['表情调试', '动作调试'];

    for (final bool dev in <bool>[true, false]) {
      await tester.pumpWidget(
        _wrap(
          DeveloperSection(
            devMode: dev,
            onDevModeChanged: (_) {},
            forcedByLaunchFlag: false,
          ),
        ),
      );
      await tester.pumpAndSettle();
      for (final String title in titles) {
        expect(
          find.text(title),
          findsNothing,
          reason: 'devMode=$dev：核心开发模式页只剩开关与诊断，不该有「$title」',
        );
      }
    }

    await tester.pumpWidget(_wrap(_section(devMode: true)));
    await tester.pumpAndSettle();
    for (final String title in titles) {
      expect(find.text(title), findsOneWidget, reason: '导演卡片那块缺少「$title」');
    }
  });

  testWidgets('表情调试：基础表情强度缺省 1.0，点表情只发 Face 预设', (WidgetTester tester) async {
    final List<(String, double)> calls = <(String, double)>[];
    await tester.pumpWidget(
      _wrap(_section(onApplyPreset: (String id, double intensity) => calls.add((id, intensity)))),
    );
    // 表情与手势强度刻意不同：表情 1.0 = 表值本身（确认基础强度），
    // 手势 1.2 才肉眼明显。两块不共用一条滑条。
    expect(kDefaultExpressionIntensity, 1.0);
    expect(find.text('基础表情强度'), findsOneWidget);

    final Finder smile = _presetButton(_labels, 'smile');
    await tester.ensureVisible(smile);
    await tester.pumpAndSettle();
    await tester.tap(smile);
    expect(calls, <(String, double)>[('smile', kDefaultExpressionIntensity)]);
  });

  testWidgets('动作调试：默认动作强度 >= 1.2，点手势时一起下发', (WidgetTester tester) async {
    final List<(String, double)> calls = <(String, double)>[];
    await tester.pumpWidget(
      _wrap(_section(onApplyPreset: (String id, double intensity) => calls.add((id, intensity)))),
    );
    // 强度口径（L1）：1.0 是「微抖」肉眼看不出来，出厂取 1.2。
    expect(
      kDefaultPresetIntensity,
      greaterThanOrEqualTo(1.2),
      reason: '出厂强度不得退回 1.0（用户实测看不出摆头）',
    );
    expect(find.text('动作强度'), findsOneWidget);

    // 两块面板叠起来比一屏高：先滚到目标再点，否则点击落在视口外（静默不触发）。
    final Finder nod = _presetButton(_labels, 'nod');
    await tester.ensureVisible(nod);
    await tester.pumpAndSettle();
    await tester.tap(nod);
    expect(calls, <(String, double)>[('nod', kDefaultPresetIntensity)]);
  });

  testWidgets('动作调试：「叠加基础表情」默认关：选过表情后点手势也只发手势', (WidgetTester tester) async {
    final List<(String, double)> calls = <(String, double)>[];
    final DateTime fixedNow = DateTime(2026, 1, 1, 12);
    final ValueNotifier<PresetStatus?> status = ValueNotifier<PresetStatus?>(null);
    addTearDown(status.dispose);
    await tester.pumpWidget(
      _wrap(
        _section(
          status: status,
          clock: () => fixedNow,
          onApplyPreset: (String id, double intensity) => calls.add((id, intensity)),
        ),
      ),
    );

    // ① 出厂「不接管」：开关本身的断言。
    final Switch overlay = tester.widget<Switch>(_overlaySwitch());
    expect(
      overlay.value,
      isFalse,
      reason: '默认不接管：出厂不叠加基础表情，否则点手势总带着上一张老脸（症状①）',
    );

    // 先在表情调试里选一条基础表情，并喂给它舞台回执（= 它**真的在演**）。
    final Finder unhappy = _presetButton(_labels, 'unhappy');
    await tester.ensureVisible(unhappy);
    await tester.pumpAndSettle();
    await tester.tap(unhappy);
    await tester.pump();
    expect(calls, <(String, double)>[('unhappy', kDefaultExpressionIntensity)]);
    _armStatus(status, 'unhappy', fixedNow);
    await tester.pump();

    // ② 行为也默认不叠加：即便基础表情正在演，开关关着就不得先发它。
    calls.clear();
    final Finder nod = _presetButton(_labels, 'nod');
    await tester.ensureVisible(nod);
    await tester.pumpAndSettle();
    await tester.tap(nod);
    expect(
      calls,
      <(String, double)>[('nod', kDefaultPresetIntensity)],
      reason: '开关默认关 ⇒ 手势点击不得隐式带上基础表情',
    );
  });

  testWidgets('动作调试：叠加基础表情开着时，先发 Face 再发 Gesture', (WidgetTester tester) async {
    final List<(String, double)> calls = <(String, double)>[];
    final DateTime fixedNow = DateTime(2026, 1, 1, 12);
    final ValueNotifier<PresetStatus?> status = ValueNotifier<PresetStatus?>(null);
    addTearDown(status.dispose);
    await tester.pumpWidget(
      _wrap(
        _section(
          status: status,
          clock: () => fixedNow,
          onApplyPreset: (String id, double intensity) => calls.add((id, intensity)),
        ),
      ),
    );

    // 开关**缺省关**：这条契约要先显式打开它（W3 起不再默认接管）。
    await tester.ensureVisible(_overlayToggle());
    await tester.pumpAndSettle();
    await tester.tap(_overlaySwitch());
    await tester.pump();
    expect(tester.widget<Switch>(_overlaySwitch()).value, isTrue);

    // 先在表情调试里选一个基础表情（它自己那一下也走 Face 槽），并喂舞台回执。
    final Finder unhappy = _presetButton(_labels, 'unhappy');
    await tester.ensureVisible(unhappy);
    await tester.pumpAndSettle();
    await tester.tap(unhappy);
    await tester.pump();
    expect(calls, <(String, double)>[('unhappy', kDefaultExpressionIntensity)]);
    _armStatus(status, 'unhappy', fixedNow);
    await tester.pump();

    calls.clear();
    final Finder nod = _presetButton(_labels, 'nod');
    await tester.ensureVisible(nod);
    await tester.pumpAndSettle();
    await tester.tap(nod);
    // Face 在前、Gesture 在后——与渲染面同帧写入顺序一致。
    expect(calls, <(String, double)>[
      ('unhappy', kDefaultExpressionIntensity),
      ('nod', kDefaultPresetIntensity),
    ]);
  });

  /// 2026-10-09：原来还断言那行「当前基础表情：无（已到点）」——它是控件说明，
  /// 随「三级功能介绍全删」一起删了。这里改成**纯行为**断言（到点即视为 none，
  /// 点手势不得把演完的老脸重新点亮 / 续期），判据落在调用序列上，不落在文案上。
  testWidgets('表情到点后：点手势不再重发它（行为断言，不看文案）', (WidgetTester tester) async {
    final List<(String, double)> calls = <(String, double)>[];
    DateTime now = DateTime(2026, 1, 1, 12);
    final ValueNotifier<PresetStatus?> status = ValueNotifier<PresetStatus?>(null);
    addTearDown(status.dispose);
    await tester.pumpWidget(
      _wrap(
        _section(
          status: status,
          clock: () => now,
          onApplyPreset: (String id, double intensity) => calls.add((id, intensity)),
        ),
      ),
    );

    // 显式打开叠加，这样「到点后不重发」才有行为意义。
    await tester.ensureVisible(_overlayToggle());
    await tester.pumpAndSettle();
    await tester.tap(_overlaySwitch());
    await tester.pump();

    final Finder smile = _presetButton(_labels, 'smile');
    await tester.ensureVisible(smile);
    await tester.pumpAndSettle();
    await tester.tap(smile);
    await tester.pump();
    expect(calls, <(String, double)>[('smile', kDefaultExpressionIntensity)]);
    _armStatus(status, 'smile', now);
    await tester.pump();

    // 到点（渲染面同口径 ttl）：行为上等于 none。
    calls.clear();
    now = now.add(_expressionTtl);
    await tester.pump(_expressionTtl);

    // 到点后点手势：不得把已经演完的那张老脸重新点亮 / 续期。
    final Finder nod = _presetButton(_labels, 'nod');
    await tester.ensureVisible(nod);
    await tester.pumpAndSettle();
    await tester.tap(nod);
    expect(
      calls,
      <(String, double)>[('nod', kDefaultPresetIntensity)],
      reason: '到点后 _expressionId 行为上等于 none：不再重发它',
    );
  });

  testWidgets('按钮组来自标签表的 channel；取不到表时如实说明', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(_section(labels: _channelTable)),
    );
    expect(
      find.widgetWithText(OutlinedButton, '眨眼（wink）'),
      findsOneWidget,
      reason: 'channel=expression 的包进「表情调试」',
    );
    expect(
      find.widgetWithText(OutlinedButton, '跳（jump）'),
      findsOneWidget,
      reason: 'channel=motion 的包进「动作调试」',
    );
    expect(
      find.widgetWithText(OutlinedButton, '中性（none）'),
      findsNothing,
      reason: 'none 是撤销哨兵（另有归零按钮），不进任何按钮组',
    );
    expect(
      find.widgetWithText(OutlinedButton, '谜（mystery）'),
      findsNothing,
      reason: 'channel=other 不属于两块调试',
    );

    // 取不到表：回落为空 + 如实说明，而不是静默显示空按钮组。
    await tester.pumpWidget(
      _wrap(_section(labels: PresetLabelTable.empty)),
    );
    expect(find.byType(OutlinedButton), findsNothing);
    expect(
      find.textContaining('预设标签表未加载'),
      findsNWidgets(2),
      reason: '表情、动作两块都要如实说明取不到表',
    );
  });

  testWidgets('诊断只在开发模式里渲染（2026-10-09：一级「诊断」取消后收进这一页）', (
    WidgetTester tester,
  ) async {
    // 传入的是一整块**已经带数据的**诊断面；本页只决定画不画。
    const Widget diagnostics = Text('DIAG-PROBE');

    await tester.pumpWidget(
      _wrap(
        DeveloperSection(
          devMode: false,
          onDevModeChanged: (_) {},
          forcedByLaunchFlag: false,
          diagnostics: diagnostics,
        ),
      ),
    );
    expect(find.text('DIAG-PROBE'), findsNothing, reason: 'devMode=false 时诊断整块不在语义树');
    expect(find.text('导演可观测'), findsNothing);

    await tester.pumpWidget(
      _wrap(
        DeveloperSection(
          devMode: true,
          onDevModeChanged: (_) {},
          forcedByLaunchFlag: false,
          diagnostics: diagnostics,
        ),
      ),
    );
    expect(find.text('DIAG-PROBE'), findsOneWidget);
    expect(
      find.text('导演可观测'),
      findsNothing,
      reason: '导演可观测住在「扩展 → 导演」卡片下级，核心链的开发模式页不再出现它',
    );
  });
}
