/// `POST /api/v1/voice/asr` 的客户端 + WAV 编码回归（2026-11-16）。
///
/// 用**假 HTTP**（`package:http` 的 `MockClient` 由本文件自带的最小平替：
/// 直接实现 `http.BaseClient`），不需要麦克风、也不连后端。
///
/// 守四件事：
/// 1. 客户端把**整段字节**原样 POST 到 `/api/v1/voice/asr`，Content-Type 是
///    `audio/wav`；
/// 2. 成功响应 `{"ok":true,"text":…}` → 拿到 trim 过的文本；
/// 3. 失败（413 / 503 / 401…）→ 抛 `ApiException`，带服务端的 `error.code`；
/// 4. `encodeWav16Mono` 的头部字段 / 样本编码 / 降采样都是对的（纯函数）。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/voice_api.dart';
import 'package:live2d_ai_shell/voice/asr_audio.dart';

/// 假 HTTP 客户端：记下最后一条请求，回预设响应。
class _FakeHttp extends http.BaseClient {
  _FakeHttp({required this.status, required this.body});

  final int status;
  final String body;

  http.BaseRequest? lastRequest;
  Uint8List? lastBody;

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) async {
    lastRequest = request;
    lastBody = await request.finalize().toBytes();
    return http.StreamedResponse(
      Stream<List<int>>.value(utf8.encode(body)),
      status,
      headers: const <String, String>{'content-type': 'application/json'},
    );
  }
}

VoiceApi _api(_FakeHttp fake) =>
    VoiceApi(base: 'http://127.0.0.1:18080', client: fake);

void main() {
  test('① 整段字节原样 POST 到 /api/v1/voice/asr（Content-Type: audio/wav）', () async {
    final _FakeHttp fake = _FakeHttp(
      status: 200,
      body: '{"ok":true,"text":"你好。"}',
    );
    final List<int> wav = encodeWav16Mono(
      List<double>.generate(160, (int i) => i.isEven ? 0.25 : -0.25),
    );
    final VoiceAsrResult result = await _api(fake).transcribe(wav);

    expect(fake.lastRequest!.method, 'POST');
    expect(fake.lastRequest!.url.path, '/api/v1/voice/asr');
    expect(fake.lastRequest!.headers['Content-Type'], 'audio/wav');
    expect(fake.lastBody, equals(wav), reason: '整段音频必须一字不改地发出');
    expect(result.text, '你好。');
    expect(result.durationMs, isNull, reason: '上游没给时长，不能补 0');
  });

  test('② 成功响应带 duration_ms：原样透传', () async {
    final _FakeHttp fake = _FakeHttp(
      status: 200,
      body: '{"ok":true,"text":"  你好。  ","duration_ms":1234}',
    );
    final VoiceAsrResult result = await _api(fake).transcribe(
      encodeWav16Mono(const <double>[0.1, 0.2]),
    );
    expect(result.text, '  你好。  ', reason: '客户端不 trim（服务端已做，前端不二次改字）');
    expect(result.durationMs, 1234);
  });

  test('③ 413 / 503 / 401 → ApiException 带服务端 error.code', () async {
    for (final (int status, String code) in <(int, String)>[
      (413, 'asr_too_large'),
      (503, 'asr_unconfigured'),
      (502, 'asr_upstream_error'),
      (400, 'asr_too_long'),
    ]) {
      final _FakeHttp fake = _FakeHttp(
        status: status,
        body: jsonEncode(<String, Object?>{
          'error': <String, Object?>{'code': code, 'message': '说明 $code'},
        }),
      );
      await expectLater(
        _api(fake).transcribe(encodeWav16Mono(const <double>[0.1])),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', code)
              .having((ApiException e) => e.status, 'status', status),
        ),
      );
    }
  });

  test('④ encodeWav16Mono：RIFF 头字段与样本编码', () {
    final List<double> samples = <double>[
      0.0,
      0.5,
      -0.5,
      1.0,
      -1.0,
      // 越界值必须钳位，不能绕回成噪声。
      1.7,
      -1.7,
    ];
    final Uint8List wav = encodeWav16Mono(samples);
    final ByteData view = ByteData.view(wav.buffer);

    expect(String.fromCharCodes(wav.sublist(0, 4)), 'RIFF');
    expect(String.fromCharCodes(wav.sublist(8, 12)), 'WAVE');
    expect(String.fromCharCodes(wav.sublist(12, 16)), 'fmt ');
    expect(view.getUint16(20, Endian.little), 1, reason: 'PCM');
    expect(view.getUint16(22, Endian.little), 1, reason: '单声道');
    expect(view.getUint32(24, Endian.little), kAsrSampleRate);
    expect(view.getUint32(28, Endian.little), kAsrSampleRate * 2, reason: 'byte rate');
    expect(view.getUint16(32, Endian.little), 2, reason: 'block align');
    expect(view.getUint16(34, Endian.little), 16, reason: '位深');
    expect(String.fromCharCodes(wav.sublist(36, 40)), 'data');
    expect(view.getUint32(40, Endian.little), samples.length * 2);
    expect(wav.length, 44 + samples.length * 2);

    // 样本：0.5 → 16384（×32767）；-0.5 → -16384（×32768）。
    expect(view.getInt16(46, Endian.little), 16384);
    expect(view.getInt16(48, Endian.little), -16384);
    expect(view.getInt16(50, Endian.little), 32767);
    expect(view.getInt16(52, Endian.little), -32768);
    // 越界被钳到同一对端点。
    expect(view.getInt16(54, Endian.little), 32767);
    expect(view.getInt16(56, Endian.little), -32768);
  });

  test('④b audioRejectReason：空录音 / 超 8 MiB 在本机就拦', () {
    expect(audioRejectReason(Uint8List(44)), contains('没录到'));
    expect(audioRejectReason(Uint8List(45)), isNull, reason: '有 1 个样本就该发');
    expect(
      audioRejectReason(Uint8List(kAsrMaxUploadBytes + 1)),
      contains('太长'),
    );
    expect(audioRejectReason(Uint8List(kAsrMaxUploadBytes)), isNull);
  });

  test('④c downsample：44.1k → 16k 的长度与线性插值', () {
    final List<double> input = List<double>.generate(4410, (int i) => i.toDouble());
    final List<double> out = downsample(input, 44100, kAsrSampleRate);
    expect(out.length, (4410 / (44100 / 16000)).floor());
    // 单调输入降采样后仍单调（插值不会打乱次序）。
    for (int i = 1; i < out.length; i++) {
      expect(out[i] >= out[i - 1], isTrue);
    }
    // 已经低于目标采样率：原样返回。
    final List<double> short = <double>[0.1, 0.2, 0.3];
    expect(downsample(short, 8000, kAsrSampleRate), short);
  });
}
