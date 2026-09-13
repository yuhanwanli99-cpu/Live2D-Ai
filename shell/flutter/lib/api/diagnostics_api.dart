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
///
/// 2026-09-11：`actions` / `action_sources` / `strength_levels` 三个字段随
/// 动作子系统一并删除（用户裁定 LLM 无工具、只做对话；`/api/v1/commands`
/// 动作目录端点也删了）。
///
/// 2026-09-12（rc.2）：`model_upload_supported` / `script_invoke_supported`
/// 也删了——**它们是假广告**（multipart ZIP 上传与脚本调用端点都不存在），
/// 服务端已同步删除。这里不再解析：前端**不持有**未实现能力的开关，
/// 就不可能出现「按钮亮了但点了必失败」。未知字段本来就被容忍。
class AppCapabilities {
  const AppCapabilities({
    this.app = '',
    this.version = '',
    this.schemaVersion = 0,
    this.runtimeWs = '',
    this.stateWs = '',
    this.wsProtocolVersion = 0,
  });

  final String app;
  final String version;
  final int schemaVersion;
  final String runtimeWs;
  final String stateWs;
  final int wsProtocolVersion;

  factory AppCapabilities.fromJson(Map<String, Object?> json) => AppCapabilities(
    app: _str(json['app']),
    version: _str(json['version']),
    schemaVersion: _int(json['schema_version']),
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
  ///
  /// # 为什么**没有** `level` / `limit` 参数（2026-09-11）
  ///
  /// 它们曾经被发出去，但服务端**根本不读查询参数**：
  /// `handle_logs()` 只按内置的 `MAX_LINES` 截尾，没有解析 `?limit=`。
  /// 于是这两个参数有两个问题——一是「静默无效」（发的人以为有用），
  /// 二是当时路由是精确匹配，带上 `?...` 会直接掉到 **501**，
  /// 整个诊断面板的日志区在成品里从来没拿到过数据。
  ///
  /// 501 已在服务端修掉（请求入口统一剥查询串，见 `web_api::strip_query`）。
  /// 这里把没用的参数**删掉**而不是留着：服务端既然不支持，
  /// 客户端就不该假装支持；哪天要限流行数，应该服务端实现并写进契约。
  Future<List<LogLine>> logs() async {
    final http.Response response = await _guard(
      () => _client.get(_uri('/api/v1/logs')),
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

String _str(Object? value) => value is String ? value : '';
int _int(Object? value) => value is num ? value.toInt() : 0;
