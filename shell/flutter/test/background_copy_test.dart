/// P0-5 回归：遮罩文案必须与 `scrimAlphaFor` 的**实际输入**一致。
///
/// `auto` 遮罩 = `kAutoScrimGain * opacity * (0.5 + 0.5 * uiTransparency)`：
/// 它只吃「图的不透明度」与「界面透明程度」。公式里曾经还有一项 `luma`
/// （按图片亮度加权），2026-09-27 已删除——而界面还在承诺
/// 「随图变亮自动加强」，那是**与实现相反**的文案。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

const String _img = 'data:image/png;base64,AAAA';

Widget _pane(DisplayPrefs prefs) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: AppearanceSection(
        prefs: prefs,
        onPrefsChanged: (DisplayPrefs _) {},
        onPickShellImage: () {},
        onClearShellImage: () {},
        onRemoveBackground: (int _) {},
        onReorderBackground: (int _, int _) {},
        onPreviewBackground: (int _) {},
        onAddPattern: (int _) {},
      ),
    ),
  ),
);

void main() {
  testWidgets('遮罩说明不再承诺已删除的「按图变亮自动加强」', (WidgetTester tester) async {
    await tester.pumpWidget(
      _pane(
        const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
          ],
        ),
      ),
    );
    expect(find.textContaining('随图变亮'), findsNothing);
    expect(find.textContaining('自动加强'), findsNothing);
    expect(
      find.textContaining('强度随图的不透明度与界面透明度算出'),
      findsOneWidget,
      reason: '文案要把 auto 遮罩真正依赖的两个输入说出来',
    );
  });
}
