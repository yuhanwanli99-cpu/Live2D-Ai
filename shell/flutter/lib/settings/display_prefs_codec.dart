/// DisplayPrefs 的序列化 / 反序列化（display_prefs.dart 的 part）。
///
/// 落盘 15 键。旧键 `stageImage` / `backgroundSource` / `edgeStrength` /
/// 铺法 / 遮罩 / 模糊 / `stagePlaylist` 读入时忽略，不再写回。
/// 背景库为空且 `stageImage` 是合法 data URL 时，把它收成库里的第一项。
/// 落盘全集守卫：test/display_prefs_persist_keys_test.dart。
part of 'display_prefs.dart';

extension DisplayPrefsCodec on DisplayPrefs {
  /// 序列化（写入 localStorage）。
  Map<String, Object?> toJson() => <String, Object?>{
    'theme': theme.wire,
    'backgrounds': <Object?>[
      for (final BackgroundItem b in backgrounds) b.toJson(),
    ],
    'backgroundOpacity': backgroundOpacity,
    'backgroundEnabled': backgroundEnabled,
    'slideInterval': slideInterval,
    'slideRandom': slideRandom,
    'uiTransparency': uiTransparency,
    'scale': scale,
    'mouthSensitivity': mouthSensitivity,
    'lipSync': lipSync,
    'idleEnabled': idleEnabled,
    'muted': muted,
    'volume': volume,
    'allowDragZoom': allowDragZoom,
    'tier': tier,
  };

  /// 反序列化：**永不抛异常**。
  ///
  /// 缺字段 / 类型不符 / 非有限数 / 越界值一律回落到默认值或夹到区间内。
  static DisplayPrefs decode(Map<String, Object?>? json) {
    if (json == null) return const DisplayPrefs();
    return DisplayPrefs(
      theme: AppThemeId.fromWire(json['theme']),
      backgrounds: readBackgrounds(json),
      backgroundOpacity: DisplayPrefs.clampBackgroundOpacity(
        _readDouble(
          json['backgroundOpacity'],
          DisplayPrefs.defaultBackgroundOpacity,
        ),
      ),
      backgroundEnabled: _readBool(json['backgroundEnabled'], true),
      slideInterval: DisplayPrefs.clampSlideInterval(
        _readInt(json['slideInterval'], DisplayPrefs.defaultSlideInterval),
      ),
      slideRandom: _readBool(json['slideRandom'], false),
      uiTransparency: DisplayPrefs.clampUiTransparency(
        _readDouble(json['uiTransparency'], DisplayPrefs.defaultUiTransparency),
      ),
      scale: DisplayPrefs.clampScale(
        _readDouble(json['scale'], DisplayPrefs.defaultScale),
      ),
      mouthSensitivity: DisplayPrefs.clampMouthSensitivity(
        _readDouble(
          json['mouthSensitivity'],
          DisplayPrefs.defaultMouthSensitivity,
        ),
      ),
      lipSync: _readBool(json['lipSync'], true),
      idleEnabled: _readBool(json['idleEnabled'], true),
      muted: _readBool(json['muted'], false),
      volume: DisplayPrefs.clampVolume(
        _readDouble(json['volume'], DisplayPrefs.defaultVolume),
      ),
      allowDragZoom: _readBool(json['allowDragZoom'], true),
      tier: clampTier(_readInt(json['tier'], DisplayPrefs.defaultTier)),
    );
  }

  /// 读背景库：数量超了截断、坏项直接丢、**永不抛**。
  ///
  /// **内置图案读回即丢**（2026-10-08）：渐变 / 光晕 / 网格 / 斜纹不再是产品
  /// 能力。旧存档里的 `kind == "pattern"` 项在这里被丢掉，于是**不会**被写回
  /// （[toJson] 只序列化库里的项），壳与舞台也就不会再画它们。
  ///
  /// 旧字段迁移（都只在库为空时发生，写回不再产生旧键）：
  ///
  /// - 没有 `backgrounds` 列表时，`shellImage` → 一项；
  /// - 库仍然为空且 `stageImage` 是长度内的 data URL → 收成第一项。
  ///   库里已经有项时不插入 `stageImage`。
  static List<BackgroundItem> readBackgrounds(Map<String, Object?> json) {
    final List<BackgroundItem> out = <BackgroundItem>[];
    final Object? raw = json['backgrounds'];
    if (raw is List) {
      for (final Object? entry in raw) {
        if (out.length >= kBackgroundMaxCount) break;
        final BackgroundItem? item = BackgroundItem.fromJson(entry);
        if (item == null) continue;
        // 内置图案读回即丢（见头注）：它不占配额，所以不参与截断计数。
        if (item is BackgroundPattern) continue;
        out.add(item);
      }
    } else {
      final String? legacy = _readImage(
        json['shellImage'],
        DisplayPrefs.kLegacyImageMaxChars,
      );
      if (legacy != null) {
        out.add(BackgroundImage(id: backgroundIdOf(legacy), dataUrl: legacy));
      }
    }
    if (out.isEmpty) {
      final String? stage = _readImage(json['stageImage'], kStageImageMaxChars);
      if (stage != null && isDataImageUrl(stage)) {
        out.add(BackgroundImage(id: backgroundIdOf(stage), dataUrl: stage));
      }
    }
    return List<BackgroundItem>.unmodifiable(out);
  }

  static double _readDouble(Object? raw, double fallback) {
    if (raw is num) {
      final double value = raw.toDouble();
      if (value.isFinite) return value;
    }
    return fallback;
  }

  static bool _readBool(Object? raw, bool fallback) =>
      raw is bool ? raw : fallback;

  /// 读背景图：非字符串 / 空串 / 超过 [maxChars] 都当作没有。
  static String? _readImage(Object? raw, int maxChars) {
    if (raw is! String || raw.isEmpty) return null;
    if (raw.length > maxChars) return null;
    return raw;
  }

  /// 读旧轮播列表。偏好本身不再保存这个字段；纯函数仍给测试用。
  static List<String> readStagePlaylist(Object? raw) {
    if (raw is! List) return const <String>[];
    final List<String> out = <String>[];
    int total = 0;
    for (final Object? item in raw) {
      if (out.length >= kStagePlaylistMaxItems) break;
      if (item is! String || item.isEmpty) continue;
      if (item.length > kStageImageMaxChars) continue;
      if (total + item.length > kStagePlaylistMaxChars) break;
      out.add(item);
      total += item.length;
    }
    return out;
  }

  static int _readInt(Object? raw, int fallback) =>
      raw is num ? raw.toInt() : fallback;
}
