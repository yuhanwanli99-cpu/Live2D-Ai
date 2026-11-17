/// Web 录音实现：`getUserMedia` + `AudioContext` 采集原始帧 → 16 位单声道 WAV。
///
/// 见 `wav_recorder.dart` 头注（为什么不用 `MediaRecorder`）。
///
/// 采集链：
///
/// ```text
/// mic ──> MediaStreamAudioSourceNode ──> AudioWorklet 替代的 ScriptProcessor ──> 静音 Gain ──> destination
/// ```
///
/// 用 `ScriptProcessorNode`（已废弃但**所有**目标浏览器都还有）而不是
/// `AudioWorklet`：Worklet 要额外一个模块文件 + 异步加载，对「录一段话」
/// 这个用途不划算。这里只做「收帧 + 拼接」，处理延迟不影响结果。
library;

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'asr_audio.dart';
import 'wav_recorder.dart';

@JS('navigator.mediaDevices')
external _MediaDevices? get _mediaDevices;

extension type _MediaDevices._(JSObject _) implements JSObject {
  external JSPromise<JSObject> getUserMedia(JSObject constraints);
}

@JS('window.AudioContext')
external JSFunction? get _audioContextCtor;

@JS('window.webkitAudioContext')
external JSFunction? get _webkitAudioContextCtor;

extension type _AudioContext._(JSObject _) implements JSObject {
  external num get sampleRate;
  external _SourceNode createMediaStreamSource(JSObject stream);
  external _ProcessorNode createScriptProcessor(int bufferSize, int inCh, int outCh);
  external _GainNode createGain();
  external JSObject get destination;
  external JSPromise<JSObject> resume();
  external JSPromise<JSObject> close();
}

extension type _SourceNode._(JSObject _) implements JSObject {
  external void connect(JSObject node);
  external void disconnect();
}

extension type _ProcessorNode._(JSObject _) implements JSObject {
  external void connect(JSObject node);
  external void disconnect();
  external set onaudioprocess(JSFunction value);
}

extension type _GainNode._(JSObject _) implements JSObject {
  external void connect(JSObject node);
  external num get gain;
  external void disconnect();
}

extension type _Stream._(JSObject _) implements JSObject {
  external JSArray<JSObject> getTracks();
}

extension type _Track._(JSObject _) implements JSObject {
  external void stop();
}

extension type _ProcessEvent._(JSObject _) implements JSObject {
  external _AudioBuffer get inputBuffer;
}

extension type _AudioBuffer._(JSObject _) implements JSObject {
  external int get numberOfChannels;
  external int get length;
  external Float32List getChannelData(int channel);
}

/// Web：`getUserMedia` 可用就返回实现，否则 `null`。
WavRecorder? createPlatformWavRecorder() {
  if (_mediaDevices == null) return null;
  if (_audioContextCtor == null && _webkitAudioContextCtor == null) return null;
  return _WebWavRecorder();
}

class _WebWavRecorder implements WavRecorder {
  _AudioContext? _ctx;
  _SourceNode? _source;
  _ProcessorNode? _processor;
  _GainNode? _mute;
  _Stream? _stream;
  final List<double> _samples = <double>[];
  int _sourceRate = kAsrSampleRate;
  DateTime? _startedAt;

  @override
  Future<void> start() async {
    await cancel();
    final _MediaDevices? devices = _mediaDevices;
    final JSFunction? ctor = _audioContextCtor ?? _webkitAudioContextCtor;
    if (devices == null || ctor == null) {
      throw StateError('这个浏览器不支持录音（需要 getUserMedia + AudioContext）');
    }
    // 只要单声道 16 kHz 附近：回声消除 / 降噪对「说话」有益，采样率交给设备，
    // 采集后自己降采样（见 downsample）。
    final _Stream stream = _Stream._(
      await devices
          .getUserMedia(
            <String, Object?>{
              'audio': <String, Object?>{
                'channelCount': 1,
                'echoCancellation': true,
                'noiseSuppression': true,
                'autoGainControl': true,
              },
            }.jsify()! as JSObject,
          )
          .toDart,
    );
    final _AudioContext ctx = _AudioContext._(ctor.callAsConstructor<JSObject>());
    // 自动播放策略：录音也要一次 resume（用户刚点了按钮，手势还在）。
    await ctx.resume().toDart;
    final _SourceNode source = ctx.createMediaStreamSource(stream);
    final _GainNode mute = ctx.createGain();
    // 支路音量保持默认 1：某些实现会把 0 增益的支路整体优化掉，处理器就收不到帧。
    // （用户听不到任何声音——destination 只是让 ScriptProcessor 跑起来的锚点，
    //  这里不把麦克风信号接到扬声器输出上。
    //  wait: 见下，我们不 connect 到 destination 也能收到帧时会更干净；
    //  但 Safari 需要一条通向 destination 的路径，所以保留。）
    final _ProcessorNode processor = ctx.createScriptProcessor(4096, 1, 1);
    processor.onaudioprocess = ((JSObject event) {
      final _ProcessEvent e = _ProcessEvent._(event);
      final _AudioBuffer buffer = e.inputBuffer;
      if (buffer.numberOfChannels < 1) return;
      final Float32List channel = buffer.getChannelData(0);
      for (int i = 0; i < channel.length && i < buffer.length; i++) {
        _samples.add(channel[i]);
      }
    }).toJS;
    source.connect(processor);
    processor.connect(mute);
    mute.connect(ctx.destination);

    _ctx = ctx;
    _source = source;
    _processor = processor;
    _mute = mute;
    _stream = stream;
    _sourceRate = ctx.sampleRate.toInt();
    _samples.clear();
    _startedAt = DateTime.now();
  }

  @override
  Future<RecordedAudio?> stop() async {
    final DateTime? started = _startedAt;
    final int rate = _sourceRate;
    final List<double> raw = List<double>.of(_samples);
    await _teardown();
    if (raw.isEmpty) return null;
    final List<double> mono = downsample(raw, rate, kAsrSampleRate);
    final Uint8List wav = encodeWav16Mono(mono);
    final int durationMs = started == null
        ? 0
        : DateTime.now().difference(started).inMilliseconds;
    return RecordedAudio(
      wav: wav,
      sampleRate: kAsrSampleRate,
      durationMs: durationMs,
    );
  }

  @override
  Future<void> cancel() async {
    _samples.clear();
    await _teardown();
  }

  @override
  Future<void> dispose() => cancel();

  Future<void> _teardown() async {
    _processor?.onaudioprocess = (() {}).toJS;
    try {
      _processor?.disconnect();
      _source?.disconnect();
      _mute?.disconnect();
    } catch (_) {
      // 已经断了 / 还没连上：忽略。
    }
    _processor = null;
    _source = null;
    _mute = null;
    final _Stream? stream = _stream;
    _stream = null;
    if (stream != null) {
      for (final JSObject track in stream.getTracks().toDart) {
        try {
          _Track._(track).stop();
        } catch (_) {
          // 轨道已经停了：忽略。
        }
      }
    }
    final _AudioContext? ctx = _ctx;
    _ctx = null;
    if (ctx != null) {
      try {
        await ctx.close().toDart;
      } catch (_) {
        // 关闭失败不影响结果。
      }
    }
    _startedAt = null;
  }
}
