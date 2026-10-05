part of 'settings_models.dart';

/// `POST /api/v1/settings/test/{llm,tts}` 的结果。
///
/// 契约：`web_api/settings_routes/test_endpoints.rs::TestOutcome`。
class SettingsTestOutcome {
  const SettingsTestOutcome({
    required this.ok,
    this.latencyMs,
    this.modelEcho,
    this.note,
    this.errorCode,
    this.errorMessage,
  });

  final bool ok;

  /// 往返耗时（毫秒）；服务端用 `u128`，这里收成 int。
  final int? latencyMs;

  /// 服务端回显的模型/音色名——用来确认「打到的确实是配置里那个」。
  final String? modelEcho;

  /// 成功但有话要说（例如「上游可达但未提供 /models」）。
  ///
  /// **只有真有解释价值时才非空**——服务端刻意不在成功路径上默认塞一句，
  /// 否则每次自检都多一句噪音，用户很快就不看它了（见服务端 `TestOutcome::note`）。
  final String? note;

  final String? errorCode;
  final String? errorMessage;

  factory SettingsTestOutcome.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    Map<String, Object?>? err;
    if (j['error'] is Map) {
      final Map<String, Object?> e = <String, Object?>{};
      (j['error']! as Map<Object?, Object?>).forEach((Object? k, Object? v) {
        if (k is String) e[k] = v;
      });
      err = e;
    }
    return SettingsTestOutcome(
      ok: j['ok'] == true,
      latencyMs: j['latency_ms'] is num
          ? (j['latency_ms']! as num).toInt()
          : null,
      modelEcho: j['model_echo'] is String ? j['model_echo']! as String : null,
      note: j['note'] is String ? j['note']! as String : null,
      errorCode: err?['code'] is String ? err!['code']! as String : null,
      errorMessage: err?['message'] is String
          ? err!['message']! as String
          : null,
    );
  }
}

/// `PATCH /api/v1/settings` 的响应。
///
/// 契约：服务端回 `{"persisted": bool, "settings": {…}}`。
/// 写盘后 supervisor 的装载/重载状态（`dto.rs::ApplyStatus`，`snake_case`）。
///
/// **必须消费它**：写盘成功不等于生效。不区分就会出现
/// 「提示已热重载但实际 503」的错位——用户按提示发了消息却收不到回复。
enum ApplyStatus {
  /// supervisor 动态创建成功 / reload 已触发 → 可以立刻对话。
  applied('applied'),

  /// 写盘成功，但无法动态创建 supervisor（配置不全）→ **需重启**。
  restartRequired('restart_required'),

  /// reload 已排队、异步生效中。
  ///
  /// 服务端注释说明当前实现下**实际不会产生**该值（装配/重建都在请求线程内
  /// 完成），保留作未来异步路径。前端仍然要处理它——协议里有的取值都要兜住。
  queued('queued'),

  /// 写盘成功但**没有** supervisor（dev-only 控制平面 / 关掉了对话引擎）。
  noSupervisor('no_supervisor'),

  /// 未识别取值（协议新增而前端还旧）。**不静默当成 applied**。
  unknown('unknown');

  const ApplyStatus(this.wire);

  final String wire;

  static ApplyStatus fromWire(String? raw) {
    for (final ApplyStatus value in values) {
      if (value != unknown && value.wire == raw) return value;
    }
    return unknown;
  }

  /// 是否「已经生效、可以直接对话」。
  bool get isLive => this == ApplyStatus.applied;
}

class SettingsPatchResult {
  const SettingsPatchResult({
    required this.persisted,
    required this.settings,
    this.applyStatus = ApplyStatus.unknown,
  });

  /// 服务端是否**真的写回了磁盘**。
  ///
  /// `false` 表示补丁与当前值完全一致（`PatchOutcome::NoChange`）——
  /// 界面可以据此显示「没有变化」而不是「已保存」，两者对用户不是一回事。
  final bool persisted;

  /// 应用补丁**之后**的服务端权威设置（用它回填界面，不要用本地草稿）。
  final SettingsView settings;

  /// 写盘后的生效状态（缺省 `unknown`：旧服务端 / 字段缺失）。
  final ApplyStatus applyStatus;

  factory SettingsPatchResult.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return SettingsPatchResult(
      persisted: j['persisted'] == true,
      settings: SettingsView.fromJson(_obj(j['settings'])),
      applyStatus: ApplyStatus.fromWire(
        j['apply_status'] is String ? j['apply_status']! as String : null,
      ),
    );
  }
}

/// 宽容取字符串：非字符串一律空串（响应缺字段时界面显示空，而不是崩）。
String _str(Object? value) => value is String ? value : '';

/// 宽容取整数：JSON 里 `24000` 与 `24000.0` 都合法。
int _int(Object? value) {
  if (value is int) return value;
  if (value is num) return value.toInt();
  return 0;
}

/// 宽容取 double：JSON 里 `0.75` 与 `1` 都合法；非数 → 给定缺省。
double _dbl(Object? value, double fallback) {
  if (value is num && value.isFinite) return value.toDouble();
  return fallback;
}

/// 宽容取**可空** double：非数字 / 非有限数 / 缺键 → `null`（= 未覆盖）。
///
/// 与 [_dbl] 的区别就是这层语义：覆盖里 `null` 是**有意义的**「回落全局」，
/// 不能像全局值那样回落到 0.75/0.80/1.0（那会把「没覆盖」写成一个覆盖）。
double? _dblOrNull(Object? value) {
  if (value is num && value.isFinite) return value.toDouble();
  return null;
}

/// 宽容解析 `action.models`：非对象 / 非法条目一律忽略（不抛）。
Map<String, ActionModelOverrideView> _modelViews(Object? raw) {
  if (raw is! Map) return const <String, ActionModelOverrideView>{};
  final Map<String, ActionModelOverrideView> out =
      <String, ActionModelOverrideView>{};
  raw.forEach((Object? k, Object? v) {
    if (k is! String || v is! Map) return;
    out[k] = ActionModelOverrideView.fromJson(_obj(v));
  });
  return out;
}

/// 宽容取对象：不是对象就返回 null（交给各自的 `fromJson` 走默认值）。
Map<String, Object?>? _obj(Object? value) {
  if (value is! Map) return null;
  final Map<String, Object?> out = <String, Object?>{};
  value.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}
