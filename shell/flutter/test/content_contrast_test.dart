/// **令牌对比度下限**：`contentFaint` 被当文字色用的那一族（2026-09-28，F-0006-2）。
///
/// # 为什么单开一个文件（而不是塞进 `theme_palette_test.dart`）
///
/// `theme_palette_test.dart` 的对比度清单量的是**实体色令牌**（`ink` / `danger`
/// / `onAccent` …）。这一条量的是另一件事：一个**半透明叠色令牌**被当文字色用时
/// 的真实对比度——必须先把 alpha 合成到「它压在哪个面上」再算，合成面不同结果
/// 也不同。F-0006-2 抓到的正是这个盲区：`contentFaint`（`onSurface @0.60`）
/// 压在白色主题的 `surfaceAlt` 上只有 **3.96**，而清单里**没有**这一项。
///
/// # 这一条的字面量为什么是「算出来的」
///
/// 期望值全部是 WCAG 相对亮度公式的确定值（不是「够用就行」）：
///
/// ```text
/// L(c) = 0.2126R + 0.7152G + 0.0722B,  每个通道先做 sRGB 线性化
/// 比值 = (max(L1,L2) + 0.05) / (min(L1,L2) + 0.05)
/// 合成 = alphaBlend(onSurface@α, 面)
/// ```
///
/// 举一个可手算的核对点：白主题 `ink #242423` 压在 `surfaceAlt #EDECE9` 上，
/// α=0.60 时合成 `#747472`，比值 3.96（= F-0006-2 实测的 3.96）；
/// α=0.68 时合成 `#646462`，比值 5.01。**旧值必须红、新值必须绿**两条都在本文件里。
///
/// 期望值在 `closeTo(…, 0.02)` 内钉死（浮点合成 vs 8 bit 取整会差千分位，
/// 但**不会**差过 0.02——若真差了，说明配色或算术被改过）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// WCAG 相对对比度（与 `theme_palette_test.dart` / `tokens.dart` 同一算法）。
double wcagContrast(Color a, Color b) {
  final double x = a.computeLuminance();
  final double y = b.computeLuminance();
  return ((x > y ? x : y) + 0.05) / ((x > y ? y : x) + 0.05);
}

/// 半透明前景压在背景上的**实际**颜色。
Color over(Color foreground, Color background) =>
    Color.alphaBlend(foreground, background);

/// 白主题的 `ink` —— 用于「旧值判别力自证」（旧令牌值 0.60 已不在源码里，
/// 断言必须能自己把它算出来，否则这条自证会随实现漂移）。
const Color _whiteInk = Color(0xFF242423);

/// 「墨色 @α 压在面上」的比值。`α` 是显式参数：新值走令牌，旧值走这里。
double _alphaOn(Color ink, Color surface, double alpha) =>
    wcagContrast(over(ink.withValues(alpha: alpha), surface), surface);

void main() {
  final AppPalette white = AppPalette.of(AppThemeId.white);
  final AppPalette black = AppPalette.of(AppThemeId.black);
  final AppPalette blue = AppPalette.of(AppThemeId.blue);
  final AppPalette gray = AppPalette.of(AppThemeId.gray);

  /// **真实接线**的令牌（`buildAppTheme` 与产品走同一条构造路径，
  /// 不是测试里重新拼一个 `ColorScheme`——那样 `onSurface` 会漂成 `fromSeed`
  /// 的近似值，量的就不是产品里那个颜色了）。
  AppColors colorsOf(AppThemeId id) => buildAppTheme(id).extension<AppColors>()!;

  /// `contentFaint`（当前令牌值）压在某个面上的比值。
  double faintOn(AppThemeId id, Color surface) =>
      wcagContrast(over(colorsOf(id).contentFaint, surface), surface);

  group('① contentFaint 作为文字色：四套主题 × 三个面都 ≥ 4.5', () {
    for (final AppThemeId id in AppThemeId.values) {
      final AppPalette p = AppPalette.of(id);
      for (final (String name, Color bed) in <(String, Color)>[
        ('surface', p.surface),
        ('surfaceAlt', p.surfaceAlt),
        ('raised', p.raised),
      ]) {
        test('$id · contentFaint vs $name ≥ 4.5（AA 正文）', () {
          final double ratio = faintOn(id, bed);
          expect(
            ratio,
            greaterThanOrEqualTo(4.5),
            reason:
                '$id 的 contentFaint 压在 $name 上只有 '
                '${ratio.toStringAsFixed(3)}:1 —— 它**已经被当文字色用了**'
                '（滑杆刻度 / 空闲相位标签 / 外观摘要），低于 AA 4.5。',
          );
        });
      }
    }
  });

  group('② 期望比值（算术可复算，白 / 黑各一组）', () {
    // 白主题：F-0006-2 实测的 3.96–4.07 就是这里的前两列旧值。
    test('白 · contentFaint vs surface = 5.21、vs surfaceAlt = 5.01、vs raised = 4.76', () {
      expect(faintOn(AppThemeId.white, white.surface), closeTo(5.21, 0.02));
      expect(faintOn(AppThemeId.white, white.surfaceAlt), closeTo(5.01, 0.02));
      expect(faintOn(AppThemeId.white, white.raised), closeTo(4.76, 0.02));
    });

    test('黑 · contentFaint vs surfaceAlt = 7.22（只升不降）', () {
      expect(faintOn(AppThemeId.black, black.surfaceAlt), closeTo(7.22, 0.02));
      expect(faintOn(AppThemeId.black, black.surface), closeTo(7.79, 0.02));
    });

    test('蓝 6.38 / 灰 5.79（对 surfaceAlt，暗色两套也 ≥ 4.5）', () {
      expect(faintOn(AppThemeId.blue, blue.surfaceAlt), closeTo(6.38, 0.02));
      expect(faintOn(AppThemeId.gray, gray.surfaceAlt), closeTo(5.79, 0.02));
    });
  });

  group('③ 判别力自证：旧值 0.60 必须红（否则 ① 是零检验力的）', () {
    // 这段算术**故意**用旧值。它红了不说明代码坏了，说明门槛真的在量东西：
    // 把 tokens.dart 的 0.68 改回 0.60，① 会以完全相同的数字失败。
    test('白 · α=0.60 vs surfaceAlt = 3.96（< 4.5，正是审计实测值）', () {
      final double old = _alphaOn(_whiteInk, white.surfaceAlt, 0.60);
      expect(old, closeTo(3.96, 0.02));
      expect(
        old,
        lessThan(4.5),
        reason: '旧值算出来竟然达标 —— 说明 ① 的阈值对 contentFaint 没有检验力',
      );
    });

    test('白 · α=0.60 vs surface = 4.08、vs raised = 3.80（都不达标）', () {
      expect(_alphaOn(_whiteInk, white.surface, 0.60), closeTo(4.08, 0.02));
      final double raised = _alphaOn(_whiteInk, white.raised, 0.60);
      expect(raised, closeTo(3.80, 0.02));
      expect(raised, lessThan(4.5));
    });

    test('临界值：α=0.66 白主题 raised 上 4.497 —— 仍差 0.003，所以下限取 0.68', () {
      final double edge = _alphaOn(_whiteInk, white.raised, 0.66);
      expect(edge, closeTo(4.50, 0.02));
      expect(edge, lessThan(4.5), reason: '0.66 就够的话我们的下限取得太保守');
    });

    test('当前令牌值确实被抬过（不是「改了断言没改实现」）', () {
      // 4.76 只在 α ≥ 0.68 时才成立；α=0.60 是 3.80。
      expect(faintOn(AppThemeId.white, white.raised), greaterThan(4.5));
    });
  });

  group('④ 层级没被拉平：contentFaint 必须仍弱于 contentMuted', () {
    test('白 · faint(0.68) < muted(0.74)（用「两档同值」修对比度是不允许的）', () {
      final AppColors colors = colorsOf(AppThemeId.white);
      final double faint = wcagContrast(
        over(colors.contentFaint, white.surfaceAlt),
        white.surfaceAlt,
      );
      final double muted = wcagContrast(
        over(colors.contentMuted, white.surfaceAlt),
        white.surfaceAlt,
      );
      expect(faint, lessThan(muted));
      expect(muted, closeTo(6.03, 0.02));
    });
  });

  group('⑤ 接线侧：白主题输入框占位文字（真 ThemeData，不是令牌算术）', () {
    test('hintStyle 压在白主题输入框填色上 ≥ 4.5，且用的是 contentMuted', () {
      final ThemeData theme = buildAppTheme(AppThemeId.white);
      final TextStyle hint = theme.inputDecorationTheme.hintStyle!;
      final AppColors colors = theme.extension<AppColors>()!;
      expect(
        hint.color,
        colors.contentMuted,
        reason: '占位文字是承载信息的文本，不是装饰；应当走「次要文本」那一档',
      );
      expect(hint.color, isNot(colors.contentFaint));

      // 亮色主题下 `fillColor = surfaceAlt`（alpha=1），所以合成面就是它。
      final double ratio = wcagContrast(
        over(hint.color!, AppPalette.white.surfaceAlt),
        AppPalette.white.surfaceAlt,
      );
      expect(ratio, greaterThanOrEqualTo(4.5));
      expect(ratio, closeTo(6.03, 0.02));
    });

    test('空闲相位标签（contentFaint 当 tone）在相位胶囊上 ≥ 4.5', () {
      // state_pill.dart:225 的胶囊底 = `tone.withValues(alpha: 0.10)` 压在 panel 上；
      // 标签字色 = `tone`。这条把「令牌值」与「真实叠层」一起算了。
      for (final AppThemeId id in AppThemeId.values) {
        final AppPalette p = AppPalette.of(id);
        final AppColors colors = colorsOf(id);
        final Color pillBed = over(
          colors.contentFaint.withValues(alpha: 0.10),
          p.surface,
        );
        final double ratio = wcagContrast(
          over(colors.contentFaint, pillBed),
          pillBed,
        );
        expect(
          ratio,
          greaterThanOrEqualTo(4.5),
          reason: '$id 的空闲标签在相位胶囊上只有 ${ratio.toStringAsFixed(3)}:1',
        );
      }
    });
  });
}
