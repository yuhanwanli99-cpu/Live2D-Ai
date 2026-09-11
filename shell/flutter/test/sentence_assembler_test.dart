import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/audio/sentence_assembler.dart';

/// 24 kHz 下单片的样本数（20 ms）。
const int kSliceSamples = 480;

/// 造一片全 [value] 的 s16le PCM。
Uint8List pcmOf(int samples, {int value = 0}) {
  final Uint8List out = Uint8List(samples * 2);
  final ByteData view = ByteData.view(out.buffer);
  for (int i = 0; i < samples; i++) {
    view.setInt16(i * 2, value, Endian.little);
  }
  return out;
}

/// 造一片正弦 s16le PCM（用来验证「缺 volume 时本地 RMS 回退」）。
Uint8List sineOf(int samples, {double freq = 440, int rate = 24000}) {
  final Uint8List out = Uint8List(samples * 2);
  final ByteData view = ByteData.view(out.buffer);
  for (int i = 0; i < samples; i++) {
    final value = (16000 * math.sin(2 * math.pi * freq * i / rate)).round();
    view.setInt16(i * 2, value, Endian.little);
  }
  return out;
}

/// 造一帧（`samples` 是样本数；给了 [pcm] 就用它）。
PcmFrame frame({
  int samples = kSliceSamples,
  int sampleRate = 24000,
  double? volume,
  bool start = false,
  bool end = false,
  int? seq,
  Uint8List? wav,
  Uint8List? pcm,
}) {
  return PcmFrame(
    pcm: pcm ?? pcmOf(samples),
    sampleRate: sampleRate,
    volume: volume,
    start: start,
    end: end,
    sentenceSeq: seq,
    wav: wav,
  );
}

String magic(Uint8List bytes) =>
    String.fromCharCodes(bytes.sublist(0, 4)) +
    String.fromCharCodes(bytes.sublist(8, 12));

void main() {
  // ───────────────────────────────────────────────────────────────────────
  // 句界：`end` 是唯一闸门（2026-09-11 协议裁定；见 sentence_assembler.dart）
  //
  // 为什么这些用例必须存在：旧服务端的 `start` 是**轮级**的（实测正文只有
  // 一句时 0 个 start=true）、`end` 是**每次广播调用的末片**（同一句实测
  // 13 个 end=true）。攒句规则的每一条分支都对应一种真实见过的坏输入。
  // ───────────────────────────────────────────────────────────────────────
  group('句界：end 是唯一闸门', () {
    test('一句 = 多片，只有带 end 的那片产出，且是一整个 WAV 容器', () {
      final SentenceAssembler a = SentenceAssembler();
      // 三片，共 1440 样本 = 0.06 s @24k。
      expect(a.push(frame(start: true, volume: 0.2)), isNull);
      expect(a.push(frame(volume: 0.4)), isNull);
      final AssembledSentence? s = a.push(frame(end: true, volume: 0.6));

      expect(s, isNotNull);
      expect(magic(s!.bytes), 'RIFFWAVE');
      // 44 字节头 + 三片裸 PCM，逐字节原样搬进去（不做格式转换）。
      expect(s.bytes.length, 44 + 3 * kSliceSamples * 2);
      expect(s.sampleRate, 24000);
      expect(s.durationSec, closeTo(0.06, 1e-9));
      expect(a.isAssembling, isFalse, reason: '封口后必须能开始下一句');
      expect(a.pendingBytes, 0);
    });

    test('不依赖 start：没有 start=true 也能攒成一句（旧服务端语义）', () {
      final SentenceAssembler a = SentenceAssembler();
      expect(a.push(frame(volume: 0.1)), isNull);
      expect(a.push(frame(volume: 0.2)), isNull);
      expect(a.push(frame(end: true, volume: 0.3)), isNotNull);
    });

    test('多句：第二句在第一句封口之后才开始攒', () {
      final SentenceAssembler a = SentenceAssembler();
      expect(a.push(frame(start: true, seq: 1, volume: 0.5)), isNull);
      final AssembledSentence? first =
          a.push(frame(end: true, seq: 1, volume: 0.5));
      expect(first, isNotNull);
      expect(first!.bytes.length, 44 + 2 * kSliceSamples * 2);

      // 第二句：累积中不产出，带 end 才产出。
      expect(a.push(frame(start: true, seq: 2, volume: 0.1)), isNull);
      expect(a.push(frame(seq: 2, volume: 0.1)), isNull);
      final AssembledSentence? second =
          a.push(frame(end: true, seq: 2, volume: 0.1));
      expect(second, isNotNull);
      expect(second!.bytes.length, 44 + 3 * kSliceSamples * 2);
    });

    test('累积中途出现 start = 上一句缺了 end：丢弃半截重来（绝不拼句）', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(start: true, samples: 100));
      a.push(frame(samples: 200)); // 上一句没有 end
      // 新一句的开头：100 + 200 样本必须被丢掉。
      expect(a.push(frame(start: true, samples: 300)), isNull);
      final AssembledSentence? s = a.push(frame(end: true, samples: 400));
      expect(s, isNotNull);
      expect(
        s!.bytes.length,
        44 + 300 * 2 + 400 * 2,
        reason: '只有 start 之后的 300 + 400 样本该进这一句',
      );
    });

    test('sentence_seq 变化 = 新一句：同样丢弃半截', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(seq: 1, samples: 100));
      expect(a.sentenceSeq, 1, reason: '当前累积句的 seq 要可观测（诊断用）');
      a.push(frame(seq: 2, samples: 300)); // seq 变了
      final AssembledSentence? s = a.push(frame(seq: 2, end: true, samples: 400));
      expect(s, isNotNull);
      expect(s!.bytes.length, 44 + 300 * 2 + 400 * 2);
    });

    test('sentence_seq 缺失时不参与判断（照常攒）', () {
      final SentenceAssembler a = SentenceAssembler();
      expect(a.push(frame(samples: 100)), isNull);
      expect(a.push(frame(samples: 100, seq: 1)), isNull);
      expect(a.push(frame(end: true, samples: 100)), isNotNull);
    });

    test('缺 end：不产出、一直攒着；flush() 收尾产出一次，再 flush 为 null', () {
      final SentenceAssembler a = SentenceAssembler();
      expect(a.push(frame(start: true, samples: 100)), isNull);
      expect(a.push(frame(samples: 200)), isNull);
      expect(a.isAssembling, isTrue, reason: '缺 end 时不许猜、不许自动封口');
      expect(a.pendingBytes, 300 * 2);

      final AssembledSentence? s = a.flush();
      expect(s, isNotNull);
      expect(s!.bytes.length, 44 + 300 * 2);
      expect(a.isAssembling, isFalse);
      expect(a.flush(), isNull, reason: '没有半截可收尾时 flush 是空操作');
    });

    test('空句（end 但没有任何 PCM）不产空 WAV', () {
      final SentenceAssembler a = SentenceAssembler();
      expect(a.push(frame(start: true, pcm: Uint8List(0))), isNull);
      expect(a.push(frame(end: true, pcm: Uint8List(0))), isNull);
      expect(a.isAssembling, isFalse);
      expect(a.pendingBytes, 0);
    });

    test('clear() 丢弃半截，之后是新的一句', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(start: true, samples: 500));
      a.clear();
      expect(a.isAssembling, isFalse);
      expect(a.pendingBytes, 0);
      expect(a.envelopePoints, 0);

      final AssembledSentence? s = a.push(frame(end: true, samples: 100));
      expect(s!.bytes.length, 44 + 100 * 2, reason: '旧半截不该混进来');
    });

    test('采样率非法时回落到 24000，句内以首片为准', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(sampleRate: 0, samples: 240));
      final AssembledSentence? s =
          a.push(frame(sampleRate: 0, end: true, samples: 240));
      expect(s!.sampleRate, kFallbackSampleRate);
      expect(s.durationSec, closeTo(480 / 24000, 1e-9));
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // 包络：媒体元素没有逐片播放时刻，口型靠「句内偏移 + currentTime 查表」
  // ───────────────────────────────────────────────────────────────────────
  group('包络：偏移与电平', () {
    test('每片的偏移 = 它在句内的起点（20 ms 粒度）', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(start: true, volume: 0.1));
      a.push(frame(volume: 0.5));
      final AssembledSentence? s = a.push(frame(end: true, volume: 0.9));

      expect(s!.envelope.length, 3);
      expect(s.envelope[0].offsetSec, closeTo(0.0, 1e-9));
      expect(s.envelope[1].offsetSec, closeTo(0.02, 1e-9));
      expect(s.envelope[2].offsetSec, closeTo(0.04, 1e-9));
      expect(
        s.envelope.map((LevelPoint p) => p.level).toList(),
        <double>[0.1, 0.5, 0.9],
      );
    });

    test('levelAt 是保持（hold）语义，不是线性插值', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(start: true, volume: 0.1));
      a.push(frame(volume: 0.5));
      final AssembledSentence? s = a.push(frame(end: true, volume: 0.9));

      expect(s!.levelAt(0.0), 0.1);
      expect(s.levelAt(0.019), 0.1, reason: '点之间保持前一点，不插值');
      expect(s.levelAt(0.02), 0.5);
      expect(s.levelAt(0.059), 0.9);
      // 越界：进度还没推进 → 首点；超出最后一点 → 末点（不是突然归零）。
      expect(s.levelAt(-1.0), 0.1);
      expect(s.levelAt(99.0), 0.9);
    });

    test('服务端 volume 优先（静音时 PCM 全零，本地 RMS 恒 0）', () {
      final SentenceAssembler a = SentenceAssembler();
      final Uint8List silent = pcmOf(kSliceSamples);
      a.push(frame(pcm: silent, volume: 0.7));
      final AssembledSentence? s =
          a.push(frame(pcm: silent, volume: 0.7, end: true));
      expect(
        s!.envelope.first.level,
        0.7,
        reason: 'PCM 被服务端置零时，volume 是口型的唯一来源',
      );
    });

    test('volume 缺失时回退本地 RMS（有声 → 正电平；持续静音 → 衰减到 0）', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(pcm: sineOf(kSliceSamples)));
      // 19 片静音 + 1 片带 end 的静音 = 400 ms，远长于 release 常数 0.10 s。
      for (int i = 0; i < 19; i++) {
        a.push(frame(pcm: pcmOf(kSliceSamples)));
      }
      final AssembledSentence? s =
          a.push(frame(pcm: pcmOf(kSliceSamples), end: true));
      final List<double> levels =
          s!.envelope.map((LevelPoint p) => p.level).toList();
      expect(levels.length, 21);
      expect(levels.first, greaterThan(0.05), reason: '有声音就该有电平');
      expect(levels.last, lessThan(0.01), reason: '持续静音必须衰减掉（闭嘴）');
      for (final double level in levels) {
        expect(level, inInclusiveRange(0.0, 1.0));
      }
    });

    test('非有限 volume 不灌进口型（回退本地 RMS）', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(pcm: sineOf(kSliceSamples), volume: double.nan));
      final AssembledSentence? s =
          a.push(frame(pcm: pcmOf(kSliceSamples), end: true, volume: 0));
      for (final LevelPoint p in s!.envelope) {
        expect(p.level.isFinite, isTrue);
        expect(p.level, inInclusiveRange(0.0, 1.0));
      }
    });

    test('volume 越界被夹到 [0,1]', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(volume: -3.0));
      final AssembledSentence? s = a.push(frame(volume: 42.0, end: true));
      expect(s!.envelope[0].level, 0.0);
      expect(s.envelope[1].level, 1.0);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // 前向兼容钩子：帧上直接带整句 WAV（当前服务端不发）
  // ───────────────────────────────────────────────────────────────────────
  group('整句 WAV 直通（前向兼容）', () {
    test('wav 非空 → 直接产出该字节，并丢弃正在攒的半截', () {
      final SentenceAssembler a = SentenceAssembler();
      a.push(frame(start: true, samples: 100)); // 半截
      final Uint8List whole = Uint8List.fromList(
        <int>[82, 73, 70, 70, 0, 0, 0, 0, 87, 65, 86, 69, 1, 2, 3, 4],
      );
      final AssembledSentence? s = a.push(
        frame(wav: whole, samples: 0, sampleRate: 16000, volume: 0.4),
      );
      expect(s, isNotNull);
      expect(s!.bytes, same(whole), reason: '原样直通，不再包一层 WAV 头');
      expect(s.sampleRate, 16000);
      expect(a.isAssembling, isFalse);
      expect(a.pendingBytes, 0, reason: '半截必须被丢弃');
    });

    test('wav 为空字节时按普通分片处理', () {
      final SentenceAssembler a = SentenceAssembler();
      final AssembledSentence? s = a.push(
        frame(wav: Uint8List(0), samples: 100, end: true),
      );
      expect(s!.bytes.length, 44 + 100 * 2);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // 串行队列：一句一单元（不许并发、不许重排）
  //
  // 契约原文：「宁可晚开口，不可中途卡顿或重复」——把顺序决策放进纯逻辑，
  // 就是为了让这条契约在 VM 上可回归（audio_player.dart 加载不了）。
  // ───────────────────────────────────────────────────────────────────────
  group('SerialQueue：一句一单元', () {
    test('严格串行：上一句没结束就不放行下一句', () {
      final SerialQueue<String> q = SerialQueue<String>();
      q.add('一');
      q.add('二');
      q.add('三');
      expect(q.waiting, 3);
      expect(q.takeNext(), '一');
      expect(q.takeNext(), isNull, reason: '还有一句在播，不许并发');
      expect(q.finishCurrent(), isTrue);
      expect(q.takeNext(), '二');
      expect(q.takeNext(), isNull);
      expect(q.finishCurrent(), isTrue);
      expect(q.takeNext(), '三');
      expect(q.finishCurrent(), isFalse, reason: '队尾之后没有下一句了');
      expect(q.isEmpty, isTrue);
    });

    test('顺序 = 入队顺序（不重排、不合并）', () {
      final SerialQueue<int> q = SerialQueue<int>();
      for (final int i in <int>[3, 1, 2]) {
        q.add(i);
      }
      final List<int> played = <int>[];
      while (true) {
        final int? next = q.takeNext();
        if (next == null) break;
        played.add(next);
        q.finishCurrent();
      }
      expect(played, <int>[3, 1, 2]);
    });

    test('takeNext 在空队列返回 null（不会凭空造一项）', () {
      final SerialQueue<String> q = SerialQueue<String>();
      expect(q.takeNext(), isNull);
      expect(q.isEmpty, isTrue);
    });

    test('all 含正在播的那一项，且排在最前（批量改音量要用）', () {
      final SerialQueue<String> q = SerialQueue<String>();
      q.add('一');
      q.add('二');
      expect(q.all, <String>['一', '二']);
      q.takeNext();
      expect(q.all, <String>['一', '二']);
    });

    test('clear 清空在播与排队（interrupt 语义）', () {
      final SerialQueue<String> q = SerialQueue<String>();
      q.add('一');
      q.takeNext();
      q.add('二');
      q.clear();
      expect(q.isEmpty, isTrue);
      expect(q.current, isNull);
      expect(q.takeNext(), isNull);
    });
  });

  // ───────────────────────────────────────────────────────────────────────
  // 端到端（纯逻辑侧）：两句话依次进队列 → 有序、不重叠
  // ───────────────────────────────────────────────────────────────────────
  group('攒句 → 队列 的顺序契约', () {
    test('两句依次封口、依次放行，第二句要等第一句结束', () {
      final SentenceAssembler a = SentenceAssembler();
      final SerialQueue<AssembledSentence> q =
          SerialQueue<AssembledSentence>();

      a.push(frame(start: true, seq: 1, volume: 0.5));
      final AssembledSentence? one = a.push(frame(end: true, seq: 1, volume: 0.5));
      q.add(one!);
      a.push(frame(start: true, seq: 2, volume: 0.2));
      final AssembledSentence? two =
          a.push(frame(end: true, seq: 2, samples: 960, volume: 0.2));
      q.add(two!);

      expect(q.takeNext(), same(one));
      expect(q.takeNext(), isNull);
      q.finishCurrent();
      expect(q.takeNext(), same(two));
      q.finishCurrent();
      expect(q.isEmpty, isTrue);
    });
  });
}
