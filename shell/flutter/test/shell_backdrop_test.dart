/// 壳全局背景（2026-09-27 重写）的单元 / widget 回归。
///
/// 守四件「坏了会很怪但不会报错」的事：
/// 1. `decodeDataUrlBytes` **永不抛**（坏存储不许把壳变红屏）；
/// 2. 没有背景 / 透明为 0 / 坏 dataURL → 这一层**零观感差异**；
/// 3. 有背景时确实按「图 / 遮罩 / 子树」三层叠起来，且遮罩不吃指针；
/// 4. 缺省不透明度**是 1.0**（2026-09-27 改：0.15 是 rc.5 装饰底纹的遗留值，
///    用户亲手挑的图被压到 15% 就等于「加了和没加一样」）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/ui/background_logic.dart';
import 'package:live2d_ai_shell/ui/shell_backdrop.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 透明 PNG 的 dataURL（合法图片字节）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

void main() {
  group('缺省值：加了图就该看得见', () {
    test('不透明度 1.0，模糊 0，遮罩 auto', () {
      expect(DisplayPrefs.defaultBackgroundOpacity, 1.0);
      expect(DisplayPrefs.defaultBackgroundBlur, 0.0);
      expect(DisplayPrefs.defaultBackgroundScrim, ScrimLevel.auto);
    });

    testWidgets('「字节还没读回来」的项**画不出来**（画底色，不画半个空位）',
        (WidgetTester tester) async {
      // 背景字节搬去 IndexedDB 之后，偏好里可能有一项而字节没到手。
      // 那时若照样走 Image.memory 路径，得到的是一张永远空白的框。
      await tester.pumpWidget(
        _host(const DisplayPrefs(), item: const BackgroundImage(id: 'pending')),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('子树'), findsOneWidget);
      expect(
        find.byType(Image),
        findsNothing,
        reason: '没有字节就不该有 Image —— 不透明度再高也画不出东西',
      );
      expect(find.byType(Stack), findsNothing, reason: '连叠放层都不需要');
    });
  });

  group('decodeDataUrlBytes：坏输入一律 null，不抛', () {
    test('合法 dataURL 解出非空字节', () {
      expect(decodeDataUrlBytes(_onePixelPng), isNotNull);
      expect(decodeDataUrlBytes(_onePixelPng)!.isNotEmpty, isTrue);
    });

    test('null / 空 / 没有逗号 / 不是 base64 / 坏 base64 / 空字节', () {
      for (final String? bad in <String?>[
        null,
        '',
        'data:image/png;base64',
        'data:image/png,notbase64!!!',
        'data:image/png;base64,@@@@',
        'data:image/png;base64,',
      ]) {
        expect(decodeDataUrlBytes(bad), isNull, reason: '不该接受 $bad');
      }
    });
  });

  group('铺法与位置的翻译层', () {
    test('只有 1 才是 contain，其余退到 cover（且都不许抛）', () {
      expect(boxFitFor(0), BoxFit.cover);
      expect(boxFitFor(1), BoxFit.contain);
      expect(boxFitFor(2), isA<BoxFit>());
      expect(boxFitFor(3), isA<BoxFit>());
      expect(boxFitFor(99), BoxFit.cover);
    });

    test('九宫格 4 是居中，越界夹回范围（不抛）', () {
      expect(alignmentFor(4), Alignment.center);
      expect(alignmentFor(0), const Alignment(-1, -1));
      expect(alignmentFor(8), const Alignment(1, 1));
      expect(alignmentFor(99), const Alignment(1, 1));
      expect(alignmentFor(-1), const Alignment(-1, -1));
    });
  });

  group('零观感差异的三个情形', () {
    testWidgets('item = null → 只有底色，连 Stack 都没有', (WidgetTester tester) async {
      await tester.pumpWidget(_host(const DisplayPrefs()));
      expect(find.text('子树'), findsOneWidget);
      expect(find.byType(Stack), findsNothing);
    });

    testWidgets('不透明度 0 → 同样等价于「只有底色」', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const DisplayPrefs(),
          item: const BackgroundImage(id: 'p', dataUrl: _onePixelPng),
          opacity: 0,
        ),
      );
      expect(find.text('子树'), findsOneWidget);
      expect(find.byType(Stack), findsNothing);
    });

    testWidgets('坏 dataURL → 退回底色而不是红屏', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const DisplayPrefs(),
          item: const BackgroundImage(id: 'bad', dataUrl: 'data:image/png;base64,@@@'),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('子树'), findsOneWidget);
    });
  });

  group('有背景时确实叠了「图 / 遮罩 / 子树」', () {
    testWidgets('重遮罩：图在、遮罩是 IgnorePointer、子树照常在', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const DisplayPrefs(),
          item: const BackgroundImage(id: 'p', dataUrl: _onePixelPng),
          opacity: 0.9,
          scrim: ScrimLevel.heavy,
        ),
      );
      expect(find.text('子树'), findsOneWidget);
      expect(find.byType(Image), findsOneWidget);
      // 遮罩不吃指针：压在上面的控件必须照常可点。
      expect(find.byType(IgnorePointer), findsWidgets);
    });

    testWidgets('遮罩=无 → 不画遮罩那一层', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          const DisplayPrefs(),
          item: const BackgroundImage(id: 'p', dataUrl: _onePixelPng),
          opacity: 0.9,
          scrim: ScrimLevel.none,
        ),
      );
      expect(find.text('子树'), findsOneWidget);
      expect(find.byKey(kShellScrimKey), findsNothing);
    });

    testWidgets('图案走 CustomPainter，不去解字节', (WidgetTester tester) async {
      final AppPalette palette = AppPalette.black;
      await tester.pumpWidget(
        _host(
          const DisplayPrefs(),
          item: const BackgroundPattern(BackgroundPatternId.gradient),
          opacity: 0.8,
          patternColors: patternColorsFor(
            BackgroundPatternId.gradient,
            palette,
          ),
        ),
      );
      expect(find.text('子树'), findsOneWidget);
      expect(find.byType(CustomPaint), findsWidgets);
      expect(find.byType(Image), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });
}

Widget _host(
  DisplayPrefs prefs, {
  BackgroundItem? item,
  double? opacity,
  int? scrim,
  PatternColors? patternColors,
}) => MaterialApp(
  // 用**真的**主题：`ShellBackdrop` 的遮罩颜色读 `AppColors/AppPalette`，
  // 手搓一个 ThemeData 会让 `appPaletteOf` 的断言当场炸（那正是它该炸的）。
  theme: buildAppTheme(prefs.theme),
  home: ShellBackdrop(
    baseColor: AppPalette.of(prefs.theme).stage,
    item: item,
    opacity: opacity ?? prefs.backgroundOpacity,
    scrim: scrim ?? DisplayPrefs.defaultBackgroundScrim,
    patternColors: patternColors,
    child: const Text('子树'),
  ),
);
