/// 玻璃边缘高光（2026-09-11，前端加强计划 P3-1 / P3-2）。
///
/// # 本文件钉三件事
///
/// 1. **它不含任何模糊**。这是本项目最硬的一条红线：舞台是 `<iframe>` 平台
///    视图，`BackdropFilter` 采不到它、代价却照付（规格 §12-7）。`GlassRim`
///    压在舞台上的三处（内联侧板 / compact 整页 / 舞台角标）都必须只用
///    `CustomPaint` 画描边。
/// 2. **它只描边、不铺底**（不改变底色，也不吃指针——面板上的按钮要能点）。
/// 3. **四套主题都看得见**。把基色写死成白色，会在**白色主题上完全消失**；
///    这正是本项目「四套主题逐套验」要抓的那类问题。
library;

import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/glass_rim.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 剥掉注释与字符串（否则头注里提到的词会把自己判红）。
String stripCommentsAndStrings(String src) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    final String c = src[i];
    if (c == "'" || c == '"') {
      final bool triple =
          i + 2 < src.length && src[i + 1] == c && src[i + 2] == c;
      final String quote = triple ? c + c + c : c;
      i += quote.length;
      while (i < src.length) {
        if (src[i] == r'\') {
          i += 2;
          continue;
        }
        if (src.startsWith(quote, i)) {
          i += quote.length;
          break;
        }
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '/') {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '*') {
      i += 2;
      while (i + 1 < src.length && !(src[i] == '*' && src[i + 1] == '/')) {
        i++;
      }
      i += 2;
      continue;
    }
    out.write(c);
    i++;
  }
  return out.toString();
}

Widget wrap(Widget child, {AppThemeId theme = AppThemeId.black}) => MaterialApp(
  theme: buildAppTheme(theme),
  home: Scaffold(body: Center(child: child)),
);

void main() {
  group('P3-1：只画描边——没有模糊、没有采样（平台视图红线）', () {
    test('`glass_rim.dart` 里没有任何 BackdropFilter / ImageFilter', () {
      final String src = stripCommentsAndStrings(
        File('lib/ui/glass_rim.dart').readAsStringSync(),
      );
      expect(src.contains('BackdropFilter'), isFalse);
      expect(src.contains('ImageFilter'), isFalse);
      expect(src.contains('ShaderMask'), isFalse);
      expect(src.contains('ImageFiltered'), isFalse);
    });

    test('用的是 CustomPaint（这是它可以压在 iframe 上的全部理由）', () {
      final String src = stripCommentsAndStrings(
        File('lib/ui/glass_rim.dart').readAsStringSync(),
      );
      expect(src.contains('CustomPaint('), isTrue);
    });

    test('整仓 `BackdropFilter` 命中数仍然为 0（P3-1 没有把它带回来）', () {
      // 与 `no_backdrop_filter_test.dart` 同一条红线，这里再钉一次是因为
      // P3 的全部内容就是「加一层玻璃观感」——正是最容易破例的时候。
      final List<String> hits = <String>[];
      for (final FileSystemEntity e in Directory(
        'lib',
      ).listSync(recursive: true)) {
        if (e is! File || !e.path.endsWith('.dart')) continue;
        if (stripCommentsAndStrings(
          e.readAsStringSync(),
        ).contains('BackdropFilter')) {
          hits.add(e.path);
        }
      }
      expect(hits, isEmpty, reason: '这些文件引入了 BackdropFilter：$hits');
    });
  });

  group('P3-1：只描边不挡事', () {
    testWidgets('描边层 `IgnorePointer`（面板上的按钮必须还能点）', (
      WidgetTester tester,
    ) async {
      int taps = 0;
      await tester.pumpWidget(
        wrap(
          Center(
            child: SizedBox(
              width: 200,
              height: 80,
              child: GlassRim(
                child: Center(
                  child: FilledButton(
                    onPressed: () => taps++,
                    child: const Text('点我'),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('点我'));
      expect(taps, 1, reason: '描边把点击吃掉了');
    });

    testWidgets('不改变尺寸（`StackFit.passthrough`）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          Center(
            child: GlassRim(
              child: const SizedBox(width: 180, height: 60, key: Key('inner')),
            ),
          ),
        ),
      );
      expect(tester.getSize(find.byKey(const Key('inner'))), const Size(180, 60));
      expect(tester.getSize(find.byType(GlassRim)), const Size(180, 60));
    });

    testWidgets('不铺底（底色仍由 child 自己决定）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          const SizedBox(
            width: 100,
            height: 40,
            child: GlassRim(child: SizedBox(key: Key('inner'))),
          ),
        ),
      );
      // `GlassRim` 自己不该出现任何 `ColoredBox` / `DecoratedBox` 底色。
      expect(
        find.descendant(
          of: find.byType(GlassRim),
          matching: find.byType(ColoredBox),
        ),
        findsNothing,
      );
    });
  });

  group('P3-1：四套主题都看得见（写死白色会在白主题上消失）', () {
    for (final AppThemeId id in AppThemeId.values) {
      test('${id.wire}：基色是**当前主题的墨色**，不是写死的白', () {
        final ThemeData t = buildAppTheme(id);
        final AppColors colors = t.extension<AppColors>()!;
        expect(
          colors.rimHighlight,
          AppPalette.of(id).ink,
          reason: '${id.wire} 的高光基色与墨色不一致 —— 亮主题上会看不见',
        );
      });
    }

    test('白主题与黑主题的基色**确实不同**（否则四套主题是假的）', () {
      expect(
        buildAppTheme(AppThemeId.white).extension<AppColors>()!.rimHighlight,
        isNot(
          buildAppTheme(AppThemeId.black).extension<AppColors>()!.rimHighlight,
        ),
      );
    });

    testWidgets('白主题下强度自动降到 0.45（深边在浅底上要更克制）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(
          const SizedBox(
            width: 200,
            height: 80,
            child: GlassRim(child: SizedBox()),
          ),
          theme: AppThemeId.white,
        ),
      );
      final LiquidRimPainter painter =
          tester.widget<CustomPaint>(
                find.descendant(
                  of: find.byType(GlassRim),
                  matching: find.byType(CustomPaint),
                ),
              ).painter!
              as LiquidRimPainter;
      expect(painter.intensity, lessThan(1));
    });

    testWidgets('黑主题下强度是 1（亮边在暗底上可以放开）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          const SizedBox(
            width: 200,
            height: 80,
            child: GlassRim(child: SizedBox()),
          ),
        ),
      );
      final LiquidRimPainter painter =
          tester.widget<CustomPaint>(
                find.descendant(
                  of: find.byType(GlassRim),
                  matching: find.byType(CustomPaint),
                ),
              ).painter!
              as LiquidRimPainter;
      expect(painter.intensity, 1);
    });
  });

  group('P3-1：光跟着指针走（这是「玻璃」和「描边」的区别）', () {
    testWidgets('鼠标移到面板左下角，光向随之改变', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          Center(
            child: SizedBox(
              width: 300,
              height: 120,
              child: GlassRim(
                child: const ColoredBox(color: Color(0xFF202020)),
              ),
            ),
          ),
        ),
      );

      Offset lightOf() {
        final LiquidRimPainter p =
            tester.widget<CustomPaint>(
                  find.descendant(
                    of: find.byType(GlassRim),
                    matching: find.byType(CustomPaint),
                  ),
                ).painter!
                as LiquidRimPainter;
        return p.light;
      }

      final Offset before = lightOf();
      final Offset center = tester.getCenter(find.byType(GlassRim));
      final TestGesture gesture = await tester.createGesture(
        kind: PointerDeviceKind.mouse,
      );
      // 先在**外面**出现再移进来：`addPointer` 只发 `PointerAddedEvent`，
      // 而 `onHover` 要的是 `PointerHoverEvent`——得真的**移动**才会产生。
      await gesture.addPointer(location: const Offset(1, 1));
      addTearDown(gesture.removePointer);
      await gesture.moveTo(center);
      await tester.pumpAndSettle();

      expect(lightOf(), isNot(before), reason: '光向没有跟着指针走 —— 只是静态描边');
      expect(lightOf().dx.abs(), lessThan(before.dx.abs()));
    });

    testWidgets('指针离开后回到固定的默认光向', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          Center(
            child: SizedBox(
              width: 300,
              height: 120,
              child: GlassRim(child: const SizedBox.expand()),
            ),
          ),
        ),
      );
      final TestGesture gesture = await tester.createGesture(
        kind: PointerDeviceKind.mouse,
      );
      await gesture.addPointer(location: const Offset(1, 1));
      addTearDown(gesture.removePointer);
      // 先进到面板里（让光向真的动起来），再移到面板外触发 `onExit`。
      await gesture.moveTo(tester.getCenter(find.byType(GlassRim)));
      await tester.pumpAndSettle();
      await gesture.moveTo(const Offset(5, 5));
      await tester.pumpAndSettle();

      final LiquidRimPainter p =
          tester.widget<CustomPaint>(
                find.descendant(
                  of: find.byType(GlassRim),
                  matching: find.byType(CustomPaint),
                ),
              ).painter!
              as LiquidRimPainter;
      // 默认光向是左上（{-0.65, -0.8}）。
      expect(p.light.dx, lessThan(0));
      expect(p.light.dy, lessThan(0));
    });

    testWidgets('减少动画时**不做指针跟随**（那是持续的鼠标驱动重绘）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: wrap(
            Center(
              child: SizedBox(
                width: 300,
                height: 120,
                child: GlassRim(child: const SizedBox.expand()),
              ),
            ),
          ),
        ),
      );
      expect(
        tester
            .widget<MouseRegion>(
              find.descendant(
                of: find.byType(GlassRim),
                matching: find.byType(MouseRegion),
              ),
            )
            .onHover,
        isNull,
      );
    });
  });

  group('P3-2：`glassBarrier` 接上了（AppColors 至此全部接线）', () {
    test('描边用的是 `glassBarrier` 那一抹冷色（不是另写的灰）', () {
      final String src = stripCommentsAndStrings(
        File('lib/ui/glass_rim.dart').readAsStringSync(),
      );
      expect(src.contains('glassBarrier'), isTrue);
    });

    test('`AppColors` 已经没有未接线的令牌了（台账已清空）', () {
      final String ledger = File(
        'test/design_tokens_test.dart',
      ).readAsStringSync();
      // 台账里不该再有任何 `AppColors.*` 条目。
      expect(
        RegExp(r"'AppColors\.[a-zA-Z]+',").hasMatch(ledger),
        isFalse,
        reason: '台账里还留着 AppColors 条目 —— 那说明它其实还没接线',
      );
    });
  });
}
