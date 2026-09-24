/// 浏览器语音识别的**最小接口**（Web Speech API），可条件导入、可测试。
///
/// # 为什么要有这一层
///
/// 「常驻语音检测」需要浏览器的 `SpeechRecognition`（Chrome/Edge 上叫
/// `webkitSpeechRecognition`）——它是**平台能力**，不是核心逻辑：
/// - 逻辑（唤醒词判定、剥词、注入）住在
///   [VoiceListenController]（纯 Dart，VM 可测）；
/// - 平台分派住在这里：Web 拿浏览器实现，VM 测试拿 stub（返回 null，
///   界面如实说「当前构建不支持」而不是摆一个按不动的按钮）。
///
/// 见 `persona_card_picker.dart` 的同款条件导入理由。
library;

export 'speech_recognizer_stub.dart'
    if (dart.library.js_interop) 'speech_recognizer_web.dart';

/// 一段识别结果。
class SpeechResult {
  const SpeechResult({required this.transcript, required this.isFinal});

  /// 转写文本（浏览器给什么就是什么，清洗交给服务端）。
  final String transcript;

  /// 是否是**定稿**（false = 还在说的临时结果）。
  final bool isFinal;
}

/// 语音识别器（生命周期由调用方持有：`start` → 多次回调 → `stop`/`onEnd`）。
abstract class SpeechRecognizer {
  /// 开始识别；实现应在开始前先停掉上一次。
  ///
  /// `onEnd` 在识别会话自然结束（静音超时 / 浏览器主动结束）时回调——
  /// 常驻监听据此重启（见 [VoiceListenController]）。
  Future<void> start({
    required String lang,
    required void Function(SpeechResult result) onResult,
    /// fatal = true 时调用方应停止常驻重启（权限被拒 / 没有麦克风）。
    required void Function(String message, bool fatal) onError,
    required void Function() onEnd,
  });

  /// 主动停止（用户按「停止听」）。
  Future<void> stop();

  /// 释放资源。
  Future<void> dispose();
}
