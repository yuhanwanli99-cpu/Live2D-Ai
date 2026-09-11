/// 设计 token：**纯逻辑、零 web 依赖**，可在 VM 上单测。
///
/// 本文件与 `typography.dart` 是**全仓库仅有的两个允许出现裸色值/裸字号的
/// 源文件**——其余业务代码一律引用这里的令牌，由
/// `test/design_tokens_lint_test.dart` 扫描守住（设计规格 §2.8.3）。
///
/// # 四处分工（设计规格 §2.1，不要混）
///
/// | 维度 | 载体 | 为什么 |
/// | --- | --- | --- |
/// | 语义槽位色 | `ColorScheme`（`theme.dart` 里由 [AppPalette] 直接指定） | Material 已定义，重造会双份真相 |
/// | **整套配色**（舞台底 / 面板 / 文字 / 强调 / 语义色） | [AppPalette]（`ThemeExtension`） | 4 套主题，必须随主题切换；`lerp` 让切换可插值 |
/// | 依赖 `onSurface` 的叠色 | [AppColors]（`ThemeExtension`） | 由当前配色派生，比写死更耐换肤 |
/// | 尺寸 / 时长 / 曲线 / 断点 | `const` 常量类 | 与主题无关 ⇒ 无插值需求，`lerp` 价值≈0 |
library;

import 'package:flutter/material.dart';

import 'breakpoints.dart';
import 'theme_id.dart';

/// 一套完整的配色（**四套之一**，见 [AppThemeId]）。
///
/// # 为什么是 `ThemeExtension` 而不是 `const` 常量类
///
/// 旧版本这里是一个「只有 `const Color` 的静态类」，因为当时是 **dark-only**。
/// 现在有四套主题，同一个语义槽位（「舞台底」「面板底」「危险色」）在不同主题下
/// 必须是不同的值 —— 再写成静态常量就等于把「默认主题的值」硬编码进了
/// 组件里，换主题时组件不会变。做成 `ThemeExtension` 之后：
///
/// - 组件统一写 `appPaletteOf(context).stage`，换主题自动跟随；
/// - `lerp` 有意义（主题切换可以插值），`copyWith` 让结构测试能逐字段对账；
/// - 「注册了没有」这件事在 `buildAppTheme` 里是编译期可见的，不会静默降级。
///
/// # 取值原则（不是随便挑的颜色）
///
/// 1. **舞台底是纯色平面**（用户裁决「舞台背影全黑/全白即可，中央不要放贴图」）：
///    黑主题 `#000000`、白主题 `#FFFFFF`，蓝/灰是各自色相的纯色。
/// 2. **文字与它所在的面**的对比度 ≥ 4.5:1（中文小字号的可读下限）；
///    由 `test/theme_test.dart` 逐主题断言，不是靠眼睛。
/// 3. **语义色按亮/暗各一套**：`#FF6B6B` 那种亮红在白底上只有 2.5:1，
///    在浅色主题里必须换成深红。这是「只换 accent 不换语义色」最容易翻车的地方。
/// 4. **强调色上的文字颜色是算出来的**，不是写死的（[onAccent]）。
@immutable
class AppPalette extends ThemeExtension<AppPalette> {
  const AppPalette({
    required this.id,
    required this.brightness,
    required this.stage,
    required this.surface,
    required this.surfaceAlt,
    required this.ink,
    required this.accent,
    required this.success,
    required this.warning,
    required this.danger,
    required this.dangerSurface,
    required this.dangerBorder,
  });

  /// 这是哪一套（设置里的选中态要读它）。
  final AppThemeId id;

  /// 亮 / 暗。**由配色决定，不是用户可选项**：
  /// 「黑」「蓝」「灰」是暗色主题，「白」是亮色主题。
  final Brightness brightness;

  /// 舞台纯色底（Live2D iframe 后面那一层）。
  final Color stage;

  /// 面板底（聊天面板、设置面板）。
  final Color surface;

  /// 次级面（助手气泡、悬停的卡片）。
  final Color surfaceAlt;

  /// 主文本色。
  final Color ink;

  /// 强调色（发送键、焦点环、选中态）。
  final Color accent;

  /// 语义色（**按亮暗各一套**，见头注第 3 条）。
  final Color success;
  final Color warning;
  final Color danger;

  /// 危险状态的**面**与**描边**（错误条、失败气泡）。
  final Color dangerSurface;
  final Color dangerBorder;

  /// 是不是暗色主题。
  bool get dark => brightness == Brightness.dark;

  /// 舞台底的 **CSS 十六进制串**（`#rrggbb`），下发到渲染面。
  ///
  /// 单独一个 getter 而不是让调用方自己拼：拼法（补零、小写、不带 alpha）
  /// 必须**唯一**——渲染面会校验这个串，格式稍有出入就会被拒而回落到默认色，
  /// 表现为「主题切了但舞台没变」。
  ///
  /// 只取 RGB：舞台底是不透明纯色（有测试守着 `alpha == 1`）。
  String get stageCss => stageColorCss(stage);

  /// 强调色上的文字色：**算出来的**（对比度高的那个）。
  ///
  /// 写死白色会在「黑」主题上直接瞎掉——那套的强调色本身是近白色。
  Color get onAccent => _contrast(accent, Colors.white) >= _contrast(accent, _black)
      ? Colors.white
      : _black;

  /// 危险色上的文字色（Material 的 `onError`）。
  ///
  /// 与 [onAccent] 同一条算法、**不写死**：暗色主题的危险色是亮红（配黑字），
  /// 亮色主题的危险色是深红（配白字）。写死任一边都会在其中一半主题上瞎掉。
  Color get onDanger => _contrast(danger, Colors.white) >= _contrast(danger, _black)
      ? Colors.white
      : _black;

  /// 1 px 分隔线 / 卡片描边。
  Color get line => ink.withValues(alpha: dark ? 0.14 : 0.10);

  /// **用户气泡**底：强调色淡淡压在面板上。
  ///
  /// 为什么不能直接用 `surfaceAlt`：助手气泡用的就是它——两者同色的话，
  /// 「谁说的」就只剩左右对齐这一个通道了（灰度截图与低视力用户都读不出来）。
  /// 这里刻意只加 16%/10% 的强调色：够分辨，又不至于变成一块彩色补丁。
  Color get bubbleUser =>
      Color.alphaBlend(accent.withValues(alpha: dark ? 0.16 : 0.10), surface);

  /// **助手气泡**底：比面板亮/暗一档的中性面。
  Color get bubbleAssistant => surfaceAlt;

  /// 危险面上的文字色（与 [danger] 同值，单独起名是为了让调用点的意图可读）。
  Color get onDangerSurface => danger;

  static const Color _black = Color(0xFF000000);

  /// 两色的 WCAG 对比度。
  static double _contrast(Color a, Color b) {
    final double x = a.computeLuminance();
    final double y = b.computeLuminance();
    return ((x > y ? x : y) + 0.05) / ((x > y ? y : x) + 0.05);
  }

  /// 四套配色的**唯一真源**。
  static const Map<AppThemeId, AppPalette> registry =
      <AppThemeId, AppPalette>{
        AppThemeId.black: black,
        AppThemeId.white: white,
        AppThemeId.blue: blue,
        AppThemeId.gray: gray,
      };

  /// 按 id 取；未知回落 [AppThemeId.fallback]（**不抛**）。
  static AppPalette of(AppThemeId id) => registry[id] ?? black;

  // ────────────────────────────────────────────────────────── 四套取值

  /// 黑（默认）：纯黑舞台 + 冷白强调。
  static const AppPalette black = AppPalette(
    id: AppThemeId.black,
    brightness: Brightness.dark,
    // **纯黑**，不是「接近黑」：用户明确说「舞台背影全黑即可」。
    stage: Color(0xFF000000),
    surface: Color(0xFF0E0E11),
    surfaceAlt: Color(0xFF17171B),
    ink: Color(0xFFF1F1F4),
    accent: Color(0xFFE8E8EC),
    success: Color(0xFF3DD68C),
    warning: Color(0xFFF5A524),
    danger: Color(0xFFFF6B6B),
    dangerSurface: Color(0xFF3A1D1D),
    dangerBorder: Color(0xFF8A3B3B),
  );

  /// 白：纯白舞台 + 近黑强调。
  static const AppPalette white = AppPalette(
    id: AppThemeId.white,
    brightness: Brightness.light,
    stage: Color(0xFFFFFFFF),
    surface: Color(0xFFF4F4F6),
    surfaceAlt: Color(0xFFEAEAEE),
    ink: Color(0xFF17171A),
    accent: Color(0xFF17171A),
    success: Color(0xFF146B3E),
    warning: Color(0xFF8A5A00),
    danger: Color(0xFFC0392B),
    dangerSurface: Color(0xFFFCEBEB),
    dangerBorder: Color(0xFFE0A9A3),
  );

  /// 蓝：深蓝舞台 + 天蓝强调。
  static const AppPalette blue = AppPalette(
    id: AppThemeId.blue,
    brightness: Brightness.dark,
    stage: Color(0xFF061223),
    surface: Color(0xFF0D1C33),
    surfaceAlt: Color(0xFF16294A),
    ink: Color(0xFFE7EFFC),
    accent: Color(0xFF4D8DFF),
    success: Color(0xFF3DD68C),
    warning: Color(0xFFF5C24C),
    danger: Color(0xFFFF7B7B),
    dangerSurface: Color(0xFF3A1D25),
    dangerBorder: Color(0xFF8C4453),
  );

  /// 灰：中灰舞台 + 灰白强调（比黑柔一档，不是「黑加一点灰」）。
  static const AppPalette gray = AppPalette(
    id: AppThemeId.gray,
    brightness: Brightness.dark,
    stage: Color(0xFF1C1C1F),
    surface: Color(0xFF26262A),
    surfaceAlt: Color(0xFF323238),
    ink: Color(0xFFECECEF),
    accent: Color(0xFFD8D8DE),
    success: Color(0xFF4FD79A),
    warning: Color(0xFFF0AE3C),
    danger: Color(0xFFFF8080),
    dangerSurface: Color(0xFF3B2323),
    dangerBorder: Color(0xFF8C5050),
  );

  /// 字段名 → 值（供测试做「新增字段忘了 copyWith/lerp」的结构枚举）。
  Map<String, Object?> toValuesMap() => <String, Object?>{
    'stage': stage,
    'surface': surface,
    'surfaceAlt': surfaceAlt,
    'ink': ink,
    'accent': accent,
    'success': success,
    'warning': warning,
    'danger': danger,
    'dangerSurface': dangerSurface,
    'dangerBorder': dangerBorder,
  };

  /// 配色字段个数（不含 `id` / `brightness`：它们不是颜色，不参与 lerp 对账）。
  static const int colorFieldCount = 10;

  @override
  AppPalette copyWith({
    AppThemeId? id,
    Brightness? brightness,
    Color? stage,
    Color? surface,
    Color? surfaceAlt,
    Color? ink,
    Color? accent,
    Color? success,
    Color? warning,
    Color? danger,
    Color? dangerSurface,
    Color? dangerBorder,
  }) {
    return AppPalette(
      id: id ?? this.id,
      brightness: brightness ?? this.brightness,
      stage: stage ?? this.stage,
      surface: surface ?? this.surface,
      surfaceAlt: surfaceAlt ?? this.surfaceAlt,
      ink: ink ?? this.ink,
      accent: accent ?? this.accent,
      success: success ?? this.success,
      warning: warning ?? this.warning,
      danger: danger ?? this.danger,
      dangerSurface: dangerSurface ?? this.dangerSurface,
      dangerBorder: dangerBorder ?? this.dangerBorder,
    );
  }

  @override
  AppPalette lerp(covariant AppPalette? other, double t) {
    if (other == null) return this;
    return AppPalette(
      // **离散字段不插值**：`t < 1` 时保持旧主题的身份，切完才换。
      // id/brightness 插值出中间值没有意义（枚举无法一半黑一半白）。
      id: t < 1 ? id : other.id,
      brightness: t < 1 ? brightness : other.brightness,
      stage: Color.lerp(stage, other.stage, t)!,
      surface: Color.lerp(surface, other.surface, t)!,
      surfaceAlt: Color.lerp(surfaceAlt, other.surfaceAlt, t)!,
      ink: Color.lerp(ink, other.ink, t)!,
      accent: Color.lerp(accent, other.accent, t)!,
      success: Color.lerp(success, other.success, t)!,
      warning: Color.lerp(warning, other.warning, t)!,
      danger: Color.lerp(danger, other.danger, t)!,
      dangerSurface: Color.lerp(dangerSurface, other.dangerSurface, t)!,
      dangerBorder: Color.lerp(dangerBorder, other.dangerBorder, t)!,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is AppPalette &&
      other.id == id &&
      other.brightness == brightness &&
      other.stage == stage &&
      other.surface == surface &&
      other.surfaceAlt == surfaceAlt &&
      other.ink == ink &&
      other.accent == accent &&
      other.success == success &&
      other.warning == warning &&
      other.danger == danger &&
      other.dangerSurface == dangerSurface &&
      other.dangerBorder == dangerBorder;

  @override
  int get hashCode => Object.hash(
    id,
    brightness,
    stage,
    surface,
    surfaceAlt,
    ink,
    accent,
    success,
    warning,
    danger,
    dangerSurface,
    dangerBorder,
  );
}


/// 依赖 `onSurface` / `primary` 的**叠色与描边**，走 `ThemeExtension`。
///
/// 为什么这几个必须能插值：它们的值是「某个语义槽位色乘一个透明度」，
/// 槽位色一变它们就该跟着变。写成 `const` 就把两者绑死了。
@immutable
class AppColors extends ThemeExtension<AppColors> {
  const AppColors({
    required this.hairline,
    required this.hoverWash,
    required this.glassScrim,
    required this.glassBarrier,
    required this.contentMuted,
    required this.contentFaint,
    required this.focusRing,
    required this.rimHighlight,
    required this.serverMutedBadgeSurface,
    required this.serverMutedBadgeBorder,
  });

  /// 由 `ColorScheme` + 当前配色派生一套。**唯一的构造入口**——
  /// 避免各处自己乘透明度。
  ///
  /// 叠色的透明度要按亮暗分档：暗色主题上「白字加 12% 透明」是一道可见的
  /// 描边，在白色主题上同样 12% 的黑几乎看不见，反之亦然。所以
  /// [glassScrim] 这类**遮罩**也跟着 [palette] 的亮暗走。
  factory AppColors.of(ColorScheme scheme, AppPalette palette) {
    final Color on = scheme.onSurface;
    final bool dark = palette.dark;
    // 遮罩必须与背景**反向**：暗主题用黑幕，亮主题用白幕。
    final Color veil = dark ? const Color(0xFF000000) : const Color(0xFFFFFFFF);
    return AppColors(
      // dark 下用 1 px 描边表达层级，而不是投影。
      hairline: on.withValues(alpha: dark ? 0.12 : 0.10),
      hoverWash: on.withValues(alpha: dark ? 0.06 : 0.04),
      glassScrim: veil.withValues(alpha: dark ? 0.60 : 0.72),
      glassBarrier: veil.withValues(alpha: dark ? 0.32 : 0.40),
      // 承载文字信息的次要文本（≥ 12 px）。
      contentMuted: on.withValues(alpha: 0.74),
      // **仅装饰/图标**，不得承载文字信息。
      contentFaint: on.withValues(alpha: 0.60),
      // 键盘焦点环**必须不透明**（对比度要可测）。
      focusRing: scheme.primary,
      // 玻璃边缘高光的**基色**（见 `GlassRim`）。用 `ink` 而不是写死白：
      // 暗主题的墨色是近白（亮边），亮主题的墨色是近黑（暗边）——
      // 这正是「玻璃边缘拾取环境光」的物理直觉，也是四套主题下
      // **同一段代码都看得见**的原因。写死白色会在白色主题上完全消失。
      rimHighlight: palette.ink,
      serverMutedBadgeSurface: palette.warning.withValues(alpha: 0.18),
      serverMutedBadgeBorder: palette.warning,
    );
  }

  /// 1 px 分隔线 / 卡片描边。
  final Color hairline;

  /// 悬停底色。
  final Color hoverWash;

  /// 舞台徽标 / 浮层底（比 barrier 更实）。
  final Color glassScrim;

  /// 弹层遮罩。
  final Color glassBarrier;

  /// 次要文本（承载信息）。
  final Color contentMuted;

  /// 更弱的色（**仅装饰/图标**）。
  final Color contentFaint;

  /// 键盘焦点环。
  final Color focusRing;

  /// 玻璃边缘高光的基色（`GlassRim` 用；**只描边、不铺底**）。
  ///
  /// 刻意不是写死的白色：白色主题上白边等于没有边。取当前主题的墨色，
  /// 亮主题自动变成一道深色细边——观感上仍然是「一圈边缘」，但看得见。
  final Color rimHighlight;

  /// 服务端静音只读徽标的底。
  final Color serverMutedBadgeSurface;

  /// 服务端静音只读徽标的描边。
  final Color serverMutedBadgeBorder;

  /// 字段个数（供测试做「新增字段忘了 copyWith/lerp」的结构枚举）。
  static const int fieldCount = 10;

  @override
  AppColors copyWith({
    Color? hairline,
    Color? hoverWash,
    Color? glassScrim,
    Color? glassBarrier,
    Color? contentMuted,
    Color? contentFaint,
    Color? focusRing,
    Color? rimHighlight,
    Color? serverMutedBadgeSurface,
    Color? serverMutedBadgeBorder,
  }) {
    return AppColors(
      hairline: hairline ?? this.hairline,
      hoverWash: hoverWash ?? this.hoverWash,
      glassScrim: glassScrim ?? this.glassScrim,
      glassBarrier: glassBarrier ?? this.glassBarrier,
      contentMuted: contentMuted ?? this.contentMuted,
      contentFaint: contentFaint ?? this.contentFaint,
      focusRing: focusRing ?? this.focusRing,
      rimHighlight: rimHighlight ?? this.rimHighlight,
      serverMutedBadgeSurface:
          serverMutedBadgeSurface ?? this.serverMutedBadgeSurface,
      serverMutedBadgeBorder:
          serverMutedBadgeBorder ?? this.serverMutedBadgeBorder,
    );
  }

  @override
  AppColors lerp(covariant AppColors? other, double t) {
    if (other == null) return this;
    return AppColors(
      hairline: Color.lerp(hairline, other.hairline, t)!,
      hoverWash: Color.lerp(hoverWash, other.hoverWash, t)!,
      glassScrim: Color.lerp(glassScrim, other.glassScrim, t)!,
      glassBarrier: Color.lerp(glassBarrier, other.glassBarrier, t)!,
      contentMuted: Color.lerp(contentMuted, other.contentMuted, t)!,
      contentFaint: Color.lerp(contentFaint, other.contentFaint, t)!,
      focusRing: Color.lerp(focusRing, other.focusRing, t)!,
      rimHighlight: Color.lerp(rimHighlight, other.rimHighlight, t)!,
      serverMutedBadgeSurface: Color.lerp(
        serverMutedBadgeSurface,
        other.serverMutedBadgeSurface,
        t,
      )!,
      serverMutedBadgeBorder: Color.lerp(
        serverMutedBadgeBorder,
        other.serverMutedBadgeBorder,
        t,
      )!,
    );
  }

  /// 供测试做结构枚举：字段名 → 值。
  Map<String, Object?> toValuesMap() => <String, Object?>{
    'hairline': hairline,
    'hoverWash': hoverWash,
    'glassScrim': glassScrim,
    'glassBarrier': glassBarrier,
    'contentMuted': contentMuted,
    'contentFaint': contentFaint,
    'focusRing': focusRing,
    'rimHighlight': rimHighlight,
    'serverMutedBadgeSurface': serverMutedBadgeSurface,
    'serverMutedBadgeBorder': serverMutedBadgeBorder,
  };

  @override
  bool operator ==(Object other) =>
      other is AppColors &&
      other.hairline == hairline &&
      other.hoverWash == hoverWash &&
      other.glassScrim == glassScrim &&
      other.glassBarrier == glassBarrier &&
      other.contentMuted == contentMuted &&
      other.contentFaint == contentFaint &&
      other.focusRing == focusRing &&
      other.rimHighlight == rimHighlight &&
      other.serverMutedBadgeSurface == serverMutedBadgeSurface &&
      other.serverMutedBadgeBorder == serverMutedBadgeBorder;

  @override
  int get hashCode => Object.hash(
    hairline,
    hoverWash,
    glassScrim,
    glassBarrier,
    contentMuted,
    contentFaint,
    focusRing,
    rimHighlight,
    serverMutedBadgeSurface,
    serverMutedBadgeBorder,
  );
}

/// 间距：4 px 基准网格，9 档。
abstract final class Space {
  static const double s0 = 0;
  static const double s1 = 4;
  static const double s2 = 8;
  static const double s3 = 12;
  static const double s4 = 16;
  static const double s5 = 20;
  static const double s6 = 24;
  static const double s7 = 32;
  static const double s8 = 48;

  /// 登记表（只服务测试）。
  static const Map<String, double> registry = <String, double>{
    's0': s0,
    's1': s1,
    's2': s2,
    's3': s3,
    's4': s4,
    's5': s5,
    's6': s6,
    's7': s7,
    's8': s8,
  };
}

/// 光学微调：**把图标/光标与相邻文字的基线对齐**用的 1–2 px。
///
/// # 为什么单独一个概念，而不是塞进 `Space`（2026-09-11，P4-2）
///
/// `Space` 是「**元素之间**的间距」，4 px 基准网格，9 档。而这几个值是
/// 「**同一个元素内部**视觉重心的微调」——例如让 16 px 的图标和 13 px 的
/// 第一行文字看起来对齐，靠的是往上推 1–2 px。
///
/// 两者混在一起会有两个后果：
/// 1. `Space` 的「4 px 网格」不再是可断言的（出现 1 和 2）；
/// 2. 把 `top: 2` 改成 `Space.s1`（=4）会**真的改变观感**——
///    那不是「归位到令牌」，是改设计。
///
/// 所以给它一个诚实的名字：它不在网格上，也不该在网格上。
abstract final class OpticalNudge {
  /// 1 px：图标与首行文字的基线微调。
  static const double hair = 1;

  /// 2 px：标签与内容的贴合间距、流式光标与正文的间隙。
  static const double thin = 2;
}

/// 圆角：6 档，**档位两两不同值**。
///
/// 命名为 `AppRadius` 而非规格里的 `Radius`：`Radius` 是 `dart:ui` 的类，
/// 且被 `package:flutter/material.dart` 导出（`Radius.circular()` 很常用）。
/// 同名会让任何同时 import 两者的文件在引用 `Radius` 时**歧义报错**。
/// 这是对规格的一处必要偏离，语义不变。
abstract final class AppRadius {
  static const double none = 0;
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 20;
  static const double pill = 999;

  /// 登记表（只服务测试）。
  static const Map<String, double> registry = <String, double>{
    'none': none,
    'xs': xs,
    'sm': sm,
    'md': md,
    'lg': lg,
    'xl': xl,
    'pill': pill,
  };

  // 刻意**不提供** `rMd` 这类便捷构造：`BorderRadius.circular(AppRadius.md)`
  // 已经足够短，多一层包装只会多一份「声明了但没人用」的死令牌，
  // 而且会让「令牌引用」的统计出现两套写法。
}

/// **语义节拍**：不属于「UI 过渡时长」那 4 档的单点节奏。
///
/// 为什么要单独一个家族，而不是塞进 [AppDurations]：
/// [AppDurations] 的语义是「一次过渡有多快」（hover/pressed/内容切换），
/// 而这里是「一个持续状态按什么节拍呼吸」。两者混在一起会让
/// 「4 档」这个约束失去意义（规格 §2.7 的时长阶梯就是 4 档）。
///
/// 三者都是**跨组件共享**的：思考呼吸同时被 `StatePill` 与
/// `StreamingIndicator` 用，如果各自写一个 `Duration(milliseconds: 1400)`，
/// 改一处忘一处就会让两个指示器不同步（同类项目的经典病）。
abstract final class AppRhythms {
  /// 思考态的呼吸周期（规格 §6.3：1.4 s）。
  ///
  /// 取值理由：慢到不像「加载转圈」（那会制造焦虑），快到一眼看出是活的。
  static const Duration thinkingBreath = Duration(milliseconds: 1400);

  /// 「瞬时状态」的保持时长：`已打断`、`已复制` 这类**就地确认**共用一档。
  ///
  /// 长到人眼能读完那几个字，短到不会和下一轮重叠。
  ///
  /// **刻意只有一档**（2026-09-11，P2-3）：下面那条关于「流式三点周期」的
  /// 说明同样适用于这里——两个**同值**令牌只会制造「改一处忘一处」的机会。
  /// 「已复制」与「已打断」确实不是同一件事，但它们要的是**同一个时长**，
  /// 那就该是同一个令牌：令牌的粒度是**数值语义**，不是文案语义。
  static const Duration interruptedHold = Duration(milliseconds: 1200);

  /// 悬停提示出现前的等待（`Tooltip.waitDuration`）。
  ///
  /// 为什么是「节奏」而不是 `AppDurations` 里的一档：那 4 档的语义是
  /// **一次过渡有多快**，而这是**等多久才开始**。两者混在一起会让
  /// 「4 档」这个约束失去意义。
  ///
  /// 取值理由：400 ms 是桌面端悬停提示的常见量级——短到不觉得迟钝，
  /// 长到鼠标划过时不至于一路弹提示。
  static const Duration hintDelay = Duration(milliseconds: 400);

  // 刻意**没有**单独的「流式三点周期」：三点跑动与呼吸光表达的是同一件事
  // （思考中），共用 [thinkingBreath] 让两个指示器**同频**——看起来是有意为之，
  // 而不是各跑各的。两个同值令牌只会制造「改一处忘一处」的机会。

  /// 登记表（只服务测试）。
  static const Map<String, Duration> registry = <String, Duration>{
    'thinkingBreath': thinkingBreath,
    'interruptedHold': interruptedHold,
    'hintDelay': hintDelay,
  };
}

/// 动效时长：4 档，全部具名。
abstract final class AppDurations {
  /// hover / pressed / 开关 / 徽标切换。
  static const Duration fast = Duration(milliseconds: 120);

  /// 内容切换、消息渐入、横幅滑入。
  static const Duration base = Duration(milliseconds: 200);

  /// 模态 / sheet 入场。
  static const Duration slow = Duration(milliseconds: 320);

  /// **仅此一处**长动画：启动揭示。
  static const Duration reveal = Duration(milliseconds: 600);

  /// 登记表（只服务测试）。
  static const Map<String, Duration> registry = <String, Duration>{
    'fast': fast,
    'base': base,
    'slow': slow,
    'reveal': reveal,
  };
}

/// 动效曲线：**只允许两条**。
abstract final class Motion {
  /// 入场 / 出场（快出慢收）。AIRI 与 Nexus 独立收敛到同一条，属跨项目验证值。
  static const Cubic enter = Cubic(0.16, 1.0, 0.3, 1.0);

  /// 状态过渡、颜色/尺寸变化、开关。
  static const Curve state = Curves.easeInOutCubic;

  /// 登记表：曲线没有可枚举数值，登记名字。
  static const List<String> names = <String>['enter', 'state'];
}

/// 阈值令牌（转发 [Breakpoints]，让「引用侧枚举」能统一按 `Breakpoints.` 统计）。
abstract final class BreakpointTokens {
  static const double mediumMin = Breakpoints.mediumMin;
  static const double expandedMin = Breakpoints.expandedMin;
}

/// 任意颜色 → 舞台底要的 CSS 十六进制串（`#rrggbb`）。
///
/// # 为什么提到顶层（2026-09-11，P1-3）
///
/// 舞台底在**主题切换**时要跟着 UI 一起**插值**（否则界面在 200 ms 里渐变、
/// iframe 已经跳到终色，看起来像「舞台先闪了一下」）。插值意味着中途会产生
/// 一堆**中间色**，它们也要拼成同样的串格式——拼法一旦有第二个实现就会漂移，
/// 而渲染面是**严格校验**这个串的（格式不对就静默回落到默认色，
/// 表现为「主题切了但舞台没变」）。
///
/// 所以拼法只有这一处，[AppPalette.stageCss] 也走它。
///
/// 只取 RGB：舞台底是不透明纯色（有测试守着 `alpha == 1`）。
String stageColorCss(Color color) =>
    '#${(color.toARGB32() & 0xFFFFFF).toRadixString(16).padLeft(6, '0')}';
