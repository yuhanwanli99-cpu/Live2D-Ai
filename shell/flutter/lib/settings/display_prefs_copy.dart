/// DisplayPrefs 的 copyWith（display_prefs.dart 的 part）。
part of 'display_prefs.dart';

extension DisplayPrefsCopy on DisplayPrefs {
  DisplayPrefs copyWith({
    AppThemeId? theme,
    List<BackgroundItem>? backgrounds,
    double? backgroundOpacity,
    bool? backgroundEnabled,
    int? slideInterval,
    bool? slideRandom,
    double? uiTransparency,
    double? scale,
    double? mouthSensitivity,
    bool? lipSync,
    bool? idleEnabled,
    bool? muted,
    double? volume,
    bool? allowDragZoom,
    int? tier,
  }) {
    return DisplayPrefs(
      theme: theme ?? this.theme,
      backgrounds: backgrounds ?? this.backgrounds,
      backgroundOpacity: backgroundOpacity ?? this.backgroundOpacity,
      backgroundEnabled: backgroundEnabled ?? this.backgroundEnabled,
      slideInterval: slideInterval ?? this.slideInterval,
      slideRandom: slideRandom ?? this.slideRandom,
      uiTransparency: uiTransparency ?? this.uiTransparency,
      scale: scale ?? this.scale,
      mouthSensitivity: mouthSensitivity ?? this.mouthSensitivity,
      lipSync: lipSync ?? this.lipSync,
      idleEnabled: idleEnabled ?? this.idleEnabled,
      muted: muted ?? this.muted,
      volume: volume ?? this.volume,
      allowDragZoom: allowDragZoom ?? this.allowDragZoom,
      tier: tier ?? this.tier,
    );
  }
}
