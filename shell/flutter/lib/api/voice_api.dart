/// 语音**识别**端点（`POST /api/v1/voice/asr`）的极小客户端。
///
/// 契约真源：`crates/live2d-ai-desktop/src/web_api/voice_asr.rs`。
///
/// # 与旧 `/api/v1/voice/transcript` 的区别（2026-11-16）
///
/// 那条路已经**退休**（`410 voice_retired`）。旧接口 POST 的是**一段文本**
/// （浏览器认出来的字），由服务端清洗 + `say` 开一轮；新接口 POST 的是
/// **整段音频字节**，拿回文本，**不带任何对话副作用**。
///
/// 所以这里的返回类型只有「一段文本」或「一个异常」——没有 `ok:false` 的
/// 「主链忙」：识别与主链是否忙无关。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// 识别结果。
class VoiceAsrResult {
  const VoiceAsrResult({required this.text, this.durationMs});

  /// 上游文本**去掉首尾空白**（中间一个字不改）。
  final String text;

  /// 上游报的时长；没给就是 `null`（**不是 0**——那是两件事）。
  final int? durationMs;
}

class VoiceApi {
  VoiceApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// 送一整段 WAV（或 Ogg）去识别。非 200 → [ApiException]（带服务端错误码）。
  Future<VoiceAsrResult> transcribe(
    List<int> audio, {
    String contentType = 'audio/wav',
  }) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/voice/asr'),
        headers: <String, String>{'Content-Type': contentType},
        body: audio is Uint8List ? audio : Uint8List.fromList(audio),
      ),
    );
    final Map<String, Object?> body = _decode(response.body);
    if (response.statusCode != 200) {
      final Map<String, Object?> error = body['error'] is Map
          ? _stringKeys(body['error']! as Map<Object?, Object?>)
          : const <String, Object?>{};
      final String code = _str(error['code']);
      throw ApiException(
        code.isEmpty ? 'http_${response.statusCode}' : code,
        _str(error['message']),
        status: response.statusCode,
      );
    }
    final Object? duration = body['duration_ms'];
    return VoiceAsrResult(
      text: _str(body['text']),
      durationMs: duration is int ? duration : null,
    );
  }

  void dispose() => _client.close();

  Future<http.Response> _guard(Future<http.Response> Function() run) async {
    try {
      return await run();
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
  }
}

String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  if (value.isEmpty) {
    final Uri base = Uri.base;
    if (base.scheme == 'http' || base.scheme == 'https') return base.origin;
    return 'http://127.0.0.1';
  }
  return value;
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
