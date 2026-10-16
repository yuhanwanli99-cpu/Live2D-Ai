/// 密钥真源 API（`/api/v1/env`，rc.2 2026-09-12）。
///
/// # 契约
///
/// - `GET /api/v1/env` → 配置里声明的键名 + **是否已设置**（**永不返回值**）；
/// - `PUT /api/v1/env` → 写 `.env` 的单个键（`{"key","value"}`），写完热重载，
///   响应回 `{key, set, apply_status}`（2026-10-16，FINDINGS S-002）：**写盘成功
///   不等于引擎装上**，[EnvWriteResult] 把两件事分开带回给界面。
///
/// # 为什么前端可以写密钥了
///
/// rc.2 用户口径：**`.env` 是唯一密钥真源**，且前端可以写它。以前密钥只在
/// 后端**进程环境**里，界面上能改模型名却改不了 key——用户的体验是「保存了，
/// 还是 401」，而且没有任何地方能改。
///
/// 安全边界没有放松：值**只会往上走**（PUT），**永不下来**（GET 只有布尔）。
/// 所以这个客户端里没有任何「读回密钥」的方法，也不该有。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';
import 'settings_models.dart';

/// 一个键的状态（**没有 value 字段，这是刻意的**）。
class EnvKey {
  const EnvKey({required this.section, required this.key, required this.set});

  /// 归属段：`llm` / `tts`。
  final String section;

  /// 环境变量名（来自配置的 `api_key_env`）。
  final String key;

  /// 是否已有非空值。
  final bool set;

  factory EnvKey.fromJson(Map<String, Object?> json) => EnvKey(
    section: json['section'] is String ? json['section']! as String : '',
    key: json['key'] is String ? json['key']! as String : '',
    set: json['set'] == true,
  );
}

/// `GET /api/v1/env` 的结果。
class EnvStatus {
  const EnvStatus({this.keys = const <EnvKey>[], this.envFile = ''});

  final List<EnvKey> keys;

  /// `.env` 的绝对路径（排障用：告诉用户「该改哪个文件」）。
  final String envFile;

  /// 取某段的键（没有声明则返回 `null`）。
  EnvKey? forSection(String section) {
    for (final EnvKey k in keys) {
      if (k.section == section && k.key.isNotEmpty) return k;
    }
    return null;
  }
}

/// `PUT /api/v1/env` 的结果：**写盘**与**是否装上**是两件事。
///
/// `set` 说的是 `.env` 里现在有没有这个键；`applyStatus` 说的是 supervisor
/// 有没有真的重建/热重载 LLM/TTS client。**只有 `applied` 才等于「立即生效」**。
class EnvWriteResult {
  const EnvWriteResult({required this.set, this.applyStatus = ApplyStatus.unknown});

  /// 写完之后该键是否已设置（空值写入 = 删除 = `false`）。
  final bool set;

  /// 热重载状态（缺字段 = 旧服务端 → [ApplyStatus.unknown]，**不静默当成已生效**）。
  final ApplyStatus applyStatus;

  factory EnvWriteResult.fromJson(Map<String, Object?> json) => EnvWriteResult(
    set: json['set'] == true,
    applyStatus: ApplyStatus.fromWire(
      json['apply_status'] is String ? json['apply_status']! as String : null,
    ),
  );
}

class EnvApi {
  EnvApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri() => Uri.parse('$_base/api/v1/env');

  /// `GET /api/v1/env`。
  Future<EnvStatus> list() async {
    final http.Response response = await _guard(() => _client.get(_uri()));
    if (response.statusCode != 200) throw _error(response);
    final Map<String, Object?> body = _decode(response.body);
    final Object? raw = body['keys'];
    return EnvStatus(
      envFile: body['env_file'] is String ? body['env_file']! as String : '',
      keys: raw is List
          ? raw
                .whereType<Map<Object?, Object?>>()
                .map(
                  (Map<Object?, Object?> m) => EnvKey.fromJson(_stringKeys(m)),
                )
                .where((EnvKey k) => k.key.isNotEmpty)
                .toList()
          : const <EnvKey>[],
    );
  }

  /// `PUT /api/v1/env`：写入（空值 = 删除该键）。
  ///
  /// 返回值同时带「写完是否已设置」与「引擎有没有装上」（见 [EnvWriteResult]）。
  Future<EnvWriteResult> write(String key, String value) async {
    final http.Response response = await _guard(
      () => _client.put(
        _uri(),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, String>{'key': key, 'value': value}),
      ),
    );
    if (response.statusCode != 200) throw _error(response);
    return EnvWriteResult.fromJson(_decode(response.body));
  }

  Future<http.Response> _guard(Future<http.Response> Function() run) async {
    try {
      return await run();
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
  }

  ApiException _error(http.Response response) {
    final Map<String, Object?> body = _decode(response.body);
    final Object? error = body['error'];
    if (error is Map) {
      return ApiException(
        error['code'] is String && (error['code']! as String).isNotEmpty
            ? error['code']! as String
            : 'error',
        error['message'] is String ? error['message']! as String : '',
        status: response.statusCode,
      );
    }
    return ApiException(
      'http_${response.statusCode}',
      response.body.isEmpty ? '请求失败' : response.body,
      status: response.statusCode,
    );
  }

  void dispose() => _client.close();
}

String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  return value.isEmpty ? Uri.base.origin : value;
}

Map<String, Object?> _decode(String body) {
  try {
    final Object? decoded = jsonDecode(body);
    if (decoded is Map) return _stringKeys(decoded);
  } catch (_) {
    // 坏 JSON → 空对象（由调用方按缺字段处理）。
  }
  return const <String, Object?>{};
}

Map<String, Object?> _stringKeys(Map<Object?, Object?> raw) {
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}
