/// 模型库 API（`/api/v1/models*`）：**纯数据层**，可注入 `http.Client`。
///
/// 契约来源：`crates/live2d-ai-desktop/src/web_api/models_routes/dto.rs`（读源码确认）。
///
/// # 一个必须如实呈现的事实
///
/// 本仓库**不捆绑模型二进制**（`assets/models/` 为空），所以
/// `GET /api/v1/models` 很可能返回 `{"models":[]}` 而 `app/status` 里的
/// `active_model_id` 却有值。界面必须把「列表为空」表达成
/// **「还没有导入模型」**，而不是一个空白面板——后者会被读成「坏了」。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// `GET /api/v1/models` 的列表项。
class ModelInfo {
  const ModelInfo({
    required this.id,
    required this.displayName,
    required this.version,
    required this.active,
    required this.hasPhysics,
    required this.hasDisplayInfo,
    required this.sizeBytes,
    required this.textureCount,
    required this.importedAt,
    required this.moc3File,
  });

  final String id;
  final String displayName;
  final int version;

  /// 是否为当前激活模型。
  final bool active;

  /// 是否有物理效果（`has_physics`）——**用它代替缩略图**：
  /// 后端没有缩略图端点，用 `Image.network` 拉贴图当缩略图在 Web 上行为不同
  /// （规格 §4.1.2 明确「不要做」）。
  final bool hasPhysics;
  final bool hasDisplayInfo;
  final int sizeBytes;
  final int textureCount;
  final String importedAt;
  final String moc3File;

  /// 人类可读的体积。
  String get humanSize {
    if (sizeBytes >= 1024 * 1024) {
      return '${(sizeBytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    if (sizeBytes >= 1024) return '${(sizeBytes / 1024).toStringAsFixed(0)} KB';
    return '$sizeBytes B';
  }

  factory ModelInfo.fromJson(Map<String, Object?> json) {
    final Object? textures = json['texture_files'];
    return ModelInfo(
      id: _str(json['id']),
      displayName: _str(json['display_name']),
      version: _int(json['version']),
      active: json['active'] == true,
      hasPhysics: json['has_physics'] == true,
      hasDisplayInfo: json['has_display_info'] == true,
      sizeBytes: _int(json['size_bytes']),
      textureCount: textures is List ? textures.length : 0,
      importedAt: _str(json['imported_at']),
      moc3File: _str(json['moc3_file']),
    );
  }
}

/// `POST /api/v1/models/{id}/activate` 的响应。
class ActivateResult {
  const ActivateResult({
    required this.activeId,
    this.prevActiveId,
    required this.requiresRestart,
    required this.modelUrl,
  });

  final String activeId;
  final String? prevActiveId;

  /// **是否需重启才生效**。服务端如实上报；界面必须显示，
  /// 否则用户以为已经换好了（规格 §4.1.2「激活/删除的反馈三件套」）。
  final bool requiresRestart;
  final String modelUrl;

  factory ActivateResult.fromJson(Map<String, Object?> json) => ActivateResult(
    activeId: _str(json['active_id']),
    prevActiveId: json['prev_active_id'] is String
        ? json['prev_active_id']! as String
        : null,
    requiresRestart: json['requires_restart'] == true,
    modelUrl: _str(json['model_url']),
  );
}

class ModelsApi {
  ModelsApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// `GET /api/v1/models`。
  Future<List<ModelInfo>> list() async {
    final http.Response response = await _guard(
      () => _client.get(_uri('/api/v1/models')),
    );
    if (response.statusCode != 200) throw _error(response);
    final Map<String, Object?> body = _decode(response.body);
    final Object? raw = body['models'];
    if (raw is! List) return const <ModelInfo>[];
    return raw
        .whereType<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> m) => ModelInfo.fromJson(_stringKeys(m)))
        .where((ModelInfo m) => m.id.isNotEmpty)
        .toList();
  }

  /// `POST /api/v1/models/{id}/activate`。
  Future<ActivateResult> activate(String id) async {
    final http.Response response = await _guard(
      () => _client.post(_uri('/api/v1/models/${Uri.encodeComponent(id)}/activate')),
    );
    if (response.statusCode != 200) throw _error(response);
    return ActivateResult.fromJson(_decode(response.body));
  }

  /// `POST /api/v1/models/import`（**dev**：只接受 `assets/models/` 下的目录名）。
  Future<void> import(String id) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/models/import'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, String>{'id': id}),
      ),
    );
    if (response.statusCode != 200) throw _error(response);
  }

  /// `DELETE /api/v1/models/{id}`（**dev**；激活中的模型会 409 `model_active`）。
  Future<void> remove(String id) async {
    final http.Response response = await _guard(
      () => _client.delete(_uri('/api/v1/models/${Uri.encodeComponent(id)}')),
    );
    if (response.statusCode != 200 && response.statusCode != 204) {
      throw _error(response);
    }
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

String _str(Object? value) => value is String ? value : '';
int _int(Object? value) => value is num ? value.toInt() : 0;
