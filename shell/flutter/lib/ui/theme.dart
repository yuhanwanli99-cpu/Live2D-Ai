/// 应用主题：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由与 `settings/display_prefs.dart`、`audio/gain.dart` 相同：
/// 主题里有几个**必须成立、且很容易被无意改坏**的约束（尤其是字体），
/// 埋在 `main.dart` 里就只能靠肉眼守。
///
/// 令牌真源在 `lib/design/`（`tokens.dart` / `typography.dart` / `breakpoints.dart`）；
/// 本文件只负责**把令牌接进 `ThemeData`**。
library;

import 'package:flutter/material.dart';

import '../design/theme_id.dart';
import '../design/tokens.dart';
import '../design/typography.dart';

/// 自托管中文字体的家族名。
///
/// **必须与 `pubspec.yaml` 的 `fonts:` 声明逐字一致**——
/// 这个对应关系由 `test/theme_test.dart` 扫描 pubspec 守住。
const String kAppFontFamily = 'NotoSansSC';

/// 构造应用主题。
///
/// # 为什么字体必须显式指定（而不是靠系统回落）
///
/// Flutter Web 的 CanvasKit **取不到设备字体**，`fontFamilyFallback` 也不会命中
/// 系统字体：遇到未打包的字形，引擎只会去 `https://fonts.gstatic.com/` 下载 Noto。
/// 对一个**本地优先的桌宠**，那等于「断网即豆腐块」。
/// 所以中文字体打包在 `assets/fonts/`（子集见该目录的 README）。
///
/// 注：`ThemeData.fontFamily` 会经 `TextTheme.apply(fontFamily:)` 覆盖掉
/// Material 3 `Typography` 里的 `'Roboto'`，并且在 `ThemeData.localize`
/// 为 CJK（`ScriptCategory.dense`）选取几何主题之后**仍然保留**——
/// 后者正是最容易静默失效的一环，故有专门的回归测试。
///
/// **注意**：[buildAppTextTheme] 产生的 `TextStyle` **不带 fontFamily**
/// （`null`），这是刻意的：`TextStyle.merge` / `copyWith` 对 `null` 是「保持原值」，
/// 于是字体族始终由 `ThemeData.fontFamily` 这**一个**入口决定，
/// 组件层无法意外覆盖它。
///
/// # 主题是怎么构造的（2026-09-11 起支持 4 套配色）
///
/// 先用 [AppPalette] 的强调色跑一次 `ColorScheme.fromSeed`——它能一次性填满
/// Material 的**几十个**槽位（`inverseSurface`、`surfaceTint`、各种 `on*`…），
/// 比自己手写一遍可靠。然后**把关键的十来个槽位覆写回我们算好的值**：
///
/// - `fromSeed` 的色调映射会让 `primary` 与我们的 `accent` 差一截
///   （种子只是「来源」，不是结果），实测能差到肉眼看得出；
/// - `surface` / `onSurface` 必须**精确等于**舞台与文字色，否则面板与舞台之间
///   会出现一条说不清来历的缝。
///
/// 覆写清单是「组件真的会读到、且读错会看得见」的那些——
/// 不是把 30 个槽位全部重写（那样 `fromSeed` 就白跑了）。
ThemeData buildAppTheme([AppThemeId theme = AppThemeId.fallback]) {
  final AppPalette palette = AppPalette.of(theme);
  final ColorScheme scheme = ColorScheme.fromSeed(
    seedColor: palette.accent,
    brightness: palette.brightness,
  ).copyWith(
    primary: palette.accent,
    onPrimary: palette.onAccent,
    primaryContainer: palette.surfaceAlt,
    onPrimaryContainer: palette.ink,
    surface: palette.surface,
    onSurface: palette.ink,
    surfaceContainerHighest: palette.surfaceAlt,
    onSurfaceVariant: palette.ink.withValues(alpha: 0.74),
    outline: palette.line,
    outlineVariant: palette.line,
    error: palette.danger,
    onError: palette.onDanger,
    errorContainer: palette.dangerSurface,
    onErrorContainer: palette.danger,
  );
  final ThemeData base = ThemeData(
    useMaterial3: true,
    brightness: palette.brightness,
    colorScheme: scheme,
    scaffoldBackgroundColor: palette.stage,
    // 亮色主题下 Material 默认那层深阴影会在纯白舞台上糊成一块灰边，
    // 所以阴影色也跟配色走：暗主题用舞台底（本来就近黑），亮主题用淡化的墨色。
    shadowColor: palette.dark
        ? palette.stage
        : palette.ink.withValues(alpha: 0.18),
    fontFamily: kAppFontFamily,
    extensions: <ThemeExtension<dynamic>>[
      palette,
      AppColors.of(scheme, palette),
    ],
  );
  return base.copyWith(textTheme: buildAppTextTheme(base.textTheme));
}

/// 取 [AppPalette] 的便捷入口。
///
/// 与 [appColorsOf] 同样**宁可炸也不静默降级**：拿不到配色就说明
/// `buildAppTheme` 的 `extensions:` 缺了一项，那时任何「回落默认色」都会让
/// 界面看起来「只是颜色有点怪」，从而把真正的接线错误藏起来。
AppPalette appPaletteOf(BuildContext context) {
  final AppPalette? palette = Theme.of(context).extension<AppPalette>();
  assert(
    palette != null,
    '主题里没有注册 AppPalette —— 检查 buildAppTheme 的 extensions',
  );
  return palette!;
}

/// 取 [AppColors] 的便捷入口。
///
/// 若主题里没注册（有人漏了 `extensions:`），这里会**抛异常而不是返回默认值**——
/// 「扩展没注册」正是同类项目里最常见的静默失效之一（SAP 的 `--primary-color`
/// 被引用 9 次却从未定义），所以宁可炸也不要悄悄降级。
AppColors appColorsOf(BuildContext context) {
  final AppColors? colors = Theme.of(context).extension<AppColors>();
  assert(colors != null, '主题里没有注册 AppColors —— 检查 buildAppTheme 的 extensions');
  return colors!;
}
