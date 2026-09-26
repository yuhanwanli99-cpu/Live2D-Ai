/// 播放时钟 → 渲染面的 stage-clock 消息（协议 §6.5 / O13 冻结 wire 名）。
///
/// # 为什么在 audio/ 而不是 live2d/
///
/// 这条消息的**唯一时间基准是音频播放时钟**（V7 §6.1）：嘴跟音频、身体也必须
/// 跟音频。采样的生产者是 AudioPlayer（它已经有 30ms 节拍与段内位置），
/// 消费端是渲染面。把「什么时候该发」这条纯逻辑放这里，就能在 VM 上回归——
/// AudioPlayer 自己 import package:web，在 flutter test 里加载不了。
library;

/// 30ms 节奏（与 AudioPlayer 的口型节拍同值）：渲染面用它替代 performance.now()。
const Duration kStageClockInterval = Duration(milliseconds: 30);

/// 一次播放位置采样：seg = sentence_seq（段号，1 起），posMs = 段内毫秒，
/// playing = 此刻是否真有声音在走（暂停 / 缓冲中为 false）。
class StageClockSample {
  const StageClockSample({
    this.seg,
    required this.posMs,
    required this.playing,
  });

  final int? seg;
  final int posMs;
  final bool playing;
}

/// 把「每次采样」压成「该下发的样本」——**不补帧**。
///
/// - 播放中：同一段最快每 [interval] 放行一次（段切换立即放行）；
/// - 停播：只在 playing 翻转为 false 的那一次放行，此后静默（不重放旧帧）；
/// - 重播（false → true）立即放行。
class StageClockCadence {
  StageClockCadence({this.interval = kStageClockInterval});

  final Duration interval;

  bool _hasLast = false;
  bool _lastPlaying = false;
  int? _lastSeg;
  Duration _lastAt = Duration.zero;

  StageClockSample? accept(StageClockSample sample, Duration now) {
    final bool stateChanged = !_hasLast || sample.playing != _lastPlaying;
    final bool segChanged = _hasLast && sample.seg != _lastSeg;
    if (!sample.playing) {
      if (!stateChanged) return null; // 停播后不再补帧
    } else if (!stateChanged && !segChanged && now - _lastAt < interval) {
      return null;
    }
    _hasLast = true;
    _lastPlaying = sample.playing;
    _lastSeg = sample.seg;
    _lastAt = now;
    return sample;
  }

  void reset() {
    _hasLast = false;
    _lastPlaying = false;
    _lastSeg = null;
    _lastAt = Duration.zero;
  }
}
