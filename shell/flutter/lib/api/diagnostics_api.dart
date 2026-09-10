/// 诊断 API：`app/status` + `app/capabilities` + `logs`（`dev_mode` 门控）。
///
/// 契约来源：`crates/live2d-ai-desktop/src/web_api/dto.rs` 与
/// `app_routes.rs`（读源码确认）。
///
/// # 诊断面板只做两件事
///
/// 1. **如实呈现**（连接、epoch、能力快照、日志）；**不猜测**。
/// 2. **一键复制快照**——把排查需要的东西一次给全，而不是让用户来回截图
///    （规格 §13.1-9：v1 只做复制到剪贴板，不做上传）。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// `GET /api/v1/app/capabilities` 的能力快照。
class AppCapabilities {
  const AppCapabilities({
    this.app = '',
    this.version = '',
    this.schemaVersion = 0,
    this.actions = const <String>[],
    this.actionSources = const <String>[],
    this.strengthLevels = const <int>[],
    this.modelUploadSupported = false,
    this.scriptInvokeSupported = false,
    this.runtimeWs = '',
    this.stateWs = '',
    this.wsProtocolVersion = 0,
  });

  final String app;
  final String version;
  final int schemaVersion;

  /// 可用动作（snake_case）。**动作按钮由它驱动显隐/置灰，不硬编码 6 个。**
  final List<String> actions;
  final List<String> actionSources;
  final List<int> strengthLevels;
  final bool modelUploadSupported;
  final bool scriptInvokeSupported;
  final String runtimeWs;
  final String stateWs;
  final int wsProtocolVersion;

  factory AppCapabilities.fromJson(Map<String, Object?> json) => AppCapabilities(
    app: _str(json['app']),
    version: _str(json['version']),
    schemaVersion: _int(json['schema_version']),
    actions: _strList(json['actions']),
    actionSources: _strList(json['action_sources']),
    strengthLevels: json['strength_levels'] is List
        ? (json['strength_levels']! as List)
              .whereType<num>()
              .map((num n) => n.toInt())
              .toList()
        : const <int>[],
    modelUploadSupported: json['model_upload_supported'] == true,
    scriptInvokeSupported: json['script_invoke_supported'] == true,
    runtimeWs: _str(json['runtime_ws']),
    stateWs: _str(json['state_ws']),
    wsProtocolVersion: _int(json['ws_protocol_version']),
  );
}

/// 一条日志。
class LogLine {
  const LogLine({required this.level, required this.target, required this.message});

  final String level;
  final String target;
  final String message;

  /// 单行文本形式（复制快照用）。
  String get asText => '[$level] $target: $message';

  factory LogLine.fromJson(Map<String, Object?> json) => LogLine(
    level: _str(json['level']),
    target: _str(json['target']),
    message: _str(json['message']),
  );
}

class DiagnosticsApi {
  DiagnosticsApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// `GET /api/v1/app/status`。
  Future<Map<String, Object?>> status() => _getObject('/api/v1/app/status');

  /// `GET /api/v1/app/capabilities`。
  Future<AppCapabilities> capabilities() async =>
      AppCapabilities.fromJson(await _getObject('/api/v1/app/capabilities'));

  /// `GET /api/v1/logs`（**需 `dev_mode=true`**，否则 403 `dev_mode_required`）。
  Future<List<LogLine>> logs({String? level, int? limit}) async {
    final StringBuffer path = StringBuffer('/api/v1/logs');
    final List<String> query = <String>[
      if (level != null && level.isNotEmpty) 'level=$level',
      if (limit != null) 'limit=$limit',
    ];
    if (query.isNotEmpty) path.write('?${query.join('&')}');
    final http.Response response = await _guard(
      () => _client.get(_uri(path.toString())),
    );
    if (response.statusCode != 200) throw _error(response);
    final Object? raw = _decode(response.body)['lines'] ?? _decode(response.body)['logs'];
    if (raw is! List) return const <LogLine>[];
    return raw
        .whereType<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> m) => LogLine.fromJson(_stringKeys(m)))
        .toList();
  }

  Future<Map<String, Object?>> _getObject(String path) async {
    final http.Response response = await _guard(() => _client.get(_uri(path)));
    if (response.statusCode != 200) throw _error(response);
    return _decode(response.body);
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
        _str(error['code']).isEmpty ? 'error' : _str(error['code']),
        _str(error['message']),
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
    // 坏 JSON → 空对象。
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

List<String> _strList(Object? value) => value is List
    ? value.whereType<String>().toList()
    : const <String>[];

String _str(Object? value) => value is String ? value : '';
int _int(Object? value) => value is num ? value.toInt() : 0;
