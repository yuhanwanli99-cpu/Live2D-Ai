import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 四套配色的**可测约束**。
///
/// # 为什么对比度要写成测试
///
/// 「黑/白/蓝/灰四色切换」最典型的翻车方式不是「颜色不好看」，而是
/// **某套主题上某段文字读不出来**——尤其是亮色主题：把原来 dark-only 的
/// `#FF6B6B` 直接搬到白底上，对比度只有 2.5:1，肉眼第一眼「还行」，
/// 长时间看就是「不舒服」。
///
/// 这种事只能算，不能看。所以这里对每套主题断言：
/// 文字与它所在的面 ≥ 4.5:1（WCAG AA 正文），非文字元件 ≥ 3:1。
void main() {
  /// WCAG 相对对比度。
  double contrast(Color a, Color b) {
    final double x = a.computeLuminance();
    final double y = b.computeLuminance();
    return ((x > y ? x : y) + 0.05) / ((x > y ? y : x) + 0.05);
  }

  /// 半透明前景压在背景上的**实际**颜色。
  Color composite(Color foreground, Color background) =>
      Color.alphaBlend(foreground, background);

  group('配色族：四套都在，且与枚举一一对应', () {
    test('registry 覆盖全部 AppThemeId，没有多也没有少', () {
      expect(
        AppPalette.registry.keys.toSet(),
        AppThemeId.values.toSet(),
        reason: '新增主题必须同时加配色，否则 of() 会静默回落到黑色',
      );
    });

    test('每套配色的 id 与它在表里的键一致（防复制粘贴改漏）', () {
      AppPalette.registry.forEach((AppThemeId id, AppPalette p) {
        expect(p.id, id, reason: '$id 的配色里 id 写成了 ${p.id}');
      });
    });

    test('of() 对每个 id 都返回对应配色，未知 id 回落默认（不抛）', () {
      for (final AppThemeId id in AppThemeId.values) {
        expect(AppPalette.of(id).id, id);
      }
    });

    test('默认主题是黑（用户裁决「保留黑色」）', () {
      expect(AppThemeId.fallback, AppThemeId.black);
      expect(AppPalette.of(AppThemeId.fallback).id, AppThemeId.black);
    });
  });

  group('AppThemeId 的存储解析：永不抛', () {
    test('四个 wire 值都能往返', () {
      for (final AppThemeId id in AppThemeId.values) {
        expect(AppThemeId.fromWire(id.wire), id);
      }
    });

    test('坏值 / null / 非字符串 → 默认', () {
      for (final Object? bad in <Object?>[null, '', 'BLACK', 3, true, '紫']) {
        expect(AppThemeId.fromWire(bad), AppThemeId.fallback);
      }
    });

    test('wire 值两两不同（否则往返会撞车）', () {
      final Set<String> wires = AppThemeId.values
          .map((AppThemeId e) => e.wire)
          .toSet();
      expect(wires.length, AppThemeId.values.length);
    });

    test('label 两两不同且非空（分段控件上要能分辨）', () {
      final Set<String> labels = AppThemeId.values
          .map((AppThemeId e) => e.label)
          .toSet();
      expect(labels.length, AppThemeId.values.length);
      for (final AppThemeId e in AppThemeId.values) {
        expect(e.label, isNotEmpty);
        expect(e.hint, isNotEmpty);
      }
    });
  });

  group('对比度：每套主题都算过，不是靠眼睛', () {
    for (final AppThemeId id in AppThemeId.values) {
      final AppPalette p = AppPalette.of(id);

      test('$id · 正文 vs 面板 ≥ 4.5（AA 正文）', () {
        expect(
          contrast(p.ink, p.surface),
          greaterThanOrEqualTo(4.5),
          reason: '$id 上正文读不清',
        );
      });

      test('$id · 正文 vs 舞台 ≥ 4.5', () {
        expect(contrast(p.ink, p.stage), greaterThanOrEqualTo(4.5));
      });

      test('$id · 正文 vs 次级面 ≥ 4.5（气泡里也是正文）', () {
        expect(contrast(p.ink, p.surfaceAlt), greaterThanOrEqualTo(4.5));
      });

      test('$id · 次要文本（74% 墨色压在面板上）≥ 4.5', () {
        // `AppColors.contentMuted` 是「承载文字信息」的次要文本，
        // 它是一条透明度而不是实色 —— 所以必须**合成之后**再算对比度。
        final Color muted = composite(p.ink.withValues(alpha: 0.74), p.surface);
        expect(contrast(muted, p.surface), greaterThanOrEqualTo(4.5));
      });

      test('$id · 强调色 vs 面板 ≥ 3.0（焦点环是非文字元件）', () {
        expect(contrast(p.accent, p.surface), greaterThanOrEqualTo(3.0));
      });

      test('$id · 强调色上的文字 ≥ 4.5（按钮文字）', () {
        expect(contrast(p.onAccent, p.accent), greaterThanOrEqualTo(4.5));
      });

      test('$id · 危险色 vs 它所在的面 ≥ 4.5（错误文案）', () {
        expect(contrast(p.danger, p.dangerSurface), greaterThanOrEqualTo(4.5));
      });

      test('$id · 危险色上的文字 ≥ 3.0（实心错误按钮，粗体大字）', () {
        expect(contrast(p.onDanger, p.danger), greaterThanOrEqualTo(3.0));
      });
    }
  });

  group('舞台底：纯色平面（用户裁决「中央不要放贴图」）', () {
    test('每套的舞台底都是**完全不透明**的纯色', () {
      for (final AppThemeId id in AppThemeId.values) {
        final Color stage = AppPalette.of(id).stage;
        expect(stage.a, 1.0, reason: '$id 的舞台底是半透明的——会透出下层');
      }
    });

    test('黑=纯黑、白=纯白（逐字对应用户说的「全黑/全白即可」）', () {
      expect(AppPalette.black.stage, const Color(0xFF000000));
      expect(AppPalette.white.stage, const Color(0xFFFFFFFF));
    });

    test('四套舞台底两两不同（否则「切换主题」看起来没反应）', () {
      final Set<Color> stages = AppThemeId.values
          .map((AppThemeId id) => AppPalette.of(id).stage)
          .toSet();
      expect(stages.length, AppThemeId.values.length);
    });

    test('stageCss 是渲染面认得的 #rrggbb（补零 / 小写 / 不带 alpha）', () {
      for (final AppThemeId id in AppThemeId.values) {
        final String css = AppPalette.of(id).stageCss;
        expect(
          RegExp(r'^#[0-9a-f]{6}$').hasMatch(css),
          isTrue,
          reason: '$id 的 stageCss=$css 不是渲染面接受的形状（会被校验拒掉）',
        );
      }
      // 逐字对照用户的「全黑/全白即可」，同时也守住「补零」这一步
      // （漏了补零会得到 `#0` 而不是 `#000000`）。
      expect(AppPalette.black.stageCss, '#000000');
      expect(AppPalette.white.stageCss, '#ffffff');
    });
  });

  group('ThemeExtension 结构枚举（防「加了字段忘了 copyWith / lerp」）', () {
    final AppPalette base = AppPalette.black;
    final AppPalette other = base.copyWith(
      stage: const Color(0xFF010203),
      surface: const Color(0xFF040506),
      surfaceAlt: const Color(0xFF070809),
      ink: const Color(0xFF0A0B0C),
      accent: const Color(0xFF0D0E0F),
      success: const Color(0xFF101112),
      warning: const Color(0xFF131415),
      danger: const Color(0xFF161718),
      dangerSurface: const Color(0xFF191A1B),
      dangerBorder: const Color(0xFF1C1D1E),
    );

    test('每个颜色字段都被 copyWith 处理（只改一个也必须与原对象不等）', () {
      final List<AppPalette> singles = <AppPalette>[
        base.copyWith(stage: const Color(0xFF010203)),
        base.copyWith(surface: const Color(0xFF010203)),
        base.copyWith(surfaceAlt: const Color(0xFF010203)),
        base.copyWith(ink: const Color(0xFF010203)),
        base.copyWith(accent: const Color(0xFF010203)),
        base.copyWith(success: const Color(0xFF010203)),
        base.copyWith(warning: const Color(0xFF010203)),
        base.copyWith(danger: const Color(0xFF010203)),
        base.copyWith(dangerSurface: const Color(0xFF010203)),
        base.copyWith(dangerBorder: const Color(0xFF010203)),
      ];
      expect(singles.length, AppPalette.colorFieldCount);
      for (final AppPalette c in singles) {
        expect(c, isNot(base), reason: '有字段被 copyWith 漏掉了');
      }
    });

    test('toValuesMap 的字段数与 colorFieldCount 一致', () {
      expect(base.toValuesMap().length, AppPalette.colorFieldCount);
    });

    test('lerp 端点正确，且中点不等于端点', () {
      expect(base.lerp(other, 0.0), base);
      expect(base.lerp(other, 1.0), other);
      final AppPalette mid = base.lerp(other, 0.5);
      expect(mid, isNot(base));
      expect(mid, isNot(other));
    });

    test('lerp 逐字段真的插值了', () {
      final AppPalette mid = base.lerp(other, 0.5);
      final Map<String, Object?> a = base.toValuesMap();
      final Map<String, Object?> b = other.toValuesMap();
      final Map<String, Object?> m = mid.toValuesMap();
      expect(m.keys, a.keys);
      for (final String k in a.keys) {
        expect(
          m[k],
          Color.lerp(a[k]! as Color, b[k]! as Color, 0.5),
          reason: '$k 的 lerp 没生效',
        );
      }
    });
  });

  group('注册侧：主题里真的生效了吗', () {
    test('每套主题都注册了配色，且只注册一份', () {
      for (final AppThemeId id in AppThemeId.values) {
        final ThemeData t = buildAppTheme(id);
        expect(t.extension<AppPalette>()?.id, id);
        expect(t.extensions.values.whereType<AppPalette>().length, 1);
      }
    });

    test('ColorScheme 的关键槽位等于配色真值（不是 fromSeed 的近似值）', () {
      for (final AppThemeId id in AppThemeId.values) {
        final AppPalette p = AppPalette.of(id);
        final ColorScheme s = buildAppTheme(id).colorScheme;
        expect(s.primary, p.accent, reason: '$id 的 primary 漂了');
        expect(s.onPrimary, p.onAccent);
        expect(s.surface, p.surface);
        expect(s.onSurface, p.ink);
        expect(s.surfaceContainerHighest, p.surfaceAlt);
        expect(s.error, p.danger);
      }
    });

    test('buildAppTheme() 不带参数 = 默认黑（调用方不需要知道默认是哪个）', () {
      expect(
        buildAppTheme().extension<AppPalette>()?.id,
        AppThemeId.fallback,
      );
    });
  });
}
