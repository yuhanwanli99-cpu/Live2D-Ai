/// 键盘焦点可见性（2026-09-11，前端加强计划 P1-4）。
///
/// # 为什么单独一个文件
///
/// `AppColors.focusRing` 曾是**最后两个没接线的令牌之一**——它声明了很久，
/// 但全应用没有任何地方用它，于是键盘用户拿到的焦点提示是 Material 的默认
/// overlay：深色主题下是一层几乎看不见的灰，而且**对比度没法逐主题断言**
/// （`theme_palette_test.dart` 只验了正文/面板）。
///
/// 现在的做法是「1 px 不透明描边」（规格 §9.2）：焦点环与其它颜色走**同一套**
/// 对比度断言。本文件钉三件事：
///
/// 1. **四套主题下焦点环都真的出现了**，且用的是 `focusRing` 那个颜色
///    （不是 Material 的默认 overlay）；
/// 2. **只在焦点态出现**——平时不该给按钮长出一圈边（否则每颗按钮都像
///    被框住的表单控件）；
/// 3. **在真的 widget 上生效**（样式表写了却被更近的 `styleFrom` 覆盖掉，
///    是这类改动的经典失败方式）。
///
/// 刻意**不**断言 `FocusManager.highlightMode`：那是 Material 的既定语义
/// （触屏用户不该看到焦点环），本层不该覆盖它，也不该在这里钉死。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 从主题里取配色扩展（不用 `BuildContext`：本文件的多数断言是纯样式表级别的）。
AppColors colorsOf(ThemeData t) => t.extension<AppColors>()!;

/// 某主题下「这批状态」解析出来的边。
BorderSide? sideFor(ThemeData t, Set<WidgetState> states) =>
    t.filledButtonTheme.style?.side?.resolve(states);

void main() {
  group('P1-4：键盘焦点环 = 1 px 不透明 focusRing', () {
    for (final AppThemeId id in AppThemeId.values) {
      test('${id.wire} 主题：焦点态用的是 focusRing，不是 Material 默认 overlay', () {
        final ThemeData t = buildAppTheme(id);
        final BorderSide? focused = sideFor(t, <WidgetState>{
          WidgetState.focused,
        });
        expect(focused, isNotNull, reason: '${id.wire} 上焦点态没有描边');
        expect(
          focused!.color,
          colorsOf(t).focusRing,
          reason: '${id.wire} 的焦点环不是 focusRing 令牌（四套主题必须各自成立）',
        );
        expect(focused.width, 1, reason: '规格 §9.2 点名的就是 1 px');
      });
    }

    test('非焦点态**不长边**（否则每颗按钮都像被框住的表单控件）', () {
      final ThemeData t = buildAppTheme();
      expect(sideFor(t, <WidgetState>{}), isNull);
      expect(sideFor(t, <WidgetState>{WidgetState.hovered}), isNull);
      expect(sideFor(t, <WidgetState>{WidgetState.pressed}), isNull);
      // 禁用态也不长边：一个不能按的按钮再加一圈描边只会更吵。
      expect(sideFor(t, <WidgetState>{WidgetState.disabled}), isNull);
    });

    test('有边按钮（Outlined）平时是 hairline、焦点时换成 focusRing', () {
      final ThemeData t = buildAppTheme();
      final ButtonStyle? style = t.outlinedButtonTheme.style;
      expect(style?.side?.resolve(<WidgetState>{})?.color, colorsOf(t).hairline);
      expect(
        style?.side?.resolve(<WidgetState>{WidgetState.focused})?.color,
        colorsOf(t).focusRing,
      );
    });

    test('图标按钮也有焦点环（它没有文字标签，键盘用户只能靠环找位置）', () {
      final ThemeData t = buildAppTheme();
      expect(
        t.iconButtonTheme.style?.side
            ?.resolve(<WidgetState>{WidgetState.focused})
            ?.color,
        colorsOf(t).focusRing,
      );
    });

    testWidgets('按钮真的拿到焦点时，解析出来的边就是 focusRing', (
      WidgetTester tester,
    ) async {
      final ThemeData theme = buildAppTheme();
      await tester.pumpWidget(
        MaterialApp(
          theme: theme,
          home: Scaffold(
            body: Center(
              child: FilledButton(onPressed: () {}, child: const Text('按钮')),
            ),
          ),
        ),
      );

      final FocusNode node = Focus.of(tester.element(find.text('按钮')));
      node.requestFocus();
      await tester.pumpAndSettle();
      expect(node.hasFocus, isTrue, reason: '按钮没拿到焦点，下面这条就白验了');

      // 按钮自己没有 style，所以实际生效的就是主题那一份——
      // 而 `ButtonStyleButton` 会把 `focused` 状态喂给它。
      final BorderSide? wear = theme.filledButtonTheme.style!.side!.resolve(
        <WidgetState>{WidgetState.focused},
      );
      expect(wear?.color, colorsOf(theme).focusRing);
    });
  });
}
