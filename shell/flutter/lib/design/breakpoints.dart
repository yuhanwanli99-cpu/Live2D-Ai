/// 断点：**纯逻辑、零依赖**，可在 VM 上单测。
///
/// 抽成纯函数的理由（设计规格 §3.2）：原实现把 `constraints.maxWidth >= 900`
/// 直接写在 `LayoutBuilder` 里，于是**断点行为无法单测**——只能靠人工改窗口宽度看。
/// 这是本次重构在可测试性上最大的净收益。
library;

/// 三档尺寸类。
///
/// 阈值与设计规格一致，**不要就地写魔法数字**：
/// 任何 `maxWidth >= 1280` 这类比较都必须走 [sizeClassOf]。
enum SizeClass {
  /// < 900：窄屏（手机 / 窄窗口）。导航走抽屉 + 底部弹层。
  compact,

  /// 900–1279：中屏。设置走底部浮层，舞台与聊天并排。
  medium,

  /// ≥ 1280：宽屏。设置侧板浮在舞台列之上（导航只有 AppBar 的「设置」一处，
  /// 分区切换在面板内的 chip 行——左侧 rail 已于 2026-09-11 删除、宽度还给舞台）。
  expanded,
}

/// 各档下限（含）。**唯一真源**——`sizeClassOf` 与测试都读这里。
abstract final class Breakpoints {
  /// 中屏下限。
  static const double mediumMin = 900;

  /// 宽屏下限。
  static const double expandedMin = 1280;

  /// 令牌登记表（只服务测试的声明侧枚举，见设计规格 §2.8.1）。
  static const Map<String, double> registry = <String, double>{
    'mediumMin': mediumMin,
    'expandedMin': expandedMin,
  };

  /// 宽度 → 尺寸类。**纯函数**，边界值含等号。
  static SizeClass sizeClassOf(double width) {
    if (!width.isFinite) return SizeClass.compact;
    if (width >= expandedMin) return SizeClass.expanded;
    if (width >= mediumMin) return SizeClass.medium;
    return SizeClass.compact;
  }
}

/// [SizeClass] 的便捷扩展（读起来更顺：`sc.isWide`）。
extension SizeClassX on SizeClass {
  bool get isCompact => this == SizeClass.compact;
  bool get isMedium => this == SizeClass.medium;
  bool get isExpanded => this == SizeClass.expanded;

  /// 是否宽到可以常驻侧栏（聊天面板并排而不是上下叠）。
  bool get hasSideChat => this != SizeClass.compact;

  /// 是否宽到可以把设置做成「浮在舞台列右缘的侧板」而不是全屏浮层。
  bool get hasInlineSettings => this == SizeClass.expanded;
}
