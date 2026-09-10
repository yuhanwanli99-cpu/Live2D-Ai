import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/design/typography.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

// ─────────────────────────────────────────────────────────────────────────────
// 扫描基础设施
// ─────────────────────────────────────────────────────────────────────────────

/// 剥掉注释与字符串字面量——**必须先剥再扫**，否则注释里提到的令牌名会被
/// 当成引用（本文件自己的注释就大量提到它们）。
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
      out.write('S');
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

/// 统计 `lib/**` 里 `Family.name` 形式的引用次数。
Map<String, int> countTokenReferences(String root, List<String> families) {
  final RegExp pattern = RegExp(
    '\\b(${families.join('|')})\\.([A-Za-z][A-Za-z0-9_]*)',
  );
  final Map<String, int> counts = <String, int>{};
  for (final FileSystemEntity entity in Directory(root).listSync(
    recursive: true,
  )) {
    if (entity is! File || !entity.path.endsWith('.dart')) continue;
    final String src = stripCommentsAndStrings(entity.readAsStringSync());
    for (final RegExpMatch m in pattern.allMatches(src)) {
      final String key = '${m.group(1)}.${m.group(2)}';
      counts[key] = (counts[key] ?? 0) + 1;
    }
  }
  return counts;
}

/// 参与双向对账的家族。
///
/// **`AppFontSizes` 刻意不在其中**：它的槽位名（`labelSmall` …）是
/// `TextTheme` 的槽位名（字符串），**不是 Dart 标识符**，没法写成
/// `AppFontSizes.labelSmall`。它的「声明 ↔ 生效」由下面「注册侧」那组测试对账。
const List<String> kTokenFamilies = <String>[
  'AppRadius',
  'Space',
  'AppDurations',
  'Motion',
  'AppPalette',
  'Breakpoints',
  'AppColors',
];

/// 每个家族**面向 UI 的令牌名**。
Map<String, Set<String>> tokenNamesByFamily() => <String, Set<String>>{
  'AppRadius': AppRadius.registry.keys.toSet(),
  'Space': Space.registry.keys.toSet(),
  'AppDurations': AppDurations.registry.keys.toSet(),
  'Motion': Motion.names.toSet(),
  'AppPalette': AppPalette.registry.keys.toSet(),
  'Breakpoints': Breakpoints.registry.keys.toSet(),
  'AppColors': AppColors.of(_scheme).toValuesMap().keys.toSet(),
};

/// 家族的**结构成员**（是 API，但不是令牌）：登记表、派生视图、构造入口等。
/// 它们不参与「死令牌」判定。
const Set<String> kStructuralMembers = <String>{
  'registry',
  'sizesOf',
  'sizeOfSlot',
  'weightOfSlot',
  'names',
  'of',
  'fieldCount',
  'sizeClassOf',
  'toValuesMap',
};

/// **未接线令牌台账**（诚实记录，逐阶段清空）。
///
/// P0 只落「地基」：令牌已声明，但消费它们的组件要到 P3/P4/P5 才重写。
/// 所以「声明了但还没人用」在这一阶段是**预期状态**，不是缺陷——但必须
/// **显式登记**，这样：① 新加的死令牌混不进来（不在台账里就必须被引用）；
/// ② 每个后续阶段删掉对应条目，**台账只减不增、进度可见**。
///
/// 规格 §2.8.2 原本要求「声明即被引用」，与它自己的分阶段路线（P0 立令牌、
/// P3 才用）冲突；本台账是两者的调和。
const Set<String> kNotYetWired = <String>{
  // ── 品牌与语义色：P3 重写聊天面板 / 错误条时接线 ──
  // （`AppPalette.warning` 不在台账里：`AppColors.of` 已经用它派生静音徽标色。）
  // ── 叠色：P3 起用（GlassPanel / 分隔线 / 悬停 / 焦点环） ──
  'AppColors.hairline',
  'AppColors.hoverWash',
  'AppColors.glassScrim',
  'AppColors.glassBarrier',
  'AppColors.contentMuted',
  'AppColors.contentFaint',
  'AppColors.focusRing',
  'AppColors.serverMutedBadgeSurface',
  'AppColors.serverMutedBadgeBorder',
  // ── 间距 / 圆角 / 动效：业务组件重写时才用 ──
  // （`Space.s1` 与 `Space.s2/s3/s5` 不在台账里：动作来源徽标与动作日志视图
  //   P2 已接线——台账只减不增，接线了就必须从这里删掉。）
  'Space.s0',
  'Space.s4',
  'Space.s7',
  'Space.s8',
  'AppRadius.none',
  'AppRadius.xs',
  'AppRadius.lg',
  'AppRadius.xl',
  'AppDurations.fast',
  'AppDurations.base',
  'AppDurations.slow',
  'AppDurations.reveal',
  'Motion.enter',
  'Motion.state',
};

void main() {
  group('声明侧自洽', () {
    test('每个家族的登记表长度与预期一致（新增档位必须显式改测试）', () {
      expect(AppRadius.registry.length, 7);
      expect(Space.registry.length, 9);
      expect(AppDurations.registry.length, 4);
      expect(Motion.names.length, 2);
      expect(AppFontSizes.registry.length, 8);
      expect(Breakpoints.registry.length, 2);
      expect(AppPalette.registry.length, 9);
      expect(AppColors.fieldCount, 9);
    });

    test('尺寸/时长类家族的取值两两互异（防「四档同值」这种没有信息量的令牌）', () {
      // 注意**不含** `AppFontSizes.sizesOf`：规格 §2.3 自己的表里
      // `bodyMedium` 与 `titleSmall` 同为 14 px（w400 正文 vs w600 小标题），
      // 这是有意的（同字号、不同字重 = 不同角色）。字号阶梯的判据见下一条。
      for (final Map<String, Object> reg in <Map<String, Object>>[
        AppRadius.registry,
        Space.registry,
        AppDurations.registry,
      ]) {
        final List<Object> values = reg.values.toList();
        expect(values.toSet().length, values.length, reason: '存在同值档位');
      }
    });

    test('字号阶梯：没有两个槽位的「字号 + 字重」完全相同（无冗余槽位）', () {
      final Set<String> pairs = AppFontSizes.registry.values
          .map(
            (({double size, FontWeight weight, double lineHeight}) v) =>
                '${v.size}/${v.weight.value}',
          )
          .toSet();
      expect(pairs.length, AppFontSizes.registry.length);
    });

    test('全阶梯只有一个槽位使用 16 px（治旧的「两个 16px 同级标题」病）', () {
      final List<String> at16 = AppFontSizes.sizesOf.entries
          .where((MapEntry<String, double> e) => e.value == 16)
          .map((MapEntry<String, double> e) => e.key)
          .toList();
      expect(at16, <String>['titleMedium']);
    });

    test('中文界面字号下限：没有小于 11 px 的槽位', () {
      for (final MapEntry<String, double> e in AppFontSizes.sizesOf.entries) {
        expect(e.value, greaterThanOrEqualTo(11), reason: '${e.key} 低于可读下限');
      }
    });
  });

  group('引用侧枚举：声明 ↔ 引用双向对账', () {
    final Map<String, int> refs = countTokenReferences('lib', kTokenFamilies);
    final Map<String, Set<String>> families = tokenNamesByFamily();

    test('所有声明的令牌要么被引用，要么在「未接线台账」里', () {
      final List<String> orphans = <String>[];
      families.forEach((String family, Set<String> names) {
        for (final String name in names) {
          final String key = '$family.$name';
          if ((refs[key] ?? 0) == 0 && !kNotYetWired.contains(key)) {
            orphans.add(key);
          }
        }
      });
      expect(
        orphans,
        isEmpty,
        reason:
            '以下令牌既没人引用、也不在 kNotYetWired 台账里 —— '
            '要么接上它，要么删掉它，要么登记进台账：$orphans',
      );
    });

    test('台账不得含有「其实已经被引用」的过期条目（只减不增）', () {
      final List<String> stale = kNotYetWired
          .where((String n) => (refs[n] ?? 0) > 0)
          .toList();
      expect(
        stale,
        isEmpty,
        reason: '以下令牌已经接线了，请从 kNotYetWired 里删掉：$stale',
      );
    });

    test('台账里的名字都真的存在（防改名 / 删档后台账变成陈旧清单）', () {
      final Set<String> declared = <String>{
        for (final MapEntry<String, Set<String>> e in families.entries)
          for (final String n in e.value) '${e.key}.$n',
      };
      final List<String> bogus = kNotYetWired
          .where((String n) => !declared.contains(n))
          .toList();
      expect(bogus, isEmpty, reason: '台账里有不存在的令牌名：$bogus');
    });

    test('被引用的名字要么是令牌、要么是已登记的结构成员', () {
      final Set<String> known = <String>{
        for (final MapEntry<String, Set<String>> e in families.entries)
          for (final String n in e.value) '${e.key}.$n',
        for (final String n in kStructuralMembers)
          for (final String f in kTokenFamilies) '$f.$n',
      };
      final List<String> unknown = refs.keys
          .where((String k) => !known.contains(k))
          .toList();
      expect(
        unknown,
        isEmpty,
        reason: '引用了既不是令牌、也不是结构成员的名字：$unknown',
      );
    });
  });

  group('ThemeExtension 结构枚举（防「加了字段忘了 copyWith / lerp」）', () {
    final AppColors base = AppColors.of(_scheme);
    final AppColors other = base.copyWith(
      hairline: const Color(0xFF010203),
      hoverWash: const Color(0xFF040506),
      glassScrim: const Color(0xFF070809),
      glassBarrier: const Color(0xFF0A0B0C),
      contentMuted: const Color(0xFF0D0E0F),
      contentFaint: const Color(0xFF101112),
      focusRing: const Color(0xFF131415),
      serverMutedBadgeSurface: const Color(0xFF161718),
      serverMutedBadgeBorder: const Color(0xFF191A1B),
    );

    test('每个字段都被 copyWith 处理（只改一个字段也必须与原对象不等）', () {
      final List<AppColors> singles = <AppColors>[
        base.copyWith(hairline: const Color(0xFF010203)),
        base.copyWith(hoverWash: const Color(0xFF010203)),
        base.copyWith(glassScrim: const Color(0xFF010203)),
        base.copyWith(glassBarrier: const Color(0xFF010203)),
        base.copyWith(contentMuted: const Color(0xFF010203)),
        base.copyWith(contentFaint: const Color(0xFF010203)),
        base.copyWith(focusRing: const Color(0xFF010203)),
        base.copyWith(serverMutedBadgeSurface: const Color(0xFF010203)),
        base.copyWith(serverMutedBadgeBorder: const Color(0xFF010203)),
      ];
      expect(singles.length, AppColors.fieldCount);
      for (final AppColors c in singles) {
        expect(c, isNot(base), reason: '有字段被 copyWith 漏掉了');
      }
    });

    test('toValuesMap 的字段数与 fieldCount 一致', () {
      expect(base.toValuesMap().length, AppColors.fieldCount);
    });

    test('lerp 端点正确，且中点不等于端点（防 lerp 直接 return this）', () {
      expect(base.lerp(other, 0.0), base);
      expect(base.lerp(other, 1.0), other);
      final AppColors mid = base.lerp(other, 0.5);
      expect(mid, isNot(base));
      expect(mid, isNot(other));
    });

    test('lerp 逐字段真的插值了（每个字段的中点都等于两端的中点）', () {
      final AppColors mid = base.lerp(other, 0.5);
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
    final ThemeData theme = buildAppTheme();

    test('AppColors 已注册，且只注册一份', () {
      expect(theme.extension<AppColors>(), isNotNull);
      expect(theme.extensions.values.whereType<AppColors>().length, 1);
    });

    test('8 个字号槽位在主题里**实际生效**的值与登记表一致', () {
      for (final String slot in AppFontSizes.registry.keys) {
        final ({double size, FontWeight weight, double lineHeight}) want =
            AppFontSizes.registry[slot]!;
        expect(
          AppFontSizes.sizeOfSlot(theme, slot),
          want.size,
          reason: '$slot 的字号没落到主题上',
        );
        expect(
          AppFontSizes.weightOfSlot(theme, slot),
          want.weight,
          reason: '$slot 的字重没落到主题上',
        );
      }
    });

    test('配色是 dark + Material 3（写死 dark 是刻意的，不是遗漏）', () {
      expect(theme.brightness, Brightness.dark);
      expect(theme.useMaterial3, isTrue);
    });
  });
}

final ColorScheme _scheme = ColorScheme.fromSeed(
  seedColor: AppPalette.seed,
  brightness: Brightness.dark,
);
