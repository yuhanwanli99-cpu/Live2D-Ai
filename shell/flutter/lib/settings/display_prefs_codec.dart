/// DisplayPrefs 的**序列化 / 反序列化 + 读入宽容**（display_prefs.dart 的 part）。

///

/// 2026-10-06 按 B2 从 display_prefs.dart 搬出：方法体**逐字搬运**，只补了

/// extension 作用域**必需**的类静态限定符 —— Dart 规定 extension 里读不到 on-type 的

/// 静态成员（实测报错：The getter 'def' isn't defined for the type 'Box'），

/// 所以那些标识符必须写成 DisplayPrefs.xxx；其余一字未改。

/// 落盘 24 键全集守卫：test/display_prefs_persist_keys_test.dart。

part of 'display_prefs.dart';

extension DisplayPrefsCodec on DisplayPrefs {

  /// 序列化（写入 localStorage）。
  Map<String, Object?> toJson() => <String, Object?>{
    'theme': theme.wire,
    'stageImage': stageImage,
    'backgrounds': <Object?>[
      for (final BackgroundItem b in backgrounds) b.toJson(),
    ],
    'backgroundSource': backgroundSource,
    'backgroundOpacity': backgroundOpacity,
    'backgroundBlur': backgroundBlur,
    'backgroundScrim': backgroundScrim,
    'backgroundEnabled': backgroundEnabled,
    'imageFit': imageFit,
    'imageAlign': imageAlign,
    'tileSize': tileSize,
    'slideInterval': slideInterval,
    'slideRandom': slideRandom,
    'uiTransparency': uiTransparency,
    'stagePlaylist': stagePlaylist,
    'scale': scale,
    'mouthSensitivity': mouthSensitivity,
    'lipSync': lipSync,
    'idleEnabled': idleEnabled,
    'muted': muted,
    'volume': volume,
    'allowDragZoom': allowDragZoom,
    'tier': tier,
    'edgeStrength': edgeStrength,
  };



  /// 反序列化：**永不抛异常**。
  ///
  /// 缺字段 / 类型不符 / 非有限数 / 越界值一律回落到默认值或夹到区间内——
  /// 一份坏的存储不该让舞台无法渲染（这正是单测覆盖的部分）。

  static DisplayPrefs decode(Map<String, Object?>? json) {

    if (json == null) return const DisplayPrefs();
    return DisplayPrefs(
      // 旧存档没有这个字段 → 用默认（黑），不是「假装用户选过白」。
      theme: AppThemeId.fromWire(json['theme']),
      // 坏值（非字符串 / 超限）一律当作「没有背景图」，**不抛**：
      // 一份被外部塞大的存储不该让舞台渲染不出来。
      stageImage: _readImage(json['stageImage'], kStageImageMaxChars),
      backgrounds: readBackgrounds(json),
      backgroundSource: _readBackgroundSource(json),
      backgroundOpacity: DisplayPrefs.clampBackgroundOpacity(
        _readDouble(json['backgroundOpacity'], DisplayPrefs.defaultBackgroundOpacity),
      ),
      backgroundBlur: DisplayPrefs.clampBackgroundBlur(
        _readDouble(json['backgroundBlur'], DisplayPrefs.defaultBackgroundBlur),
      ),
      backgroundScrim: DisplayPrefsLimits._clampInt(
        _readInt(json['backgroundScrim'], DisplayPrefs.defaultBackgroundScrim),
        DisplayPrefs.minBackgroundScrim,
        DisplayPrefs.maxBackgroundScrim,
        DisplayPrefs.defaultBackgroundScrim,
      ),
      // 旧档没有这个键 → `true`（迁移默认：老用户界面不变）。
      backgroundEnabled: _readBool(json['backgroundEnabled'], true),
      // 上界 3（Stage B 扩档）：旧档里的 2/3 现在是**合法档**，不再是坏值。
      imageFit: DisplayPrefsLimits._clampInt(
        _readInt(json['imageFit'], DisplayPrefs.defaultImageFit),
        0,
        DisplayPrefs.maxImageFit,
        DisplayPrefs.defaultImageFit,
      ),
      imageAlign: DisplayPrefsLimits._clampInt(
        _readInt(json['imageAlign'], DisplayPrefs.defaultImageAlign),
        0,
        DisplayPrefs.maxImageAlign,
        DisplayPrefs.defaultImageAlign,
      ),
      tileSize: DisplayPrefs.clampTileSize(_readDouble(json['tileSize'], DisplayPrefs.defaultTileSize)),
      // DEC-1：**端点夹持**（0 仍＝关），不是「越界回落默认」——
      // 回落 0 会把用户开着的轮播静默关掉（详见 [DisplayPrefs.clampSlideInterval]）。
      slideInterval: DisplayPrefs.clampSlideInterval(
        _readInt(json['slideInterval'], DisplayPrefs.defaultSlideInterval),
      ),
      slideRandom: _readBool(json['slideRandom'], false),
      uiTransparency: DisplayPrefs.clampUiTransparency(
        _readDouble(json['uiTransparency'], DisplayPrefs.defaultUiTransparency),
      ),
      stagePlaylist: readStagePlaylist(json['stagePlaylist']),
      scale: DisplayPrefs.clampScale(_readDouble(json['scale'], DisplayPrefs.defaultScale)),
      mouthSensitivity: DisplayPrefs.clampMouthSensitivity(
        _readDouble(json['mouthSensitivity'], DisplayPrefs.defaultMouthSensitivity),
      ),
      lipSync: _readBool(json['lipSync'], true),
      idleEnabled: _readBool(json['idleEnabled'], true),
      muted: _readBool(json['muted'], false),
      volume: DisplayPrefs.clampVolume(_readDouble(json['volume'], DisplayPrefs.defaultVolume)),
      // 旧存档没有这两个字段 → 用新默认值（不是 false/0）。
      allowDragZoom: _readBool(json['allowDragZoom'], true),
      tier: clampTier(_readInt(json['tier'], DisplayPrefs.defaultTier)),
      // 旧存档没有这两个字段 → 默认 1.0（不是「假装用户调过」）。
      edgeStrength: DisplayPrefs.clampEdgeStrength(
        _readDouble(json['edgeStrength'], DisplayPrefs.defaultEdgeStrength),
      ),
    );
  }



  /// 读「壳画哪一张」，**含旧字段迁移**。
  ///
  /// 旧形态的 `syncShellStageBg` 是「壳跟随舞台」，默认 `true`。迁移规则：
  ///
  /// - **确实有一张舞台图**（`stageImage != null`）→ 迁到「舞台那张」，
  ///   老用户**看到的界面一个像素都不变**；
  /// - **没有舞台图**（绝大多数用户，包括把图加进背景库的那些）→ 迁到
  ///   「背景库」，也就是新默认。
  ///
  /// 写成「读旧键」而不是「改旧键的默认值」：改默认值会让「显式设过 true」
  /// 和「从没设过」分不开，而那正是本缺陷的成因。
  static int _readBackgroundSource(Map<String, Object?> json) {
    final Object? stored = json['backgroundSource'];
    if (stored is int) {
      return stored == DisplayPrefs.backgroundSourceStageImage
          ? DisplayPrefs.backgroundSourceStageImage
          : DisplayPrefs.backgroundSourceLibrary;
    }
    final bool legacySync = _readBool(json['syncShellStageBg'], false);
    final String? stage = _readImage(json['stageImage'], kStageImageMaxChars);
    return legacySync && stage != null
        ? DisplayPrefs.backgroundSourceStageImage
        : DisplayPrefs.backgroundSourceLibrary;
  }



  /// 读背景库：数量超了截断、坏项直接丢、**永不抛**。
  ///
  /// **逐项的大小上限已经不在这里**（2026-09-27）：字节住 IndexedDB，
  /// 偏好里只剩 id，本地已经没什么可超的了。留下的只有「最多几项」。
  ///
  /// 顺带做**旧字段迁移**：`shellImage`（2026-09-14 rc.5 的单张 `String?`）
  /// → 单元素列表。写回时**不再**写 `shellImage`，所以这是一次性迁移。
  static List<BackgroundItem> readBackgrounds(Map<String, Object?> json) {
    final List<BackgroundItem> out = <BackgroundItem>[];
    final Object? raw = json['backgrounds'];
    if (raw is List) {
      for (final Object? entry in raw) {
        if (out.length >= kBackgroundMaxCount) break;
        final BackgroundItem? item = BackgroundItem.fromJson(entry);
        if (item == null) continue;
        out.add(item);
      }
      return List<BackgroundItem>.unmodifiable(out);
    }
    final String? legacy = _readImage(json['shellImage'], DisplayPrefs.kLegacyImageMaxChars);
    return legacy == null
        ? const <BackgroundItem>[]
        : <BackgroundItem>[
            // id 同样由内容算 ⇒ 这张图与背景库里同一张图会被认成同一项。
            BackgroundImage(id: backgroundIdOf(legacy), dataUrl: legacy),
          ];
  }



  static double _readDouble(Object? raw, double fallback) {
    if (raw is num) {
      final value = raw.toDouble();
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

  /// 读轮播列表：**逐项宽容**，超出预算的部分保留前面的、丢掉放不下的。
  ///
  /// 规则（三条预算各自独立生效，坏值只丢自己那一项）：
  /// 1. 非 `List` → 空列表；项非字符串 / 空串 / 单项超 [kStageImageMaxChars] → 丢该项；
  /// 2. 项数达到 [kStagePlaylistMaxItems] → 停止（后面的丢掉）；
  /// 3. 累计字符数会把总长推过 [kStagePlaylistMaxChars] → 停止（同上）。
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

  /// 读 int（非数字回落默认）。
  static int _readInt(Object? raw, int fallback) =>
      raw is num ? raw.toInt() : fallback;

}
