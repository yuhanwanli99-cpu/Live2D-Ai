/// 字号阶梯：**纯逻辑、零 web 依赖**，可在 VM 上单测。
///
/// 8 级，映射到 `TextTheme` 槽位；**业务代码只读槽位，不写 `fontSize:`**
/// （由 `test/design_tokens_lint_test.dart` 扫描守住）。
///
/// # 对设计规格的两处偏离（都是为了能真的跑起来，语义不变）
///
/// **① 「档位两两不同」在字号上不可能成立。**
/// 规格 §2.8.2 的测试要求 `AppFontSizes.sizesOf` 的取值两两互异，
/// 但规格 §2.3 **自己的表**里 `bodyMedium` 与 `titleSmall` **同为 14 px**
/// （前者 w400 是正文、后者 w600 是分组小标题）。两条要求互相矛盾。
/// 本实现取**机械上真正有意义**的那条：**不得有两个槽位的「字号 + 字重」完全相同**
/// （那才是真正的冗余槽位），并额外钉住「**只有一个槽位用 16 px**」——
/// 后者正是规格想治的那个病（旧的 `main.dart` 16/w700 与 `display_panel.dart`
/// 16/w600 两个「同级标题」；`display_panel.dart` 已于 2026-09-11 作为死代码删除，
/// 此处保留引用只为让「为什么要这条约束」可被检索）。
///
/// **② 类名带 `App` 前缀。** 见 `tokens.dart` 里 `AppRadius` 的同款说明。
library;

import 'package:flutter/material.dart';

/// 8 级字号的**声明侧唯一真源**。
///
/// 只暴露 [registry] 一张表：槽位名 → (字号, 字重, 行高 px)。
/// **刻意不给每个槽位单独开一个 `xxxSize` 常量**——那样会有两个真源，
/// 且常量很容易变成「声明了但没人引用」的死令牌（同类项目里最典型的静默失效）。
abstract final class AppFontSizes {
  /// 8 个槽位的完整声明。
  ///
  /// 槽位语义（与 Material 的 `TextTheme` 槽位一一对应）：
  /// - `labelSmall` 11：角色名、来源徽标、时间戳、字段单位
  /// - `bodySmall` 12：辅助说明、错误条正文、日志行
  /// - `labelLarge` 13：按钮文字、字段标签
  /// - `bodyMedium` 14：**正文**（聊天气泡、字段值、设置说明）
  /// - `titleSmall` 14：分组小标题（字号同正文，靠**字重**区分角色）
  /// - `titleMedium` 16：分区标题、面板标题。**全阶梯唯一使用 16 px 的槽位。**
  /// - `titleLarge` 20：应用名 / 大屏空态标题
  /// - `headlineSmall` 24：首次引导页标题（唯一可选级）
  static const Map<String, ({double size, FontWeight weight, double lineHeight})>
  registry =
      <String, ({double size, FontWeight weight, double lineHeight})>{
        'labelSmall': (size: 11, weight: FontWeight.w500, lineHeight: 16),
        'bodySmall': (size: 12, weight: FontWeight.w400, lineHeight: 18),
        'labelLarge': (size: 13, weight: FontWeight.w500, lineHeight: 18),
        'bodyMedium': (size: 14, weight: FontWeight.w400, lineHeight: 22),
        'titleSmall': (size: 14, weight: FontWeight.w600, lineHeight: 20),
        'titleMedium': (size: 16, weight: FontWeight.w600, lineHeight: 24),
        'titleLarge': (size: 20, weight: FontWeight.w600, lineHeight: 28),
        'headlineSmall': (size: 24, weight: FontWeight.w600, lineHeight: 32),
      };

  /// 派生视图：槽位 → 字号。
  static Map<String, double> get sizesOf => <String, double>{
    for (final MapEntry<String, ({double size, FontWeight weight, double lineHeight})> e
        in registry.entries)
      e.key: e.value.size,
  };

  /// 从 [theme] 里读出某槽位**实际生效**的字号。
  ///
  /// 这是「声明 ↔ 生效」对账的关键：只查登记表等于自己骗自己，
  /// 必须确认主题里真的落了这个值。
  static double? sizeOfSlot(ThemeData theme, String slot) =>
      _slotOf(theme, slot)?.fontSize;

  /// 从 [theme] 里读出某槽位实际生效的字重。
  static FontWeight? weightOfSlot(ThemeData theme, String slot) =>
      _slotOf(theme, slot)?.fontWeight;

  static TextStyle? _slotOf(ThemeData theme, String slot) {
    final TextTheme t = theme.textTheme;
    return switch (slot) {
      'labelSmall' => t.labelSmall,
      'bodySmall' => t.bodySmall,
      'labelLarge' => t.labelLarge,
      'bodyMedium' => t.bodyMedium,
      'titleSmall' => t.titleSmall,
      'titleMedium' => t.titleMedium,
      'titleLarge' => t.titleLarge,
      'headlineSmall' => t.headlineSmall,
      _ => null,
    };
  }
}

/// 按 [AppFontSizes] 生成 8 个槽位的 [TextTheme]，叠加在 [base] 之上。
///
/// **实现要点（踩过的坑）**：必须用 `baseSlot.copyWith(...)` 派生，**不能**新建
/// `TextStyle(...)`。因为 `TextTheme.copyWith` 是**整体替换**槽位而不是合并，
/// 新建的样式 `fontFamily` 为 `null`，会把 `ThemeData.fontFamily` 设好的字体族
/// **悄悄抹掉**——于是 CanvasKit 找不到中文字形、又回去 `fonts.gstatic.com` 下载，
/// 断网就变豆腐块。`theme_test.dart` 的 CJK 用例正是为守这一条而存在。
///
/// **不设置 `fontFamily`**：字体族由 `ThemeData.fontFamily` 统一决定
/// （自托管的 `NotoSansSC`），组件层不得覆盖字体族——这是 Nexus 的教训
/// （它的两个核心 CSS 丢了中文字体）。
TextTheme buildAppTextTheme(TextTheme base) {
  TextStyle tune(String name, TextStyle? baseStyle) {
    final ({double size, FontWeight weight, double lineHeight}) spec =
        AppFontSizes.registry[name]!;
    return (baseStyle ?? const TextStyle()).copyWith(
      fontSize: spec.size,
      fontWeight: spec.weight,
      // Flutter 的 `height` 是**行高倍数**，登记表里存的是 px，这里换算。
      height: spec.lineHeight / spec.size,
    );
  }

  return base.copyWith(
    labelSmall: tune('labelSmall', base.labelSmall),
    bodySmall: tune('bodySmall', base.bodySmall),
    labelLarge: tune('labelLarge', base.labelLarge),
    bodyMedium: tune('bodyMedium', base.bodyMedium),
    titleSmall: tune('titleSmall', base.titleSmall),
    titleMedium: tune('titleMedium', base.titleMedium),
    titleLarge: tune('titleLarge', base.titleLarge),
    headlineSmall: tune('headlineSmall', base.headlineSmall),
  );
}
