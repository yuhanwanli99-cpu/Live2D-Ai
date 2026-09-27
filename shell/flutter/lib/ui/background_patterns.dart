/// 内置背景图案的绘制（渐变 / 光晕 / 网格 / 斜纹）。
///
/// # 为什么是 `CustomPainter` 而不是图片资源
///
/// 1. **零存储**：四个图案加起来 0 字节，而一张图要占几十到几百 KB 的
///    localStorage 配额（背景库的总预算只有 1.5 M 字符）。
/// 2. **零网络**：断网可用是本项目的红线。
/// 3. **任意分辨率不糊**：`Cover` 的图片在 4K 屏上会被放大，图案不会。
/// 4. **跟着主题走**：配色由 [patternColorsFor] 从当前 `AppPalette` 算出来，
///    所以切主题时背景一起变，不需要为每套主题各存一份图。
///
/// 代价：它不是「用户自己的图」。所以它们与图片**同列**在背景库里
/// （`BackgroundItem` 的判别联合），用户可以把渐变和照片排在一起轮播。
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../design/background_item.dart';
import 'background_logic.dart';

/// 画一个内置图案。
class BackgroundPatternPainter extends CustomPainter {
  const BackgroundPatternPainter({required this.id, required this.colors});

  final int id;
  final PatternColors colors;

  @override
  void paint(Canvas canvas, Size size) {
    switch (id) {
      case BackgroundPatternId.gradient:
        _paintGradient(canvas, size);
      case BackgroundPatternId.glow:
        _paintGlow(canvas, size);
      case BackgroundPatternId.grid:
        _paintGrid(canvas, size);
      case BackgroundPatternId.stripes:
        _paintStripes(canvas, size);
      default:
        break;
    }
  }

  /// 线性渐变：左上 → 右下，accent 色 → 次级面色的更深一档。
  void _paintGradient(Canvas canvas, Size size) {
    final Rect rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: <Color>[colors.a, colors.b],
        ).createShader(rect),
    );
  }

  /// 三团径向光晕。位置与半径**固定**（不随帧变），所以不会「呼吸」。
  void _paintGlow(Canvas canvas, Size size) {
    final double w = size.width;
    final double h = size.height;
    final Paint paint = Paint();
    void blob(Offset center, double radius, Color color) {
      paint.shader = RadialGradient(
        colors: <Color>[color, color.withValues(alpha: 0)],
      ).createShader(Rect.fromCircle(center: center, radius: radius));
      canvas.drawCircle(center, radius, paint);
    }

    blob(
      Offset(w * 0.53, h * 0.27),
      w * 0.44,
      colors.a.withValues(alpha: 0.55),
    );
    blob(
      Offset(w * 0.95, h * 0.82),
      w * 0.37,
      colors.b.withValues(alpha: 0.50),
    );
    blob(
      Offset(w * 0.35, h * 1.03),
      w * 0.37,
      colors.a.withValues(alpha: 0.35),
    );
  }

  /// 细网格点阵：**固定种子**，所以点不会每帧重排。
  void _paintGrid(Canvas canvas, Size size) {
    final math.Random rng = math.Random(kGridSeed);
    final Paint paint = Paint()..color = colors.ink;
    for (double y = kGridSpacing; y < size.height; y += kGridSpacing) {
      for (double x = kGridSpacing; x < size.width; x += kGridSpacing) {
        paint.color = colors.ink.withValues(
          alpha: kGridDotMin + (kGridDotMax - kGridDotMin) * rng.nextDouble(),
        );
        canvas.drawCircle(Offset(x, y), 1, paint);
      }
    }
  }

  /// 45° 斜细纹，间距 [kGridSpacing] 的两倍（比网格更安静）。
  void _paintStripes(Canvas canvas, Size size) {
    final Paint paint = Paint()
      ..color = colors.ink
      ..strokeWidth = 1;
    final double gap = kGridSpacing * 2;
    // 从左下往右上画，保证覆盖整个矩形（只按 width 算会漏掉斜线的端点）。
    for (double x = -size.height; x < size.width + size.height; x += gap) {
      canvas.drawLine(
        Offset(x, size.height),
        Offset(x + size.height, 0),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(BackgroundPatternPainter old) =>
      old.id != id ||
      old.colors.a != colors.a ||
      old.colors.b != colors.b ||
      old.colors.ink != colors.ink;
}

/// 一个内置图案的迷你预览（设置里的图库格子用）。
class PatternPreview extends StatelessWidget {
  const PatternPreview({
    required this.id,
    required this.colors,
    this.size = kPatternPreviewSize,
    super.key,
  });

  final int id;
  final PatternColors colors;
  final double size;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: size,
    height: size,
    child: RepaintBoundary(
      child: CustomPaint(
        painter: BackgroundPatternPainter(id: id, colors: colors),
      ),
    ),
  );
}
