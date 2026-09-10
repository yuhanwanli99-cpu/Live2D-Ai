import 'dart:async';
import 'dart:collection';
import 'dart:js_interop';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:web/web.dart' as web;

import 'schedule.dart';

/// 后端 WS audio 帧解码后的 PCM（s16le 单声道）。
class PcmFrame {
  const PcmFrame({
    required this.pcm,
    required this.sampleRate,
    this.start = false,
    this.end = false,
  });

  final Uint8List pcm;
  final int sampleRate;

  /// 服务端 `audio.start`：**仅表示「本轮语音的首片」**，不是时间轴重置指令。
  ///
  /// 调度**不读**本字段（原因见 [scheduleSlice] 的长注释）：曾据它把时间轴
  /// 拽回 now，导致一轮音频最多 17 片同时发声。保留字段以便协议自描述与排查。
  final bool start;

  /// 服务端 `audio.end`：仅作协议标记，调度不使用。
  final bool end;
}

/// s16le → [-1,1] 的 RMS 包络。
///
/// 参数与后端 `RmsMeter` 对齐：窗口 50ms、attack 0.03s、release 0.10s，
/// 按每帧（默认 20ms）推进。输出 `level ∈ [0,1]`。
class MouthEnvelope {
  MouthEnvelope({
    this.windowSec = 0.05,
    this.attackSec = 0.03,
    this.releaseSec = 0.10,
  });

  final double windowSec;
  final double attackSec;
  final double releaseSec;

  final ListQueue<double> _squares = ListQueue<double>();
  int _rate = 0;
  int _windowSamples = 0;
  double _sumSquares = 0;
  double _level = 0;

  double get level => _level;

  void reset() {
    _squares.clear();
    _sumSquares = 0;
    _level = 0;
  }

  /// 推入一帧 PCM，返回平滑后的口型 level。
  double push(Uint8List pcm, int sampleRate) {
    final rate = sampleRate > 0 ? sampleRate : 24000;
    if (rate != _rate) {
      _rate = rate;
      _windowSamples = math.max(1, (rate * windowSec).round());
      _squares.clear();
      _sumSquares = 0;
    }
    final samples = pcm.length ~/ 2;
    if (samples == 0) return _level;
    for (var i = 0; i < samples; i++) {
      var value = (pcm[i * 2 + 1] << 8) | pcm[i * 2];
      if (value >= 0x8000) value -= 0x10000;
      final normalized = value / 32768.0;
      final square = normalized * normalized;
      _squares.addLast(square);
      _sumSquares += square;
    }
    while (_squares.length > _windowSamples) {
      _sumSquares -= _squares.removeFirst();
    }
    final count = _squares.length;
    final rms = count == 0 ? 0.0 : math.sqrt(_sumSquares / count);
    final target = rms < 0 ? 0.0 : (rms > 1 ? 1.0 : rms);
    final frameSec = samples / rate;
    final tau = target > _level ? attackSec : releaseSec;
    var alpha = tau <= 0 ? 1.0 : frameSec / tau;
    if (alpha < 0) {
      alpha = 0;
    } else if (alpha > 1) {
      alpha = 1;
    }
    _level += (target - _level) * alpha;
    if (_level < 0) {
      _level = 0;
    } else if (_level > 1) {
      _level = 1;
    }
    return _level;
  }
}

class _ScheduledSource {
  const _ScheduledSource(this.node, this.endTime);

  final web.AudioBufferSourceNode node;
  final double endTime;
}

/// WebAudio 播放器 + 口型包络。
///
/// 三条不变量（「很吵的杂音」+「口型超前」两个缺陷的完整修复，
/// 算术核心与证据见 [scheduleSlice] / [LevelTimeline] 的长注释）：
///
/// 1. **时间轴只前进**：每片经 [scheduleSlice] 排在 `_nextStart`，**绝不因服务端
///    分片起始标记回退**。只有 [interrupt]（epoch 切换 / stop）才清零重排。
///    WebAudio 对同时发声的音源逐样本相加，回退即与上一片重叠 → 削波杂音。
/// 2. **口型按播放时刻释放**：包络在到达时计算（顺序与音频一致），但经
///    [LevelTimeline] 延迟到真正出声的时刻才推给渲染面——TTS 分片是 1 ms 级
///    突发，直接在到达时推电平会让嘴唇比声音早最多 3.36 s。
/// 3. **降级不破链**：无 WebAudio 或单帧调度失败只影响该帧，口型照常驱动。
///
/// 浏览器 autoplay 限制：首次用户手势调用 [unlock] resume 上下文。
class AudioPlayer {
  final StreamController<double> _levels =
      StreamController<double>.broadcast();
  final MouthEnvelope _envelope = MouthEnvelope();
  final LevelTimeline _pendingLevels = LevelTimeline();
  final List<_ScheduledSource> _scheduled = <_ScheduledSource>[];

  /// 口型释放节拍：20 ms（与音频分片同尺度；消费端 Live2D 桥另有 <=30 Hz 节流）。
  static const Duration _levelTick = Duration(milliseconds: 20);

  web.AudioContext? _context;
  Timer? _ticker;
  double _nextStart = 0;
  double _level = 0;
  bool _disposed = false;

  Stream<double> get levels => _levels.stream;
  double get level => _level;
  bool get hasAudioContext => _context != null;

  /// 首次用户手势调用：创建并 resume AudioContext。
  void unlock() {
    final context = _ensureContext();
    if (context == null) return;
    try {
      context.resume().toDart.ignore();
    } catch (_) {
      // resume 失败：播放降级，但口型仍由 RMS 驱动
    }
  }

  web.AudioContext? _ensureContext() {
    if (_disposed) return null;
    final existing = _context;
    if (existing != null) return existing;
    try {
      final context = web.AudioContext();
      _context = context;
      return context;
    } catch (_) {
      return null; // 无 WebAudio：静音播放，口型照常
    }
  }

  /// 播放一帧并推进口型包络。
  void feed(PcmFrame frame) {
    if (_disposed) return;
    // 包络在**到达时**计算：帧序即音频序，且 RMS 是流式状态。
    final level = _envelope.push(frame.pcm, frame.sampleRate);
    final context = _ensureContext();
    if (context == null) {
      // 无 WebAudio：没有播放时刻可对齐，口型照常驱动。
      _emitLevel(level);
      return;
    }
    try {
      final when = _schedule(context, frame);
      // 电平延迟到**真正出声的时刻**再释放（不变量 2）。
      _pendingLevels.push(when, level);
      _ticker ??= Timer.periodic(_levelTick, (_) => _tick());
    } catch (_) {
      // 单帧调度失败不影响后续帧与口型。
      _emitLevel(level);
    }
  }

  /// 排一片音频，返回其开始播放的时刻（AudioContext 时间轴）。
  double _schedule(web.AudioContext context, PcmFrame frame) {
    final samples = _toFloat32(frame.pcm);
    if (samples.isEmpty) return context.currentTime;
    _prune(context);
    final plan = scheduleSlice(
      nextStart: _nextStart,
      now: context.currentTime,
      durationSec: samples.length / frame.sampleRate,
    );
    final buffer = context.createBuffer(1, samples.length, frame.sampleRate);
    buffer.copyToChannel(samples.toJS, 0);
    final node = context.createBufferSource();
    node.buffer = buffer;
    node.connect(context.destination);
    node.start(plan.when);
    _nextStart = plan.nextStartAfter;
    _scheduled.add(_ScheduledSource(node, plan.nextStartAfter));
    return plan.when;
  }

  /// 释放到期的口型电平；待释放队列排空即停表（不在空闲时占着定时器）。
  void _tick() {
    final context = _context;
    if (_disposed || context == null) {
      _stopTicker();
      return;
    }
    final due = _pendingLevels.drain(context.currentTime);
    if (due != null) _emitLevel(due);
    if (_pendingLevels.isEmpty) _stopTicker();
  }

  void _stopTicker() {
    _ticker?.cancel();
    _ticker = null;
  }

  void _emitLevel(double level) {
    _level = level;
    if (!_levels.isClosed) _levels.add(level);
  }

  void _prune(web.AudioContext context) {
    final now = context.currentTime;
    _scheduled.removeWhere((scheduled) {
      if (scheduled.endTime > now) return false;
      try {
        scheduled.node.disconnect();
      } catch (_) {
        // 节点已结束
      }
      return true;
    });
  }

  /// 打断当前播放（epoch 切换 / stop）并清零口型。
  void interrupt() {
    for (final scheduled in _scheduled) {
      try {
        scheduled.node.stop();
      } catch (_) {
        // 未 start 或已停止
      }
      try {
        scheduled.node.disconnect();
      } catch (_) {
        // 已断开
      }
    }
    _scheduled.clear();
    _pendingLevels.clear();
    _stopTicker();
    _nextStart = 0;
    _envelope.reset();
    _emitLevel(0);
  }

  void dispose() {
    if (_disposed) return;
    interrupt();
    _disposed = true;
    final context = _context;
    _context = null;
    if (context != null) {
      try {
        context.close().toDart.ignore();
      } catch (_) {
        // 上下文已关闭
      }
    }
    _levels.close();
  }
}

Float32List _toFloat32(Uint8List pcm) {
  final count = pcm.length ~/ 2;
  final out = Float32List(count);
  for (var i = 0; i < count; i++) {
    var value = (pcm[i * 2 + 1] << 8) | pcm[i * 2];
    if (value >= 0x8000) value -= 0x10000;
    out[i] = value / 32768.0;
  }
  return out;
}
