/// 停止 / 新消息的取消纪律（协议 V7 §6.6 + V10 §9.3）。
///
/// **同步取消、不补帧**：清动作 cue 计划 + 撤销两个槽（动作 / 表情）+
/// 丢掉 TTS 待播 + 回该会话 baseline。这里只编排**四个回调的顺序**——
/// 每个回调的实体（DirectorCuePlan / Live2DStage / AudioPlayer / host API）
/// 由 main.dart 注入，纯逻辑因此可在 VM 上回归。
library;

/// 取消的一步：接收原因（stop / new-message / voice …），无返回值。
typedef StageCancelStep = void Function(String reason);

class StageCancellation {
  StageCancellation({
    required this.clearCuePlan,
    required this.revokeStage,
    required this.dropPendingAudio,
    required this.returnToBaseline,
  });

  /// ① 清动作：丢掉当前 action_cue 计划（取用即移除 ⇒ 之后不补帧）。
  final StageCancelStep clearCuePlan;

  /// ② 清动作 + 表情：向渲染面发一次撤销哨兵（两槽同清）。
  final StageCancelStep revokeStage;

  /// ③ 清 TTS 待播：音频队列 + 攒句器 + 口型归零。
  final StageCancelStep dropPendingAudio;

  /// ④ 回该会话 baseline（V10；host 侧接口未冻结，见报告未决）。
  final StageCancelStep returnToBaseline;

  int get cancelCount => _cancelCount;
  int _cancelCount = 0;

  /// 最近一次取消按序执行的步骤（排障 / 回归用）。
  List<String> get lastSteps => List<String>.unmodifiable(_lastSteps);
  List<String> _lastSteps = const <String>[];

  /// 执行一次取消。[reason] 原样传给四个回调。
  void cancel(String reason) {
    _cancelCount++;
    final List<String> steps = <String>[];
    clearCuePlan(reason);
    steps.add('clear-cue-plan');
    revokeStage(reason);
    steps.add('revoke-stage');
    dropPendingAudio(reason);
    steps.add('drop-pending-audio');
    returnToBaseline(reason);
    steps.add('return-to-baseline');
    _lastSteps = steps;
  }
}
