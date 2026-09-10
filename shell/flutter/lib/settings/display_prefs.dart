/// 舞台显示偏好：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由：这些值会被持久化（localStorage）并在启动时反序列化，
/// 「非法/缺失/越界输入回落到什么」是必须能回归的算术，不该埋在 UI 里。
/// 持久化本身在 `main.dart` 做（那里才允许 `package:web`）。
library;

/// 舞台显示偏好。
class DisplayPrefs {
  const DisplayPrefs({
    this.scale = defaultScale,
    this.mouthSensitivity = defaultMouthSensitivity,
    this.lipSync = true,
    this.idleEnabled = true,
  });

  /// 模型缩放（渲染面 `stage-config.scale`，同区间）。
  final double scale;

  /// 口型灵敏度：乘在渲染面「RMS → dB 映射」的结果上。
  ///
  /// 默认 [defaultMouthSensitivity] = 1.0 是**实测标定值**：真实 TTS 语音的
  /// 线性 RMS p90 ≈ 0.118，经 dB 映射后约开到 0.58（见
  /// `crates/l2d-wasm-demo/src/mouth.rs` 的标定表）。
  final double mouthSensitivity;

  /// 口型总开关。
  final bool lipSync;

  /// 空闲生命体征（呼吸/眨眼/微表情）。
  final bool idleEnabled;

  /// 缩放默认值与区间（与渲染面 clamp 一致）。
  static const double defaultScale = 1.0;
  static const double minScale = 0.5;
  static const double maxScale = 2.0;

  /// 口型灵敏度默认值与区间。
  ///
  /// 上限取 3.0（而非渲染面的 4.0）：再往上普通语音会长期贴着满幅，看起来像穿模。
  static const double defaultMouthSensitivity = 1.0;
  static const double minMouthSensitivity = 0.2;
  static const double maxMouthSensitivity = 3.0;

  DisplayPrefs copyWith({
    double? scale,
    double? mouthSensitivity,
    bool? lipSync,
    bool? idleEnabled,
  }) {
    return DisplayPrefs(
      scale: scale ?? this.scale,
      mouthSensitivity: mouthSensitivity ?? this.mouthSensitivity,
      lipSync: lipSync ?? this.lipSync,
      idleEnabled: idleEnabled ?? this.idleEnabled,
    );
  }

  /// 序列化（写入 localStorage）。
  Map<String, Object?> toJson() => <String, Object?>{
    'scale': scale,
    'mouthSensitivity': mouthSensitivity,
    'lipSync': lipSync,
    'idleEnabled': idleEnabled,
  };

  /// 反序列化：**永不抛异常**。
  ///
  /// 缺字段 / 类型不符 / 非有限数 / 越界值一律回落到默认值或夹到区间内——
  /// 一份坏的存储不该让舞台无法渲染（这正是单测覆盖的部分）。
  factory DisplayPrefs.fromJson(Map<String, Object?>? json) {
    if (json == null) return const DisplayPrefs();
    return DisplayPrefs(
      scale: clampScale(_readDouble(json['scale'], defaultScale)),
      mouthSensitivity: clampMouthSensitivity(
        _readDouble(json['mouthSensitivity'], defaultMouthSensitivity),
      ),
      lipSync: _readBool(json['lipSync'], true),
      idleEnabled: _readBool(json['idleEnabled'], true),
    );
  }

  /// 缩放夹到 `[minScale, maxScale]`；非有限值回落默认。
  static double clampScale(double value) {
    if (!value.isFinite) return defaultScale;
    return value.clamp(minScale, maxScale);
  }

  /// 口型灵敏度夹到 `[minMouthSensitivity, maxMouthSensitivity]`；非有限值回落默认。
  static double clampMouthSensitivity(double value) {
    if (!value.isFinite) return defaultMouthSensitivity;
    return value.clamp(minMouthSensitivity, maxMouthSensitivity);
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

  @override
  bool operator ==(Object other) =>
      other is DisplayPrefs &&
      other.scale == scale &&
      other.mouthSensitivity == mouthSensitivity &&
      other.lipSync == lipSync &&
      other.idleEnabled == idleEnabled;

  @override
  int get hashCode => Object.hash(scale, mouthSensitivity, lipSync, idleEnabled);

  @override
  String toString() =>
      'DisplayPrefs(scale: $scale, mouth: $mouthSensitivity, '
      'lipSync: $lipSync, idle: $idleEnabled)';
}
