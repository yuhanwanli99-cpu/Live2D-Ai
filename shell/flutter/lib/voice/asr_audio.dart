/// 录下来的音频：16 位单声道 PCM WAV（**纯数据 + 纯函数**，VM 可测）。
///
/// # 为什么自己拼 WAV 头
///
/// 浏览器的 `MediaRecorder` 不给 WAV（它只会 webm/ogg），而项目的识别端点
/// 要的是**整段** 16 位 PCM WAV。裸 PCM 样本浏览器给得到（`AudioContext`
/// 的 `Float32` 帧），缺的只是那 44 字节的 RIFF 头——自己拼比引依赖省事，
/// 也让「规格是 16 kHz / 单声道 / 16 位」这件事在代码里**可读、可测**。
library;

import 'dart:typed_data';

/// 送去识别的采样率。
///
/// 16 kHz 是语音识别的常规输入（上游只要 WAV 头自洽就能收）；单声道 + 16 位
/// 让一分钟的音频只有 1.9 MB，远小于 8 MiB 上限。
const int kAsrSampleRate = 16000;

/// 上传上限（与服务端 `asr::MAX_UPLOAD_BYTES` 逐字一致）。
///
/// 在**本机**先判：超过就不发，界面上说清「太长了」，而不是让服务端回 413
/// 之后才发现录了一小时。
const int kAsrMaxUploadBytes = 8 * 1024 * 1024;

/// 把归一化到 [-1, 1] 的样本编码成 16 位单声道 WAV（44 字节头 + 裸 PCM）。
///
/// 超出 [-1, 1] 的样本先钳位——录音链路里回声消除 / 增益偶尔会越界，
/// 直接乘 32767 会绕回成刺耳的噪声。
Uint8List encodeWav16Mono(List<double> samples, {int sampleRate = kAsrSampleRate}) {
  final int dataLen = samples.length * 2;
  final ByteData out = ByteData(44 + dataLen);

  void ascii(int offset, String text) {
    for (int i = 0; i < text.length; i++) {
      out.setUint8(offset + i, text.codeUnitAt(i));
    }
  }

  ascii(0, 'RIFF');
  out.setUint32(4, 36 + dataLen, Endian.little);
  ascii(8, 'WAVE');
  ascii(12, 'fmt ');
  out.setUint32(16, 16, Endian.little); // fmt 块长度
  out.setUint16(20, 1, Endian.little); // PCM
  out.setUint16(22, 1, Endian.little); // 单声道
  out.setUint32(24, sampleRate, Endian.little);
  out.setUint32(28, sampleRate * 2, Endian.little); // byte rate
  out.setUint16(32, 2, Endian.little); // block align
  out.setUint16(34, 16, Endian.little); // 位深
  ascii(36, 'data');
  out.setUint32(40, dataLen, Endian.little);

  for (int i = 0; i < samples.length; i++) {
    final double clamped = samples[i].clamp(-1.0, 1.0);
    // 负半轴乘 32768（-1.0 → -32768），正半轴乘 32767（+1.0 → 32767）。
    final int v = clamped < 0
        ? (clamped * 32768).round().clamp(-32768, 32767)
        : (clamped * 32767).round();
    out.setInt16(44 + i * 2, v, Endian.little);
  }
  return out.buffer.asUint8List();
}

/// 这段音频能不能直接发给服务端（**本机**判，省一次无效上行）。
///
/// 返回 `null` = 可以发；否则是一句给用户看的原因。
String? audioRejectReason(Uint8List wav) {
  if (wav.length <= 44) return '什么都没录到（再说一次）';
  if (wav.length > kAsrMaxUploadBytes) return '录音太长（超过 8 MiB），分几次说';
  return null;
}

/// 把浏览器给的一段 `Float32` 帧降采样到 [targetRate]（线性插值）。
///
/// 只在源采样率与目标不同时调用；`AudioContext` 的采样率是设备决定的
/// （常见 44.1k / 48k），不降采样会让一分钟音频大 3 倍，还可能超出上游的
/// 时长判据。
List<double> downsample(List<double> input, int sourceRate, int targetRate) {
  if (sourceRate <= targetRate || input.isEmpty) return List<double>.of(input);
  final double ratio = sourceRate / targetRate;
  final int outLen = (input.length / ratio).floor();
  final List<double> out = List<double>.filled(outLen, 0);
  for (int i = 0; i < outLen; i++) {
    final double srcPos = i * ratio;
    final int i0 = srcPos.floor();
    final int i1 = i0 + 1 < input.length ? i0 + 1 : i0;
    final double t = srcPos - i0;
    out[i] = input[i0] * (1 - t) + input[i1] * t;
  }
  return out;
}
