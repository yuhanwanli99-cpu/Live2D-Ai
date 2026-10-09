/// DisplayPrefs 的派生判据（display_prefs.dart 的 part）。
///
/// 壳画背景库。当前这一项投影到舞台是 [stageProjectionUrl]，管道仍然分开。
part of 'display_prefs.dart';

extension DisplayPrefsDerived on DisplayPrefs {
  /// 库的第一项。轮播「当前项」是外壳上的运行时索引，不在这份偏好里。
  BackgroundItem? get effectiveBackground {
    if (backgrounds.isEmpty) return null;
    return backgrounds[_clampIndex(0, backgrounds.length - 1)];
  }

  /// 库第一项的下标。空库为 0。
  int get effectiveBackgroundIndex =>
      backgrounds.isEmpty ? 0 : _clampIndex(0, backgrounds.length - 1);

  /// 第一项是不是一张图片（图案不算）。
  bool get currentItemIsImage => effectiveBackground is BackgroundImage;

  /// 有没有东西真的画得出来。渲染层把自己的当前项传进来。
  bool hasBackgroundAt(BackgroundItem? item) =>
      backgroundEnabled &&
      item != null &&
      item.isRenderable &&
      effectiveImageOpacity(item, backgroundOpacity) > 0;

  /// 逐图覆盖 ?? 全局。读入已忽略逐图键，产品路径上覆盖恒为 null。
  static double effectiveImageOpacity(BackgroundItem? item, double global) =>
      (item is BackgroundImage ? item.opacity : null) ?? global;

  static int effectiveImageFit(BackgroundItem? item, int global) =>
      (item is BackgroundImage ? item.fit : null) ?? global;

  static int effectiveImageAlign(BackgroundItem? item, int global) =>
      (item is BackgroundImage ? item.align : null) ?? global;

  bool get hasBackground => hasBackgroundAt(effectiveBackground);

  /// 当前库项要交给渲染面 `stage-bg` 的 data URL。
  ///
  /// null = 舞台回到纯色。壳仍可画图案，也可画超过帧上限的大图
  /// （那些字节在 IndexedDB，塞不进一帧）。
  static String? stageProjectionUrl(DisplayPrefs prefs, int index) {
    if (!prefs.backgroundEnabled || prefs.backgrounds.isEmpty) return null;
    final int last = prefs.backgrounds.length - 1;
    final int i = index < 0 ? 0 : (index > last ? last : index);
    final BackgroundItem item = prefs.backgrounds[i];
    if (item is! BackgroundImage) return null;
    final String? url = item.dataUrl;
    if (url == null || !isDataImageUrl(url)) return null;
    if (url.length > kStageImageMaxChars) return null;
    return url;
  }

  static int _clampIndex(int index, int max) =>
      max < 0 ? 0 : index.clamp(0, max);
}
