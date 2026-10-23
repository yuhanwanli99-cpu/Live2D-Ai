/// 导演「表演倾向 / 动作频率」的**草稿**（0.4.4-rc.2，2026-10-23）。
///
/// # 两个键
///
/// 都写进 `mods.json` 的 `mods.director.config`：
///
/// | 键 | 值 | 缺省 |
/// | --- | --- | --- |
/// | [kDirectorMotionBiasKey]（`motion_bias`） | 克制 / 平常 / 主动 | 平常 |
/// | [kDirectorMotionRateKey]（`motion_rate`） | 少 / 平常 / 多 | 平常 |
///
/// # 为什么是一份独立草稿（而不是 Mod 卡片表单）
///
/// 设置面的规则（`docs/architecture/CURRENT.md` §5.1）：改动只进草稿，只有
/// 底部「保存并重载」才一次写盘。这两个键既不属于 `live2d-ai.toml`（`[action]`
/// 幅度走 `SettingsDraft`），也不属于「扩展」里的 Mod 卡片（导演已常驻核心、
/// 不再是可启停的 Mod）——所以给它们一份自己的 [ChangeNotifier]：
///
/// - 宿主把它并进外壳的重建监听（底部按钮的可见性读 [dirty]）；
/// - 保存时宿主从 [payload] 一次拿到两个键，**一次** POST
///   `/api/v1/mods/director/config`；
/// - 读当前值走 `GET /api/v1/mods/director/config` → [adoptSaved]。
///
/// **纯逻辑、无 IO**：本类只回答「显示哪一档 / 脏不脏 / 发哪两个键」，
/// 不认识 HTTP、不认识 widget，可在 Dart VM 上单测。
library;

import 'package:flutter/foundation.dart';

/// `mods.director.config.motion_bias`（表演倾向）。
///
/// 稳定码与 Rust `live2d_ai_mod_director::motion_bias::MotionBias::parse`
/// 同一组词；**不要在 Dart 侧另起一套**。
const String kDirectorMotionBiasKey = 'motion_bias';

/// `mods.director.config.motion_rate`（动作频率）。
const String kDirectorMotionRateKey = 'motion_rate';

/// 倾向三档的稳定码（UI 中文标签在 `sections/director_section.dart`）。
const List<String> kDirectorMotionBiasValues = <String>[
  'restrained',
  'normal',
  'eager',
];

/// 频率三档的稳定码（UI 中文标签在 `sections/director_section.dart`）。
const List<String> kDirectorMotionRateValues = <String>[
  'sparse',
  'normal',
  'dense',
];

/// 缺省倾向 = 平常（缺键 / 坏值也归它，与 Rust 侧 `parse` 的回落一致）。
const String kDirectorMotionDefaultBias = 'normal';

/// 缺省频率 = 平常。
const String kDirectorMotionDefaultRate = 'normal';

/// 「表演倾向 / 动作频率」的草稿（可单测）。
///
/// # 三份值的关系
///
/// - `saved*`：**已保存**那份（服务端 `GET …/config` 读回来的，或刚保存成功的）；
/// - `bias` / `rate`：**界面此刻显示**的那份（草稿）；
/// - [dirty]：两份不等 = 有未保存改动。
///
/// [adoptSaved] 是唯一写 `saved*` 的入口（保存成功后也走 [markSaved]，语义相同）：
/// 它**不覆盖脏草稿**——后台重新载入（例如点了「扩展 → 重新载入」）不能把用户
/// 刚选的档位抹掉。反过来，**干净**草稿会跟随已保存值，这样服务端归一化 /
/// 别处改过配置之后界面不会停在旧档。
class DirectorMotionDraft extends ChangeNotifier {
  String _savedBias = kDirectorMotionDefaultBias;
  String _savedRate = kDirectorMotionDefaultRate;
  String _bias = kDirectorMotionDefaultBias;
  String _rate = kDirectorMotionDefaultRate;

  /// 界面此刻显示的倾向（稳定码）。
  String get bias => _bias;

  /// 界面此刻显示的频率（稳定码）。
  String get rate => _rate;

  /// 已保存的倾向（核对外部改动用）。
  String get savedBias => _savedBias;

  /// 已保存的频率。
  String get savedRate => _savedRate;

  /// 有未保存改动（底部「保存并重载」的可见性凭据之一）。
  bool get dirty => _bias != _savedBias || _rate != _savedRate;

  /// 改倾向：未知档位忽略（协议里只认三档），同值不通知。
  void selectBias(String value) {
    if (!kDirectorMotionBiasValues.contains(value) || value == _bias) return;
    _bias = value;
    notifyListeners();
  }

  /// 改频率：未知档位忽略，同值不通知。
  void selectRate(String value) {
    if (!kDirectorMotionRateValues.contains(value) || value == _rate) return;
    _rate = value;
    notifyListeners();
  }

  /// 采纳一份**已保存**的配置（`GET …/config` 的 `config` 对象）。
  ///
  /// - 脏草稿：只更新 `saved*` 基线，**不动**用户选的那两档；
  /// - 干净草稿：两档跟随已保存值；
  /// - 缺键 / 坏值 → 缺省平常（Rust 侧同一条回落）。
  ///
  /// 值没有变化时不发通知（避免每次后台刷新都让外壳重画一次）。
  void adoptSaved(Map<String, Object?> config) {
    final String bias = normalizeBias(config[kDirectorMotionBiasKey]);
    final String rate = normalizeRate(config[kDirectorMotionRateKey]);
    if (bias == _savedBias && rate == _savedRate) return;
    final bool wasDirty = dirty;
    _savedBias = bias;
    _savedRate = rate;
    if (!wasDirty) {
      // 干净草稿跟随已保存值（界面永远显示「已生效」的那一档）。
      _bias = bias;
      _rate = rate;
    }
    notifyListeners();
  }

  /// 保存成功：把当前两档认成已保存值。干净时不通知。
  void markSaved() {
    if (!dirty) return;
    _savedBias = _bias;
    _savedRate = _rate;
    notifyListeners();
  }

  /// 放弃草稿：回到已保存那份（切分区 / 关设置 / 取消 / 放弃改动）。
  void discard() {
    if (!dirty) return;
    _bias = _savedBias;
    _rate = _savedRate;
    notifyListeners();
  }

  /// 保存载荷：**两个键一起发**（服务端旁路一次写、一次重载）。
  Map<String, Object?> payload() => <String, Object?>{
    kDirectorMotionBiasKey: _bias,
    kDirectorMotionRateKey: _rate,
  };

  /// 倾向稳定码归一化：只认三档，其余（缺键 / null / 坏值）→ 平常。
  static String normalizeBias(Object? raw) =>
      raw is String && kDirectorMotionBiasValues.contains(raw)
      ? raw
      : kDirectorMotionDefaultBias;

  /// 频率稳定码归一化：只认三档，其余 → 平常。
  static String normalizeRate(Object? raw) =>
      raw is String && kDirectorMotionRateValues.contains(raw)
      ? raw
      : kDirectorMotionDefaultRate;
}
