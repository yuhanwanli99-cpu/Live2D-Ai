/// 设计 token：**纯逻辑、零 web 依赖**，可在 VM 上单测。
///
/// 本文件与 `typography.dart` 是**全仓库仅有的两个允许出现裸色值/裸字号的
/// 源文件**——其余业务代码一律引用这里的令牌，由
/// `test/design_tokens_lint_test.dart` 扫描守住（设计规格 §2.8.3）。
///
/// # 三处分工（设计规格 §2.1，不要混）
///
/// | 维度 | 载体 | 为什么 |
/// | --- | --- | --- |
/// | 语义槽位色 | `ColorScheme`（`theme.dart` 里 `fromSeed`） | Material 已定义，重造会双份真相 |
/// | 依赖 `onSurface` 的叠色 | [AppColors]（`ThemeExtension`） | 必须随主题插值，`lerp` 有意义 |
/// | 尺寸 / 时长 / 曲线 / 断点 | `const` 常量类 | 本应用 dark-only、无换肤 ⇒ 无主题动画需求，`lerp` 价值≈0 |
library;

import 'package:flutter/material.dart';

import 'breakpoints.dart';

/// 品牌色与固定底色。
///
/// 这些**不随主题变化**，故用 `const` 而非 `ThemeExtension`。
abstract final class AppPalette {
  /// `ColorScheme.fromSeed` 的唯一种子（沿用既有产品视觉基因，不换）。
  static const Color seed = Color(0xFF7C5CFF);

  /// 脚手架底 + 舞台底（合并了原先散在两处的同一硬编码）。
  static const Color stageBackdrop = Color(0xFF0B0D12);

  /// 聊天面板底。
  static const Color chatPanelSurface = Color(0xFF10131A);

  /// 助手气泡底。
  static const Color bubbleAssistant = Color(0xFF1B2030);

  /// 失败气泡 / 错误条底。
  static const Color dangerSurface = Color(0xFF3A1D1D);

  /// 错误条描边。
  static const Color dangerBorder = Color(0xFF8A3B3B);

  /// 语义色：成功（已连接、测试通过）。
  static const Color success = Color(0xFF3DD68C);

  /// 语义色：警告（连接中、待重启、服务端静音徽标）。
  static const Color warning = Color(0xFFF5A524);

  /// 语义色：失败（已关闭、解析失败）。
  static const Color danger = Color(0xFFFF6B6B);

  /// 登记表（只服务测试的声明侧枚举）。**生产代码不得用本表取值**。
  static const Map<String, Color> registry = <String, Color>{
    'seed': seed,
    'stageBackdrop': stageBackdrop,
    'chatPanelSurface': chatPanelSurface,
    'bubbleAssistant': bubbleAssistant,
    'dangerSurface': dangerSurface,
    'dangerBorder': dangerBorder,
    'success': success,
    'warning': warning,
    'danger': danger,
  };
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
    required this.serverMutedBadgeSurface,
    required this.serverMutedBadgeBorder,
  });

  /// 由 `ColorScheme` 派生一套。**唯一的构造入口**——避免各处自己乘透明度。
  factory AppColors.of(ColorScheme scheme) {
    final Color on = scheme.onSurface;
    const Color black = Color(0xFF000000);
    return AppColors(
      // dark 下用 1 px 描边表达层级，而不是投影。
      hairline: on.withValues(alpha: 0.12),
      hoverWash: on.withValues(alpha: 0.06),
      glassScrim: black.withValues(alpha: 0.60),
      glassBarrier: black.withValues(alpha: 0.32),
      // 承载文字信息的次要文本（≥ 12 px）。
      contentMuted: on.withValues(alpha: 0.74),
      // **仅装饰/图标**，不得承载文字信息。
      contentFaint: on.withValues(alpha: 0.60),
      // 键盘焦点环**必须不透明**（对比度要可测）。
      focusRing: scheme.primary,
      serverMutedBadgeSurface: AppPalette.warning.withValues(alpha: 0.18),
      serverMutedBadgeBorder: AppPalette.warning,
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

  /// 服务端静音只读徽标的底。
  final Color serverMutedBadgeSurface;

  /// 服务端静音只读徽标的描边。
  final Color serverMutedBadgeBorder;

  /// 字段个数（供测试做「新增字段忘了 copyWith/lerp」的结构枚举）。
  static const int fieldCount = 9;

  @override
  AppColors copyWith({
    Color? hairline,
    Color? hoverWash,
    Color? glassScrim,
    Color? glassBarrier,
    Color? contentMuted,
    Color? contentFaint,
    Color? focusRing,
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

  /// 「已打断」的保持时长（规格 §6.3 的瞬时状态）。
  ///
  /// 长到人眼能读完「已打断」，短到不会和下一轮重叠。
  static const Duration interruptedHold = Duration(milliseconds: 1200);

  // 刻意**没有**单独的「流式三点周期」：三点跑动与呼吸光表达的是同一件事
  // （思考中），共用 [thinkingBreath] 让两个指示器**同频**——看起来是有意为之，
  // 而不是各跑各的。两个同值令牌只会制造「改一处忘一处」的机会。

  /// 登记表（只服务测试）。
  static const Map<String, Duration> registry = <String, Duration>{
    'thinkingBreath': thinkingBreath,
    'interruptedHold': interruptedHold,
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
