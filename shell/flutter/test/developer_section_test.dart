/// 开发模式开关的「不谎报可关」契约（rc.3 N3，2026-09-13）。
///
/// `--dev-mode` 会让服务端**强制**开启 `dev_mode`。那时界面里的开关必须
/// 显示为「由启动参数强制开启、关不掉」，而不是让用户点一下、看起来关了、其实没关。
///
/// 本文件锁的是**组件契约**（`forcedByLaunchFlag` → 开关禁用 + 说明文案）；
/// 「强制与否」的判据在组合根 `lib/app/shell_settings.dart`：有效 dev_mode 为 on、
/// 落盘设置为 off。那条判据没有纯函数可测（它读两个既有字段），所以这里只钉组件行为。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
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
    final Switch toggle = tester.widget<Switch>(find.byType(Switch));
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
    final Switch toggle = tester.widget<Switch>(find.byType(Switch));
    expect(toggle.onChanged, isNotNull);
    expect(find.textContaining('启动参数'), findsNothing);
    await tester.tap(find.byType(Switch));
    expect(changed, isTrue, reason: '非强制时点一下必须真的改状态');
  });
}
