import 'dart:convert';

import 'package:http/http.dart' as http;

import 'settings_models.dart';

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
  ApiClient({String? base, http.Client? client})
    : base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String base;

  /// HTTP 客户端。**可注入**：`flutter test` 里用 `package:http/testing.dart`
  /// 的 `MockClient` 就能在 VM 上覆盖全部请求/响应契约（零新依赖，
  /// `http` 本来就是运行时依赖）。
  final http.Client _client;

  static String _normalize(String raw) {
    var value = raw.trim();
    while (value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    if (value.isEmpty) {
      // 同源相对路径的绝对化根。
      //
      // `Uri.base.origin` 对**非 http(s) 方案会抛**（`file://` 尤其常见）。
      // 生产（Web）里 `Uri.base` 一定是 http(s)，所以这不是线上缺陷；
      // 但它在 `flutter test` 里会让 `ApiClient()` 直接构造失败——
      // 于是「用真控制器当桩」这种很自然的测试写法会莫名其妙地崩。
      // 回落一个 loopback 地址，构造永不抛；真要用的时候 base 都会被显式给出。
      final Uri base = Uri.base;
      if (base.scheme == 'http' || base.scheme == 'https') return base.origin;
      return 'http://127.0.0.1';
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

  /// `GET /api/v1/settings` → 服务端权威设置。
  ///
  /// **密钥永不下发**：只回 `has_api_key` 布尔（治理红线，见 AGENTS.md）。
  Future<SettingsView> fetchSettings() async {
    final response = await _guard(
      () => _client.get(_uri('/api/v1/settings')),
    );
    if (response.statusCode != 200) throw _errorFrom(response);
    return SettingsView.fromJson(_decodeObject(response.body));
  }

  /// `PATCH /api/v1/settings` → 部分更新。
  ///
  /// **是 PATCH 不是 PUT**：服务端 `match_route` 只认 `Method::Patch`，
  /// 用 `PUT` 会 **404**（实测 `PUT→404` / `PATCH→200`）。
  ///
  /// 请求体只含 [patch] 里**真的给出**的字段——三态由 [SettingsPatch.toJson]
  /// 负责。**绝不把整个表单原样发过去**：那样未触碰的字段会一并被清空。
  Future<SettingsPatchResult> patchSettings(SettingsPatch patch) async {
    final response = await _guard(
      () => _client.patch(
        _uri('/api/v1/settings'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(patch.toJson()),
      ),
    );
    if (response.statusCode != 200) throw _errorFrom(response);
    return SettingsPatchResult.fromJson(_decodeObject(response.body));
  }

  /// `POST /api/v1/settings/test/llm`：连通性自检（回显模型名与耗时）。
  Future<SettingsTestOutcome> testLlm({int? timeoutMs}) =>
      _testEndpoint('llm', timeoutMs);

  /// `POST /api/v1/settings/test/tts`：连通性自检（回显音色与耗时）。
  Future<SettingsTestOutcome> testTts({int? timeoutMs}) =>
      _testEndpoint('tts', timeoutMs);

  /// 两个自检端点的共同实现。
  ///
  /// 注意：**自检失败通常是 200 + `ok:false` + `error`**（服务端把
  /// 「连不上你的端点」当作一次成功的自检结果），所以 `ok` 为假时**不抛异常**——
  /// 由调用方读 [SettingsTestOutcome.ok] 与错误详情展示。只有 HTTP 层失败才抛。
  Future<SettingsTestOutcome> _testEndpoint(String kind, int? timeoutMs) async {
    final response = await _guard(
      () => _client.post(
        _uri('/api/v1/settings/test/$kind'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, Object?>{'timeout_ms': ?timeoutMs}),
      ),
    );
    if (response.statusCode != 200) throw _errorFrom(response);
    return SettingsTestOutcome.fromJson(_decodeObject(response.body));
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
