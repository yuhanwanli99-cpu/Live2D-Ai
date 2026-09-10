/// 面板容器：半透明纯色 + 1 px 高光描边 + `hairline` 分隔（L3）。
///
/// # 为什么**没有** `BackdropFilter`
///
/// 毛玻璃（`backdrop-blur`）在同类项目里是「值得直接抄的配方」，但在
/// **Flutter Web + iframe 平台视图**这个组合下它是纯代价：
/// `BackdropFilter` **模糊不到 iframe platform view**（上游 issue #184996），
/// 于是舞台区域不会被模糊，而每一帧的模糊开销照付
/// 【调研 oss §7】【附录 A-6 裁决】。
///
/// 所以本项目的「玻璃」= **半透明纯色 + 1 px 高光描边**。视觉上足够表达
/// 「浮在舞台之上」，且零 GPU 模糊成本。
/// `no_backdrop_filter_test.dart` 会扫描全仓库确保没人偷偷加回来。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'theme.dart';

/// 浮层面板。
class GlassPanel extends StatelessWidget {
  const GlassPanel({
    required this.child,
    this.padding = const EdgeInsets.all(Space.s4),
    this.borderRadius,
    this.highlight = true,
    this.maxWidth,
    super.key,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;

  /// 圆角档位；缺省 `lg`。
  final BorderRadius? borderRadius;

  /// 是否画顶部 1 px 高光描边（浮层用 true，内嵌用 false）。
  final bool highlight;

  /// 最大宽度（浮层按断点给值）。
  final double? maxWidth;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final AppColors colors = appColorsOf(context);
    final BorderRadius radius =
        borderRadius ?? BorderRadius.circular(AppRadius.lg);

    return ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth ?? double.infinity),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.glassScrim,
          borderRadius: radius,
          border: Border.all(color: colors.hairline),
        ),
        child: ClipRRect(
          borderRadius: radius,
          child: Stack(
            children: <Widget>[
              if (highlight)
                // 1 px 高光：只在上边缘，模拟「受光的玻璃」。不用渐变/模糊。
                Positioned(
                  top: 0,
                  left: 0,
                  right: 0,
                  child: Container(height: 1, color: colors.hoverWash),
                ),
              // 面板底必须比纯 scrim 更实一点，否则正文在舞台亮部读不清。
              ColoredBox(
                color: scheme.surface.withValues(alpha: 0.72),
                child: Padding(padding: padding, child: child),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
