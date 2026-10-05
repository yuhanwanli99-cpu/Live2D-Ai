/// 设计 token：**纯逻辑、零 web 依赖**，可在 VM 上单测。
///
/// 本文件族（`tokens.dart` + `tokens_material.dart` / `tokens_motion.dart`
/// 两个 part）与 `typography.dart` 是**全仓库仅有的允许出现裸色值/裸字号的
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
///
/// # 行数拆分（2026-10-06，R4-T2）
///
/// 原本单文件 **950 行**，超 500（豁免上限 1000）。现在 = 库（670 行）+ 两个
/// part。库这一半仍是四套配色的**唯一真源**：每套 11 个颜色字段都要带
/// 「为什么是这个值」的取值理由，拆开会让「四套必须同步改」从一次编辑变成
/// 四处编辑——而漏改一处正是本项目 P4「静默失效」要治的病。
///
/// | 内容 | 去处 | 行数 |
/// | --- | --- | --- |
/// | 材质旋钮 / 浮起面阴影 / 间距 / 光学微调 / 圆角 | `tokens_material.dart` | 181 |
/// | 语义节拍 / 动效时长 / 曲线 / 断点转发 / 舞台色串 | `tokens_motion.dart` | 106 |
///
/// 两个 part 仍是**令牌声明处**：lint 的豁免面是**路径前缀**
/// `lib/design/tokens`，新文件自动落在豁免面内（不必往名单里加行）。
library;

import 'package:flutter/material.dart';

import 'breakpoints.dart';
import 'theme_id.dart';

part 'tokens_material.dart';
part 'tokens_motion.dart';

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
///    **舞台底不参与下面的「色相偏移」**——它是渲染面 framebuffer 的清屏色，
///    带上一点色相会在模型边缘出现一圈色边（由
///    `theme_palette_test` 的「舞台底必须中性」断言守着）。
/// 2. **文字与它所在的面**的对比度 ≥ 4.5:1（中文小字号的可读下限）；
///    由 `test/theme_test.dart` 逐主题断言，不是靠眼睛。
/// 3. **语义色按亮/暗各一套**：`#FF6B6B` 那种亮红在白底上只有 2.5:1，
///    在浅色主题里必须换成深红。这是「只换 accent 不换语义色」最容易翻车的地方。
/// 4. **强调色上的文字颜色是算出来的**，不是写死的（[onAccent]）。
///
/// # 色相偏移（2026-09-27，第二轮观感）
///
/// 实测（1440×900 同尺寸截图，`docs/design/assets/visual-substance-2026-09-27/`）：
/// 本项目改动前的界面**几乎没有颜色**——暗色主题下 91.5% 的像素绝对色度 ≤6/255，
/// 层级只能靠 1px 发丝线；表面也只有 1.5 级（`#0E0E11` → `#17171B`，
/// 感知亮度只差 0.035，落在同一个 4 级桶里）。
///
/// 病根是**四套配色的中性色全落在中性轴上**（`#0E0E11`/`#17171B`/`#F1F1F4`/`#E8E8EC`）。
/// 修法是给每套配色一个**固定的色相锚点**，把中性色整体往那个方向推一点：
///
/// | 配色 | 色相锚点 | 中性色里的偏移量 |
/// | --- | --- | --- |
/// | 黑 | 冷靛 | 20%（墨色 9%） |
/// | 白 | 暖白 | 13%（墨色**不推**：见下） |
/// | 蓝 | 蓝紫 | 20%（墨色 9%） |
/// | 灰 | 中性偏暖 | 20%（墨色 9%） |
///
/// 刻意**不给强调色上色**：强调色是 P2 定的「一个品牌色」，
/// 往里混色相会变成「第二种强调色」。三个语义色同理，它们本来就带色相。
///
/// **白套的墨色是例外**（2026-09-27 A3c 对账）：偏移表曾把它写成「墨色 9%」，
/// 但实测 `ink = #242423` 的色度只有 **1/255**（真按 9% 推会得到 ≈3/255）——
/// 白套只有三级面推了暖，墨色仍是近中性。这里按**实情**改写而不是假装推过；
/// 真要让它带上暖调，改的是颜色（四套中性色的取值），不是这条注释。
/// 回归：`theme_palette_test` 的「白套：三级面推暖，但墨色刻意近中性」。
///
/// 偏移量为什么是 9%–20%：再高就不是「有色彩倾向」而是「彩色界面」了
/// （实测 Morrow 的深色 chrome 平均色度 13/255，本项目改动后落在 9–30，
/// 比它更克制、且在纯黑舞台旁边更干净）。
///
/// # 表面阶梯：三档，级差 ≥ 0.03 感知亮度
///
/// `stage`（不动）→ [surface]（面板底）→ [surfaceAlt]（气泡 / 输入框）→ [raised]
/// （卡片 / 浮层 / 弹窗）。改动前只有前三级且级差 0.035 落在同一个桶里；
/// 现在每两档之间 ≥0.03，**不描边也读得出层级**。
/// 由 `test/theme_palette_test.dart` 的阶梯断言钉死。
@immutable
class AppPalette extends ThemeExtension<AppPalette> {
  const AppPalette({
    required this.id,
    required this.brightness,
    required this.stage,
    required this.surface,
    required this.surfaceAlt,
    required this.raised,
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

  /// **浮起面**（卡片 / 弹窗 / 底部浮层 / snackbar）。
  ///
  /// 2026-09-27 新增：[surface] 与 [surfaceAlt] 之间只有 0.035 的感知亮度差，
  /// 两者压在纯黑舞台旁边读作同一块。补这一档之后三级各差 ≥0.03，
  /// 「哪一层浮在谁上面」不靠描边也读得出来。
  final Color raised;

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
  Color get onAccent =>
      _contrast(accent, Colors.white) >= _contrast(accent, _black)
      ? Colors.white
      : _black;

  /// 危险色上的文字色（Material 的 `onError`）。
  ///
  /// 与 [onAccent] 同一条算法、**不写死**：暗色主题的危险色是亮红（配黑字），
  /// 亮色主题的危险色是深红（配白字）。写死任一边都会在其中一半主题上瞎掉。
  Color get onDanger =>
      _contrast(danger, Colors.white) >= _contrast(danger, _black)
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
  static const Map<AppThemeId, AppPalette> registry = <AppThemeId, AppPalette>{
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
    // 以下四色是同一族**冷靛**：往 #8A94FF 推 20%（墨色 9%）。
    surface: Color(0xFF11121B),
    surfaceAlt: Color(0xFF1D1E2A),
    raised: Color(0xFF252634),
    ink: Color(0xFFECEDF7),
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
    // **暖白**（往 #FFF2E0 推 13%）：纯中性白在屏幕上永远偏冷，
    // 与纸感更接近的做法就是给它一点点暖。
    surface: Color(0xFFF7F5F3),
    surfaceAlt: Color(0xFFEDECE9),
    raised: Color(0xFFE2E0DE),
    // 近中性墨色（色度 1/255）：白套**刻意不推暖**，见上方偏移表的说明与
    // `theme_palette_test` 的对账断言。
    ink: Color(0xFF242423),
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
    // 蓝紫家族（往 #386FF0 推 20%）：比旧值更亮一档，让三级阶梯都落进
    // 同一个蓝色相里，而不是「深蓝 + 深蓝 + 稍浅的深蓝」。
    surface: Color(0xFF161E30),
    surfaceAlt: Color(0xFF212A3F),
    raised: Color(0xFF28324B),
    ink: Color(0xFFE5EBF8),
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
    // 刻意做成四套里**最中性**的一套（色度只有 4–6/255）：它是「不要颜色」
    // 的那一档。但仍带一点暖（往 #FFEDD1 推 20%），免得和纯灰糊在一起。
    surface: Color(0xFF282623),
    surfaceAlt: Color(0xFF353330),
    raised: Color(0xFF3E3C38),
    ink: Color(0xFFECEAE8),
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
    'raised': raised,
    'ink': ink,
    'accent': accent,
    'success': success,
    'warning': warning,
    'danger': danger,
    'dangerSurface': dangerSurface,
    'dangerBorder': dangerBorder,
  };

  /// 配色字段个数（不含 `id` / `brightness`：它们不是颜色，不参与 lerp 对账）。
  static const int colorFieldCount = 11;

  @override
  AppPalette copyWith({
    AppThemeId? id,
    Brightness? brightness,
    Color? stage,
    Color? surface,
    Color? surfaceAlt,
    Color? raised,
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
      raised: raised ?? this.raised,
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
      raised: Color.lerp(raised, other.raised, t)!,
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
      other.raised == raised &&
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
    raised,
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
    required this.panelAlpha,
    required this.radiusScale,
  });

  /// 由 `ColorScheme` + 当前配色派生一套。**唯一的构造入口**——
  /// 避免各处自己乘透明度。
  ///
  /// 叠色的透明度要按亮暗分档：暗色主题上「白字加 12% 透明」是一道可见的
  /// 描边，在白色主题上同样 12% 的黑几乎看不见，反之亦然。所以
  /// [glassScrim] 这类**遮罩**也跟着 [palette] 的亮暗走。
  ///
  /// [edgeStrength]（2026-09-27）：用户的「描边强度」偏好，**只乘 [hairline]**。
  /// 不乘 [hoverWash]（那是填充不是描边）、不乘 [focusRing] / 语义色
  /// ——后两者是可用性下限，审美旋钮不该动它们。
  factory AppColors.of(
    ColorScheme scheme,
    AppPalette palette, {
    double edgeStrength = 1.0,
    AppMaterial material = AppMaterial.neutral,
  }) {
    final Color on = scheme.onSurface;
    final bool dark = palette.dark;
    // 遮罩必须与背景**反向**：暗主题用黑幕，亮主题用白幕。
    final Color veil = dark ? const Color(0xFF000000) : const Color(0xFFFFFFFF);
    return AppColors(
      // dark 下用 1 px 描边表达层级，而不是投影。
      hairline: on.withValues(alpha: (dark ? 0.12 : 0.10) * edgeStrength),
      hoverWash: on.withValues(alpha: dark ? 0.06 : 0.04),
      glassScrim: veil.withValues(alpha: dark ? 0.60 : 0.72),
      glassBarrier: veil.withValues(alpha: dark ? 0.32 : 0.40),
      // 承载文字信息的次要文本（≥ 12 px）。
      contentMuted: on.withValues(alpha: 0.74),
      // **装饰/图标**优先；0.68 不是审美值，而是**对比度下限**（见 contentFaint）。
      contentFaint: on.withValues(alpha: 0.68),
      // 键盘焦点环**必须不透明**（对比度要可测）。
      focusRing: scheme.primary,
      // 玻璃边缘高光的**基色**（见 `GlassRim`）。用 `ink` 而不是写死白：
      // 暗主题的墨色是近白（亮边），亮主题的墨色是近黑（暗边）——
      // 这正是「玻璃边缘拾取环境光」的物理直觉，也是四套主题下
      // **同一段代码都看得见**的原因。写死白色会在白色主题上完全消失。
      rimHighlight: palette.ink,
      serverMutedBadgeSurface: palette.warning.withValues(alpha: 0.18),
      serverMutedBadgeBorder: palette.warning,
      panelAlpha: panelAlphaFor(material.uiTransparency),
      radiusScale: material.radiusScale,
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

  /// 更弱的色（**装饰/图标**优先）。
  ///
  /// # 为什么是 0.68 而不是「更淡」（2026-09-28，F-0006-2）
  ///
  /// 这个令牌的自注原本是「**仅装饰/图标**，不得承载文字信息」——语义没错，
  /// 但审计实读发现它**真的**被当文字色用了（输入框占位文字、滑杆两端刻度、
  /// 空闲相位标签、外观分区的摘要行…；跨行的 `style:` 写法能绕过同行正则守卫）。
  /// 白色主题实测 **3.96–4.07 < WCAG AA 4.5**，低视力用户读不清。
  ///
  /// 「别拿它写文字」只能靠评审守（守卫挡不住跨行写法），**数值下限却可以被
  /// 算术守住**——所以这里把下限抬到「就算被误用为文字也读得清」：
  ///
  /// | 面（白主题） | 旧 0.60 | 现 0.68 |
  /// | --- | --- | --- |
  /// | `surface` #F7F5F3 | 4.08 | **5.21** |
  /// | `surfaceAlt` #EDECE9 | 3.96 | **5.01** |
  /// | `raised` #E2E0DE | 3.80 | **4.76** |
  ///
  /// 暗色三套只升不降（对 `surfaceAlt`：黑 5.93→7.22 / 蓝 5.31→6.38 /
  /// 灰 4.88→5.79），所以「黑主题不得因此降到 4.5 以下」自动成立。
  /// 0.66 是白主题 `raised` 上的临界值（4.497，差 0.003 不达标），故取 0.68 留余量。
  /// **纯函数算术回归**（含「旧值 0.60 必须红」的判别力自证）见
  /// `test/content_contrast_test.dart`。
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

  /// **面板面的不透明度**（1.0 = 不透明，0.55 = 最透）。
  ///
  /// 2026-09-27：用户的「界面透明程度」落到这里，而不是散在各个 widget 的
  /// `withValues(alpha: …)` 上——**一处判据**，所有面板读同一个数。
  ///
  /// 换算：`1 - 0.45 * t`，下界 [kMinPanelAlpha] 是可读性底线：
  /// 再透面板就不是「浮在上面」，而是文字直接落在图上。
  final double panelAlpha;

  /// 面板不透明度的下界（`uiTransparency = 1` 时正好取到它）。
  static const double kMinPanelAlpha = 0.55;

  /// 由「界面透明程度」推出面板 alpha。**纯函数，可 VM 单测**。
  static double panelAlphaFor(double uiTransparency) =>
      (1.0 - (1.0 - kMinPanelAlpha) * uiTransparency.clamp(0.0, 1.0)).clamp(
        kMinPanelAlpha,
        1.0,
      );

  /// **圆角缩放系数**（用户的「圆角幅度」）。
  ///
  /// 它**刻意不在** [toValuesMap] 的令牌面里：自建盒子一律通过 [radius] 取值，
  /// 没有人直接读这个字段——把它登记成「令牌」只会让「死令牌」检查一直报警。
  /// 它仍然参与相等性、hashCode 与 [kStructuralScalarCount] 的结构枚举。
  final double radiusScale;

  /// 圆角令牌 × 缩放系数。**自建盒子一律走这里**，不许写裸 `AppRadius`。
  double radius(double token) => token * radiusScale;

  /// **不放在令牌面、但必须被结构枚举覆盖**的成员。
  ///
  /// 为什么它不能进 [toValuesMap]：那里面的东西会被「声明↔引用」双向对账
  /// 当成令牌，而 `radiusScale` 的**唯一**访问入口是 [radius] 方法，
  /// 没人直接读这个字段——进令牌面就等于「天天报死令牌」。
  static const List<String> kStructuralScalarNames = <String>['radiusScale'];

  /// 字段个数（供测试做「新增字段忘了 copyWith/lerp」的结构枚举）。
  static const int fieldCount = 12;

  @override
  AppColors copyWith({
    Color? hairline,
    double? panelAlpha,
    double? radiusScale,
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
      panelAlpha: panelAlpha ?? this.panelAlpha,
      radiusScale: radiusScale ?? this.radiusScale,
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
      panelAlpha: panelAlpha + (other.panelAlpha - panelAlpha) * t,
      radiusScale: radiusScale + (other.radiusScale - radiusScale) * t,
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
    'panelAlpha': panelAlpha,
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
      other.serverMutedBadgeBorder == serverMutedBadgeBorder &&
      other.panelAlpha == panelAlpha &&
      other.radiusScale == radiusScale;

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
    panelAlpha,
    radiusScale,
  );
}

