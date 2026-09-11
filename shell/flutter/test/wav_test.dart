import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/audio/wav.dart';

/// WAV 封装 + 播放进度取电平的回归（纯逻辑，VM 直接跑）。
///
/// 来历（2026-09-11）：用户裁定音频路径为「后端出 WAV、前端 `<audio>` 播」，
/// 起因是「浏览器禁用声音却依旧有声音传出」。实测确认后端无发声能力
/// （`audio.backend = "none"`）且本机 TTS 出不了 mp3（`mp3`/`wav` 均 HTTP 400，
/// 只有 `pcm` 200）。所以 WAV 由我们自造——那 44 字节的头部必须逐字段对，
/// 错一个字节浏览器就整段不播，而且**不报错**（静默无声），所以钉在这里。
void main() {
  group('wavFromPcm16：44 字节头逐字段', () {
    // 4 个样本（s16le）= 8 字节载荷，便于手算。
    final Uint8List pcm = Uint8List.fromList(<int>[
      0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0xF0, 0xFF,
    ]);
    final Uint8List wav = wavFromPcm16(pcm, sampleRate: 24000, channels: 1);
    final ByteData view = ByteData.view(wav.buffer);

    String ascii(int offset, int len) =>
        String.fromCharCodes(wav.sublist(offset, offset + len));

    test('总长度 = 44 + 载荷', () {
      expect(wav.length, 44 + pcm.length);
    });

    test('RIFF / WAVE / fmt / data 四个标记', () {
      expect(ascii(0, 4), 'RIFF');
      expect(ascii(8, 4), 'WAVE');
      expect(ascii(12, 4), 'fmt ');
      expect(ascii(36, 4), 'data');
    });

    test('RIFF 长度字段 = 文件长度 - 8（写错浏览器整段不播）', () {
      expect(view.getUint32(4, Endian.little), wav.length - 8);
    });

    test('fmt 子块：PCM(1) / 单声道 / 24 kHz / 16 bit', () {
      expect(view.getUint32(16, Endian.little), 16, reason: 'PCM fmt 块长度必须是 16');
      expect(view.getUint16(20, Endian.little), 1, reason: 'audioFormat=1 才是 PCM');
      expect(view.getUint16(22, Endian.little), 1);
      expect(view.getUint32(24, Endian.little), 24000);
      expect(view.getUint16(34, Endian.little), 16);
    });

    test('byteRate / blockAlign 与采样率一致（浏览器据此算时长）', () {
      expect(view.getUint32(28, Endian.little), 24000 * 1 * 2);
      expect(view.getUint16(32, Endian.little), 1 * 2);
    });

    test('data 长度 = 载荷长度', () {
      expect(view.getUint32(40, Endian.little), pcm.length);
    });

    test('载荷与原始 PCM **逐字节相同**（不做格式转换）', () {
      expect(wav.sublist(44), equals(pcm));
    });

    test('空载荷也是合法 WAV（一句没声也要能播，不是崩）', () {
      final Uint8List empty = wavFromPcm16(Uint8List(0), sampleRate: 24000);
      expect(empty.length, 44);
      expect(ByteData.view(empty.buffer).getUint32(40, Endian.little), 0);
      expect(String.fromCharCodes(empty.sublist(0, 4)), 'RIFF');
    });

    test('48 kHz / 立体声：byteRate 与 blockAlign 跟着变', () {
      final Uint8List w = wavFromPcm16(pcm, sampleRate: 48000, channels: 2);
      final ByteData v = ByteData.view(w.buffer);
      expect(v.getUint16(22, Endian.little), 2);
      expect(v.getUint32(28, Endian.little), 48000 * 2 * 2);
      expect(v.getUint16(32, Endian.little), 4);
    });
  });

  group('levelAtSec：按播放进度取口型电平', () {
    const List<LevelPoint> env = <LevelPoint>[
      (offsetSec: 0.0, level: 0.2),
      (offsetSec: 0.1, level: 0.9),
      (offsetSec: 0.2, level: 0.4),
    ];

    test('进度落在两点之间 → 保持前一点（不做插值）', () {
      expect(levelAtSec(env, 0.15), 0.9);
      expect(levelAtSec(env, 0.05), 0.2);
    });

    test('恰好落在点上 → 取该点', () {
      expect(levelAtSec(env, 0.1), 0.9);
      expect(levelAtSec(env, 0.2), 0.4);
    });

    test('进度未推进（currentTime=0）→ 第一个点', () {
      expect(levelAtSec(env, 0.0), 0.2);
    });

    test('负进度（时钟回拨）→ 第一个点，不越界', () {
      expect(levelAtSec(env, -1.0), 0.2);
    });

    test('进度超出末尾 → 保持最后一点（**不是突然归零**）', () {
      // 归零会让嘴唇在收尾那一帧"啪"地闭上，观感比多张 20ms 更差。
      expect(levelAtSec(env, 99.0), 0.4);
    });

    test('空包络 → 0（不抛）', () {
      expect(levelAtSec(const <LevelPoint>[], 1.0), 0);
    });

    test('单点包络：任何进度都取它', () {
      const List<LevelPoint> one = <LevelPoint>[(offsetSec: 0.3, level: 0.7)];
      expect(levelAtSec(one, 0.0), 0.7);
      expect(levelAtSec(one, 5.0), 0.7);
    });
  });
}
