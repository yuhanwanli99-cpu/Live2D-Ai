part of 'tokens.dart';

/// **材质旋钮**（2026-09-27）：与配色**正交**的一小组缩放系数。
///
/// # 为什么单独一个类，而不是两个 `double` 参数
///
/// Dart 的可选参数列表**不能混用**（`[a]` 与 `{b}` 不能共存），
/// 而 `buildAppTheme()` / `buildAppTheme(id)` 两种旧调用形式必须继续有效。
/// 打包成一个对象既保住了旧调用，又让「配色轴」与「材质轴」在签名上就分开——
/// 将来再加旋钮不会退化成第三、第四个散参数。
///
/// # 抄的是什么
///
/// Morrow 的 `VisualStyle.radiusScale`（`lib/appearance.dart:26`）证明了
/// 「一个旋钮缩放整套圆角」比逐控件调整便宜得多，而本项目此前**对外观没有任何
/// 表达**（除了 4 套配色）。抄的是这个**机制**，不是它的 7 种风格——
/// 4 套配色 × 7 种风格 = 28 种组合的回归面，与「刻意不做功能堆砌」的裁决冲突。
@immutable
class AppMaterial {
  const AppMaterial({
    this.radiusScale = kFixedRadiusScale,
    this.edgeStrength = 1.0,
    this.uiTransparency = 0.0,
  });

  /// **圆角缩放系数 —— 固定值，不再是用户可调项**（2026-09-27 减法）。
  ///
  /// 用户口径：「本身 web 端无需繁杂设置」。所以滑杆与偏好字段都删了，
  /// 只留**这一个数**作为全仓库圆角的唯一入口。
  ///
  /// 机制**没有删**：`AppColors.radius(token)` 仍然是所有表面的取圆角方式。
  /// 保留它是因为它把「圆角从哪来」收在一个地方——将来若真要调，
  /// 改这一个常数即可，而**不用**再去翻十几个 `BorderRadius.circular`。
  final double radiusScale;

  /// **界面**的透明程度（0 = 面板不透明，1 = 尽量透）。
  ///
  /// 与「背景图不透明度」是**两个轴**：那个调的是图，这个调的是面板。
  /// 落地在 [AppColors.panelAlpha]（面板面的 alpha）。
  final double uiTransparency;

  /// **圆角缩放系数 —— 全仓库固定值**（2026-09-27 减法）。
  ///
  /// 用户口径：「本身 web 端无需繁杂设置」。所以「圆角幅度」滑杆与它的
  /// 偏好字段都删了，只留这一个数作为**所有**圆角的唯一入口。
  ///
  /// 机制**没有删**：[AppColors.radius] 仍然是每一处表面的取圆角方式。
  /// 留它的理由是把「圆角从哪来」收在一处——将来若真要调，改这一个常数，
  /// 而不用去翻十几处 `BorderRadius.circular`。
  static const double kFixedRadiusScale = 1.0;

  /// `AppColors.hairline` 透明度的缩放系数。
  ///
  /// **不缩放**焦点环 / 危险描边：它们是可用性下限，不是审美旋钮。
  final double edgeStrength;

  /// 中性取值（两个旋钮都不动）＝**改动前的观感**。
  static const AppMaterial neutral = AppMaterial();

  // ⚠️ 这三个字段**每一个**都要出现在这里与 [hashCode] 里。
  // 2026-09-27 漏了 `uiTransparency`：两个只有透明度不同的 AppMaterial 判为相等，
  // 于是「只改界面透明」的那一次主题切换**不会被认成变化**。
  // 漏字段不会报错、不会崩，只是那一次改动静默不生效——本项目 P4 的头号病。
  @override
  bool operator ==(Object other) =>
      other is AppMaterial &&
      other.radiusScale == radiusScale &&
      other.edgeStrength == edgeStrength &&
      other.uiTransparency == uiTransparency;

  @override
  int get hashCode => Object.hash(radiusScale, edgeStrength, uiTransparency);

  @override
  String toString() =>
      'AppMaterial(radius: $radiusScale, edge: $edgeStrength, '
      'uiTransparency: $uiTransparency)';
}

/// **浮起面的阴影**（2026-09-27）。
///
/// # 为什么是函数而不是 `const` 列表
///
/// 阴影颜色跟亮暗走（暗色用舞台底压暗、亮色用淡墨），所以它不是常数。
/// 两条纪律：
///
/// 1. **只有我们自己构建的盒子能用它**（设置面板、会话抽屉…）。
///    Material 的 `Card` / `Dialog` / `SnackBar` 拿的是 `elevation`，
///    而 `Material` 把 elevation 直接交给引擎的 `Canvas::drawShadow`
///    （`painting.dart:8408`）——形状算死、主题层改不了。
///    那些控件**继续 elevation 0**，层级由 [AppPalette.raised] 那一档面差承担。
/// 2. **只给一条、向下的软阴影**。有了三级面差，再叠多层阴影只会把画面做糊。
List<BoxShadow> appRaisedShadow(AppPalette palette) => <BoxShadow>[
  BoxShadow(
    color: palette.dark
        ? palette.stage.withValues(alpha: 0.55)
        : palette.ink.withValues(alpha: 0.16),
    offset: const Offset(0, 8),
    blurRadius: 24,
  ),
];

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

