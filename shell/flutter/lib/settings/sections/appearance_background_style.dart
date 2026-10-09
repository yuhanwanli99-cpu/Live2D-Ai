part of 'appearance_section.dart';

/// 缩略图：走 [decodeDataUrlBytes]，坏图退回纯色面而不是红屏。
class _ImageTile extends StatelessWidget {
  const _ImageTile({required this.dataUrl, required this.palette});

  final String dataUrl;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);
    if (bytes == null) return ColoredBox(color: palette.surfaceAlt);
    return Image.memory(
      bytes,
      fit: BoxFit.cover,
      // 缩略图只按 96 px 解码：不缩放的话一张大图会以原尺寸进缓存。
      cacheWidth: kPatternPreviewSize.toInt(),
      gaplessPlayback: true,
      errorBuilder: (_, _, _) => ColoredBox(color: palette.surfaceAlt),
    );
  }
}
