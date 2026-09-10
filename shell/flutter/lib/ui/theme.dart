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
ThemeData buildAppTheme() {
  final ColorScheme scheme = ColorScheme.fromSeed(
    seedColor: AppPalette.seed,
    brightness: Brightness.dark,
  );
  final ThemeData base = ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: scheme,
    scaffoldBackgroundColor: AppPalette.stageBackdrop,
    fontFamily: kAppFontFamily,
    extensions: <ThemeExtension<dynamic>>[AppColors.of(scheme)],
  );
  return base.copyWith(textTheme: buildAppTextTheme(base.textTheme));
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
