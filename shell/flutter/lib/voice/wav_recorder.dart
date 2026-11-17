/// 浏览器录音：**16 位单声道 WAV 整段**（点一下开始、再点一下结束）。
///
/// # 为什么不用 `MediaRecorder`
///
/// `MediaRecorder` 给的是 webm/ogg 的**压缩流**，各浏览器编码参数还不一致；
/// 而识别端点要的是「整段 16 位 PCM WAV」。这里走 `getUserMedia` +
/// `AudioContext`：拿到的就是原始 `Float32` 帧，自己降采样 + 拼 RIFF 头
/// （见 `asr_audio.dart` 的 `encodeWav16Mono`）。
///
/// # 看不见的部分
///
/// 浏览器把音频送进 `AudioContext` 需要**一个输出节点**才会跑（脚本处理器
/// 自己不拉流）。所以这里接一个静音的 `GainNode` 到 destination——
/// 用户听不到任何东西，但处理器会持续收到帧。
library;

// 条件导入 + 条件导出各写一次：
// - **导入**：本库自己要用 `createPlatformWavRecorder`（见文件末尾的工厂函数）；
// - **导出**：调用方只 `import 'wav_recorder.dart'` 就够，不用知道平台分派。
import 'wav_recorder_stub.dart'
    if (dart.library.js_interop) 'wav_recorder_web.dart' as platform;
export 'wav_recorder_stub.dart'
    if (dart.library.js_interop) 'wav_recorder_web.dart'
    show createPlatformWavRecorder;

/// 一段录好的音频。
class RecordedAudio {
  const RecordedAudio({
    required this.wav,
    required this.sampleRate,
    required this.durationMs,
  });

  /// 整段 16 位单声道 WAV（44 字节头 + 裸 PCM）。
  final List<int> wav;

  /// 编码用的采样率（恒定 [kAsrSampleRate]，留字段是为了日志与断言）。
  final int sampleRate;

  /// 录制时长（毫秒）。
  final int durationMs;
}

/// 录音器（生命周期由调用方持有：`start` → `stop` → `RecordedAudio`）。
abstract class WavRecorder {
  /// 开始录音。失败（没权限 / 没设备）抛异常，消息**直接给用户看**。
  Future<void> start();

  /// 结束录音并返回整段 WAV。
  ///
  /// 没有任何样本时返回 `null`（调用方按「没听清」处理，不编一个空文件）。
  Future<RecordedAudio?> stop();

  /// 放弃这一段（用户中途取消 / 页面离开）。
  Future<void> cancel();

  /// 释放资源（关麦克风轨道）。
  Future<void> dispose();
}

/// 当前构建能不能录音（Web 看有没有 `getUserMedia`；VM 恒 false）。
bool get canRecordAudio => createWavRecorder() != null;

/// 创建录音器；不支持的构建返回 `null`（界面据此禁用按钮并说明）。
///
/// `createPlatformWavRecorder` 由条件导入提供（Web 实现 / 非 Web 占位）。
WavRecorder? createWavRecorder() => platform.createPlatformWavRecorder();
