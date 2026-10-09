/// P0-5 回归：遮罩文案 / 控件必须与 `scrimAlphaFor` 的**实际输入**一致。
///
/// `auto` 遮罩 = `kAutoScrimGain * opacity * (0.5 + 0.5 * uiTransparency)`：
/// 它只吃「图的不透明度」与「界面透明程度」。公式里曾经还有一项 `luma`
/// （按图片亮度加权），2026-09-27 已删除——而界面还在承诺
/// 「随图变亮自动加强」，那是**与实现相反**的文案。
///
/// 2026-10-07 外观卡片简化：遮罩档不再是用户旋钮（产品路径固定 `auto`），
/// 那一整块文案也跟着控件从界面上删掉了。所以这条回归改成两半：
///
/// ① **负向**：界面源码与泵出来的树上都不再有那两句承诺（也不再有遮罩控件）；
/// ② **正向**：`scrimAlphaFor` 仍只吃那两个真实自变量，且四档两两不同
///    （防止「文案删了、渲染函数也悄悄退化」）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/background_logic.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

const String _img = 'data:image/png;base64,AAAA';

Widget _pane(DisplayPrefs prefs) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: ThemeSection(
        prefs: prefs,
        onPrefsChanged: (DisplayPrefs _) {},
        onPickShellImage: () {},
        onClearShellImage: () {},
        onRemoveBackground: (int _) {},
        onReorderBackground: (int _, int _) {},
        onPreviewBackground: (int _) {},
      ),
    ),
  ),
);

void main() {
  const List<String> goneCopy = <String>[
    '随图变亮',
    '自动加强',
    '强度随图的不透明度与界面透明度算出',
  ];

  test('外观源码里不再有这几句遮罩文案（注释不算）', () {
    final String code = stripCommentsKeepStrings(
      readLibrarySource('lib/settings/sections/appearance_section.dart'),
    );
    for (final String gone in goneCopy) {
      expect(
        code,
        isNot(contains(gone)),
        reason: '$gone 已随遮罩控件在 2026-10-07 删除：界面不再谈遮罩',
      );
    }
  });

  testWidgets('泵出来的树上也没有那两句承诺（也不再承诺任何遮罩行为）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _pane(
        const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
          ],
        ),
      ),
    );
    for (final String gone in goneCopy) {
      expect(find.textContaining(gone), findsNothing, reason: '$gone 不该在界面上');
    }
  });

  test('scrimAlphaFor：仍只吃「图不透明度」与「界面透明度」两个真实自变量', () {
    expect(
      scrimAlphaFor(imageOpacity: 0.6, level: ScrimLevel.auto),
      greaterThan(scrimAlphaFor(imageOpacity: 0.1, level: ScrimLevel.auto)),
      reason: '图越实，压得越狠',
    );
    expect(
      scrimAlphaFor(
        imageOpacity: 0.6,
        level: ScrimLevel.auto,
        uiTransparency: 1,
      ),
      greaterThan(scrimAlphaFor(imageOpacity: 0.6, level: ScrimLevel.auto)),
      reason: '面板越透，背景要压得越多，否则是浅底上的浅灰字',
    );
    // 四档仍两两不同：渲染函数没有随控件一起退化。
    expect(
      <double>{
        for (final int level in <int>[0, 1, 2, 3])
          scrimAlphaFor(imageOpacity: 0.6, level: level),
      }.length,
      4,
    );
  });
}
