/// 非 Web 目标的语音识别兜底：**如实说不可用**（`null` = 没有实现）。
///
/// 见 `speech_recognizer.dart` 头注（条件导入的理由）。
library;

import 'speech_recognizer.dart';

/// VM / 非 Web：没有浏览器 Web Speech，返回 null。
SpeechRecognizer? createSpeechRecognizer() => null;
