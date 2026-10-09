/// 壳全局背景（2026-09-27 重写）：铺在**整个壳**背后的一层。
///
/// # 它和舞台背景是什么关系（没有变）
///
/// 舞台背景走渲染面协议 `stage-bg`（iframe 里的 canvas 自己画）；
/// 壳背景是 **Flutter 自己画的**（聊天 / 侧栏背后那片区域）。
/// 壳画背景库。当前这一项由宿主用 [DisplayPrefs.stageProjectionUrl] 投影到 `stage-bg`。
/// 两条管道仍然分开：图案和超限大图只画在壳上，舞台回到纯色。
///
/// # 这一版多了什么（2026-09-28 · Stage B · B-a）
///
/// - **铺法四档**：`cover` / `contain` / `stretch`（`FittedBox(fit: BoxFit.fill)`）/
///   `tile`（[ImageRepeat.repeat] 平铺，贴片边长 = [ShellBackdrop.tileSize]）；
/// - **逐图样式覆盖**：[BackgroundImage] 上的 `opacity/fit/align` 三项
///   （`null` = 没设过 ⇒ 回落全局）。解析**只在本文件的 [_imageLayer] 里做一次**，
///   遮罩与图层读同一个「解析后」的值；
/// - **全局开关** [ShellBackdrop.enabled]（DEC-4）：关掉 = 不画图、只留底色；
/// - 背景库（多图 + 内置图案 + 轮播）、九宫格位置、模糊、可读性遮罩、换图过渡。
///
/// 全部只读 `DisplayPrefs`，**一个字节都不落盘**（除了偏好里的那几个数）。
///
/// # 仍然不做
///
/// - **模糊舞台**：舞台是 `<iframe>` 平台视图，模糊不到它（F1）。
///   本文件的模糊只作用在壳自己画的这一层。
/// - **分区背景**：那要改渲染面 = 改后端。
/// - **任意 CSS**：逐图样式是**有类型**的三个字段，不是 CSS 字符串（parity §4 D4）。
library;

import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import '../data/background_decode_cache.dart';
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
///
/// **同一个串返回同一个 `Uint8List` 实例**（2026-09-28 · F-0006-1 / F-0003-1）：
/// 实现搬去 [decodeDataUrlBytesCached]（`data/background_decode_cache.dart`），
/// 这里只留委托。为什么非这样不可：`MemoryImage.==` 比的是 bytes 的**对象身份**，
/// 每次新建实例 = `ImageCache` 恒 miss = 每个流式 delta 重解码整张壁纸。
/// 语义**一字未改**：判据、`null` 约定、不抛约定都与搬家前逐字相同。
Uint8List? decodeDataUrlBytes(String? dataUrl) =>
    decodeDataUrlBytesCached(dataUrl);

/// 平铺层的 key（测试按它断言「这一档真的走了平铺那条路」）。
const Key kShellTileKey = ValueKey<String>('shell-tile');

/// 平铺贴片的 `paintImage` scale：让**一块贴片的宽度**正好是 [tileSize] 逻辑像素。
///
/// `paintImage` 的贴片尺寸 = `图片固有尺寸 ÷ scale`，所以
/// `scale = 固有宽 ÷ 想要的贴片宽`；高度按同一 scale 走 ⇒ 贴片保持原图比例
/// （等价于 CSS `background-size: <tileSize>px auto`）。
///
/// 固有宽 ≤ 0（还没解码 / 空图）或 `tileSize` 非法 → 回 1.0（不缩放、不抛）。
double tileScaleFor(int imageWidth, double tileSize) {
  if (imageWidth <= 0 || !tileSize.isFinite || tileSize <= 0) return 1.0;
  return imageWidth / tileSize;
}

/// 铺法 int → `BoxFit`（`DisplayPrefs.fitName` 的翻译层）。
///
/// | fit | 名字 | `BoxFit` | 这一层怎么画 |
/// | --- | --- | --- | --- |
/// | 0 | cover | `cover` | 铺满、裁掉溢出 |
/// | 1 | contain | `contain` | 完整放进来、留边 |
/// | 2 | stretch | `fill` | `FittedBox` 包一层（见下） |
/// | 3 | tile | `none` | 按原尺寸平铺（贴片由 [tileScaleFor] 定） |
///
/// # 为什么 `stretch` 要 `FittedBox` 包一层
///
/// `Image` 的 `fit: BoxFit.fill` 已经会「不管比例铺满」，但它的布局仍受父约束
/// 影响：图比框**小**时不会被放大到失真铺满。`FittedBox(fit: BoxFit.fill)`
/// 先让子节点按**固有尺寸**布局、再整体拉伸到框上，语义与 CSS
/// `background-size: 100% 100%` 一致，且对「图比框小」同样成立。
/// 代价是一次额外布局（只在选「拉伸」时付）。
///
/// 区间外的值一律回 `cover`（与 `DisplayPrefs.fitName` 的兜底一致）。
BoxFit boxFitFor(int fit) => switch (fit) {
  DisplayPrefs.fitContain => BoxFit.contain,
  DisplayPrefs.fitStretch => BoxFit.fill,
  DisplayPrefs.fitTile => BoxFit.none,
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
    this.enabled = DisplayPrefs.defaultBackgroundEnabled,
    this.opacity = DisplayPrefs.defaultBackgroundOpacity,
    this.fit = DisplayPrefs.defaultImageFit,
    this.align = DisplayPrefs.defaultImageAlign,
    this.tileSize = DisplayPrefs.defaultTileSize,
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

  /// **全局背景开关**（DEC-4）：`false` = 不画图，只留底色。
  ///
  /// ⚠️ 调用点必须传 `prefs.backgroundEnabled`（`app/app_shell.dart`）。
  /// 默认 `true` 只是让旧调用点/测试少改一行，**不是**「这个开关可以没接线」。
  final bool enabled;

  final double opacity;
  final int fit;
  final int align;

  /// 平铺（[fit] = `DisplayPrefs.fitTile`）的**贴片边长**（逻辑像素）；
  /// 其余三档不读它。
  ///
  /// 产品路径传 [DisplayPrefs.defaultTileSize]。平铺档不再暴露给用户。
  final double tileSize;

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
    // DEC-4：全局开关关掉 = 不画图（底色 + 子树），与「没有背景」同一条路。
    // 它**不动**用户的透明度设置：重新打开还是他原来看到的样子。
    if (!enabled || current == null || !current.isRenderable) {
      return ColoredBox(color: baseColor, child: child);
    }

    // 逐图 ?? 全局：三项覆盖**只解析一次**。解析点住在 `DisplayPrefs`
    // （`effectiveImage*`），与 `hasBackgroundAt` 共用同一个函数——
    // 判据与画面不许各自解析一套。
    final double effectiveOpacity = DisplayPrefs.effectiveImageOpacity(
      current,
      opacity,
    );
    final int effectiveFit = DisplayPrefs.effectiveImageFit(current, fit);
    final int effectiveAlign = DisplayPrefs.effectiveImageAlign(current, align);

    final Widget? layer = switch (current) {
      final BackgroundImage img => img.dataUrl == null
          ? null
          : _imageLayer(
              img.dataUrl!,
              effectiveOpacity,
              effectiveFit,
              effectiveAlign,
            ),
      final BackgroundPattern p =>
        patternColors == null ? null : _patternLayer(p.id, patternColors!),
    };
    if (layer == null || effectiveOpacity <= 0) {
      return ColoredBox(color: baseColor, child: child);
    }

    final double scrimAlpha = scrimAlphaFor(
      imageOpacity: effectiveOpacity,
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

  /// 图片层（`opacity / fit / align` 已是**解析后**的值）。
  ///
  /// 四个档位在这里分道：
  ///
  /// - `cover` / `contain`：`Image.memory(fit: …)`；
  /// - `stretch`：`FittedBox(fit: BoxFit.fill)` 包一层（先按固有尺寸布局，
  ///   再整体拉满——`Image` 自己的 `fill` 在「图比框小」时不会放大铺满）；
  /// - `tile`：[_TiledImage]（`ImageRepeat.repeat`，贴片 = [tileSize] 逻辑像素）。
  ///
  /// **坏图 / 坏 base64 一律退回底色**（返回空层，下面的底色照常露出）——
  /// 沿用 [decodeDataUrlBytes] 的既有约定：不抛、不红屏。
  Widget _imageLayer(String dataUrl, double opacity, int fit, int align) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);
    if (bytes == null) return const SizedBox.shrink();
    final Widget content;
    if (fit == DisplayPrefs.fitTile) {
      content = _TiledImage(
        bytes: bytes,
        tileSize: tileSize,
        fit: fit,
        alignment: alignmentFor(align),
      );
    } else if (fit == DisplayPrefs.fitStretch) {
      content = FittedBox(
        fit: boxFitFor(fit),
        child: Image.memory(
          bytes,
          gaplessPlayback: true,
          // 字节合法但**不是图片**时退回底色，而不是红屏 + 异常。
          errorBuilder: (_, _, _) => const SizedBox.shrink(),
        ),
      );
    } else {
      content = Image.memory(
        bytes,
        fit: boxFitFor(fit),
        alignment: alignmentFor(align),
        gaplessPlayback: true,
        errorBuilder: (_, _, _) => const SizedBox.shrink(),
      );
    }
    Widget layer = Opacity(opacity: opacity.clamp(0.0, 1.0), child: content);
    if (blur > 0) {
      // sigma 用半径换算（σ ≈ r/2），再乘 0.5 让滑杆的手感更线性。
      layer = ImageFiltered(
        imageFilter: ui.ImageFilter.blur(sigmaX: blur / 2, sigmaY: blur / 2),
        child: layer,
      );
    }
    return layer;
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
        imageFilter: ui.ImageFilter.blur(sigmaX: blur / 2, sigmaY: blur / 2),
        child: layer,
      );
    }
    return Opacity(opacity: opacity.clamp(0.0, 1.0), child: layer);
  }
}
/// 平铺一层图（`ImageRepeat.repeat`，贴片边长 = [ShellBackdrop.tileSize]）。
///
/// # 为什么不用 `Image(repeat: ImageRepeat.repeat)`
///
/// 那个 API 的**贴片尺寸**是「图片固有像素 ÷ `scale`」，而固有尺寸要等解码
/// 才知道。用 `Image(width: tileSize)` 只改**布局框**，贴片本身仍是固有尺寸
/// ——于是 1×1 的图会平铺成一片噪声，而 4000 px 的图一块就占满屏幕。
/// 用户要的是「一块 24 px 的小贴片」，所以这里拿解码后的 `ui.Image` 自己调
/// `paintImage`：`scale` 由 [tileScaleFor] 算出，贴片边长是**绝对**的。
///
/// **坏图不抛**：还没解码完、或解码失败（`onError`），这一层就只是不画
/// （`CustomPaint.painter` 为 null），底色照旧——与 [decodeDataUrlBytes]
/// 的既有约定一致。
class _TiledImage extends StatefulWidget {
  const _TiledImage({
    required this.bytes,
    required this.tileSize,
    required this.fit,
    required this.alignment,
  });

  final Uint8List bytes;
  final double tileSize;
  final int fit;
  final Alignment alignment;

  @override
  State<_TiledImage> createState() => _TiledImageState();
}

class _TiledImageState extends State<_TiledImage> {
  ImageStream? _stream;
  ImageStreamListener? _listener;
  ui.Image? _image;

  @override
  void initState() {
    super.initState();
    _resolve();
  }

  @override
  void didUpdateWidget(covariant _TiledImage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.bytes, widget.bytes)) {
      _detach();
      _resolve();
    }
  }

  @override
  void dispose() {
    _detach();
    super.dispose();
  }

  void _resolve() {
    final ImageStream stream = MemoryImage(
      widget.bytes,
    ).resolve(ImageConfiguration.empty);
    final ImageStreamListener listener = ImageStreamListener(
      (ImageInfo info, bool synchronousCall) {
        if (!mounted || identical(_image, info.image)) return;
        // 同步回调可能发生在 build 期间 ⇒ 不能直接 setState。
        if (synchronousCall) {
          scheduleMicrotask(() {
            if (mounted && !identical(_image, info.image)) {
              setState(() => _image = info.image);
            }
          });
        } else {
          setState(() => _image = info.image);
        }
      },
      // 解码失败 = 这一层画不出来（底色照旧），**不抛**。
      onError: (Object _, StackTrace? _) {},
    );
    _listener = listener;
    _stream = stream..addListener(listener);
  }

  void _detach() {
    final ImageStreamListener? listener = _listener;
    if (listener != null) _stream?.removeListener(listener);
    _stream = null;
    _listener = null;
  }

  @override
  Widget build(BuildContext context) {
    final ui.Image? image = _image;
    return SizedBox.expand(
      key: kShellTileKey,
      child: CustomPaint(
        painter: image == null
            ? null
            : _TilePainter(
                image: image,
                scale: tileScaleFor(image.width, widget.tileSize),
                fit: boxFitFor(widget.fit),
                alignment: widget.alignment,
              ),
      ),
    );
  }
}

/// 真正平铺的那支笔（把 `paintImage` 的 `repeat` 铺满整块）。
class _TilePainter extends CustomPainter {
  const _TilePainter({
    required this.image,
    required this.scale,
    required this.fit,
    required this.alignment,
  });

  final ui.Image image;
  final double scale;
  final BoxFit fit;
  final Alignment alignment;

  @override
  void paint(Canvas canvas, Size size) {
    paintImage(
      canvas: canvas,
      rect: Offset.zero & size,
      image: image,
      scale: scale,
      fit: fit,
      repeat: ImageRepeat.repeat,
      alignment: alignment,
      filterQuality: FilterQuality.low,
    );
  }

  @override
  bool shouldRepaint(covariant _TilePainter oldDelegate) =>
      !identical(oldDelegate.image, image) ||
      oldDelegate.scale != scale ||
      oldDelegate.fit != fit ||
      oldDelegate.alignment != alignment;
}
