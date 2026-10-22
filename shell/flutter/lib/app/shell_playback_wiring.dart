part of 'package:live2d_ai_shell/main.dart';

/// 播放忙闲的两条下游（0.3.14）：**静默上报** + 「播完清按句 cue」。
///
/// 共用同一个信号：`AudioPlayer.pendingState`（[AudioPlayer.hasPending] 的变化
/// 通知，**只读**——播放路径本身一行没改）。
///
/// 1. `POST /api/v1/playback {"busy":…}`：服务端据此判断浏览器播完没有，好放行
///    下一条外部注入。失败一律静默（见 `api/playback_api.dart` 的文件头注）。
/// 2. 播放**从忙变闲**、且聊天不在 `streaming` ⇒ 还没开播的按句 cue 作废：
///    它们属于上一轮，而下一轮的 `sentence_seq` 从 1 重新计（执行单 §原因 3）。
///
/// 为什么是 part 而不是 `main.dart`：main.dart 贴着 800 行硬棘轮，而这一块是
/// **接线**（与 `shell_cue_voice_wiring.dart` 同一层）。去重与传输在
/// `api/playback_api.dart`（VM 可测）；这里只有订阅与释放。
extension _ShellPlaybackWiring on _ShellRootState {
  void _initPlaybackWiring() {
    _playbackApi = PlaybackApi();
    _playbackReporter = PlaybackReporter(send: _playbackApi.report);
    _audio.pendingState.addListener(_onPlaybackPendingChanged);
  }

  /// `hasPending` 翻转：**先**静默上报，**再**判要不要清计划。
  ///
  /// 只在「忙 → 闲**且**聊天不在 `streaming`」清：还在 streaming 说明后面
  /// 还会有句子到（音频是逐句下发的），这时清掉会把**本轮下一句**的 cue 也丢进
  /// 垃圾桶。上报与清理都只看这一帧的通知，不额外轮询队列。
  void _onPlaybackPendingChanged() {
    final bool busy = _audio.pendingState.value;
    _playbackReporter.report(busy);
    if (!busy && !_chat.streaming) _directorCues.clear();
  }

  void _disposePlaybackWiring() {
    _audio.pendingState.removeListener(_onPlaybackPendingChanged);
    _playbackApi.dispose();
  }
}
