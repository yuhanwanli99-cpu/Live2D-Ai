/// 「什么时候必须**立刻**打断音频」的**纯决策**（规格 §11.4 钉子 13）。
///
/// # 为什么把它单独抽出来
///
/// 这条判据守的是一个**真实缺陷的修复**，但它的原实现埋在 `chat_controller.dart`
/// 里——而那个文件必须 `import 'package:web'`，**在 `flutter test` 里加载不了**。
/// 于是「停止按钮等于失灵」这类回归永远抓不住。抽成纯函数后可单测。
///
/// # 缺陷本身
///
/// TTS 分片是 **1 ms 级突发**到达的（一句 2 s 音频在眨眼间到齐），
/// 播放时间轴最多排到 **3 s 之后**（实测调度提前量峰值 3.03 s）。
/// 如果只在「下一帧带新 epoch」时才比较代次，那么用户按下停止后，
/// **旧音频还会继续响到新轮次开口**——停止按钮等于失灵。
///
/// 所以 `new_epoch` 必须**立刻**打断，不能等下一片音频来做代次比较。
library;

import '../api/ws_frame.dart';

/// 这一帧是否要求**立刻**清空已排队的音频。
///
/// 只有 `runtime_status{event:"new_epoch"}` 是。它由服务端在
/// 「停止 / 抢占 / 新一轮受理」时下发，语义是「上一轮作废」。
bool mustInterruptAudio(WsEvent event) =>
    event is RuntimeStatusEvent && event.event == 'new_epoch';

/// 音频片的 epoch 与当前轮次不同 → 也要打断（跨轮残留的片）。
///
/// `current == null` 表示**还没开始过**，此时不算切换（第一片音频不该触发打断）。
bool audioEpochChanged({required int? current, required int incoming}) =>
    current != null && current != incoming;

/// 一次音频帧的处理决策（把两个判据合成一句，调用方照做即可）。
///
/// 顺序有讲究：**先判 epoch 变化，再更新当前 epoch**——
/// 反过来的话 `audioEpochChanged` 永远为假，跨轮残留的片就漏掉了。
class AudioGateDecision {
  const AudioGateDecision({required this.interrupt, required this.epoch});

  /// 处理这一片之前是否要先 `interrupt()`。
  final bool interrupt;

  /// 处理之后应记录为「当前轮次」的值。
  final int epoch;
}

/// 给定当前轮次与这一片的 epoch，给出该怎么做。
AudioGateDecision decideAudioFrame({
  required int? currentEpoch,
  required int incomingEpoch,
}) => AudioGateDecision(
  interrupt: audioEpochChanged(current: currentEpoch, incoming: incomingEpoch),
  epoch: incomingEpoch,
);
