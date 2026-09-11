/// 玻璃边缘高光：**一圈跟着指针走的描边**，不含任何模糊。
///
/// 2026-09-11（前端加强计划 P3-1）新增。形状与参数来自 Morrow 的
/// `LiquidRimPainter`（`lib/liquid_glass.dart:248-313`，Apache-2.0，
/// 见 `CREDITS.md` §6.5），但**刻意只搬了它的一半**。
///
/// # 为什么只搬「边缘高光」，不搬它的玻璃本体
///
/// Morrow 的玻璃是 `BackdropFilter` + `ImageFilter.shader` 做逐像素折射。
/// 本项目两条红线都碰不得：
///
/// 1. **舞台是 `<iframe>` 平台视图**：`BackdropFilter` 的输入是 Flutter 的
///    图层合成结果，**平台视图不在其中**——压在舞台上的玻璃会采到空/错色，
///    而模糊的代价照付（规格 §12-7，`test/no_backdrop_filter_test.dart` 守着
///    「命中数必须为 0」）。
/// 2. **Morrow 自己也知道这条路到不了底**：它唯一那个 `BackdropFilter`
///    被 `_filterEnabled` 挡着，透明画布上「中央一个像素都不画，只保留边缘
///    高光」（`lib/liquid_glass.dart:204-206` 的注释）。
///
/// 所以这里搬的正是它**已经验证过的那个逃生分支**：只用 `CustomPaint`
/// 画描边——零模糊、零采样、零依赖，压在平台视图上也不会有任何问题。
///
/// # 三层描边（数字直接照抄，别即兴调）
///
/// | 层 | 内缩 | 线宽 | 颜色 |
/// | --- | --- | --- | --- |
/// | 外圈 | 0.8 | 1.6 | 四段渐变：基色 .94 → .08 → 冷灰 .12 → .55 |
/// | 中圈 | 2 | 3 | 基色 .17 → 透明（柔光） |
/// | 内圈 | 5 | 0.65 | 基色 .10 |
///
/// 这些数字是 Morrow 用**像素级测试**钉过的（中心 alpha == 0、上边缘 alpha > 0），
/// 比我们从零调参便宜得多。
///
/// # 基色来自主题，不是写死的白
///
/// `AppColors.rimHighlight` = 当前主题的墨色：暗主题是近白（亮边）、
/// 亮主题是近黑（暗边）。写死白色会在**白色主题上完全消失**——
/// 这正是「四套主题逐套验」的意义。
library;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'theme.dart';

/// 指针不在面板上时的默认光向（左上）。
///
/// 取 `(-.65, -.8)` 而不是正上方：光源稍微偏左更像自然光，
/// 也让圆角矩形的左上角成为最亮处（人眼预期那里最亮）。
const Offset _kRestingLight = Offset(-0.65, -0.8);

/// 给 [child] 加一圈跟随指针的边缘高光。
///
/// 只加描边：**不改底色、不加模糊、不采样背景**。所以它可以安全地
/// 压在舞台 iframe 上。
class GlassRim extends StatefulWidget {
  const GlassRim({
    required this.child,
    this.borderRadius,
    this.intensity,
    this.duration,
    super.key,
  });

  final Widget child;

  /// 圆角（要和 [child] 自己的圆角一致，否则描边会跟内容错位）。
  final BorderRadius? borderRadius;

  /// 高光强度 0..1。
  ///
  /// `null` = 按主题亮度取：暗主题 `1.0`（亮边本来就在暗底上跳出来），
  /// 亮主题 `0.45`（近黑的边在浅底上要更克制，否则看着像加粗的边框）。
  final double? intensity;

  /// 光向跟随的时长。缺省 `AppDurations.fast`。
  final Duration? duration;

  @override
  State<GlassRim> createState() => _GlassRimState();
}

class _GlassRimState extends State<GlassRim> {
  Offset _light = _kRestingLight;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final bool dark = appPaletteOf(context).dark;
    final double intensity = widget.intensity ?? (dark ? 1 : 0.45);
    final BorderRadius radius =
        widget.borderRadius ?? BorderRadius.circular(AppRadius.lg);

    // 指针位置 → 归一化光向。减少动画时**不做跟随**（那是持续的鼠标驱动
    // 重绘，正是「减少动画」要少掉的那类东西），固定到一个方向即可。
    final bool trackPointer = !MediaQuery.disableAnimationsOf(context);

    return MouseRegion(
      onHover: trackPointer
          ? (PointerHoverEvent event) {
              final RenderBox? box =
                  context.findRenderObject() as RenderBox?;
              if (box == null || box.size.isEmpty) return;
              final Offset p = box.globalToLocal(event.position);
              setState(
                () => _light = Offset(
                  (p.dx / box.size.width * 2 - 1).clamp(-1.0, 1.0),
                  (p.dy / box.size.height * 2 - 1).clamp(-1.0, 1.0),
                ),
              );
            }
          : null,
      onExit: (_) => setState(() => _light = _kRestingLight),
      child: Stack(
        fit: StackFit.passthrough,
        children: <Widget>[
          widget.child,
          Positioned.fill(
            // 描边只是装饰：绝不吃指针（面板上的按钮必须正常可点）。
            child: IgnorePointer(
              child: TweenAnimationBuilder<Offset>(
                tween: Tween<Offset>(end: _light),
                duration: appMotionLite(context, widget.duration),
                curve: Curves.easeOutCubic,
                builder: (BuildContext context, Offset light, Widget? _) =>
                    CustomPaint(
                      painter: LiquidRimPainter(
                        radius: radius.topLeft.x,
                        light: light,
                        highlight: colors.rimHighlight,
                        cool: colors.glassBarrier,
                        intensity: intensity,
                      ),
                    ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// 光向跟随的时长。
///
/// 这里是 `AppDurations.fast`（120 ms）——比 Morrow 的 180 ms 快一档，
/// 因为本项目只有 4 档时长，而 120 ms 那一档的语义正是
/// 「指针悬停这类即时反馈」。180 ms 那种「跟着鼠标慢慢追」在本项目里
/// 属于新造一档，不值得。
Duration appMotionLite(BuildContext context, Duration? override) =>
    MediaQuery.disableAnimationsOf(context)
    ? Duration.zero
    : (override ?? AppDurations.fast);

/// 三层描边（见文件头注的对照表）。
class LiquidRimPainter extends CustomPainter {
  const LiquidRimPainter({
    required this.radius,
    required this.light,
    required this.highlight,
    required this.cool,
    this.intensity = 1,
  });

  final double radius;
  final Offset light;

  /// 高光基色（主题墨色）。
  final Color highlight;

  /// 渐变中段那一抹冷色（`glassBarrier`，四套主题各自成立）。
  final Color cool;

  final double intensity;

  @override
  void paint(Canvas canvas, Size size) {
    if (size.isEmpty || intensity <= 0) return;

    final Rect rect = (Offset.zero & size).deflate(0.8);
    final RRect rrect = RRect.fromRectAndRadius(
      rect,
      Radius.circular(radius),
    );

    // 外圈：四段渐变，方向由指针决定——这是「光在动」的全部来源。
    canvas.drawRRect(
      rrect,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.6
        ..shader = LinearGradient(
          begin: Alignment(light.dx, light.dy),
          end: Alignment(-light.dx, -light.dy),
          colors: <Color>[
            highlight.withValues(alpha: 0.94 * intensity),
            highlight.withValues(alpha: 0.08 * intensity),
            cool.withValues(alpha: 0.12 * intensity),
            highlight.withValues(alpha: 0.55 * intensity),
          ],
          stops: const <double>[0, 0.36, 0.66, 1],
        ).createShader(rect),
    );

    // 中圈：内缩 2 px 的柔光，让边缘看起来有厚度。
    canvas.drawRRect(
      rrect.deflate(2),
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 3
        ..shader = LinearGradient(
          begin: Alignment(light.dx, light.dy),
          end: Alignment.center,
          colors: <Color>[
            highlight.withValues(alpha: 0.17 * intensity),
            Colors.transparent,
          ],
        ).createShader(rect),
    );

    // 内圈：一道极细的实边，把玻璃「封口」。
    canvas.drawRRect(
      rrect.deflate(5),
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 0.65
        ..color = highlight.withValues(alpha: 0.10 * intensity),
    );
  }

  @override
  bool shouldRepaint(LiquidRimPainter old) =>
      old.intensity != intensity ||
      old.radius != radius ||
      old.light != light ||
      old.highlight != highlight ||
      old.cool != cool;
}
