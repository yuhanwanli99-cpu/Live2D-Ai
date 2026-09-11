import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/design/typography.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
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

/// 统计 `lib/**` 里对令牌的**真实引用**次数。
///
/// # 为什么不能只数 `Family.name`（2026-09-11 修的假阴性）
///
/// `AppColors` 是 `ThemeExtension`，**没有任何一处代码写得出 `AppColors.hairline`**
/// ——它必须从 context 取。真实写法只有两种：
///
/// ```dart
/// appColorsOf(context).hairline            // ① 就地取
/// final AppColors colors = appColorsOf(context);  … colors.hairline   // ② 先绑局部
/// ```
///
/// 旧实现只匹配 `AppColors.hairline`，于是这 9 个令牌的引用数**永远是 0**：
/// 「声明↔引用双向对账」在那条路径上恒真，`kNotYetWired` 台账里
/// 已经接线的条目也永远不会被判成过期——**门禁在假装工作**。
///
/// 本实现同时识别三种形式：
/// 1. `Family.name`（常量类令牌，如 `Space.s3` / `AppDurations.base`）；
/// 2. `appColorsOf(...).name`（就地取）；
/// 3. `AppColors alias = …` 之后出现的 `alias.name`（先绑局部）。
///
/// 形式 3 靠**类型标注**识别别名，不做类型推断——只扫 `lib/**`，
/// 且别名写法在本仓库只有 `colors` 一种，精确够用。
Map<String, int> countTokenReferences(String root, List<String> families) {
  final RegExp direct = RegExp(
    '\\b(${families.join('|')})\\.([A-Za-z][A-Za-z0-9_]*)',
  );
  final RegExp accessor = RegExp(
    r'AppColors\s*Of\s*\([^)]*\)\s*\.\s*([A-Za-z][A-Za-z0-9_]*)',
    caseSensitive: false,
  );
  final RegExp aliasDecl = RegExp(r'\bAppColors\s+([a-z_][A-Za-z0-9_]*)\b');
  final Map<String, int> counts = <String, int>{};
  void bump(String key) => counts[key] = (counts[key] ?? 0) + 1;
  for (final FileSystemEntity entity in Directory(root).listSync(
    recursive: true,
  )) {
    if (entity is! File || !entity.path.endsWith('.dart')) continue;
    final String src = stripCommentsAndStrings(entity.readAsStringSync());
    for (final RegExpMatch m in direct.allMatches(src)) {
      bump('${m.group(1)}.${m.group(2)}');
    }
    for (final RegExpMatch m in accessor.allMatches(src)) {
      bump('AppColors.${m.group(1)}');
    }
    final Set<String> aliases = <String>{
      for (final RegExpMatch m in aliasDecl.allMatches(src)) m.group(1)!,
    };
    for (final String alias in aliases) {
      final RegExp aliasUse = RegExp('\\b$alias\\.([A-Za-z][A-Za-z0-9_]*)');
      for (final RegExpMatch m in aliasUse.allMatches(src)) {
        bump('AppColors.${m.group(1)}');
      }
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
  'AppRhythms',
  'Motion',
  'Breakpoints',
  'AppColors',
];

/// 每个家族**面向 UI 的令牌名**。
Map<String, Set<String>> tokenNamesByFamily() => <String, Set<String>>{
  'AppRadius': AppRadius.registry.keys.toSet(),
  'Space': Space.registry.keys.toSet(),
  'AppDurations': AppDurations.registry.keys.toSet(),
  'AppRhythms': AppRhythms.registry.keys.toSet(),
  'Motion': Motion.names.toSet(),
  'Breakpoints': Breakpoints.registry.keys.toSet(),
  'AppColors': AppColors.of(_scheme, AppPalette.black).toValuesMap().keys.toSet(),
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
  // ── 叠色 ──
  //
  // **2026-09-11 修正**：这里原本列了 9 个 `AppColors.*`，但其中 7 个
  // （hairline / hoverWash / glassScrim / contentMuted / contentFaint /
  // serverMutedBadgeSurface / serverMutedBadgeBorder）**其实早就接线了**。
  // 它们之所以一直显示「未接线」，是因为 `countTokenReferences` 只认
  // `AppColors.hairline` 这种写不出来的形式（该类型必须从 context 取）。
  // 尺子已修（见该函数头注），台账随之只减不增。
  //
  // **2026-09-11 再修正**：`focusRing` 已在 P1-4 接到 `buildAppComponents`
  // 的 `focusSide()` 上；最后剩下的 `glassBarrier` 也在 P3-2 接到了
  // `GlassRim` 的冷色段上。**`AppColors` 至此全部接线，台账清空。**
  // ── 间距 / 圆角 ──
  //
  // **2026-09-11（P4-2）再减三条**：`Space.s6`(24) / `s7`(32) / `s8`(48)
  // 过去「留着等 UI 用到」，其实一直有地方在用裸数字写同样的值。
  // 这一轮把 `EdgeInsets` 里的裸数字全部归位，它们自然就被引用了——
  // 顺带新增了「裸间距」扫描规则（见 `design_tokens_lint_test.dart`）。
  //
  // 台账**只减不增**：接线了就必须从这里删掉（P2 已删 s2/s3/s5，
  // P3 已删 s1/s4、AppRadius.xs/lg）。
  'Space.s0',
  'AppRadius.none',
  // ── 动效：**2026-09-11 已全部接线**，从台账里删掉 ──
  //
  // `AppDurations.base/slow/reveal` 与 `Motion.enter` 曾长期挂在这里，
  // 因为那时全应用只有一处 `AnimatedContainer`——令牌齐全、接线为零。
  // 前端加强计划 P1-0/P1-1（`lib/ui/soft_motion.dart` +
  // `lib/app/collapsible_panel.dart`）把它们接上了：
  // `appMotion(context, AppDurations.xxx)` 是 now 唯一出口。
  //
  // 2026-09-11（P1-2）：`AppDurations.reveal` 已接到 `StartupReveal`
  // （`lib/ui/soft_motion.dart`）——外壳首帧淡入一次，把舞台 iframe 的
  // 空白首帧挡在背后。4 档时长至此**全部接线**。
};

void main() {
  group('声明侧自洽', () {
    test('每个家族的登记表长度与预期一致（新增档位必须显式改测试）', () {
      expect(AppRadius.registry.length, 7);
      expect(Space.registry.length, 9);
      expect(AppDurations.registry.length, 4);
      expect(AppRhythms.registry.length, 3);
      expect(Motion.names.length, 2);
      expect(AppFontSizes.registry.length, 8);
      expect(Breakpoints.registry.length, 2);
      // 四套配色（2026-09-11）。`AppPalette` 不再是「平的令牌表」，
      // 所以它退出了 kTokenFamilies —— 它的结构由下面「配色族」那组测试守。
      expect(AppPalette.registry.length, AppThemeId.values.length);
      // 2026-09-11（P3-1）：+`rimHighlight`（玻璃边缘高光的基色）。
      expect(AppColors.fieldCount, 10);
    });

    test('尺寸/时长类家族的取值两两互异（防「四档同值」这种没有信息量的令牌）', () {
      // 注意**不含** `AppFontSizes.sizesOf`：规格 §2.3 自己的表里
      // `bodyMedium` 与 `titleSmall` 同为 14 px（w400 正文 vs w600 小标题），
      // 这是有意的（同字号、不同字重 = 不同角色）。字号阶梯的判据见下一条。
      for (final Map<String, Object> reg in <Map<String, Object>>[
        AppRadius.registry,
        Space.registry,
        AppDurations.registry,
        AppRhythms.registry,
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
    final AppColors base = AppColors.of(_scheme, AppPalette.black);
    final AppColors other = base.copyWith(
      hairline: const Color(0xFF010203),
      hoverWash: const Color(0xFF040506),
      glassScrim: const Color(0xFF070809),
      glassBarrier: const Color(0xFF0A0B0C),
      contentMuted: const Color(0xFF0D0E0F),
      contentFaint: const Color(0xFF101112),
      focusRing: const Color(0xFF131415),
      rimHighlight: const Color(0xFF1A1B1C),
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
        base.copyWith(rimHighlight: const Color(0xFF010203)),
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

    test('主题亮度与配色一致（四套各自成立，不是全局写死 dark）', () {
      for (final AppThemeId id in AppThemeId.values) {
        final ThemeData t = buildAppTheme(id);
        expect(
          t.brightness,
          AppPalette.of(id).brightness,
          reason: '$id 的主题亮度与配色不符',
        );
        expect(t.useMaterial3, isTrue);
        expect(
          t.scaffoldBackgroundColor,
          AppPalette.of(id).stage,
          reason: '$id 的脚手架底必须等于它的舞台底（否则面板与舞台之间会有缝）',
        );
      }
    });
  });
}

final ColorScheme _scheme = ColorScheme.fromSeed(
  seedColor: AppPalette.black.accent,
  brightness: Brightness.dark,
);
