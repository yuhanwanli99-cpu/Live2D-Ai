/// Web 目标的语音识别：浏览器 Web Speech API（`SpeechRecognition` /
/// `webkitSpeechRecognition`）。
///
/// 见 `speech_recognizer.dart` 头注（条件导入的理由）。
library;

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'speech_recognizer.dart';

@JS('window.SpeechRecognition')
external JSFunction? get _speechRecognitionCtor;

@JS('window.webkitSpeechRecognition')
external JSFunction? get _webkitSpeechRecognitionCtor;

extension type _Recognition._(JSObject _) implements JSObject {
  external set lang(String value);
  external set continuous(bool value);
  external set interimResults(bool value);
  external set maxAlternatives(int value);
  external void start();
  external void stop();
  external void abort();
  external set onresult(JSFunction value);
  external set onerror(JSFunction value);
  external set onend(JSFunction value);
}

extension type _ResultEvent._(JSObject _) implements JSObject {
  external int get resultIndex;
  external _ResultList get results;
}

extension type _ResultList._(JSObject _) implements JSObject {
  external int get length;
  external _RecognitionResult operator [](int index);
}

extension type _RecognitionResult._(JSObject _) implements JSObject {
  external int get length;
  external bool get isFinal;
  external _Alternative operator [](int index);
}

extension type _Alternative._(JSObject _) implements JSObject {
  external String get transcript;
}

extension type _ErrorEvent._(JSObject _) implements JSObject {
  external String get error;
}

/// 创建浏览器识别器；这个浏览器既不支持前缀版也不支持标准版 → null。
SpeechRecognizer? createSpeechRecognizer() {
  final JSFunction? ctor =
      _speechRecognitionCtor ?? _webkitSpeechRecognitionCtor;
  if (ctor == null) return null;
  return _WebSpeechRecognizer(ctor);
}

class _WebSpeechRecognizer implements SpeechRecognizer {
  _WebSpeechRecognizer(this._ctor);

  final JSFunction _ctor;
  _Recognition? _rec;

  @override
  Future<void> start({
    required String lang,
    required void Function(SpeechResult result) onResult,
    required void Function(String message, bool fatal) onError,
    required void Function() onEnd,
  }) async {
    await stop();
    final _Recognition rec =
        _Recognition._(_ctor.callAsConstructor<JSObject>());
    rec.lang = lang;
    // 常驻：一句话说完不结束会话（静音仍会 onend，调用方重启）。
    rec.continuous = true;
    rec.interimResults = true;
    rec.maxAlternatives = 1;
    rec.onresult = ((JSObject event) {
      final _ResultEvent e = _ResultEvent._(event);
      for (int i = e.resultIndex; i < e.results.length; i++) {
        final _RecognitionResult r = e.results[i];
        if (r.length == 0) continue;
        onResult(
          SpeechResult(transcript: r[0].transcript, isFinal: r.isFinal),
        );
      }
    }).toJS;
    rec.onerror = ((JSObject event) {
      final String code = _ErrorEvent._(event).error;
      final String message = _errorMessage(code);
      if (message.isNotEmpty) onError(message, _isFatal(code));
    }).toJS;
    rec.onend = (() {
      _rec = null;
      onEnd();
    }).toJS;
    _rec = rec;
    try {
      rec.start();
    } catch (e) {
      _rec = null;
      onError('启动语音识别失败：$e', true);
    }
  }

  @override
  Future<void> stop() async {
    final _Recognition? rec = _rec;
    _rec = null;
    if (rec == null) return;
    try {
      rec.abort();
    } catch (_) {
      // 已经停了 / 还没起：忽略。
    }
  }

  @override
  Future<void> dispose() => stop();

  /// 浏览器错误码 → 一句可处置的人话（空串 = 不值得打扰用户，如主动中止）。
  String _errorMessage(String code) => switch (code) {
    'not-allowed' || 'service-not-allowed' =>
      '麦克风权限被拒绝：在浏览器地址栏允许麦克风后再试',
    'audio-capture' => '没有找到可用的麦克风设备',
    'network' => '语音识别服务不可达（浏览器的 Web Speech 需要联网）',
    'no-speech' => '没听到声音',
    'aborted' => '',
    _ => '语音识别出错（$code）',
  };

  /// 这些错误重试也不会好：权限 / 设备。网络与「没听到」可以继续常驻。
  bool _isFatal(String code) => switch (code) {
    'not-allowed' || 'service-not-allowed' || 'audio-capture' => true,
    _ => false,
  };
}
