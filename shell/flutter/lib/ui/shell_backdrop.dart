/// 壳全局背景（2026-09-27 重写）：铺在**整个壳**背后的一层。
///
/// # 它和舞台背景是什么关系（没有变）
///
/// 舞台背景走渲染面协议 `stage-bg`（iframe 里的 canvas 自己画）；
/// 壳背景是 **Flutter 自己画的**（聊天 / 侧栏背后那片区域）。
/// 两者画的是同一张还是两张**由 [DisplayPrefs.backgroundSource] 决定**，
/// 但**管道完全不同**——不要因为「看起来是同一张图」就把它们合成一条路。
///
/// # 这一版多了什么
///
/// 背景库（多图 + 内置图案 + 轮播）、铺法（cover/contain 两档，
/// `DisplayPrefs.maxImageFit = 1`）、
/// 九宫格位置、模糊、可读性遮罩、换图过渡。
/// 全部只读 `DisplayPrefs`，**一个字节都不落盘**（除了偏好里的那几个数）。
///
/// # 仍然不做
///
/// - **模糊舞台**：舞台是 `<iframe>` 平台视图，模糊不到它（F1）。
///   本文件的模糊只作用在壳自己画的这一层。
/// - **分区背景**：那要改渲染面 = 改后端。
library;

import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';

import '../design/background_item.dart';
import '../settings/display_prefs.dart';
import 'background_logic.dart';
import 'background_patterns.dart';
import 'theme.dart';

/// 遮罩层的 key（测试按它断言「有没有画遮罩」）。
///
/// 为什么不靠 widget 类型断言：框架自己也会插 `IgnorePointer`
/// （MaterialApp / 路由都在用），按类型数会把它们算进来。
const Key kShellScrimKey = ValueKey<String>('shell-scrim');

/// 把 dataURL 解成图片字节。
///
/// **永不抛异常**：不是 dataURL（没有逗号）/ 不是 base64 / 解码失败一律回 `null`，
/// 由调用方退回「只有底色」。理由与 `DisplayPrefs.fromJson` 同一条：
/// 一份被外部塞坏的存储不该让整个壳渲染不出来。
Uint8List? decodeDataUrlBytes(String? dataUrl) {
  if (dataUrl == null) return null;
  final int comma = dataUrl.indexOf(',');
  if (comma <= 0) return null;
  final String meta = dataUrl.substring(0, comma);
  if (!meta.contains('base64')) return null;
  try {
    final Uint8List bytes = base64Decode(dataUrl.substring(comma + 1));
    return bytes.isEmpty ? null : bytes;
  } catch (_) {
    return null;
  }
}

/// 铺法 int → `BoxFit`（`DisplayPrefs.fitName` 的翻译层）。
///
/// **没有 `BoxFit.fill` 那一档的实现**：`fill` 与 `stretch` 在 Flutter 里是同一个
/// 枚举值，所以「拉伸」这一档**画的时候按 cover 处理**——真要拉伸得自己画
/// `FittedBox(fit: BoxFit.fill)` 包一层（会引入一次额外布局）。
/// 保留这一档是为了存储里能表达用户的意图，未来接上时不用改数据形状。
BoxFit boxFitFor(int fit) => switch (fit) {
  1 => BoxFit.contain,
  _ => BoxFit.cover,
};

/// 九宫格索引 → `Alignment`（`DisplayPrefs.alignTable` 的翻译层）。
Alignment alignmentFor(int index) {
  final ({double x, double y}) cell = DisplayPrefs
      .alignTable[index.clamp(0, DisplayPrefs.alignTable.length - 1)];
  return Alignment(cell.x, cell.y);
}

/// 壳根背景层。
///
/// 结构（从下到上）：
///
/// 1. **底色** `baseColor`（必须不透明，否则透明处会露出 app 的默认底色）；
/// 2. **图案**（若当前项是内置图案）；
/// 3. **图片**（若当前项是图片）按 `opacity` × `fit` × `alignment` 铺，
///    可选 [blur]（`ImageFiltered`，外面套 `RepaintBoundary` 让它只算一次）；
/// 4. **可读性遮罩**（强度由 [scrimAlphaFor] 推，颜色是主题的 `stage`）；
/// 5. [child]。
///
/// 库里没有东西、或当前项**还没有字节**（[BackgroundItem.isRenderable] 为
/// false）时，这一层**等价于** `ColoredBox(baseColor, child)`——
/// 画不出来就画底色，不画「半个空位」。
class ShellBackdrop extends StatelessWidget {
  const ShellBackdrop({
    required this.baseColor,
    required this.item,
    required this.child,
    this.opacity = DisplayPrefs.defaultBackgroundOpacity,
    this.fit = DisplayPrefs.defaultImageFit,
    this.align = DisplayPrefs.defaultImageAlign,
    this.blur = DisplayPrefs.defaultBackgroundBlur,
    this.scrim = DisplayPrefs.defaultBackgroundScrim,
    this.uiTransparency = 0.0,
    this.patternColors,
    super.key,
  });

  /// 底色（取主题的舞台色 `AppPalette.stage`，与壳原来的脚手架底一致）。
  final Color baseColor;

  /// 当前要画的那一项（`null` = 没有背景，只有底色）。
  final BackgroundItem? item;

  final double opacity;
  final int fit;
  final int align;

  /// 模糊半径（px）。**不做补间**——动高斯半径是实测出来的性能回归
  /// （见计划书 §9.4）。
  final double blur;

  /// 遮罩档位（见 [ScrimLevel]）。
  final int scrim;

  /// 界面的透明程度（0–1）。**只用来加强遮罩**——它本身不画任何东西。
  final double uiTransparency;

  /// 图案要用的配色（只有 `item is BackgroundPattern` 时有意义）。
  final PatternColors? patternColors;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final BackgroundItem? current = item;
    if (current == null || !current.isRenderable || opacity <= 0) {
      return ColoredBox(color: baseColor, child: child);
    }

    final Widget? layer = switch (current) {
      BackgroundImage(:final String? dataUrl) => dataUrl == null
          ? null
          : _imageLayer(dataUrl),
      BackgroundPattern(:final int id) =>
        patternColors == null ? null : _patternLayer(id, patternColors!),
    };
    if (layer == null) return ColoredBox(color: baseColor, child: child);

    final double scrimAlpha = scrimAlphaFor(
      imageOpacity: opacity,
      level: scrim,
      uiTransparency: uiTransparency,
    );

    return ColoredBox(
      color: baseColor,
      child: Stack(
        fit: StackFit.expand,
        children: <Widget>[
          // 图片/图案层外面套 RepaintBoundary：模糊与不透明度变化
          // 不应该让下面整棵子树重绘。
          RepaintBoundary(child: layer),
          if (scrimAlpha > 0)
            Positioned.fill(
              key: kShellScrimKey,
              // 遮罩只是压暗，**不吃指针**——压在上面的控件必须照常可点
              // （这条与 `StagePointerInterceptor` 是同一个道理）。
              child: IgnorePointer(
                child: ColoredBox(
                  color: scrimColorFor(appPaletteOf(context))
                      .withValues(alpha: scrimAlpha),
                ),
              ),
            ),
          child,
        ],
      ),
    );
  }

  Widget _imageLayer(String dataUrl) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);
    if (bytes == null) return const SizedBox.shrink();
    Widget image = Opacity(
      opacity: opacity,
      child: Image.memory(
        bytes,
        fit: boxFitFor(fit),
        alignment: alignmentFor(align),
        gaplessPlayback: true,
        // 字节合法但**不是图片**时退回底色，而不是红屏 + 异常。
        errorBuilder: (_, _, _) => const SizedBox.shrink(),
      ),
    );
    if (blur > 0) {
      // sigma 用半径换算（σ ≈ r/2），再乘 0.5 让滑杆的手感更线性。
      image = ImageFiltered(
        imageFilter: ImageFilter.blur(sigmaX: blur / 2, sigmaY: blur / 2),
        child: image,
      );
    }
    return image;
  }

  /// 图案层。
  ///
  /// 2026-09-27：**模糊对它同样生效**（之前只给图片加了 `ImageFiltered`，
  /// 于是切到图案后「模糊」滑杆静默失灵）。铺法与位置对图案**不适用**——
  /// 它是 `CustomPainter` 画满整块、没有「原图尺寸」可言，
  /// 所以那两项在设置里按「当前是图案」藏起来，而不是留两个无效控件。
  Widget _patternLayer(int id, PatternColors colors) {
    Widget layer = CustomPaint(
      painter: BackgroundPatternPainter(id: id, colors: colors),
      size: Size.infinite,
    );
    if (blur > 0) {
      layer = ImageFiltered(
        imageFilter: ImageFilter.blur(sigmaX: blur / 2, sigmaY: blur / 2),
        child: layer,
      );
    }
    return Opacity(opacity: opacity.clamp(0.0, 1.0), child: layer);
  }
}
