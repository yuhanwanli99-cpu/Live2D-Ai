import 'dart:convert';

import 'package:http/http.dart' as http;

/// `POST /api/v1/chat` 受理响应（字段对齐 `chat_routes.rs::ChatAccepted`）。
class ChatAccepted {
  const ChatAccepted({
    required this.accepted,
    required this.epoch,
    this.pendingCleared = false,
  });

  final bool accepted;

  /// 受理时 supervisor 的当前代次（前端用它关联 WS 上的 turn_state）。
  final int epoch;
  final bool pendingCleared;
}

/// 后端统一错误体 `{"error":{"code","message","details"}}`。
class ApiException implements Exception {
  const ApiException(this.code, this.message, {this.status});

  final String code;
  final String message;
  final int? status;

  @override
  String toString() =>
      status == null ? '$code：$message' : 'HTTP $status $code：$message';
}

/// 核心链路 HTTP 客户端。
///
/// base 缺省 = 同源（页面 origin）；可用
/// `--dart-define=API_BASE=http://127.0.0.1:18080` 覆盖。
class ApiClient {
  ApiClient({String? base})
      : base = _normalize(base ?? const String.fromEnvironment('API_BASE'));

  final String base;
  final http.Client _client = http.Client();

  static String _normalize(String raw) {
    var value = raw.trim();
    while (value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    if (value.isEmpty) {
      return Uri.base.origin; // 同源相对路径的绝对化根
    }
    return value;
  }

  Uri _uri(String path) => Uri.parse('$base$path');

  /// 发送一条文本消息。200 返回受理结果；400/429/503 抛 [ApiException]。
  Future<ChatAccepted> sendChat(String text) async {
    final response = await _guard(
      () => _client.post(
        _uri('/api/v1/chat'),
        headers: const <String, String>{
          'Content-Type': 'application/json',
        },
        body: jsonEncode(<String, String>{'text': text}),
      ),
    );
    if (response.statusCode == 200) {
      final data = _decodeObject(response.body);
      return ChatAccepted(
        accepted: data['accepted'] == true,
        epoch: (data['epoch'] as num?)?.toInt() ?? 0,
        pendingCleared: data['pending_cleared'] == true,
      );
    }
    throw _errorFrom(response);
  }

  /// `POST /api/v1/chat/stop`（幂等）。
  Future<void> stopChat() async {
    final response = await _guard(
      () => _client.post(_uri('/api/v1/chat/stop')),
    );
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw _errorFrom(response);
    }
  }

  /// `GET /api/v1/app/status`（顶栏状态展示用）。
  Future<Map<String, Object?>> fetchStatus() async {
    final response = await _guard(
      () => _client.get(_uri('/api/v1/app/status')),
    );
    if (response.statusCode != 200) throw _errorFrom(response);
    return _decodeObject(response.body);
  }

  Future<http.Response> _guard(Future<http.Response> Function() run) async {
    try {
      return await run();
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
  }

  Map<String, Object?> _decodeObject(String body) {
    try {
      final decoded = jsonDecode(body);
      if (decoded is Map) return Map<String, Object?>.from(decoded);
    } catch (_) {
      return const <String, Object?>{};
    }
    return const <String, Object?>{};
  }

  ApiException _errorFrom(http.Response response) {
    final data = _decodeObject(response.body);
    final error = data['error'];
    if (error is Map) {
      final code = error['code'];
      final message = error['message'];
      return ApiException(
        code is String ? code : 'error',
        message is String ? message : response.body,
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
