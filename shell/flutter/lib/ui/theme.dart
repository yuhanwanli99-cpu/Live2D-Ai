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
  return buildAppComponents(
    base.copyWith(textTheme: buildAppTextTheme(base.textTheme)),
    palette,
  );
}

/// **组件外观层**：把「一看就是 Material 默认」的那几处压下去。
///
/// # 为什么值得单独一层（2026-09-11 用户裁决）
///
/// 用户原话：「整个前端 ui 很 **ai 化同质**，看起来不舒服」。
/// 「AI 味」在 Flutter 里的具体来源不是配色，而是**每个控件都带着 Material
/// 的出厂造型**：`FilledButton` 的 20 px 圆角 + 高度阴影、`TextField` 的
/// 浮动标签 + 四边框、`Dialog` 的 24 px 圆角 + elevation 6、`AppBar` 的
/// surface tint 滚动变色、`Switch` 的默认尺寸…… 单个都「没错」，
/// 叠在一起就是「随便一个 AI 生成的 Material demo」。
///
/// 这一层的做法是**统一**而不是逐个改控件：圆角只取 [AppRadius]。
/// `md`/`lg`，描边只取 1 px hairline，**所有 elevation 归零**，
/// 文字层级只靠 `AppFontSizes` 的 8 个槽位。控件自己的 `styleFrom` 就只剩
/// 「这一处确实要不一样」的部分。
///
/// 刻意**不做**的事：不改控件的交互语义（禁用态、水波纹、`highlightMode`
/// 都保留），不引入自绘控件。这一层只改「长什么样」——**焦点环的形状**也算
/// 「长什么样」（见 [focusSide]）。
ThemeData buildAppComponents(ThemeData base, AppPalette palette) {
  final ColorScheme scheme = base.colorScheme;
  final AppColors colors = AppColors.of(scheme, palette);
  final TextTheme text = base.textTheme;
  final OutlinedBorder buttonShape = RoundedRectangleBorder(
    borderRadius: BorderRadius.circular(AppRadius.md),
  );

  /// 焦点可见性：**1 px 不透明 `focusRing` 边框**（规格 §9.2）。
  ///
  /// # 为什么是边框，不是阴影（2026-09-11 接线，P1-4）
  ///
  /// Material 默认的焦点提示在深色主题上是一层很淡的 overlay，
  /// 四套配色下对比度**没法逐主题断言**——而本项目的红线是
  /// 「对比度必须可测」（`theme_palette_test.dart` 逐主题验 ≥ 4.5:1）。
  /// 换成 1 px 实心描边之后，焦点环走和其他颜色**同一套**断言。
  ///
  /// 另一条：**不要覆盖 `FocusManager.highlightMode`**。触屏用户本来就不该
  /// 看到焦点环（那是 Material 的既定语义），本层只决定「环长什么样」。
  WidgetStateProperty<BorderSide?> focusSide(BorderSide? resting) =>
      WidgetStateProperty.resolveWith<BorderSide?>((Set<WidgetState> states) {
        if (states.contains(WidgetState.focused)) {
          return BorderSide(color: colors.focusRing, width: 1);
        }
        return resting;
      });

  final ButtonStyle flatButton = ButtonStyle(
    elevation: const WidgetStatePropertyAll<double>(0),
    // 按钮文字统一用 13/w500 的 `labelLarge`，不用 Material 的 14/w500——
    // 后者与正文同字号，按钮和正文会糊成一层。
    textStyle: WidgetStatePropertyAll<TextStyle?>(text.labelLarge),
    padding: const WidgetStatePropertyAll<EdgeInsetsGeometry>(
      EdgeInsets.symmetric(horizontal: Space.s4, vertical: Space.s2),
    ),
    shape: WidgetStatePropertyAll<OutlinedBorder>(buttonShape),
    // 最小高度 40：Material 的 40 是给触屏的拇指尺寸，保留。
    minimumSize: const WidgetStatePropertyAll<Size>(Size(0, 40)),
    // 无描边的按钮（Filled / Text / Elevated）平时不带边，**只在获得键盘焦点时**
    // 长出一圈 focusRing。
    side: focusSide(null),
  );

  return base.copyWith(
    // ── 顶栏：**透明、无高度阴影、标题小一号且带字距** ──
    // 标题不再抢正文的注意力（「Live2D Ai」不是用户来看的东西），
    // 字距让它读起来像标识而不是标题。
    appBarTheme: AppBarTheme(
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      scrolledUnderElevation: 0,
      centerTitle: false,
      titleSpacing: Space.s4,
      toolbarHeight: 52,
      titleTextStyle: text.labelLarge?.copyWith(
        color: palette.ink,
        letterSpacing: 2.2,
        fontWeight: FontWeight.w600,
      ),
      iconTheme: IconThemeData(color: palette.ink, size: 18),
    ),

    // ── 按钮：全部压平、同一圆角、同一字级 ──
    filledButtonTheme: FilledButtonThemeData(style: flatButton),
    textButtonTheme: TextButtonThemeData(
      style: flatButton.copyWith(
        // 文字按钮是**最轻**的一档：不要底色，不要最小高度撑开布局。
        minimumSize: const WidgetStatePropertyAll<Size>(Size(0, 32)),
        padding: const WidgetStatePropertyAll<EdgeInsetsGeometry>(
          EdgeInsets.symmetric(horizontal: Space.s2, vertical: Space.s1),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: flatButton.copyWith(
        // 有边按钮：平时是 hairline，**焦点时换成 focusRing**（不是两套并存）。
        side: focusSide(BorderSide(color: colors.hairline)),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(style: flatButton),
    iconButtonTheme: IconButtonThemeData(
      style: ButtonStyle(
        // **没有圆形 splash 底**：Material 的 IconButton 默认带一层
        // `surfaceContainerHighest` 的悬停/按下底，在深色主题上是一块灰斑。
        foregroundColor: WidgetStatePropertyAll<Color>(palette.ink),
        iconSize: const WidgetStatePropertyAll<double>(18),
        visualDensity: VisualDensity.compact,
        shape: WidgetStatePropertyAll<OutlinedBorder>(
          RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(AppRadius.sm),
          ),
        ),
        // 图标按钮没有文字标签，键盘用户尤其需要看得见焦点在哪。
        side: focusSide(null),
      ),
    ),

    // ── 输入框：**填色、无四边框、不浮动标签** ──
    // Material 的「OutlineInputBorder + 浮动 label」是最强的一处 AI 味：
    // 一个空框里飘着半截标题。这里改成安静的填色块，说明文字在框**上方**
    // （`FieldRow` 一直是这么排的，是边框样式在拖后腿）。
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: palette.surfaceAlt.withValues(alpha: palette.dark ? 0.9 : 1),
      isDense: true,
      contentPadding: const EdgeInsets.symmetric(
        horizontal: Space.s3,
        vertical: Space.s3,
      ),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: BorderSide.none,
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: BorderSide(color: colors.hairline),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: BorderSide(color: scheme.primary, width: 1.5),
      ),
      errorBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: BorderSide(color: palette.danger),
      ),
      focusedErrorBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: BorderSide(color: palette.danger, width: 1.5),
      ),
      hintStyle: text.bodyMedium?.copyWith(color: colors.contentFaint),
    ),

    // ── 分区导航：文字 chip，无头像图标 ──
    chipTheme: ChipThemeData(
      backgroundColor: Colors.transparent,
      selectedColor: palette.surfaceAlt,
      side: BorderSide(color: colors.hairline),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
      ),
      labelStyle: text.labelLarge?.copyWith(color: palette.ink),
      secondaryLabelStyle: text.labelLarge?.copyWith(color: palette.ink),
      padding: const EdgeInsets.symmetric(horizontal: Space.s2),
      showCheckmark: false,
    ),

    // ── 分段按钮：与按钮同一圆角，去掉选中勾 ──
    segmentedButtonTheme: SegmentedButtonThemeData(
      style: ButtonStyle(
        textStyle: WidgetStatePropertyAll<TextStyle?>(text.labelLarge),
        side: WidgetStatePropertyAll<BorderSide>(
          BorderSide(color: colors.hairline),
        ),
        shape: WidgetStatePropertyAll<OutlinedBorder>(buttonShape),
        visualDensity: VisualDensity.compact,
      ),
    ),

    // ── 容器类：**elevation 一律 0**，层级靠 1 px 描边与面差 ──
    cardTheme: CardThemeData(
      elevation: 0,
      color: palette.surface,
      surfaceTintColor: Colors.transparent,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.lg),
        side: BorderSide(color: colors.hairline),
      ),
    ),
    dialogTheme: DialogThemeData(
      elevation: 0,
      backgroundColor: palette.surface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.xl),
        side: BorderSide(color: colors.hairline),
      ),
    ),
    bottomSheetTheme: BottomSheetThemeData(
      elevation: 0,
      backgroundColor: palette.surface,
      surfaceTintColor: Colors.transparent,
      showDragHandle: true,
      dragHandleColor: colors.contentFaint,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(AppRadius.xl),
        ),
      ),
    ),
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      backgroundColor: palette.surfaceAlt,
      contentTextStyle: text.bodyMedium?.copyWith(color: palette.ink),
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
      ),
    ),
    tooltipTheme: TooltipThemeData(
      decoration: BoxDecoration(
        color: palette.surfaceAlt,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        border: Border.all(color: colors.hairline),
      ),
      textStyle: text.labelSmall?.copyWith(color: palette.ink),
      waitDuration: AppRhythms.hintDelay,
    ),

    // ── 其余细节 ──
    dividerTheme: DividerThemeData(
      color: colors.hairline,
      thickness: 1,
      space: 1,
    ),
    sliderTheme: SliderThemeData(
      trackHeight: 3,
      activeTrackColor: scheme.primary,
      inactiveTrackColor: colors.hairline,
      thumbColor: scheme.primary,
      overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
    ),
    switchTheme: SwitchThemeData(
      thumbColor: WidgetStateProperty.resolveWith<Color?>(
        (Set<WidgetState> states) => states.contains(WidgetState.selected)
            ? palette.onAccent
            : colors.contentFaint,
      ),
      trackColor: WidgetStateProperty.resolveWith<Color?>(
        (Set<WidgetState> states) => states.contains(WidgetState.selected)
            ? scheme.primary
            : colors.hoverWash,
      ),
      trackOutlineColor: WidgetStatePropertyAll<Color>(colors.hairline),
    ),
    progressIndicatorTheme: ProgressIndicatorThemeData(
      color: scheme.primary,
      linearTrackColor: colors.hairline,
      circularTrackColor: colors.hairline,
    ),
    listTileTheme: ListTileThemeData(
      iconColor: colors.contentMuted,
      textColor: palette.ink,
      selectedColor: scheme.primary,
      selectedTileColor: colors.hoverWash,
      titleTextStyle: text.bodyMedium?.copyWith(color: palette.ink),
      subtitleTextStyle: text.bodySmall?.copyWith(color: colors.contentMuted),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
      ),
    ),
  );
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
