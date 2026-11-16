/// 非 Web 构建的录音器占位（永远不支持）。
///
/// 见 `wav_recorder.dart` 头注（条件导入的理由）：VM 测试与桌面构建没有
/// `getUserMedia`，这里明确回「不支持」，界面据此禁用按钮——而不是摆一个
/// 按不动的按钮假装能录。
library;

import 'wav_recorder.dart';

/// 非 Web：永远 `null`（不支持录音）。
WavRecorder? createPlatformWavRecorder() => null;
