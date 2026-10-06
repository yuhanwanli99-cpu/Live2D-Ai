/// DisplayPrefs 的 copyWith（display_prefs.dart 的 part）。

///

/// 2026-10-06 按 B2 从 display_prefs.dart **逐字搬出**，方法体一字未改

/// （原样剪贴 + 加 extension 头）。扩展方法对 on-type 的**实例字段**可直接引用。

part of 'display_prefs.dart';

extension DisplayPrefsCopy on DisplayPrefs {

  DisplayPrefs copyWith({
    AppThemeId? theme,
    // 刻意**不能**用 `copyWith()` 把背景图设成「还是原值」——清除走
    // `clearStageImage: true`，这样 `copyWith()` 不会因为「参数缺省 = null」
    // 而把「设成 null（清除）」和「不改」混成同一件事。
    String? stageImage,
    bool clearStageImage = false,
    List<BackgroundItem>? backgrounds,
    int? backgroundSource,
    double? backgroundOpacity,
    double? backgroundBlur,
    int? backgroundScrim,
    bool? backgroundEnabled,
    int? imageFit,
    int? imageAlign,
    double? tileSize,
    int? slideInterval,
    bool? slideRandom,
    double? uiTransparency,
    List<String>? stagePlaylist,
    double? scale,
    double? mouthSensitivity,
    bool? lipSync,
    bool? idleEnabled,
    bool? muted,
    double? volume,
    bool? allowDragZoom,
    int? tier,
    double? edgeStrength,
  }) {
    return DisplayPrefs(
      theme: theme ?? this.theme,
      stageImage: clearStageImage ? null : (stageImage ?? this.stageImage),
      backgrounds: backgrounds ?? this.backgrounds,
      backgroundSource: backgroundSource ?? this.backgroundSource,
      backgroundOpacity: backgroundOpacity ?? this.backgroundOpacity,
      backgroundBlur: backgroundBlur ?? this.backgroundBlur,
      backgroundScrim: backgroundScrim ?? this.backgroundScrim,
      backgroundEnabled: backgroundEnabled ?? this.backgroundEnabled,
      imageFit: imageFit ?? this.imageFit,
      imageAlign: imageAlign ?? this.imageAlign,
      tileSize: tileSize ?? this.tileSize,
      slideInterval: slideInterval ?? this.slideInterval,
      slideRandom: slideRandom ?? this.slideRandom,
      uiTransparency: uiTransparency ?? this.uiTransparency,
      stagePlaylist: stagePlaylist ?? this.stagePlaylist,
      scale: scale ?? this.scale,
      mouthSensitivity: mouthSensitivity ?? this.mouthSensitivity,
      lipSync: lipSync ?? this.lipSync,
      idleEnabled: idleEnabled ?? this.idleEnabled,
      muted: muted ?? this.muted,
      volume: volume ?? this.volume,
      allowDragZoom: allowDragZoom ?? this.allowDragZoom,
      tier: tier ?? this.tier,
      edgeStrength: edgeStrength ?? this.edgeStrength,
    );
  }

}
