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

/// 统一「停止」键可按吗（2026-10-15 执行单 §2）。
///
/// 三条任一非空就得让用户按得到：
/// - [turnInFlight]：这一轮还在飞（`ChatController.streaming`）；
/// - [audioPending]：浏览器里还排着没播完的句子（`AudioPlayer.hasPending`）；
/// - [cuesPending]：舞台上还压着没演完的导演 cue（`DirectorCuePlan.length`）。
///
/// 输入区那颗圆键与 Esc 都用这三条判据，**界面相位不再参与**——服务端推完
/// PCM 之后相位先归零，尾巴只剩这三条看得见。三者都没有 = 按下去什么都
/// 不会发生，那时**不可按**（不摆假出路）。
bool canStopPlayback({
  required bool turnInFlight,
  required bool audioPending,
  required bool cuesPending,
}) => turnInFlight || audioPending || cuesPending;

/// 按下统一「停止」：**永远**先取消舞台与待播音频，仅当这一轮还在飞时再中止本轮。
///
/// # 两步的分工（顺序是契约）
///
/// 1. [cancelStage]（`StageCancellation.cancel` 那条路）：清表演计划、撤销两个
///    槽、丢掉 TTS 待播与半截攒句、回待机。**同步取消、不补帧**。
/// 2. [stopTurn]（`ChatController.stop` 那条路）：本地收口 + `POST /chat/stop`。
///    只有 [turnInFlight] 为真才走——空闲时那一条会凭空收口一轮（落盘 + 一条
///    「已停止本轮」），而用户表达的只是「别把尾巴播完」；`/chat/stop` 本身幂等，
///    也不为了丢队列再打一次。
///
/// 先取消再 stop 与既有 `_stopWithCancellation` 同一条纪律：等服务端回执才清
/// 动作，就会在没有声音时把旧 cue 演完（V7 §6.6）。
///
/// 抽成纯编排的理由与 [StageCancellation] 相同：`main.dart` 是 `package:web`
/// 组合根，VM 里 import 不了，只能把「谁在什么条件下被调用、按什么顺序」钉在
/// 可测的这一层（回归见 `test/stage_cancel_test.dart`）。
Future<void> stopPlayback({
  required bool turnInFlight,
  required String reason,
  required StageCancelStep cancelStage,
  required Future<void> Function() stopTurn,
}) async {
  cancelStage(reason);
  if (!turnInFlight) return;
  await stopTurn();
}
