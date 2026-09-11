/// Mod 管理 API（`GET /api/v1/mods` + `POST /api/v1/mods/{id}/{action}`）。
///
/// 契约来源：`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`（读源码确认）。
///
/// # 「启用」这件事在 Mod 边界上是敏感的
///
/// Mod 是本项目**唯一**允许往里堆功能的地方（用户裁决：「堆砌就往 mod 方向
/// 仅提供接口即可」）。所以这个面板的职责是**如实呈现**每个 Mod 的状态与
/// 配置入口，而**不是**替 Mod 创造核心能力。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// `GET /api/v1/mods` 的列表项。
class ModInfo {
  const ModInfo({
    required this.id,
    required this.name,
    required this.version,
    required this.apiVersion,
    required this.enabled,
    required this.status,
  });

  final String id;
  final String name;
  final String version;
  final int apiVersion;

  /// 是否已启用。
  final bool enabled;

  /// 服务端上报的运行状态（`running` / `disabled` / 其它）。
  final String status;

  /// 是否正在运行。
  bool get isRunning => status == 'running';

  /// 状态的中文标签（**每个状态都要有文字**，不靠颜色）。
  String get statusLabel => switch (status) {
    'running' => '运行中',
    'disabled' => '已停用',
    'error' => '出错',
    'starting' => '启动中',
    _ => status.isEmpty ? '未知' : status,
  };

  factory ModInfo.fromJson(Map<String, Object?> json) => ModInfo(
    id: _str(json['id']),
    name: _str(json['name']),
    version: _str(json['version']),
    apiVersion: _int(json['api_version']),
    enabled: json['enabled'] == true,
    status: _str(json['status']),
  );
}

class ModsApi {
  ModsApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// `GET /api/v1/mods`。
  Future<List<ModInfo>> list() async {
    final http.Response response = await _guard(
      () => _client.get(_uri('/api/v1/mods')),
    );
    if (response.statusCode != 200) throw _error(response);
    final Object? raw = _decode(response.body)['mods'];
    if (raw is! List) return const <ModInfo>[];
    return raw
        .whereType<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> m) => ModInfo.fromJson(_stringKeys(m)))
        .where((ModInfo m) => m.id.isNotEmpty)
        .toList();
  }

  /// `POST /api/v1/mods/{id}/{action}`；`action` ∈ `enable` / `disable`。
  Future<void> setEnabled(String id, bool enabled) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/mods/${Uri.encodeComponent(id)}/${enabled ? 'enable' : 'disable'}'),
      ),
    );
    if (response.statusCode != 200) throw _error(response);
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
