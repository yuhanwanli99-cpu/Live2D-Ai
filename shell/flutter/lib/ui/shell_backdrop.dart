/// 壳全局背景（2026-09-14，rc.5）：铺在**整个壳**背后的一层背景图。
///
/// # 为什么单独成文件
///
/// 「把 dataURL 解成图片字节」是一段**纯逻辑**（不碰 web、不碰渲染面），
/// 必须能在 VM 上单测——坏 dataURL 不许把壳变成红屏。UI 部分只是把它铺开。
///
/// # 它和舞台背景是什么关系
///
/// 舞台背景走渲染面协议 `stage-bg`（iframe 里的 canvas 自己画）；
/// 壳背景是 **Flutter 自己画的**（聊天 / 侧栏背后那片区域）。
/// 两者可以在偏好里共用**同一张图**（`DisplayPrefs.effectiveShellImage`），
/// 但**管道完全不同**——不要因为「看起来是同一张图」就把它们合成一条路。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';

/// 壳背景的**固定**透明度。
///
/// 2026-09-14 用户口径：固定低透明度**约 0.15**，**不做** opacity 滑条——
/// 这个数只为「让壳不至于一块死色」而存在，不是给用户调的参数。
const double kShellBackdropOpacity = 0.15;

/// 有壳背景时聊天面板的面透明度。
///
/// 0.15 的图压在下面，面板留一点透（0.86 而不是 1.0）才看得到背景；
/// 但它仍然**接近不透明**，所以文字对比度与不透明面上的标定值几乎无差
/// （「聊天/侧栏保持可读」这条不是靠感觉，是靠这个下界守住的）。
const double kShellSurfaceAlpha = 0.86;

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

/// 壳根背景层：先铺 [baseColor]（**必须不透明**，否则透明处会露出 app 的
/// 默认底色），再以 [opacity] 盖一张 cover 的 [image]，最后放 [child]。
///
/// [image] 为 `null` 或解不出字节时**等价于** `ColoredBox(baseColor, child)`——
/// 没有背景图时这一层是零观感差异的。
class ShellBackdrop extends StatelessWidget {
  const ShellBackdrop({
    required this.baseColor,
    required this.image,
    required this.child,
    this.opacity = kShellBackdropOpacity,
    super.key,
  });

  /// 底色（取主题的舞台色 `AppPalette.stage`，与壳原来的脚手架底一致）。
  final Color baseColor;

  /// 壳背景图 dataURL（`DisplayPrefs.effectiveShellImage`）。
  final String? image;

  /// 背景图透明度（固定，不暴露给用户）。
  final double opacity;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final Uint8List? bytes = decodeDataUrlBytes(image);
    if (bytes == null) return ColoredBox(color: baseColor, child: child);
    return ColoredBox(
      color: baseColor,
      child: Stack(
        fit: StackFit.expand,
        children: <Widget>[
          Positioned.fill(
            child: Opacity(
              opacity: opacity,
              child: Image.memory(
                bytes,
                fit: BoxFit.cover,
                gaplessPlayback: true,
                // 字节合法但**不是图片**时退回底色，而不是红屏 + 异常。
                errorBuilder: (_, _, _) => const SizedBox.shrink(),
              ),
            ),
          ),
          child,
        ],
      ),
    );
  }
}
