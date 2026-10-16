/// 阶段4d：停止 / 新消息的**取消纪律**回归（V7 §6.6 / V10 §9.3）。
///
/// 停止键与新用户消息都必须**同步**做四件事：清动作 cue 计划 + 撤销舞台两个槽
/// （动作 / 表情）+ 丢掉 TTS 待播 + 回该会话 baseline。清完**不补帧**——旧 cue
/// 已经不在计划里，任何迟到的 apply 都是空操作。
///
/// 顺序与「四步都发生」由 `StageCancellation`（纯逻辑）钉住；接线在 main.dart
/// （`package:web` 加载不了）用**源码扫描**钉住，先例见 `action_scales_wiring_test.dart`。
///
/// 2026-10-06（R4-T5，D6）：接线那半从「钉字面量」改成「钉**语义**」——旧写法把
/// `onRetryLast: … _sendWithCancellation()` 这个**静默 no-op 形态**锁成了契约
/// （测试替缺陷站岗）。现在只断言「出口接的是哪条路」「那条路有没有先取消」，
/// 不认任何具体写法；注释一律剥掉，防止本文件自己的注释骗过断言。
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/stage_cancel.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

void main() {
  test(
    'stop_and_new_message_clear_action_expression_and_pending_tts',
    () async {
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
      expect(calls.where((String c) => c.endsWith(':new-message')), <String>[
        'clear-cue-plan:new-message',
        'revoke-stage:new-message',
        'drop-pending-audio:new-message',
        'return-to-baseline:new-message',
      ]);
    },
  );

  test('接线（语义级）：停止（Esc / 圆键）/ 发送 / 重试三条出口都先走取消入口', () {
    // 库 + parts（`main.dart` 的四个 part 一起扫）；字符串留着，注释剥掉。
    final String main = stripCommentsKeepStrings(
      readLibrarySource('lib/main.dart'),
    );
    final String shellChat = stripCommentsAndStrings(
      readLibrarySource('lib/app/shell_chat.dart'),
    );
    // Esc 出口：AppShell 的 `onStop` 已删，`main.dart` 里**只剩这一处**
    // `onStop:`（Esc）。它接的是与圆键同一条 `_stopPlayback`，带 `canStop`
    // 门，不再接 `_stopWithCancellation`（那条现在只服务「打断并重发」）。
    expect(
      _countOccurrences(main, 'onStop:'),
      1,
      reason: '两个 widget 的独立 onStop 都已删，剩下的只有 Esc',
    );
    expect(
      _argLine(main, 'onStop:'),
      contains('_stopPlayback'),
      reason: 'Esc 与圆键走同一条停止路',
    );
    expect(
      _argLine(main, 'onStop:'),
      isNot(contains('_stopWithCancellation')),
      reason: '别再让 Esc 走「打断并重发」那条',
    );
    expect(
      _argLine(main, 'onStop:'),
      contains('canStop'),
      reason: 'canStop 为假时 Esc 什么都不做（不接线，而不是空回调）',
    );
    expect(
      main.contains('onSend: () => unawaited(_sendWithCancellation())'),
      isTrue,
      reason: '新消息必须先取消',
    );
    // 「重试」= **重发上一条用户消息**（2026-10-06 裁决 R4-T5）：只钉它接在哪条
    // 路上，不钉写法——旧写法（读输入框的 `_sendWithCancellation`）正是缺陷。
    expect(
      _argLine(main, 'onRetryLast:'),
      contains('_resendLastUserMessage'),
      reason: '重试 = 重发上一条用户消息',
    );
    expect(
      _argLine(main, 'onRetryLast:'),
      isNot(contains('_sendWithCancellation')),
      reason: '旧形态读输入框 ⇒ 失败之后按下去毫无反应（静默 no-op）',
    );
    // 而「重发」这条路**自己**必须先取消——这才是本文件要守的纪律。
    expect(
      _functionBody(shellChat, 'Future<void> _resendLastUserMessage() async'),
      contains('_cancelStageForTurnBoundary'),
      reason: '重发也是新消息 ⇒ 必须先取消上一轮编排（与发送同一条纪律）',
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

  group('统一「停止」：可按条件 + 空闲时不动会话（2026-10-15 执行单 §2）', () {
    test('三条队列任一非空才可按（只在飞 / 只剩音频 / 只剩 cue / 都没有）', () {
      expect(
        canStopPlayback(
          turnInFlight: true,
          audioPending: false,
          cuesPending: false,
        ),
        isTrue,
        reason: '这一轮还在飞',
      );
      expect(
        canStopPlayback(
          turnInFlight: false,
          audioPending: true,
          cuesPending: false,
        ),
        isTrue,
        reason: '服务端已把 PCM 推完、相位已落，但浏览器里还排着句子',
      );
      expect(
        canStopPlayback(
          turnInFlight: false,
          audioPending: false,
          cuesPending: true,
        ),
        isTrue,
        reason: '音频播完了，舞台上还压着没演完的 cue',
      );
      expect(
        canStopPlayback(
          turnInFlight: false,
          audioPending: false,
          cuesPending: false,
        ),
        isFalse,
        reason: '三者都没有 ⇒ 不可按（不摆按下去什么都不做的假出路）',
      );
    });

    test('空闲时按下停止：只取消，不调 ChatController.stop', () async {
      final List<String> calls = <String>[];
      await stopPlayback(
        turnInFlight: false,
        reason: 'stop',
        cancelStage: (String reason) => calls.add('cancel:$reason'),
        stopTurn: () async => calls.add('stop-turn'),
      );
      expect(
        calls,
        <String>['cancel:stop'],
        reason: 'ChatController.stop 是唯一会收口本轮 / 写「已停止本轮」/ 落盘 / '
            'POST /chat/stop 的入口；空闲时它一次都不该被调到',
      );
    });

    test('这一轮还在飞时按下停止：先取消、再中止本轮（顺序固定）', () async {
      final List<String> calls = <String>[];
      await stopPlayback(
        turnInFlight: true,
        reason: 'stop',
        cancelStage: (String reason) => calls.add('cancel:$reason'),
        stopTurn: () async => calls.add('stop-turn'),
      );
      expect(calls, <String>['cancel:stop', 'stop-turn']);
    });

    test('接线（语义级）：新键接 _stopPlayback，判据读三条队列', () {
      final String main = stripCommentsKeepStrings(
        readLibrarySource('lib/main.dart'),
      );
      final String wiring = stripCommentsAndStrings(
        readLibrarySource('lib/app/shell_cue_voice_wiring.dart'),
      );
      final String audio = stripCommentsAndStrings(
        readLibrarySource('lib/audio/audio_player.dart'),
      );
      expect(
        main.contains('final bool canStop = canStopPlayback('),
        isTrue,
        reason: '可按性由纯函数算（不在 UI 层再判一次相位）',
      );
      expect(
        main.contains('canStopPlayback: canStop,'),
        isTrue,
        reason: '圆键与 Esc 拿的是同一个局部，判据不散成两处',
      );
      expect(
        main.contains('onStopPlayback: () => unawaited(_stopPlayback())'),
        isTrue,
        reason: '圆键停止态接的是「丢队列」那条路，不是 _stopWithCancellation',
      );
      expect(
        wiring,
        contains('turnInFlight: _chat.streaming'),
        reason: '是否顺带中止本轮，按「这一轮还在飞」分流',
      );
      expect(wiring, contains('stopTurn: _stop'));
      expect(
        audio,
        contains('bool get hasPending => !_queue.isEmpty;'),
        reason: 'hasPending 只读队列事实——不改播放 / 口型 / WAV',
      );
    });
  });
}

/// `anchor`（形如 `onRetryLast:`）起到这条实参结束（配平后的逗号）为止。
///
/// `dart format` 会把三元表达式折行。判据仍是「这条出口接的是哪条路」，
/// 不要求写在同一行，也不比对整段字面量——见文件头注。
String _argLine(String src, String anchor) {
  final int at = src.indexOf(anchor);
  expect(at, greaterThanOrEqualTo(0), reason: '找不到 `$anchor` 接线');
  var depth = 0;
  for (var i = at; i < src.length; i++) {
    final String c = src[i];
    if (c == '(' || c == '[' || c == '{') {
      depth++;
    } else if (c == ')' || c == ']' || c == '}') {
      if (depth == 0) break;
      depth--;
    } else if (c == ',' && depth == 0) {
      return src.substring(at, i);
    }
  }
  final int end = src.indexOf('\n', at);
  return src.substring(at, end < 0 ? src.length : end);
}

/// [needle] 在 [src] 里出现几次（钉「唯一出口」用）。
int _countOccurrences(String src, String needle) {
  var count = 0;
  var at = 0;
  while (true) {
    final int i = src.indexOf(needle, at);
    if (i < 0) return count;
    count++;
    at = i + needle.length;
  }
}

/// 取 [signature] 那个函数的**函数体**（含花括号）。
String _functionBody(String src, String signature) {
  final int at = src.indexOf(signature);
  expect(at, greaterThanOrEqualTo(0), reason: '找不到 `$signature`');
  final int brace = src.indexOf('{', at);
  final int end = matchBracket(src, brace, '{', '}');
  expect(end, greaterThan(brace), reason: '`$signature` 的花括号不平衡');
  return src.substring(brace, end + 1);
}
