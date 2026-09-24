/// 语音转写注入端点（`POST /api/v1/voice/transcript`）的**极小客户端**。
///
/// 契约真源：`crates/live2d-ai-desktop/src/web_api/voice_routes.rs` +
/// `docs/voice-input.md`。这里只做一件事：把一段转写文本交给主链。
///
/// # 为什么 POST 的是**含唤醒词的原文**
///
/// 端点侧的总闸要求文本里**包含**唤醒词（命中后才剥词）——真源是
/// `live2d_ai_mod_voice_input::gate::evaluate`（纯函数，四态）。前端本地剥词
/// 只用来判断「这句话是不是在叫角色、后面有没有正文」，**不**取代服务端判定：
/// 所以这里送原文，服务端回 `text` = 剥词 + 归一化后的正文（响应里可核对）。
///
/// 这是刻意的「一份真相」：唤醒词匹配规则（忽略空白 / 大小写）只有 Rust 一份。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// 转写注入结果。
class VoiceTranscriptResult {
  const VoiceTranscriptResult({
    required this.ok,
    this.text = '',
    this.code,
    this.message,
  });

  /// 服务端是否受理（`200 ok:true`）。
  final bool ok;

  /// 服务端剥掉唤醒词 / 归一化后的正文（含唤醒词的原文不回显）。
  final String text;

  /// 未受理时的错误码（如 `busy`）——**不猜**，原样透传。
  final String? code;
  final String? message;
}

class VoiceApi {
  VoiceApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// 注入一段转写。非 200 → [ApiException]（带服务端的错误码）。
  ///
  /// `200` 且 `ok:false`（主链忙）**不抛异常**：那是可退避的正常结果，
  /// 面板/按钮旁要如实显示它，而不是当成链路故障。
  /// `ptt`：按住说话（P0-4）——服务端闸门对该请求**跳过唤醒匹配**；
  /// 其余（手动闸 / 总闸 / 清洗 / 归一化 / 长度 / token）完全不变。
  Future<VoiceTranscriptResult> sendTranscript(
    String text, {
    bool ptt = false,
  }) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/voice/transcript'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, Object?>{'text': text, 'ptt': ptt}),
      ),
    );
    final Map<String, Object?> body = _decode(response.body);
    final Map<String, Object?> error = body['error'] is Map
        ? _stringKeys(body['error']! as Map<Object?, Object?>)
        : const <String, Object?>{};
    if (response.statusCode != 200) {
      final String code = _str(error['code']);
      throw ApiException(
        code.isEmpty ? 'http_${response.statusCode}' : code,
        _str(error['message']),
        status: response.statusCode,
      );
    }
    if (body['ok'] == false) {
      return VoiceTranscriptResult(
        ok: false,
        code: _str(error['code']),
        message: _str(error['message']),
      );
    }
    return VoiceTranscriptResult(ok: true, text: _str(body['text']));
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
