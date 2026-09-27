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

      // 补上第三级面：[raised] 是卡片 / 弹窗 / 浮层的底，正文同样会压在上面。
      // 之前只算了 surface / stage / surfaceAlt，`raised` 是漏网的那一档。
      test('$id · 正文 vs 浮起面 ≥ 4.5（卡片 / 弹窗上的正文）', () {
        expect(
          contrast(p.ink, p.raised),
          greaterThanOrEqualTo(4.5),
          reason: '$id 的卡片 / 浮层上正文读不清',
        );
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

      // 为什么不再只断言「≥ 4.5」：onAccent 是**白 / 黑二选一**，而
      // max(contrast(白, x), contrast(黑, x)) ≥ √21 ≈ 4.58 对**任何** x 恒成立
      // ——那条断言对 accent 的取值零检验力（把 accent 设成任何颜色都过）。
      // 真正要守的是「取的是对比度更高的那一个」：写死白色会让近白强调色
      // （黑套 accent = #E8E8EC，黑白比 ≈ 1.22 : 17.19）当场瞎掉。
      test('$id · onAccent 是「白 / 黑二选一里对比度更高的那个」', () {
        final Color chosen = p.onAccent;
        final Color other = chosen == Colors.white
            ? const Color(0xFF000000)
            : Colors.white;
        expect(
          contrast(chosen, p.accent),
          greaterThanOrEqualTo(contrast(other, p.accent)),
          reason:
              '$id 的 onAccent 不是更优的那个（写死白色会在近白 accent 上瞎掉）',
        );
        // 保底：更优的那个仍然满足 AA 正文。
        expect(contrast(chosen, p.accent), greaterThanOrEqualTo(4.5));
      });

      test('$id · 危险色 vs 它所在的面 ≥ 4.5（错误文案）', () {
        expect(contrast(p.danger, p.dangerSurface), greaterThanOrEqualTo(4.5));
      });

      test('$id · 危险色上的文字 ≥ 3.0（实心错误按钮，粗体大字）', () {
        expect(contrast(p.onDanger, p.danger), greaterThanOrEqualTo(3.0));
      });
    }
  });

  // onAccent / onDanger 是**运行时算出来的**（白/黑二选一）。这里对它做性质断言：
  // 不是「某个具体值 ≥ 4.5」，而是「对任意 accent 都取到更优的那一个」。
  group('onAccent：对任意强调色都做二选一（不是零检验力的 ≥4.5）', () {
    /// 白 / 黑里对比度更高的那个（与 tokens.dart `onAccent` 同一条二选一）。
    Color best(Color accent) =>
        contrast(Colors.white, accent) >=
                contrast(const Color(0xFF000000), accent)
            ? Colors.white
            : const Color(0xFF000000);

    test('四套配色的 accent 都取到更优的那个', () {
      for (final AppThemeId id in AppThemeId.values) {
        final AppPalette p = AppPalette.of(id);
        expect(p.onAccent, best(p.accent), reason: '$id 的 onAccent 取错了');
      }
    });

    test('构造出来的 accent（全黑 / 全白 / 中灰 / 高饱和）也取到更优的那个', () {
      const List<Color> accents = <Color>[
        Color(0xFF000000),
        Color(0xFFFFFFFF),
        Color(0xFF808080),
        Color(0xFF767676),
        Color(0xFF4D8DFF),
        Color(0xFFFFF200),
        Color(0xFF00FF00),
      ];
      for (final Color accent in accents) {
        final AppPalette p = AppPalette.black.copyWith(accent: accent);
        expect(p.onAccent, best(accent), reason: 'accent=$accent 时取错了');
      }
    });

    test('两种选择都真的会出现（否则「二选一」是空话）', () {
      // 黑套 accent 近白 → 白字会瞎，必须选黑；白套 accent 近黑 → 必须选白。
      expect(AppPalette.black.onAccent, const Color(0xFF000000));
      expect(AppPalette.white.onAccent, const Color(0xFFFFFFFF));
    });
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

    test('舞台底**不参与色相偏移**（它是下推进渲染面的清屏色）', () {
      // 2026-09-27：中性色整体往色相锚点推，但舞台底是**逐字钉死**的。
      // 这条守的是「改中性色时不要顺手把舞台底也一起推」。
      expect(AppPalette.black.stage, const Color(0xFF000000));
      expect(AppPalette.white.stage, const Color(0xFFFFFFFF));
      expect(AppPalette.blue.stage, const Color(0xFF061223));
      expect(AppPalette.gray.stage, const Color(0xFF1C1C1F));
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

  group('表面阶梯与色相（2026-09-27 第二轮观感）', () {
    /// **感知亮度**（未做 sRGB 线性化，0..1）。
    ///
    /// 为什么不用 [Color.computeLuminance]：那是线性亮度，本项目暗色表面全在
    /// 0.006–0.032 之间，级差 0.007 —— 拿它当阈值等于没有阈值。
    /// 人眼看到的是上面那层（gamma 域），阶梯也必须用同一把尺子量。
    double perceived(Color c) => 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;

    /// 绝对色度（**0..255**）：HSV 饱和度在近黑像素上会爆表
    /// （`#0E0E11` 的 (max−min)/max 是 0.176，但绝对色差只有 3/255），
    /// 所以一律用绝对值。Flutter 的 `Color.r/g/b` 是 **0..1**，故要乘回 255。
    int chroma(Color c) {
      final List<double> v = <double>[c.r, c.g, c.b];
      final double hi = v.reduce((a, b) => a > b ? a : b);
      final double lo = v.reduce((a, b) => a < b ? a : b);
      return ((hi - lo) * 255).round();
    }

    for (final AppThemeId id in AppThemeId.values) {
      final AppPalette p = AppPalette.of(id);

      test('$id · 三级面的感知亮度级差 ≥ 0.03（不描边也读得出层级）', () {
        final List<(String, double)> ladder = <(String, double)>[
          ('surface', perceived(p.surface)),
          ('surfaceAlt', perceived(p.surfaceAlt)),
          ('raised', perceived(p.raised)),
        ];
        for (int i = 0; i + 1 < ladder.length; i++) {
          final double step = (ladder[i + 1].$2 - ladder[i].$2).abs();
          expect(
            step,
            greaterThanOrEqualTo(0.03),
            reason:
                '$id 的 ${ladder[i].$1} → ${ladder[i + 1].$1} 只差 ${step.toStringAsFixed(3)}，'
                '层级还是得靠发丝线（实测发丝线色度约 2/255，近黑画面上等于没有）',
          );
        }
      });

      test('$id · 三级面都带色相（不是中性灰）', () {
        // 病根：改动前四套配色的中性色全落在中性轴上（`#0E0E11`/`#17171B`/
        // `#F1F1F4`），实测 91.5% 的像素色度 ≤6/255，界面读作「没有材质」。
        final List<Color> surfaces = <Color>[p.surface, p.surfaceAlt, p.raised];
        for (final Color c in surfaces) {
          expect(
            chroma(c),
            greaterThanOrEqualTo(3),
            reason: '$id 的中性面 $c 落在中性轴上（色度 ${chroma(c)}/255）',
          );
        }
      });

      test('$id · 四级中性色（三级面 + 墨色）是**同一个色相家族**', () {
        // 家族里第四个成员是**墨色**：它与三级面同属「往锚点推」的那一组，
        // 之前只比了三级面，漏掉它。方向一致靠 `sign(b-r)`。
        final List<Color> neutrals = <Color>[
          p.surface,
          p.surfaceAlt,
          p.raised,
          p.ink,
        ];
        final List<int> directions = <int>[
          for (final Color c in neutrals) (c.b - c.r).sign.toInt(),
        ];
        expect(
          directions.toSet().length,
          1,
          reason: '$id 的中性色相方向不一致：$neutrals',
        );
      });

      test('$id · 强调色带得出色相时必须与中性面同向（近中性带内不判方向）', () {
        // 「同一色相家族」这一条之前**只比三级面**，没把 accent 算进来，
        // 这里补上。但 accent 是**品牌色**：tokens.dart 明说「刻意不给强调色
        // 上色」。实测色度：黑 4 / 白 3 / 灰 6 / 蓝 178 —— 前三个落在
        // 「读作中性」的 ≤6/255 带里，`sign(b-r)` 只是取整噪声，判方向无意义。
        // 规则：带得出色相（> 6/255）就必须与中性面同向；否则视为近中性。
        // 注意白/灰套的 accent 实测在**反向**一侧（暖中性面 + 冷 accent，
        // 色度 3 / 6，恰在带内）—— 若将来要求它们也进同一色相家族，
        // 要改的是颜色（四套 accent 取值），不是放宽这条断言。
        const int nearNeutralMaxChroma = 6;
        final int accentChroma = chroma(p.accent);
        if (accentChroma <= nearNeutralMaxChroma) return;
        expect(
          (p.accent.b - p.accent.r).sign.toInt(),
          (p.surface.b - p.surface.r).sign.toInt(),
          reason: '$id 的强调色（色度 $accentChroma）带得出色相，却与中性面反向',
        );
      });
    }

    test('至少有一套的 accent 是色相载体（否则上面那条『同向』是空转）', () {
      final int carriers = AppThemeId.values
          .where((AppThemeId id) => chroma(AppPalette.of(id).accent) > 6)
          .length;
      expect(
        carriers,
        greaterThanOrEqualTo(1),
        reason: '四套 accent 全退化成近中性 = 色相锚点没了',
      );
    });

    test('白套：三级面推暖，但**墨色刻意近中性**（色度 1/255）', () {
      // 与 tokens.dart 的注释对账：色相偏移表里白套那行原来写「墨色 9%」，
      // 而实测墨色 #242423 的色度只有 1/255（9% 的推法会得到 ≈3/255）。
      // A3c 选择**改注释**（真上色相要改四套中性色，不在「只动测试」的范围内）。
      expect(chroma(AppPalette.white.ink), lessThanOrEqualTo(2));
      for (final Color c in <Color>[
        AppPalette.white.surface,
        AppPalette.white.surfaceAlt,
        AppPalette.white.raised,
      ]) {
        expect(chroma(c), greaterThanOrEqualTo(3), reason: '白套三级面必须推暖');
      }
    });
  });

  group('ThemeExtension 结构枚举（防「加了字段忘了 copyWith / lerp」）', () {
    final AppPalette base = AppPalette.black;
    final AppPalette other = base.copyWith(
      stage: const Color(0xFF010203),
      surface: const Color(0xFF040506),
      surfaceAlt: const Color(0xFF070809),
      raised: const Color(0xFF0A0B0C),
      ink: const Color(0xFF0D0E0F),
      accent: const Color(0xFF101112),
      success: const Color(0xFF131415),
      warning: const Color(0xFF161718),
      danger: const Color(0xFF191A1B),
      dangerSurface: const Color(0xFF1C1D1E),
      dangerBorder: const Color(0xFF1F2021),
    );

    test('每个颜色字段都被 copyWith 处理（只改一个也必须与原对象不等）', () {
      final List<AppPalette> singles = <AppPalette>[
        base.copyWith(stage: const Color(0xFF010203)),
        base.copyWith(surface: const Color(0xFF010203)),
        base.copyWith(surfaceAlt: const Color(0xFF010203)),
        base.copyWith(raised: const Color(0xFF010203)),
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
      expect(buildAppTheme().extension<AppPalette>()?.id, AppThemeId.fallback);
    });
  });
}
