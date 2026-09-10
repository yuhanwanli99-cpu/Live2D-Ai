/// 应用主题：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由与 `settings/display_prefs.dart`、`audio/gain.dart` 相同：
/// 主题里有几个**必须成立、且很容易被无意改坏**的约束（尤其是字体），
/// 埋在 `main.dart` 里就只能靠肉眼守。
library;

import 'package:flutter/material.dart';

/// 自托管中文字体的家族名。
///
/// **必须与 `pubspec.yaml` 的 `fonts:` 声明逐字一致**——
/// 这个对应关系由 `test/theme_test.dart` 扫描 pubspec 守住。
const String kAppFontFamily = 'NotoSansSC';

/// 主题种子色。
const Color kSeedColor = Color(0xFF7C5CFF);

/// 舞台/脚手架底色。
const Color kScaffoldBackground = Color(0xFF0B0D12);

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
ThemeData buildAppTheme() {
  final scheme = ColorScheme.fromSeed(
    seedColor: kSeedColor,
    brightness: Brightness.dark,
  );
  return ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: scheme,
    scaffoldBackgroundColor: kScaffoldBackground,
    fontFamily: kAppFontFamily,
  );
}
