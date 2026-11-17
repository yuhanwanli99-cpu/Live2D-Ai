/// 听写控制器回归（2026-11-16：整条链路改为「录音 → /api/v1/voice/asr → 输入框」）。
///
/// # 这一版守什么
///
/// ① 点一下开录、再点一下停：**录到的东西**才送识别，空录 = 「没听清」；
/// ② 识别结果**只交宿主**（[VoiceListenController.onDictation]），控制器
///    本身不发对话（与端点不调 `say` 是同一条纪律）；
/// ③ 失败 / 空文本都走同一个回调（空串 = 宿主不动输入框）；
/// ④ 没有录音能力的构建（VM）→ `supported == false` + 可读原因；
/// ⑤ 出声期间不许开录（回声会把自己念的字收进去）；
/// ⑥ 太大的音频在本机就拦（不发一次注定被 413 的请求）。
///
/// 用假录音器 + 假识别器，**不需要 WSL 有麦克风**。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/voice/asr_audio.dart';
import 'package:live2d_ai_shell/voice/voice_listen_controller.dart';
import 'package:live2d_ai_shell/voice/wav_recorder.dart';

/// 假录音器：`stop()` 返回预设的一段 WAV（`null` = 什么都没录到）。
class FakeRecorder implements WavRecorder {
  FakeRecorder({List<int>? wav, bool silent = false})
    : _wav = silent ? null : (wav ?? _tinyWav());

  final List<int>? _wav;
  int starts = 0;
  int stops = 0;
  int cancels = 0;
  int disposes = 0;
  bool failOnStart = false;
  bool started = false;

  @override
  Future<void> start() async {
    starts++;
    if (failOnStart) throw StateError('麦克风权限被拒绝：在浏览器地址栏允许麦克风后再试');
    started = true;
  }

  @override
  Future<RecordedAudio?> stop() async {
    stops++;
    started = false;
    final List<int>? wav = _wav;
    if (wav == null) return null;
    return RecordedAudio(wav: wav, sampleRate: kAsrSampleRate, durationMs: 700);
  }

  @override
  Future<void> cancel() async {
    cancels++;
    started = false;
  }

  @override
  Future<void> dispose() async {
    disposes++;
  }

  static List<int> _tinyWav() =>
      encodeWav16Mono(List<double>.generate(1600, (int i) => 0.1));
}

/// 假识别器：记下收到的字节，回预设文本（或抛异常）。
class FakeTranscriber {
  FakeTranscriber({this.text = '你好。', this.error});

  String text;
  Object? error;
  List<List<int>> calls = <List<int>>[];

  Future<String> call(List<int> wav) async {
    calls.add(wav);
    final Object? e = error;
    if (e != null) throw e;
    return text;
  }
}

VoiceListenController make({
  WavRecorder? recorder,
  bool? noRecorder,
  FakeTranscriber? transcriber,
  void Function(String)? onDictation,
}) {
  final FakeTranscriber t = transcriber ?? FakeTranscriber();
  return VoiceListenController(
    // `noRecorder: true` 显式表达「这个构建没有录音器」（与「没传 = 用假录音器」区分）。
    recorder: noRecorder == true ? null : (recorder ?? FakeRecorder()),
    transcribe: t.call,
    onDictation: onDictation ?? (_) {},
  );
}

void main() {
  test('① 点一下开录、再点一下停：录到的整段 WAV 才送识别', () async {
    final FakeRecorder recorder = FakeRecorder();
    final FakeTranscriber transcriber = FakeTranscriber(text: '你好，世界。');
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      recorder: recorder,
      transcriber: transcriber,
      onDictation: delivered.add,
    );

    expect(c.listening, isFalse);
    await c.toggleDictation();
    expect(c.listening, isTrue, reason: '第一次点 = 开始录');
    expect(recorder.starts, 1);
    expect(c.statusLine, contains('正在听'));

    await c.toggleDictation();
    expect(c.listening, isFalse, reason: '第二次点 = 结束');
    expect(recorder.stops, 1);
    expect(transcriber.calls, hasLength(1), reason: '必须把整段音频送出去');
    expect(delivered, <String>['你好，世界。']);
    expect(c.lastText, '你好，世界。');
    expect(c.error, isNull);
  });

  test('② 识别结果只交宿主：控制器不自己发对话（没有 send 通道）', () async {
    final List<String> delivered = <String>[];
    final FakeTranscriber transcriber = FakeTranscriber(text: '嗯。');
    final VoiceListenController c = make(
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.toggleDictation();
    // 唯一的产物就是回调那一次；控制器上没有「发一轮对话」的入口。
    expect(delivered, <String>['嗯。']);
    expect(c.lastText, '嗯。');
  });

  test('③a 什么都没录到：不送识别，回调给空串 + 「没听清」', () async {
    final FakeRecorder recorder = FakeRecorder(silent: true);
    final FakeTranscriber transcriber = FakeTranscriber();
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      recorder: recorder,
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.toggleDictation();
    expect(transcriber.calls, isEmpty, reason: '空录音不该打网络');
    expect(delivered, <String>['']);
    expect(c.error, contains('没听清'));
  });

  test('③b 识别返回空文本：仍是「没听清」，宿主拿到空串', () async {
    final FakeTranscriber transcriber = FakeTranscriber(text: '   ');
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.toggleDictation();
    expect(transcriber.calls, hasLength(1));
    expect(delivered, <String>['']);
    expect(c.error, contains('没听清'));
  });

  test('③c 识别抛异常：错误可读，宿主拿到空串（输入框不动）', () async {
    final FakeTranscriber transcriber = FakeTranscriber(
      error: StateError('network_error: 无法连接后端'),
    );
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.toggleDictation();
    expect(delivered, <String>['']);
    expect(c.error, contains('无法连接后端'));
  });

  test('④ 没有录音能力：supported=false，点了给可读原因，不开麦', () async {
    final VoiceListenController c = make(noRecorder: true);
    expect(c.supported, isFalse);
    // 构造时给的是 null，内部 record 为 null。
    await c.toggleDictation();
    expect(c.listening, isFalse);
    expect(c.error, kDictationUnsupportedMessage);
  });

  test('④b 开麦失败（权限 / 没设备）：错误直接给用户看，不进入录音态', () async {
    final FakeRecorder recorder = FakeRecorder()..failOnStart = true;
    final VoiceListenController c = make(recorder: recorder);
    await c.toggleDictation();
    expect(c.listening, isFalse);
    expect(c.error, contains('麦克风权限被拒绝'));
  });

  test('⑤ 出声期间不许开录（暂停期点「听」给明确原因）', () async {
    final FakeRecorder recorder = FakeRecorder();
    final VoiceListenController c = make(recorder: recorder);
    await c.suspendForPlayback();
    await c.toggleDictation();
    expect(c.listening, isFalse);
    expect(recorder.starts, 0, reason: '暂停期一个字节都不该开麦');
    expect(c.error, kDictationBusyMessage);

    await c.resumeAfterPlayback();
    await c.toggleDictation();
    expect(c.listening, isTrue, reason: '恢复之后可以正常开录');
  });

  test('⑤b 播报开始时正在录：停掉并保留已听到的部分（不丢用户的话）', () async {
    final FakeRecorder recorder = FakeRecorder();
    final FakeTranscriber transcriber = FakeTranscriber(text: '保留我。');
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      recorder: recorder,
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.suspendForPlayback();
    expect(c.listening, isFalse);
    expect(delivered, <String>['保留我。'], reason: '已经录到的要交出去');
  });

  test('⑥ 过大的音频在本机就拦（不发一次注定被 413 的请求）', () async {
    final List<int> huge = List<int>.filled(kAsrMaxUploadBytes + 64, 0);
    final FakeRecorder recorder = FakeRecorder(wav: huge);
    final FakeTranscriber transcriber = FakeTranscriber();
    final List<String> delivered = <String>[];
    final VoiceListenController c = make(
      recorder: recorder,
      transcriber: transcriber,
      onDictation: delivered.add,
    );
    await c.toggleDictation();
    await c.toggleDictation();
    expect(transcriber.calls, isEmpty, reason: '超限音频不该发给后端');
    expect(delivered, <String>['']);
    expect(c.error, contains('太长'));
  });

  test('⑦ 忘记再点一下：到点自动停（不一直占着麦克风）', () async {
    final FakeRecorder recorder = FakeRecorder();
    final FakeTranscriber transcriber = FakeTranscriber(text: '自动。');
    final List<String> delivered = <String>[];
    final VoiceListenController c = VoiceListenController(
      recorder: recorder,
      transcribe: transcriber.call,
      onDictation: delivered.add,
      busyWindow: const Duration(milliseconds: 30),
    );
    await c.toggleDictation();
    expect(c.listening, isTrue);
    await Future<void>.delayed(const Duration(milliseconds: 120));
    expect(c.listening, isFalse, reason: '到点应自动结束');
    expect(recorder.stops, 1);
    expect(delivered, <String>['自动。']);
  });

  test('⑧ appendVoiceText：空串不动输入框；非空接在后面（中间一个空格）', () {
    expect(appendVoiceText('已有', ''), '已有');
    expect(appendVoiceText('已有', '   '), '已有');
    expect(appendVoiceText('', '新'), '新');
    expect(appendVoiceText('已有', '新'), '已有 新');
  });

  test('⑨ dispose 之后不再通知也不再送（防泄漏）', () async {
    final FakeRecorder recorder = FakeRecorder();
    final VoiceListenController c = make(recorder: recorder);
    c.dispose();
    expect(recorder.disposes, 1);
    await c.toggleDictation();
    expect(c.listening, isFalse);
  });
}
