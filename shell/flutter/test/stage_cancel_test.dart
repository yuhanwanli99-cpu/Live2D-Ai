/// 阶段4d：停止 / 新消息的**取消纪律**回归（V7 §6.6 / V10 §9.3）。
///
/// 停止键与新用户消息都必须**同步**做四件事：清动作 cue 计划 + 撤销舞台两个槽
/// （动作 / 表情）+ 丢掉 TTS 待播 + 回该会话 baseline。清完**不补帧**——旧 cue
/// 已经不在计划里，任何迟到的 apply 都是空操作。
///
/// 顺序与「四步都发生」由 `StageCancellation`（纯逻辑）钉住；接线在 main.dart
/// （`package:web` 加载不了）用源码扫描钉住，先例见 `action_scales_wiring_test.dart`。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/stage_cancel.dart';

void main() {
  test('stop_and_new_message_clear_action_expression_and_pending_tts', () async {
    // 计划里先放一条真 cue：取消必须把它清掉。
    final DirectorCuePlan plan = DirectorCuePlan();
    plan.replace(<ActionCue>[
      const ActionCue(
        sentenceSeq: 2,
        presetId: 'nod',
        intensity: 2,
        ttlMs: 900,
        priority: 0,
      ),
    ]);
    expect(plan.length, 1);

    final List<String> calls = <String>[];
    final StageCancellation canceller = StageCancellation(
      clearCuePlan: (String reason) {
        plan.replace(const <ActionCue>[]);
        calls.add('clear-cue-plan:$reason');
      },
      revokeStage: (String reason) => calls.add('revoke-stage:$reason'),
      dropPendingAudio: (String reason) =>
          calls.add('drop-pending-audio:$reason'),
      returnToBaseline: (String reason) =>
          calls.add('return-to-baseline:$reason'),
    );

    // ① 停止键。
    canceller.cancel('stop');
    expect(calls, <String>[
      'clear-cue-plan:stop',
      'revoke-stage:stop',
      'drop-pending-audio:stop',
      'return-to-baseline:stop',
    ], reason: '四步都要发生，且顺序固定');
    expect(plan.length, 0, reason: '清动作');

    // ② **不补帧**：计划已清，该句再「开始播放」也不得 apply 任何 cue。
    int applied = 0;
    await plan.applyForSeq(2, (ActionCue cue) async {
      applied += 1;
    });
    expect(applied, 0, reason: '取消后不补帧：旧 cue 不得被重放');

    // ③ 新用户消息：同一套四步（不是只清音频）。
    canceller.cancel('new-message');
    expect(canceller.cancelCount, 2);
    expect(
      calls.where((String c) => c.endsWith(':new-message')),
      <String>[
        'clear-cue-plan:new-message',
        'revoke-stage:new-message',
        'drop-pending-audio:new-message',
        'return-to-baseline:new-message',
      ],
    );
  });

  test('接线（源码扫描）：发送 / 停止 / 重试都先走取消入口', () {
    final String main = File('lib/main.dart').readAsStringSync();
    expect(
      main.contains('onStop: () => unawaited(_stopWithCancellation())'),
      isTrue,
      reason: '停止键必须先取消（不能等服务端 new_epoch）',
    );
    expect(
      main.contains('onSend: () => unawaited(_sendWithCancellation())'),
      isTrue,
      reason: '新消息必须先取消',
    );
    expect(
      main.contains('onRetryLast: () => unawaited(_sendWithCancellation())'),
      isTrue,
      reason: '重试也是新消息',
    );
    expect(
      main.contains('_stageCancellation = StageCancellation('),
      isTrue,
      reason: '四个出口必须在组合根注入（顺序由纯逻辑钉住）',
    );
    expect(
      main.contains('_directorCues.replace(const <ActionCue>[])'),
      isTrue,
      reason: 'clearCuePlan 的实体 = 清空 action_cue 计划',
    );
    expect(
      main.contains("applyPreset('none', source: reason)"),
      isTrue,
      reason: 'revokeStage 的实体 = 撤销两个槽（none 哨兵）',
    );
    expect(
      main.contains('dropPendingAudio: (String reason) => _audio.interrupt()'),
      isTrue,
      reason: 'dropPendingAudio 的实体 = AudioPlayer.interrupt（清待播 + 口型归零）',
    );
    // 「新消息」还有语音注入这条入口（它不走 _send）。
    expect(
      main.contains("_cancelStageForTurnBoundary('voice')"),
      isTrue,
      reason: '语音注入也是新消息，必须先取消',
    );
  });
}
