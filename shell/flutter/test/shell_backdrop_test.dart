/// 壳全局背景（2026-09-14，rc.5）的单元 / widget 回归。
///
/// 守两件事：
/// 1. `decodeDataUrlBytes` **永不抛**（坏存储不许把壳变红屏）；
/// 2. 有图时确实按**固定** [kShellBackdropOpacity] 铺了一层，且没有滑条改它。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/shell_backdrop.dart';

/// 1×1 透明 PNG 的 dataURL（合法图片字节）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

void main() {
  group('decodeDataUrlBytes：坏输入一律 null，不抛', () {
    test('null / 空 / 非 dataURL', () {
      expect(decodeDataUrlBytes(null), isNull);
      expect(decodeDataUrlBytes(''), isNull);
      expect(decodeDataUrlBytes('https://example.com/a.png'), isNull);
      expect(decodeDataUrlBytes('data:image/png,notbase64'), isNull);
    });

    test('base64 载荷非法 → null', () {
      expect(decodeDataUrlBytes('data:image/png;base64,!!!'), isNull);
      expect(decodeDataUrlBytes('data:image/png;base64,'), isNull);
    });

    test('合法 dataURL → 字节非空', () {
      expect(decodeDataUrlBytes(_onePixelPng), isNotNull);
    });
  });

  test('透明度是固定 0.15（不做滑条的契约值）', () {
    expect(kShellBackdropOpacity, 0.15);
    // 面板留一点透，但必须接近不透明——这是「聊天/侧栏可读」的下界。
    expect(kShellSurfaceAlpha, greaterThanOrEqualTo(0.8));
    expect(kShellSurfaceAlpha, lessThan(1.0));
  });

  testWidgets('没有背景图 → 只有底色，没有图片层', (WidgetTester tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: ShellBackdrop(
          baseColor: Color(0xFF123456),
          image: null,
          child: SizedBox.expand(),
        ),
      ),
    );
    expect(find.byType(Image), findsNothing);
    // 只看 ShellBackdrop 自己那层底（MaterialApp 自己也有 ColoredBox）。
    final ColoredBox box = tester.widget<ColoredBox>(
      find
          .descendant(
            of: find.byType(ShellBackdrop),
            matching: find.byType(ColoredBox),
          )
          .first,
    );
    expect(box.color, const Color(0xFF123456));
  });

  testWidgets('有背景图 → 按固定透明度铺一层 Image', (WidgetTester tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: ShellBackdrop(
          baseColor: Color(0xFF123456),
          image: _onePixelPng,
          child: SizedBox.expand(),
        ),
      ),
    );
    await tester.pump();
    expect(
      find.byWidgetPredicate(
        (Widget w) => w is Opacity && w.opacity == kShellBackdropOpacity,
      ),
      findsOneWidget,
    );
    expect(find.byType(Image), findsOneWidget);
  });
}
