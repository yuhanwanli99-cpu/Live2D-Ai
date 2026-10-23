/// 本机 Pi 出站的最小 HTTP 客户端（0.4.5 rc.2）。
///
/// # 契约（0.4.5 执行单「出站」）
///
/// `POST /api/v1/peers/pi/message`，body `{"text":"…","session_id":"…"}`。
/// 200 → `{"text":"<Pi 那一句>"}`；错误体是后端统一的
/// `{"error":{"code":…}}`。五个码的中文文案在 `chat/peer_route.dart`
/// （transport 层只负责搬码，**不写界面文案**）。
///
/// 与 `api/playback_api.dart` 同一条口径：base 缺省 = 同源（页面 origin），
/// 可用 `--dart-define=API_BASE=…` 覆盖；`http.Client` 可注入，于是
/// `flutter test` 里用 `MockClient` 就能覆盖全部请求 / 响应契约——**零新依赖**。
library;

import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

/// 出站端点固定为 `pi`（Claude Code 这一版不接：前端连按钮都不可选，
/// 后端对它回 `peer_unavailable` 是另一道闸）。
const String kPeerPiPath = '/api/v1/peers/pi/message';

/// 出站失败的**码**。
class PeerApiException implements Exception {
  const PeerApiException(this.code, {this.status});

  /// `peer_missing` / `peer_busy` / `peer_timeout` / `peer_failed` /
  /// `peer_unavailable`。
  final String code;

  /// HTTP 状态；连接期失败（根本没有响应）时为 `null`。
  final int? status;

  @override
  String toString() =>
      'PeerApiException($code${status == null ? '' : ', status=$status'})';
}

/// `POST /api/v1/peers/pi/message` 的最小客户端（可注入，VM 可测）。
class PeerApi {
  PeerApi({String? base, http.Client? client, Duration? timeout})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client(),
      _timeout = timeout ?? const Duration(seconds: 130);

  final String _base;
  final http.Client _client;

  /// 比后端 120 秒多留 10 秒：**超时判定以后端为准**（504 `peer_timeout`
  /// 带统一错误体），客户端这一层只兜「连接彻底没有响应」。
  final Duration _timeout;

  /// 把这一条原文发给 Pi；成功返回助手正文。
  ///
  /// [sessionId] 为空串 = 这一条不记会话（与后端契约一致）。
  Future<String> send({required String text, String sessionId = ''}) async {
    http.Response response;
    try {
      response = await _client
          .post(
            Uri.parse('$_base$kPeerPiPath'),
            headers: const <String, String>{'Content-Type': 'application/json'},
            body: jsonEncode(<String, String>{
              'text': text,
              'session_id': sessionId,
            }),
          )
          .timeout(_timeout);
    } on TimeoutException {
      throw const PeerApiException('peer_timeout');
    } catch (_) {
      // 连接失败 / 客户端异常：与「Pi 没有返回一句可读的话」同码。
      throw const PeerApiException('peer_failed');
    }

    if (response.statusCode == 200) {
      final String? reply = _replyText(response.body);
      if (reply != null) return reply;
      // 200 但不是约定的 JSON、或正文为空：与服务端的 peer_failed 同义。
      throw const PeerApiException('peer_failed', status: 200);
    }
    throw PeerApiException(_errorCode(response), status: response.statusCode);
  }

  void dispose() => _client.close();

  static String? _replyText(String body) {
    try {
      final Object? data = jsonDecode(body);
      if (data is Map && data['text'] is String) {
        final String text = (data['text'] as String).trim();
        return text.isEmpty ? null : text;
      }
    } catch (_) {
      // 落到 null：坏 JSON 与「没有可读正文」同码。
    }
    return null;
  }

  /// 错误体优先取 `error.code`；体缺失 / 坏 JSON 时按 HTTP 状态兜底。
  ///
  /// 404 兜底取 `peer_missing`：`peer_unavailable`（claude）在体里能分辨，
  /// 而前端根本不会发 claude，所以兜底选更常见的那个。
  static String _errorCode(http.Response response) {
    try {
      final Object? data = jsonDecode(response.body);
      if (data is Map) {
        final Object? error = data['error'];
        if (error is Map && error['code'] is String) {
          final String code = (error['code'] as String).trim();
          if (code.isNotEmpty) return code;
        }
      }
    } catch (_) {
      // 坏 JSON：走状态码兜底。
    }
    return switch (response.statusCode) {
      409 => 'peer_busy',
      504 => 'peer_timeout',
      404 => 'peer_missing',
      _ => 'peer_failed',
    };
  }
}

/// 同源根：`API_BASE` 覆盖优先，否则用页面 origin（VM 下回落 loopback）。
///
/// `Uri.base.origin` 对非 http(s) 方案会抛（`file://` 在 `flutter test` 里
/// 很常见），构造必须永不抛——与 `api/playback_api.dart` 的同一段口径。
String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  if (value.isNotEmpty) return value;
  final Uri base = Uri.base;
  return base.scheme == 'http' || base.scheme == 'https'
      ? base.origin
      : 'http://127.0.0.1';
}
