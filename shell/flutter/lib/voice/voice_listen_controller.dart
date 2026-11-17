/// 产品听写的**控制器**：点一下录、再点一下停 → 整段 WAV 交给
/// `POST /api/v1/voice/asr` → 文本进输入框。
///
/// # 2026-11-16：整条识别链路换了
///
/// - **不再用浏览器 Web Speech**：那会把音频送到浏览器厂商的云端，而且
///   「识别结果 → 输入框」这条路上还有一层厂商 SDK 的行为差异；
/// - **不再有唤醒词 / 常驻监听 / PTT / Mod 启停预检**：这些随 `voice-input`
///   Mod 一起退休（端点 `/api/v1/voice/transcript` 现在是 `410 voice_retired`）；
/// - 现在是：录音（`WavRecorder`）→ 16 位单声道 WAV → `/api/v1/voice/asr`
///   → 上游文本去首尾空白 → [onDictation] → 宿主写进输入框。
///
/// # 一门心思只做一件事
///
/// 控制器**不发对话**：识别成功不等于用户要发这句话。文本回到输入框，
/// 由用户自己按发送（这与核心端点「不调用 `say`」是同一条纪律）。
library;

// 私有字段 + 命名参数无法用 this._x 初始化（Dart 不允许下划线开头的命名参数），
// ignore_for_file: prefer_initializing_formals

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'asr_audio.dart';
import 'wav_recorder.dart';

/// 扬声器在说话时不允许开录的一句（回声会让识别把自己念的字也收进去）。
const String kDictationBusyMessage = '正在出声：说完再点「听」';

/// 这个构建没有录音能力时的一句（Web 才有 `getUserMedia`）。
const String kDictationUnsupportedMessage =
    '这个构建不能录音（需要浏览器 + 麦克风权限）';

/// 听写按钮旁的一句**诚实说明**。
///
/// 2026-11-16 起音频送的是**本项目配置的识别端点**（DeepSeek `/v1/tools/asr`），
/// 不再是浏览器厂商的 Web Speech 云端：所以这段文案要说清「录音会上传」，
/// 但要与「浏览器云端」那句区分开。
const String kDictationAsrNote = '听写会录一段音频上传到已配置的识别端点；结果只进输入框，不会自动发送。';

/// 识别器把整段 WAV 送出去、拿回一段文本。
///
/// 抛异常（网络 / 上游错误）= 没听清；返回空串同样是没听清。
typedef AsrTranscribe = Future<String> Function(List<int> wav);

/// 识别结果落回宿主的回调（空串 = 没听清，宿主不动输入框）。
typedef VoiceResultSink = void Function(String text);

class VoiceListenController extends ChangeNotifier {
  VoiceListenController({
    WavRecorder? recorder,
    required AsrTranscribe transcribe,
    this.onDictation,
    this.busyWindow = const Duration(seconds: 8),
  }) : _recorder = recorder,
       _transcribe = transcribe;

  /// 录音器（`null` = 这个构建不支持录音；按钮禁用并说明）。
  final WavRecorder? _recorder;
  final AsrTranscribe _transcribe;

  /// **产品听写的定稿**：成功与失败（空串）都走它。
  final VoiceResultSink? onDictation;

  /// 录多久之后自动停（保护：用户忘了再点一下）。
  final Duration busyWindow;

  bool _dictating = false;
  bool _sending = false;
  bool _suspended = false;
  bool _disposed = false;
  String? _error;
  Timer? _autoStop;
  String _lastSent = '';

  /// 这个构建能不能录音（Web 且有 `getUserMedia`）。
  bool get supported => _recorder != null;

  /// 正在录。
  bool get listening => _dictating;

  /// 最后一句识别结果（空串 = 还没有过）。
  String get lastText => _lastSent;

  /// 可读的错误（`null` = 没有错误）。
  String? get error => _error;

  /// 按钮上方那一行状态（`null` = 不画）。
  String? get statusLine {
    if (_dictating) return '正在听…（再点一下结束）';
    if (_sending) return '识别中…';
    return null;
  }

  /// 点一下听、再点一下停。**产品路径只有这一条**。
  Future<void> toggleDictation() async {
    if (_dictating) {
      await stopDictation();
      return;
    }
    await startDictation();
  }

  /// 开始录音。
  ///
  /// 三个前置拒绝（都不开麦）：不支持 / 正在出声 / 已经送过一次还在等结果。
  Future<void> startDictation() async {
    if (_disposed || _dictating) return;
    final WavRecorder? recorder = _recorder;
    if (recorder == null) {
      _error = kDictationUnsupportedMessage;
      _safeNotify();
      return;
    }
    if (_suspended) {
      _error = kDictationBusyMessage;
      _safeNotify();
      return;
    }
    if (_sending) return;
    _error = null;
    _lastSent = '';
    try {
      await recorder.start();
    } catch (e) {
      _error = '$e';
      _safeNotify();
      return;
    }
    if (_disposed) {
      await recorder.cancel();
      return;
    }
    _dictating = true;
    // 忘了再点一下的保护：到点自动停，不会一直占着麦克风。
    _autoStop?.cancel();
    _autoStop = Timer(busyWindow, () {
      if (_dictating) unawaited(stopDictation());
    });
    _safeNotify();
  }

  /// 结束录音 → 整段 WAV → `/api/v1/voice/asr` → 交给宿主。
  Future<void> stopDictation() async {
    if (!_dictating) return;
    _dictating = false;
    _autoStop?.cancel();
    _autoStop = null;
    final WavRecorder? recorder = _recorder;
    final RecordedAudio? audio = await recorder?.stop();
    if (_disposed) return;
    if (audio == null) {
      _finish('', '没听清，再点一次说');
      return;
    }
    // 本机先判大小：省一次注定被 413 的上行。
    final String? reject = audioRejectReason(
      audio.wav is Uint8List ? audio.wav as Uint8List : Uint8List.fromList(audio.wav),
    );
    if (reject != null) {
      _finish('', reject);
      return;
    }
    _sending = true;
    _safeNotify();
    String text = '';
    String? failure;
    try {
      text = (await _transcribe(audio.wav)).trim();
    } catch (e) {
      failure = '$e';
    } finally {
      _sending = false;
    }
    if (_disposed) return;
    if (failure != null) {
      _finish('', failure);
      return;
    }
    if (text.isEmpty) {
      _finish('', '没听清，再点一次说');
      return;
    }
    _finish(text, null);
  }

  /// 收尾：记结果 / 错误，交给宿主（**成功与失败都走** [onDictation]）。
  void _finish(String text, String? error) {
    _error = error;
    _lastSent = text;
    _safeNotify();
    onDictation?.call(text);
  }

  /// 角色开始出声 → 不允许开录（回声会把自己念的字收进去）。
  ///
  /// 正在录的话**停掉并保留已听到的部分**（与旧行为同一条：用户说的话不丢）。
  Future<void> suspendForPlayback() async {
    if (_disposed || _suspended) return;
    _suspended = true;
    if (_dictating) {
      await stopDictation();
      return;
    }
    _safeNotify();
  }

  /// 出声结束 → 允许再录。
  Future<void> resumeAfterPlayback() async {
    if (_disposed || !_suspended) return;
    _suspended = false;
    _safeNotify();
  }

  void _safeNotify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _autoStop?.cancel();
    _autoStop = null;
    unawaited(_recorder?.dispose());
    super.dispose();
  }
}

/// 把一次听写定稿接到输入框已有文字的后面（**中间补一个空格**）。纯函数，可单测。
///
/// - [incoming] trim 后为空 → **原样返回** [existing]（输入框不动）；
/// - [existing] trim 后为空 → 就是 trim 过的 [incoming]；
/// - 否则 = `existing + ' ' + incoming`（光标由调用方落到末尾）。
String appendVoiceText(String existing, String incoming) {
  final String body = incoming.trim();
  if (body.isEmpty) return existing;
  return existing.trim().isEmpty ? body : '$existing $body';
}
